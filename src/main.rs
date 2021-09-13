mod clients;
mod database;
mod util;

type Error = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() {
    clients::run().await;
}
