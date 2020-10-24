use std::sync::Arc;
use std::io;
use warp::Filter;
use crate::register::Register;
use liblog::storage;
use tokio::fs;

// TODO: make AsyncWAL and LogWriter a type arguments.
pub async fn main() -> io::Result<()> {
    let data_file = fs::File::create("data.txt").await.unwrap();
    let index_file = fs::File::create("index.bin").await.unwrap();

    let log_writer = storage::InstantLogWriter::new(
        storage::SimpleFileWAL::new(
            data_file,
            index_file,
            storage::SyncDataFileSyncer::default(),
        ).await?);
    let reg = Arc::new(Register::new(0, log_writer));
    
    let routes = warp::post()
        .and(warp::path("inc"))
        .and(warp::path::param())
        .and(warp::any().map(move || reg.clone()))
        .and_then(update);
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    Ok(())
}


async fn update<AWAL, L>(val: u64, reg: Arc<Register<AWAL, L>>) -> Result<impl warp::Reply, std::convert::Infallible>
where AWAL: storage::AsyncWAL,
      L: storage::LogWriter<AWAL>,
{
    match reg.add_value(val).await {
        Ok(res) => Ok(format!("You got {}.\n", res)),
        Err(e) => Ok(format!("Error: {}", e.to_string())),
    }
}
