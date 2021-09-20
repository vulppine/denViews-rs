use crate::database::{DenViewSettings, view_manager::*, database_tools::DatabaseTools};
use crate::Error;
use bb8_postgres::tokio_postgres::config::Config;
use hyper::{body::to_bytes, Body, Method, Request, Response};
use std::{
    collections::HashMap,
    sync::Arc,
};

pub struct ToolsHandler<'a> {
    db: Arc<ViewManager>,
    tools: Arc<DatabaseTools>,
    pages: HashMap<&'a str, Vec<u8>>
}

impl <'a> ToolsHandler<'a> {
    pub async fn new(db: Arc<ViewManager>, config: Config) -> Result<self::ToolsHandler<'a>, Error> {
        let mut pages = HashMap::new();
        pages.insert("init", (&include_bytes!("init.html")[..]).to_vec());

        Ok(ToolsHandler {
            db,
            tools: Arc::new(DatabaseTools::new(config).await?),
            pages
        })
    }

    pub async fn check(&self) -> Result<bool, Error> { self.tools.check().await }

    pub async fn handle(&self, req: &mut Request<Body>) -> Result<Response<Body>, Error> {
        let pq = req.uri().path_and_query().unwrap();
        let path = pq.path()[1..]
            .split('/')
            .collect::<Vec<&str>>();

        if path[0] != "_denViews_dash" {
            return Ok(Response::builder().status(401).body(Body::from("Unauthorized access to tools."))?);
        }

        if path.len() < 2 {
            return Ok(Response::builder().status(404).body(Body::from("Dashboard not implemented yet."))?);
        }

        Ok(match (req.method(), path[1]) {
            (&Method::GET, p) => match p {
                "init" => match self.tools.check().await? {
                    false => Response::new(Body::from(self.pages["init"].clone())),
                    true => Response::new(Body::from("denViews is already initialized."))
                }

                _ => Response::builder().status(404).body(Body::from("Not found."))?
            }

            (&Method::POST, p) => match p {
                "init" => match self.tools.check().await? {
                    false => {
                        let res: Result<DenViewSettings, serde_qs::Error> = serde_qs::from_bytes(&to_bytes(req.body_mut()).await?);
                        let settings = match res {
                            Err(e) => { return Ok(Response::builder()
                                .status(500)
                                .body(Body::from(format!("An error occurred while initializing denViews: {}", e)))?) }
                            Ok(v) => v
                        };

                        println!("{:?}", settings);
                        match self.tools.init(settings).await {
                            Ok(_) => Response::new(Body::from("denViews successfully initialized. Restart denViews to track sites.")),
                            Err(e) => Response::builder()
                                .status(500)
                                .body(Body::from(format!("An error occurred while initializing denViews: {}", e)))?
                        }
                    }
                    true => Response::new(Body::from("denViews is already initalized."))
                }

                _ => Response::builder().status(404).body(Body::from("Not found."))?
            }

            _ => Response::builder().status(405).body(Body::from("Not allowed."))?
        })
    }
}
