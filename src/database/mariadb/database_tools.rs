use crate::database::*;
use crate::Error;
use bb8::Pool;
use bb8_postgres::{tokio_postgres::NoTls, PostgresConnectionManager};
use crypto::digest::Digest;
use crypto::sha3::Sha3;
use sqlx::{mysql, ConnectOptions, Row};

pub struct MariaDBDatabaseTools {
    db_pool: mysql::MySqlPool,
}

impl MariaDBDatabaseTools {
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

        Ok(MariaDBDatabaseTools {
            db_pool: mysql::MySqlPoolOptions::new()
                .max_connections(pool_amount)
                .connect_with(init_conn)
                .await?,
        })
    }
}

#[async_trait::async_trait]
impl DatabaseTool for MariaDBDatabaseTools {
    async fn check(&self) -> Result<bool, Error> {
        //TODO: More indepth method of actually checking for a valid database.
        // At the moment, this is just to ensure that the database has a *settings* file,
        // and isn't some kind of validation check.
        let check = sqlx::query("SELECT setting FROM settings WHERE setting_name = 'schema_ver'")
            .fetch_optional(&self.db_pool)
            .await;

        Ok(match check {
            Err(e) => {
                println!("An error occurred while verifying the database. Returning is_init: false, in case it is not initialized. Error: {}", e);
                false
            }
            Ok(r) => r.is_some(),
        })
    }

    async fn get_folder(&self, folder_id: i32) -> Result<FolderRecord, Error> {
        let mut pages: Vec<ViewRecord> = Vec::new();
        let mut folders: Vec<FolderRecordPartial> = Vec::new();

        let folder = sqlx::query(
            "
            SELECT folder_name, parent_id
            FROM folders
            WHERE folder_id = ?
            ",
        )
        .bind(&folder_id)
        .fetch_one(&self.db_pool)
        .await?;

        let folder_name: String = folder.get(0);
        let folder_parent: Option<i32> = folder.get(1);

        let mut page_rows = sqlx::query(
            "
            SELECT page_name, path_id
            FROM pages
            INNER JOIN folders
            ON pages.folder_id = folders.folder_id
            WHERE folders.folder_id = ?
            ",
        )
        .bind(&folder_id)
        .fetch_all(&self.db_pool)
        .await?;

        if let Some(id) = folder_parent {
            if let Some(r) = sqlx::query(
                "
                SELECT '###self###' AS page_name, path_id
                FROM pages
                WHERE folder_id = ? AND page_name = ?
                ",
            )
            .bind(&id)
            .bind(&folder_name)
            .fetch_optional(&self.db_pool)
            .await?
            {
                page_rows.push(r);
            }
        }

        for row in page_rows {
            let path_id: i32 = row.get(1);

            let views = sqlx::query(
                format!(
                    "
                SELECT view_count, hit_count
                FROM path_{}
                ",
                    path_id
                )
                .as_str(),
            )
            .fetch_one(&self.db_pool)
            .await?;

            pages.push(ViewRecord {
                page: row.get(0),
                views: views.get(0),
                hits: views.get(1),
            });
        }

        let folder_rows = sqlx::query(
            "
            SELECT folder_id, folder_name
            FROM folders
            WHERE parent_id = ?
            ",
        )
        .bind(&folder_id)
        .fetch_all(&self.db_pool)
        .await?;

        for folder in folder_rows {
            folders.push(FolderRecordPartial {
                id: folder.get(0),
                name: folder.get(1),
            });
        }

        Ok(FolderRecord {
            id: folder_id,
            parent_id: folder_parent,
            name: folder_name,
            folders,
            pages,
        })
    }

