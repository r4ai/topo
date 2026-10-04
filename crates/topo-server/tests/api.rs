//! The API over an in-memory SQLite database and a fake GitHub.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use futures::executor::block_on;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use topo_server::db::{BoxFuture, Db, DbError, Rows};
use topo_server::github::{GitHub, GitHubUser};
use topo_server::sql::{self, Stmt};
use topo_server::sqlite::SqliteDb;
use topo_server::{AppState, router};
use tower::ServiceExt;

/// GitHub with three users. An access token is `gh-<login>`.
struct FakeGitHub;

const USERS: [(u64, &str); 3] = [(1, "alice"), (2, "bob"), (3, "carol")];

fn user(login: &str) -> Option<GitHubUser> {
    USERS.iter().find(|(_, l)| l.eq_ignore_ascii_case(login)).map(|(id, l)| GitHubUser { id: *id, login: (*l).into() })
}

impl GitHub for FakeGitHub {
    fn check_token<'a>(&'a self, access_token: &'a str) -> BoxFuture<'a, Result<Option<GitHubUser>, String>> {
        Box::pin(async move { Ok(access_token.strip_prefix("gh-").and_then(user)) })
    }

    fn user_by_login<'a>(&'a self, login: &'a str) -> BoxFuture<'a, Result<Option<GitHubUser>, String>> {
        Box::pin(async move { Ok(user(login)) })
    }
}

struct Api(Router);

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
}

impl Api {
    fn new() -> Self {
        Self::over(Arc::new(SqliteDb::new()))
    }

    fn over(db: Arc<dyn Db>) -> Self {
        Self(router(AppState { db, github: Arc::new(FakeGitHub), github_client_id: "client".into() }))
    }

