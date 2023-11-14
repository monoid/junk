mod config;
mod model;
mod raft_network;
mod raft_storage;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use tokio;
use tokio::join;

#[derive(Debug, Parser)]
#[clap(
    version = "1.1.1.1.1.1.1.1",
    author = "monoid",
    about = "Simple RAFT experiment"
)]
struct Args {
    config: PathBuf,
    #[arg(long = "self")]
    self_id: u64,
}

#[tokio::main]
pub async fn main() {
    let args = Args::parse();
    let conf = config::load_config(&args.config).expect("Valid YAML config expected");

    eprintln!(
        "HTTP port: {}, RAFT port: {}",
        conf.http_port, conf.raft_port
    );
    eprintln!("Self ID: {}", args.self_id);
    eprintln!("Nodes:");
    for n in &conf.nodes {
        eprintln!("{}", n);
    }
    let config = conf.raft_config.validate();
    eprintln!("Raft config: {:?}", config);

    let storage = memstore::MemStore::new(args.self_id);
    let network = Arc::new(raft_network::RaftRouter::with_nodes(&conf.nodes));
    let network1 = network.clone();

    let raft = async_raft::Raft::new(
        args.self_id,
        Arc::new(config.expect("Expected valid config")),
        network,
        Arc::new(storage),
    );

    let raft = Arc::new(raft);
    let raft1 = raft.clone();

    join!(
        raft_network::network_server_endpoint(raft1, network1, conf.http_port),
        raft.initialize((0u64..conf.nodes.len() as u64).collect::<_>())
    )
    .1
    .expect("Result");
}
