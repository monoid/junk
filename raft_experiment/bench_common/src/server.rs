use crate::register::Register;
use liblog::storage;
use std::io;
use std::sync::Arc;
use tokio::fs;
use warp::Filter;

// TODO: make AsyncWAL and LogWriter a type arguments.
pub async fn main() -> io::Result<()> {
    let data_file = fs::File::create("data.txt").await.unwrap();
    let index_file = fs::File::create("index.bin").await.unwrap();

    let log_writer = storage::InstantLogWriter::new(
        storage::SimpleFileWAL::new(
            data_file,
            index_file,
            // 201.73 trans/sec, on my notebook's SSD
            storage::SyncDataFileSyncer::default(),
            // 10582.01 trans/sec
            // storage::NoopFileSyncer::default(),
        )
        .await?,
    );
    let reg = Arc::new(Register::new(0, log_writer));

    let routes = warp::post()
        .and(warp::path("inc"))
        .and(warp::path::param())
        .and(warp::any().map(move || reg.clone()))
        .and_then(update);
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    Ok(())
}

async fn update<L>(
    val: u64,
    reg: Arc<Register<L>>,
) -> Result<impl warp::Reply, std::convert::Infallible>
where
    L: storage::LogWriter,
{
    match reg.add_value(val).await {
        Ok(res) => Ok(format!("You got {}.\n", res)),
        Err(e) => Ok(format!("Error: {}", e.to_string())),
    }
}
