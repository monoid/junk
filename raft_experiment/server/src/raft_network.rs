use crate::model;
use anyhow::Result;
use async_raft::Raft;
use async_raft::{
    network::RaftNetwork,
    raft::{
        AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest,
        InstallSnapshotResponse, VoteRequest, VoteResponse,
    },
};
use async_trait::async_trait;
use reqwest;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt::Debug;
use std::sync::Arc;
use warp::hyper::body::Bytes;
use warp::hyper::Body;
use warp::{reply::Response, Filter};

const APPEND_ENTRIES_PATH: &'static str = "append_entries";
const INSTALL_SNAPSHOT_PATH: &'static str = "install_snapshot";
const VOTE_PATH: &'static str = "vote";

pub struct RaftRouter {
    nodes: HashMap<usize, String>,
    client: reqwest::Client,
}

impl RaftRouter {
    async fn send_req<Req: Serialize + Debug, Resp: DeserializeOwned>(
        &self,
        target: u64,
        method: &'static str,
        req: Req,
    ) -> Result<Resp> {
        eprintln!("{}/{}: {:?}", method, target, req);
        let mut url = self.resolve(target).unwrap().clone();
        url += "/";
        url += method;
        // TODO: use tokio-serde and stream instead of memory buffer
        let mem = bincode::serialize(&req)?;
        let http_data = &self
            .client
            .post(&url)
            .body(mem)
            .send()
            .await?
            .bytes()
            .await?;
        Ok(bincode::deserialize(http_data)?)
    }
}

impl RaftRouter {
    pub fn with_nodes(nodes: &Vec<String>) -> Self {
        Self {
            nodes: nodes.iter().cloned().enumerate().collect::<_>(),
            client: reqwest::Client::new(),
        }
    }

    pub fn resolve(&self, node_id: u64) -> Option<&String> {
        self.nodes.get(&(node_id as usize))
    }
}

