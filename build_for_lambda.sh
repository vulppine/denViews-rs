cargo build --target x86_64-unknown-linux-musl --no-default-features --features aws-lambda --release
mv target/x86_64-unknown-linux-musl/release/bootstrap ./
zip denviews-aws.zip bootstrap
