//! Operations: the one way a graph is changed through `topo apply` and the
//! cloud API. A batch is applied all-or-nothing.
//!
//! A node reference is an exact id, or `$<ref>` for a node added earlier in the
//! same batch.

use std::collections::{BTreeMap, BTreeSet};

use jiff::civil::Date;
use serde::{Deserialize, Deserializer, Serialize};

use crate::Error;
use crate::graph::{Edit, Graph};
use crate::model::{self, Kind, Node, NodeId, Priority, Status};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    Add {
        /// Id of the new node. Generated when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<NodeId>,
        /// Name by which later operations of the batch refer to the node, as `$<ref>`.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        title: String,
        #[serde(default)]
        kind: Kind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        due: Option<Date>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tags: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        priority: Option<Priority>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        assignee: Option<String>,
        /// Pull requests: URLs, or `owner/repo#123` on GitHub.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        prs: Vec<String>,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        notes: String,
        /// Nodes the new node depends on.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        depends_on: Vec<String>,
        /// Milestones whose set the new task belongs to.
        #[serde(default, rename = "in", skip_serializing_if = "Vec::is_empty")]
        milestones: Vec<String>,
    },
    Link {
        from: String,
        to: String,
    },
    Unlink {
        from: String,
        to: String,
    },
    Join {
        task: String,
        milestone: String,
    },
    Leave {
        task: String,
        milestone: String,
    },
    Status {
        id: String,
        status: Status,
        /// The operation fails unless the node currently has this status.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        if_status: Option<Status>,
    },
    Edit {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<Kind>,
        /// Absent leaves the due date unchanged; `null` clears it.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present")]
        #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, format = Date))]
        due: Option<Option<Date>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tags: Option<Vec<String>>,
        /// Absent leaves the priority unchanged; `null` clears it.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present")]
        #[cfg_attr(feature = "openapi", schema(value_type = Option<Priority>))]
        priority: Option<Option<Priority>>,
        /// Absent leaves the assignee unchanged; `null` clears it.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present")]
        #[cfg_attr(feature = "openapi", schema(value_type = Option<String>))]
        assignee: Option<Option<String>>,
        /// Replaces the pull requests; an empty list clears them.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prs: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        notes: Option<String>,
    },
    Remove {
        id: String,
    },
}

/// Deserializes a present field, `null` included, as `Some`.
fn present<'de, T: Deserialize<'de>, D: Deserializer<'de>>(deserializer: D) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

impl Op {
    /// The node references of this operation.
    pub fn references_mut(&mut self) -> Vec<&mut String> {
        match self {
            Op::Add { depends_on, milestones, .. } => depends_on.iter_mut().chain(milestones).collect(),
            Op::Link { from, to } | Op::Unlink { from, to } => vec![from, to],
            Op::Join { task, milestone } | Op::Leave { task, milestone } => vec![task, milestone],
            Op::Status { id, .. } | Op::Edit { id, .. } | Op::Remove { id } => vec![id],
        }
    }
}

/// Applies every operation to `graph` and returns the ids created for each `ref`.
///
/// The operations are rewritten in place to the form that was applied: every
/// `$<ref>` becomes an id and every `add` carries its id.
/// On error `graph` may be partially modified; the caller must discard it.
pub fn apply(graph: &mut Graph, ops: &mut [Op]) -> Result<BTreeMap<String, NodeId>, Error> {
    let mut refs = BTreeMap::new();
    for (index, op) in ops.iter_mut().enumerate() {
        apply_one(graph, &mut refs, op).map_err(|source| Error::Op { index, source: Box::new(source) })?;
    }
    Ok(refs)
}