    async fn get_page(&self, folder_id: i32, page_name: String) -> Result<PageRecord, Error> {
        let page = sqlx::query(
            "
            SELECT path_id, page_id, folder_id
            FROM pages
            WHERE folder_id = ? AND page_name = ?
            ",
        )
        .bind(&folder_id)
        .bind(&page_name)
        .fetch_one(&self.db_pool)
        .await?;
        let path_id: i32 = page.get(0);

        let page_views = sqlx::query(
            format!(
                "
            SELECT view_count, hit_count
            FROM path_{}
            ",
                path_id
            )
            .as_str(),
        )
        .fetch_one(&self.db_pool)
        .await?;

        Ok(PageRecord {
            id: page.get(1),
            path_id,
            folder_id: page.get(2),
            page: page_name,
            views: page_views.get(0),
            hits: page_views.get(1),
        })
    }

    // delete_folder
    //
    // Performs a cascading delete on a folder.
    // This should not be used lightly.
    //
    // TODO: Find out a way to delete the path views related to
    // deleting folders!!!
    async fn delete_folder(&self, folder_id: i32) -> Result<(), Error> {
        sqlx::query(
            "
            DELETE FROM folders
            WHERE folder_id = ?
            ",
        )
        .bind(&folder_id)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    // delete_page
    //
    // Deletes a single page from the database.
    async fn delete_page(&self, folder_id: i32, page_name: String) -> Result<(), Error> {
        let path_id: i32 = sqlx::query(
            "
            SELECT path_id
            FROM pages
            WHERE page_name = ? AND folder_id = ?
            ",
        )
        .bind(&page_name)
        .bind(&folder_id)
        .fetch_one(&self.db_pool)
        .await?
        .get(0);

        sqlx::query(format!("DROP VIEW path_{}", path_id).as_str())
            .execute(&self.db_pool)
            .await?;

        /*
        conn.execute(
            "
            DELETE FROM pages
            WHERE page_name = $1 AND folder_id = $2
            ",
            &[&page_name, &folder_id],
        )
        .await?;
        */

        sqlx::query(
            "
            DELETE FROM paths
            WHERE path_id = ?
            ",
        )
        .bind(&path_id)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    async fn get_settings(&self) -> Result<DenViewSettings, Error> {
        match self.check().await {
            Err(e) => Err(e),
            Ok(v) => match v {
                false => Ok(DenViewSettings::default()),
                true => Ok(serde_json::from_value(
                    sqlx::query(
                        "
                    SELECT setting
                    FROM settings
                    WHERE setting_name = 'current_settings'
                    ",
                    )
                    .fetch_one(&self.db_pool)
                    .await?
                    .get(0),
                )?),
            },
        }
    }

    async fn update_settings(&self, settings: DenViewSettings) -> Result<(), Error> {
        sqlx::query(
            "
            UPDATE settings
            SET setting = ?
            WHERE setting_name = 'current_settings'
            ",
        )
        .bind(&serde_json::to_value(settings)?)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    async fn auth(&self, user: String, pass: String) -> Result<bool, Error> {
        let mut hasher = Sha3::sha3_256();
        hasher.input_str(&pass);
        let hashed_pass = hasher.result_str();

        Ok(match self.check().await? {
            false => (user == "denviews") && (pass == "denviews"),
            true => {
                let db_user: String = serde_json::from_value(
                    sqlx::query(
                        "
                        SELECT setting
                        FROM settings
                        WHERE setting_name = 'user'
                        ",
                    )
                    .fetch_one(&self.db_pool)
                    .await?
                    .get(0),
                )?;

                let db_pass: String = serde_json::from_value(
                    sqlx::query(
                        "
                        SELECT setting
                        FROM settings
                        WHERE setting_name = 'password'
                        ",
                    )
                    .fetch_one(&self.db_pool)
                    .await?
                    .get(0),
                )?;

                (user == db_user) && (hashed_pass == db_pass)
            }
        })
    }

    async fn init(&self, init: DenViewInit) -> Result<(), Error> {
        log::info!("!!! CREATING DATABASE NOW !!!");
        let mut transaction = self.db_pool.begin().await?;

        log::info!("creating table folders");
        sqlx::query(
            "
            CREATE TABLE folders (
                folder_id INT AUTO_INCREMENT PRIMARY KEY,
                parent_id INT
                    REFERENCES folders (folder_id)
                    ON DELETE CASCADE,
                folder_name TEXT NOT NULL
            )
            ",
        )
        .execute(&mut transaction)
        .await?;

        log::info!("inserting root folder");
        sqlx::query(
            "
            INSERT INTO folders
            VALUES (0, null, 'root')
            ",
        )
        .execute(&mut transaction)
        .await?;

        log::info!("creating table paths");
        sqlx::query(
            "
            CREATE TABLE paths (
                path_id INT AUTO_INCREMENT PRIMARY KEY,
                path TEXT NOT NULL
            )
            ",
        )
        .execute(&mut transaction)
        .await?;

        log::info!("creating table pages");
        sqlx::query(
            "
            CREATE TABLE pages (
                page_id INT AUTO_INCREMENT PRIMARY KEY,
                folder_id INT NOT NULL
                    REFERENCES folders
                    ON DELETE CASCADE,
                path_id INT NOT NULL
                    REFERENCES paths
                    ON DELETE CASCADE,
                page_name TEXT NOT NULL,
                first_visited TIMESTAMP,
                total_views BIGINT NOT NULL DEFAULT 0,
                total_hits BIGINT NOT NULL DEFAULT 0
            )
            ",
        )
        .execute(&mut transaction)
        .await?;

        log::info!("creating table visitors");
        sqlx::query(
            "
            CREATE TABLE visitors (
                visitor_id TEXT PRIMARY KEY
                    DEFAULT '---'
            )
            ",
        )
        .execute(&mut transaction)
        .await?;

        log::info!("creating many-many relationship between pages and visitors");
        sqlx::query(
            "
            CREATE TABLE page_visitors (
                page_id INT NOT NULL
                    REFERENCES pages
                    ON DELETE CASCADE,
                visitor_id TEXT
                    REFERENCES visitors
                    ON DELETE SET DEFAULT,
                visitor_hits INT NOT NULL DEFAULT 1
            )
            ",
        )
        .execute(&mut transaction)
        .await?;

        log::info!("creating view total_views");
        sqlx::query(
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
        )
        .execute(&mut transaction)
        .await?;

        log::info!("creating salt");
        sqlx::query("CREATE TABLE salt (salt TEXT NOT NULL)")
            .execute(&mut transaction)
            .await?;
        let salt = util::create_salt();

        sqlx::query("INSERT INTO salt (salt) VALUES (?)")
            .bind(&salt)
            .execute(&mut transaction)
            .await?;

        log::info!("creating table settings");
        sqlx::query(
            "
            CREATE TABLE settings (
                setting_name TEXT PRIMARY KEY,
                setting JSON
            )
            ",
        )
        .execute(&mut transaction)
        .await?;
        sqlx::query("INSERT INTO settings VALUES ('schema_ver', '1')")
            .execute(&mut transaction)
            .await?;
        sqlx::query("INSERT INTO settings VALUES ('user', ?)")
            .bind(&serde_json::to_value(&init.user)?)
            .execute(&mut transaction)
            .await?;

        let mut hasher = Sha3::sha3_256();
        hasher.input_str(&init.pass);
        let hashed_pass = hasher.result_str();

        sqlx::query("INSERT INTO settings VALUES ('password', ?)")
            .bind(&serde_json::to_value(&hashed_pass)?)
            .execute(&mut transaction)
            .await?;
        sqlx::query("INSERT INTO settings VALUES ('current_settings', ?)")
            .bind(&serde_json::to_value(DenViewSettings::from(init))?)
            .execute(&mut transaction)
            .await?;

        log::info!("committing to database");
        transaction.commit().await?;

        log::info!("!!! DATABASE CREATION COMPLETE !!!");
        Ok(())
    }
}
