// several repeats of the run() command but separated into several cfg macros, oh man

pub mod hosted;
pub mod api_handler;

#[cfg(feature = "hosted")]
pub async fn run() {
    hosted::serve().await;
}

#[cfg(feature = "aws-lambda")]
pub async fn run() {
}