fn apply_one(graph: &mut Graph, refs: &mut BTreeMap<String, NodeId>, op: &mut Op) -> Result<(), Error> {
    for reference in op.references_mut() {
        if let Some(name) = reference.strip_prefix('$') {
            *reference = refs.get(name).ok_or_else(|| Error::UnknownRef(name.to_owned()))?.0.clone();
        }
    }
    let id = |s: &String| NodeId(s.clone());
    match op {
        Op::Add {
            id: new_id,
            name,
            title,
            kind,
            due,
            tags,
            priority,
            assignee,
            prs,
            notes,
            depends_on,
            milestones,
        } => {
            normalize(assignee.as_mut(), prs)?;
            let new_id = new_id.get_or_insert_with(|| graph.fresh_id()).clone();
            new_id.validate()?;
            let mut node = Node::new(new_id.clone(), *kind, title.clone());
            node.depends_on = depends_on.iter().map(id).collect();
            node.milestones = milestones.iter().map(id).collect();
            node.due = *due;
            node.tags = tags.clone();
            node.priority = *priority;
            node.assignee = assignee.clone();
            node.prs = prs.clone();
            node.body = notes.clone();
            graph.insert(node)?;
            if let Some(name) = name
                && refs.insert(name.clone(), new_id).is_some()
            {
                return Err(Error::DuplicateRef(name.clone()));
            }
        }
        Op::Link { from, to } => graph.link(&id(from), &id(to))?,
        Op::Unlink { from, to } => graph.unlink(&id(from), &id(to))?,
        Op::Join { task, milestone } => graph.join(&id(task), &id(milestone))?,
        Op::Leave { task, milestone } => graph.leave(&id(task), &id(milestone))?,
        Op::Status { id: target, status, if_status } => {
            let target = id(target);
            if let Some(expected) = *if_status {
                let actual = graph.get(&target).ok_or_else(|| Error::NotFound(target.0.clone()))?.status;
                if actual != expected {
                    return Err(Error::StatusMismatch { id: target, expected, actual });
                }
            }
            graph.set_status(&target, *status)?;
        }
        Op::Edit { id: target, title, kind, due, tags, priority, assignee, prs, notes } => {
            normalize(assignee.as_mut().and_then(Option::as_mut), prs.as_mut().map_or(&mut [], Vec::as_mut_slice))?;
            let edit = Edit {
                title: title.clone(),
                kind: *kind,
                due: *due,
                tags: tags.clone(),
                body: notes.clone(),
                priority: *priority,
                assignee: assignee.clone(),
                prs: prs.clone(),
            };
            graph.edit(&id(target), edit)?;
        }
        Op::Remove { id: target } => {
            graph.remove(&id(target))?;
        }
    }
    Ok(())
}

/// Rewrites an assignee and pull requests as given by a person to their canonical form.
fn normalize(assignee: Option<&mut String>, prs: &mut [String]) -> Result<(), Error> {
    if let Some(assignee) = assignee {
        *assignee = model::normalize_assignee(assignee)?;
    }
    for pr in prs {
        *pr = model::normalize_pr(pr)?;
    }
    Ok(())
}

/// Operations that turn the nodes `before` into the nodes `after`.
///
/// Both must be valid graphs. Timestamps are not part of the result: whoever
/// applies the operations records them ([`Graph::stamp`]). The order keeps every intermediate graph valid:
/// removals come first because a removed node can be what makes a new edge look
/// like a cycle; edges are dropped before kinds change and added after, so each
/// membership meets a task and a milestone; and every edge added is an edge of
/// `after`, which has no cycle.
pub fn diff(before: &BTreeMap<NodeId, Node>, after: &BTreeMap<NodeId, Node>) -> Vec<Op> {
    let removed: BTreeSet<&NodeId> = before.keys().filter(|id| !after.contains_key(*id)).collect();
    // What each node of `after` looks like once the removals and additions are applied.
    let base: Vec<Node> = after
        .values()
        .map(|node| match before.get(&node.id) {
            Some(old) => Node {
                depends_on: old.depends_on.iter().filter(|d| !removed.contains(d)).cloned().collect(),
                milestones: old.milestones.iter().filter(|m| !removed.contains(m)).cloned().collect(),
                ..old.clone()
            },
            None => Node { status: Status::Todo, depends_on: Vec::new(), milestones: Vec::new(), ..node.clone() },
        })
        .collect();
    let pairs = || base.iter().zip(after.values());
    let s = |id: &NodeId| id.0.clone();

    let removes = removed.iter().map(|id| Op::Remove { id: s(id) });
    let adds = after.values().filter(|n| !before.contains_key(&n.id)).map(|n| Op::Add {
        id: Some(n.id.clone()),
        name: None,
        title: n.title.clone(),
        kind: n.kind,
        due: n.due,
        tags: n.tags.clone(),
        priority: n.priority,
        assignee: n.assignee.clone(),
        prs: n.prs.clone(),
        notes: n.body.clone(),
        depends_on: Vec::new(),
        milestones: Vec::new(),
    });
    let leaves = pairs().flat_map(|(old, new)| {
        let (dropped, _) = edge_changes(&old.milestones, &new.milestones);
        dropped.into_iter().map(|m| Op::Leave { task: s(&new.id), milestone: s(m) })
    });
    let unlinks = pairs().flat_map(|(old, new)| {
        let (dropped, _) = edge_changes(&old.depends_on, &new.depends_on);
        dropped.into_iter().map(|d| Op::Unlink { from: s(&new.id), to: s(d) })
    });
    let edits = pairs().filter_map(|(old, new)| {
        let edit = Op::Edit {
            id: s(&new.id),
            title: (old.title != new.title).then(|| new.title.clone()),
            kind: (old.kind != new.kind).then_some(new.kind),
            due: (old.due != new.due).then_some(new.due),
            tags: (old.tags != new.tags).then(|| new.tags.clone()),
            priority: (old.priority != new.priority).then_some(new.priority),
            assignee: (old.assignee != new.assignee).then(|| new.assignee.clone()),
            prs: (old.prs != new.prs).then(|| new.prs.clone()),
            notes: (old.body != new.body).then(|| new.body.clone()),
        };
        let unchanged = Op::Edit {
            id: s(&new.id),
            title: None,
            kind: None,
            due: None,
            tags: None,
            priority: None,
            assignee: None,
            prs: None,
            notes: None,
        };
        (edit != unchanged).then_some(edit)
    });
    let links = pairs().flat_map(|(old, new)| {
        let (_, added) = edge_changes(&old.depends_on, &new.depends_on);
        added.iter().map(|d| Op::Link { from: s(&new.id), to: s(d) })
    });
    let joins = pairs().flat_map(|(old, new)| {
        let (_, added) = edge_changes(&old.milestones, &new.milestones);
        added.iter().map(|m| Op::Join { task: s(&new.id), milestone: s(m) })
    });
    let statuses = pairs().filter(|(old, new)| old.status != new.status).map(|(_, new)| Op::Status {
        id: s(&new.id),
        status: new.status,
        if_status: None,
    });

    removes.chain(adds).chain(leaves).chain(unlinks).chain(edits).chain(links).chain(joins).chain(statuses).collect()
}