    fn call(&self, method: &str, path: &str, token: &str, headers: &[(&str, &str)], body: Option<Value>) -> Reply {
        let mut request = Request::builder().method(method).uri(path);
        if !token.is_empty() {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let body = match body {
            Some(json) => {
                request = request.header("content-type", "application/json");
                Body::from(json.to_string())
            }
            None => Body::empty(),
        };
        let response = block_on(self.0.clone().oneshot(request.body(body).unwrap())).unwrap();
        let (parts, body) = response.into_parts();
        let bytes = block_on(body.collect()).unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        Reply { status: parts.status, headers: parts.headers, body }
    }

    fn get(&self, path: &str, token: &str) -> Reply {
        self.call("GET", path, token, &[], None)
    }

    fn send(&self, method: &str, path: &str, token: &str, body: Value) -> Reply {
        self.call(method, path, token, &[], Some(body))
    }

    /// Signs a user in and returns their token.
    fn sign_in(&self, login: &str) -> String {
        let reply =
            self.send("POST", "/v1/auth/github", "", json!({ "access_token": format!("gh-{login}"), "name": "test" }));
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
        reply.body["token"].as_str().unwrap().to_owned()
    }

    fn workspace(&self, token: &str) -> String {
        let reply = self.send("POST", "/v1/workspaces", token, json!({ "name": "ws" }));
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
        reply.body["id"].as_str().unwrap().to_owned()
    }

    fn apply(&self, wid: &str, token: &str, key: &str, ops: Value) -> Reply {
        let path = format!("/v1/workspaces/{wid}/apply");
        self.call("POST", &path, token, &[("idempotency-key", key)], Some(json!({ "ops": ops })))
    }
}

fn code(reply: &Reply) -> (u16, &str) {
    (reply.status.as_u16(), reply.body["error"]["code"].as_str().unwrap_or(""))
}

/// Inserts a competing transaction immediately before the selected database batch.
#[derive(Default)]
struct Interleaving {
    inner: SqliteDb,
    next: std::sync::Mutex<Option<(&'static str, Vec<Stmt>)>>,
}

impl Interleaving {
    fn before(&self, sql_prefix: &'static str, statements: Vec<Stmt>) {
        *self.next.lock().unwrap() = Some((sql_prefix, statements));
    }
}

impl Db for Interleaving {
    fn batch(&self, statements: Vec<Stmt>) -> BoxFuture<'_, Result<Vec<Rows>, DbError>> {
        Box::pin(async move {
            let competing = {
                let mut next = self.next.lock().unwrap();
                if next.as_ref().is_some_and(|(prefix, _)| statements.iter().any(|s| s.sql().starts_with(prefix))) {
                    next.take().map(|(_, statements)| statements)
                } else {
                    None
                }
            };
            if let Some(competing) = competing {
                self.inner.batch(competing).await?;
            }
            self.inner.batch(statements).await
        })
    }
}

#[test]
fn revoked_owners_and_tokens_cannot_finish_pending_mutations() {
    for operation in ["member", "remove", "rename", "delete", "apply", "import", "token"] {
        let db = Arc::new(Interleaving::default());
        let api = Api::over(db.clone());
        let alice = api.sign_in("alice");
        let bob = api.sign_in("bob");
        let wid = api.workspace(&alice);
        block_on(db.inner.batch(vec![sql::upsert_member(&wid, 2, topo_core::wire::Role::Owner)])).unwrap();
        let (prefix, competing) = if operation == "token" {
            let tokens = api.get("/v1/tokens", &alice).body;
            let token_id = tokens[0]["id"].as_str().unwrap();
            ("INSERT INTO \"changes\"", sql::delete_token(token_id, 1))
        } else {
            let prefix = match operation {
                "member" => "INSERT INTO \"workspace_members\"",
                "remove" => "DELETE FROM \"workspace_members\"",
                "rename" => "UPDATE \"workspaces\"",
                "delete" => "DELETE FROM \"workspaces\"",
                _ => "INSERT INTO \"changes\"",
            };
            (prefix, sql::delete_member(&wid, 1))
        };
        db.before(prefix, vec![competing]);
        let response = match operation {
            "member" => {
                api.send("PUT", &format!("/v1/workspaces/{wid}/members/alice"), &alice, json!({ "role": "owner" }))
            }
            "remove" => api.call("DELETE", &format!("/v1/workspaces/{wid}/members/bob"), &alice, &[], None),
            "rename" => api.send("PATCH", &format!("/v1/workspaces/{wid}"), &alice, json!({ "name": "changed" })),
            "delete" => api.call("DELETE", &format!("/v1/workspaces/{wid}"), &alice, &[], None),
            "import" => api.call(
                "PUT",
                &format!("/v1/workspaces/{wid}/graph"),
                &alice,
                &[("idempotency-key", "k")],
                Some(json!({ "nodes": [{ "id": "a", "title": "changed" }] })),
            ),
            _ => api.apply(&wid, &alice, "k", json!([{ "op": "add", "title": "changed" }])),
        };
        assert!(
            matches!(response.status.as_u16(), 401 | 403 | 404),
            "{operation}: {} {}",
            response.status,
            response.body
        );
        assert!(db.next.lock().unwrap().is_none(), "interleaving did not run: {operation}");
        let graph = api.get(&format!("/v1/workspaces/{wid}/graph"), &bob);
        assert_eq!(graph.status, StatusCode::OK);
        assert_eq!(graph.body["version"], 0);
        assert_eq!(graph.body["nodes"], json!([]));
        assert_eq!(api.get("/v1/workspaces", &bob).body[0]["name"], "ws");
        let members = api.get(&format!("/v1/workspaces/{wid}/members"), &bob).body;
        assert_eq!(
            members.as_array().unwrap().iter().filter(|m| m["role"] == "owner").count(),
            if operation == "token" { 2 } else { 1 }
        );
    }
}

#[test]
fn revoked_sessions_cannot_mint_replacement_tokens_or_create_workspaces() {
    for operation in ["token", "workspace"] {
        let db = Arc::new(Interleaving::default());
        let api = Api::over(db.clone());
        let alice = api.sign_in("alice");
        let token_id = api.get("/v1/tokens", &alice).body[0]["id"].as_str().unwrap().to_owned();
        let prefix = if operation == "token" { "INSERT INTO \"tokens\"" } else { "INSERT INTO \"workspaces\"" };
        db.before(prefix, vec![sql::delete_token(&token_id, 1)]);
        let path = if operation == "token" { "/v1/tokens" } else { "/v1/workspaces" };
        let response = api.send("POST", path, &alice, json!({ "name": "replacement" }));
        assert_eq!(code(&response), (401, "unauthenticated"), "{operation}: {}", response.body);
        assert!(db.next.lock().unwrap().is_none());
        let replacement = api.sign_in("alice");
        assert_eq!(api.get("/v1/tokens", &replacement).body.as_array().unwrap().len(), 1);
        assert_eq!(api.get("/v1/workspaces", &replacement).body, json!([]));
    }
}

#[test]
fn competing_owner_demotions_never_leave_the_workspace_without_an_owner() {
    for method in ["PUT", "DELETE"] {
        let db = Arc::new(Interleaving::default());
        let api = Api::over(db.clone());
        let alice = api.sign_in("alice");
        let wid = api.workspace(&alice);
        block_on(
            db.inner.batch(vec![sql::upsert_user(2, "bob"), sql::upsert_member(&wid, 2, topo_core::wire::Role::Owner)]),
        )
        .unwrap();
        let prefix =
            if method == "PUT" { "INSERT INTO \"workspace_members\"" } else { "DELETE FROM \"workspace_members\"" };
        db.before(prefix, vec![sql::upsert_member(&wid, 2, topo_core::wire::Role::Editor)]);
        let response = api.call(
            method,
            &format!("/v1/workspaces/{wid}/members/alice"),
            &alice,
            &[],
            (method == "PUT").then(|| json!({ "role": "editor" })),
        );
        assert_eq!(code(&response), (409, "conflict"), "{method}: {}", response.body);
        assert!(db.next.lock().unwrap().is_none());
        let members = api.get(&format!("/v1/workspaces/{wid}/members"), &alice).body;
        assert_eq!(members.as_array().unwrap().iter().filter(|m| m["role"] == "owner").count(), 1);
    }
}

#[test]
fn graph_read_rechecks_access_with_the_returned_snapshot() {
    for headers in [vec![], vec![("if-none-match", "\"0\"")], vec![("if-none-match", "\"1\"")]] {
        let db = Arc::new(Interleaving::default());
        let api = Api::over(db.clone());
        let alice = api.sign_in("alice");
        let wid = api.workspace(&alice);
        let n = topo_core::Node::new(
            topo_core::NodeId("a".into()),
            topo_core::Kind::Task,
            "created after revocation".into(),
        );
        db.before("SELECT \"id\", \"kind\"", vec![sql::delete_member(&wid, 1), sql::upsert_nodes(&wid, &[&n])]);
        let response = api.call("GET", &format!("/v1/workspaces/{wid}/graph"), &alice, &headers, None);
        assert!(db.next.lock().unwrap().is_none());
        assert_eq!(code(&response), (404, "not_found"));
        assert!(response.body.get("nodes").is_none());
    }
}

#[test]
fn import_checks_workspace_access_before_validating_the_graph() {
    let api = Api::new();
    let alice = api.sign_in("alice");
    let bob = api.sign_in("bob");
    let wid = api.workspace(&alice);
    let import = || {
        api.call(
            "PUT",
            &format!("/v1/workspaces/{wid}/graph"),
            &bob,
            &[("idempotency-key", "k")],
            Some(json!({ "nodes": [{ "id": "a", "title": "a", "depends_on": ["a"] }] })),
        )
    };
    assert_eq!(code(&import()), (404, "not_found"));
    api.send("PUT", &format!("/v1/workspaces/{wid}/members/bob"), &alice, json!({ "role": "viewer" }));
    assert_eq!(code(&import()), (403, "forbidden"));
}

#[test]
fn member_removal_uses_stable_identity_instead_of_cached_or_reassigned_logins() {
    let db = Arc::new(SqliteDb::new());
    let api = Api::over(db.clone());
    let alice = api.sign_in("alice");
    let wid = api.workspace(&alice);
    api.send("PUT", &format!("/v1/workspaces/{wid}/members/bob"), &alice, json!({ "role": "editor" }));
    // GitHub now assigns this label to carol (id 3), whereas our cached row is id 2.
    block_on(db.batch(vec![sql::upsert_user(2, "carol")])).unwrap();
    assert_eq!(
        code(&api.call("DELETE", &format!("/v1/workspaces/{wid}/members/carol"), &alice, &[], None)),
        (404, "not_found")
    );
    assert!(
        api.get(&format!("/v1/workspaces/{wid}/members"), &alice)
            .body
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["user_id"] == 2)
    );
    // A deleted account need not resolve on GitHub to revoke its membership by id.
    block_on(db.batch(vec![sql::upsert_user(2, "deleted-account")])).unwrap();
    assert_eq!(
        api.call("DELETE", &format!("/v1/workspaces/{wid}/members/by-id/2"), &alice, &[], None).status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        code(&api.call("DELETE", &format!("/v1/workspaces/{wid}/members/by-id/1"), &alice, &[], None)),
        (409, "conflict")
    );
}

