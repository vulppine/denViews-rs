use crate::database::{DenViewSettings, view_manager::*};
use crate::Error;
use hyper::{
    header::{LOCATION, USER_AGENT},
    Body, Client, Method, Request, Response, Uri,
};
use std::{net::SocketAddr, sync::Arc};
use super::tools::ToolsHandler;

pub struct APIHandler<'a> {
    db: Arc<ViewManager>,
    tools: ToolsHandler<'a>,
    settings: Arc<DenViewSettings>,
    init_check: bool, // lazy, find a better way to do this
}

pub struct APIRequest {
    pub req: Request<Body>,
    pub ip: SocketAddr,
    pub auth: bool, // maybe make a struct for this?
}

#[derive(Debug)]
pub struct APIError {
    reason: String
}

impl std::error::Error for APIError {}

impl std::fmt::Display for APIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "api error: {}", self.reason)
    }
}

impl<'a> APIHandler<'a> {
    pub async fn new() -> Result<self::APIHandler<'a>, Error> {
        // this assumes your DB server and this application server are on
        // the same network, and the DB server is otherwise inaccessible from
        // the outside without some kind of VPN/gateway into the network
        let user = std::env::var("DENVIEWS_USER").unwrap_or_else(|_| "denviews".to_string());
        let pass = std::env::var("DENVIEWS_PASS").unwrap_or_else(|_| "denviews".to_string());
        let host = std::env::var("DENVIEWS_HOST").unwrap_or_else(|_| "localhost".to_string());
        let pool_amount = std::env::var("DENVIEWS_POOL_AMOUNT")
            .unwrap_or_else(|_| "16".to_string())
            .parse::<u32>()?;
        let db = Arc::new(ViewManager::new(
            pool_amount,
            format!("postgresql://{1}:{2}@{0}", host, user, pass).parse()?,
        )
        .await?);
        let tools = ToolsHandler::new(db.clone(), format!("postgresql://{1}:{2}@{0}", host, user, pass).parse()?).await?;
        let init_check = tools.check().await?;

        if !init_check {
            println!("!!!-- denViews MUST be set up before it is ready! Visit https://[host]/_denViews_dash/init and fill out the form! --!!!");
        }

        let settings = match init_check {
            true => Arc::new(db.get_settings().await?),
            false => Arc::new(DenViewSettings::default())
        };


