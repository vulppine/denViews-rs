#[derive(rust_embed::RustEmbed)]
#[folder = "dashboard/build/"]
struct PageResources;

pub fn get_resource(res: &str) -> Option<std::borrow::Cow<'static, [u8]>> {
    match PageResources::get(res) {
        None => None,
        Some(v) => Some(v.data),
    }
}

/*

#[derive(std::hash::Hash, PartialEq, Eq)]
pub enum DashboardPage {
    Init,
    GetPage,
    GetFolder,
}

pub struct PageManager {
    pages: HashMap<DashboardPage, Vec<u8>>,
}

impl PageManager {
    pub fn new() -> Result<Self, Error> {
        let mut pages = HashMap::new();
        pages.insert(
            DashboardPage::Init,
            (&include_bytes!("init.html")[..]).to_vec(),
        );
        // pages.insert(GET_PAGE, (&include_bytes!("get_page.html")[..]).to_vec());
        // pages.insert(GET_FOLDER, (&include_bytes!("get_folder.html")[..]).to_vec());

        Ok(PageManager { pages })
    }

    pub fn get(&self, page: DashboardPage) -> &Vec<u8> {
        &self.pages[&page]
    }
}

pub struct ResourceManager {
    resources: HashMap<String, Vec<u8>>,
}

impl ResourceManager {
    pub fn new() -> Result<Self, Error> {
        let mut resources = HashMap::new();

        Ok(ResourceManager { resources })
    }

    pub fn get(&self, resource: String) -> &Vec<u8> {
        &self.resources[&resource]
    }
}
*/
