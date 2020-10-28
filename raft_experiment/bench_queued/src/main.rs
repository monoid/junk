use std::time::Duration;

use bench_common::server;
use liblog::{batch, storage};
use tokio::fs;

#[tokio::main]
async fn main() {
    let data_file = fs::File::create("data.txt").await.unwrap();
    let index_file = fs::File::create("index.bin").await.unwrap();
    let config = batch::BatchLogConfig {
        record_count: 31,
        // SSD:
        flush_timeout: Duration::from_millis(1000 / 100),
        // HDD:
        // flush_timeout: Duration::from_millis(1000/30)
    };

    let log_writer = batch::BatchLogWriter::new(
        storage::SimpleFileWAL::new(
            data_file,
            index_file,
            // was: 201.73 trans/sec, on my notebook's SSD
            // now: 4301 with 32 req threads and buffer of 31
            //
            // was ~60 trans/sec on FastVPS' shared HDD.
            // now 1800 trans/sec with 32 threads and buffer of 31
            //     2305 trans/sec with 64 threads and buffer of 63
            storage::SyncDataFileSyncer::default(),
            //
            // was: 10582.01 trans/sec on my notebook's SSD
            // now: 11510.79, nothing really changed (and it shouldn't)
            // storage::NoopFileSyncer::default(),
        )
        .await
        .unwrap(),
        config,
    );
    server::main(log_writer).await.unwrap();
}
