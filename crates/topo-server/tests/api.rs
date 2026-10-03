//! The API over an in-memory SQLite database and a fake GitHub.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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
    assert_eq!(members, json!([{ "login": "alice", "role": "owner" }, { "login": "bob", "role": "editor" }]));
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
