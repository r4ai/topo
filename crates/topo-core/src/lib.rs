//! Core of topo: every task and milestone is a node in one
//! dependency DAG, stored as one Markdown file per node.

#[cfg(feature = "fs")]
pub mod files;
pub mod graph;
pub mod model;
pub mod ops;
#[cfg(feature = "fs")]
pub mod store;
pub mod wire;

use std::path::PathBuf;

pub use graph::{Edit, Graph};
pub use model::{Kind, Node, NodeId, Status};
pub use ops::Op;
#[cfg(feature = "fs")]
pub use store::{Remote, Workspace};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("{path}: {message}")]
    InvalidFile { path: PathBuf, message: String },
    #[error("no .topo workspace found in {0} or its ancestors (run `topo init`)")]
    NoWorkspace(PathBuf),
    #[error("workspace already exists: {0}")]
    AlreadyInitialized(PathBuf),
    #[error("no node matches `{0}`")]
    NotFound(String),
    #[error("`{prefix}` is ambiguous: {}", candidates.iter().map(|id| id.as_str()).collect::<Vec<_>>().join(", "))]
    Ambiguous { prefix: String, candidates: Vec<NodeId> },
    #[error("duplicate node id `{0}`")]
    DuplicateId(NodeId),
    #[error("`{node}` references missing node `{target}`")]
    DanglingReference { node: NodeId, target: NodeId },
    #[error("`{node}` references `{target}` twice")]
    DuplicateReference { node: NodeId, target: NodeId },
    #[error("`{0}` is not a milestone")]
    NotAMilestone(NodeId),
    #[error("`{0}` is a milestone; only tasks can belong to milestones")]
    MilestoneInMilestone(NodeId),
    #[error("cannot change the kind of `{0}` while it has milestone members or memberships")]
    KindChangeBreaksMembership(NodeId),
    #[error("`{task}` is not in milestone `{milestone}`")]
    NotMember { task: NodeId, milestone: NodeId },
    #[error("dependency cycle: {}", .0.iter().map(|id| id.as_str()).collect::<Vec<_>>().join(" -> "))]
    Cycle(Vec<NodeId>),
    #[error("`{from}` cannot require `{to}`: `{to}` already requires `{from}` (this would create a cycle)")]
    WouldCycle { from: NodeId, to: NodeId },
    #[error("`{from}` does not depend on `{to}`")]
    NotLinked { from: NodeId, to: NodeId },
    #[error("invalid node id `{0}` (expected 1 to 32 characters of 0-9 and a-z)")]
    InvalidId(String),
    #[error("unknown ref `${0}`")]
    UnknownRef(String),
    #[error("ref `${0}` is defined twice")]
    DuplicateRef(String),
    #[error("`{id}` is {actual}, not {expected}")]
    StatusMismatch { id: NodeId, expected: Status, actual: Status },
    #[error("operation #{index}: {source}")]
    Op { index: usize, source: Box<Error> },
    #[error(transparent)]
    Remote(Box<dyn std::error::Error + Send + Sync>),
}

#[cfg(feature = "fs")]
impl Error {
    fn io(path: &std::path::Path, source: std::io::Error) -> Self {
        Error::Io { path: path.to_owned(), source }
    }
}
