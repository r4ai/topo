mod batch;
mod render;
mod tui;

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use jiff::civil::Date;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use topo_core::{Edit, Graph, Kind, Node, NodeId, Status, Workspace};
use topo_jev::{Client, Config, organize};

/// topo: every task and milestone is a node in one dependency graph.
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
    /// Make FROM depend on TO (FROM can be `-` to read IDs from stdin).
    Link { from: String, to: String },
    /// Remove the dependency of FROM on TO (FROM can be `-` for stdin).
    Unlink { from: String, to: String },
    /// Add TASK to the set of MILESTONE (TASK can be `-` for stdin).
    Join { task: String, milestone: String },
    /// Remove TASK from the set of MILESTONE (TASK can be `-` for stdin).
    Leave { task: String, milestone: String },
    /// Set status (ID can be `-` to read IDs from stdin).
    Status {
        id: String,
        #[arg(value_parser = parse_enum::<Status>)]
        status: Status,
    },
    /// Change node fields (ID can be `-` to read IDs from stdin).
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
    /// Delete a node and every edge to it (ID can be `-` for stdin).
    Rm { id: String },
    /// List nodes (open ones unless --all).
    Ls {
        /// Restrict the list to IDs read from stdin.
        #[arg(value_parser = ["-"])]
        input: Option<String>,
        #[command(flatten)]
        filters: ListFilters,
        #[arg(long, value_enum, conflicts_with = "json")]
        format: Option<render::ListFormat>,
    },
    /// Show one node with its neighbors.
    Show { id: String },
    /// Tasks that can be started now.
    Ready {
        #[arg(long, value_name = "ID")]
        under: Option<String>,
        #[arg(long, value_enum, conflicts_with = "json")]
        format: Option<render::ListFormat>,
    },
    /// Milestones with progress and critical path.
    Milestones {
        #[arg(long, value_enum, conflicts_with = "json")]
        format: Option<render::ListFormat>,
    },
    /// Requirements of ID, including members of a milestone (default: IDs).
    Deps {
        id: String,
        /// Include all transitive requirements.
        #[arg(long)]
        transitive: bool,
        #[arg(long, value_enum, conflicts_with = "json")]
        format: Option<render::ListFormat>,
    },
    /// Nodes depending on ID (default: IDs).
    Dependents {
        id: String,
        /// Include all transitive dependents and containing milestones.
        #[arg(long)]
        transitive: bool,
        #[arg(long, value_enum, conflicts_with = "json")]
        format: Option<render::ListFormat>,
    },
    /// Tasks belonging to a milestone (default: IDs).
    Members {
        id: String,
        #[arg(long, value_enum, conflicts_with = "json")]
        format: Option<render::ListFormat>,
    },
    /// Longest remaining task chain in execution order (default: IDs).
    CriticalPath {
        id: String,
        #[arg(long, value_enum, conflicts_with = "json")]
        format: Option<render::ListFormat>,
    },
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