#[test]
fn signing_in_exchanges_a_github_token_for_a_token() {
    let api = Api::new();
    assert_eq!(api.get("/v1/auth/config", "").body, json!({ "github_client_id": "client" }));
    let token = api.sign_in("alice");
    assert!(token.starts_with("topo_"));
    assert_eq!(api.get("/v1/user", &token).body, json!({ "id": 1, "login": "alice" }));

    let other_app = api.send("POST", "/v1/auth/github", "", json!({ "access_token": "gh-mallory", "name": "x" }));
    assert_eq!(code(&other_app), (401, "unauthenticated"));
    assert_eq!(code(&api.get("/v1/user", "topo_unknown")), (401, "unauthenticated"));
    assert_eq!(code(&api.get("/v1/user", "")), (401, "unauthenticated"));
    let malformed = api.send("POST", "/v1/auth/github", "", json!({ "access_token": "gh-alice" }));
    assert_eq!(code(&malformed), (400, "bad_request"));
}

#[test]
fn tokens_can_be_restricted_expired_and_revoked() {
    let api = Api::new();
    let alice = api.sign_in("alice");
    let (mine, other) = (api.workspace(&alice), api.workspace(&alice));
    let create = |body: Value| api.send("POST", "/v1/tokens", &alice, body);

    let agent = create(json!({ "name": "agent", "workspace_id": mine }));
    assert_eq!(agent.status, StatusCode::OK);
    let secret = agent.body["token"].as_str().unwrap();
    assert_eq!(api.apply(&mine, secret, "k", json!([{ "op": "add", "title": "t" }])).status, StatusCode::OK);
    assert_eq!(api.get(&format!("/v1/workspaces/{mine}/graph"), secret).status, StatusCode::OK);
    // It reaches nothing else: not another workspace, not the management routes.
    assert_eq!(code(&api.get(&format!("/v1/workspaces/{other}/graph"), secret)), (404, "not_found"));
    assert_eq!(code(&api.get("/v1/workspaces", secret)), (403, "forbidden"));
    assert_eq!(code(&api.get("/v1/tokens", secret)), (403, "forbidden"));
    assert_eq!(code(&api.send("POST", "/v1/tokens", secret, json!({ "name": "child" }))), (403, "forbidden"));
    assert_eq!(code(&api.get(&format!("/v1/workspaces/{mine}/members"), secret)), (403, "forbidden"));

    let expired = create(json!({ "name": "expired", "expires_in": 0 }));
    assert_eq!(code(&api.get("/v1/user", expired.body["token"].as_str().unwrap())), (401, "unauthenticated"));
    assert_eq!(code(&create(json!({ "name": "x", "workspace_id": "nope" }))), (404, "not_found"));
    assert_eq!(code(&create(json!({ "name": " " }))), (400, "bad_request"));
    assert_eq!(code(&create(json!({ "name": "x", "expires_in": u64::MAX }))), (400, "bad_request"));

    let listed = api.get("/v1/tokens", &alice).body;
    let names: Vec<&str> = listed.as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names.len(), 3);
    assert!(listed.to_string().find("topo_").is_none(), "secrets are never listed");

    let id = agent.body["id"].as_str().unwrap();
    assert_eq!(api.call("DELETE", &format!("/v1/tokens/{id}"), &alice, &[], None).status, StatusCode::NO_CONTENT);
    assert_eq!(code(&api.get(&format!("/v1/workspaces/{mine}/graph"), secret)), (401, "unauthenticated"));
    assert_eq!(code(&api.call("DELETE", &format!("/v1/tokens/{id}"), &alice, &[], None)), (404, "not_found"));
}

