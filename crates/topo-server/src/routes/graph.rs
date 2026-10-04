use std::collections::BTreeMap;

use axum::extract::{Path, State};
use axum::http::header::{ETAG, IF_MATCH, IF_NONE_MATCH};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use topo_core::wire::{ApplyRequest, ApplyResult, Change, ErrorBody, ImportRequest, Snapshot};
use topo_core::{Graph, Node, Op, ops};

use crate::auth::Caller;
use crate::error::{ApiError, Json, Query};
use crate::write::{self, Changed, Write};
use crate::{AppState, sql};

fn etag(version: u64) -> String {
    format!("\"{version}\"")
}

/// The headers of a write. `If-Match` holds a version as the `ETag` of the graph gives it.
fn write_headers<'a>(caller: &'a Caller, wid: &'a str, headers: &'a HeaderMap) -> Result<Write<'a>, ApiError> {
    let text = |name: &str| -> Result<Option<&'a str>, ApiError> {
        let value = headers.get(name).map(|value| value.to_str()).transpose();
        value.map_err(|_| ApiError::BadRequest(format!("{name} is not text")))
    };
    let bounded = |name: &str| -> Result<Option<&'a str>, ApiError> {
        match text(name)? {
            Some(value) if value.is_empty() || value.len() > 200 => {
                Err(ApiError::BadRequest(format!("{name} must have 1 to 200 characters")))
            }
            value => Ok(value),
        }
    };
    let if_match = text(IF_MATCH.as_str())?
        .map(|value| value.trim_matches('"').parse())
        .transpose()
        .map_err(|_| ApiError::BadRequest("If-Match must be a version, such as \"12\"".into()))?;
    Ok(Write {
        workspace_id: wid,
        caller,
        idempotency_key: bounded("idempotency-key")?
            .ok_or_else(|| ApiError::BadRequest("the Idempotency-Key header is required".into()))?,
        if_match,
        agent: bounded("topo-agent")?,
    })
}

/// Every node of a workspace.
///
/// Clients compute ready tasks, critical paths, and every other query from
/// this response. To poll, send the `ETag` back as `If-None-Match`.
#[utoipa::path(
    get, path = "/v1/workspaces/{wid}/graph", tag = "graph",
    params(
        ("wid" = String, Path, description = "Workspace id."),
        ("If-None-Match" = Option<String>, Header, description = "`ETag` of the version the client has."),
    ),
    responses(
        (status = 200, body = Snapshot, headers(("ETag" = String, description = "The version, quoted."))),
        (status = 304, description = "The workspace is still at that version."),
        (status = "4XX", body = ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    caller: Caller,
    Path(wid): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    // Accept only the exact strong numeric ETag emitted by this endpoint.
    let known = headers.get(IF_NONE_MATCH).and_then(|value| value.to_str().ok()).and_then(|value| {
        let version = value.strip_prefix('"')?.strip_suffix('"')?.parse::<u64>().ok()?;
        (etag(version) == value).then_some(version)
    });
    let mut results =
        state.db.batch(vec![caller.membership(&wid), sql::version(&wid), sql::nodes_changed(&wid, known)]).await?;
    caller.role(&wid, results.remove(0))?;
    let version = write::version(results.remove(0))?;
    if known == Some(version) {
        return Ok((StatusCode::NOT_MODIFIED, [(ETAG, etag(version))]).into_response());
    }
    let nodes = write::nodes(results.remove(0))?.into_iter().map(Into::into).collect();
    Ok(([(ETAG, etag(version))], axum::Json(Snapshot { version, nodes })).into_response())
}

/// Replace every node of a workspace, keeping the given ids.
///
/// This imports a local workspace. The nodes must form a valid graph. They are
/// stored with the timestamps they carry; an import records no times of its own.
#[utoipa::path(
    put, path = "/v1/workspaces/{wid}/graph", tag = "graph",
    params(
        ("wid" = String, Path, description = "Workspace id."),
        ("Idempotency-Key" = String, Header, description = "Unique per write; reuse it to retry."),
        ("If-Match" = Option<String>, Header, description = "Fail unless the workspace is at this version."),
        ("Topo-Agent" = Option<String>, Header, description = "Label recorded with the change."),
    ),
    request_body = ImportRequest,
    responses(
        (status = 200, body = ApplyResult),
        (status = 412, body = ErrorBody, description = "`If-Match` is not the current version."),
        (status = 422, body = ErrorBody, description = "The nodes are not a valid graph."),
        (status = 503, body = ErrorBody, description = "Lost the race to concurrent writes; retry with the same key."),
        (status = "4XX", body = ErrorBody),
    )
)]
pub async fn import(
    State(state): State<AppState>,
    caller: Caller,
    Path(wid): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ImportRequest>,
) -> Result<axum::Json<ApplyResult>, ApiError> {
    let write = write_headers(&caller, &wid, &headers)?;
    let result = write::write(&state, &write, |before, _| {
        let nodes: Vec<Node> = request.nodes.iter().cloned().map(Into::into).collect();
        let graph = Graph::from_nodes(nodes)?;
        let ops = ops::diff(&before.clone().into_nodes(), &graph.clone().into_nodes());
        Ok(Changed { graph, ops, created: BTreeMap::new() })
    });
    Ok(axum::Json(result.await?))
}

