//! The commands that talk to a topo cloud server without opening the graph:
//! signing in, tokens, and moving a workspace to or from the server.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use serde_json::json;
use topo_cloud::credentials::{self, Credential};
use topo_cloud::{Client, CloudConfig, config, device};
use topo_core::Workspace;
use topo_core::wire::{NewToken, Role};

use crate::{parse_enum, print, topo_dir};

#[derive(Args)]
pub struct Server {
    /// Base URL of the server (default: the one this workspace is linked to).
    #[arg(long, env = "TOPO_CLOUD_URL", global = true)]
    url: Option<String>,
}

#[derive(Subcommand)]
pub enum TokenCommand {
    /// Create a token and print its secret, which is shown only now.
    Create {
        #[arg(long)]
        name: String,
        /// Restrict the token to the nodes of one workspace (by id).
        #[arg(long, value_name = "ID")]
        workspace: Option<String>,
        /// Lifetime, such as `90d`, `12h`, `30m`, or `45s` (default: no expiry).
        #[arg(long, value_parser = parse_duration)]
        expires: Option<Duration>,
    },
    /// List your tokens.
    Ls,
    /// Revoke a token by its id.
    Revoke { id: String },
}

#[derive(Subcommand)]
pub enum CloudCommand {
    /// Create a cloud workspace from the local nodes and link this directory to it.
    Push {
        /// Name of the cloud workspace (default: the name of the directory).
        #[arg(long)]
        name: Option<String>,
    },
    /// Write the cloud workspace to `.topo/nodes` and unlink this directory.
    Pull,
    /// Link this directory to an existing cloud workspace.
    Link {
        /// Id of the workspace, as `topo cloud ls` shows it.
        workspace: String,
    },
    /// List your cloud workspaces.
    Ls,
    /// List the members of the linked workspace.
    Members,
    /// Add a GitHub user to the linked workspace, or change their role.
    Invite {
        login: String,
        #[arg(long, default_value = "editor", value_parser = parse_enum::<Role>)]
        role: Role,
    },
    /// Remove a member from the linked workspace.
    Remove {
        #[arg(required_unless_present = "user_id", conflicts_with = "user_id")]
        login: Option<String>,
        /// Stable GitHub id from `topo cloud members --json`.
        #[arg(long)]
        user_id: Option<u64>,
    },
    /// Show who changed the linked workspace.
    Log {
        /// Show the writes after this version.
        #[arg(long, default_value_t = 0)]
        after: u64,
    },
}

fn parse_duration(text: &str) -> Result<Duration, String> {
    let split = text.len().saturating_sub(1);
    let unit = match &text[split..] {
        "s" => 1,
        "m" => 60,
        "h" => 60 * 60,
        "d" => 24 * 60 * 60,
        _ => return Err("expected a number followed by s, m, h, or d".into()),
    };
    let count: u64 = text[..split].parse().map_err(|_| "expected a number followed by s, m, h, or d".to_owned())?;
    Ok(Duration::from_secs(count * unit))
}

/// The link of the workspace the command runs in, if it has one.
fn link(dir: &Option<PathBuf>) -> Result<Option<(PathBuf, CloudConfig)>> {
    let Ok(dir) = topo_dir(dir) else {
        return Ok(None);
    };
    Ok(config::load(&dir)?.map(|cloud| (dir, cloud)))
}

/// The server URL: the one given, or the one the workspace is linked to.
fn url(dir: &Option<PathBuf>, server: Server) -> Result<String> {
    if let Some(url) = server.url {
        return Ok(url.trim_end_matches('/').to_owned());
    }
    match link(dir)? {
        Some((_, cloud)) => Ok(cloud.url),
        None => bail!("which server? pass --url or set TOPO_CLOUD_URL"),
    }
}

fn client(dir: &Option<PathBuf>, server: Server) -> Result<Client> {
    let url = url(dir, server)?;
    Ok(Client::new(&url, credentials::token(&url)?))
}

/// The client and the workspace id of the linked workspace.
fn linked(dir: &Option<PathBuf>) -> Result<(PathBuf, Client, String)> {
    let (dir, cloud) =
        link(dir)?.context("this workspace is not linked to a cloud workspace (see `topo cloud push`)")?;
    let client = Client::new(&cloud.url, credentials::token(&cloud.url)?);
    Ok((dir, client, cloud.workspace))
}

pub fn login(dir: &Option<PathBuf>, server: Server, name: String, json: bool) -> Result<()> {
    let _ = dir;
    let url = server.url.context(
        "sign-in requires an explicit --url or TOPO_CLOUD_URL; verify the server before approving GitHub access",
    )?;
    let url = url.trim_end_matches('/').to_owned();
    let anonymous = Client::new(&url, String::new());
    let client_id = anonymous.auth_config()?.github_client_id;
    let access_token = device::sign_in(device::GITHUB, &client_id, |prompt| {
        eprintln!("Open {} and enter the code {}", prompt.verification_uri, prompt.user_code);
    })?;
    let created = anonymous.sign_in(access_token, name)?;
    let user = Client::new(&url, created.token.clone()).user()?;
    credentials::store(&url, Some(&Credential { token: created.token, id: created.info.id }))?;
    print(json, json!({ "login": user.login, "url": url }), || format!("signed in to {url} as {}", user.login))
}

pub fn logout(dir: &Option<PathBuf>, server: Server) -> Result<()> {
    let url = url(dir, server)?;
    let credential = credentials::load(&url)?.with_context(|| format!("not signed in to {url}"))?;
    match Client::new(&url, credential.token).revoke_token(&credential.id) {
        // The server no longer knows the token, so there is nothing left to revoke.
        Ok(()) | Err(topo_cloud::Error::Api { status: 401 | 404, .. }) => {}
        Err(e) => return Err(e.into()),
    }
    Ok(credentials::store(&url, None)?)
}

