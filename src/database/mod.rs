pub mod database_tools;
mod util;
pub mod view_manager;

// COMMON STRUCTS
#[derive(serde::Serialize)]
pub struct ViewRecord {
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
}
