use super::response_utils;
use super::tools::ToolsHandler;
use crate::database::{
    postgres::database::Postgres, Database, DatabaseOperation, DatabaseTool, DenViewSettings,
};
use crate::Error;
use hyper::{
    header::{LOCATION, USER_AGENT},
    Body, Client, Method, Request, Response, Uri,
};
use std::{net::SocketAddr, sync::Arc};

pub struct APIHandler<D, T> {
    db: Arc<D>,
    tools: ToolsHandler<T>,
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
    reason: String,
}

impl std::error::Error for APIError {}

impl std::fmt::Display for APIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "api error: {}", self.reason)
    }
}

impl<D: Database, T: DatabaseTool> APIHandler<D, T> {
    pub async fn new(db: Arc<D>, tools: T) -> Result<Self, Error> {
        // this assumes your DB server and this application server are on
        // the same network, and the DB server is otherwise inaccessible from
        // the outside without some kind of VPN/gateway into the network
        /*
        let db = Arc::new(
            Postgres::new(format!("postgresql://{1}:{2}@{0}", host, user, pass).parse()?).await?,
        );
        */
        let tools = ToolsHandler::new(tools);
        let init_check = tools.check().await?;

        if !init_check {
            println!("!!!-- denViews MUST be set up before it is ready! Visit https://[host]/_denViews_dash/init and fill out the form! --!!!");
        }

        let settings = match init_check {
            true => Arc::new(db.get_settings().await?),
            false => Arc::new(DenViewSettings::default()),
        };

        Ok(APIHandler {
            db,
            tools,
            settings,
            init_check,
        })
    }

    pub fn settings(&self) -> Arc<DenViewSettings> {
        self.settings.clone()
    }

    pub async fn auth(&self, user: String, pass: String) -> Result<bool, Error> {
        self.tools.auth(user, pass).await
    }

    pub async fn execute(&self, req: APIRequest) -> Result<Response<Body>, Error> {
        log::info!("{:?} {:?}", req.req.method(), req.req.uri());
        let path = self.path_as_vec(&req.req);

        if !self.init_check {
            return match (req.req.method(), path[0].as_str()) {
                (_, "_denViews_dash") => match req.auth {
                    true => self.tools.handle(req.req).await,
                    false => Ok(response_utils::request_auth!()),
                },

                _ => Ok(response_utils::internal_error!(
                    "denViews has not been initialized yet"
                )),
            };
        }

        match (req.req.method(), path[0].as_str()) {
            // TODO: Analytical dashboard for the database. (andauthorizatiomethod)
            (_, "_denViews_dash") => match req.auth {
                true => self.tools.handle(req.req).await,
                false => Ok(response_utils::request_auth!()),
            },

            (&Method::POST, "_denViews_flush") => match req.auth {
                true => self.db_op(DatabaseOperation::Flush, false).await,
                false => Ok(response_utils::request_auth!()),
            },

            (&Method::GET, _) => {
                self.db_op(
                    DatabaseOperation::Get(path.join("/").trim_end_matches('/')),
                    true,
                )
                .await
            }
            (&Method::POST, _) => {
                self.db_op(
                    DatabaseOperation::UpdatePage(
                        path.join("/").trim_end_matches('/'),
                        // will the EU scream at me for this? :eye:
                        &(req.ip.ip().to_string()
                            + req.req.headers()[USER_AGENT].to_str().unwrap_or("")),
                    ),
                    true,
                )
                .await
            }

            _ => Ok(response_utils::response_with_code!(405)),
        }
    }

    fn path_as_vec(&self, req: &Request<Body>) -> Vec<String> {
        let mut path: Vec<String>;
        if let Some(p) = req.uri().path_and_query() {
            path = p.path()[1..]
                .split('/')
                .map(|p| p.to_string())
                .collect::<Vec<String>>();
            let path_len = path.len();
            if self.settings.ignore_queries {
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

    async fn db_op(&self, op: DatabaseOperation<'_>, check: bool) -> Result<Response<Body>, Error> {
        log::info!("running operation: {:?}", op);
        match op {
            DatabaseOperation::Get(p) | DatabaseOperation::UpdatePage(p, _) => {
                if check {
                    let check = self.check_site(p).await;
                    log::info!("check performed: response was {:?}", check);
                    match check {
                        Err(e) => return Ok(response_utils::internal_error!(e)),
                        Ok(v) => {
                            if !v.0 {
                                log::error!("error running site check on {}: {}", p, v.1);
                                return Ok(response_utils::internal_error!(v.1));
                            }
                        }
                    };
                }

                match self.db.execute(&op).await {
                    Err(e) => Ok(response_utils::internal_error!(format!(
                        "{:?} error: {}",
                        op, e
                    ))),
                    Ok(r) => match r {
                        Some(r) => Ok(Response::builder()
                            .header("Access-Control-Allow-Origin", "*")
                            .body(Body::from(serde_json::to_string(&r)?))?),
                        None => Ok(response_utils::ok!()),
                    },
                }
            }

            _ => match self.db.execute(&op).await {
                Ok(_) => Ok(Response::new(Body::from(""))),
                Err(e) => Ok(response_utils::internal_error!(format!(
                    "error performing operation {:?}: {}",
                    &op,
                    e.to_string()
                ))),
            },
        }
    }

    async fn check_site(&self, path: &str) -> Result<(bool, String), Error> {
        use http::uri::Scheme;
        use std::convert::TryFrom;

        let tracking = self.settings.site.parse::<Uri>()?.into_parts();

        if tracking.authority.is_none() {
            return Err(Box::new(APIError {
                reason: "no authority found in tracking url".into(),
            }));
        }

        let uri = Uri::builder()
            .scheme(tracking.scheme.unwrap_or(Scheme::try_from("https")?))
            .authority(tracking.authority.unwrap())
            .path_and_query(String::from("/") + path)
            .build()?;
        log::info!("checking {:?}", uri);
        let https = hyper_rustls::HttpsConnector::with_webpki_roots();
        let client = Client::builder().build::<_, Body>(https);
        let check = client.get(uri).await;
        log::debug!("{:?}", check);

        match check {
            Err(e) => Ok((false, e.to_string())),
            Ok(r) => {
                // only a single layer down into the redirect,
                // to avoid any fuckery
                if r.status().is_redirection() {
                    let redirect = r.headers()[LOCATION].to_str()?;
                    log::info!("redirected, checking {}", redirect);
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
