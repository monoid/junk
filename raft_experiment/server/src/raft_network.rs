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
use serde::{de::DeserializeOwned, Serialize};
use reqwest;
use warp::hyper::Body;
use warp::{Filter, reply::Response};
use std::collections::HashMap;
use std::sync::Arc;
use std::convert::Infallible;
use warp::hyper::body::Bytes;


const APPEND_ENTRIES_PATH: &'static str = "append_entries";
const INSTALL_SNAPSHOT_PATH: &'static str = "install_snapshot";
const VOTE_PATH: &'static str = "vote";


#[derive(Clone, Debug)]
pub struct Node {
    url: String
}

pub struct RaftRouter {
    nodes: HashMap<u64, Node>,
    client: reqwest::Client,
}

impl RaftRouter {
    async fn send_req<Req: Serialize, Resp: DeserializeOwned>(&self, target: u64, method: &'static str, req: Req) -> Result<Resp> {
        let mut url = self.nodes.get(&target).unwrap().url.clone();
        url += "/";
        url += method;
        // TODO: use tokio-serde and stream instead of memory buffer
        let mem = bincode::serialize(&req)?;
        let http_data = &self.client
            .post(&url)
            .body(mem)
            .send()
            .await?
            .bytes()
            .await?;
        Ok(bincode::deserialize(http_data)?)
    }
}

impl Default for RaftRouter {
    fn default() -> Self {
        RaftRouter {
            client: reqwest::Client::new(),
            ..Default::default()
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
        self.send_req(target, VOTE_PATH, rpc).await
    }
}

fn err_wrapper<R: warp::reply::Reply + 'static>(r: Result<R, anyhow::Error>) -> Result<Box<dyn warp::reply::Reply>, Infallible> {
    Ok(match r {
        Ok(reply) => Box::new(reply),
        Err(e) => {
            Box::new(warp::reply::with_status(
                e.to_string(),
                warp::http::StatusCode::INTERNAL_SERVER_ERROR)
            )
        }
    })
}

pub(crate) async fn network_server_endpoint<A, R, S>(
    raft: Arc<Raft<A, R, RaftRouter, S>>,
    port: u16,
)
where A: async_raft::AppData,
      R: async_raft::AppDataResponse,
      S: async_raft::RaftStorage<A, R>
{
    let get_raft = move || {
        let copy = raft.clone();
        move || copy.clone()
    };

    async fn append_entries_body<A, R, S>(body: Bytes, raft: Arc<Raft<A, R, RaftRouter, S>>) -> anyhow::Result<Response>
    where A: async_raft::AppData,
          R: async_raft::AppDataResponse,
          S: async_raft::RaftStorage<A, R>
    {
        let data = bincode::deserialize(&body)?;
        let out = bincode::serialize(&raft.append_entries(data).await?)?;
        Ok(Response::new(out.into()))
    }

    let append = warp::path(APPEND_ENTRIES_PATH).and(
        warp::filters::method::post()
    ).and(
        warp::body::bytes()
    ).and(
        warp::any().map(get_raft())
    ).and_then(|body, raft| async {
        err_wrapper(append_entries_body(body, raft).await)
    });

    async fn install_snapshot_body<A, R, S>(body: Bytes, raft: Arc<Raft<A, R, RaftRouter, S>>) -> anyhow::Result<Response>
    where A: async_raft::AppData,
          R: async_raft::AppDataResponse,
          S: async_raft::RaftStorage<A, R>
    {
        let data = bincode::deserialize(&body)?;
        let out = bincode::serialize(&raft.install_snapshot(data).await?)?;
        Ok(Response::new(out.into()))
    }
    let install_snapshot = warp::path(INSTALL_SNAPSHOT_PATH).and(
        warp::filters::method::post()
    ).and(
        warp::body::bytes()
    ).and(
        warp::any().map(get_raft())
    ).and_then(|body, raft| async {
        err_wrapper(install_snapshot_body(body, raft).await)
    });

    async fn vote_body<A, R, S>(body: Bytes, raft: Arc<Raft<A, R, RaftRouter, S>>) -> anyhow::Result<Response>
    where A: async_raft::AppData,
          R: async_raft::AppDataResponse,
          S: async_raft::RaftStorage<A, R>
    {
        let data = bincode::deserialize(&body)?;
        let out = bincode::serialize(&raft.vote(data).await?)?;
        Ok(Response::new(Into::<Body>::into(out)))
    }

    let vote = warp::path(VOTE_PATH).and(
        warp::filters::method::post()
    ).and(
        warp::body::bytes()
    ).and(
        warp::any().map(get_raft())
    ).and_then(|body, raft| async {
        err_wrapper(vote_body(body, raft).await)
    });

    let all = vote.or(install_snapshot).or(append);

    warp::serve(all).run(([127, 0, 0, 1], port)).await
}
