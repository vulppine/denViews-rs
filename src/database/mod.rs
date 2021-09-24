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

impl From<DenViewInit> for DenViewSettings {
    fn from(init: DenViewInit) -> Self {
        DenViewSettings {
            site: init.site,
            use_https: init.use_https,
            ignore_queries: init.ignore_queries,
            remove_index_pages: init.remove_index_pages,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct DenViewInit {
    pub site: String,
    pub use_https: bool,
    pub ignore_queries: bool,
    pub remove_index_pages: bool,
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
            user: "".into(),
            pass: "".into(),
        }
    }
}
