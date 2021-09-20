pub mod view_manager;
pub mod database_tools;
mod util;

// COMMON STRUCTS
#[derive(serde::Serialize)]
pub struct ViewRecord {
    #[serde(skip)]
    pub id: i32,
    pub page: String,
    pub views: i64,
    pub hits: i64,
}

pub struct FolderRecordPartial {
    pub id: i32,
    pub name: String,
}

pub struct FolderRecord {
    pub id: i32,
    pub name: String,
    pub folders: Vec<FolderRecordPartial>,
    pub pages: Vec<ViewRecord>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct DenViewSettings {
    pub site: String,
    pub use_https: bool,
    pub ignore_queries: bool,
    pub remove_index_pages: bool
}