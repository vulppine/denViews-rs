// database_tools.rs
//
// Database tools. These should all only be accessible via
// authenticated methods, most of the time. These tools are
// similar to ViewManager's calls, but have more calls
// that imply a need for permissions (e.g., updating settings).

use super::*;
use crate::util::base64::bytes_to_base64;
use crate::Error;
use bb8::Pool;
use bb8_postgres::{
    tokio_postgres::{config::Config, NoTls},
    PostgresConnectionManager,
};
use crypto::digest::Digest;
use crypto::sha3::Sha3;

pub struct DatabaseTools {
    db_pool: Pool<PostgresConnectionManager<NoTls>>,
}

impl DatabaseTools {
    pub async fn new(config: Config) -> Result<Self, Error> {
        Ok(DatabaseTools {
            db_pool: Pool::builder()
                .max_size(4) // just in case - maybe make this configurable later?
                .build(PostgresConnectionManager::new(config, NoTls))
                .await?,
        })
    }

    pub async fn check(&self) -> Result<bool, Error> {
        //TODO: More indepth method of actually checking for a valid database.
        // At the moment, this is just to ensure that the database has a *settings* file,
        // and isn't some kind of validation check.
        let conn = self.db_pool.get().await?;
        let check = conn
            .query_opt(
                "SELECT setting FROM settings WHERE setting_name = 'schema_ver'",
                &[],
            )
            .await;

        Ok(match check {
            Err(e) => {
                println!("An error occurred while verifying the database. Returning is_init: false, in case it is not initialized. Error: {}", e);
                false
            }
            Ok(r) => r.is_some(),
        })
    }

