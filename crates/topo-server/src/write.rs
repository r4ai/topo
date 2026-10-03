//! The one write path: read the workspace, change it in memory, and store the
//! difference under the next version, unless another write took that version first.

use std::collections::BTreeMap;

use serde::Deserialize;
use topo_core::wire::ApplyResult;
use topo_core::{Graph, Node, NodeId, Op};

use crate::auth::{Caller, WRITERS, permit};
use crate::db::{DbError, Rows};
use crate::error::ApiError;
use crate::{AppState, sql};

pub struct Write<'a> {
    pub workspace_id: &'a str,
    pub caller: &'a Caller,
    pub idempotency_key: &'a str,
    /// The version the workspace must be at.
    pub if_match: Option<u64>,
    pub agent: Option<&'a str>,
}

/// The outcome of changing a graph in memory.
pub struct Changed {
    pub graph: Graph,
    /// The operations to record, with ids resolved.
    pub ops: Vec<Op>,
    pub created: BTreeMap<String, NodeId>,
}

/// How often a write is recomputed after losing its version to another write.
const ATTEMPTS: usize = 3;

pub async fn write(
    state: &AppState,
    write: &Write<'_>,
    change: impl Fn(&Graph) -> Result<Changed, ApiError>,
) -> Result<ApplyResult, ApiError> {
    let wid = write.workspace_id;
    for _ in 0..ATTEMPTS {
        let read = vec![
            write.caller.membership(wid),
            sql::version(wid),
            sql::change_by_key(wid, write.idempotency_key),
            sql::nodes(wid),
        ];
        let [membership, current, recorded, nodes] =
            <[Rows; 4]>::try_from(state.db.batch(read).await?).expect("one result per statement");
        permit(write.caller.role(wid, membership)?, &WRITERS)?;
        if let Some(recorded) = recorded.first::<Recorded>()? {
            let created = serde_json::from_str(&recorded.created).map_err(internal)?;
            return Ok(ApplyResult { version: recorded.version, created });
        }
        let current = version(current)?;
        if write.if_match.is_some_and(|expected| expected != current) {
            return Err(ApiError::PreconditionFailed(current));
        }
        let before = graph(nodes)?;
        let changed = change(&before)?;
        let (old, new) = (before.into_nodes(), changed.graph.into_nodes());
        let upserts: Vec<&Node> = new.values().filter(|node| old.get(&node.id) != Some(node)).collect();
        let deletes: Vec<&NodeId> = old.keys().filter(|id| !new.contains_key(id)).collect();
        let record = sql::insert_change(sql::NewChange {
            workspace_id: wid,
            version: current + 1,
            user_id: write.caller.user_id,
            token_id: &write.caller.token_id,
            token_name: &write.caller.token_name,
            agent: write.agent,
            idempotency_key: write.idempotency_key,
            ops: serde_json::to_string(&changed.ops).map_err(internal)?,
            created: serde_json::to_string(&changed.created).map_err(internal)?,
        });
        match state.db.batch(vec![record, sql::upsert_nodes(wid, &upserts), sql::delete_nodes(wid, &deletes)]).await {
            Ok(_) => return Ok(ApplyResult { version: current + 1, created: changed.created }),
            // Another write took this version or this key. Reading again tells which.
            Err(DbError::Constraint) => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(ApiError::Busy)
}

#[derive(Deserialize)]
struct Recorded {
    version: u64,
    created: String,
}

/// The row of a `sql::version` statement.
pub fn version(rows: Rows) -> Result<u64, ApiError> {
    #[derive(Deserialize)]
    struct Row {
        version: u64,
    }
    let row: Option<Row> = rows.first()?;
    Ok(row.expect("an aggregate returns one row").version)
}

/// The rows of a `sql::nodes` statement.
pub fn nodes(rows: Rows) -> Result<Vec<Node>, ApiError> {
    rows.0.into_iter().map(|row| sql::node(row).map_err(internal)).collect()
}

fn graph(rows: Rows) -> Result<Graph, ApiError> {
    // Only valid graphs are stored, so a failure here is a defect of the server.
    Graph::from_nodes(nodes(rows)?).map_err(internal)
}

fn internal(e: impl std::fmt::Display) -> ApiError {
    ApiError::Internal(e.to_string())
}
