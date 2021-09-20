mod servers;
mod database;
mod dashboard;
mod util;

type Error = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() {
    servers::run().await;
}
