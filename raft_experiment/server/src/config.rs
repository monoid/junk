use serde::Deserialize;
use std::collections::HashSet;
use std::convert::From;
use std::fs::File;
use std::io;
use std::path::Path;

#[derive(Deserialize)]
pub struct Config {
    /// Client HTTP port
    pub http_port: u16,
    /// RAFT port for cluster intercommunication
    pub raft_port: u16,
    /// Initial list of nodes
    pub nodes: Vec<String>,

    pub raft_config: async_raft::ConfigBuilder,
}

#[derive(Debug)]
pub enum Error {
    IOError(io::Error),
    YamlError(serde_yaml_ng::Error),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IOError(err)
    }
}

impl From<serde_yaml_ng::Error> for Error {
    fn from(err: serde_yaml_ng::Error) -> Self {
        Error::YamlError(err)
    }
}

pub fn load_config(path: &Path) -> Result<Config, Error> {
    let mut file = File::open(path)?;
    return Ok(serde_yaml_ng::from_reader(&mut file)?);
}
