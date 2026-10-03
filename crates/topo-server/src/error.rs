use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use topo_core::wire::{ErrorBody, ErrorDetail};

use crate::db::DbError;

/// A failed request. Each variant is one `code` of the error body.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("the token is missing, unknown, or expired")]
    Unauthenticated,
    #[error("{0}")]
    Forbidden(&'static str),
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    Conflict(String),
    #[error("the workspace is at version {0}")]
    PreconditionFailed(u64),
    #[error("{0}")]
    InvalidGraph(String),
    #[error("too many concurrent writes; retry with the same Idempotency-Key")]
    Busy,
    #[error("internal error")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            ApiError::Unauthenticated => (StatusCode::UNAUTHORIZED, "unauthenticated"),
            ApiError::Forbidden(_) => (StatusCode::FORBIDDEN, "forbidden"),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            ApiError::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            ApiError::PreconditionFailed(_) => (StatusCode::PRECONDITION_FAILED, "precondition_failed"),
            ApiError::InvalidGraph(_) => (StatusCode::UNPROCESSABLE_ENTITY, "invalid_graph"),
            ApiError::Busy => (StatusCode::SERVICE_UNAVAILABLE, "busy"),
            ApiError::Internal(detail) => {
                log(detail);
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
        };
        let error = ErrorDetail { code: code.to_owned(), message: self.to_string() };
        (status, axum::Json(ErrorBody { error })).into_response()
    }
}

/// The detail of an internal error goes to the log, not to the caller.
fn log(detail: &str) {
    #[cfg(target_arch = "wasm32")]
    worker::console_error!("{detail}");
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{detail}");
}

impl From<DbError> for ApiError {
    fn from(e: DbError) -> Self {
        ApiError::Internal(e.to_string())
    }
}

impl From<topo_core::Error> for ApiError {
    fn from(e: topo_core::Error) -> Self {
        let message = e.to_string();
        match e {
            topo_core::Error::Op { source, .. } if matches!(*source, topo_core::Error::StatusMismatch { .. }) => {
                ApiError::Conflict(message)
            }
            _ => ApiError::InvalidGraph(message),
        }
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        ApiError::BadRequest(rejection.body_text())
    }
}

impl From<QueryRejection> for ApiError {
    fn from(rejection: QueryRejection) -> Self {
        ApiError::BadRequest(rejection.body_text())
    }
}

/// `axum::Json` whose rejection is an [`ApiError`], so every error has the same body.
#[derive(axum::extract::FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct Json<T>(pub T);

/// `axum::extract::Query` whose rejection is an [`ApiError`].
#[derive(axum::extract::FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(ApiError))]
pub struct Query<T>(pub T);

/// A name a person chose: not blank and at most 100 characters.
pub fn name(value: &str) -> Result<&str, ApiError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 100 {
        return Err(ApiError::BadRequest("a name must have 1 to 100 characters".into()));
    }
    Ok(value)
}
