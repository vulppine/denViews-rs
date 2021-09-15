use super::util::base64::*;
use super::Error;
use crypto::digest::Digest;
use crypto::sha3::Sha3;
use bb8::Pool;
use bb8_postgres::{
    tokio_postgres::{config::Config, NoTls, Row},
    PostgresConnectionManager,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::time::SystemTime;

pub struct DatabaseClient {
    db_pool: Pool<PostgresConnectionManager<NoTls>>,
}

#[derive(Debug)]
pub enum DatabaseOperation<'a> {
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

    Init
}

#[derive(serde::Serialize)]
pub struct ViewRecord {
    pub page: String,
    pub views: i64,
    pub hits: i64,
}

#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub port: serde_json::value::Value,
    pub site: String,
    pub use_https: bool,
    pub ignore_queries: bool,
    pub remove_index_pages: bool
}

impl Default for DatabaseSettings {
    fn default() -> Self {
        DatabaseSettings {
            port: 36621.into(),
            site: "localhost".into(),
            use_https: false, // not until the dashboard is implemented
            ignore_queries: true,
            remove_index_pages: true,
        }
    }
}

impl DatabaseClient {
    pub async fn new(pool_size: u32, config: Config) -> Result<Self, Error> {
        Ok(DatabaseClient {
            db_pool: Pool::builder()
                .max_size(pool_size)
                .build(PostgresConnectionManager::new(config, NoTls)).await?,
        })
    }

    pub async fn execute(&self, op: &DatabaseOperation<'_>) -> Result<Option<ViewRecord>, Error> {
        match op {
            DatabaseOperation::Get(path) => Ok(Some(self.get_page_info(path).await?)),
            DatabaseOperation::UpdatePage(path, info) => {
                self.append_visitor(path, info).await?;
                Ok(None)
            }
            /*
            DatabaseOperation::CreatePage(path) => {
                self.create_page(path)?;
                Ok(None)
            },
            */
            DatabaseOperation::Flush => {
                self.flush().await?;
                Ok(None)
            },
            DatabaseOperation::Init => {
                self.init().await?;
                Ok(None)
            }
        }
    }

    async fn get_page_info(&self, path: &str) -> Result<ViewRecord, Error> {
        let conn = self.db_pool.get().await?;

        // this should ONLY RETURN a single row, ALWAYS
        // any other result is completely wrong

        let path_id: i32 = conn
            .query_one("SELECT path_id FROM paths WHERE path = $1", &[&path]).await?
            .get(0);

        // This is only safe because of the path_id abstraction that occurs.
        // This should not be replicated in any other circumstance.
        let record = conn.query_one(
            format!("SELECT view_count, hit_count FROM path_{}", path_id).as_str(),
            &[],
        ).await?;

        Ok(ViewRecord {
            page: path.to_string(),
            views: record.get(0),
            hits: record.get(1),
        })
    }

    async fn append_visitor(&self, path: &str, visitor_info: &str) -> Result<(), Error> {
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
            ).await?
            .unwrap_or(self.create_page(path).await?)
            .get(0);

        let mut hasher = Sha3::sha3_256();
        let salt: String = conn.query_one("SELECT salt FROM salt", &[]).await?.get(0);
        hasher.input_str(&(visitor_info.to_string() + &salt));
        let visitor_hash = hasher.result_str();
        println!("{}", visitor_hash);

        // optional! this is because if the visitor doesn't already exist, it is instead
        // added into the visitors table
        let visitor = conn.query_opt(
            "SELECT visitor_id FROM visitors WHERE visitor_id = $1",
            &[&visitor_hash],
        ).await?;

