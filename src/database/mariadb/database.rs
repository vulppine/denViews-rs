// view_manager.rs
//
// The view manager. This should hold all the functions
// needed for the core data model of denViews to function
// properly. The only two things here that require outright
// authentication/higher permissions should be the Flush
// and Init operations.

use crate::database::util;
use crate::database::{Database, DatabaseOperation};
use crate::database::{DenViewSettings, ViewRecord};
use crate::Error;
use bb8::Pool;
use bb8_postgres::{
    tokio_postgres::{config::Config as PostgresConfig, NoTls},
    PostgresConnectionManager,
};
use chrono::{offset::Utc, DateTime};
use crypto::digest::Digest;
use crypto::sha3::Sha3;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use sqlx::{mysql, ConnectOptions, Row};
use std::time::SystemTime;

pub struct MariaDB {
    db_pool: mysql::MySqlPool,
}

impl MariaDB {
    pub async fn new() -> Result<Self, Error> {
        let user = std::env::var("DENVIEWS_USER").unwrap_or_else(|_| "denviews".to_string());
        let pass = std::env::var("DENVIEWS_PASS").unwrap_or_else(|_| "denviews".to_string());
        let host = std::env::var("DENVIEWS_HOST").unwrap_or_else(|_| "localhost".to_string());
        let pool_amount = std::env::var("DENVIEWS_POOL_AMOUNT")
            .unwrap_or_else(|_| "16".to_string())
            .parse::<u32>()?;

        let init_conn = mysql::MySqlConnectOptions::new()
            .host(&host)
            .username(&user)
            .password(&pass)
            .database("denviews");

        let db_pool = mysql::MySqlPoolOptions::new()
            .max_connections(pool_amount)
            .connect_with(init_conn)
            .await?;

        sqlx::query("SET SESSION sql_mode = 'NO_AUTO_VALUE_ON_ZERO'")
            .execute(&db_pool)
            .await?;

        Ok(MariaDB { db_pool })
    }
}

