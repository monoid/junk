use crate::model;
use anyhow::Result;
use async_raft::{
    network::RaftNetwork,
    raft::{
        AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest,
        InstallSnapshotResponse, VoteRequest, VoteResponse,
    },
};
use async_trait::async_trait;

pub struct RaftRouter {}

#[async_trait]
impl RaftNetwork<model::Change> for RaftRouter {
    async fn append_entries(
        &self,
        target: u64,
        rpc: AppendEntriesRequest<model::Change>,
    ) -> Result<AppendEntriesResponse> {
        // ... snip ...
        todo!()
    }

    /// Send an InstallSnapshot RPC to the target Raft node (§7).
    async fn install_snapshot(
        &self,
        target: u64,
        rpc: InstallSnapshotRequest,
    ) -> Result<InstallSnapshotResponse> {
        todo!()
    }

    /// Send a RequestVote RPC to the target Raft node (§5).
    async fn vote(&self, target: u64, rpc: VoteRequest) -> Result<VoteResponse> {
        todo!()
    }
}