pub fn token(dir: &Option<PathBuf>, server: Server, command: TokenCommand, json: bool) -> Result<()> {
    let client = client(dir, server)?;
    match command {
        TokenCommand::Create { name, workspace, expires } => {
            let request = NewToken { name, workspace_id: workspace, expires_in: expires.map(|d| d.as_secs()) };
            let created = client.create_token(&request)?;
            print(json, serde_json::to_value(&created)?, || created.token.clone())
        }
        TokenCommand::Ls => {
            let tokens = client.tokens()?;
            print(json, serde_json::to_value(&tokens)?, || {
                let line = |t: &topo_core::wire::Token| {
                    let scope = t.workspace_id.as_deref().unwrap_or("all workspaces");
                    let expiry = t.expires_at.map_or("no expiry".to_owned(), |at| format!("expires at {at}"));
                    format!("{}  {}  ({scope}, {expiry})", t.id, t.name)
                };
                tokens.iter().map(line).collect::<Vec<_>>().join("\n")
            })
        }
        TokenCommand::Revoke { id } => Ok(client.revoke_token(&id)?),
    }
}

pub fn cloud(dir: &Option<PathBuf>, server: Server, command: CloudCommand, json: bool) -> Result<()> {
    match command {
        CloudCommand::Push { name } => {
            let topo = topo_dir(dir)?;
            anyhow::ensure!(config::load(&topo)?.is_none(), "this workspace is already linked to a cloud workspace");
            let ws = Workspace::open(topo.clone())?;
            let client = client(dir, server)?;
            let name = name.map_or_else(|| directory_name(&topo), Ok)?;
            let workspace = client.create_workspace(&name)?;
            client.import(&workspace.id, ws.graph.nodes().cloned().map(Into::into).collect())?;
            let link = CloudConfig { url: client.url().to_owned(), workspace: workspace.id };
            config::store(&topo, Some(&link))?;
            print(json, serde_json::to_value(&link)?, || {
                format!(
                    "pushed to {} as workspace {}\nthe files in {} are no longer read; `topo cloud pull` rewrites them",
                    link.url,
                    link.workspace,
                    ws.nodes_dir().display()
                )
            })
        }
        CloudCommand::Pull => {
            let (topo, _, _) = linked(dir)?;
            let cloud = topo_cloud::open(topo.clone())?;
            std::fs::create_dir_all(cloud.nodes_dir()).with_context(|| cloud.nodes_dir().display().to_string())?;
            let mut files = Workspace::open(topo.clone())?;
            files.graph = cloud.graph;
            files.save()?;
            config::store(&topo, None)?;
            let count = files.graph.nodes().count();
            print(json, json!({ "nodes": count }), || format!("wrote {count} nodes to {}", files.nodes_dir().display()))
        }
        CloudCommand::Link { workspace } => {
            let topo = match topo_dir(dir) {
                Ok(topo) => topo,
                Err(_) => std::env::current_dir()?.join(topo_core::store::DIR_NAME),
            };
            anyhow::ensure!(config::load(&topo)?.is_none(), "this workspace is already linked to a cloud workspace");
            let client = client(dir, server)?;
            // Fails unless the workspace exists and the token reaches it.
            client.graph(&workspace, None)?;
            std::fs::create_dir_all(&topo).with_context(|| topo.display().to_string())?;
            let link = CloudConfig { url: client.url().to_owned(), workspace };
            config::store(&topo, Some(&link))?;
            print(json, serde_json::to_value(&link)?, || {
                format!("linked {} to workspace {}", topo.display(), link.workspace)
            })
        }
        CloudCommand::Ls => {
            let workspaces = client(dir, server)?.workspaces()?;
            print(json, serde_json::to_value(&workspaces)?, || {
                workspaces
                    .iter()
                    .map(|w| format!("{}  {}  ({:?})", w.id, w.name, w.role))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        }
        CloudCommand::Members => {
            let (_, client, workspace) = linked(dir)?;
            let members = client.members(&workspace)?;
            print(json, serde_json::to_value(&members)?, || {
                members.iter().map(|m| format!("{}  ({:?})", m.login, m.role)).collect::<Vec<_>>().join("\n")
            })
        }
        CloudCommand::Invite { login, role } => {
            let (_, client, workspace) = linked(dir)?;
            Ok(client.put_member(&workspace, &login, role)?)
        }
        CloudCommand::Remove { login, user_id } => {
            let (_, client, workspace) = linked(dir)?;
            match user_id {
                Some(id) => Ok(client.remove_member_by_id(&workspace, id)?),
                None => Ok(client.remove_member(&workspace, &login.context("pass a login or --user-id")?)?),
            }
        }
        CloudCommand::Log { after } => {
            let (_, client, workspace) = linked(dir)?;
            let changes = client.changes(&workspace, after)?;
            print(json, serde_json::to_value(&changes)?, || {
                let line = |c: &topo_core::wire::Change| {
                    let by = c.agent.as_deref().map_or(c.token.clone(), |agent| format!("{} / {agent}", c.token));
                    let ops = serde_json::to_string(&c.ops).expect("operations serialize to JSON");
                    format!("{}  {} ({by})  {ops}", c.version, c.user)
                };
                changes.iter().map(line).collect::<Vec<_>>().join("\n")
            })
        }
    }
}

/// The name of the directory that contains `.topo`.
fn directory_name(topo: &Path) -> Result<String> {
    let parent = topo.canonicalize()?.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
    parent.context("pass --name: the workspace directory has no name")
}
