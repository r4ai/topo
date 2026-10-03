//! The cloud API of topo: an axum router over a database and GitHub.
//!
//! The router is the same on Cloudflare Workers (`entry`, with D1) and on the
//! host (with SQLite, for tests). `docs/cloud` describes the design.

pub mod auth;
pub mod db;
pub mod error;
pub mod github;
mod routes;
pub mod sql;
mod write;

#[cfg(target_arch = "wasm32")]
mod entry;
#[cfg(feature = "native")]
pub mod sqlite;

use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use utoipa::OpenApi;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use utoipa_scalar::{Scalar, Servable};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<dyn db::Db>,
    pub github: Arc<dyn github::GitHub>,
    /// Client id of the GitHub OAuth app, which clients need for the device flow.
    pub github_client_id: String,
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "topo cloud API",
        description = "Stores the nodes of topo workspaces and serializes writes to them. \
            The server answers no graph queries: clients download the graph and evaluate it."
    ),
    security(("token" = [])),
    modifiers(&BearerToken),
    tags(
        (name = "auth", description = "Signing in"),
        (name = "tokens", description = "Tokens for clients and agents"),
        (name = "workspaces", description = "Workspaces and their members"),
        (name = "graph", description = "Nodes of a workspace"),
    )
)]
struct ApiDoc;

struct BearerToken;

impl utoipa::Modify for BearerToken {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let scheme = HttpBuilder::new()
            .scheme(HttpAuthScheme::Bearer)
            .description(Some("A token from `POST /v1/auth/github` or `POST /v1/tokens`."))
            .build();
        openapi.components.get_or_insert_default().add_security_scheme("token", SecurityScheme::Http(scheme));
    }
}

/// The API, its OpenAPI document at `/openapi.json`, and its reference at `/docs`.
pub fn router(state: AppState) -> Router {
    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(routes::auth::config))
        .routes(routes!(routes::auth::github))
        .routes(routes!(routes::auth::user))
        .routes(routes!(routes::tokens::list, routes::tokens::create))
        .routes(routes!(routes::tokens::revoke))
        .routes(routes!(routes::workspaces::list, routes::workspaces::create))
        .routes(routes!(routes::workspaces::rename, routes::workspaces::delete))
        .routes(routes!(routes::members::list))
        .routes(routes!(routes::members::put, routes::members::remove))
        .routes(routes!(routes::graph::get, routes::graph::import))
        .routes(routes!(routes::graph::apply))
        .routes(routes!(routes::graph::changes))
        .split_for_parts();
    let spec = api.clone();
    router
        .route("/openapi.json", get(move || std::future::ready(axum::Json(spec.clone()))))
        .merge(Scalar::with_url("/docs", api))
        .with_state(state)
}
