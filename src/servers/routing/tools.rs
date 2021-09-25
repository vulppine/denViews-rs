use super::response_utils;
use crate::dashboard;
use crate::database::{database_tools::DatabaseTools, view_manager::*, DenViewSettings, *};
use crate::Error;
use bb8_postgres::tokio_postgres::config::Config;
use hyper::{body::to_bytes, Body, Method, Request, Response, Uri};
use std::sync::Arc;

pub struct ToolsHandler {
    tools: Arc<DatabaseTools>,
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
    pub async fn new(config: Config) -> Result<Self, Error> {
        Ok(ToolsHandler {
            tools: Arc::new(DatabaseTools::new(config).await?),
        })
    }

    pub async fn check(&self) -> Result<bool, Error> {
        self.tools.check().await
    }

    pub async fn auth(&self, user: String, pass: String) -> Result<bool, Error> {
        self.tools.auth(user, pass).await
    }

    pub async fn handle(&self, req: Request<Body>) -> Result<Response<Body>, Error> {
        let pq = req.uri().path_and_query().unwrap();
        let path = pq.path()[1..]
            .split('/')
            .map(|s| s.to_string())
            .collect::<Vec<String>>();

        if path[0] != "_denViews_dash" {
            return Ok(response_utils::response_with_code!(401, "unauthorized"));
        }

        if path.len() < 2 {
            return Ok(Response::builder()
                .status(301)
                .header(hyper::header::LOCATION, "/_denViews_dash/dash")
                .body(Body::from(""))?);
        }

        // yeah, a lot of this stuff is like either
        // dash/[page]
        // or dash/api/[endpoint]
        if path.len() > 4 {
            return Ok(response_utils::not_found!());
        }

        Ok(match (req.method(), path[1].as_str()) {
            (&Method::GET, p) => match p {
                "api" => match path.len() < 3 {
                    true => response_utils::not_found!(),
                    false => self.db_op(req, &path[2]).await?,
                },
                "" => Response::builder()
                    .status(301)
                    .header(hyper::header::LOCATION, "/_denViews_dash/dash")
                    .body(Body::from(""))?,
                _ => self.get_resource(&path[1..].join("/")).await?,
            },

            (&Method::POST, p) => match p {
                "api" => match path.len() < 3 {
                    true => response_utils::not_found!(),
                    false => self.db_op(req, &path[2]).await?,
                },

                _ => response_utils::not_found!(),
            },

            (&Method::DELETE, p) => match p {
                "api" => match path.len() < 3 {
                    true => response_utils::not_found!(),
                    false => self.db_op(req, &path[2]).await?,
                },

                _ => response_utils::not_found!(),
            },

            _ => response_utils::response_with_code!(405, "not allowed"),
        })
    }

    async fn get_resource(&self, page_route: &str) -> Result<Response<Body>, Error> {
        Ok(match page_route {
            "init" => match self.tools.check().await? {
                true => {
                    response_utils::response_with_code!(401, "denViews is already initialized.")
                }
                false => response_utils::ok!(dashboard::get_resource(
                    &[page_route, "html"].join(".")
                )
                .unwrap()),
            },
            "dash" | "settings" => response_utils::ok!(dashboard::get_resource(
                &[page_route, "html"].join(".")
            )
            .unwrap()),
            _ => match dashboard::get_resource(page_route) {
                Some(p) => response_utils::ok!(p),
                None => response_utils::not_found!(),
            },
        })
    }

    async fn db_op(
        &self,
        mut req: Request<Body>,
        api_route: &str,
    ) -> Result<Response<Body>, Error> {
        log::info!(
            "running through route {} (method: {:?}) now",
            api_route,
            req.method()
        );
        Ok(match (req.method(), api_route) {
            (&Method::GET, "init") => match self.tools.check().await? {
                true => response_utils::internal_error!("denViews is already initialized."),
                false => response_utils::ok!(serde_json::to_string(&DenViewInit::default())?),
            },
            (&Method::POST, "init") => {
                match self.tools.check().await? {
                    false => {
                        let res: Result<DenViewInit, serde_qs::Error> =
                            serde_qs::from_bytes(&to_bytes(req.body_mut()).await?);
                        let settings = match res {
                            Err(e) => return Ok(response_utils::internal_error!(e)),
                            Ok(v) => v,
                        };

                        log::debug!("{:?}", settings);
                        match self.tools.init(settings).await {
                            Ok(_) => response_utils::ok!("denViews successfully initialized. Restart denViews to track sites."),
                            Err(e) => response_utils::internal_error!(e)
                        }
                    }
                    true => Response::new(Body::from("denViews is already initalized.")),
                }
            }

            (&Method::GET, "page") => match query_to_struct::<PageQuery>(req.uri()) {
                None => response_utils::malformed!(),
                Some(v) => match &self.tools.get_page(v.folder_id as i32, v.name).await {
                    Ok(v) => response_utils::ok!(serde_json::to_string(v)?),
                    Err(e) => response_utils::internal_error!(e),
                },
            },
            (&Method::GET, "folder") => match query_to_struct::<FolderQuery>(req.uri()) {
                None => response_utils::malformed!(),
                Some(v) => match &self.tools.get_folder(v.folder_id as i32).await {
                    Ok(v) => Response::builder()
                        .header("Access-Control-Allow-Origin", "*")
                        .body(Body::from(serde_json::to_string(v)?))?,
                    Err(e) => response_utils::internal_error!(e),
                },
            },

            (&Method::DELETE, "page") => match query_to_struct::<PageQuery>(req.uri()) {
                None => response_utils::malformed!(),
                Some(v) => match &self.tools.delete_page(v.folder_id as i32, v.name).await {
                    Ok(v) => response_utils::ok!(serde_json::to_string(v)?),
                    Err(e) => response_utils::internal_error!(e),
                },
            },
            (&Method::DELETE, "folder") => match query_to_struct::<FolderQuery>(req.uri()) {
                None => response_utils::malformed!(),
                Some(v) => match &self.tools.delete_folder(v.folder_id as i32).await {
                    Ok(v) => response_utils::ok!(serde_json::to_string(v)?),
                    Err(e) => response_utils::internal_error!(e),
                },
            },

            (&Method::GET, "settings") => {
                response_utils::ok!(serde_json::to_string(&self.tools.get_settings().await?)?)
            }
            (&Method::POST, "settings") => {
                match serde_qs::from_bytes::<'_, DenViewSettings>(&to_bytes(req.body_mut()).await?)
                {
                    Err(e) => response_utils::internal_error!(e),
                    Ok(s) => match self.tools.update_settings(s).await {
                        Err(e) => response_utils::internal_error!(e),
                        Ok(_) => response_utils::ok!("settings updated - please restart denViews!"),
                    },
                }
            }

            _ => response_utils::response_with_code!(405, "Not allowed."),
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
