// database_tools.rs
//
// Database tools. These should all only be accessible via
// authenticated methods, most of the time. These tools are
// similar to ViewManager's calls, but have more calls
// that imply a need for permissions (e.g., updating settings).

use super::*;
use crate::Error;
use bb8::Pool;
use bb8_postgres::{
    tokio_postgres::{config::Config, NoTls, Row},
    PostgresConnectionManager,
};

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
                SELECT 'self' AS page_name, path_id
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

    pub async fn get_page(&self, folder_id: i32, page_name: String) -> Result<ViewRecord, Error> {
        let conn = self.db_pool.get().await?;

        let page = conn
            .query_one(
                "
            SELECT path_id
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

        Ok(ViewRecord {
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

        conn.execute(
            "
            DELETE FROM pages
            WHERE page_name = $1 AND folder_id = $2
            ",
            &[&page_name, &folder_id],
        )
        .await?;

        conn.execute(
            "
            DELETE FROM paths
            WHERE path_id = $1
            ",
            &[&path_id],
        )
        .await?;

        conn.execute(format!("DROP VIEW path_{}", path_id).as_str(), &[])
            .await?;

        Ok(())
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

    pub async fn init(&self, settings: DenViewSettings) -> Result<(), Error> {
        let mut conn = self.db_pool.get().await?;

        let transaction = conn.transaction().await?;

        transaction
            .execute(
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
            )
            .await?;
        transaction
            .execute(
                "
            INSERT INTO folders
            VALUES (0, null, '')
            ",
                &[],
            )
            .await?;

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

        transaction
            .execute(
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
            )
            .await?;

        transaction
            .execute(
                "
            CREATE TABLE visitors (
                visitor_id TEXT PRIMARY KEY
            )
            ",
                &[],
            )
            .await?;

        transaction
            .execute(
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
            )
            .await?;

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

        transaction
            .execute("CREATE TABLE salt (salt TEXT NOT NULL)", &[])
            .await?;
        let salt = util::create_salt();

        transaction
            .execute("INSERT INTO salt (salt) VALUES ($1)", &[&salt])
            .await?;

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
                "INSERT INTO settings VALUES ('current_settings', $1)",
                &[&serde_json::to_value(settings)?],
            )
            .await?;

        transaction.commit().await?;

        Ok(())
    }
}
