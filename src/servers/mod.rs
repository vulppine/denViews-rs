// several repeats of the run() command but separated into several cfg macros, oh man

mod handlers;
mod routing;

#[cfg(feature = "hosted")]
pub async fn run() {
    handlers::hosted::serve().await;
}

#[cfg(feature = "aws-lambda")]
pub async fn run() {
}
