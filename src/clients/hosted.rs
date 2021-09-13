use crate::database::*;
use crate::Error;
use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{header::USER_AGENT, Body, Client, Method, Request, Response, Server, Uri};
use std::{net::SocketAddr, sync::Arc};

pub struct HostedClient {
    db: DatabaseClient,
}

impl HostedClient {
    fn new() -> Result<Self, Error> {
        Ok(HostedClient {
            db: DatabaseClient::new(16, "host=localhost, user=postgres".parse()?)?,
        })
    }

    pub async fn serve() {
        // I REALLY
        let client = Arc::new(Self::new().unwrap());
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
        let addr = SocketAddr::from(([127, 0, 0, 1], 3621));

        Server::bind(&addr).serve(service).await.unwrap();
    }

    pub async fn execute(
        &self,
        req: &Request<Body>,
        ip: &SocketAddr,
    ) -> Result<Response<Body>, Error> {
        let tracking = "http://127.0.0.1".parse::<Uri>()?.into_parts();
        let uri = Uri::builder()
            .scheme(tracking.scheme.unwrap())
            .authority(tracking.authority.unwrap())
            .path_and_query(req.uri().path())
            .build()?;
        let check = Client::new().get(uri).await;
        match check {
            Err(e) => {
                return Ok(Response::builder()
                    .status(500)
                    .body(Body::from(e.to_string()))?)
            }
            Ok(r) => {
                if !r.status().is_success() {
                    return Ok(Response::builder()
                        .status(r.status())
                        .body(Body::from(""))?);
                }
            }
        }

        let op: DatabaseOperation;

        match (req.method(), req.uri().path()) {
            // TODO: Analytical dashboard for the database.
            //
            // This requires safe authorization.
            //
            // The method of doing this will be separate from
            // any maintenance clients for the server, as well
            // as well as the current database's abstractions.
            (&Method::GET, _) => op = DatabaseOperation::Get(req.uri().path().to_string()),


            // TODO: Authorization method!!!
            (&Method::POST, "/flush") => op = DatabaseOperation::Flush,
            (&Method::POST, _) => {
                op = DatabaseOperation::UpdatePage(
                    req.uri().path().to_string(),
                    ip.to_string() + req.headers()[USER_AGENT].to_str()?,
                )
            }

            _ => return Ok(Response::builder().status(405).body(Body::from(""))?),
        }

        let result = self.db.execute(op)?;
        let response = if let Some(record) = result {
            Response::new(Body::from(serde_json::to_string(&record)?))
        } else {
            Response::new(Body::from(""))
        };

        Ok(response)
    }
}

async fn test_run() {
    HostedClient::serve().await;
}
