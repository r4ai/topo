//! Copying and pasting nodes, and selecting all of them.
//!
//! The clipboard holds two forms of the copied nodes: a Markdown list that any
//! application can paste, and, as its metadata, the versioned [`Clip`] that a
//! topo window pastes as new nodes.

use std::collections::BTreeMap;

use gpui::{ClipboardEntry, ClipboardItem, Context};
use serde::{Deserialize, Serialize};
use topo_core::wire::WireNode;
use topo_core::{Graph, Kind, Node, NodeId, Status};

use crate::TopoApp;

const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Clip {
    /// Format version. A clip of another version is not pasted.
    topo_clipboard: u32,
    /// The `.topo` directory the nodes were copied from.
    workspace: String,
    nodes: Vec<WireNode>,
}

/// The copied nodes as text for other applications.
fn outline(nodes: &[&Node]) -> String {
    let line = |node: &&Node| {
        let mark = match (node.kind, node.status.is_closed()) {
            (Kind::Milestone, _) => "◆",
            (Kind::Task, true) => "[x]",
            (Kind::Task, false) => "[ ]",
        };
        format!("- {mark} {}", node.title)
    };
    nodes.iter().map(line).collect::<Vec<_>>().join("\n")
}

/// `graph` with a copy of every node of `clip` added, and the ids of the copies.
///
/// A copy is new work: it gets a fresh id, starts as `todo`, and has no
/// assignee, pull requests or timestamps. Its title, kind, due date, tags,
/// priority and notes are kept. Edges between copied nodes connect the copies.
/// An edge to a node outside the clip is kept when pasting into the workspace
/// it was copied from and that node still exists, and dropped otherwise.
fn paste(graph: &Graph, clip: Clip, workspace: &str) -> Result<(Graph, Vec<NodeId>), topo_core::Error> {
    let same_workspace = clip.workspace == workspace;
    let sources: Vec<Node> = clip.nodes.into_iter().map(Node::from).collect();
    let mut ids: BTreeMap<&NodeId, NodeId> = BTreeMap::new();
    for source in &sources {
        let taken = |id: &NodeId| graph.get(id).is_some() || ids.values().any(|new| new == id);
        let id = std::iter::repeat_with(NodeId::random).find(|id| !taken(id)).expect("infinite iterator");
        ids.insert(&source.id, id);
    }
    let remap = |reference: &NodeId, kind: Option<Kind>| {
        let outside = || {
            let node = graph.get(reference).filter(|_| same_workspace)?;
            kind.is_none_or(|kind| node.kind == kind).then(|| reference.clone())
        };
        ids.get(reference).cloned().or_else(outside)
    };
    let copies: Vec<Node> = sources
        .iter()
        .map(|source| Node {
            depends_on: source.depends_on.iter().filter_map(|d| remap(d, None)).collect(),
            milestones: source.milestones.iter().filter_map(|m| remap(m, Some(Kind::Milestone))).collect(),
            status: Status::Todo,
            assignee: None,
            prs: Vec::new(),
            created_at: None,
            updated_at: None,
            completed_at: None,
            ..Node { id: ids[&source.id].clone(), ..source.clone() }
        })
        .collect();
    let new_ids = copies.iter().map(|n| n.id.clone()).collect();
    // Validating the whole graph rejects a clip that is not a valid set of nodes.
    Ok((Graph::from_nodes(graph.nodes().cloned().chain(copies))?, new_ids))
}

impl TopoApp {
    fn workspace_key(&self) -> String {
        self.ws.dir().display().to_string()
    }

    /// Selects every node the canvas shows undimmed by the priority filter.
    pub(crate) fn select_all(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<NodeId> = self.graph().nodes().filter(|n| self.passes_filter(n)).map(|n| n.id.clone()).collect();
        self.selected_nodes = ids.into_iter().collect();
        if self.selected.as_ref().is_none_or(|id| !self.selected_nodes.contains(id)) {
            self.selected = self.selected_nodes.first().cloned();
        }
        cx.notify();
    }

