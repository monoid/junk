use crate::register::Register;
use liblog::storage;
use std::io;
use std::sync::Arc;
use warp::Filter;

// TODO: make AsyncWAL and LogWriter a type arguments.
pub async fn main<LW: storage::LogWriter<u64> + Sync + Send + 'static>(
    log_writer: LW,
) -> io::Result<()> {
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
    L: storage::LogWriter<u64>,
{
    match reg.add_value(val).await {
        Ok(res) => Ok(format!("You got {}.\n", res)),
        Err(e) => Ok(format!("Error: {}", e)),
    }
}
