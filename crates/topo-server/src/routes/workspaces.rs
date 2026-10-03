use axum::extract::{Path, State};
use axum::http::StatusCode;
use topo_core::wire::{ErrorBody, Role, WorkspaceInfo, WorkspaceName};

use crate::auth::{Caller, permit, random_hex};
use crate::db::DbError;
use crate::error::{ApiError, Json, name};
use crate::{AppState, sql};

/// The workspaces the caller belongs to.
#[utoipa::path(
    get, path = "/v1/workspaces", tag = "workspaces",
    responses((status = 200, body = [WorkspaceInfo]), (status = "4XX", body = ErrorBody))
)]
pub async fn list(State(state): State<AppState>, caller: Caller) -> Result<axum::Json<Vec<WorkspaceInfo>>, ApiError> {
    caller.unscoped()?;
    Ok(axum::Json(state.db.batch(vec![sql::workspaces(caller.user_id, None)]).await?.remove(0).all()?))
}

/// Create an empty workspace. The caller becomes its owner.
#[utoipa::path(
    post, path = "/v1/workspaces", tag = "workspaces",
    request_body = WorkspaceName,
    responses((status = 200, body = WorkspaceInfo), (status = "4XX", body = ErrorBody))
)]
pub async fn create(
    State(state): State<AppState>,
    caller: Caller,
    Json(request): Json<WorkspaceName>,
) -> Result<axum::Json<WorkspaceInfo>, ApiError> {
    caller.unscoped()?;
    let info = WorkspaceInfo { id: random_hex(8), name: name(&request.name)?.to_owned(), role: Role::Owner };
    let member = sql::upsert_member(&info.id, caller.user_id, info.role);
    let result = state
        .db
        .batch(vec![sql::insert_workspace_authorized(&info.id, &info.name, caller.user_id, &caller.token_id), member])
        .await;
    if matches!(result, Err(DbError::Constraint)) {
        caller.require_session(&state).await?;
    }
    result?;
    Ok(axum::Json(info))
}

/// Rename a workspace. Owners only.
#[utoipa::path(
    patch, path = "/v1/workspaces/{wid}", tag = "workspaces",
    params(("wid" = String, Path, description = "Workspace id.")),
    request_body = WorkspaceName,
    responses((status = 204), (status = "4XX", body = ErrorBody))
)]
pub async fn rename(
    State(state): State<AppState>,
    caller: Caller,
    Path(wid): Path<String>,
    Json(request): Json<WorkspaceName>,
) -> Result<StatusCode, ApiError> {
    caller.unscoped()?;
    caller.require(&state, &wid, &[Role::Owner]).await?;
    let mut results = state
        .db
        .batch(vec![
            caller.membership(&wid),
            sql::rename_workspace(&wid, name(&request.name)?, caller.user_id, &caller.token_id),
        ])
        .await?;
    permit(caller.role(&wid, results.remove(0))?, &[Role::Owner])?;
    if results.remove(0).0.is_empty() {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Delete a workspace with its nodes and history. Owners only.
#[utoipa::path(
    delete, path = "/v1/workspaces/{wid}", tag = "workspaces",
    params(("wid" = String, Path, description = "Workspace id.")),
    responses((status = 204), (status = "4XX", body = ErrorBody))
)]
pub async fn delete(
    State(state): State<AppState>,
    caller: Caller,
    Path(wid): Path<String>,
) -> Result<StatusCode, ApiError> {
    caller.unscoped()?;
    caller.require(&state, &wid, &[Role::Owner]).await?;
    let mut results = state
        .db
        .batch(vec![caller.membership(&wid), sql::delete_workspace(&wid, caller.user_id, &caller.token_id)])
        .await?;
    permit(caller.role(&wid, results.remove(0))?, &[Role::Owner])?;
    if results.remove(0).0.is_empty() {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
