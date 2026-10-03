use axum::extract::{Path, State};
use axum::http::StatusCode;
use topo_core::wire::{CreatedToken, ErrorBody, NewToken, Token};

use crate::auth::{Caller, hash, random_hex};
use crate::db::DbError;
use crate::error::{ApiError, Json, name};
use crate::routes::auth::{created, new_secret};
use crate::{AppState, sql};

/// Ten years, in seconds.
const MAX_LIFETIME: u64 = 10 * 365 * 24 * 60 * 60;

/// The caller's tokens, without their secrets.
#[utoipa::path(
    get, path = "/v1/tokens", tag = "tokens",
    responses((status = 200, body = [Token]), (status = "4XX", body = ErrorBody))
)]
pub async fn list(State(state): State<AppState>, caller: Caller) -> Result<axum::Json<Vec<Token>>, ApiError> {
    caller.unscoped()?;
    Ok(axum::Json(state.db.batch(vec![sql::tokens(caller.user_id)]).await?.remove(0).all()?))
}

/// Create a token, for example for an agent.
///
/// A token with `workspace_id` can only read and write the nodes of that
/// workspace. The secret is returned once.
#[utoipa::path(
    post, path = "/v1/tokens", tag = "tokens",
    request_body = NewToken,
    responses((status = 200, body = CreatedToken), (status = "4XX", body = ErrorBody))
)]
pub async fn create(
    State(state): State<AppState>,
    caller: Caller,
    Json(request): Json<NewToken>,
) -> Result<axum::Json<CreatedToken>, ApiError> {
    caller.unscoped()?;
    let name = name(&request.name)?;
    if request.expires_in.is_some_and(|seconds| seconds > MAX_LIFETIME) {
        return Err(ApiError::BadRequest("expires_in is at most ten years; omit it for a token without expiry".into()));
    }
    let scope = request.workspace_id.as_deref();
    if let Some(workspace_id) = scope {
        let membership = state.db.batch(vec![caller.membership(workspace_id)]).await?.remove(0);
        caller.role(workspace_id, membership)?;
    }
    let token = new_secret();
    let insert = sql::insert_token_authorized(
        &random_hex(6),
        &hash(&token),
        caller.user_id,
        scope,
        name,
        request.expires_in,
        &caller.token_id,
    );
    let rows = match state.db.batch(vec![insert]).await {
        Ok(mut rows) => rows.remove(0),
        Err(DbError::Constraint) => {
            caller.require_session(&state).await?;
            if let Some(wid) = scope {
                caller
                    .require(
                        &state,
                        wid,
                        &[topo_core::wire::Role::Owner, topo_core::wire::Role::Editor, topo_core::wire::Role::Viewer],
                    )
                    .await?;
            }
            return Err(DbError::Constraint.into());
        }
        Err(error) => return Err(error.into()),
    };
    Ok(axum::Json(CreatedToken { info: created(rows)?, token }))
}

/// Revoke one of the caller's tokens.
#[utoipa::path(
    delete, path = "/v1/tokens/{id}", tag = "tokens",
    params(("id" = String, Path, description = "Token id, as listed.")),
    responses((status = 204), (status = "4XX", body = ErrorBody))
)]
pub async fn revoke(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    // A restricted token may revoke itself, which is how a client signs out.
    if caller.token_id != id {
        caller.unscoped()?;
    }
    let deleted =
        state.db.batch(vec![sql::delete_token_authorized(&id, caller.user_id, &caller.token_id)]).await?.remove(0);
    if deleted.0.is_empty() { Err(ApiError::NotFound) } else { Ok(StatusCode::NO_CONTENT) }
}
