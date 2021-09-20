use crate::dashboard::*;
use crate::database::{database_tools::DatabaseTools, view_manager::*, DenViewSettings, *};
use crate::Error;
use bb8_postgres::tokio_postgres::config::Config;
use hyper::{body::to_bytes, Body, Method, Request, Response, Uri};
use std::sync::Arc;

pub struct ToolsHandler {
    db: Arc<ViewManager>,
    tools: Arc<DatabaseTools>,
    pages: PageManager,
}

#[derive(serde::Deserialize)]
struct PageQuery {
    folder_id: u32,
    name: String,
}

#[derive(serde::Deserialize)]
struct FolderQuery {
    folder_id: u32,
}

impl ToolsHandler {
    pub async fn new(db: Arc<ViewManager>, config: Config) -> Result<Self, Error> {
        let pages = PageManager::new()?;

        Ok(ToolsHandler {
            db,
            tools: Arc::new(DatabaseTools::new(config).await?),
            pages,
        })
    }

    pub async fn check(&self) -> Result<bool, Error> {
        self.tools.check().await
    }

    pub async fn handle(&self, req: Request<Body>) -> Result<Response<Body>, Error> {
        let pq = req.uri().path_and_query().unwrap();
        let path = pq.path()[1..]
            .split('/')
            .map(|s| s.to_string())
            .collect::<Vec<String>>();

        if path[0] != "_denViews_dash" {
            return Ok(Response::builder()
                .status(401)
                .body(Body::from("Unauthorized access to tools."))?);
        }

        if path.len() < 2 {
            return Ok(Response::builder()
                .status(404)
                .body(Body::from("Dashboard not implemented yet."))?);
        }

        // yeah, a lot of this stuff is like either
        // dash/[page]
        // or dash/api/[endpoint]
        if path.len() > 4 {
            return Ok(Response::builder()
                .status(404)
                .body(Body::from("Not found."))?);
        }

        Ok(match (req.method(), path[1].as_str()) {
            (&Method::GET, p) => match p {
                "api" => match path.len() < 3 {
                    true => Response::builder()
                        .status(404)
                        .body(Body::from("Not found."))?,
                    false => self.db_op(req, &path[2]).await?,
                },
                p => self.get_page(p).await?,
            },

            (&Method::POST, p) => match p {
                "api" => match path.len() < 3 {
                    true => Response::builder()
                        .status(404)
                        .body(Body::from("Not found."))?,
                    false => self.db_op(req, &path[2]).await?,
                },

                _ => Response::builder()
                    .status(404)
                    .body(Body::from("Not found."))?,
            },

            _ => Response::builder()
                .status(405)
                .body(Body::from("Not allowed."))?,
        })
    }

    async fn get_page(&self, page_route: &str) -> Result<Response<Body>, Error> {
        Ok(match page_route {
            "init" => match self.tools.check().await? {
                false => Response::new(Body::from(self.pages.get(DashboardPage::Init).clone())),
                true => Response::new(Body::from("denViews is already initialized.")),
            },

            _ => Response::builder()
                .status(404)
                .body(Body::from("Not found."))?,
        })
    }

    async fn db_op(
        &self,
        mut req: Request<Body>,
        api_route: &str,
    ) -> Result<Response<Body>, Error> {
        Ok(match (req.method(), api_route) {
            (&Method::POST, "init") => {
                match self.tools.check().await? {
                    false => {
                        let res: Result<DenViewSettings, serde_qs::Error> =
                            serde_qs::from_bytes(&to_bytes(req.body_mut()).await?);
                        let settings = match res {
                            Err(e) => {
                                return Ok(Response::builder().status(500).body(Body::from(
                                    format!("An error occurred while initializing denViews: {}", e),
                                ))?)
                            }
                            Ok(v) => v,
                        };

                        println!("{:?}", settings);
                        match self.tools.init(settings).await {
                        Ok(_) => Response::new(Body::from("denViews successfully initialized. Restart denViews to track sites.")),
                        Err(e) => Response::builder()
                            .status(500)
                            .body(Body::from(format!("An error occurred while initializing denViews: {}", e)))?
                    }
                    }
                    true => Response::new(Body::from("denViews is already initalized.")),
                }
            }

            (&Method::GET, "page") => match query_to_struct::<PageQuery>(req.uri()) {
                None => Response::builder()
                    .status(400)
                    .body(Body::from("Malformed request."))?,
                Some(v) => match &self.tools.get_page(v.folder_id as i32, v.name).await {
                    Ok(v) => Response::new(Body::from(serde_json::to_string(v)?)),
                    Err(e) => Response::builder().status(500).body(Body::from(format!(
                        "an error occurred during processing: {}",
                        e
                    )))?,
                },
            },
            (&Method::GET, "folder") => match query_to_struct::<FolderQuery>(req.uri()) {
                None => Response::builder()
                    .status(400)
                    .body(Body::from("Malformed request."))?,
                Some(v) => match &self.tools.get_folder(v.folder_id as i32).await {
                    Ok(v) => Response::new(Body::from(serde_json::to_string(v)?)),
                    Err(e) => Response::builder().status(500).body(Body::from(format!(
                        "an error occurred during processing: {}",
                        e
                    )))?,
                },
            },

            (&Method::DELETE, "page") => match query_to_struct::<PageQuery>(req.uri()) {
                None => Response::builder()
                    .status(400)
                    .body(Body::from("Malformed request."))?,
                Some(v) => match &self.tools.delete_page(v.folder_id as i32, v.name).await {
                    Ok(v) => Response::new(Body::from(serde_json::to_string(v)?)),
                    Err(e) => Response::builder().status(500).body(Body::from(format!(
                        "an error occurred during processing: {}",
                        e
                    )))?,
                },
            },
            (&Method::DELETE, "folder") => match query_to_struct::<FolderQuery>(req.uri()) {
                None => Response::builder()
                    .status(400)
                    .body(Body::from("Malformed request."))?,
                Some(v) => match &self.tools.delete_folder(v.folder_id as i32).await {
                    Ok(v) => Response::new(Body::from(serde_json::to_string(v)?)),
                    Err(e) => Response::builder().status(500).body(Body::from(format!(
                        "an error occurred during processing: {}",
                        e
                    )))?,
                },
            },

            (&Method::POST, "settings") => {
                match serde_qs::from_bytes::<'_, DenViewSettings>(&to_bytes(req.body_mut()).await?)
                {
                    Err(e) => Response::builder().status(500).body(Body::from(format!(
                        "an error occurred during processing: {}",
                        e
                    )))?,
                    Ok(s) => match self.tools.update_settings(s).await {
                        Err(e) => Response::builder().status(500).body(Body::from(format!(
                            "an error occurred during processing: {}",
                            e
                        )))?,
                        Ok(_) => Response::new(Body::from("settings updated")),
                    },
                }
            }

            _ => Response::builder()
                .status(405)
                .body(Body::from("Not allowed."))?,
        })
    }
}

fn query_to_struct<'de, T: serde::Deserialize<'de>>(uri: &'de Uri) -> Option<T> {
    match uri.path_and_query() {
        None => None,
        Some(pq) => match pq.query() {
            None => None,
            Some(q) => match serde_qs::from_str::<'de, T>(q) {
                Err(_) => None,
                Ok(v) => Some(v),
            },
        },
    }
}
