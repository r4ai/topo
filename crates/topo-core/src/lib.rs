//! Core of topo: every task and milestone is a node in one
//! dependency DAG, stored as one Markdown file per node.

pub mod graph;
pub mod model;
pub mod store;

use std::path::{Path, PathBuf};

pub use graph::{Edit, Graph};
pub use model::{Kind, Node, NodeId, Status};
pub use store::Workspace;

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
}

impl Error {
    fn io(path: &Path, source: std::io::Error) -> Self {
        Error::Io { path: path.to_owned(), source }
    }
}
