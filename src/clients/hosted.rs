use http::{Request, Response};
use hyper::service::Service;
use r2d2::Pool;
use r2d2_postgres::{postgres::NoTls, PostgresConnectionManager};

struct HostedClient {
    db_pool: Pool<PostgresConnectionManager<NoTls>>,
}

impl Service<Request<Vec<u8>>> for HostedClient {
    type Response = Response<String>

impl HostedClient {
    pub fn new() -> Self {
        HostedClient {
            db_pool: Pool::builder()
                .max_size(16)
                .build(PostgresConnectionManager::new(
                        // TODO: FIX FIX FIX FIX FIX
                        "host=localhost user=postgres".parse().unwrap(),
                        NoTls))
                .unwrap()
        }
    }
}
