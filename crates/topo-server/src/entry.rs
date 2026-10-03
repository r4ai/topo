//! The Cloudflare Workers entry point: the router over D1 and the GitHub API.

use std::sync::Arc;

use base64::Engine;
use serde::Deserialize;
use tower_service::Service;
use worker::send::SendFuture;
use worker::wasm_bindgen::JsValue;
use worker::{Context, D1Database, Env, Fetch, Headers, HttpRequest, Method, Request, RequestInit, event};

use crate::db::{BoxFuture, Db, DbError, Rows};
use crate::github::{GitHub, GitHubUser};
use crate::sql::{Param, Stmt};
use crate::{AppState, router};

#[event(fetch)]
async fn fetch(
    request: HttpRequest,
    env: Env,
    _ctx: Context,
) -> worker::Result<axum::http::Response<axum::body::Body>> {
    let client_id = env.var("GITHUB_CLIENT_ID")?.to_string();
    let credentials = format!("{client_id}:{}", env.secret("GITHUB_CLIENT_SECRET")?);
    let github = GitHubApi {
        client_id: client_id.clone(),
        authorization: format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(credentials)),
    };
    let state = AppState { db: Arc::new(D1(env.d1("DB")?)), github: Arc::new(github), github_client_id: client_id };
    Ok(router(state).call(request).await?)
}

struct D1(D1Database);

impl Db for D1 {
    fn batch(&self, statements: Vec<Stmt>) -> BoxFuture<'_, Result<Vec<Rows>, DbError>> {
        // Workers are single-threaded, so a future over JavaScript values can be called `Send`.
        Box::pin(SendFuture::new(async move {
            let prepared = statements.iter().map(|statement| {
                let params: Vec<JsValue> = statement
                    .params()
                    .iter()
                    .map(|param| match param {
                        Param::Null => JsValue::NULL,
                        // Every integer of the schema (ids, versions, times) fits a JavaScript number.
                        Param::Int(i) => JsValue::from_f64(*i as f64),
                        Param::Text(s) => JsValue::from_str(s),
                    })
                    .collect();
                self.0.prepare(statement.sql()).bind(&params)
            });
            let prepared = prepared.collect::<worker::Result<Vec<_>>>().map_err(db_error)?;
            let results = self.0.batch(prepared).await.map_err(db_error)?;
            results.iter().map(|result| result.results().map(Rows).map_err(db_error)).collect()
        }))
    }
}

/// D1 reports a violated constraint only as text.
fn db_error(e: worker::Error) -> DbError {
    let text = match &e {
        worker::Error::D1(d1) => format!("{e} {}", d1.cause()),
        _ => e.to_string(),
    };
    if text.contains("constraint failed") { DbError::Constraint } else { DbError::Other(text) }
}

struct GitHubApi {
    client_id: String,
    /// Basic credentials of the OAuth app.
    authorization: String,
}

impl GitHubApi {
    /// Sends a request and returns the user in its response, or `None` on the statuses that mean "no such thing".
    async fn user(
        &self,
        method: Method,
        url: String,
        body: Option<String>,
        missing: &[u16],
        user: impl FnOnce(serde_json::Value) -> serde_json::Value,
    ) -> Result<Option<GitHubUser>, String> {
        #[derive(Deserialize)]
        struct User {
            id: u64,
            login: String,
        }
        let run = async {
            let headers = Headers::new();
            headers.set("authorization", &self.authorization)?;
            headers.set("accept", "application/vnd.github+json")?;
            headers.set("user-agent", "topo-server")?;
            let mut init = RequestInit::new();
            init.with_method(method).with_headers(headers).with_body(body.map(|b| JsValue::from_str(&b)));
            let mut response = Fetch::Request(Request::new_with_init(&url, &init)?).send().await?;
            let status = response.status_code();
            if missing.contains(&status) {
                return Ok(None);
            }
            if status != 200 {
                return Err(worker::Error::RustError(format!("GitHub answered {status}")));
            }
            let user: User = serde_json::from_value(user(response.json().await?))?;
            Ok(Some(GitHubUser { id: user.id, login: user.login }))
        };
        run.await.map_err(|e: worker::Error| format!("GitHub: {e}"))
    }
}

impl GitHub for GitHubApi {
    fn check_token<'a>(&'a self, access_token: &'a str) -> BoxFuture<'a, Result<Option<GitHubUser>, String>> {
        let url = format!("https://api.github.com/applications/{}/token", self.client_id);
        let body = serde_json::json!({ "access_token": access_token }).to_string();
        // 404: GitHub does not know the token, or it belongs to another app. 422: it is malformed.
        Box::pin(SendFuture::new(
            self.user(Method::Post, url, Some(body), &[404, 422], |mut token| token["user"].take()),
        ))
    }

    fn user_by_login<'a>(&'a self, login: &'a str) -> BoxFuture<'a, Result<Option<GitHubUser>, String>> {
        let url = format!("https://api.github.com/users/{login}");
        Box::pin(SendFuture::new(self.user(Method::Get, url, None, &[404], |user| user)))
    }
}