    /// Puts the selected nodes on the clipboard. Returns whether there were any.
    pub(crate) fn copy_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let nodes: Vec<&Node> = self.selected_nodes.iter().filter_map(|id| self.graph().get(id)).collect();
        if nodes.is_empty() {
            return false;
        }
        let clip = Clip {
            topo_clipboard: VERSION,
            workspace: self.workspace_key(),
            nodes: nodes.iter().map(|n| WireNode::from((*n).clone())).collect(),
        };
        let count = nodes.len();
        cx.write_to_clipboard(ClipboardItem::new_string_with_json_metadata(outline(&nodes), clip));
        self.toast(format!("Copied {count} node{}", if count == 1 { "" } else { "s" }), false, cx);
        true
    }

    /// Copies the selection and deletes it. The deletion is one undo step, and
    /// the nodes stay on the clipboard whatever happens to a later paste.
    pub(crate) fn cut_selection(&mut self, cx: &mut Context<Self>) {
        if self.copy_selection(cx) {
            self.delete_selected(cx);
        }
    }

    /// Adds the nodes on the clipboard as new nodes, in one undo step, and selects them.
    pub(crate) fn paste_nodes(&mut self, cx: &mut Context<Self>) {
        let clip = cx.read_from_clipboard().and_then(|item| {
            item.entries().iter().find_map(|entry| match entry {
                ClipboardEntry::String(text) => text.metadata_json::<Clip>(),
                _ => None,
            })
        });
        let Some(clip) = clip.filter(|clip| clip.topo_clipboard == VERSION) else {
            return self.toast("The clipboard holds no topo nodes", true, cx);
        };
        let workspace = self.workspace_key();
        let mut pasted = Vec::new();
        let ok = self.mutate(cx, |graph| {
            let (next, ids) = paste(graph, clip, &workspace)?;
            (*graph, pasted) = (next, ids);
            Ok(())
        });
        if ok {
            let count = pasted.len();
            // A cloud save can come back with ids that other writers removed meanwhile.
            self.selected_nodes = pasted.into_iter().filter(|id| self.graph().get(id).is_some()).collect();
            self.selected = self.selected_nodes.first().cloned();
            self.frame_selection();
            self.toast(format!("Pasted {count} node{}", if count == 1 { "" } else { "s" }), false, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use topo_core::Priority;

    use super::*;

    fn id(s: &str) -> NodeId {
        NodeId(s.into())
    }

    fn graph() -> Graph {
        let mut m = Node::new(id("m"), Kind::Milestone, "m".into());
        m.depends_on = vec![];
        let ext = Node::new(id("ext"), Kind::Task, "ext".into());
        let mut a = Node::new(id("a"), Kind::Task, "a".into());
        a.depends_on = vec![id("ext")];
        a.milestones = vec![id("m")];
        a.status = Status::Done;
        a.assignee = Some("me".into());
        a.prs = vec!["https://github.com/o/r/pull/1".into()];
        a.priority = Some(Priority::High);
        a.created_at = Some(jiff::Timestamp::from_second(1).unwrap());
        let mut b = Node::new(id("b"), Kind::Task, "b".into());
        b.depends_on = vec![id("a")];
        Graph::from_nodes([m, ext, a, b]).unwrap()
    }

    fn clip(graph: &Graph, ids: &[&str], workspace: &str) -> Clip {
        let nodes = ids.iter().map(|i| WireNode::from(graph.get(&id(i)).unwrap().clone())).collect();
        Clip { topo_clipboard: VERSION, workspace: workspace.into(), nodes }
    }

    #[test]
    fn copies_are_new_work_with_remapped_edges() {
        let graph = graph();
        let (pasted, ids) = paste(&graph, clip(&graph, &["a", "b"], "w"), "w").unwrap();
        assert_eq!(pasted.nodes().count(), 6);
        let (a, b) = (pasted.get(&ids[0]).unwrap(), pasted.get(&ids[1]).unwrap());
        assert!(graph.get(&a.id).is_none() && a.id != b.id);
        // Inside the clip the copy depends on the copy; outside references stay.
        assert_eq!(b.depends_on, std::slice::from_ref(&a.id));
        assert_eq!((a.depends_on.clone(), a.milestones.clone()), (vec![id("ext")], vec![id("m")]));
        assert_eq!((a.status, a.priority, a.title.as_str()), (Status::Todo, Some(Priority::High), "a"));
        assert_eq!((a.assignee.clone(), a.prs.len(), a.created_at), (None, 0, None));
        // The originals are untouched.
        assert_eq!(pasted.get(&id("a")), graph.get(&id("a")));
    }

    #[test]
    fn another_workspace_drops_outside_references() {
        let source = graph();
        // `ext` exists here too, but it is another workspace's node of that name.
        let target = Graph::from_nodes([Node::new(id("ext"), Kind::Task, "other".into())]).unwrap();
        let (pasted, ids) = paste(&target, clip(&source, &["a", "b"], "w"), "elsewhere").unwrap();
        let a = pasted.get(&ids[0]).unwrap();
        assert!(a.depends_on.is_empty() && a.milestones.is_empty());
        assert_eq!(pasted.get(&ids[1]).unwrap().depends_on, std::slice::from_ref(&a.id));
    }

    #[test]
    fn an_invalid_clip_pastes_nothing() {
        let graph = graph();
        let mut twice = clip(&graph, &["a", "a"], "w");
        assert!(paste(&graph, Clip { nodes: std::mem::take(&mut twice.nodes), ..twice }, "w").is_err());
        // A membership in a node that became a task since the copy is dropped, not an error.
        let clip = clip(&graph, &["a"], "w");
        let mut changed = graph.clone();
        changed.leave(&id("a"), &id("m")).unwrap();
        changed.edit(&id("m"), topo_core::Edit { kind: Some(Kind::Task), ..Default::default() }).unwrap();
        let (pasted, ids) = paste(&changed, clip, "w").unwrap();
        assert!(pasted.get(&ids[0]).unwrap().milestones.is_empty());
    }

    #[test]
    fn the_text_form_is_a_markdown_list() {
        let graph = graph();
        let nodes: Vec<&Node> = ["m", "a", "b"].iter().map(|i| graph.get(&id(i)).unwrap()).collect();
        assert_eq!(outline(&nodes), "- ◆ m\n- [x] a\n- [ ] b");
    }
}