/// The edges to drop from `old` and then append, in order, to arrive at `new`.
///
/// Appending is the only way to add an edge, so the edges that stay must be a
/// prefix of `new` that appears in `old` in the same order.
fn edge_changes<'a>(old: &'a [NodeId], new: &'a [NodeId]) -> (Vec<&'a NodeId>, &'a [NodeId]) {
    let mut rest = old.iter();
    let kept = new.iter().take_while(|id| rest.any(|o| o == *id)).count();
    let dropped = old.iter().filter(|id| !new[..kept].contains(id)).collect();
    (dropped, &new[kept..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Vec<Op> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn builds_a_milestone_with_refs() {
        let mut graph = Graph::default();
        let mut ops = parse(
            r#"[
                {"op": "add", "ref": "m", "title": "v1", "kind": "milestone", "due": "2026-10-31"},
                {"op": "add", "ref": "a", "title": "design", "in": ["$m"]},
                {"op": "add", "ref": "b", "title": "build", "depends_on": ["$a"], "in": ["$m"]},
                {"op": "status", "id": "$a", "status": "done"}
            ]"#,
        );
        let refs = apply(&mut graph, &mut ops).unwrap();
        assert_eq!(graph.progress(&refs["m"]), (1, 2));
        assert_eq!(graph.ready_tasks(None)[0].id, refs["b"]);
        // The batch is rewritten to what was applied, so it can be replayed on a copy.
        let mut replay = Graph::default();
        apply(&mut replay, &mut ops.clone()).unwrap();
        assert_eq!(replay, graph);
        assert_eq!(ops[3], Op::Status { id: refs["a"].0.clone(), status: Status::Done, if_status: None });
    }

    #[test]
    fn rejects_bad_operations() {
        let err = |json: &str| apply(&mut Graph::default(), &mut parse(json)).unwrap_err().to_string();
        assert!(err(r#"[{"op": "link", "from": "$x", "to": "$y"}]"#).contains("operation #0: unknown ref `$x`"));
        assert!(err(r#"[{"op": "add", "id": "../x", "title": "t"}]"#).contains("invalid node id"));
        assert!(
            err(r#"[{"op": "add", "id": "a", "title": "t"}, {"op": "add", "id": "a", "title": "t"}]"#)
                .contains("duplicate node id")
        );
        assert!(
            err(r#"[{"op": "add", "ref": "a", "title": "t"}, {"op": "add", "ref": "a", "title": "t"}]"#)
                .contains("defined twice")
        );
        assert!(serde_json::from_str::<Vec<Op>>(r#"[{"op": "add", "title": "t", "bogus": 1}]"#).is_err());
        let cycle = r#"[
            {"op": "add", "ref": "a", "title": "a"},
            {"op": "add", "ref": "b", "title": "b", "depends_on": ["$a"]},
            {"op": "link", "from": "$a", "to": "$b"}
        ]"#;
        assert!(err(cycle).contains("cycle"));
    }

    #[test]
    fn status_can_require_the_current_status() {
        let mut graph = Graph::default();
        apply(&mut graph, &mut parse(r#"[{"op": "add", "id": "a", "title": "a"}]"#)).unwrap();
        let claim = r#"[{"op": "status", "id": "a", "status": "doing", "if_status": "todo"}]"#;
        apply(&mut graph, &mut parse(claim)).unwrap();
        let err = apply(&mut graph, &mut parse(claim)).unwrap_err();
        assert!(matches!(err, Error::Op { source, .. } if matches!(*source, Error::StatusMismatch { .. })));
    }

    #[test]
    fn edit_distinguishes_an_absent_due_from_null() {
        let mut graph = Graph::default();
        let id = NodeId("a".into());
        apply(&mut graph, &mut parse(r#"[{"op": "add", "id": "a", "title": "a", "due": "2026-10-31"}]"#)).unwrap();
        apply(&mut graph, &mut parse(r#"[{"op": "edit", "id": "a", "title": "b"}]"#)).unwrap();
        assert!(graph.get(&id).unwrap().due.is_some());
        let mut clear = parse(r#"[{"op": "edit", "id": "a", "due": null}]"#);
        assert_eq!(serde_json::to_string(&clear).unwrap(), r#"[{"op":"edit","id":"a","due":null}]"#);
        apply(&mut graph, &mut clear).unwrap();
        assert!(graph.get(&id).unwrap().due.is_none());
    }

    #[test]
    fn metadata_is_normalized_set_and_cleared() {
        let mut graph = Graph::default();
        let a = NodeId("a".into());
        let mut add = parse(
            r#"[{"op": "add", "id": "a", "title": "a", "priority": "high", "assignee": " me ", "prs": ["o/r#7"]}]"#,
        );
        apply(&mut graph, &mut add).unwrap();
        let node = graph.get(&a).unwrap();
        assert_eq!(node.priority, Some(Priority::High));
        assert_eq!(node.assignee.as_deref(), Some("me"));
        assert_eq!(node.prs, ["https://github.com/o/r/pull/7"]);
        // The batch records what was applied.
        assert!(
            serde_json::to_string(&add).unwrap().contains(r#""assignee":"me","prs":["https://github.com/o/r/pull/7"]"#)
        );

        apply(&mut graph, &mut parse(r#"[{"op": "edit", "id": "a", "title": "b"}]"#)).unwrap();
        assert_eq!(graph.get(&a).unwrap().priority, Some(Priority::High));
        apply(&mut graph, &mut parse(r#"[{"op": "edit", "id": "a", "priority": null, "assignee": null, "prs": []}]"#))
            .unwrap();
        let node = graph.get(&a).unwrap();
        assert_eq!((node.priority, node.assignee.clone(), node.prs.len()), (None, None, 0));

        let err = |json: &str| apply(&mut graph.clone(), &mut parse(json)).unwrap_err().to_string();
        assert!(
            err(r#"[{"op": "edit", "id": "a", "prs": ["o/r#1", "https://github.com/o/r/pull/1"]}]"#).contains("twice")
        );
        assert!(err(r#"[{"op": "edit", "id": "a", "prs": ["nope"]}]"#).contains("invalid pull request"));
        assert!(err(r#"[{"op": "edit", "id": "a", "assignee": ""}]"#).contains("invalid assignee"));
        assert!(serde_json::from_str::<Vec<Op>>(r#"[{"op": "edit", "id": "a", "priority": "p0"}]"#).is_err());
    }

    #[test]
    fn a_claim_assigns_only_when_it_wins() {
        let mut graph = Graph::default();
        apply(&mut graph, &mut parse(r#"[{"op": "add", "id": "a", "title": "a"}]"#)).unwrap();
        let claim = |who: &str| {
            parse(&format!(
                r#"[{{"op": "status", "id": "a", "status": "doing", "if_status": "todo"}},
                    {{"op": "edit", "id": "a", "assignee": "{who}"}}]"#
            ))
        };
        apply(&mut graph, &mut claim("one")).unwrap();
        // A batch is all or nothing: the caller discards the graph of a failed one.
        let mut lost = graph.clone();
        assert!(apply(&mut lost, &mut claim("two")).is_err());
        assert_eq!(graph.get(&NodeId("a".into())).unwrap().assignee.as_deref(), Some("one"));
    }

    fn node(id: &str, kind: Kind, deps: &[&str], milestones: &[&str]) -> Node {
        let mut n = Node::new(NodeId(id.into()), kind, id.into());
        n.depends_on = deps.iter().map(|d| NodeId((*d).into())).collect();
        n.milestones = milestones.iter().map(|m| NodeId((*m).into())).collect();
        n
    }

    fn assert_diff_round_trips(before: &Graph, after: &Graph) {
        let (old, new) = (before.clone().into_nodes(), after.clone().into_nodes());
        let mut ops = diff(&old, &new);
        let mut graph = before.clone();
        apply(&mut graph, &mut ops).unwrap_or_else(|e| panic!("{e}\nbefore: {old:#?}\nafter: {new:#?}\nops: {ops:#?}"));
        assert_eq!(graph.into_nodes(), new, "ops: {ops:#?}");
    }

    #[test]
    fn diff_removes_before_it_links() {
        // While `x` exists, `m` requires `b` through its member `x`, so `b -> m` would be a cycle.
        let before = Graph::from_nodes([
            node("m", Kind::Milestone, &[], &[]),
            node("b", Kind::Task, &[], &[]),
            node("x", Kind::Task, &["b"], &["m"]),
        ])
        .unwrap();
        let after =
            Graph::from_nodes([node("m", Kind::Milestone, &[], &[]), node("b", Kind::Task, &["m"], &[])]).unwrap();
        assert_diff_round_trips(&before, &after);
    }

    #[test]
    fn diff_of_equal_graphs_is_empty() {
        let graph = Graph::from_nodes([node("a", Kind::Task, &[], &[]), node("b", Kind::Task, &["a"], &[])]).unwrap();
        assert!(diff(&graph.clone().into_nodes(), &graph.into_nodes()).is_empty());
    }

    /// A graph reached by random mutations, most of which the invariants reject.
    fn random_graph(rng: &mut fastrand::Rng) -> Graph {
        const IDS: [&str; 8] = ["a", "b", "c", "d", "e", "f", "g", "h"];
        let mut graph = Graph::default();
        let pick = |rng: &mut fastrand::Rng| NodeId(IDS[rng.usize(..IDS.len())].into());
        for _ in 0..rng.usize(..60) {
            let (x, y) = (pick(rng), pick(rng));
            let kind = if rng.bool() { Kind::Task } else { Kind::Milestone };
            let _ = match rng.usize(..10) {
                0 | 1 => graph.insert(Node::new(x, kind, rng.alphabetic().to_string())),
                2 | 3 => graph.link(&x, &y),
                4 => graph.unlink(&x, &y),
                5 | 6 => graph.join(&x, &y),
                7 => graph.leave(&x, &y),
                8 => graph.remove(&x).map(|_| ()),
                _ => {
                    let status = [Status::Todo, Status::Doing, Status::Done, Status::Dropped][rng.usize(..4)];
                    let due = Some(rng.bool().then(|| jiff::civil::date(2026, 1, rng.i8(1..28))));
                    let tags = Some(vec!["t".to_owned(); rng.usize(..2)]);
                    let body = Some(rng.alphabetic().to_string());
                    let priority = Some(rng.bool().then(|| Priority::ALL[rng.usize(..4)]));
                    let assignee = Some(rng.bool().then(|| rng.alphabetic().to_string()));
                    let prs = Some(vec![format!("https://github.com/o/r/pull/{}", rng.u8(1..3)); rng.usize(..2)]);
                    let edit = Edit { title: None, kind: Some(kind), due, tags, body, priority, assignee, prs };
                    graph.set_status(&x, status).and_then(|()| graph.edit(&x, edit))
                }
            };
        }
        graph
    }

    #[test]
    fn diff_round_trips_between_random_graphs() {
        for seed in 0..5000 {
            let mut rng = fastrand::Rng::with_seed(seed);
            assert_diff_round_trips(&random_graph(&mut rng), &random_graph(&mut rng));
        }
    }
}
