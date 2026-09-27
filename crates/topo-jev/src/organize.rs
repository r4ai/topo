//! Graph organizing on top of a decision model.
//!
//! A System One model does not generate anything, so every operation enumerates
//! candidates from the graph, asks typed questions about them, and turns
//! confident answers into [`Proposal`]s. Proposals are applied separately.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Value, json};
use topo_core::{Edit, Graph, Kind, Node, NodeId};

use crate::{Client, Error, Question};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "proposal", rename_all = "snake_case")]
pub enum Proposal {
    /// `from` should depend on `to`.
    Link {
        from: NodeId,
        to: NodeId,
        probability: f64,
    },
    SetKind {
        id: NodeId,
        kind: Kind,
        probability: f64,
    },
    /// Informational: merging is left to a human or an agent.
    Duplicate {
        a: NodeId,
        b: NodeId,
        probability: f64,
    },
}

impl Proposal {
    fn probability(&self) -> f64 {
        match self {
            Proposal::Link { probability, .. }
            | Proposal::SetKind { probability, .. }
            | Proposal::Duplicate { probability, .. } => *probability,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Applied {
    #[serde(flatten)]
    pub proposal: Proposal,
    /// `None` when applied, otherwise why it was skipped.
    pub skipped: Option<String>,
}

/// Applies proposals, most probable first, so that when two proposals would
/// form a cycle the weaker one is the one skipped.
pub fn apply(graph: &mut Graph, mut proposals: Vec<Proposal>) -> Vec<Applied> {
    proposals.sort_by(|a, b| b.probability().total_cmp(&a.probability()));
    proposals
        .into_iter()
        .map(|proposal| {
            let result = match &proposal {
                Proposal::Link { from, to, .. } => graph.link(from, to).map_err(|e| e.to_string()),
                Proposal::SetKind { id, kind, .. } => {
                    graph.edit(id, Edit { kind: Some(*kind), ..Edit::default() }).map_err(|e| e.to_string())
                }
                Proposal::Duplicate { .. } => Err("duplicates are not merged automatically".into()),
            };
            Applied { proposal, skipped: result.err() }
        })
        .collect()
}

/// What the model sees of a node: its content and its immediate neighborhood.
fn describe(graph: &Graph, node: &Node) -> Value {
    let titles = |nodes: Vec<&Node>| nodes.iter().map(|n| n.title.clone()).collect::<Vec<_>>();
    json!({
        "id": node.id,
        "kind": node.kind,
        "title": node.title,
        "status": node.status,
        "due": node.due,
        "tags": node.tags,
        "notes": node.body,
        "depends_on": titles(node.depends_on.iter().filter_map(|d| graph.get(d)).collect()),
        "needed_by": titles(graph.dependents(&node.id).collect()),
    })
}

fn open_nodes(graph: &Graph) -> impl Iterator<Item = &Node> {
    graph.nodes().filter(|n| !n.status.is_closed())
}

/// Asks, for every open task, which unrelated open tasks must be finished first.
pub fn dependencies(graph: &Graph, client: &Client) -> Result<Vec<Proposal>, Error> {
    let mut proposals = Vec::new();
    for task in open_nodes(graph).filter(|n| n.kind == Kind::Task) {
        let candidates: Vec<&Node> = open_nodes(graph)
            .filter(|c| c.kind == Kind::Task)
            .filter(|c| !graph.reaches(&task.id, &c.id) && !graph.reaches(&c.id, &task.id))
            .collect();
        if candidates.is_empty() {
            continue;
        }
        let questions = candidates
            .iter()
            .map(|c| {
                let instructions = format!(
                    "Must candidate `{}` (\"{}\") be finished before `task` can be started or completed? \
                     Answer yes only for a real prerequisite, not for mere relatedness.",
                    c.id, c.title
                );
                (c.id.to_string(), Question::Noul { instructions })
            })
            .collect();
        let state = json!({
            "task": describe(graph, task),
            "candidates": candidates.iter().map(|c| describe(graph, c)).collect::<Vec<_>>(),
        });
        let answers = client.decide(state, questions)?;
        for c in candidates {
            let probability = answers.noul(c.id.as_str())?;
            if probability >= client.threshold() {
                proposals.push(Proposal::Link { from: task.id.clone(), to: c.id.clone(), probability });
            }
        }
    }
    Ok(proposals)
}

/// Open tasks that no milestone (transitively) depends on.
pub fn unplaced_tasks(graph: &Graph) -> Vec<NodeId> {
    let placed: Vec<NodeId> =
        graph.nodes().filter(|n| n.kind == Kind::Milestone).flat_map(|m| graph.descendants(&m.id)).collect();
    open_nodes(graph).filter(|n| n.kind == Kind::Task && !placed.contains(&n.id)).map(|n| n.id.clone()).collect()
}

/// Asks which open milestone each task contributes to.
pub fn placement(graph: &Graph, client: &Client, tasks: &[NodeId]) -> Result<Vec<Proposal>, Error> {
    const NONE: &str = "none";
    let milestones: Vec<&Node> = open_nodes(graph).filter(|n| n.kind == Kind::Milestone).collect();
    if milestones.is_empty() || tasks.is_empty() {
        return Ok(Vec::new());
    }
    let mut criteria: BTreeMap<String, String> =
        milestones.iter().map(|m| (m.id.to_string(), format!("Milestone \"{}\"", m.title))).collect();
    criteria.insert(NONE.into(), "The task does not belong to any of these milestones".into());
    let questions = tasks
        .iter()
        .map(|id| {
            let instructions = format!(
                "Which milestone does task `{id}` (\"{}\") need to be completed for?",
                graph.get(id).expect("caller passes existing ids").title
            );
            (id.to_string(), Question::Choice { instructions, criteria: criteria.clone() })
        })
        .collect();
    let state = json!({
        "milestones": milestones.iter().map(|m| describe(graph, m)).collect::<Vec<_>>(),
        "tasks": tasks.iter().map(|id| describe(graph, graph.get(id).expect("caller passes existing ids"))).collect::<Vec<_>>(),
    });
    let answers = client.decide(state, questions)?;
    let mut proposals = Vec::new();
    for id in tasks {
        let (choice, probability) = answers.choice(id.as_str())?;
        if choice != NONE && probability >= client.threshold() {
            proposals.push(Proposal::Link { from: NodeId(choice), to: id.clone(), probability });
        }
    }
    Ok(proposals)
}

/// Asks, pairwise, whether two open nodes describe the same work.
pub fn duplicates(graph: &Graph, client: &Client) -> Result<Vec<Proposal>, Error> {
    let nodes: Vec<&Node> = open_nodes(graph).collect();
    let mut proposals = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        let candidates: Vec<&&Node> = nodes[i + 1..].iter().filter(|c| c.kind == node.kind).collect();
        if candidates.is_empty() {
            continue;
        }
        let questions = candidates
            .iter()
            .map(|c| {
                let instructions =
                    format!("Do `node` and candidate `{}` (\"{}\") describe the same piece of work?", c.id, c.title);
                (c.id.to_string(), Question::Noul { instructions })
            })
            .collect();
        let state = json!({
            "node": describe(graph, node),
            "candidates": candidates.iter().map(|c| describe(graph, c)).collect::<Vec<_>>(),
        });
        let answers = client.decide(state, questions)?;
        for c in candidates {
            let probability = answers.noul(c.id.as_str())?;
            if probability >= client.threshold() {
                proposals.push(Proposal::Duplicate { a: node.id.clone(), b: c.id.clone(), probability });
            }
        }
    }
    Ok(proposals)
}

/// Asks whether each open node is a concrete task or a milestone.
pub fn kinds(graph: &Graph, client: &Client) -> Result<Vec<Proposal>, Error> {
    let nodes: Vec<&Node> = open_nodes(graph).collect();
    if nodes.is_empty() {
        return Ok(Vec::new());
    }
    let criteria = BTreeMap::from([
        ("task".to_owned(), "A concrete piece of work someone can directly do".to_owned()),
        ("milestone".to_owned(), "A goal or checkpoint that is reached when other work is complete".to_owned()),
    ]);
    let questions = nodes
        .iter()
        .map(|n| {
            let instructions = format!("Is node `{}` (\"{}\") a task or a milestone?", n.id, n.title);
            (n.id.to_string(), Question::Choice { instructions, criteria: criteria.clone() })
        })
        .collect();
    let state = json!({ "nodes": nodes.iter().map(|n| describe(graph, n)).collect::<Vec<_>>() });
    let answers = client.decide(state, questions)?;
    let mut proposals = Vec::new();
    for node in nodes {
        let (choice, probability) = answers.choice(node.id.as_str())?;
        let kind = if choice == "milestone" { Kind::Milestone } else { Kind::Task };
        if kind != node.kind && probability >= client.threshold() {
            proposals.push(Proposal::SetKind { id: node.id.clone(), kind, probability });
        }
    }
    Ok(proposals)
}

#[derive(Debug, Serialize)]
pub struct Priority {
    pub id: NodeId,
    pub title: String,
    /// 0 = low, 1 = critical.
    pub score: f64,
}

/// Scores the ready tasks (optionally under `scope`), highest priority first.
pub fn prioritize(graph: &Graph, client: &Client, scope: Option<&NodeId>) -> Result<Vec<Priority>, Error> {
    let tasks = graph.ready_tasks(scope);
    if tasks.is_empty() {
        return Ok(Vec::new());
    }
    let criteria: Vec<String> = ["low", "medium", "high", "critical"].map(String::from).into();
    let questions = tasks
        .iter()
        .map(|t| {
            let instructions = format!(
                "How important is it to work on task `{}` (\"{}\") now, considering deadlines \
                 and how much other work it unblocks?",
                t.id, t.title
            );
            (t.id.to_string(), Question::Score { instructions, criteria: criteria.clone() })
        })
        .collect();
    let state = json!({
        "ready_tasks": tasks.iter().map(|t| describe(graph, t)).collect::<Vec<_>>(),
        "milestones": open_nodes(graph).filter(|n| n.kind == Kind::Milestone).map(|m| describe(graph, m)).collect::<Vec<_>>(),
    });
    let answers = client.decide(state, questions)?;
    let mut ranked = tasks
        .iter()
        .map(|t| Ok(Priority { id: t.id.clone(), title: t.title.clone(), score: answers.score(t.id.as_str())? }))
        .collect::<Result<Vec<_>, Error>>()?;
    ranked.sort_by(|a, b| b.score.total_cmp(&a.score));
    Ok(ranked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;

    fn node(id: &str, kind: Kind, title: &str, deps: &[&str]) -> Node {
        let mut n = Node::new(NodeId(id.into()), kind, title.into());
        n.depends_on = deps.iter().map(|d| NodeId((*d).into())).collect();
        n
    }

    /// Serves `/v1/systemone`, answering each question with `answer(state, key, question)`.
    fn server(answer: fn(&Value, &str, &Value) -> Value) -> (mockito::ServerGuard, Client) {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/v1/systemone")
            .with_body_from_request(move |req| {
                let body: Value = serde_json::from_slice(req.body().unwrap()).unwrap();
                let answers: serde_json::Map<String, Value> = body["questions"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, q)| (k.clone(), answer(&body["state"], k, q)))
                    .collect();
                serde_json::to_vec(&json!({ "answers": answers })).unwrap()
            })
            .create();
        let client = Client::new(Config { base_url: server.url(), ..Config::default() });
        (server, client)
    }

    #[test]
    fn dependencies_proposes_confident_links_and_apply_skips_cycles() {
        let mut graph = Graph::from_nodes([
            node("design", Kind::Task, "Design API", &[]),
            node("impl", Kind::Task, "Implement API", &[]),
        ])
        .unwrap();
        // The model claims both directions; only the stronger one may be applied.
        let (_s, client) = server(|state, key, _| {
            let yes = match (state["task"]["id"].as_str().unwrap(), key) {
                ("impl", "design") => 0.95,
                ("design", "impl") => 0.75,
                _ => 0.1,
            };
            json!({ "noul": yes })
        });
        let proposals = dependencies(&graph, &client).unwrap();
        assert_eq!(proposals.len(), 2);

        let applied = apply(&mut graph, proposals);
        assert_eq!(
            applied[0].proposal,
            Proposal::Link { from: NodeId("impl".into()), to: NodeId("design".into()), probability: 0.95 }
        );
        assert!(applied[0].skipped.is_none());
        assert!(applied[1].skipped.as_deref().unwrap().contains("cycle"));
    }

    #[test]
    fn placement_links_milestone_to_task() {
        let graph = Graph::from_nodes([
            node("m1", Kind::Milestone, "v1 release", &[]),
            node("t1", Kind::Task, "Write changelog", &[]),
            node("t2", Kind::Task, "Water plants", &[]),
        ])
        .unwrap();
        let (_s, client) = server(|_, key, question| {
            assert_eq!(question["type"], "choice");
            let (choice, p) = if key == "t1" { ("m1", 0.9) } else { ("none", 0.8) };
            json!({ "choice": choice, "probabilities": { choice: p } })
        });
        let tasks = unplaced_tasks(&graph);
        assert_eq!(tasks.len(), 2);
        assert_eq!(
            placement(&graph, &client, &tasks).unwrap(),
            [Proposal::Link { from: NodeId("m1".into()), to: NodeId("t1".into()), probability: 0.9 }]
        );
    }

    #[test]
    fn prioritize_sorts_by_score_and_missing_answers_fail() {
        let graph = Graph::from_nodes([node("a", Kind::Task, "a", &[]), node("b", Kind::Task, "b", &[])]).unwrap();
        let (_s, client) = server(|_, key, _| json!({ "score": if key == "b" { 0.9 } else { 0.2 } }));
        let ranked = prioritize(&graph, &client, None).unwrap();
        assert_eq!(ranked.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["b", "a"]);

        let (_s, client) = server(|_, _, _| json!({}));
        assert!(matches!(prioritize(&graph, &client, None), Err(Error::MissingAnswer { .. })));
    }

    #[test]
    fn unreachable_server_is_a_clear_error() {
        let client = Client::new(Config { base_url: "http://127.0.0.1:9".into(), ..Config::default() });
        let graph = Graph::from_nodes([node("a", Kind::Task, "a", &[])]).unwrap();
        let err = kinds(&graph, &client).unwrap_err();
        assert!(err.to_string().contains("jev.base_url"));
    }
}
