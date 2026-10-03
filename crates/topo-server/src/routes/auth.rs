use axum::extract::State;
use topo_core::wire::{AuthConfig, CreatedToken, ErrorBody, GithubSignIn, Token, User};

use crate::auth::{Caller, hash, random_hex};
use crate::error::{ApiError, Json, name};
use crate::{AppState, sql};

/// What a client needs to start the GitHub device flow.
#[utoipa::path(get, path = "/v1/auth/config", tag = "auth", security(()), responses((status = 200, body = AuthConfig)))]
pub async fn config(State(state): State<AppState>) -> axum::Json<AuthConfig> {
    axum::Json(AuthConfig { github_client_id: state.github_client_id.clone() })
}

/// Exchange a GitHub access token for a token of this server.
///
/// The access token must come from the device flow of this server's OAuth app.
/// It is checked with GitHub and not kept.
#[utoipa::path(
    post, path = "/v1/auth/github", tag = "auth", security(()),
    request_body = GithubSignIn,
    responses(
        (status = 200, body = CreatedToken),
        (status = 401, body = ErrorBody, description = "GitHub does not know the token, or another app issued it."),
    )
)]
pub async fn github(
    State(state): State<AppState>,
    Json(request): Json<GithubSignIn>,
) -> Result<axum::Json<CreatedToken>, ApiError> {
    let name = name(&request.name)?;
    let user = state.github.check_token(&request.access_token).await.map_err(ApiError::Internal)?;
    let user = user.ok_or(ApiError::Unauthenticated)?;
    let token = new_secret();
    let insert = sql::insert_token(&random_hex(6), &hash(&token), user.id, None, name, None);
    let mut results = state.db.batch(vec![sql::upsert_user(user.id, &user.login), insert]).await?;
    Ok(axum::Json(CreatedToken { info: created(results.remove(1))?, token }))
}

/// The user the token belongs to.
#[utoipa::path(get, path = "/v1/user", tag = "auth", responses((status = 200, body = User), (status = 401, body = ErrorBody)))]
pub async fn user(caller: Caller) -> axum::Json<User> {
    axum::Json(User { id: caller.user_id, login: caller.login })
}

pub fn new_secret() -> String {
    format!("topo_{}", random_hex(32))
}

/// The row an `insert_token` statement returned.
pub fn created(rows: crate::db::Rows) -> Result<Token, ApiError> {
    rows.first()?.ok_or_else(|| ApiError::Internal("the token insert returned no row".into()))
}
