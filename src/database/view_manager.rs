// view_manager.rs
//
// The view manager. This should hold all the functions
// needed for the core data model of denViews to function
// properly. The only two things here that require outright
// authentication/higher permissions should be the Flush
// and Init operations.

use super::util;
use super::{DenViewSettings, ViewRecord};
use crate::Error;
use bb8::Pool;
use bb8_postgres::{
    tokio_postgres::{config::Config, NoTls, Row},
    PostgresConnectionManager,
};
use crypto::digest::Digest;
use crypto::sha3::Sha3;
use std::time::SystemTime;

pub struct ViewManager {
    db_pool: Pool<PostgresConnectionManager<NoTls>>,
}

#[derive(Debug)]
pub enum ViewManagerOperation<'a> {
    // GET: Gets a page's views by string path.
    // If the record does not exist, this will always return an error.
    Get(&'a str),

    // UPDATE: Updates a page's views by string path.
    //
    // If the record does not exist, this will always return an error.
    // Records should be tested for correctness before calling it
    // into the database.
    //
    // This will, as of v0.1, only increment views.
    UpdatePage(&'a str, &'a str),

    /*
    // CREATE: Creates a new page in the database.
    //
    // This is to resolve the two errors from above. Checking for
    // correctness must be done by the caller.
    CreatePage(String),
    */
    // FLUSH: This flushes the page_visitors table to the database,
    // calculating all required values and adding them to the record,
    // before deleting all related records from the page_visitors table.
    //
    // This level of denormalization is required for performance, as otherwise
    // you would have to deal with querying n rows for several pages
    // in the worst case (it is practically O(n) to calculate views from the
    // database from page_visitors alone).
    //
    // Due to the length of time it could take to flush records to database,
    // compared to fetching/updating, this should only be done by authorized
    // clients/callers in order to ensure that the database is not overloaded
    // with concurrent transactions.
    Flush,
}

impl Default for DenViewSettings {
    fn default() -> Self {
        DenViewSettings {
            site: "localhost".into(),
            use_https: true,
            ignore_queries: true,
            remove_index_pages: true,
        }
    }
}

impl ViewManager {
    pub async fn new(pool_size: u32, config: Config) -> Result<Self, Error> {
        Ok(ViewManager {
            db_pool: Pool::builder()
                .max_size(pool_size)
                .build(PostgresConnectionManager::new(config, NoTls))
                .await?,
        })
    }

    pub async fn execute(
        &self,
        op: &ViewManagerOperation<'_>,
    ) -> Result<Option<ViewRecord>, Error> {
        match op {
            ViewManagerOperation::Get(path) => Ok(Some(self.get_page_info(path).await?)),
            ViewManagerOperation::UpdatePage(path, info) => {
                self.append_visitor(path, info).await?;
                Ok(None)
            }
            /*
            DatabaseOperation::CreatePage(path) => {
                self.create_page(path)?;
                Ok(None)
            },
            */
            ViewManagerOperation::Flush => {
                self.flush().await?;
                Ok(None)
            } /*
              ViewManagerOperation::Init(s) => {
                  self.init(s).await?;
                  Ok(None)
              }
              */
        }
    }

    async fn get_page_info(&self, path: &str) -> Result<ViewRecord, Error> {
        let conn = self.db_pool.get().await?;

        // this should ONLY RETURN a single row, ALWAYS
        // any other result is completely wrong

        let path_id: i32 = conn
            .query_one("SELECT path_id FROM paths WHERE path = $1", &[&path])
            .await?
            .get(0);

        // This is only safe because of the path_id abstraction that occurs.
        // This should not be replicated in any other circumstance.
        let record = conn
            .query_one(
                format!("SELECT view_count, hit_count FROM path_{}", path_id).as_str(),
                &[],
            )
            .await?;

        Ok(ViewRecord {
            page: path.to_string(),
            views: record.get(0),
            hits: record.get(1),
        })
    }

    async fn append_visitor(&self, path: &str, visitor_info: &str) -> Result<(), Error> {
        log::debug!("recording new visitor");
        let conn = self.db_pool.get().await?;

        // either get a page ID, or create it
        let page_id: i32 = conn
            .query_opt(
                "
                SELECT page_id
                FROM pages
                WHERE path_id = (
                        SELECT path_id
                        FROM paths
                        WHERE path = $1
                    )
                ",
                &[&path],
            )
            .await?
            .unwrap_or(self.create_page(path).await?)
            .get(0);

        let mut hasher = Sha3::sha3_256();
        let salt: String = conn.query_one("SELECT salt FROM salt", &[]).await?.get(0);
        hasher.input_str(&(visitor_info.to_string() + &salt));
        let visitor_hash = hasher.result_str();
        println!("{}", visitor_hash);

        // optional! this is because if the visitor doesn't already exist, it is instead
        // added into the visitors table
        let visitor = conn
            .query_opt(
                "SELECT visitor_id FROM visitors WHERE visitor_id = $1",
                &[&visitor_hash],
            )
            .await?;

        if let Some(v) = visitor {
            let id: String = v.get(0);
            let page_visitor = conn
                .query_opt(
                    "
                    SELECT
                        visitor_id,
                        page_id,
                        visitor_hits
                    FROM page_visitors
                    WHERE visitor_id = $1 AND page_id = $2
                    ",
                    &[&id, &page_id],
                )
                .await?;

            if let Some(p) = page_visitor {
                let hits: i32 = p.get(2);
                conn.execute(
                    "
                    UPDATE page_visitors
                    SET visitor_hits = $3
                    WHERE visitor_id = $1 AND page_id = $2
                    ",
                    &[&id, &page_id, &(hits + 1)],
                )
                .await?;
            } else {
                conn.execute(
                    "INSERT INTO page_visitors (visitor_id, page_id) VALUES ($1, $2)",
                    &[&id, &page_id],
                )
                .await?;
            }
        } else {
            conn.execute(
                "INSERT INTO visitors (visitor_id) VALUES ($1)",
                &[&visitor_hash],
            )
            .await?;
            conn.execute(
                "INSERT INTO page_visitors (visitor_id, page_id) VALUES ($1, $2)",
                &[&visitor_hash, &page_id],
            )
            .await?;
        }

        Ok(())
    }