        if let Some(v) = visitor {
            let id: String = v.get(0);
            let page_visitor = conn.query_opt(
                "
                    SELECT
                        visitor_id,
                        page_id,
                        visitor_hits
                    FROM page_visitors
                    WHERE visitor_id = $1 AND page_id = $2
                    ",
                &[&id, &page_id],
            ).await?;

            if let Some(p) = page_visitor {
                let hits: i32 = p.get(2);
                conn.execute(
                    "
                    UPDATE page_visitors
                    SET visitor_hits = $3
                    WHERE visitor_id = $1 AND page_id = $2
                    ",
                    &[&id, &page_id, &(hits + 1)],
                ).await?;
            } else {
                conn.execute(
                    "INSERT INTO page_visitors (visitor_id, page_id) VALUES ($1, $2)",
                    &[&id, &page_id],
                ).await?;
            }
        } else {
            conn.execute(
                "INSERT INTO visitors (visitor_id) VALUES ($1)",
                &[&visitor_hash],
            ).await?;
            conn.execute(
                "INSERT INTO page_visitors (visitor_id, page_id) VALUES ($1, $2)",
                &[&visitor_hash, &page_id],
            ).await?;
        }

        Ok(())
    }

    // This will create a page record, as well as any path categories
    // that eventually lead to the page record itself.
    //
    // If the path length is one, however, it will just create a page record,
    // and assume that the path category is the root of the website.
    async fn create_page(&self, path: &str) -> Result<Row, Error> {
        let conn = self.db_pool.get().await?;

        let row = conn.query_opt(
            "SELECT * FROM pages WHERE path_id = (SELECT path_id FROM paths WHERE path = $1)",
            &[&path],
        ).await?;

        if let Some(r) = row {
            return Ok(r);
        }

        let path_id: i32 = conn
            .query_one(
                "INSERT INTO paths (path) VALUES ($1) RETURNING path_id",
                &[&path],
            ).await?
            .get(0);

        let parts = match path.len() {
            0 => vec![""],
            _ => path.split('/').collect::<Vec<&str>>()
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
            let folder: Option<Row> = conn.query_opt(
                "
                SELECT
                    folder_id,
                    parent_id
                FROM folders
                WHERE folder_name = $1 AND parent_id = $2
                ",
                &[&part, &last_part_id],
            ).await?;

            if let Some(r) = folder {
                last_part_id = r.get(0);
                continue;
            }

            last_part_id = conn
                .query_one(
                    "INSERT INTO folders (folder_name) VALUES ($1) RETURNING folder_id",
                    &[&part],
                ).await?
                .get(0);
        }

        let row = conn.query_one(
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
        ).await?;

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
        ).await?;

        Ok(row)
    }

    async fn flush(&self) -> Result<(), Error> {
        let mut conn = self.db_pool.get().await?;

        let views = conn.query("SELECT * FROM total_views", &[]).await?;
        let transaction = conn.transaction().await?;

        for page in views {
            let id: i32 = page.get(0);
            let views: i64 = page.get(1);
            let hits: i64 = page.get(2);

            transaction.execute(
                "
                UPDATE pages
                SET
                    total_views = $1,
                    total_hits = $2
                WHERE page_id = $3
                ",
                &[&views, &hits, &id],
            ).await?;

            transaction.execute(
                "
                DELETE FROM page_visitors
                WHERE page_id = $1
                ",
                &[&id],
            ).await?;
        }

        let mut rng = StdRng::from_entropy();
        let mut salt_raw: [u8; 32] = [0; 32];
        rng.fill(&mut salt_raw[..]);
        let salt = bytes_to_base64(salt_raw.to_vec());

        transaction.execute("DELETE FROM salt", &[]).await?;
        transaction.execute("INSERT INTO salt (salt) VALUES ($1)", &[&salt]).await?;

        transaction.execute("DELETE FROM visitors", &[]).await?;
        transaction.commit().await?;

        Ok(())
    }

    pub async fn get_settings(&self) -> Result<DatabaseSettings, Error> {
        let conn = self.db_pool.get().await?;

        let port: serde_json::Value = conn.query_one("SELECT * FROM settings WHERE setting_name = 'port'", &[]).await?.get(1);
        let site: serde_json::Value = conn.query_one("SELECT * FROM settings WHERE setting_name = 'site'", &[]).await?.get(1);
        let ignore_queries: serde_json::Value = conn.query_one("SELECT * FROM settings WHERE setting_name = 'ignore_queries'", &[]).await?.get(1);
        let remove_index_pages: serde_json::Value = conn.query_one("SELECT * FROM settings WHERE setting_name = 'remove_index_pages'", &[]).await?.get(1);


        Ok(DatabaseSettings {
            port,
            site: site.as_str().unwrap().to_string(),
            use_https: false, // not until the dashboard is implemented!!! seriously!!!
            ignore_queries: ignore_queries.as_bool().unwrap(),
            remove_index_pages: remove_index_pages.as_bool().unwrap()
        })
    }

    async fn init(&self) -> Result<(), Error> {
        let mut conn = self.db_pool.get().await?;

        let transaction = conn.transaction().await?;

        transaction.execute(
            "
            CREATE TABLE folders (
                folder_id INT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
                parent_id INT,
                folder_name TEXT NOT NULL,
                CONSTRAINT parent_folder
                    FOREIGN KEY (parent_id)
                    REFERENCES folders (folder_id)
            )
            ",
            &[],
        ).await?;

        transaction.execute(
            "
            CREATE TABLE paths (
                path_id INT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
                path TEXT NOT NULL
            )
            ",
            &[],
        ).await?;

        transaction.execute(
            "
            CREATE TABLE pages (
                page_id INT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
                folder_id INT NOT NULL,
                path_id INT NOT NULL,
                page_name TEXT NOT NULL,
                first_visited TIMESTAMP,
                total_views BIGINT NOT NULL DEFAULT 0,
                total_hits BIGINT NOT NULL DEFAULT 0,
                CONSTRAINT page_folder
                    FOREIGN KEY (folder_id)
                    REFERENCES folders (folder_id),
                CONSTRAINT page_path
                    FOREIGN KEY (path_id)
                    REFERENCES paths (path_id)
            )
            ",
            &[],
        ).await?;

        transaction.execute(
            "
            CREATE TABLE visitors (
                visitor_id TEXT PRIMARY KEY
            )
            ",
            &[],
        ).await?;

        transaction.execute(
            "
            CREATE TABLE page_visitors (
                page_id INT NOT NULL,
                visitor_id TEXT NOT NULL,
                visitor_hits INT NOT NULL DEFAULT 1,
                CONSTRAINT fk_page
                    FOREIGN KEY (page_id)
                    REFERENCES pages (page_id),
                CONSTRAINT fk_visitor
                    FOREIGN KEY (visitor_id)
                    REFERENCES visitors (visitor_id)
            )
            ",
            &[],
        ).await?;

        transaction.execute(
            "
            CREATE VIEW
                total_views (page_id, view_count, hit_count)
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
                    GROUP BY page_id) AS view_count
                LEFT JOIN pages
                ON view_count.page_id = pages.page_id
            ",
            &[],
        ).await?;

        transaction.execute("CREATE TABLE salt (salt TEXT NOT NULL)", &[]).await?;
        let mut rng = StdRng::from_entropy();
        let mut salt_raw: [u8; 32] = [0; 32];
        rng.fill(&mut salt_raw[..]);
        let salt = bytes_to_base64(salt_raw.to_vec());

        transaction.execute("INSERT INTO salt (salt) VALUES ($1)", &[&salt]).await?;

        transaction.execute(
            "
            CREATE TABLE settings (
                setting_name TEXT PRIMARY KEY,
                setting JSON
            )
            ",
            &[]
        ).await?;
        transaction.execute("INSERT INTO settings VALUES ('schema_ver', '1'::JSON)", &[]).await?;
        transaction.execute("INSERT INTO settings VALUES ('ignore_queries', 'false')", &[]).await?;
        transaction.execute("INSERT INTO settings VALUES ('remove_index_pages', 'true')", &[]).await?;
        transaction.execute("INSERT INTO settings VALUES ('site', '\"localhost\"')", &[]).await?;
        transaction.execute("INSERT INTO settings VALUES ('port', '36621'::JSON)", &[]).await?;

        transaction.commit().await?;

        Ok(())
    }
}