#[test]
fn members_get_the_access_of_their_role() {
    let api = Api::new();
    let (alice, bob, carol) = (api.sign_in("alice"), api.sign_in("bob"), api.sign_in("carol"));
    let wid = api.workspace(&alice);
    let graph = format!("/v1/workspaces/{wid}/graph");
    let member = |login: &str| format!("/v1/workspaces/{wid}/members/{login}");
    let add = [json!({ "op": "add", "title": "t" })];

    // A workspace does not exist for those outside it.
    assert_eq!(code(&api.get(&graph, &bob)), (404, "not_found"));
    assert_eq!(code(&api.apply(&wid, &bob, "k1", json!(add))), (404, "not_found"));

    assert_eq!(api.send("PUT", &member("Bob"), &alice, json!({ "role": "viewer" })).status, StatusCode::NO_CONTENT);
    assert_eq!(api.get(&graph, &bob).status, StatusCode::OK);
    assert_eq!(code(&api.apply(&wid, &bob, "k2", json!(add))), (403, "forbidden"));
    assert_eq!(code(&api.send("PUT", &member("carol"), &bob, json!({ "role": "viewer" }))), (403, "forbidden"));
    assert_eq!(
        code(&api.send("PATCH", &format!("/v1/workspaces/{wid}"), &bob, json!({ "name": "x" }))),
        (403, "forbidden")
    );

    assert_eq!(api.send("PUT", &member("bob"), &alice, json!({ "role": "editor" })).status, StatusCode::NO_CONTENT);
    assert_eq!(api.apply(&wid, &bob, "k3", json!(add)).status, StatusCode::OK);
    let members = api.get(&format!("/v1/workspaces/{wid}/members"), &bob).body;
    assert_eq!(
        members,
        json!([{ "user_id": 1, "login": "alice", "role": "owner" }, { "user_id": 2, "login": "bob", "role": "editor" }])
    );
    assert_eq!(api.get("/v1/workspaces", &bob).body, json!([{ "id": wid, "name": "ws", "role": "editor" }]));

    assert_eq!(code(&api.send("PUT", &member("nobody"), &alice, json!({ "role": "viewer" }))), (404, "not_found"));
    assert_eq!(code(&api.send("PUT", &member("a%2Fb"), &alice, json!({ "role": "viewer" }))), (400, "bad_request"));
    assert_eq!(code(&api.send("PUT", &member("alice"), &alice, json!({ "role": "editor" }))), (409, "conflict"));
    assert_eq!(code(&api.call("DELETE", &member("alice"), &alice, &[], None)), (409, "conflict"));
    assert_eq!(api.call("DELETE", &member("bob"), &alice, &[], None).status, StatusCode::NO_CONTENT);
    assert_eq!(code(&api.get(&graph, &bob)), (404, "not_found"));
    assert_eq!(code(&api.get(&graph, &carol)), (404, "not_found"));

    assert_eq!(api.call("DELETE", &format!("/v1/workspaces/{wid}"), &alice, &[], None).status, StatusCode::NO_CONTENT);
    assert_eq!(code(&api.get(&graph, &alice)), (404, "not_found"));
}

#[test]
fn apply_is_atomic_idempotent_and_conditional() {
    let api = Api::new();
    let alice = api.sign_in("alice");
    let wid = api.workspace(&alice);
    let graph = format!("/v1/workspaces/{wid}/graph");
    let batch = json!([
        { "op": "add", "ref": "m", "title": "v1", "kind": "milestone", "due": "2026-10-31" },
        { "op": "add", "ref": "a", "id": "aaaaaa", "title": "it's; DROP TABLE nodes", "in": ["$m"], "notes": "n" },
    ]);

    let first = api.apply(&wid, &alice, "k1", batch.clone());
    assert_eq!(first.status, StatusCode::OK, "{}", first.body);
    assert_eq!(first.body["version"], 1);
    assert_eq!(first.body["created"]["a"], "aaaaaa");
    // A retry with the same key returns the first result and writes nothing.
    assert_eq!(api.apply(&wid, &alice, "k1", batch).body, first.body);

    let snapshot = api.get(&graph, &alice);
    assert_eq!(snapshot.headers["etag"], "\"1\"");
    assert_eq!(snapshot.body["version"], 1);
    let task = &snapshot.body["nodes"].as_array().unwrap().iter().find(|n| n["id"] == "aaaaaa").unwrap();
    assert_eq!(
        **task,
        json!({
            "id": "aaaaaa", "kind": "task", "title": "it's; DROP TABLE nodes", "status": "todo",
            "milestones": [first.body["created"]["m"]], "body": "n",
            "created_at": task["created_at"], "updated_at": task["updated_at"],
        })
    );
    let unchanged = api.call("GET", &graph, &alice, &[("if-none-match", "\"1\"")], None);
    assert_eq!((unchanged.status, &unchanged.headers["etag"]), (StatusCode::NOT_MODIFIED, &"\"1\"".parse().unwrap()));
    assert_eq!(api.call("GET", &graph, &alice, &[("if-none-match", "\"0\"")], None).status, StatusCode::OK);

    // A batch that fails changes nothing, not even its valid operations.
    let failing =
        json!([{ "op": "add", "id": "bbbbbb", "title": "b" }, { "op": "link", "from": "aaaaaa", "to": "zzzzzz" }]);
    let failed = api.apply(&wid, &alice, "k2", failing);
    assert_eq!(code(&failed), (422, "invalid_graph"));
    assert!(failed.body["error"]["message"].as_str().unwrap().contains("operation #1"));
    assert_eq!(api.get(&graph, &alice).body, snapshot.body);

    let claim = json!([{ "op": "status", "id": "aaaaaa", "status": "doing", "if_status": "todo" }]);
    assert_eq!(api.apply(&wid, &alice, "k3", claim.clone()).body["version"], 2);
    assert_eq!(code(&api.apply(&wid, &alice, "k4", claim)), (409, "conflict"));

    let path = format!("/v1/workspaces/{wid}/apply");
    let clear = json!({ "ops": [{ "op": "edit", "id": "aaaaaa", "due": null, "title": "a" }] });
    let headers =
        |if_match: &'static str| [("idempotency-key", "k5"), ("if-match", if_match), ("topo-agent", "agent-7")];
    assert_eq!(
        code(&api.call("POST", &path, &alice, &headers("\"1\""), Some(clear.clone()))),
        (412, "precondition_failed")
    );
    assert_eq!(code(&api.call("POST", &path, &alice, &headers("x"), Some(clear.clone()))), (400, "bad_request"));
    assert_eq!(api.call("POST", &path, &alice, &headers("\"2\""), Some(clear.clone())).body["version"], 3);
    assert_eq!(code(&api.call("POST", &path, &alice, &[], Some(clear))), (400, "bad_request"));
    assert_eq!(
        code(&api.call("POST", &path, &alice, &[("idempotency-key", "k6")], Some(json!({ "ops": [{ "op": "x" }] })))),
        (400, "bad_request")
    );

    let changes = api.get(&format!("/v1/workspaces/{wid}/changes?after=1"), &alice).body;
    let summary: Vec<(u64, &str, &str, Option<&str>)> = changes
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["version"].as_u64().unwrap(),
                c["user"].as_str().unwrap(),
                c["token"].as_str().unwrap(),
                c["agent"].as_str(),
            )
        })
        .collect();
    assert_eq!(summary, [(2, "alice", "test", None), (3, "alice", "test", Some("agent-7"))]);
    assert_eq!(changes[1]["ops"], json!([{ "op": "edit", "id": "aaaaaa", "title": "a", "due": null }]));
    assert_eq!(code(&api.get(&format!("/v1/workspaces/{wid}/changes?limit=1001"), &alice)), (400, "bad_request"));
}