    pub async fn get_folder(&self, folder_id: i32) -> Result<FolderRecord, Error> {
        let conn = self.db_pool.get().await?;

        let mut pages: Vec<ViewRecord> = Vec::new();
        let mut folders: Vec<FolderRecordPartial> = Vec::new();

        let folder = conn
            .query_one(
                "
            SELECT folder_name, parent_id
            FROM folders
            WHERE folder_id = $1
            ",
                &[&folder_id],
            )
            .await?;

        let folder_name: String = folder.get(0);
        let folder_parent: Option<i32> = folder.get(1);

        let mut page_rows = conn
            .query(
                "
            SELECT page_name, path_id
            FROM pages
            INNER JOIN folders
            ON pages.folder_id = folders.folder_id
            WHERE folders.folder_id = $1
            ",
                &[&folder_id],
            )
            .await?;

        if let Some(id) = folder_parent {
            if let Some(r) = conn
                .query_opt(
                    "
                SELECT '###self###' AS page_name, path_id
                FROM pages
                WHERE folder_id = $1 AND page_name = $2
                ",
                    &[&id, &folder_name],
                )
                .await?
            {
                page_rows.push(r);
            }
        }

        for row in page_rows {
            let path_id: i32 = row.get(1);

            let views = conn
                .query_one(
                    format!(
                        "
                SELECT view_count, hit_count
                FROM path_{}
                ",
                        path_id
                    )
                    .as_str(),
                    &[],
                )
                .await?;

            pages.push(ViewRecord {
                page: row.get(0),
                views: views.get(0),
                hits: views.get(1),
            });
        }

        let folder_rows = conn
            .query(
                "
            SELECT folder_id, folder_name
            FROM folders
            WHERE parent_id = $1
            ",
                &[&folder_id],
            )
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

    pub async fn get_page(&self, folder_id: i32, page_name: String) -> Result<PageRecord, Error> {
        let conn = self.db_pool.get().await?;

        let page = conn
            .query_one(
                "
            SELECT path_id, page_id, folder_id
            FROM pages
            WHERE folder_id = $1 AND page_name = $2
            ",
                &[&folder_id, &page_name],
            )
            .await?;
        let path_id: i32 = page.get(0);

        let page_views = conn
            .query_one(
                format!(
                    "
            SELECT view_count, hit_count
            FROM path_{}
            ",
                    path_id
                )
                .as_str(),
                &[],
            )
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
    pub async fn delete_folder(&self, folder_id: i32) -> Result<(), Error> {
        let conn = self.db_pool.get().await?;

        conn.execute(
            "
            DELETE FROM folders
            WHERE folder_id = $1
            ",
            &[&folder_id],
        )
        .await?;

        Ok(())
    }

    // delete_page
    //
    // Deletes a single page from the database.
    pub async fn delete_page(&self, folder_id: i32, page_name: String) -> Result<(), Error> {
        let conn = self.db_pool.get().await?;

        let path_id: i32 = conn
            .query_one(
                "
            SELECT path_id
            FROM pages
            WHERE page_name = $1 AND folder_id = $2
            ",
                &[&page_name, &folder_id],
            )
            .await?
            .get(0);

        conn.execute(format!("DROP VIEW path_{}", path_id).as_str(), &[])
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

        conn.execute(
            "
            DELETE FROM paths
            WHERE path_id = $1
            ",
            &[&path_id],
        )
        .await?;

        Ok(())
    }

    pub async fn get_settings(&self) -> Result<DenViewSettings, Error> {
        let conn = self.db_pool.get().await?;

        match self.check().await {
            Err(e) => Err(e),
            Ok(v) => match v {
                false => Ok(DenViewSettings::default()),
                true => Ok(serde_json::from_value(
                    conn.query_one(
                        "
                    SELECT setting
                    FROM settings
                    WHERE setting_name = 'current_settings'
                    ",
                        &[],
                    )
                    .await?
                    .get(0),
                )?),
            },
        }
    }

    pub async fn update_settings(&self, settings: DenViewSettings) -> Result<(), Error> {
        let conn = self.db_pool.get().await?;

        conn.execute(
            "
            UPDATE settings
            SET setting = $1
            WHERE setting_name = 'current_settings'
            ",
            &[&serde_json::to_value(settings)?],
        )
        .await?;

        Ok(())
    }

    pub async fn auth(&self, user: String, pass: String) -> Result<bool, Error> {
        let conn = self.db_pool.get().await?;

        let mut hasher = Sha3::sha3_256();
        hasher.input_str(&pass);
        let hashed_pass = hasher.result_str();

        Ok(match self.check().await? {
            false => (user == "denviews") && (pass == "denviews"),
            true => {
                let db_user: String = serde_json::from_value(
                    conn.query_one(
                        "
                SELECT setting
                FROM settings
                WHERE setting_name = 'user'
                ",
                        &[],
                    )
                    .await?
                    .get(0),
                )?;

                let db_pass: String = serde_json::from_value(
                    conn.query_one(
                        "
                SELECT setting
                FROM settings
                WHERE setting_name = 'password'
                ",
                        &[],
                    )
                    .await?
                    .get(0),
                )?;

                (user == db_user) && (hashed_pass == db_pass)
            }
        })
    }

    pub async fn init(&self, init: DenViewInit) -> Result<(), Error> {
        log::info!("!!! CREATING DATABASE NOW !!!");
        let mut conn = self.db_pool.get().await?;

        let transaction = conn.transaction().await?;

        log::info!("creating table folders");
        transaction
            .execute(
                "
            CREATE TABLE folders (
                folder_id INT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
                parent_id INT
                    REFERENCES folders (folder_id)
                    ON DELETE CASCADE,
                folder_name TEXT NOT NULL
            )
            ",
                &[],
            )
            .await?;

        log::info!("inserting root folder");
        transaction
            .execute(
                "
            INSERT INTO folders
            VALUES (0, null, 'root')
            ",
                &[],
            )
            .await?;

        log::info!("creating table paths");
        transaction
            .execute(
                "
            CREATE TABLE paths (
                path_id INT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
                path TEXT NOT NULL
            )
            ",
                &[],
            )
            .await?;

        log::info!("creating table pages");
        transaction
            .execute(
                "
            CREATE TABLE pages (
                page_id INT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
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
                &[],
            )
            .await?;

        log::info!("creating table visitors");
        transaction
            .execute(
                "
            CREATE TABLE visitors (
                visitor_id TEXT PRIMARY KEY
                    DEFAULT '---'
            )
            ",
                &[],
            )
            .await?;

        log::info!("creating many-many relationship between pages and visitors");
        transaction
            .execute(
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
                &[],
            )
            .await?;

        log::info!("creating view total_views");
        transaction
            .execute(
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
            )
            .await?;

        log::info!("creating salt");
        transaction
            .execute("CREATE TABLE salt (salt TEXT NOT NULL)", &[])
            .await?;
        let salt = util::create_salt();

        transaction
            .execute("INSERT INTO salt (salt) VALUES ($1)", &[&salt])
            .await?;

        log::info!("creating table settings");
        transaction
            .execute(
                "
            CREATE TABLE settings (
                setting_name TEXT PRIMARY KEY,
                setting JSON
            )
            ",
                &[],
            )
            .await?;
        transaction
            .execute("INSERT INTO settings VALUES ('schema_ver', '1'::JSON)", &[])
            .await?;
        transaction
            .execute(
                "INSERT INTO settings VALUES ('user', $1)",
                &[&serde_json::to_value(&init.user)?],
            )
            .await?;

        let mut hasher = Sha3::sha3_256();
        hasher.input_str(&init.pass);
        let hashed_pass = hasher.result_str();

        transaction
            .execute(
                "INSERT INTO settings VALUES ('password', $1)",
                &[&serde_json::to_value(&hashed_pass)?],
            )
            .await?;
        transaction
            .execute(
                "INSERT INTO settings VALUES ('current_settings', $1)",
                &[&serde_json::to_value(DenViewSettings::from(init))?],
            )
            .await?;

        log::info!("committing to database");
        transaction.commit().await?;

        log::info!("!!! DATABASE CREATION COMPLETE !!!");
        Ok(())
    }
}
