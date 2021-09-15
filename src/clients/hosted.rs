use crate::Error;
use super::api_handler::APIHandler;
use hyper::{
    server::{
        accept::*,
        conn::{AddrStream, Http},
    },
    service::{make_service_fn, service_fn},
    Server,
};
use std::{fs::File, net::SocketAddr, sync::Arc, io::Read, task::Poll};
use tokio::{
    net::TcpListener,
    sync::broadcast
};
use tokio_native_tls::native_tls::{TlsAcceptor, Identity};



pub async fn serve() {
    let client = Arc::new(APIHandler::new().await.unwrap());
    let settings = &client.clone().settings();
    let addr = SocketAddr::from(([127, 0, 0, 1], client.settings().port.as_u64().unwrap() as u16));

    match settings.use_https {
        false => {
            let service_wrapper = make_service_fn(move |conn: &AddrStream| {
                let client = client.clone();
                let ip = conn.remote_addr();
                async move {
                    Ok::<_, Error>(service_fn(move |req| {
                        let client = client.clone();

                        async move { client.execute(&req, &ip).await }
                    }))
                }
            });

            Server::bind(&addr).serve(service_wrapper).await.unwrap();
        }
        true => {
            unimplemented!()
            // having some issues with lifetimes here,
            // will come back to it later when the internal
            // dashboard is required for self-hosted solutions

            /*
            let mut der: Vec<u8> = Vec::new();
            File::open(std::env::var("DENVIEWS_CERT").unwrap()).unwrap().read_to_end(&mut der).unwrap();
            let cert = Identity::from_pkcs12(&der, &std::env::var("DENVIEWS_CERT_PASS").unwrap()).unwrap();

            let listener = TcpListener::bind(addr).await.unwrap();
            let tls = tokio_native_tls::TlsAcceptor::from(TlsAcceptor::builder(cert).build().unwrap());

            let poll = poll_fn(move |ctx| {
                Poll::Ready
            });
            */


            /*
            loop {
                let (socket, ip) = listener.accept().await.unwrap();
                let tls = tls.clone();
                let client = client.clone();
                let service = move |req| {
                    let client = client.clone();
                    async move { client.execute(&req, &ip).await }
                };

                tokio::task::spawn(async move {
                    let tls_socket = tls.accept(socket).await.unwrap();
                    Http::new()
                        .serve_connection(tls_socket, service_fn(service))
                        .await
                });
            }
            */
        }
    }
}
