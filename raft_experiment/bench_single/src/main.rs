use bench_common::server;

#[tokio::main]
async fn main() {
    server::main().await.unwrap();
}
