use super::api_handler::{APIHandler, APIRequest};
use async_stream::stream;
use crate::Error;
use hyper::{
    server::accept::from_stream,
    server::conn::{AddrStream, Http},
    service::{make_service_fn, service_fn},
    Body, Response, Server,
};
use std::{
    fs::File,
    io,
    io::{BufReader, Error as IoError},
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::io::ReadBuf;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};

// use tokio_native_tls::{native_tls, native_tls::Identity, TlsAcceptor};
use tokio_rustls::{
    rustls::{
        internal::pemfile::{certs, rsa_private_keys},
        NoClientAuth, ServerConfig,
    },
    server::TlsStream,
    TlsAcceptor,
};
// use native_tls::{TlsAcceptor, TlsStream, Identity};

pub async fn serve() {
    let client = Arc::new(APIHandler::new().await.unwrap());
    let settings = &client.clone().settings();
    let addr = SocketAddr::from((
        [127, 0, 0, 1],
        client.settings().port.as_u64().unwrap() as u16,
    ));

    match settings.use_https {
        false => {
            let service_wrapper = make_service_fn(move |conn: &AddrStream| {
                let client = client.clone();
                let ip = conn.remote_addr();
                async move {
                    Ok::<_, Error>(service_fn(move |req| {
                        let client = client.clone();
                        let auth = ip.ip() == SocketAddr::from(([127, 0, 0, 1], 0)).ip();

                        async move { client.execute(&APIRequest { req, ip, auth }).await }
                    }))
                }
            });

            Server::bind(&addr).serve(service_wrapper).await.unwrap();
        }
        true => {
            let certs = certs(&mut BufReader::new(
                File::open(std::env::var("DENVIEWS_CERT").unwrap()).unwrap(),
            ))
            .unwrap();
            let mut keys = rsa_private_keys(&mut BufReader::new(
                File::open(std::env::var("DENVIEWS_CERT_KEY").unwrap()).unwrap(),
            ))
            .unwrap();
            let mut tls_config = ServerConfig::new(NoClientAuth::new());
            tls_config.set_single_cert(certs, keys.remove(0)).unwrap();

            let listener = TcpListener::bind(addr).await.unwrap();
            let tls = TlsAcceptor::from(Arc::new(tls_config));

            // !!! UNTESTED CODE !!!

            let stream = from_stream(stream! {
                loop {
                    let (socket, ip) = listener.accept().await.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                    let tls = tls.clone();
                    let tls_socket = tls.accept(socket).await.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

                    yield Ok::<_, io::Error>(TlsIncomingStream(tls_socket, ip))
                }
            });

            let service_wrapper = make_service_fn(move |conn: &TlsIncomingStream| {
                let client = client.clone();
                let ip = conn.ip();
                async move {
                    Ok::<_, Error>(service_fn(move |req| {
                        let client = client.clone();
                        let auth = ip.ip() == SocketAddr::from(([127, 0, 0, 1], 0)).ip();

                        async move { client.execute(&APIRequest { req, ip, auth }).await }
                    }))
                }
            });

            Server::builder(stream)
                .serve(service_wrapper)
                .await
                .unwrap();
        }
    }
}

struct TlsIncomingStream(TlsStream<TcpStream>, SocketAddr);

impl TlsIncomingStream {
    fn ip(&self) -> SocketAddr {
        self.1
    }
}

impl AsyncRead for TlsIncomingStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_read(ctx, buf)
    }
}

impl AsyncWrite for TlsIncomingStream {
    fn poll_flush(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(ctx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(ctx)
    }

    fn poll_write(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.0).poll_write(ctx, buf)
    }
}
