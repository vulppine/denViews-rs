use crate::Error;
use std::collections::HashMap;

pub const INIT: &str = "init";
pub const GET_PAGE: &str = "get_page";
pub const GET_FOLDER: &str = "get_folder";

#[derive(std::hash::Hash, PartialEq, Eq)]
pub enum DashboardPage {
    Init,
    GetPage,
    GetFolder
}

pub struct PageManager {
    pages: HashMap<DashboardPage, Vec<u8>>
}

impl PageManager {
    pub fn new() -> Result<Self, Error> {
        let mut pages = HashMap::new();
        pages.insert(DashboardPage::Init, (&include_bytes!("init.html")[..]).to_vec());
        // pages.insert(GET_PAGE, (&include_bytes!("get_page.html")[..]).to_vec());
        // pages.insert(GET_FOLDER, (&include_bytes!("get_folder.html")[..]).to_vec());

        Ok(PageManager { pages })
    }

    pub fn get(&self, page: DashboardPage) -> &Vec<u8> {
        self.pages.get(&page).unwrap()
    }
}
