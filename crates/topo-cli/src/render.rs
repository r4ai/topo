//! Text and JSON views of the graph.

use std::collections::BTreeSet;

use clap::ValueEnum;
use serde::Serialize;
use serde_json::{Value, json};
use topo_core::model::pr_label;
use topo_core::{Graph, Kind, Node, NodeId, Status};
use topo_jev::organize::{Applied, Proposal};

pub fn node_json(graph: &Graph, node: &Node) -> Value {
    let mut value = serde_json::to_value(node).expect("nodes serialize");
    value["notes"] = json!(node.body);
    value["ready"] = json!(graph.is_ready(node));
    value
}

pub fn nodes_json(graph: &Graph, nodes: &[&Node]) -> Value {
    Value::Array(nodes.iter().map(|n| node_json(graph, n)).collect())
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ListFormat {
    /// Human-readable node or milestone lines.
    Text,
    /// One node ID per line.
    Ids,
    /// Tab-separated ID, kind, status, due date, and title, without a header.
    Tsv,
    /// One compact JSON object per line.
    Jsonl,
}

pub fn list_json(graph: &Graph, nodes: &[&Node], milestones: bool) -> Value {
    if milestones {
        Value::Array(nodes.iter().map(|n| milestone_json(graph, n)).collect())
    } else {
        nodes_json(graph, nodes)
    }
}

pub fn list_text(graph: &Graph, nodes: &[&Node], format: ListFormat, milestones: bool) -> String {
    nodes
        .iter()
        .map(|node| match format {
            ListFormat::Text if milestones => milestone_line(graph, node),
            ListFormat::Text => node_line(node),
            ListFormat::Ids => node.id.to_string(),
            ListFormat::Tsv => {
                let kind = match node.kind {
                    Kind::Task => "task",
                    Kind::Milestone => "milestone",
                };
                let status = match node.status {
                    Status::Todo => "todo",
                    Status::Doing => "doing",
                    Status::Done => "done",
                    Status::Dropped => "dropped",
                };
                let due = node.due.map(|d| d.to_string()).unwrap_or_default();
                format!("{}\t{kind}\t{status}\t{due}\t{}", tsv_field(node.id.as_str()), tsv_field(&node.title))
            }
            ListFormat::Jsonl => {
                if milestones { milestone_json(graph, node) } else { node_json(graph, node) }.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tsv_field(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\t', "\\t").replace('\n', "\\n").replace('\r', "\\r")
}

pub fn status_mark(status: Status) -> &'static str {
    match status {
        Status::Todo => "[ ]",
        Status::Doing => "[~]",
        Status::Done => "[x]",
        Status::Dropped => "[-]",
    }
}

pub fn node_line(node: &Node) -> String {
    let kind = if node.kind == Kind::Milestone { "◆ " } else { "" };
    let priority = node.priority.map(|p| format!("  !{p}")).unwrap_or_default();
    let assignee = node.assignee.as_ref().map(|a| format!("  @{a}")).unwrap_or_default();
    let due = node.due.map(|d| format!("  due {d}")).unwrap_or_default();
    let tags: String = node.tags.iter().map(|t| format!("  #{t}")).collect();
    let prs: String = node.prs.iter().map(|pr| format!("  PR {}", pr_label(pr))).collect();
    format!("{} {}  {kind}{}{priority}{assignee}{due}{tags}{prs}", status_mark(node.status), node.id, node.title)
}

pub fn detail_json(graph: &Graph, node: &Node) -> Value {
    let mut value = node_json(graph, node);
    value["needed_by"] = json!(graph.dependents(&node.id).map(|n| &n.id).collect::<Vec<_>>());
    if node.kind == Kind::Milestone {
        value["milestone"] = milestone_json(graph, node);
    }
    value
}

pub fn detail_text(graph: &Graph, node: &Node) -> String {
    let neighbors = |label: &str, nodes: Vec<&Node>| match nodes.is_empty() {
        true => String::new(),
        false => format!(
            "\n{label}:\n{}",
            nodes.iter().map(|n| format!("  {}", node_line(n))).collect::<Vec<_>>().join("\n")
        ),
    };
    let lookup = |ids: &[NodeId]| ids.iter().map(|d| graph.get(d).expect("graph invariant")).collect();
    let mut text = node_line(node);
    if node.kind == Kind::Milestone {
        text += &format!("\n{}", milestone_line(graph, node));
    }
    // Times are known only for nodes written since they were recorded.
    for (label, time) in [("created", node.created_at), ("updated", node.updated_at), ("completed", node.completed_at)]
    {
        let missing = if label == "completed" && node.status != Status::Done { "Not completed" } else { "Unknown" };
        text += &format!("\n{label} {}", time.map(|t| t.to_string()).unwrap_or_else(|| missing.to_owned()));
    }
    if !node.prs.is_empty() {
        text += &format!(
            "\npull requests:\n{}",
            node.prs.iter().map(|pr| format!("  {pr}")).collect::<Vec<_>>().join("\n")
        );
    }
    text += &neighbors("members", graph.members(&node.id).collect());
    text += &neighbors("in milestones", lookup(&node.milestones));
    text += &neighbors("depends on", lookup(&node.depends_on));
    text += &neighbors("needed by", graph.dependents(&node.id).collect());
    if !node.body.is_empty() {
        text += &format!("\n\n{}", node.body.trim_end());
    }
    text
}

pub fn milestone_json(graph: &Graph, milestone: &Node) -> Value {
    let (done, total) = graph.progress(&milestone.id);
    json!({
        "id": milestone.id,
        "title": milestone.title,
        "status": milestone.status,
        "due": milestone.due,
        "done": done,
        "total": total,
        "members": graph.members(&milestone.id).map(|n| &n.id).collect::<Vec<_>>(),
        "reached": graph.is_ready(milestone) || milestone.status.is_closed(),
        "critical_path": graph.critical_path(&milestone.id),
    })
}

pub fn milestone_line(graph: &Graph, milestone: &Node) -> String {
    let (done, total) = graph.progress(&milestone.id);
    let percent = (done * 100).checked_div(total).unwrap_or(100);
    let bar: String = (0..10).map(|i| if i * 10 < percent { '█' } else { '░' }).collect();
    let steps = graph.critical_path(&milestone.id).len();
    format!("{}  {bar} {done}/{total}  {steps} step(s) left on critical path", node_line(milestone))
}

pub fn proposal_lines(graph: &Graph, proposals: &[Proposal]) -> String {
    if proposals.is_empty() {
        return "no proposals".into();
    }
    proposals.iter().map(|p| proposal_line(graph, p)).collect::<Vec<_>>().join("\n")
}

pub fn applied_lines(graph: &Graph, applied: &[Applied]) -> String {
    if applied.is_empty() {
        return "no proposals".into();
    }
    applied
        .iter()
        .map(|a| match &a.skipped {
            None => format!("applied  {}", proposal_line(graph, &a.proposal)),
            Some(why) => format!("skipped  {}  ({why})", proposal_line(graph, &a.proposal)),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn proposal_line(graph: &Graph, proposal: &Proposal) -> String {
    let title = |id: &NodeId| graph.get(id).map_or("?", |n| n.title.as_str()).to_owned();
    match proposal {
        Proposal::Link { from, to, probability } => {
            format!("{probability:.2}  link  {from} \"{}\" depends on {to} \"{}\"", title(from), title(to))
        }
        Proposal::Join { task, milestone, probability } => {
            format!("{probability:.2}  join  {task} \"{}\" in {milestone} \"{}\"", title(task), title(milestone))
        }
        Proposal::SetKind { id, kind, probability } => {
            format!("{probability:.2}  kind  {id} \"{}\" -> {kind:?}", title(id))
        }
        Proposal::Duplicate { a, b, probability } => {
            format!("{probability:.2}  dupe  {a} \"{}\" ~ {b} \"{}\"", title(a), title(b))
        }
    }
}

#[derive(Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// Indented requirement tree from the top nodes down.
    Tree,
    /// Mermaid flowchart (prerequisite --> dependent, member -.-> milestone).
    Mermaid,
    /// Graphviz DOT (prerequisite -> dependent, dotted member -> milestone).
    Dot,
}

pub fn graph(graph: &Graph, format: Format, under: Option<&NodeId>) -> String {
    let ids: BTreeSet<NodeId> = match under {
        Some(root) => graph.descendants(root).into_iter().chain([root.clone()]).collect(),
        None => graph.nodes().map(|n| n.id.clone()).collect(),
    };
    let nodes: Vec<&Node> = ids.iter().map(|id| graph.get(id).expect("ids come from the graph")).collect();
    let edges = || nodes.iter().flat_map(|n| n.depends_on.iter().map(move |d| (d, &n.id)));
    let memberships =
        || nodes.iter().flat_map(|n| n.milestones.iter().filter(|m| ids.contains(*m)).map(move |m| (&n.id, m)));
    match format {
        Format::Tree => tree(graph, under),
        Format::Mermaid => {
            let mut out = String::from("flowchart LR\n  classDef closed opacity:0.5\n");
            for n in &nodes {
                let label = format!("{} {}", status_mark(n.status), n.title).replace('"', "#quot;");
                let shape = match n.kind {
                    Kind::Milestone => format!("{{{{\"{label}\"}}}}"),
                    Kind::Task => format!("[\"{label}\"]"),
                };
                let class = if n.status.is_closed() { ":::closed" } else { "" };
                out += &format!("  {}{shape}{class}\n", n.id);
            }
            for (from, to) in edges() {
                out += &format!("  {from} --> {to}\n");
            }
            for (task, milestone) in memberships() {
                out += &format!("  {task} -.-> {milestone}\n");
            }
            out
        }
        Format::Dot => {
            let mut out = String::from("digraph topo {\n  rankdir=LR;\n");
            for n in &nodes {
                let label = format!("{} {}", status_mark(n.status), n.title).replace('\\', "\\\\").replace('"', "\\\"");
                let shape = if n.kind == Kind::Milestone { "hexagon" } else { "box" };
                let style = if n.status.is_closed() { ", style=dashed" } else { "" };
                out += &format!("  \"{}\" [label=\"{label}\", shape={shape}{style}];\n", n.id);
            }
            for (from, to) in edges() {
                out += &format!("  \"{from}\" -> \"{to}\";\n");
            }
            for (task, milestone) in memberships() {
                out += &format!("  \"{task}\" -> \"{milestone}\" [style=dotted];\n");
            }
            out + "}\n"
        }
    }
}

/// Each node followed by its requirements (dependencies, and members of a milestone), indented. Nodes reached a second
/// time are printed once more as a reference only.
fn tree(graph: &Graph, under: Option<&NodeId>) -> String {
    let roots: Vec<NodeId> = match under {
        Some(id) => vec![id.clone()],
        None => graph.roots().iter().map(|n| n.id.clone()).collect(),
    };
    let mut out = String::new();
    let mut seen = BTreeSet::new();
    let mut pending: Vec<_> = roots.iter().rev().map(|id| (id, 0)).collect();
    while let Some((id, depth)) = pending.pop() {
        let node = graph.get(id).expect("ids come from the graph");
        // Keep every node while bounding output size for deeply nested remote graphs.
        let mut indent = "  ".repeat(depth.min(32));
        if depth > 32 {
            indent += &format!("…(depth {depth}) ");
        }
        if !seen.insert(id.clone()) {
            out += &format!("{indent}↑ {} {}\n", node.id, node.title);
            continue;
        }
        out += &format!("{indent}{}\n", node_line(node));
        pending.extend(graph.requirements(node).into_iter().rev().map(|dep| (dep, depth + 1)));
    }
    out
}
