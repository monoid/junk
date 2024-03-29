use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;

use crate::model;
use crate::raft_network;
use async_raft;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Deserialize, Serialize)]
pub struct FileStoreSnapshot {
    pub index: u64,
    pub term: u64,
}

#[derive(Clone, Debug)]
pub struct FileStateMachine<T> {
    pub last_applied_log_index: u64,
    pub last_applied_log_term: u64,
    pub membership_config: HashMap<async_raft::NodeId, String>,
    pub nested: T,
}

pub struct FileStorage<T> {
    node_id: async_raft::NodeId,
    state: RwLock<FileStateMachine<T>>,
    storage_dir: PathBuf,
    counter: AtomicUsize,
}

impl<T> FileStorage<T> {
    pub fn new(storage_dir: PathBuf, node_id: async_raft::NodeId, initial: T) -> Self {
        Self {
            node_id,
            state: RwLock::new(FileStateMachine {
                last_applied_log_index: 0,
                last_applied_log_term: 0,
                membership_config: Default::default(),
                nested: initial,
            }),
            storage_dir,
            counter: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl async_raft::RaftStorage<model::Change, model::ClientResponse> for FileStorage<model::State> {
    type Snapshot = tokio::fs::File;
    type ShutdownError = std::io::Error;

    async fn get_membership_config(&self) -> anyhow::Result<async_raft::raft::MembershipConfig> {
        todo!()
    }

    async fn get_initial_state(&self) -> anyhow::Result<async_raft::storage::InitialState> {
        Ok(async_raft::storage::InitialState::new_initial(self.node_id))
    }

    async fn save_hard_state(&self, hs: &async_raft::storage::HardState) -> anyhow::Result<()> {
        todo!()
    }

    async fn get_log_entries(
        &self,
        start: u64,
        stop: u64,
    ) -> anyhow::Result<Vec<async_raft::raft::Entry<model::Change>>> {
        todo!()
    }

    async fn delete_logs_from(&self, start: u64, stop: Option<u64>) -> anyhow::Result<()> {
        todo!()
    }

    async fn append_entry_to_log(
        &self,
        entry: &async_raft::raft::Entry<model::Change>,
    ) -> anyhow::Result<()> {
        todo!()
    }

    async fn replicate_to_log(
        &self,
        entries: &[async_raft::raft::Entry<model::Change>],
    ) -> anyhow::Result<()> {
        todo!()
    }

    async fn apply_entry_to_state_machine(
        &self,
        index: &u64,
        data: &model::Change,
    ) -> anyhow::Result<model::ClientResponse> {
        todo!()
    }

    async fn replicate_to_state_machine(
        &self,
        entries: &[(&u64, &model::Change)],
    ) -> anyhow::Result<()> {
        let mut state = self.state.write().await;
        for (index, command) in entries {
            state.last_applied_log_index = **index;
            state.nested.apply(command).await;
        }
        Ok(())
    }

    async fn do_log_compaction(
        &self,
    ) -> anyhow::Result<async_raft::storage::CurrentSnapshotData<Self::Snapshot>> {
        let data = {
            let state = self.state.read().await;
            (*state).clone()
        };

        todo!()
    }

    async fn create_snapshot(&self) -> anyhow::Result<(String, Box<Self::Snapshot>)> {
        todo!()
    }

    async fn finalize_snapshot_installation(
        &self,
        index: u64,
        term: u64,
        delete_through: Option<u64>,
        id: String,
        snapshot: Box<Self::Snapshot>,
    ) -> anyhow::Result<()> {
        todo!()
    }

    async fn get_current_snapshot(
        &self,
    ) -> anyhow::Result<Option<async_raft::storage::CurrentSnapshotData<Self::Snapshot>>> {
        todo!()
    }
}
