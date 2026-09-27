//! Core of topological-todo: every task and milestone is a node in one
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
    #[error("`{node}` depends on missing node `{dep}`")]
    DanglingDependency { node: NodeId, dep: NodeId },
    #[error("`{node}` depends on `{dep}` twice")]
    DuplicateDependency { node: NodeId, dep: NodeId },
    #[error("dependency cycle: {}", .0.iter().map(|id| id.as_str()).collect::<Vec<_>>().join(" -> "))]
    Cycle(Vec<NodeId>),
    #[error("`{from}` cannot depend on `{to}`: `{to}` already depends on `{from}` (this would create a cycle)")]
    WouldCycle { from: NodeId, to: NodeId },
    #[error("`{from}` does not depend on `{to}`")]
    NotLinked { from: NodeId, to: NodeId },
}

impl Error {
    fn io(path: &Path, source: std::io::Error) -> Self {
        Error::Io { path: path.to_owned(), source }
    }
}
