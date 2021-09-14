use crate::database::*;
use crate::Error;
use hyper::{
    header::USER_AGENT,
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Body, Client, Method, Request, Response, Server, Uri,
};
use std::{net::SocketAddr, sync::Arc};

pub struct HostedClient {
    db: DatabaseClient,
    settings: DatabaseSettings
}

impl HostedClient {
    async fn new() -> Result<Self, Error> {
        // this assumes your DB server and this application server are on
        // the same network, and the DB server is otherwise inaccessible from
        // the outside without some kind of VPN/gateway into the network
        let user = std::env::var("DENVIEWS_USER").unwrap_or_else(|_| "denviews".to_string());
        let host = std::env::var("DENVIEWS_HOST").unwrap_or_else(|_| "localhost".to_string());
        let db = DatabaseClient::new(16, format!("host={}, user={}", host, user).parse()?).await?;
        let settings = db.get_settings().await?;

        Ok(HostedClient {
            db,
            settings
        })
    }

    pub async fn serve() {
        // I REALLY
        let client = Arc::new(Self::new().await.unwrap());
        let addr = SocketAddr::from(([127, 0, 0, 1], client.settings.port.as_u64().unwrap() as u16));
        let service = make_service_fn(move |conn: &AddrStream| {
            // LIKE MAKING
            let client = client.clone();
            let ip = conn.remote_addr();
            async move {
                // THREE CLONES
                let client = client.clone();
                Ok::<_, Error>(service_fn(move |req| {
                    // TO THE EXACT SAME FUCKING THING
                    let client = client.clone();

                    async move { client.execute(&req, &ip).await }
                }))
            }
        });

        Server::bind(&addr).serve(service).await.unwrap();
    }

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
            (&Method::POST, "flush") => self.db_op(DatabaseOperation::Flush, false).await,
            (&Method::POST, "init") => self.db_op(DatabaseOperation::Init, false).await,

            (&Method::GET, _) => {
                self.db_op(
                    DatabaseOperation::Get(&path.join("/")), true,
                )
                .await
            }
            (&Method::POST, _) => {
                self.db_op(
                    DatabaseOperation::UpdatePage(
                        &path.join("/"),

                        // will the EU scream at me for this? :eye:
                        &(ip.to_string() + req.headers()[USER_AGENT].to_str().unwrap_or("")),
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
            .path_and_query(path)
            .build()?;
        let check = Client::new().get(uri).await;

        match check {
            Err(e) => Ok((false, e.to_string())),
            Ok(r) => Ok((r.status().is_success(), "".into())),
        }
    }
}
