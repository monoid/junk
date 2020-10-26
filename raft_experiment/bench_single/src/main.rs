use tokio::fs;
use liblog::storage;
use bench_common::server;

#[tokio::main]
async fn main() {
    let data_file = fs::File::create("data.txt").await.unwrap();
    let index_file = fs::File::create("index.bin").await.unwrap();

    let log_writer = storage::InstantLogWriter::new(
        storage::SimpleFileWAL::new(
            data_file,
            index_file,
            // 201.73 trans/sec, on my notebook's SSD
            // ~60 trans/sec on FastVPS' shared HDD.
            storage::SyncDataFileSyncer::default(),
            // 10582.01 trans/sec on my notebook's SSD
            // storage::NoopFileSyncer::default(),
        )
        .await.unwrap(),
    );
    server::main(log_writer).await.unwrap();
}