        Ok(APIHandler { db, tools, settings, init_check })
    }

    pub fn settings(&self) -> Arc<DenViewSettings> {
        self.settings.clone()
    }

    pub async fn execute(&self, mut req: APIRequest) -> Result<Response<Body>, Error> {
        println!("{:?}", req.req.uri());
        let path = self.path_as_vec(&req.req);

        if !self.init_check {
            return match (req.req.method(), path[0].as_str()) {
                (&Method::GET, "_denViews_dash") | (&Method::POST, "_denViews_dash") => match req.auth {
                    true => self.tools.handle(&mut req.req).await,
                    false => Ok(Response::builder()
                        .status(401)
                        .body(Body::from("You are not authorized."))?)
                }

                _ => Ok(Response::builder()
                    .status(500)
                    .body(Body::from("denViews has not been initialized yet, or there is a settings error."))?)
            }
        }

        match (req.req.method(), path[0].as_str()) {
            // TODO: Analytical dashboard for the database. (andauthorizatiomethod)
            (&Method::GET, "favicon.ico") | (&Method::GET, "_denViews_res") => {
                Ok(Response::builder()
                    .status(404)
                    .body(Body::from("Resource grabbing is not implemented yet."))?)
            }
            (&Method::GET, "_denViews_dash") | (&Method::POST, "_denViews_dash") => match req.auth {
                true => self.tools.handle(&mut req.req).await,
                false => Ok(Response::builder()
                    .status(401)
                    .body(Body::from("You are not authorized."))?),
            },

            (&Method::POST, "_denViews_flush") => match req.auth {
                true => self.db_op(ViewManagerOperation::Flush, false).await,
                false => Ok(Response::builder().status(401).body(Body::from("You are not authorized."))?),
            },

            (&Method::GET, _) => {
                self.db_op(
                    ViewManagerOperation::Get(path.join("/").trim_end_matches('/')),
                    true,
                )
                .await
            }
            (&Method::POST, _) => {
                self.db_op(
                    ViewManagerOperation::UpdatePage(
                        path.join("/").trim_end_matches('/'),
                        // will the EU scream at me for this? :eye:
                        &(req.ip.ip().to_string()
                            + req.req.headers()[USER_AGENT].to_str().unwrap_or("")),
                    ),
                    true,
                )
                .await
            }

            _ => Ok(Response::builder().status(405).body(Body::from(""))?),
        }
    }

    fn path_as_vec(&self, req: &Request<Body>) -> Vec<String> {
        let mut path: Vec<String>;
        if let Some(p) = req.uri().path_and_query() {
            path = p.path()[1..]
                .split('/')
                .map(|p| p.to_string())
                .collect::<Vec<String>>();
            let path_len = path.len(); if self.settings.ignore_queries {
                if let Some(q) = p.query() {
                    path[path_len - 1] = [&path[path.len() - 1], q].join("?");
                }
            }
        } else {
            path = vec!["".into()];
        }

        if self.settings.remove_index_pages && path[path.len() - 1] == "index.html" {
            path.truncate(path.len() - 1);
        }

        path
    }

    async fn db_op(&self, op: ViewManagerOperation<'_>, check: bool) -> Result<Response<Body>, Error> {
        println!("running operation: {:?}", op);
        match op {
            ViewManagerOperation::Get(p) | ViewManagerOperation::UpdatePage(p, _) => {
                if check {
                    let check = self.check_site(p).await;
                    match check {
                        Err(e) => return Ok(Response::builder().status(500).body(Body::from(e.to_string()))?),
                        Ok(v) => {
                            if !v.0 {
                                return Ok(Response::builder().status(500).body(Body::from(v.1))?);
                            }
                        }
                    };
                }

                match self.db.execute(&op).await {
                    Err(e) => Ok(Response::builder()
                        .status(500)
                        .body(Body::from(format!("error running {:?}: {}", op, e)))?),
                    Ok(r) => match r {
                        Some(r) => Ok(Response::new(Body::from(serde_json::to_string(&r)?))),
                        None => Ok(Response::new(Body::from(""))),
                    },
                }
            }

            _ => match self.db.execute(&op).await {
                Ok(_) => Ok(Response::new(Body::from(""))),
                Err(e) => Ok(Response::builder().status(500).body(Body::from(format!(
                    "error performing operation {:?}: {}",
                    &op,
                    e.to_string()
                )))?),
            },
        }
    }

    async fn check_site(&self, path: &str) -> Result<(bool, String), Error> {
        use http::uri::Scheme;
        use std::convert::TryFrom;

        let tracking = self.settings.site.parse::<Uri>()?.into_parts();

        if tracking.authority.is_none() {
            return Err(Box::new(APIError { reason: "no authority found in tracking url".into() }));
        }

        let uri = Uri::builder()
            .scheme(tracking.scheme.unwrap_or(Scheme::try_from("http")?))
            .authority(tracking.authority.unwrap())
            .path_and_query(String::from("/") + path)
            .build()?;
        println!("{:?}", uri);
        let https = hyper_tls::HttpsConnector::new();
        let client = Client::builder().build::<_, Body>(https);
        let check = client.get(uri).await;
        println!("{:?}", check);

        match check {
            Err(e) => Ok((false, e.to_string())),
            Ok(r) => {
                // only a single layer down into the redirect,
                // to avoid any fuckery
                if r.status().is_redirection() {
                    let redirect = r.headers()[LOCATION].to_str()?;
                    let result = client.get(redirect.parse()?).await;
                    return match result {
                        Err(e) => Ok((false, e.to_string())),
                        Ok(r) => Ok((r.status().is_success(), "".into())),
                    };
                }

                Ok((r.status().is_success(), "".into()))
            }
        }
    }
}
