pub mod postgres;
mod util;

use crate::Error;

// COMMON STRUCTS
#[derive(serde::Serialize)]
pub struct ViewRecord {
    pub page: String,
    pub views: i64,
    pub hits: i64,
}

#[derive(serde::Serialize)]
pub struct PageRecord {
    pub id: i32,
    pub path_id: i32,
    pub folder_id: i32,
    pub page: String,
    pub views: i64,
    pub hits: i64,
}

#[derive(serde::Serialize)]
pub struct FolderRecordPartial {
    pub id: i32,
    pub name: String,
}

#[derive(serde::Serialize)]
pub struct FolderRecord {
    pub id: i32,
    pub parent_id: Option<i32>,
    pub name: String,
    pub folders: Vec<FolderRecordPartial>,
    pub pages: Vec<ViewRecord>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct DenViewSettings {
    pub site: String,
    pub use_https: bool,
    pub ignore_queries: bool,
    pub remove_index_pages: bool,
    pub always_auth_locally: bool,
}

impl Default for DenViewSettings {
    fn default() -> Self {
        DenViewSettings {
            site: "localhost".into(),
            use_https: true,
            ignore_queries: true,
            remove_index_pages: true,
            always_auth_locally: false,
        }
    }
}

impl From<DenViewInit> for DenViewSettings {
    fn from(init: DenViewInit) -> Self {
        DenViewSettings {
            site: init.site,
            use_https: init.use_https,
            ignore_queries: init.ignore_queries,
            remove_index_pages: init.remove_index_pages,
            always_auth_locally: init.always_auth_locally,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct DenViewInit {
    pub site: String,
    pub use_https: bool,
    pub ignore_queries: bool,
    pub remove_index_pages: bool,
    pub always_auth_locally: bool,
    pub user: String,
    pub pass: String,
}

impl Default for DenViewInit {
    fn default() -> Self {
        DenViewInit {
            site: "localhost".into(),
            use_https: true,
            ignore_queries: true,
            remove_index_pages: true,
            always_auth_locally: false,
            user: "denviews".into(),
            pass: "".into(),
        }
    }
}

#[async_trait::async_trait]
pub trait Database {
    async fn execute(&self, op: &DatabaseOperation<'_>) -> Result<Option<ViewRecord>, Error>;

    async fn get_settings(&self) -> Result<DenViewSettings, Error>;
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
}

#[async_trait::async_trait]
pub trait DatabaseTool {
    async fn check(&self) -> Result<bool, Error>;

    async fn get_folder(&self, folder_id: i32) -> Result<FolderRecord, Error>;

    async fn get_page(&self, folder_id: i32, page_name: String) -> Result<PageRecord, Error>;

    async fn delete_folder(&self, folder_id: i32) -> Result<(), Error>;

    async fn delete_page(&self, folder_id: i32, page_name: String) -> Result<(), Error>;

    async fn get_settings(&self) -> Result<DenViewSettings, Error>;

    async fn update_settings(&self, settinsg: DenViewSettings) -> Result<(), Error>;

    async fn auth(&self, user: String, pass: String) -> Result<bool, Error>;

    async fn init(&self, init: DenViewInit) -> Result<(), Error>;
}