#[test]
fn import_replaces_the_graph_and_keeps_ids() {
    let api = Api::new();
    let alice = api.sign_in("alice");
    let wid = api.workspace(&alice);
    let graph = format!("/v1/workspaces/{wid}/graph");
    let put = |key: &'static str, nodes: Value| {
        api.call("PUT", &graph, &alice, &[("idempotency-key", key)], Some(json!({ "nodes": nodes })))
    };
    api.apply(&wid, &alice, "k0", json!([{ "op": "add", "id": "old", "title": "old" }]));

    let nodes = json!([
        { "id": "m", "kind": "milestone", "title": "m", "status": "todo" },
        { "id": "a", "kind": "task", "title": "a", "status": "done", "depends_on": ["b"], "milestones": ["m"], "body": "x" },
        { "id": "b", "kind": "task", "title": "b", "status": "todo", "tags": ["t"] },
    ]);
    assert_eq!(put("k1", nodes.clone()).body, json!({ "version": 2, "created": {} }));
    let mut stored = api.get(&graph, &alice).body["nodes"].as_array().unwrap().clone();
    stored.sort_by_key(|n| n["id"].as_str().unwrap().to_owned());
    let mut expected = nodes.as_array().unwrap().clone();
    expected.sort_by_key(|n| n["id"].as_str().unwrap().to_owned());
    assert_eq!(stored, expected);

    let cycle = json!([{ "id": "a", "title": "a", "depends_on": ["a"] }]);
    assert_eq!(code(&put("k2", cycle)), (422, "invalid_graph"));
    let traversal = json!([{ "id": "../../etc/passwd", "title": "a" }]);
    assert_eq!(code(&put("k3", traversal)), (422, "invalid_graph"));
    assert_eq!(api.get(&graph, &alice).body["version"], 2);
}

/// A database whose clock the test sets, in Unix seconds.
struct Clock {
    inner: SqliteDb,
    now: AtomicI64,
}

impl Db for Clock {
    fn batch(&self, statements: Vec<Stmt>) -> BoxFuture<'_, Result<Vec<Rows>, DbError>> {
        Box::pin(async move {
            let clock = statements.iter().position(|s| s.sql() == sql::clock().sql());
            let mut results = self.inner.batch(statements).await?;
            if let Some(index) = clock {
                results[index] = Rows(vec![json!({ "now": self.now.load(Ordering::SeqCst) })]);
            }
            Ok(results)
        })
    }
}

