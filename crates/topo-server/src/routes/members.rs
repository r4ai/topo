use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use topo_core::wire::{ErrorBody, Member, MemberRole, Role};

use crate::auth::{Caller, permit};
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
    Ok(axum::Json(members.into_iter().map(|m| Member { login: m.login, role: m.role }).collect()))
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
    let members = members(&state, &caller, &wid, &[Role::Owner]).await?;
    // The login becomes part of a GitHub URL, so it must be nothing but a login.
    if login.is_empty() || login.len() > 39 || !login.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err(ApiError::BadRequest("not a GitHub login".into()));
    }
    let user = state.github.user_by_login(&login).await.map_err(ApiError::Internal)?.ok_or(ApiError::NotFound)?;
    if let Some(current) = members.iter().find(|m| m.user_id == user.id)
        && request.role != Role::Owner
    {
        keep_an_owner(&members, current)?;
    }
    let statements = vec![sql::upsert_user(user.id, &user.login), sql::upsert_member(&wid, user.id, request.role)];
    state.db.batch(statements).await?;
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
    let members = members(&state, &caller, &wid, &[Role::Owner]).await?;
    let target = members.iter().find(|m| m.login.eq_ignore_ascii_case(&login)).ok_or(ApiError::NotFound)?;
    keep_an_owner(&members, target)?;
    state.db.batch(vec![sql::delete_member(&wid, target.user_id)]).await?;
    Ok(StatusCode::NO_CONTENT)
}
