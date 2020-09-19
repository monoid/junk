mod config;
use clap;

fn main() {
    let matches = clap::App::new("Simple RAFT experiment")
        .version("1.1.1.1.1.1.1")
        .author("monoid")
        .arg(clap::Arg::with_name("config")
             .long("config")
             .required(true)
             .takes_value(true))
        .arg(clap::Arg::with_name("self")
             .long("self")
             .required(true)
             .takes_value(true))
        .get_matches();

    let path = matches.value_of("config").expect("YAML config path expected");
    let conf = config::load_config(&path).expect("Valid YAML config expected");
    let node_self = matches.value_of("self").expect("self name expected");

    eprintln!("HTTP port: {}, RAFT port: {}", conf.http_port, conf.raft_port);
    eprintln!("Self: {}", node_self);
    eprintln!("Nodes:");
    for n in &conf.nodes {
        eprintln!("{}", n);
    }
}