/// The node `id` of the workspace, as `GET .../graph` returns it.
fn stored(api: &Api, wid: &str, token: &str, id: &str) -> Value {
    let graph = api.get(&format!("/v1/workspaces/{wid}/graph"), token).body;
    graph["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).unwrap().clone()
}

#[test]
fn apply_stores_metadata_and_records_times() {
    let db = Arc::new(Clock { inner: SqliteDb::new(), now: AtomicI64::new(1_000) });
    let api = Api::over(db.clone());
    let alice = api.sign_in("alice");
    let wid = api.workspace(&alice);
    let at = |seconds: i64| json!(jiff::Timestamp::from_second(seconds).unwrap().to_string());
    assert_eq!(at(1_000), "1970-01-01T00:16:40Z");
    let node = || stored(&api, &wid, &alice, "a");
    let times = || {
        let n = node();
        [n["created_at"].clone(), n["updated_at"].clone(), n["completed_at"].clone()]
    };

    let add = json!([
        { "op": "add", "id": "a", "title": "a", "priority": "high", "assignee": " alice ", "prs": ["o/r#7", "https://example.com/pr/2/"] },
        { "op": "add", "id": "b", "title": "b" },
    ]);
    assert_eq!(api.apply(&wid, &alice, "k1", add.clone()).status, StatusCode::OK);
    let created = json!({
        "id": "a", "kind": "task", "title": "a", "status": "todo", "priority": "high", "assignee": "alice",
        "prs": ["https://github.com/o/r/pull/7", "https://example.com/pr/2"],
        "created_at": at(1_000), "updated_at": at(1_000),
    });
    assert_eq!(node(), created);
    // The change log holds what was applied: canonical values and no times.
    let log = api.get(&format!("/v1/workspaces/{wid}/changes"), &alice).body;
    assert_eq!(log[0]["ops"][0]["prs"], created["prs"]);
    assert_eq!(log[0]["ops"][0]["assignee"], "alice");
    assert!(log[0]["ops"][0].get("created_at").is_none());

    // A retry with the same key writes nothing, so no time moves.
    db.now.store(2_000, Ordering::SeqCst);
    assert_eq!(api.apply(&wid, &alice, "k1", add).body["version"], 1);
    assert_eq!(node(), created);
    // Neither does a write that leaves the node as it is: only `b` changes here.
    let same =
        json!([{ "op": "edit", "id": "a", "priority": "high" }, { "op": "status", "id": "b", "status": "doing" }]);
    assert_eq!(api.apply(&wid, &alice, "k2", same).body["version"], 2);
    assert_eq!(node(), created);
    assert_eq!(stored(&api, &wid, &alice, "b")["updated_at"], at(2_000));

    let done = json!([{ "op": "status", "id": "a", "status": "done" }]);
    assert_eq!(api.apply(&wid, &alice, "k3", done).body["version"], 3);
    assert_eq!(times(), [at(1_000), at(2_000), at(2_000)]);
    db.now.store(3_000, Ordering::SeqCst);
    let clear = json!([{ "op": "edit", "id": "a", "priority": null, "assignee": null, "prs": [] }]);
    assert_eq!(api.apply(&wid, &alice, "k4", clear).body["version"], 4);
    let cleared = node();
    assert!(["priority", "assignee", "prs"].iter().all(|field| cleared.get(field).is_none()), "{cleared}");
    assert_eq!(times(), [at(1_000), at(3_000), at(2_000)]);
    db.now.store(4_000, Ordering::SeqCst);
    let reopen = json!([{ "op": "status", "id": "a", "status": "todo" }]);
    assert_eq!(api.apply(&wid, &alice, "k5", reopen).body["version"], 5);
    assert_eq!(times(), [at(1_000), at(4_000), Value::Null]);

    for (key, edit) in [
        ("e1", json!({ "prs": ["nope"] })),
        ("e2", json!({ "prs": ["o/r#1", "https://github.com/o/r/pull/1"] })),
        ("e3", json!({ "assignee": " " })),
    ] {
        let mut op = json!({ "op": "edit", "id": "a" });
        op.as_object_mut().unwrap().extend(edit.as_object().unwrap().clone());
        assert_eq!(code(&api.apply(&wid, &alice, key, json!([op]))), (422, "invalid_graph"), "{key}");
    }
    let unknown = json!([{ "op": "edit", "id": "a", "priority": "p0" }]);
    assert_eq!(code(&api.apply(&wid, &alice, "e4", unknown)), (400, "bad_request"));
    assert_eq!(api.get(&format!("/v1/workspaces/{wid}/graph"), &alice).body["version"], 5);
}

#[test]
fn a_claim_assigns_the_task_to_the_one_agent_that_wins() {
    let api = Api::new();
    let alice = api.sign_in("alice");
    let wid = api.workspace(&alice);
    api.apply(&wid, &alice, "k0", json!([{ "op": "add", "id": "a", "title": "a" }]));
    let claim = |agent: &str| {
        let ops = json!([
            { "op": "status", "id": "a", "status": "doing", "if_status": "todo" },
            { "op": "edit", "id": "a", "assignee": agent },
        ]);
        api.apply(&wid, &alice, agent, ops)
    };
    assert_eq!(claim("agent-1").status, StatusCode::OK);
    assert_eq!(code(&claim("agent-2")), (409, "conflict"));
    let node = stored(&api, &wid, &alice, "a");
    assert_eq!((node["status"].as_str(), node["assignee"].as_str()), (Some("doing"), Some("agent-1")));
    assert_eq!(api.get(&format!("/v1/workspaces/{wid}/graph"), &alice).body["version"], 2);
}

#[test]
fn import_keeps_the_timestamps_it_is_given() {
    let db = Arc::new(Clock { inner: SqliteDb::new(), now: AtomicI64::new(1_000) });
    let api = Api::over(db.clone());
    let alice = api.sign_in("alice");
    let wid = api.workspace(&alice);
    let graph = format!("/v1/workspaces/{wid}/graph");
    let nodes = json!([
        { "id": "a", "kind": "task", "title": "a", "status": "done", "priority": "low", "assignee": "bob",
          "prs": ["https://github.com/o/r/pull/1"], "created_at": "2026-01-01T00:00:00Z",
          "updated_at": "2026-02-01T00:00:00Z", "completed_at": "2026-02-01T00:00:00Z" },
        // A node from before the timestamps existed: its times stay unknown.
        { "id": "b", "kind": "task", "title": "b", "status": "todo" },
    ]);
    let put = |key: &'static str, nodes: Value| {
        api.call("PUT", &graph, &alice, &[("idempotency-key", key)], Some(json!({ "nodes": nodes })))
    };
    assert_eq!(put("k1", nodes.clone()).status, StatusCode::OK);
    assert_eq!(stored(&api, &wid, &alice, "a"), nodes[0]);
    assert_eq!(stored(&api, &wid, &alice, "b"), nodes[1]);

    // A later change records the time of the server and keeps the creation time.
    api.apply(
        &wid,
        &alice,
        "k2",
        json!([{ "op": "edit", "id": "a", "title": "a2" }, { "op": "edit", "id": "b", "title": "b2" }]),
    );
    let (a, b) = (stored(&api, &wid, &alice, "a"), stored(&api, &wid, &alice, "b"));
    assert_eq!((&a["created_at"], &a["completed_at"]), (&nodes[0]["created_at"], &nodes[0]["completed_at"]));
    assert_eq!(a["updated_at"], "1970-01-01T00:16:40Z");
    assert_eq!((b.get("created_at"), &b["updated_at"]), (None, &a["updated_at"]));

    let unnormalized = json!([{ "id": "c", "title": "c", "prs": ["o/r#1"] }]);
    assert_eq!(code(&put("k3", unnormalized)), (422, "invalid_graph"));
}

