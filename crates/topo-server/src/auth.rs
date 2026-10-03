use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use topo_core::wire::{Role, WorkspaceInfo};

use crate::error::ApiError;
use crate::{AppState, sql};

/// The token a request presented and the user it belongs to.
#[derive(Debug, Clone, Deserialize)]
pub struct Caller {
    pub token_id: String,
    pub token_name: String,
    /// The one workspace the token is restricted to.
    pub scope: Option<String>,
    pub user_id: u64,
    pub login: String,
}

impl FromRequestParts<AppState> for Caller {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthenticated)?;
        let rows = state.db.batch(vec![sql::caller(&hash(token))]).await?.remove(0);
        rows.first()?.ok_or(ApiError::Unauthenticated)
    }
}

impl Caller {
    /// Tokens, workspaces, and members are managed only with an unrestricted token.
    pub fn unscoped(&self) -> Result<(), ApiError> {
        match self.scope {
            None => Ok(()),
            Some(_) => Err(ApiError::Forbidden("a token restricted to a workspace can only read and write its nodes")),
        }
    }

    /// The statement that reads the caller's membership of a workspace; its
    /// rows go to [`Caller::role`]. It is separate so a handler can put it in
    /// a batch with what it reads next.
    pub fn membership(&self, workspace_id: &str) -> sql::Stmt {
        sql::membership(workspace_id, self.user_id, &self.token_id)
    }

    /// The caller's role in the workspace. A workspace the caller cannot see
    /// is indistinguishable from one that does not exist.
    pub fn role(&self, workspace_id: &str, membership: crate::db::Rows) -> Result<Role, ApiError> {
        let visible = self.scope.as_deref().is_none_or(|scope| scope == workspace_id);
        let info: Option<WorkspaceInfo> = membership.first()?;
        info.filter(|_| visible).map(|info| info.role).ok_or(ApiError::NotFound)
    }

    pub async fn require(&self, state: &AppState, workspace_id: &str, allowed: &[Role]) -> Result<(), ApiError> {
        let membership = state.db.batch(vec![self.membership(workspace_id)]).await?.remove(0);
        permit(self.role(workspace_id, membership)?, allowed)
    }

    pub async fn require_session(&self, state: &AppState) -> Result<(), ApiError> {
        let rows = state.db.batch(vec![sql::session(self.user_id, &self.token_id)]).await?.remove(0);
        if rows.0.is_empty() { Err(ApiError::Unauthenticated) } else { Ok(()) }
    }
}

pub fn permit(role: Role, allowed: &[Role]) -> Result<(), ApiError> {
    if allowed.contains(&role) { Ok(()) } else { Err(ApiError::Forbidden("your role does not allow this")) }
}

pub const WRITERS: [Role; 2] = [Role::Owner, Role::Editor];

/// SHA-256 in hex. Tokens are random, so a fast hash is enough to store them.
pub fn hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

pub fn random_hex(bytes: usize) -> String {
    let mut buffer = vec![0; bytes];
    getrandom::fill(&mut buffer).expect("the platform provides randomness");
    hex(&buffer)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
