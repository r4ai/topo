//! The `topo` binary against the real API, served in this process over SQLite.

use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::Arc;

use serde_json::Value;
use tempfile::TempDir;
use topo_server::db::BoxFuture;
use topo_server::github::{GitHub, GitHubUser};
use topo_server::sqlite::SqliteDb;
use topo_server::{AppState, router};

/// GitHub with one user, whose access token is `gh-alice`.
struct FakeGitHub;

impl GitHub for FakeGitHub {
    fn check_token<'a>(&'a self, access_token: &'a str) -> BoxFuture<'a, Result<Option<GitHubUser>, String>> {
        let user = (access_token == "gh-alice").then(|| GitHubUser { id: 1, login: "alice".into() });
        Box::pin(async move { Ok(user) })
    }

    fn user_by_login<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<Option<GitHubUser>, String>> {
        Box::pin(async { Ok(None) })
    }
}

/// Serves the API on a free local port and returns its URL and a token of alice.
fn serve() -> (String, String) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let state = AppState { db: Arc::new(SqliteDb::new()), github: Arc::new(FakeGitHub), github_client_id: "c".into() };
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_io().build().unwrap();
        runtime.block_on(async {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            axum::serve(listener, router(state)).await.unwrap();
        });
    });
    let token = topo_cloud::Client::new(&url, String::new()).sign_in("gh-alice".into(), "test".into()).unwrap().token;
    (url, token)
}

/// A directory in which `topo` runs as a user with the given token.
struct Dir {
    root: TempDir,
    token: String,
}

impl Dir {
    fn new(token: &str) -> Self {
        Self { root: tempfile::tempdir().unwrap(), token: token.to_owned() }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_topo"));
        command.current_dir(self.root.path()).args(args).env("TOPO_TOKEN", &self.token);
        // Neither the developer's credentials nor their environment reach the tests.
        command.env("XDG_CONFIG_HOME", self.root.path().join("config")).env_remove("TOPO_AGENT");
        command.env_remove("TOPO_DIR").env_remove("TOPO_CLOUD_URL");
        command
    }

    fn ok(&self, args: &[&str]) -> String {
        success(self.command(args).output().unwrap())
    }

    fn json(&self, args: &[&str]) -> Value {
        serde_json::from_str(&self.ok(args)).unwrap()
    }

    fn fails(&self, args: &[&str]) -> String {
        failure(self.command(args).output().unwrap())
    }

    fn nodes(&self) -> Vec<String> {
        let mut titles: Vec<String> = self
            .json(&["ls", "--all", "--json"])
            .as_array()
            .unwrap()
            .iter()
            .map(|n| {
                format!(
                    "{} {} {}",
                    n["id"].as_str().unwrap(),
                    n["status"].as_str().unwrap(),
                    n["title"].as_str().unwrap()
                )
            })
            .collect();
        titles.sort();
        titles
    }

    fn path(&self, relative: &str) -> std::path::PathBuf {
        self.root.path().join(relative)
    }
}

fn success(output: Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    String::from_utf8(output.stdout).unwrap().trim_end().to_owned()
}

fn failure(output: Output) -> String {
    assert!(!output.status.success(), "{}", String::from_utf8_lossy(&output.stdout));
    String::from_utf8(output.stderr).unwrap()
}

fn files(dir: &Path) -> usize {
    std::fs::read_dir(dir).unwrap().count()
}