/// A database in which other writers take the next version just before each of the first `races` writes.
struct Racing {
    inner: SqliteDb,
    workspace: std::sync::Mutex<String>,
    races: AtomicUsize,
}

impl Db for Racing {
    fn batch(&self, statements: Vec<Stmt>) -> BoxFuture<'_, Result<Vec<Rows>, DbError>> {
        Box::pin(async move {
            let writes = statements.first().is_some_and(|s| s.sql().starts_with("INSERT INTO \"changes\""));
            let race =
                writes && self.races.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok();
            if race {
                let wid = self.workspace.lock().unwrap().clone();
                let version =
                    self.inner.batch(vec![sql::version(&wid)]).await?.remove(0).0[0]["version"].as_u64().unwrap();
                let other = sql::insert_change(sql::NewChange {
                    workspace_id: &wid,
                    version: version + 1,
                    user_id: 1,
                    token_id: "other",
                    token_name: "other",
                    agent: None,
                    idempotency_key: &format!("other-{version}"),
                    ops: "[]".into(),
                    created: "{}".into(),
                });
                self.inner.batch(vec![other]).await?;
            }
            self.inner.batch(statements).await
        })
    }
}

#[test]
fn a_write_that_loses_its_version_is_recomputed() {
    let db = Arc::new(Racing { inner: SqliteDb::new(), workspace: Default::default(), races: AtomicUsize::new(0) });
    let api = Api::over(db.clone());
    let alice = api.sign_in("alice");
    let wid = api.workspace(&alice);
    *db.workspace.lock().unwrap() = wid.clone();
    let add = |key: &str| api.apply(&wid, &alice, key, json!([{ "op": "add", "title": key }]));

    db.races.store(2, Ordering::SeqCst);
    assert_eq!(add("k1").body["version"], 3);
    db.races.store(3, Ordering::SeqCst);
    assert_eq!(code(&add("k2")), (503, "busy"));
    let nodes = api.get(&format!("/v1/workspaces/{wid}/graph"), &alice).body;
    assert_eq!((nodes["version"].as_u64(), nodes["nodes"].as_array().unwrap().len()), (Some(6), 1));
}

#[test]
fn the_openapi_document_lists_every_route() {
    let api = Api::new();
    let spec = api.get("/openapi.json", "").body;
    // Methods are sorted here because the order of JSON keys depends on how serde_json is built.
    let operations: BTreeMap<&str, BTreeSet<&str>> = spec["paths"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(path, item)| (path.as_str(), item.as_object().unwrap().keys().map(String::as_str).collect()))
        .collect();
    let expected = BTreeMap::from([
        ("/v1/auth/config", BTreeSet::from(["get"])),
        ("/v1/auth/github", BTreeSet::from(["post"])),
        ("/v1/tokens", BTreeSet::from(["get", "post"])),
        ("/v1/tokens/{id}", BTreeSet::from(["delete"])),
        ("/v1/user", BTreeSet::from(["get"])),
        ("/v1/workspaces", BTreeSet::from(["get", "post"])),
        ("/v1/workspaces/{wid}", BTreeSet::from(["delete", "patch"])),
        ("/v1/workspaces/{wid}/apply", BTreeSet::from(["post"])),
        ("/v1/workspaces/{wid}/changes", BTreeSet::from(["get"])),
        ("/v1/workspaces/{wid}/graph", BTreeSet::from(["get", "put"])),
        ("/v1/workspaces/{wid}/members", BTreeSet::from(["get"])),
        ("/v1/workspaces/{wid}/members/{login}", BTreeSet::from(["delete", "put"])),
        ("/v1/workspaces/{wid}/members/by-id/{user_id}", BTreeSet::from(["delete"])),
    ]);
    assert_eq!(operations, expected);
    assert_eq!(spec["components"]["securitySchemes"]["token"]["scheme"], "bearer");
    // Sign-in needs no token; everything else inherits the document's requirement.
    assert_eq!(spec["paths"]["/v1/auth/github"]["post"]["security"], json!([{}]));
    assert!(spec["components"]["schemas"]["Op"]["oneOf"].as_array().unwrap().len() == 8);

    let docs = block_on(api.0.clone().oneshot(Request::get("/docs").body(Body::empty()).unwrap())).unwrap();
    assert_eq!(docs.status(), StatusCode::OK);
    let html = block_on(docs.into_body().collect()).unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&html).contains("scalar"));
}

#[derive(Default)]
struct CountedDb {
    inner: SqliteDb,
    batches: AtomicUsize,
    statements: AtomicUsize,
    node_rows: AtomicUsize,
}

