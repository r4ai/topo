use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use topo_core::wire::{ErrorBody, Member, MemberRole, Role};

use crate::auth::{Caller, permit};
use crate::db::DbError;
use crate::error::{ApiError, Json};
use crate::{AppState, sql};

#[derive(Deserialize)]
struct Row {
    login: String,
    role: Role,
    user_id: u64,
}

/// The members of a workspace the caller belongs to, read with the caller's own membership.
async fn members(state: &AppState, caller: &Caller, wid: &str, allowed: &[Role]) -> Result<Vec<Row>, ApiError> {
    caller.unscoped()?;
    let mut results = state.db.batch(vec![caller.membership(wid), sql::members(wid)]).await?;
    let rows = results.remove(1);
    permit(caller.role(wid, results.remove(0))?, allowed)?;
    Ok(rows.all()?)
}

/// A workspace always has an owner, so its last one cannot be removed or demoted.
fn keep_an_owner(members: &[Row], target: &Row) -> Result<(), ApiError> {
    let owners = members.iter().filter(|m| m.role == Role::Owner).count();
    if target.role == Role::Owner && owners == 1 {
        return Err(ApiError::Conflict("the workspace would be left without an owner".into()));
    }
    Ok(())
}

/// The members of a workspace.
#[utoipa::path(
    get, path = "/v1/workspaces/{wid}/members", tag = "workspaces",
    params(("wid" = String, Path, description = "Workspace id.")),
    responses((status = 200, body = [Member]), (status = "4XX", body = ErrorBody))
)]
pub async fn list(
    State(state): State<AppState>,
    caller: Caller,
    Path(wid): Path<String>,
) -> Result<axum::Json<Vec<Member>>, ApiError> {
    let members = members(&state, &caller, &wid, &[Role::Owner, Role::Editor, Role::Viewer]).await?;
    Ok(axum::Json(members.into_iter().map(|m| Member { user_id: m.user_id, login: m.login, role: m.role }).collect()))
}

fn validate_login(login: &str) -> Result<(), ApiError> {
    if login.is_empty() || login.len() > 39 || !login.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err(ApiError::BadRequest("not a GitHub login".into()));
    }
    Ok(())
}

/// Add a GitHub user to a workspace, or change a member's role. Owners only.
///
/// The login is resolved with GitHub, so the user need not have signed in yet.
#[utoipa::path(
    put, path = "/v1/workspaces/{wid}/members/{login}", tag = "workspaces",
    params(("wid" = String, Path, description = "Workspace id."), ("login" = String, Path, description = "GitHub login.")),
    request_body = MemberRole,
    responses(
        (status = 204),
        (status = 409, body = ErrorBody, description = "The workspace would be left without an owner."),
        (status = "4XX", body = ErrorBody),
    )
)]
pub async fn put(
    State(state): State<AppState>,
    caller: Caller,
    Path((wid, login)): Path<(String, String)>,
    Json(request): Json<MemberRole>,
) -> Result<StatusCode, ApiError> {
    let current_members = members(&state, &caller, &wid, &[Role::Owner]).await?;
    // The login becomes part of a GitHub URL, so it must be nothing but a login.
    validate_login(&login)?;
    let user = state.github.user_by_login(&login).await.map_err(ApiError::Internal)?.ok_or(ApiError::NotFound)?;
    if let Some(current) = current_members.iter().find(|m| m.user_id == user.id)
        && request.role != Role::Owner
    {
        keep_an_owner(&current_members, current)?;
    }
    let statements = vec![
        sql::upsert_user(user.id, &user.login),
        sql::upsert_member_authorized(&wid, user.id, request.role, caller.user_id, &caller.token_id),
    ];
    match state.db.batch(statements).await {
        Ok(_) => {}
        Err(DbError::Constraint) => {
            members(&state, &caller, &wid, &[Role::Owner]).await?;
            return Err(ApiError::Conflict("the workspace would be left without an owner".into()));
        }
        Err(error) => return Err(error.into()),
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Remove a member from a workspace. Owners only.
#[utoipa::path(
    delete, path = "/v1/workspaces/{wid}/members/{login}", tag = "workspaces",
    params(("wid" = String, Path, description = "Workspace id."), ("login" = String, Path, description = "GitHub login.")),
    responses(
        (status = 204),
        (status = 409, body = ErrorBody, description = "The workspace would be left without an owner."),
        (status = "4XX", body = ErrorBody),
    )
)]
pub async fn remove(
    State(state): State<AppState>,
    caller: Caller,
    Path((wid, login)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    members(&state, &caller, &wid, &[Role::Owner]).await?;
    validate_login(&login)?;
    // Logins can change or be reassigned; resolve the current account, never a cached label.
    let user = state.github.user_by_login(&login).await.map_err(ApiError::Internal)?.ok_or(ApiError::NotFound)?;
    remove_user(&state, &caller, &wid, user.id).await
}

/// Remove a member by their stable GitHub id, including a renamed or deleted account.
#[utoipa::path(
    delete, path = "/v1/workspaces/{wid}/members/by-id/{user_id}", tag = "workspaces",
    params(("wid" = String, Path, description = "Workspace id."), ("user_id" = u64, Path, description = "Stable GitHub user id.")),
    responses((status = 204), (status = "4XX", body = ErrorBody))
)]
pub async fn remove_by_id(
    State(state): State<AppState>,
    caller: Caller,
    Path((wid, user_id)): Path<(String, u64)>,
) -> Result<StatusCode, ApiError> {
    if user_id == 0 || user_id > i64::MAX as u64 {
        return Err(ApiError::BadRequest("not a GitHub user id".into()));
    }
    remove_user(&state, &caller, &wid, user_id).await
}

async fn remove_user(state: &AppState, caller: &Caller, wid: &str, user_id: u64) -> Result<StatusCode, ApiError> {
    caller.unscoped()?;
    let mut results = state
        .db
        .batch(vec![
            caller.membership(wid),
            sql::members(wid),
            sql::delete_member_authorized(wid, user_id, caller.user_id, &caller.token_id),
        ])
        .await?;
    let deleted = results.remove(2);
    let rows: Vec<Row> = results.remove(1).all()?;
    permit(caller.role(wid, results.remove(0))?, &[Role::Owner])?;
    let target = rows.iter().find(|m| m.user_id == user_id).ok_or(ApiError::NotFound)?;
    keep_an_owner(&rows, target)?;
    if deleted.0.is_empty() {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