#[async_trait::async_trait]
impl Database for MariaDB {
    async fn execute(&self, op: &DatabaseOperation<'_>) -> Result<Option<ViewRecord>, Error> {
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
            } /*
              DatabaseOperation::Init(s) => {
                  self.init(s).await?;
                  Ok(None)
              }
              */
        }
    }

    async fn get_settings(&self) -> Result<DenViewSettings, Error> {
        let settings =
            sqlx::query("SELECT setting FROM settings WHERE setting_name = 'current_settings'")
                .fetch_optional(&self.db_pool)
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

impl MariaDB {
    async fn get_page_info(&self, path: &str) -> Result<ViewRecord, Error> {
        // this should ONLY RETURN a single row, ALWAYS
        // any other result is completely wrong

        let path_id: i32 = sqlx::query("SELECT path_id FROM paths WHERE path = ?")
            .bind(&path)
            .fetch_one(&self.db_pool)
            .await?
            .get(0);

        // This is only safe because of the path_id abstraction that occurs.
        // This should not be replicated in any other circumstance.
        let record =
            sqlx::query(format!("SELECT view_count, hit_count FROM path_{}", path_id).as_str())
                .fetch_one(&self.db_pool)
                .await?;

        Ok(ViewRecord {
            page: path.to_string(),
            views: record.get(0),
            hits: record.get::<Decimal, usize>(1).to_i64().unwrap(),
        })
    }

    async fn append_visitor(&self, path: &str, visitor_info: &str) -> Result<(), Error> {
        log::debug!("recording new visitor");
        // either get a page ID, or create it
        let page_id: i32 = sqlx::query(
            "
            SELECT page_id
            FROM pages
            WHERE path_id = (
                    SELECT path_id
                    FROM paths
                    WHERE path = ?
                )
            ",
        )
        .bind(&path)
        .fetch_optional(&self.db_pool)
        .await?
        .unwrap_or(self.create_page(path).await?)
        .get(0);

        let mut hasher = Sha3::sha3_256();
        let salt: String = sqlx::query("SELECT salt FROM salt")
            .fetch_one(&self.db_pool)
            .await?
            .get(0);
        hasher.input_str(&(visitor_info.to_string() + &salt));
        let visitor_hash = hasher.result_str();
        println!("{}", visitor_hash);

        // optional! this is because if the visitor doesn't already exist, it is instead
        // added into the visitors table
        let visitor = sqlx::query("SELECT visitor_id FROM visitors WHERE visitor_id = ?")
            .bind(&visitor_hash)
            .fetch_optional(&self.db_pool)
            .await?;

        if let Some(v) = visitor {
            let id: String = v.get(0);
            let page_visitor = sqlx::query(
                "
                    SELECT
                        visitor_id,
                        page_id,
                        visitor_hits
                    FROM page_visitors
                    WHERE visitor_id = ? AND page_id = ?
                    ",
            )
            .bind(&id)
            .bind(&page_id)
            .fetch_optional(&self.db_pool)
            .await?;

            if let Some(p) = page_visitor {
                let hits: i32 = p.get(2);
                sqlx::query(
                    "
                    UPDATE page_visitors
                    SET visitor_hits = ?
                    WHERE visitor_id = ? AND page_id = ?
                    ",
                )
                .bind(&(hits + 1))
                .bind(&id)
                .bind(&page_id)
                .execute(&self.db_pool)
                .await?;
            } else {
                sqlx::query("INSERT INTO page_visitors (visitor_id, page_id) VALUES (?, ?)")
                    .bind(&id)
                    .bind(&page_id)
                    .execute(&self.db_pool)
                    .await?;
            }
        } else {
            sqlx::query("INSERT INTO visitors (visitor_id) VALUES (?)")
                .bind(&visitor_hash)
                .execute(&self.db_pool)
                .await?;
            sqlx::query("INSERT INTO page_visitors (visitor_id, page_id) VALUES (?, ?)")
                .bind(&visitor_hash)
                .bind(&page_id)
                .execute(&self.db_pool)
                .await?;
        }

        Ok(())
    }

    // This will create a page record, as well as any path categories
    // that eventually lead to the page record itself.
    //
    // If the path length is one, however, it will just create a page record,
    // and assume that the path category is the root of the website.
    async fn create_page(&self, path: &str) -> Result<mysql::MySqlRow, Error> {
        log::info!("inserting {} into database now...", path);

        let row = sqlx::query(
            "SELECT * FROM pages WHERE path_id = (SELECT path_id FROM paths WHERE path = ?)",
        )
        .bind(&path)
        .fetch_optional(&self.db_pool)
        .await?;

        if let Some(r) = row {
            return Ok(r);
        }

        let path_id: i32 = sqlx::query("INSERT INTO paths (path) VALUES (?) RETURNING path_id")
            .bind(&path)
            .fetch_one(&self.db_pool)
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
            let folder: Option<mysql::MySqlRow> = sqlx::query(
                "
                SELECT
                    folder_id,
                    parent_id
                FROM folders
                WHERE folder_name = ? AND parent_id = ?
                ",
            )
            .bind(&part)
            .bind(&last_part_id)
            .fetch_optional(&self.db_pool)
            .await?;

            if let Some(r) = folder {
                last_part_id = r.get(0);
                continue;
            }

            last_part_id = sqlx::query(
                "INSERT INTO folders (folder_name, parent_id) VALUES (?, ?) RETURNING folder_id",
            )
            .bind(&part)
            .bind(&last_part_id)
            .fetch_one(&self.db_pool)
            .await?
            .get(0);
        }

        let row = sqlx::query(
            "
            INSERT INTO
                pages (folder_id, path_id, page_name, first_visited)
            VALUES
                (?, ?, ?, ?)
            RETURNING
                page_id
            ",
        )
        .bind(&last_part_id)
        .bind(&path_id)
        .bind(&parts[parts.len() - 1])
        .bind(DateTime::<Utc>::from(SystemTime::now()))
        .fetch_one(&self.db_pool)
        .await?;

        sqlx::query(
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
                row.get::<i32, usize>(0),
            )
            .as_str(),
        )
        .execute(&self.db_pool)
        .await?;

        Ok(row)
    }

    async fn flush(&self) -> Result<(), Error> {
        log::info!("flushing page_visitors to database now...");

        let views = sqlx::query("SELECT * FROM total_views")
            .fetch_all(&self.db_pool)
            .await?;
        let mut transaction = self.db_pool.begin().await?;
        for page in views {
            let id: i32 = page.get(0);
            let views: i64 = page.get(1);
            let hits: i64 = page.get::<Decimal, usize>(2).to_i64().unwrap();

            sqlx::query(
                "
                UPDATE pages
                SET
                    total_views = ?,
                    total_hits = ?
                WHERE page_id = ?
                ",
            )
            .bind(&views)
            .bind(&hits)
            .bind(&id)
            .execute(&mut transaction)
            .await?;

            sqlx::query(
                "
                DELETE FROM page_visitors
                WHERE page_id = ?
                ",
            )
            .bind(&id)
            .execute(&mut transaction)
            .await?;
        }

        let salt = util::create_salt();

        sqlx::query("DELETE FROM salt")
            .execute(&mut transaction)
            .await?;
        sqlx::query("INSERT INTO salt (salt) VALUES (?)")
            .bind(&salt)
            .execute(&mut transaction)
            .await?;

        sqlx::query("DELETE FROM visitors")
            .execute(&mut transaction)
            .await?;
        transaction.commit().await?;

        Ok(())
    }
}
