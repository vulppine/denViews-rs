use super::api_handler::{APIHandler, APIRequest};
use async_stream::stream;
use crate::Error;
use futures_core::Stream;
use futures_util::{future::join, TryFutureExt};
use hyper::{
    server::accept::{Accept, from_stream},
    server::conn::{AddrStream, Http},
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server, Uri
};
use std::{
    convert::Infallible,
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

    match settings.use_https {

        false => {
            let addr = SocketAddr::from((
                [127, 0, 0, 1],
                80
            ));

            let service_wrapper = make_service_fn(move |conn: &AddrStream| {
                let client = client.clone();
                let ip = conn.remote_addr();
                async move {
                    Ok::<_, Error>(service_fn(move |req| {
                        let client = client.clone();

                        // in HTTP mode, only when the server is accessed locally is it considered
                        // authenticated
                        let auth = ip.ip() == SocketAddr::from(([127, 0, 0, 1], 0)).ip();

                        async move { client.execute(APIRequest { req, ip, auth }).await }
                    }))
                }
            });

            Server::bind(&addr).serve(service_wrapper).await.unwrap();
        }
        true => {
            let redirect_addr = SocketAddr::from((
                [127, 0, 0, 1],
                80
            ));
            let addr = SocketAddr::from((
                [127, 0, 0, 1],
                443
            ));


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

            // This no longer crashes the server, but honestly, a user-friendly error *should* be
            // returned...
            //
            // TODO: Find a way to return a user-friendly error!
            let stream = TlsStreamWrap(Box::pin(stream! {
                loop {
                    let (socket, ip) = listener.accept().await?;
                    let tls = tls.clone();
                    let tls_accept = tls.accept(socket).await;

                    match tls_accept {
                        Ok(s) => { yield Ok::<_, io::Error>(IncomingStream::<TlsStream<TcpStream>>(s, ip)); }
                        Err(e) => { println!("error occurred on tls connect: {}", e); continue; }
                    }
                }
            }));

            // TODO: Get the internal, current representation of the site's authority
            let redirect_wrapper = make_service_fn(move |_: &AddrStream| {
                async move {
                    Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                        async move {
                            Ok::<_, Infallible>(Response::builder()
                                .status(301)
                                .header("location", Uri::builder()
                                        .scheme("https")
                                        .authority(req.headers()[hyper::header::HOST].to_str().unwrap())
                                        .path_and_query(match req.uri().path_and_query() {
                                            Some(v) => v.as_str(),
                                            None => ""
                                        })
                                        .build()
                                        .unwrap()
                                        .to_string())
                                .body(Body::from(""))
                                .unwrap()
                            )}
                    }))
                }
            });


            let service_wrapper = make_service_fn(move |conn: &IncomingStream<_>| {
                let client = client.clone();
                let ip = conn.ip();
                async move {
                    Ok::<_, Error>(service_fn(move |req| {
                        let client = client.clone();
                        let auth = ip.ip() == SocketAddr::from(([127, 0, 0, 1], 0)).ip();

                        async move { client.execute(APIRequest { req, ip, auth }).await }
                    }))
                }
            });

            let (res1, res2) = join(Server::builder(stream).serve(service_wrapper), Server::bind(&redirect_addr).serve(redirect_wrapper)).await;
            res1.unwrap();
            res2.unwrap();
        }
    }
}

struct TlsStreamWrap(Pin<Box<dyn Stream<Item = Result<IncomingStream<TlsStream<TcpStream>>, io::Error>>>>);

impl Accept for TlsStreamWrap {
    type Conn = IncomingStream<TlsStream<TcpStream>>;
    type Error = io::Error;

    fn poll_accept(
        mut self: Pin<&mut Self>,
        ctx: &mut Context,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
        Pin::new(&mut self.0).poll_next(ctx)
    }
}

struct IncomingStream<S: AsyncRead + AsyncWrite>(S, SocketAddr);

impl <S: AsyncRead + AsyncWrite> IncomingStream<S> {
    fn ip(&self) -> SocketAddr {
        self.1
    }
}

impl <S: AsyncRead + AsyncWrite + Unpin>AsyncRead for IncomingStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_read(ctx, buf)
    }
}

impl <S: AsyncRead + AsyncWrite + Unpin>AsyncWrite for IncomingStream<S> {
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
