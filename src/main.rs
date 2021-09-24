mod dashboard;
mod database;
mod servers;
mod util;

type Error = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    servers::run().await;
}
