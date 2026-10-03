//! Blocking HTTP client of the cloud API.

use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use topo_core::wire::{
    ApplyRequest, ApplyResult, AuthConfig, Change, CreatedToken, ErrorBody, GithubSignIn, ImportRequest, Member,
    MemberRole, NewToken, Role, Snapshot, Token, User, WireNode, WorkspaceInfo, WorkspaceName,
};
use topo_core::{Op, Remote};
use ureq::http::{Method, Request, StatusCode};
use ureq::tls::{RootCerts, TlsConfig};

use crate::Error;

#[derive(Clone)]
pub struct Client {
    agent: ureq::Agent,
    url: String,
    token: String,
    /// Label recorded with every write, from `TOPO_AGENT`.
    label: Option<String>,
}

/// How often a write is sent again after the server answered `busy`.
const BUSY_RETRIES: u32 = 5;

impl Client {
    /// A client for the server at `url`. An empty `token` suits only the sign-in calls.
    pub fn new(url: &str, token: String) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(15)))
            // Error responses carry a body that says what went wrong.
            .http_status_as_error(false)
            .tls_config(TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build())
            .build();
        let label = std::env::var("TOPO_AGENT").ok().filter(|label| !label.is_empty());
        Self { agent: config.into(), url: url.trim_end_matches('/').to_owned(), token, label }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    /// Sends a request and returns the response if its status is `expected`.
    fn send(
        &self,
        method: Method,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<&impl Serialize>,
        expected: &[StatusCode],
    ) -> Result<ureq::http::Response<ureq::Body>, Error> {
        let url = format!("{}{path}", self.url);
        let mut request = Request::builder().method(method).uri(&url);
        if !self.token.is_empty() {
            request = request.header("authorization", format!("Bearer {}", self.token));
        }
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let text = match body {
            Some(body) => {
                request = request.header("content-type", "application/json");
                serde_json::to_string(body).expect("request bodies serialize to JSON")
            }
            None => String::new(),
        };
        let request = request.body(text).expect("the request is well formed");
        let http = |source| Error::Http { url: url.clone(), source };
        let mut response = self.agent.run(request).map_err(http)?;
        let status = response.status();
        if expected.contains(&status) {
            return Ok(response);
        }
        let text = response.body_mut().read_to_string().map_err(http)?;
        Err(match serde_json::from_str::<ErrorBody>(&text) {
            Ok(body) => Error::Api { status: status.as_u16(), code: body.error.code, message: body.error.message },
            // Not an answer of the API: a proxy or another server.
            Err(_) => Error::Api { status: status.as_u16(), code: String::new(), message: format!("{url}: {status}") },
        })
    }

    fn json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<&impl Serialize>,
    ) -> Result<T, Error> {
        let mut response = self.send(method, path, headers, body, &[StatusCode::OK])?;
        let url = format!("{}{path}", self.url);
        response.body_mut().read_json().map_err(|source| Error::Http { url, source })
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        self.json(Method::GET, path, &[], None::<&()>)
    }

    fn done(&self, method: Method, path: &str, body: Option<&impl Serialize>) -> Result<(), Error> {
        self.send(method, path, &[], body, &[StatusCode::NO_CONTENT]).map(|_| ())
    }

    /// Sends a write, again with the same key while the server is busy with others.
    fn write<T: DeserializeOwned>(&self, method: Method, path: &str, body: &impl Serialize) -> Result<T, Error> {
        let key = format!("{:032x}", fastrand::u128(..));
        let mut headers = vec![("idempotency-key", key.as_str())];
        headers.extend(self.label.as_deref().map(|label| ("topo-agent", label)));
        let mut attempt = 0;
        loop {
            match self.json(method.clone(), path, &headers, Some(body)) {
                Err(Error::Api { code, .. }) if code == "busy" && attempt < BUSY_RETRIES => {
                    attempt += 1;
                    std::thread::sleep(Duration::from_millis(fastrand::u64(50..150) * u64::from(attempt)));
                }
                result => return result,
            }
        }
    }

    // ---- signing in --------------------------------------------------------

    pub fn auth_config(&self) -> Result<AuthConfig, Error> {
        self.get("/v1/auth/config")
    }

    /// Exchanges a GitHub access token for a token of the server, named `name`.
    pub fn sign_in(&self, access_token: String, name: String) -> Result<CreatedToken, Error> {
        self.json(Method::POST, "/v1/auth/github", &[], Some(&GithubSignIn { access_token, name }))
    }

    pub fn user(&self) -> Result<User, Error> {
        self.get("/v1/user")
    }

    // ---- tokens ------------------------------------------------------------

    pub fn tokens(&self) -> Result<Vec<Token>, Error> {
        self.get("/v1/tokens")
    }

    pub fn create_token(&self, token: &NewToken) -> Result<CreatedToken, Error> {
        self.json(Method::POST, "/v1/tokens", &[], Some(token))
    }

    pub fn revoke_token(&self, id: &str) -> Result<(), Error> {
        self.done(Method::DELETE, &format!("/v1/tokens/{id}"), None::<&()>)
    }

    // ---- workspaces and members --------------------------------------------

    pub fn workspaces(&self) -> Result<Vec<WorkspaceInfo>, Error> {
        self.get("/v1/workspaces")
    }

    pub fn create_workspace(&self, name: &str) -> Result<WorkspaceInfo, Error> {
        self.json(Method::POST, "/v1/workspaces", &[], Some(&WorkspaceName { name: name.to_owned() }))
    }

    pub fn delete_workspace(&self, workspace: &str) -> Result<(), Error> {
        self.done(Method::DELETE, &format!("/v1/workspaces/{workspace}"), None::<&()>)
    }

    pub fn members(&self, workspace: &str) -> Result<Vec<Member>, Error> {
        self.get(&format!("/v1/workspaces/{workspace}/members"))
    }

    pub fn put_member(&self, workspace: &str, login: &str, role: Role) -> Result<(), Error> {
        self.done(Method::PUT, &format!("/v1/workspaces/{workspace}/members/{login}"), Some(&MemberRole { role }))
    }

    pub fn remove_member(&self, workspace: &str, login: &str) -> Result<(), Error> {
        self.done(Method::DELETE, &format!("/v1/workspaces/{workspace}/members/{login}"), None::<&()>)
    }

    // ---- nodes -------------------------------------------------------------

    /// The workspace, or `None` if it is still at version `known`.
    pub fn graph(&self, workspace: &str, known: Option<u64>) -> Result<Option<Snapshot>, Error> {
        let etag = known.map(|version| format!("\"{version}\""));
        let headers: Vec<(&str, &str)> = etag.iter().map(|etag| ("if-none-match", etag.as_str())).collect();
        let path = format!("/v1/workspaces/{workspace}/graph");
        let expected = [StatusCode::OK, StatusCode::NOT_MODIFIED];
        let mut response = self.send(Method::GET, &path, &headers, None::<&()>, &expected)?;
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(None);
        }
        let url = format!("{}{path}", self.url);
        response.body_mut().read_json().map(Some).map_err(|source| Error::Http { url, source })
    }

    pub fn apply(&self, workspace: &str, ops: &[Op]) -> Result<ApplyResult, Error> {
        self.write(Method::POST, &format!("/v1/workspaces/{workspace}/apply"), &ApplyRequest { ops: ops.to_vec() })
    }

    /// Replaces every node of the workspace.
    pub fn import(&self, workspace: &str, nodes: Vec<WireNode>) -> Result<ApplyResult, Error> {
        self.write(Method::PUT, &format!("/v1/workspaces/{workspace}/graph"), &ImportRequest { nodes })
    }

    /// The recorded writes after a version, oldest first.
    pub fn changes(&self, workspace: &str, after: u64) -> Result<Vec<Change>, Error> {
        self.get(&format!("/v1/workspaces/{workspace}/changes?after={after}"))
    }
}