#[test]
fn a_workspace_moves_to_the_cloud_and_back() {
    let (url, token) = serve();
    let one = Dir::new(&token);
    one.ok(&["init"]);
    let milestone = one.ok(&["add", "v1", "--milestone", "--due", "2026-10-31"]);
    let design = one.ok(&["add", "design", "--in", &milestone, "--note", "notes"]);
    one.ok(&["add", "build", "--in", &milestone, "--dep", &design, "--tag", "x"]);
    let local = one.nodes();

    let pushed = one.json(&["cloud", "push", "--url", &url, "--name", "demo", "--json"]);
    let workspace = pushed["workspace"].as_str().unwrap();
    assert_eq!(one.nodes(), local);
    assert!(one.fails(&["cloud", "push", "--url", &url]).contains("already linked"));
    assert_eq!(one.json(&["cloud", "ls", "--json"])[0]["name"], "demo");

    // A second checkout links to the same workspace and sees the same graph.
    let two = Dir::new(&token);
    two.ok(&["cloud", "link", workspace, "--url", &url]);
    assert_eq!(two.nodes(), local);
    assert_eq!(two.ok(&["ready"]), one.ok(&["ready"]));
    assert!(two.ok(&["show", &design]).contains("notes"));

    // Writes from either side reach the other, whichever kind of command makes them.
    let extra = two.ok(&["add", "ship", "--dep", &design[..3]]);
    one.ok(&["status", &design, "done"]);
    one.ok(&["edit", &extra, "--title", "release", "--due", "2026-11-01"]);
    two.ok(&["edit", &extra, "--no-due"]);
    let batch = r#"[{"op": "add", "ref": "t", "title": "test"}, {"op": "link", "from": "$t", "to": "EXTRA"}]"#;
    let batch = batch.replace("EXTRA", &extra[..4]);
    std::fs::write(two.path("batch.json"), batch).unwrap();
    assert!(two.ok(&["apply", "batch.json"]).starts_with("$t = "));
    assert_eq!(one.nodes(), two.nodes());
    assert_eq!(one.nodes().len(), 5);
    assert!(one.nodes().contains(&format!("{extra} todo release")));
    assert!(one.fails(&["link", &design, &extra]).contains("cycle"));

    let log = one.json(&["cloud", "log", "--json"]);
    assert_eq!(log.as_array().unwrap().len(), 6);
    assert_eq!(log[5]["ops"][1]["op"], "link");

    // Pulling writes the files again and cuts the link.
    assert_eq!(files(&two.path(".topo")), 1);
    two.ok(&["cloud", "pull"]);
    assert_eq!(files(&two.path(".topo/nodes")), 5);
    assert!(!std::fs::read_to_string(two.path(".topo/config.toml")).unwrap().contains("cloud"));
    two.ok(&["rm", &extra]);
    assert_eq!(two.nodes().len(), 4);
    assert_eq!(one.nodes().len(), 5);
}

#[test]
fn of_two_agents_claiming_a_task_one_wins() {
    let (url, token) = serve();
    let dir = Dir::new(&token);
    dir.ok(&["init"]);
    let task = dir.ok(&["add", "task"]);
    let workspace = dir.json(&["cloud", "push", "--url", &url, "--json"])["workspace"].as_str().unwrap().to_owned();

    // Each agent has a token of its own, restricted to the workspace.
    let agent = |name: &str| {
        let secret = dir.ok(&["token", "create", "--name", name, "--workspace", &workspace, "--expires", "1h"]);
        assert!(secret.starts_with("topo_"));
        let agent = Dir::new(&secret);
        std::fs::create_dir(agent.path(".topo")).unwrap();
        std::fs::copy(dir.path(".topo/config.toml"), agent.path(".topo/config.toml")).unwrap();
        agent
    };
    let (a, b) = (agent("agent-a"), agent("agent-b"));
    assert_eq!(a.ok(&["ready", "--format", "ids"]), task);
    assert!(a.fails(&["cloud", "ls"]).contains("restricted to a workspace"));

    let claim = |agent: &Dir, label: &str| {
        let mut command = agent.command(&["status", &task, "doing", "--if", "todo"]);
        command.env("TOPO_AGENT", label).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap()
    };
    let (first, second) = (claim(&a, "task-1"), claim(&b, "task-2"));
    let outcomes = [first.wait_with_output().unwrap(), second.wait_with_output().unwrap()];
    let winners: Vec<&Output> = outcomes.iter().filter(|o| o.status.success()).collect();
    assert_eq!(winners.len(), 1, "{outcomes:?}");
    assert_eq!(dir.nodes(), [format!("{task} doing task")]);

    let log = dir.json(&["cloud", "log", "--after", "1", "--json"]);
    let claims = log.as_array().unwrap();
    assert_eq!(claims.len(), 1);
    let (winner, label) = (claims[0]["token"].as_str().unwrap(), claims[0]["agent"].as_str().unwrap());
    assert!(matches!((winner, label), ("agent-a", "task-1") | ("agent-b", "task-2")), "{winner} {label}");
    assert_eq!(dir.json(&["token", "ls", "--json"]).as_array().unwrap().len(), 3);
}

#[test]
fn a_linked_workspace_needs_a_token() {
    let (url, token) = serve();
    let dir = Dir::new(&token);
    dir.ok(&["init"]);
    dir.ok(&["cloud", "push", "--url", &url]);

    let no_token = failure(dir.command(&["ls"]).env_remove("TOPO_TOKEN").output().unwrap());
    assert!(no_token.contains("not signed in to http://127.0.0.1"), "{no_token}");
    let wrong = failure(dir.command(&["ls"]).env("TOPO_TOKEN", "topo_wrong").output().unwrap());
    assert!(wrong.contains("the token is missing, unknown, or expired"), "{wrong}");
    assert!(dir.fails(&["token", "create", "--name", "x", "--expires", "soon"]).contains("followed by s, m, h, or d"));
}
