use std::fmt;

use jiff::civil::Date;
use serde::{Deserialize, Serialize};

/// Short random identifier of a node. It is also the node's file stem.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub String);

impl NodeId {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    const LEN: usize = 6;

    pub fn random() -> Self {
        let id = (0..Self::LEN).map(|_| Self::ALPHABET[fastrand::usize(..Self::ALPHABET.len())] as char).collect();
        Self(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Task,
    Milestone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Todo,
    Doing,
    Done,
    Dropped,
}

impl Status {
    /// A closed node no longer blocks the nodes depending on it.
    pub fn is_closed(self) -> bool {
        matches!(self, Status::Done | Status::Dropped)
    }
}

/// A task or milestone. `depends_on` orders work; a task's `milestones` puts it
/// in the set of work each of those milestones consists of.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    #[serde(default)]
    pub kind: Kind,
    pub title: String,
    #[serde(default)]
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<Date>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<NodeId>,
    /// Milestones this task belongs to (tasks only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub milestones: Vec<NodeId>,
    /// Free-form Markdown notes (the file body, not part of the frontmatter).
    #[serde(skip)]
    pub body: String,
}

impl Node {
    pub fn new(id: NodeId, kind: Kind, title: String) -> Self {
        Self {
            id,
            kind,
            title,
            status: Status::Todo,
            due: None,
            tags: Vec::new(),
            depends_on: Vec::new(),
            milestones: Vec::new(),
            body: String::new(),
        }
    }
}