/// One workspace of a server, as the store of a `Workspace`.
pub struct HttpRemote {
    client: Client,
    workspace: String,
}

impl HttpRemote {
    pub fn new(client: Client, workspace: String) -> Self {
        Self { client, workspace }
    }
}

fn remote(e: Error) -> topo_core::Error {
    topo_core::Error::Remote(Box::new(e))
}

impl Remote for HttpRemote {
    fn fetch(&self, known: Option<u64>) -> Result<Option<Snapshot>, topo_core::Error> {
        self.client.graph(&self.workspace, known).map_err(remote)
    }

    fn apply(&self, ops: &[Op]) -> Result<ApplyResult, topo_core::Error> {
        self.client.apply(&self.workspace, ops).map_err(remote)
    }
}

#[cfg(test)]
mod tests {
    use mockito::Matcher;

    use super::*;

    #[test]
    fn sends_the_token_verbatim_and_reads_the_version_headers() {
        let mut server = mockito::Server::new();
        // A sandbox may hand out a placeholder that only its proxy can turn into the token.
        let client = Client::new(&format!("{}/", server.url()), "{{placeholder}}".into());
        let unchanged = server
            .mock("GET", "/v1/workspaces/w/graph")
            .match_header("authorization", "Bearer {{placeholder}}")
            .match_header("if-none-match", "\"7\"")
            .with_status(304)
            .create();
        assert!(client.graph("w", Some(7)).unwrap().is_none());
        unchanged.assert();

        let fresh = server
            .mock("GET", "/v1/workspaces/w/graph")
            .match_header("if-none-match", Matcher::Missing)
            .with_body(r#"{"version":8,"nodes":[{"id":"a","title":"a","body":"n"}]}"#)
            .create();
        let snapshot = client.graph("w", None).unwrap().unwrap();
        assert_eq!((snapshot.version, snapshot.nodes.len()), (8, 1));
        fresh.assert();
    }

    #[test]
    fn a_busy_write_is_sent_again_with_the_same_key() {
        let mut server = mockito::Server::new();
        let client = Client::new(&server.url(), "t".into());
        let keys = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = keys.clone();
        // Of the mocks that match, the oldest one still expecting requests answers.
        let busy = server
            .mock("POST", "/v1/workspaces/w/apply")
            .match_request(move |request| {
                seen.lock().unwrap().push(request.header("idempotency-key")[0].to_str().unwrap().to_owned());
                true
            })
            .with_status(503)
            .with_body(r#"{"error":{"code":"busy","message":"busy"}}"#)
            .expect(2)
            .create();
        let done = server
            .mock("POST", "/v1/workspaces/w/apply")
            .match_header("idempotency-key", Matcher::Regex("^[0-9a-f]{32}$".into()))
            .match_body(Matcher::JsonString(r#"{"ops":[{"op":"remove","id":"a"}]}"#.into()))
            .with_body(r#"{"version":3,"created":{}}"#)
            .create();
        let result = client.apply("w", &[Op::Remove { id: "a".into() }]).unwrap();
        assert_eq!(result.version, 3);
        busy.assert();
        done.assert();
        let keys = keys.lock().unwrap();
        assert!(keys.len() >= 2 && keys.iter().all(|key| key == &keys[0]), "{keys:?}");
    }

    #[test]
    fn errors_carry_the_code_and_message_of_the_api() {
        let mut server = mockito::Server::new();
        let client = Client::new(&server.url(), "t".into());
        server
            .mock("GET", "/v1/user")
            .with_status(401)
            .with_body(r#"{"error":{"code":"unauthenticated","message":"the token is missing"}}"#)
            .create();
        let error = client.user().unwrap_err();
        assert!(matches!(&error, Error::Api { status: 401, code, .. } if code == "unauthenticated"));
        assert_eq!(error.to_string(), "the token is missing");

        server.mock("GET", "/v1/tokens").with_status(502).with_body("<html>bad gateway</html>").create();
        assert!(client.tokens().unwrap_err().to_string().ends_with("/v1/tokens: 502 Bad Gateway"));
    }
}