#[derive(Args)]
struct ListFilters {
    #[arg(long, value_parser = parse_enum::<Kind>)]
    kind: Option<Kind>,
    #[arg(long, value_parser = parse_enum::<Status>)]
    status: Option<Status>,
    /// Only nodes that ID transitively requires (milestone members and their prerequisites).
    #[arg(long, value_name = "ID")]
    under: Option<String>,
    #[arg(long)]
    all: bool,
    /// Require every supplied tag (repeatable).
    #[arg(long = "tag")]
    tags: Vec<String>,
    /// Due on or before DATE.
    #[arg(long, value_name = "DATE", conflicts_with = "no_due")]
    due_before: Option<Date>,
    /// Due on or after DATE.
    #[arg(long, value_name = "DATE", conflicts_with = "no_due")]
    due_after: Option<Date>,
    /// Only nodes without a due date.
    #[arg(long)]
    no_due: bool,
    /// Require membership in every supplied milestone (repeatable).
    #[arg(long = "in", value_name = "MILESTONE")]
    milestones: Vec<String>,
    /// Only open nodes whose requirements are closed.
    #[arg(long, conflicts_with = "blocked")]
    ready: bool,
    /// Only open nodes with unfinished requirements.
    #[arg(long)]
    blocked: bool,
    /// Case-insensitive title substring.
    #[arg(long)]
    title: Option<String>,
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

fn print(json: bool, value: Value, text: impl FnOnce() -> String) -> Result<()> {
    let output = if json { format!("{value:#}") } else { text() };
    if !output.is_empty() {
        let mut stdout = io::stdout().lock();
        match writeln!(stdout, "{output}").and_then(|_| stdout.flush()) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

fn print_list(graph: &Graph, nodes: &[&Node], json: bool, format: render::ListFormat, milestones: bool) -> Result<()> {
    print(json, if json { render::list_json(graph, nodes, milestones) } else { Value::Null }, || {
        render::list_text(graph, nodes, format, milestones)
    })
}

/// `-` is the only implicit source of IDs; ordinary arguments never read stdin.
fn input_ids(graph: &Graph, input: &str, stdin: &str) -> Result<Vec<NodeId>> {
    if input != "-" {
        return Ok(vec![graph.resolve(input)?]);
    }
    let mut seen = BTreeSet::new();
    let mut ids = Vec::new();
    for prefix in stdin.split_whitespace() {
        let id = graph.resolve(prefix)?;
        if seen.insert(id.clone()) {
            ids.push(id);
        }
    }
    Ok(ids)
}

/// Like `apply`, validate in memory and save only when the whole batch succeeds.
fn mutate_ids(
    ws: &mut Workspace,
    input: &str,
    stdin: &str,
    mut update: impl FnMut(&mut Graph, &NodeId) -> Result<(), topo_core::Error>,
) -> Result<Vec<NodeId>> {
    let ids = input_ids(&ws.graph, input, stdin)?;
    for id in &ids {
        update(&mut ws.graph, id)?;
    }
    if !ids.is_empty() {
        ws.save()?;
    }
    Ok(ids)
}

fn print_mutation(json: bool, input: &str, values: impl Iterator<Item = Value>) -> Result<()> {
    if !json {
        return Ok(());
    }
    let mut values: Vec<Value> = values.collect();
    let value = if input == "-" { Value::Array(values) } else { values.pop().expect("one explicit ID") };
    print(json, value, String::new)
}

fn list_nodes<'a>(graph: &'a Graph, input: Option<&str>, stdin: &str, filters: ListFilters) -> Result<Vec<&'a Node>> {
    let input =
        input.map(|i| input_ids(graph, i, stdin).map(|ids| ids.into_iter().collect::<BTreeSet<_>>())).transpose()?;
    let scope = filters.under.map(|u| graph.resolve(&u)).transpose()?.map(|u| graph.descendants(&u));
    let milestones = resolve_all(graph, &filters.milestones)?;
    for id in &milestones {
        anyhow::ensure!(graph.get(id).unwrap().kind == Kind::Milestone, "{id} is not a milestone");
    }
    let title = filters.title.map(|s| s.to_lowercase());
    Ok(graph
        .nodes()
        .filter(|n| input.as_ref().is_none_or(|ids| ids.contains(&n.id)))
        .filter(|n| filters.kind.is_none_or(|k| n.kind == k))
        .filter(|n| match filters.status {
            Some(s) => n.status == s,
            None => filters.all || !n.status.is_closed(),
        })
        .filter(|n| scope.as_ref().is_none_or(|s| s.contains(&n.id)))
        .filter(|n| filters.tags.iter().all(|tag| n.tags.contains(tag)))
        .filter(|n| filters.due_before.is_none_or(|date| n.due.is_some_and(|due| due <= date)))
        .filter(|n| filters.due_after.is_none_or(|date| n.due.is_some_and(|due| due >= date)))
        .filter(|n| !filters.no_due || n.due.is_none())
        .filter(|n| milestones.iter().all(|id| n.milestones.contains(id)))
        .filter(|n| !filters.ready || graph.is_ready(n))
        .filter(|n| !filters.blocked || (!n.status.is_closed() && !graph.is_ready(n)))
        .filter(|n| title.as_ref().is_none_or(|title| n.title.to_lowercase().contains(title)))
        .collect())
}

fn run(cli: Cli) -> Result<()> {
    // clap checks subcommand conflicts before propagating preceding global flags.
    let list_format = match &cli.command {
        Command::Ls { format, .. }
        | Command::Ready { format, .. }
        | Command::Milestones { format }
        | Command::Deps { format, .. }
        | Command::Dependents { format, .. }
        | Command::Members { format, .. }
        | Command::CriticalPath { format, .. } => *format,
        _ => None,
    };
    anyhow::ensure!(!cli.json || list_format.is_none(), "--json cannot be used with --format");
    if let Command::Init = cli.command {
        let ws = Workspace::init(&std::env::current_dir()?)?;
        let dir = ws.dir().display().to_string();
        print(cli.json, json!({ "dir": dir }), || format!("initialized {dir}"))?;
        return Ok(());
    }
    let reads_stdin = match &cli.command {
        Command::Ls { input, .. } => input.as_deref() == Some("-"),
        Command::Status { id, .. } | Command::Edit { id, .. } | Command::Rm { id } => id == "-",
        Command::Link { from, .. } | Command::Unlink { from, .. } => from == "-",
        Command::Join { task, .. } | Command::Leave { task, .. } => task == "-",
        Command::Apply { file: None } => true,
        _ => false,
    };
    // The producer may create or change nodes, so load the graph after its output is complete.
    let stdin = if reads_stdin { io::read_to_string(io::stdin()).context("reading stdin")? } else { String::new() };
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
            print(json, render::node_json(&ws.graph, ws.graph.get(&id).unwrap()), || id.to_string())?;
        }
        Command::Link { from, to } => {
            let to = ws.graph.resolve(&to)?;
            let ids = mutate_ids(&mut ws, &from, &stdin, |g, id| g.link(id, &to))?;
            print_mutation(json, &from, ids.iter().map(|from| json!({ "from": from, "to": to })))?;
        }
        Command::Unlink { from, to } => {
            let to = ws.graph.resolve(&to)?;
            let ids = mutate_ids(&mut ws, &from, &stdin, |g, id| g.unlink(id, &to))?;
            print_mutation(json, &from, ids.iter().map(|from| json!({ "from": from, "to": to })))?;
        }
        Command::Join { task, milestone } => {
            let milestone = ws.graph.resolve(&milestone)?;
            let ids = mutate_ids(&mut ws, &task, &stdin, |g, id| g.join(id, &milestone))?;
            print_mutation(json, &task, ids.iter().map(|task| json!({ "task": task, "milestone": milestone })))?;
        }
        Command::Leave { task, milestone } => {
            let milestone = ws.graph.resolve(&milestone)?;
            let ids = mutate_ids(&mut ws, &task, &stdin, |g, id| g.leave(id, &milestone))?;
            print_mutation(json, &task, ids.iter().map(|task| json!({ "task": task, "milestone": milestone })))?;
        }
        Command::Status { id, status } => {
            let ids = mutate_ids(&mut ws, &id, &stdin, |g, id| g.set_status(id, status))?;
            print_mutation(json, &id, ids.iter().map(|id| render::node_json(&ws.graph, ws.graph.get(id).unwrap())))?;
        }
        Command::Edit { id, title, kind, due, no_due, tags, note } => {
            let due = if no_due { Some(None) } else { due.map(Some) };
            let edit = Edit { title, kind, due, tags, body: note };
            let ids = mutate_ids(&mut ws, &id, &stdin, |g, id| g.edit(id, edit.clone()))?;
            print_mutation(json, &id, ids.iter().map(|id| render::node_json(&ws.graph, ws.graph.get(id).unwrap())))?;
        }
        Command::Rm { id } => {
            let ids = mutate_ids(&mut ws, &id, &stdin, |g, id| g.remove(id).map(|_| ()))?;
            print_mutation(json, &id, ids.iter().map(|id| json!({ "removed": id })))?;
        }
        Command::Ls { input, filters, format } => {
            let nodes = list_nodes(&ws.graph, input.as_deref(), &stdin, filters)?;
            print_list(&ws.graph, &nodes, json, format.unwrap_or(render::ListFormat::Text), false)?;
        }
        Command::Show { id } => {
            let node = ws.graph.get(&ws.graph.resolve(&id)?).unwrap();
            print(json, render::detail_json(&ws.graph, node), || render::detail_text(&ws.graph, node))?;
        }
        Command::Ready { under, format } => {
            let scope = under.map(|u| ws.graph.resolve(&u)).transpose()?;
            let nodes = ws.graph.ready_tasks(scope.as_ref());
            print_list(&ws.graph, &nodes, json, format.unwrap_or(render::ListFormat::Text), false)?;
        }
        Command::Milestones { format } => {
            let milestones: Vec<&Node> = ws.graph.nodes().filter(|n| n.kind == Kind::Milestone).collect();
            print_list(&ws.graph, &milestones, json, format.unwrap_or(render::ListFormat::Text), true)?;
        }
        Command::Deps { id, transitive, format } => {
            let id = ws.graph.resolve(&id)?;
            let ids = if transitive {
                ws.graph.descendants(&id)
            } else {
                ws.graph.requirements(ws.graph.get(&id).unwrap()).into_iter().cloned().collect()
            };
            let nodes: Vec<&Node> = ids.iter().map(|id| ws.graph.get(id).unwrap()).collect();
            print_list(&ws.graph, &nodes, json, format.unwrap_or(render::ListFormat::Ids), false)?;
        }
        Command::Dependents { id, transitive, format } => {
            let id = ws.graph.resolve(&id)?;
            let nodes: Vec<&Node> = if transitive {
                ws.graph.ancestors(&id).iter().map(|id| ws.graph.get(id).unwrap()).collect()
            } else {
                ws.graph.dependents(&id).collect()
            };
            print_list(&ws.graph, &nodes, json, format.unwrap_or(render::ListFormat::Ids), false)?;
        }
        Command::Members { id, format } => {
            let id = ws.graph.resolve(&id)?;
            anyhow::ensure!(ws.graph.get(&id).unwrap().kind == Kind::Milestone, "{id} is not a milestone");
            let nodes: Vec<&Node> = ws.graph.members(&id).collect();
            print_list(&ws.graph, &nodes, json, format.unwrap_or(render::ListFormat::Ids), false)?;
        }
        Command::CriticalPath { id, format } => {
            let id = ws.graph.resolve(&id)?;
            let nodes: Vec<&Node> = ws.graph.critical_path(&id).iter().map(|id| ws.graph.get(id).unwrap()).collect();
            print_list(&ws.graph, &nodes, json, format.unwrap_or(render::ListFormat::Ids), false)?;
        }
        Command::Graph { format, under } => {
            let under = under.map(|u| ws.graph.resolve(&u)).transpose()?;
            let text = render::graph(&ws.graph, format, under.as_ref());
            print(json, json!({ "format": format, "text": text }), || text.trim_end().to_owned())?;
        }
        Command::Apply { file } => {
            let text = match file {
                Some(path) => std::fs::read_to_string(&path).with_context(|| path.display().to_string())?,
                None => stdin,
            };
            let refs = batch::apply(&mut ws.graph, &text)?;
            ws.save()?;
            print(json, json!({ "refs": refs }), || {
                refs.iter().map(|(name, id)| format!("${name} = {id}")).collect::<Vec<_>>().join("\n")
            })?;
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
                    })?;
                    return Ok(());
                }
            };
            if apply {
                let applied = organize::apply(&mut ws.graph, proposals);
                ws.save()?;
                print(json, serde_json::to_value(&applied)?, || render::applied_lines(&ws.graph, &applied))?;
            } else {
                print(json, serde_json::to_value(&proposals)?, || render::proposal_lines(&ws.graph, &proposals))?;
            }
        }
        Command::Tui => tui::run(ws)?,
    }
    Ok(())
}
