use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Data model.
///
/// We keep single u32 value and list of nodes, as nodes may come and go.
///
/// The value can be incremented, decremented (wrapping) and set.
///

pub type Value = u32;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct State {
    pub value: Value,
    pub nodes: HashSet<String>,
    // Master is not part of the log, see below.
    // pub master: String,
}

impl State {
    pub fn new(nodes: HashSet<String>) -> Self {
        Self { value: 0, nodes }
    }

    pub async fn apply(&mut self, command: &Change) {
        match command {
            Change::Add(n) => {
                self.value = self.value.wrapping_add(*n);
            }
            Change::Sub(n) => {
                self.value = self.value.wrapping_sub(*n);
            }
            Change::Set(n) => {
                self.value = *n;
            }
            Change::AddNode(node) => {
                self.nodes.insert(node.clone());
            }
            Change::RemoveNode(node) => {
                self.nodes.remove(node);
            }
        }
    }
}

/// Changes.
///
/// Node changes (AddNode, RemoveNode) are intented to be internal,
/// but can be issued by an operator.  It is yet out of scope of
/// this project.
///
/// This struct is intended to be stored in the Raft log.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Change {
    Add(Value),
    Sub(Value),
    Set(Value),
    AddNode(String),
    RemoveNode(String),
}

impl async_raft::AppData for Change {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientResponse(Result<Option<String>, ()>);

impl async_raft::AppDataResponse for ClientResponse {}