impl Default for RaftRouter {
    fn default() -> Self {
        eprintln!("Raft network");
        Self {
            nodes: Default::default(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl<A: async_raft::AppData> RaftNetwork<A> for RaftRouter {
    /// Append entries to target Raft node.
    async fn append_entries(
        &self,
        target: u64,
        rpc: AppendEntriesRequest<A>,
    ) -> Result<AppendEntriesResponse> {
        self.send_req(target, APPEND_ENTRIES_PATH, rpc).await
    }

    /// Send an InstallSnapshot RPC to the target Raft node (§7).
    async fn install_snapshot(
        &self,
        target: u64,
        rpc: InstallSnapshotRequest,
    ) -> Result<InstallSnapshotResponse> {
        self.send_req(target, INSTALL_SNAPSHOT_PATH, rpc).await
    }

    /// Send a RequestVote RPC to the target Raft node (§5).
    async fn vote(&self, target: u64, rpc: VoteRequest) -> Result<VoteResponse> {
        self.send_req(target, VOTE_PATH, rpc).await.map_err(|e| {
            eprintln!("Send req error: {:?}", e);
            e
        })
    }
}

fn err_wrapper<R: warp::reply::Reply + 'static>(
    r: Result<R, anyhow::Error>,
) -> Result<Box<dyn warp::reply::Reply>, Infallible> {
    Ok(match r {
        Ok(reply) => Box::new(reply),
        Err(e) => {
            let msg = e.to_string();
            eprintln!("Reply error: {}", msg);
            Box::new(warp::reply::with_status(
                msg,
                warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    })
}

pub(crate) async fn network_server_endpoint<S>(
    raft: Arc<Raft<memstore::ClientRequest, memstore::ClientResponse, RaftRouter, S>>,
    network: Arc<RaftRouter>,
    port: u16,
) where
    S: async_raft::RaftStorage<memstore::ClientRequest, memstore::ClientResponse>,
{
    let get_raft = move || {
        let copy = raft.clone();
        move || copy.clone()
    };

    async fn append_entries_body<A, R, S>(
        body: Bytes,
        raft: Arc<Raft<A, R, RaftRouter, S>>,
    ) -> anyhow::Result<Response>
    where
        A: async_raft::AppData,
        R: async_raft::AppDataResponse,
        S: async_raft::RaftStorage<A, R>,
    {
        let data = bincode::deserialize(&body)?;
        let out = bincode::serialize(&raft.append_entries(data).await?)?;
        Ok(Response::new(out.into()))
    }

    let append = warp::path(APPEND_ENTRIES_PATH)
        .and(warp::filters::method::post())
        .and(warp::body::bytes())
        .and(warp::any().map(get_raft()))
        .and_then(|body, raft| async { err_wrapper(append_entries_body(body, raft).await) });

    async fn install_snapshot_body<A, R, S>(
        body: Bytes,
        raft: Arc<Raft<A, R, RaftRouter, S>>,
    ) -> anyhow::Result<Response>
    where
        A: async_raft::AppData,
        R: async_raft::AppDataResponse,
        S: async_raft::RaftStorage<A, R>,
    {
        let data = bincode::deserialize(&body)?;
        let out = bincode::serialize(&raft.install_snapshot(data).await?)?;
        Ok(Response::new(out.into()))
    }
    let install_snapshot = warp::path(INSTALL_SNAPSHOT_PATH)
        .and(warp::filters::method::post())
        .and(warp::body::bytes())
        .and(warp::any().map(get_raft()))
        .and_then(|body, raft| async { err_wrapper(install_snapshot_body(body, raft).await) });

    async fn vote_body<A, R, S>(
        body: Bytes,
        raft: Arc<Raft<A, R, RaftRouter, S>>,
    ) -> anyhow::Result<Response>
    where
        A: async_raft::AppData,
        R: async_raft::AppDataResponse,
        S: async_raft::RaftStorage<A, R>,
    {
        let data = bincode::deserialize(&body)?;
        eprintln!("vote resp: {:?}", data);
        let out = bincode::serialize(&raft.vote(data).await?)?;
        Ok(Response::new(Into::<Body>::into(out)))
    }

    let vote = warp::path(VOTE_PATH)
        .and(warp::filters::method::post())
        .and(warp::body::bytes())
        .and(warp::any().map(get_raft()))
        .and_then(|body, raft| async { err_wrapper(vote_body(body, raft).await) });

    async fn client_update_body<S>(
        client: String,
        status: String,
        serial: u64,
        raft: Arc<Raft<memstore::ClientRequest, memstore::ClientResponse, RaftRouter, S>>,
        network: Arc<RaftRouter>,
    ) -> anyhow::Result<Response>
    where
        S: async_raft::RaftStorage<memstore::ClientRequest, memstore::ClientResponse>,
    {
        let resp = raft
            .client_write(async_raft::raft::ClientWriteRequest::new(
                memstore::ClientRequest {
                    client,
                    serial,
                    status,
                },
            ))
            .await;
        match resp {
            Ok(res) => Ok(Response::new(format!("{:?}", res).into())),
            Err(async_raft::error::ClientWriteError::ForwardToLeader(_, to)) => Ok(Response::new(
                format!("Redirect {:?}", to.map(|x| network.resolve(x)).flatten()).into(),
            )),
            Err(e) => Ok(Response::new(format!("{}", e.to_string()).into())),
        }
    }

    let client_update = warp::path("update")
        .and(warp::path::param())
        .and(warp::path::param())
        .and(warp::path::param())
        .and(warp::any().map(get_raft()))
        .and(warp::any().map(move || network.clone()))
        .and_then(
            |client: String, status: String, serial: u64, network, raft| async move {
                err_wrapper(client_update_body(client, status, serial, network, raft).await)
            },
        );
    let all = vote.or(install_snapshot).or(append).or(client_update);

    warp::serve(all).run(([127, 0, 0, 1], port)).await
}
