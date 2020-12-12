mod config;
mod model;
mod raft_network;
mod raft_storage;
use std::sync::Arc;

use clap;
use tokio;
use tokio::join;

#[tokio::main]
pub async fn main() {
    let matches = clap::App::new("Simple RAFT experiment")
        .version("1.1.1.1.1.1.1")
        .author("monoid")
        .arg(
            clap::Arg::with_name("config")
                .long("config")
                .required(true)
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("self")
                .long("self")
                .required(true)
                .takes_value(true),
        )
        .get_matches();

    let path = matches
        .value_of("config")
        .expect("YAML config path expected");
    let conf = config::load_config(&path).expect("Valid YAML config expected");
    let node_self = matches.value_of("self").expect("self name expected");

    eprintln!(
        "HTTP port: {}, RAFT port: {}",
        conf.http_port, conf.raft_port
    );
    eprintln!("Self: {}", node_self);
    eprintln!("Nodes:");
    for n in &conf.nodes {
        eprintln!("{}", n);
    }
    let config = conf.raft_config.validate();
    eprintln!("Raft config: {:?}", config);

    let node_id = node_self.parse().unwrap();
    let storage = memstore::MemStore::new(node_id);
    let raft = async_raft::Raft::new(
        node_id,
        Arc::new(config.expect("Expected valid config")),
        Arc::new(raft_network::RaftRouter::with_nodes(&conf.nodes)),
        Arc::new(storage),
    );

    let raft = Arc::new(raft);
    let raft1 = raft.clone();

    join!(
        raft_network::network_server_endpoint(raft1, conf.http_port),
        raft.initialize((0u64..conf.nodes.len() as u64).collect::<_>())
    )
    .1
    .expect("Result");
}
