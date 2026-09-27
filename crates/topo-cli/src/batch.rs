//! `topo apply`: a JSON array of operations applied all-or-nothing.
//!
//! Nodes created earlier in the batch can be referenced as `$<ref>`; other ids
//! may be exact ids or unique prefixes.

use std::collections::BTreeMap;

use anyhow::{Context, Result, anyhow};
use jiff::civil::Date;
use serde::Deserialize;
use topo_core::{Edit, Graph, Kind, Node, NodeId, Status};

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Op {
    Add {
        #[serde(rename = "ref")]
        name: Option<String>,
        title: String,
        #[serde(default)]
        kind: Kind,
        due: Option<Date>,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        notes: String,
        /// Nodes the new node depends on.
        #[serde(default)]
        depends_on: Vec<String>,
        /// Nodes that depend on the new node (e.g. its milestone).
        #[serde(default, rename = "for")]
        needed_by: Vec<String>,
    },
    Link {
        from: String,
        to: String,
    },
    Unlink {
        from: String,
        to: String,
    },
    Status {
        id: String,
        status: Status,
    },
    Edit {
        id: String,
        title: Option<String>,
        kind: Option<Kind>,
        due: Option<Date>,
        tags: Option<Vec<String>>,
        notes: Option<String>,
    },
    Remove {
        id: String,
    },
}

/// Applies every operation to `graph` and returns the ids created for each `ref`.
/// On error `graph` may be partially modified; the caller must not save it.
pub fn apply(graph: &mut Graph, json: &str) -> Result<BTreeMap<String, NodeId>> {
    let ops: Vec<Op> = serde_json::from_str(json).context("invalid operations JSON")?;
    let mut refs = BTreeMap::new();
    for (index, op) in ops.into_iter().enumerate() {
        apply_one(graph, &mut refs, op).with_context(|| format!("operation #{index}"))?;
    }
    Ok(refs)
}

fn apply_one(graph: &mut Graph, refs: &mut BTreeMap<String, NodeId>, op: Op) -> Result<()> {
    let id = |graph: &Graph, refs: &BTreeMap<String, NodeId>, s: &str| -> Result<NodeId> {
        match s.strip_prefix('$') {
            Some(name) => refs.get(name).cloned().ok_or_else(|| anyhow!("unknown ref `${name}`")),
            None => Ok(graph.resolve(s)?),
        }
    };
    match op {
        Op::Add { name, title, kind, due, tags, notes, depends_on, needed_by } => {
            let mut node = Node::new(graph.fresh_id(), kind, title);
            node.depends_on = depends_on.iter().map(|d| id(graph, refs, d)).collect::<Result<_>>()?;
            node.due = due;
            node.tags = tags;
            node.body = notes;
            let new_id = node.id.clone();
            graph.insert(node)?;
            for parent in &needed_by {
                graph.link(&id(graph, refs, parent)?, &new_id)?;
            }
            if let Some(name) = name
                && refs.insert(name.clone(), new_id).is_some()
            {
                return Err(anyhow!("ref `${name}` is defined twice"));
            }
        }
        Op::Link { from, to } => graph.link(&id(graph, refs, &from)?, &id(graph, refs, &to)?)?,
        Op::Unlink { from, to } => graph.unlink(&id(graph, refs, &from)?, &id(graph, refs, &to)?)?,
        Op::Status { id: target, status } => graph.set_status(&id(graph, refs, &target)?, status)?,
        Op::Edit { id: target, title, kind, due, tags, notes } => {
            let edit = Edit { title, kind, due: due.map(Some), tags, body: notes };
            graph.edit(&id(graph, refs, &target)?, edit)?;
        }
        Op::Remove { id: target } => {
            graph.remove(&id(graph, refs, &target)?)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_milestone_with_refs() {
        let mut graph = Graph::default();
        let refs = apply(
            &mut graph,
            r#"[
                {"op": "add", "ref": "m", "title": "v1", "kind": "milestone", "due": "2026-10-31"},
                {"op": "add", "ref": "a", "title": "design", "for": ["$m"]},
                {"op": "add", "ref": "b", "title": "build", "depends_on": ["$a"], "for": ["$m"]},
                {"op": "status", "id": "$a", "status": "done"}
            ]"#,
        )
        .unwrap();
        assert_eq!(graph.progress(&refs["m"]), (1, 2));
        assert_eq!(graph.ready_tasks(None)[0].id, refs["b"]);
    }

    #[test]
    fn rejects_bad_operations() {
        let mut graph = Graph::default();
        let err = |json: &str| apply(&mut Graph::default(), json).unwrap_err().to_string();
        assert!(err(r#"[{"op": "link", "from": "$x", "to": "$y"}]"#).contains("operation #0"));
        assert!(err(r#"[{"op": "add", "title": "t", "bogus": 1}]"#).contains("invalid"));
        let cycle = r#"[
            {"op": "add", "ref": "a", "title": "a"},
            {"op": "add", "ref": "b", "title": "b", "depends_on": ["$a"]},
            {"op": "link", "from": "$a", "to": "$b"}
        ]"#;
        let e = apply(&mut graph, cycle).unwrap_err();
        assert!(format!("{e:#}").contains("cycle"));
    }
}
