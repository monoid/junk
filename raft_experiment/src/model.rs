use std::collections::HashSet;
use serde::{Deserialize, Serialize};

/// Data model.
///
/// We keep single u32 value and list of nodes, as nodes may come and go.
///
/// The value can be incremented, decremented (wrapping) and set.
///

pub type Value = u32;

#[derive(Debug, Deserialize, Serialize)]
pub struct State {
    pub value: Value,
    pub nodes: HashSet<String>,
    // Master is not part of the log, see below.
    // pub master: String,
}

impl State {
    pub fn new(nodes: HashSet<String>) -> Self {
        Self {
            value: 0,
            nodes,
        }
    }
    pub fn apply(&mut self, command: &Command) {
        match command {
            Command::Add(n) => {
                self.value = self.value.wrapping_add(*n);
            }
            Command::Sub(n) => {
                self.value = self.value.wrapping_sub(*n);
            }
            Command::Set(n) => {
                self.value = *n;
            }
            Command::AddNode(node) => {
                self.nodes.insert(node.clone());
            }
            Command::RemoveNode(node) => {
                self.nodes.remove(node);
            }
        }
    }
}

/// Commands.
///
/// Node commands are intented to be internal, but can be issued by an
/// operator.  It is yet out of scope of this project.
///
/// This struct is intended to be stored in RAFT log.
#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    Add(Value),
    Sub(Value),
    Set(Value),
    AddNode(String),
    RemoveNode(String),
    // It should be part of the log to help new or recovering nodes?
    // Or do their get master info from elsewhere?  Yep, it is an
    // chicken-egg problem.

    // SetMaster(String),
}

