mod batch;
mod render;
mod tui;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use jiff::civil::Date;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use topo_core::{Edit, Graph, Kind, Node, NodeId, Status, Workspace};
use topo_jev::{Client, Config, organize};

/// Topological todo: every task and milestone is a node in one dependency graph.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Print machine-readable JSON instead of text.
    #[arg(long, global = true)]
    json: bool,
    /// The `.topo` directory to use instead of searching from the current directory.
    #[arg(long, global = true, env = "TOPO_DIR")]
    topo_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a `.topo` workspace in the current directory.
    Init,
    /// Add a task (or a milestone with --milestone).
    Add {
        title: String,
        #[arg(long)]
        milestone: bool,
        /// The new node depends on this node (repeatable).
        #[arg(long = "dep", value_name = "ID")]
        deps: Vec<String>,
        /// Milestone whose set the new task belongs to (repeatable).
        #[arg(long = "in", value_name = "MILESTONE")]
        milestones: Vec<String>,
        #[arg(long)]
        due: Option<Date>,
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// Markdown notes stored as the file body.
        #[arg(long)]
        note: Option<String>,
    },
    /// Make FROM depend on TO (TO must be finished before FROM).
    Link { from: String, to: String },
    /// Remove the dependency of FROM on TO.
    Unlink { from: String, to: String },
    /// Add TASK to the set of MILESTONE.
    Join { task: String, milestone: String },
    /// Remove TASK from the set of MILESTONE.
    Leave { task: String, milestone: String },
    /// Set the status of a node.
    Status {
        id: String,
        #[arg(value_parser = parse_enum::<Status>)]
        status: Status,
    },
    /// Change fields of a node.
    Edit {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, value_parser = parse_enum::<Kind>)]
        kind: Option<Kind>,
        #[arg(long, conflicts_with = "no_due")]
        due: Option<Date>,
        #[arg(long)]
        no_due: bool,
        /// Replace all tags (repeatable).
        #[arg(long = "tag")]
        tags: Option<Vec<String>>,
        #[arg(long)]
        note: Option<String>,
    },
    /// Delete a node and every edge to it.
    Rm { id: String },
    /// List nodes (open ones unless --all).
    Ls {
        #[arg(long, value_parser = parse_enum::<Kind>)]
        kind: Option<Kind>,
        #[arg(long, value_parser = parse_enum::<Status>)]
        status: Option<Status>,
        /// Only nodes that ID transitively requires (for a milestone: its members and their prerequisites).
        #[arg(long, value_name = "ID")]
        under: Option<String>,
        #[arg(long)]
        all: bool,
    },
    /// Show one node with its neighbors.
    Show { id: String },
    /// Tasks that can be started now.
    Ready {
        #[arg(long, value_name = "ID")]
        under: Option<String>,
    },
    /// Milestones with progress and critical path.
    Milestones,
    /// Print the graph.
    Graph {
        #[arg(long, value_enum, default_value_t = render::Format::Tree)]
        format: render::Format,
        #[arg(long, value_name = "ID")]
        under: Option<String>,
    },
    /// Apply a JSON array of operations atomically (from FILE or stdin).
    Apply { file: Option<PathBuf> },
    /// Organize the graph with a Jev-compatible decision model.
    Organize {
        #[arg(value_enum)]
        what: Organize,
        /// Nodes to place (for `place`; default: tasks under no milestone).
        ids: Vec<String>,
        /// Scope for `prioritize`.
        #[arg(long, value_name = "ID")]
        under: Option<String>,
        /// Apply the proposals instead of only printing them.
        #[arg(long)]
        apply: bool,
    },
    /// Interactive terminal UI.
    Tui,
}

#[derive(Clone, Copy, ValueEnum)]
enum Organize {
    /// Propose missing dependencies between tasks.
    Deps,
    /// Propose which milestone each task belongs to.
    Place,
    /// Find nodes that describe the same work.
    Dupes,
    /// Propose task/milestone kind corrections.
    Kinds,
    /// Rank ready tasks by priority.
    Prioritize,
}

