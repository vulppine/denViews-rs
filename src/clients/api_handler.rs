use crate::database::*;
use crate::Error;
use hyper::{
    header::{LOCATION, USER_AGENT},
    server::conn::Http,
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Body, Client, Method, Request, Response, Server, Uri,
};
use std::{fs::File, net::SocketAddr, sync::Arc, io::Read};
use tokio::net::TcpListener;
use tokio_native_tls::native_tls::{TlsAcceptor, Identity};

pub struct APIHandler {
    db: DatabaseClient,
    settings: Arc<DatabaseSettings>,
}

impl APIHandler {
    pub async fn new() -> Result<Self, Error> {
        // this assumes your DB server and this application server are on
        // the same network, and the DB server is otherwise inaccessible from
        // the outside without some kind of VPN/gateway into the network
        let user = std::env::var("DENVIEWS_USER").unwrap_or_else(|_| "denviews".to_string());
        let host = std::env::var("DENVIEWS_HOST").unwrap_or_else(|_| "localhost".to_string());
        let db = DatabaseClient::new(16, format!("host={}, user={}", host, user).parse()?).await?;
        let mut settings = db.get_settings().await;

        if settings.is_err() {
            db.execute(&DatabaseOperation::Init).await?;
            settings = db.get_settings().await;
        };

        let settings = Arc::new(settings.unwrap());

        Ok(APIHandler {
            db,
            settings,
        })
    }

    pub fn settings(&self) -> Arc<DatabaseSettings> {
        self.settings.clone()
    }

    /*
    pub async fn serve() {
            }
    */

    pub async fn execute(
        &self,
        req: &Request<Body>,
        ip: &SocketAddr,
    ) -> Result<Response<Body>, Error> {
        let path = self.path_as_vec(req);

        match (req.method(), path[0].as_str()) {
            // TODO: Analytical dashboard for the database.
            (&Method::GET, "dash") => Ok(Response::builder().status(500).body(Body::from(
                "Internal dashboard not implemented yet. Whoops!",
            ))?),
            // TODO: Authorization method!!!
            (&Method::POST, "flush") => {
                if ip.ip() == "127.0.0.1".parse::<std::net::Ipv4Addr>()? {
                    return self.db_op(DatabaseOperation::Flush, false).await
                }

                Ok(Response::builder().status(401).body(Body::from(""))?)
            }

            (&Method::GET, _) => {
                self.db_op(
                    DatabaseOperation::Get(path.join("/").trim_end_matches('/')), true,
                )
                .await
            }
            (&Method::POST, _) => {
                self.db_op(
                    DatabaseOperation::UpdatePage(
                        path.join("/").trim_end_matches('/'),

                        // will the EU scream at me for this? :eye:
                        &(ip.ip().to_string() + req.headers()[USER_AGENT].to_str().unwrap_or("")),
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
            path = p.path()[1..].split('/').map(|p| p.to_string()).collect::<Vec<String>>();
            let path_len = path.len();
            if self.settings.ignore_queries {
                if let Some(q) = p.query() {
                    path[path_len - 1] = String::from(&path[path.len() - 1]) + "?" + q;
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

    async fn db_op(
        &self,
        op: DatabaseOperation<'_>,
        check: bool,
    ) -> Result<Response<Body>, Error> {
        println!("running operation: {:?}", op);
        match op {
            DatabaseOperation::Get(p) | DatabaseOperation::UpdatePage(p, _) => {
                if check {
                    let (check, reason) = self.check_site(p).await?;
                    if !check {
                        return Ok(Response::builder().status(500).body(Body::from(reason))?);
                    }
                }

                match self.db.execute(&op).await {
                    Err(e) => Ok(Response::builder()
                        .status(500)
                        .body(Body::from(format!("error running {:?}: {}", op, e)))?),
                    Ok(r) => match r {
                        Some(r) => Ok(Response::new(Body::from(serde_json::to_string(&r)?))),
                        None => Ok(Response::new(Body::from("")))
                    }
                }
            },

            _ => {
                match self.db.execute(&op).await {
                    Ok(_) => Ok(Response::new(Body::from(""))),
                    Err(e) => Ok(Response::builder()
                        .status(500)
                        .body(Body::from(format!("error performing operation {:?}: {}", &op, e.to_string())))?)
                }
            }
        }
    }

    async fn check_site(&self, path: &str) -> Result<(bool, String), Error> {
        let tracking = self.settings.site.parse::<Uri>()?.into_parts();
        let uri = Uri::builder()
            .scheme(tracking.scheme.unwrap())
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
                        Ok(r) => Ok((r.status().is_success(), "".into()))
                    }
                }

                Ok((r.status().is_success(), "".into()))
            },
        }
    }
}