    // This will create a page record, as well as any path categories
    // that eventually lead to the page record itself.
    //
    // If the path length is one, however, it will just create a page record,
    // and assume that the path category is the root of the website.
    async fn create_page(&self, path: &str) -> Result<Row, Error> {
        log::info!("inserting {} into database now...", path);
        let conn = self.db_pool.get().await?;

        let row = conn
            .query_opt(
                "SELECT * FROM pages WHERE path_id = (SELECT path_id FROM paths WHERE path = $1)",
                &[&path],
            )
            .await?;

        if let Some(r) = row {
            return Ok(r);
        }

        let path_id: i32 = conn
            .query_one(
                "INSERT INTO paths (path) VALUES ($1) RETURNING path_id",
                &[&path],
            )
            .await?
            .get(0);

        let parts = match path.len() {
            0 => vec![""],
            _ => path.split('/').collect::<Vec<&str>>(),
        };
        /*
        if parts.len() == 1 {
                conn.execute(
                "INSERT INTO pages VALUES (0, $1, $2, $3)",
                &[&path_id.to_string(), &parts[0], &"date"],
            )?;
            return Ok(());
        }
        */

        let mut last_part_id = 0i32;
        for part in parts[..parts.len() - 1].iter() {
            let folder: Option<Row> = conn
                .query_opt(
                    "
                SELECT
                    folder_id,
                    parent_id
                FROM folders
                WHERE folder_name = $1 AND parent_id = $2
                ",
                    &[&part, &last_part_id],
                )
                .await?;

            if let Some(r) = folder {
                last_part_id = r.get(0);
                continue;
            }

            last_part_id = conn
                .query_one(
                    "INSERT INTO folders (folder_name, parent_id) VALUES ($1, $2) RETURNING folder_id",
                    &[&part, &last_part_id],
                ).await?
                .get(0);
        }

        let row = conn
            .query_one(
                "
            INSERT INTO
                pages (folder_id, path_id, page_name, first_visited)
            VALUES
                ($1, $2, $3, $4)
            RETURNING
                page_id",
                &[
                    &last_part_id,
                    &path_id,
                    &parts[parts.len() - 1],
                    &SystemTime::now(),
                ],
            )
            .await?;

        conn.execute(
            format!(
                "
                CREATE VIEW
                    path_{} (page_id, view_count, hit_count)
                AS
                    SELECT
                        view_count.page_id,
                        view_count.views + total_views,
                        view_count.hits + total_hits
                    FROM (
                        SELECT
                            page_id,
                            COUNT(visitor_id) AS views,
                            SUM(visitor_hits) AS hits
                        FROM page_visitors
                        WHERE page_id = {}
                        GROUP BY page_id
                        UNION ALL
                        SELECT {1}, 0, 0) AS view_count
                    LEFT JOIN pages
                    ON view_count.page_id = pages.page_id
                    LIMIT 1
                ",
                path_id,
                row.get::<usize, i32>(0),
            )
            .as_str(),
            &[],
        )
        .await?;

        Ok(row)
    }

    async fn flush(&self) -> Result<(), Error> {
        log::info!("flushing page_visitors to database now...");
        let mut conn = self.db_pool.get().await?;

        let views = conn.query("SELECT * FROM total_views", &[]).await?;
        let transaction = conn.transaction().await?;
        for page in views {
            let id: i32 = page.get(0);
            let views: i64 = page.get(1);
            let hits: i64 = page.get(2);

            transaction
                .execute(
                    "
                UPDATE pages
                SET
                    total_views = $1,
                    total_hits = $2
                WHERE page_id = $3
                ",
                    &[&views, &hits, &id],
                )
                .await?;

            transaction
                .execute(
                    "
                DELETE FROM page_visitors
                WHERE page_id = $1
                ",
                    &[&id],
                )
                .await?;
        }

        let salt = util::create_salt();

        transaction.execute("DELETE FROM salt", &[]).await?;
        transaction
            .execute("INSERT INTO salt (salt) VALUES ($1)", &[&salt])
            .await?;

        transaction.execute("DELETE FROM visitors", &[]).await?;
        transaction.commit().await?;

        Ok(())
    }

    pub async fn get_settings(&self) -> Result<DenViewSettings, Error> {
        let conn = self.db_pool.get().await?;

        let settings = conn
            .query_opt(
                "SELECT setting FROM settings WHERE setting_name = 'current_settings'",
                &[],
            )
            .await;

        match settings {
            Err(_) => Ok(DenViewSettings::default()),
            Ok(v) => match v {
                Some(s) => Ok(serde_json::from_value(s.get(0))?),
                None => Ok(DenViewSettings::default()),
            },
        }
    }
}