fn parse_enum<T: DeserializeOwned>(s: &str) -> Result<T, String> {
    serde_json::from_value(Value::String(s.to_owned())).map_err(|e| e.to_string())
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn open(cli: &Cli) -> Result<Workspace> {
    Ok(match &cli.topo_dir {
        Some(dir) => Workspace::open(dir.clone())?,
        None => Workspace::discover(&std::env::current_dir()?)?,
    })
}

fn resolve_all(graph: &Graph, ids: &[String]) -> Result<Vec<NodeId>> {
    ids.iter().map(|id| Ok(graph.resolve(id)?)).collect()
}

fn print(json: bool, value: Value, text: impl FnOnce() -> String) {
    if json {
        println!("{value:#}");
    } else {
        let text = text();
        if !text.is_empty() {
            println!("{text}");
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    if let Command::Init = cli.command {
        let ws = Workspace::init(&std::env::current_dir()?)?;
        let dir = ws.dir().display().to_string();
        print(cli.json, json!({ "dir": dir }), || format!("initialized {dir}"));
        return Ok(());
    }
    let mut ws = open(&cli)?;
    let json = cli.json;
    match cli.command {
        Command::Init => unreachable!("handled above"),
        Command::Add { title, milestone, deps, milestones, due, tags, note } => {
            let kind = if milestone { Kind::Milestone } else { Kind::Task };
            let mut node = Node::new(ws.graph.fresh_id(), kind, title);
            node.depends_on = resolve_all(&ws.graph, &deps)?;
            node.milestones = resolve_all(&ws.graph, &milestones)?;
            node.due = due;
            node.tags = tags;
            node.body = note.unwrap_or_default();
            let id = node.id.clone();
            ws.graph.insert(node)?;
            ws.save()?;
            print(json, render::node_json(&ws.graph, &ws.graph.get(&id).unwrap().clone()), || id.to_string());
        }
        Command::Link { from, to } => {
            let (from, to) = (ws.graph.resolve(&from)?, ws.graph.resolve(&to)?);
            ws.graph.link(&from, &to)?;
            ws.save()?;
            print(json, json!({ "from": from, "to": to }), String::new);
        }
        Command::Unlink { from, to } => {
            let (from, to) = (ws.graph.resolve(&from)?, ws.graph.resolve(&to)?);
            ws.graph.unlink(&from, &to)?;
            ws.save()?;
            print(json, json!({ "from": from, "to": to }), String::new);
        }
        Command::Join { task, milestone } => {
            let (task, milestone) = (ws.graph.resolve(&task)?, ws.graph.resolve(&milestone)?);
            ws.graph.join(&task, &milestone)?;
            ws.save()?;
            print(json, json!({ "task": task, "milestone": milestone }), String::new);
        }
        Command::Leave { task, milestone } => {
            let (task, milestone) = (ws.graph.resolve(&task)?, ws.graph.resolve(&milestone)?);
            ws.graph.leave(&task, &milestone)?;
            ws.save()?;
            print(json, json!({ "task": task, "milestone": milestone }), String::new);
        }
        Command::Status { id, status } => {
            let id = ws.graph.resolve(&id)?;
            ws.graph.set_status(&id, status)?;
            ws.save()?;
            print(json, render::node_json(&ws.graph, ws.graph.get(&id).unwrap()), String::new);
        }
        Command::Edit { id, title, kind, due, no_due, tags, note } => {
            let id = ws.graph.resolve(&id)?;
            let due = if no_due { Some(None) } else { due.map(Some) };
            ws.graph.edit(&id, Edit { title, kind, due, tags, body: note })?;
            ws.save()?;
            print(json, render::node_json(&ws.graph, ws.graph.get(&id).unwrap()), String::new);
        }
        Command::Rm { id } => {
            let id = ws.graph.resolve(&id)?;
            ws.graph.remove(&id)?;
            ws.save()?;
            print(json, json!({ "removed": id }), String::new);
        }
        Command::Ls { kind, status, under, all } => {
            let scope = under.map(|u| ws.graph.resolve(&u)).transpose()?.map(|u| ws.graph.descendants(&u));
            let nodes: Vec<&Node> = ws
                .graph
                .nodes()
                .filter(|n| kind.is_none_or(|k| n.kind == k))
                .filter(|n| match status {
                    Some(s) => n.status == s,
                    None => all || !n.status.is_closed(),
                })
                .filter(|n| scope.as_ref().is_none_or(|s| s.contains(&n.id)))
                .collect();
            print(json, render::nodes_json(&ws.graph, &nodes), || render::node_lines(&nodes));
        }
        Command::Show { id } => {
            let node = ws.graph.get(&ws.graph.resolve(&id)?).unwrap();
            print(json, render::detail_json(&ws.graph, node), || render::detail_text(&ws.graph, node));
        }
        Command::Ready { under } => {
            let scope = under.map(|u| ws.graph.resolve(&u)).transpose()?;
            let nodes = ws.graph.ready_tasks(scope.as_ref());
            print(json, render::nodes_json(&ws.graph, &nodes), || render::node_lines(&nodes));
        }
        Command::Milestones => {
            let milestones: Vec<&Node> = ws.graph.nodes().filter(|n| n.kind == Kind::Milestone).collect();
            print(
                json,
                Value::Array(milestones.iter().map(|m| render::milestone_json(&ws.graph, m)).collect()),
                || milestones.iter().map(|m| render::milestone_line(&ws.graph, m)).collect::<Vec<_>>().join("\n"),
            );
        }
        Command::Graph { format, under } => {
            let under = under.map(|u| ws.graph.resolve(&u)).transpose()?;
            let text = render::graph(&ws.graph, format, under.as_ref());
            print(json, json!({ "format": format, "text": text }), || text.trim_end().to_owned());
        }
        Command::Apply { file } => {
            let text = match file {
                Some(path) => std::fs::read_to_string(&path).with_context(|| path.display().to_string())?,
                None => std::io::read_to_string(std::io::stdin())?,
            };
            let refs = batch::apply(&mut ws.graph, &text)?;
            ws.save()?;
            print(json, json!({ "refs": refs }), || {
                refs.iter().map(|(name, id)| format!("${name} = {id}")).collect::<Vec<_>>().join("\n")
            });
        }
        Command::Organize { what, ids, under, apply } => {
            let client = Client::new(Config::load(ws.dir())?);
            let proposals = match what {
                Organize::Deps => organize::dependencies(&ws.graph, &client)?,
                Organize::Place => {
                    let tasks = match ids.is_empty() {
                        true => organize::unplaced_tasks(&ws.graph),
                        false => resolve_all(&ws.graph, &ids)?,
                    };
                    organize::placement(&ws.graph, &client, &tasks)?
                }
                Organize::Dupes => organize::duplicates(&ws.graph, &client)?,
                Organize::Kinds => organize::kinds(&ws.graph, &client)?,
                Organize::Prioritize => {
                    let scope = under.map(|u| ws.graph.resolve(&u)).transpose()?;
                    let ranked = organize::prioritize(&ws.graph, &client, scope.as_ref())?;
                    print(json, serde_json::to_value(&ranked)?, || {
                        ranked
                            .iter()
                            .map(|p| format!("{:.2}  {}  {}", p.score, p.id, p.title))
                            .collect::<Vec<_>>()
                            .join("\n")
                    });
                    return Ok(());
                }
            };
            if apply {
                let applied = organize::apply(&mut ws.graph, proposals);
                ws.save()?;
                print(json, serde_json::to_value(&applied)?, || render::applied_lines(&ws.graph, &applied));
            } else {
                print(json, serde_json::to_value(&proposals)?, || render::proposal_lines(&ws.graph, &proposals));
            }
        }
        Command::Tui => tui::run(ws)?,
    }
    Ok(())
}