/// Apply a batch of operations to the current graph, all or nothing.
///
/// This is the only way to change nodes. References are exact ids, or `$ref`
/// for a node added earlier in the batch. The server records `created_at`,
/// `updated_at`, and `completed_at` of the nodes the batch changes.
#[utoipa::path(
    post, path = "/v1/workspaces/{wid}/apply", tag = "graph",
    params(
        ("wid" = String, Path, description = "Workspace id."),
        ("Idempotency-Key" = String, Header, description = "Unique per write; reuse it to retry."),
        ("If-Match" = Option<String>, Header, description = "Fail unless the workspace is at this version."),
        ("Topo-Agent" = Option<String>, Header, description = "Label recorded with the change."),
    ),
    request_body = ApplyRequest,
    responses(
        (status = 200, body = ApplyResult),
        (status = 409, body = ErrorBody, description = "An `if_status` did not hold."),
        (status = 412, body = ErrorBody, description = "`If-Match` is not the current version."),
        (status = 422, body = ErrorBody, description = "An operation violates a graph invariant."),
        (status = 503, body = ErrorBody, description = "Lost the race to concurrent writes; retry with the same key."),
        (status = "4XX", body = ErrorBody),
    )
)]
pub async fn apply(
    State(state): State<AppState>,
    caller: Caller,
    Path(wid): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ApplyRequest>,
) -> Result<axum::Json<ApplyResult>, ApiError> {
    let write = write_headers(&caller, &wid, &headers)?;
    let result = write::write(&state, &write, |before, now| {
        let (mut graph, mut ops) = (before.clone(), request.ops.clone());
        let created = ops::apply(&mut graph, &mut ops)?;
        graph.stamp(&before.clone().into_nodes(), now);
        Ok(Changed { graph, ops, created })
    });
    Ok(axum::Json(result.await?))
}

#[derive(Deserialize, utoipa::IntoParams)]
#[serde(deny_unknown_fields)]
pub struct ChangesQuery {
    /// Return the writes after this version.
    #[serde(default)]
    after: u64,
    /// At most this many, up to 1000. The default is 100.
    limit: Option<u64>,
}

/// The recorded writes of a workspace, oldest first.
///
/// This is an audit log: who changed what. Clients do not replay it to sync.
#[utoipa::path(
    get, path = "/v1/workspaces/{wid}/changes", tag = "graph",
    params(("wid" = String, Path, description = "Workspace id."), ChangesQuery),
    responses((status = 200, body = [Change]), (status = "4XX", body = ErrorBody))
)]
pub async fn changes(
    State(state): State<AppState>,
    caller: Caller,
    Path(wid): Path<String>,
    Query(query): Query<ChangesQuery>,
) -> Result<axum::Json<Vec<Change>>, ApiError> {
    #[derive(Deserialize)]
    struct Row {
        version: u64,
        user: String,
        token: String,
        agent: Option<String>,
        created_at: u64,
        ops: String,
    }
    let limit = query.limit.unwrap_or(100);
    if limit > 1000 {
        return Err(ApiError::BadRequest("limit is at most 1000".into()));
    }
    let mut results = state.db.batch(vec![caller.membership(&wid), sql::changes(&wid, query.after, limit)]).await?;
    let rows: Vec<Row> = results.remove(1).all()?;
    caller.role(&wid, results.remove(0))?;
    let changes = rows.into_iter().map(|row| {
        let ops: Vec<Op> = serde_json::from_str(&row.ops).map_err(|e| ApiError::Internal(e.to_string()))?;
        Ok(Change {
            version: row.version,
            user: row.user,
            token: row.token,
            agent: row.agent,
            created_at: row.created_at,
            ops,
        })
    });
    Ok(axum::Json(changes.collect::<Result<_, ApiError>>()?))
}