impl CountedDb {
    fn reset(&self) {
        self.batches.store(0, Ordering::SeqCst);
        self.statements.store(0, Ordering::SeqCst);
        self.node_rows.store(0, Ordering::SeqCst);
    }
    fn counts(&self) -> (usize, usize, usize) {
        (
            self.batches.load(Ordering::SeqCst),
            self.statements.load(Ordering::SeqCst),
            self.node_rows.load(Ordering::SeqCst),
        )
    }
}
impl Db for CountedDb {
    fn batch(&self, statements: Vec<Stmt>) -> BoxFuture<'_, Result<Vec<Rows>, DbError>> {
        Box::pin(async move {
            self.batches.fetch_add(1, Ordering::SeqCst);
            self.statements.fetch_add(statements.len(), Ordering::SeqCst);
            let nodes: Vec<_> = statements
                .iter()
                .enumerate()
                .filter(|(_, s)| s.sql().starts_with("SELECT \"id\", \"kind\""))
                .map(|(i, _)| i)
                .collect();
            let rows = self.inner.batch(statements).await?;
            for i in nodes {
                self.node_rows.fetch_add(rows[i].0.len(), Ordering::SeqCst);
            }
            Ok(rows)
        })
    }
}

#[test]
fn reads_and_replays_bound_database_work_without_returning_stale_nodes() {
    let db = Arc::new(CountedDb::default());
    let api = Api::over(db.clone());
    let alice = api.sign_in("alice");
    let wid = api.workspace(&alice);
    let ops = json!([{"op":"add", "id":"a", "title":"a"}]);
    let first = api.apply(&wid, &alice, "first", ops.clone());
    assert_eq!(first.status, StatusCode::OK);
    let path = format!("/v1/workspaces/{wid}/graph");
    db.reset();
    let full = api.get(&path, &alice);
    assert_eq!(full.body["nodes"][0]["id"], "a");
    // Includes the separate bearer-token lookup; graph itself is one batch/three statements.
    assert_eq!(db.counts(), (2, 4, 1));
    db.reset();
    assert_eq!(api.call("GET", &path, &alice, &[("if-none-match", "\"1\"")], None).status, StatusCode::NOT_MODIFIED);
    assert_eq!(db.counts(), (2, 4, 0));
    db.reset();
    let replay = api.apply(&wid, &alice, "first", ops);
    assert_eq!(replay.body, first.body);
    assert_eq!(db.node_rows.load(Ordering::SeqCst), 0);
    // Unquoted or weak validators must not accidentally become a 304.
    for known in ["1", "W/\"1\"", "\"01\"", "\"18446744073709551616\""] {
        assert_eq!(api.call("GET", &path, &alice, &[("if-none-match", known)], None).status, StatusCode::OK);
    }
    api.apply(&wid, &alice, "second", json!([{"op":"add", "id":"b", "title":"b"}]));
    db.reset();
    let changed = api.call("GET", &path, &alice, &[("if-none-match", "\"1\"")], None);
    assert_eq!(changed.body["version"], 2);
    assert_eq!(changed.body["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(db.counts(), (2, 4, 2));
    let bob = api.sign_in("bob");
    assert_eq!(
        api.send("PUT", &format!("/v1/workspaces/{wid}/members/bob"), &alice, json!({"role":"viewer"})).status,
        StatusCode::NO_CONTENT
    );
    db.reset();
    assert_eq!(code(&api.apply(&wid, &bob, "denied", json!([]))), (403, "forbidden"));
    assert_eq!(db.node_rows.load(Ordering::SeqCst), 0);
    // Empty operations still have their own durable audit/idempotency record, but no node DML.
    db.reset();
    assert_eq!(api.apply(&wid, &alice, "empty", json!([])).status, StatusCode::OK);
    assert_eq!(db.counts(), (3, 7, 2));
}

#[test]
#[ignore = "manual native SQLite timing, not production D1 latency"]
fn graph_database_load_benchmark() {
    let db = Arc::new(SqliteDb::new());
    let api = Api::over(db.clone());
    let alice = api.sign_in("alice");
    let wid = api.workspace(&alice);
    let token_id = api.get("/v1/tokens", &alice).body[0]["id"].as_str().unwrap().to_owned();
    let nodes: Vec<_> = (0..5000)
        .map(|i| topo_core::Node::new(topo_core::NodeId(format!("n{i}")), topo_core::Kind::Task, format!("task {i}")))
        .collect();
    let refs: Vec<_> = nodes.iter().collect();
    block_on(db.batch(vec![sql::upsert_nodes(&wid, &refs)])).unwrap();
    let loops = 100;
    for mode in ["full_before", "full_after", "unchanged_before", "unchanged_after", "replay_before", "replay_after"] {
        let started = std::time::Instant::now();
        let mut returned = 0;
        for _ in 0..loops {
            if mode == "full_before" {
                block_on(db.batch(vec![sql::membership(&wid, 1, &token_id), sql::version(&wid)])).unwrap();
            }
            let statements = match mode {
                "unchanged_before" => vec![sql::membership(&wid, 1, &token_id), sql::version(&wid)],
                "unchanged_after" => {
                    vec![sql::membership(&wid, 1, &token_id), sql::version(&wid), sql::nodes_changed(&wid, Some(0))]
                }
                "replay_before" => vec![sql::nodes(&wid)],
                "replay_after" => vec![sql::nodes_for_write(&wid, "replay", 1, &token_id)],
                _ => vec![sql::membership(&wid, 1, &token_id), sql::version(&wid), sql::nodes(&wid)],
            };
            let mut rows = block_on(db.batch(statements)).unwrap();
            if mode != "unchanged_before" {
                returned += rows.pop().unwrap().0.len();
            }
        }
        eprintln!(
            "server_db_bench mode={mode} nodes=5000 loops={loops} elapsed_ms={:.3} node_rows={returned}",
            started.elapsed().as_secs_f64() * 1000.0
        );
        if mode == "unchanged_after" {
            // Establish the key used by the next replay measurements.
            assert_eq!(api.apply(&wid, &alice, "replay", json!([])).status, StatusCode::OK);
        }
    }
}
