use std::collections::{BTreeMap, BTreeSet, HashMap};

use jiff::civil::Date;

use crate::Error;
use crate::model::{Kind, Node, NodeId, Status};

/// All nodes of a workspace.
///
/// Invariants: every `depends_on` entry names an existing node, no node depends
/// on itself or twice on the same node, and the dependency relation is acyclic.
/// Every mutation below preserves them, so readers never re-check.
#[derive(Debug, Clone, Default)]
pub struct Graph {
    nodes: BTreeMap<NodeId, Node>,
}

/// Field changes for [`Graph::edit`]. `None` leaves a field unchanged.
#[derive(Debug, Clone, Default)]
pub struct Edit {
    pub title: Option<String>,
    pub kind: Option<Kind>,
    pub due: Option<Option<Date>>,
    pub tags: Option<Vec<String>>,
    pub body: Option<String>,
}

impl Graph {
    pub fn from_nodes(nodes: impl IntoIterator<Item = Node>) -> Result<Self, Error> {
        let mut graph = Graph::default();
        for node in nodes {
            if graph.nodes.contains_key(&node.id) {
                return Err(Error::DuplicateId(node.id));
            }
            graph.nodes.insert(node.id.clone(), node);
        }
        for node in graph.nodes.values() {
            graph.check_deps(node)?;
        }
        graph.topo_order()?;
        Ok(graph)
    }

    pub fn get(&self, id: &NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    pub fn into_nodes(self) -> BTreeMap<NodeId, Node> {
        self.nodes
    }

    /// Resolves an exact id or a unique id prefix.
    pub fn resolve(&self, prefix: &str) -> Result<NodeId, Error> {
        let id = NodeId(prefix.to_owned());
        if self.nodes.contains_key(&id) {
            return Ok(id);
        }
        let matches: Vec<&NodeId> = self.nodes.keys().filter(|k| k.0.starts_with(prefix)).collect();
        match matches.as_slice() {
            [] => Err(Error::NotFound(prefix.to_owned())),
            [one] => Ok((*one).clone()),
            many => Err(Error::Ambiguous {
                prefix: prefix.to_owned(),
                candidates: many.iter().map(|id| (*id).clone()).collect(),
            }),
        }
    }

    fn node(&self, id: &NodeId) -> Result<&Node, Error> {
        self.nodes.get(id).ok_or_else(|| Error::NotFound(id.0.clone()))
    }

    fn node_mut(&mut self, id: &NodeId) -> Result<&mut Node, Error> {
        self.nodes.get_mut(id).ok_or_else(|| Error::NotFound(id.0.clone()))
    }

    fn check_deps(&self, node: &Node) -> Result<(), Error> {
        let mut seen = BTreeSet::new();
        for dep in &node.depends_on {
            if dep == &node.id {
                return Err(Error::Cycle(vec![node.id.clone()]));
            }
            if !self.nodes.contains_key(dep) {
                return Err(Error::DanglingDependency { node: node.id.clone(), dep: dep.clone() });
            }
            if !seen.insert(dep) {
                return Err(Error::DuplicateDependency { node: node.id.clone(), dep: dep.clone() });
            }
        }
        Ok(())
    }

    // ---- queries -------------------------------------------------------

    /// Nodes that directly depend on `id`.
    pub fn dependents<'a>(&'a self, id: &'a NodeId) -> impl Iterator<Item = &'a Node> + 'a {
        self.nodes.values().filter(move |n| n.depends_on.contains(id))
    }

    /// All transitive dependencies of `id`, excluding `id` itself.
    pub fn descendants(&self, id: &NodeId) -> BTreeSet<NodeId> {
        let mut seen = BTreeSet::new();
        let mut stack: Vec<&NodeId> = self.nodes[id].depends_on.iter().collect();
        while let Some(next) = stack.pop() {
            if seen.insert(next.clone()) {
                stack.extend(&self.nodes[next].depends_on);
            }
        }
        seen
    }

    /// Whether `from` transitively depends on `to` (or they are the same node).
    pub fn reaches(&self, from: &NodeId, to: &NodeId) -> bool {
        from == to || self.descendants(from).contains(to)
    }

    /// An open node whose dependencies are all closed.
    pub fn is_ready(&self, node: &Node) -> bool {
        !node.status.is_closed() && node.depends_on.iter().all(|d| self.nodes[d].status.is_closed())
    }

    /// Open tasks that can be started now, optionally limited to the work under `scope`.
    pub fn ready_tasks(&self, scope: Option<&NodeId>) -> Vec<&Node> {
        let in_scope = scope.map(|s| self.descendants(s));
        self.nodes
            .values()
            .filter(|n| n.kind == Kind::Task && self.is_ready(n))
            .filter(|n| in_scope.as_ref().is_none_or(|set| set.contains(&n.id)))
            .collect()
    }

    /// `(closed, total)` tasks among the transitive dependencies of `id`.
    pub fn progress(&self, id: &NodeId) -> (usize, usize) {
        let tasks: Vec<&Node> =
            self.descendants(id).iter().map(|d| &self.nodes[d]).filter(|n| n.kind == Kind::Task).collect();
        (tasks.iter().filter(|n| n.status.is_closed()).count(), tasks.len())
    }

    /// The longest chain of open tasks below `id` (inclusive), dependencies first.
    /// Its length is the minimum number of sequential steps left.
    pub fn critical_path(&self, id: &NodeId) -> Vec<NodeId> {
        fn walk(g: &Graph, id: &NodeId, memo: &mut HashMap<NodeId, Vec<NodeId>>) -> Vec<NodeId> {
            if let Some(path) = memo.get(id) {
                return path.clone();
            }
            let node = &g.nodes[id];
            let mut path = node
                .depends_on
                .iter()
                .filter(|d| !g.nodes[*d].status.is_closed())
                .map(|d| walk(g, d, memo))
                .max_by_key(Vec::len)
                .unwrap_or_default();
            if node.kind == Kind::Task && !node.status.is_closed() {
                path.push(id.clone());
            }
            memo.insert(id.clone(), path.clone());
            path
        }
        walk(self, id, &mut HashMap::new())
    }

    /// Node ids ordered so that every node comes after its dependencies.
    /// Fails with the offending cycle if the relation is not acyclic.
    pub fn topo_order(&self) -> Result<Vec<NodeId>, Error> {
        #[derive(Clone, Copy, PartialEq)]
        enum Mark {
            Visiting,
            Done,
        }
        fn visit<'a>(
            g: &'a Graph,
            id: &'a NodeId,
            marks: &mut HashMap<&'a NodeId, Mark>,
            path: &mut Vec<&'a NodeId>,
            out: &mut Vec<NodeId>,
        ) -> Result<(), Error> {
            match marks.get(id) {
                Some(Mark::Done) => return Ok(()),
                Some(Mark::Visiting) => {
                    let start = path.iter().position(|p| *p == id).expect("visiting node is on the path");
                    return Err(Error::Cycle(path[start..].iter().map(|p| (*p).clone()).collect()));
                }
                None => {}
            }
            marks.insert(id, Mark::Visiting);
            path.push(id);
            for dep in &g.nodes[id].depends_on {
                visit(g, dep, marks, path, out)?;
            }
            path.pop();
            marks.insert(id, Mark::Done);
            out.push(id.clone());
            Ok(())
        }
        let mut marks = HashMap::new();
        let mut out = Vec::with_capacity(self.nodes.len());
        for id in self.nodes.keys() {
            visit(self, id, &mut marks, &mut Vec::new(), &mut out)?;
        }
        Ok(out)
    }

    /// Nodes that no other node depends on (the tops of the graph).
    pub fn roots(&self) -> Vec<&Node> {
        let depended: BTreeSet<&NodeId> = self.nodes.values().flat_map(|n| &n.depends_on).collect();
        self.nodes.values().filter(|n| !depended.contains(&n.id)).collect()
    }

    // ---- mutations -----------------------------------------------------

    /// Inserts a new node. Its dependencies must already exist.
    pub fn insert(&mut self, node: Node) -> Result<(), Error> {
        if self.nodes.contains_key(&node.id) {
            return Err(Error::DuplicateId(node.id));
        }
        self.check_deps(&node)?;
        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    /// Generates an id that is not used yet.
    pub fn fresh_id(&self) -> NodeId {
        std::iter::repeat_with(NodeId::random).find(|id| !self.nodes.contains_key(id)).expect("infinite iterator")
    }

    /// Makes `from` depend on `to`. Linking an existing edge is a no-op.
    pub fn link(&mut self, from: &NodeId, to: &NodeId) -> Result<(), Error> {
        self.node(to)?;
        if self.node(from)?.depends_on.contains(to) {
            return Ok(());
        }
        if self.reaches(to, from) {
            return Err(Error::WouldCycle { from: from.clone(), to: to.clone() });
        }
        self.node_mut(from)?.depends_on.push(to.clone());
        Ok(())
    }

    pub fn unlink(&mut self, from: &NodeId, to: &NodeId) -> Result<(), Error> {
        let deps = &mut self.node_mut(from)?.depends_on;
        let index =
            deps.iter().position(|d| d == to).ok_or_else(|| Error::NotLinked { from: from.clone(), to: to.clone() })?;
        deps.remove(index);
        Ok(())
    }

    pub fn set_status(&mut self, id: &NodeId, status: Status) -> Result<(), Error> {
        self.node_mut(id)?.status = status;
        Ok(())
    }

    pub fn edit(&mut self, id: &NodeId, edit: Edit) -> Result<(), Error> {
        let node = self.node_mut(id)?;
        if let Some(title) = edit.title {
            node.title = title;
        }
        if let Some(kind) = edit.kind {
            node.kind = kind;
        }
        if let Some(due) = edit.due {
            node.due = due;
        }
        if let Some(tags) = edit.tags {
            node.tags = tags;
        }
        if let Some(body) = edit.body {
            node.body = body;
        }
        Ok(())
    }

    /// Removes a node and every edge pointing at it.
    pub fn remove(&mut self, id: &NodeId) -> Result<Node, Error> {
        let node = self.nodes.remove(id).ok_or_else(|| Error::NotFound(id.0.clone()))?;
        for other in self.nodes.values_mut() {
            other.depends_on.retain(|d| d != id);
        }
        Ok(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, kind: Kind, deps: &[&str]) -> Node {
        let mut n = Node::new(NodeId(id.into()), kind, id.into());
        n.depends_on = deps.iter().map(|d| NodeId((*d).into())).collect();
        n
    }

    fn id(s: &str) -> NodeId {
        NodeId(s.into())
    }

    /// m <- b <- a, m <- c
    fn sample() -> Graph {
        Graph::from_nodes([
            node("a", Kind::Task, &[]),
            node("b", Kind::Task, &["a"]),
            node("c", Kind::Task, &[]),
            node("m", Kind::Milestone, &["b", "c"]),
        ])
        .unwrap()
    }

    #[test]
    fn rejects_invalid_graphs() {
        assert!(matches!(Graph::from_nodes([node("a", Kind::Task, &["x"])]), Err(Error::DanglingDependency { .. })));
        assert!(matches!(
            Graph::from_nodes([node("a", Kind::Task, &["b"]), node("b", Kind::Task, &["a"])]),
            Err(Error::Cycle(_))
        ));
        assert!(matches!(Graph::from_nodes([node("a", Kind::Task, &["a"])]), Err(Error::Cycle(_))));
    }

    #[test]
    fn link_rejects_cycles_and_is_idempotent() {
        let mut g = sample();
        assert!(matches!(g.link(&id("a"), &id("m")), Err(Error::WouldCycle { .. })));
        assert!(matches!(g.link(&id("a"), &id("a")), Err(Error::WouldCycle { .. })));
        g.link(&id("b"), &id("a")).unwrap();
        assert_eq!(g.get(&id("b")).unwrap().depends_on, vec![id("a")]);
        g.link(&id("c"), &id("a")).unwrap();
        assert!(g.reaches(&id("c"), &id("a")));
    }

    #[test]
    fn ready_progress_and_critical_path_follow_status() {
        let mut g = sample();
        let ready = |g: &Graph| g.ready_tasks(None).iter().map(|n| n.id.0.clone()).collect::<Vec<_>>();
        assert_eq!(ready(&g), ["a", "c"]);
        assert_eq!(g.progress(&id("m")), (0, 3));
        assert_eq!(g.critical_path(&id("m")), [id("a"), id("b")]);

        g.set_status(&id("a"), Status::Done).unwrap();
        assert_eq!(ready(&g), ["b", "c"]);
        assert_eq!(g.progress(&id("m")), (1, 3));
        assert_eq!(g.critical_path(&id("m")).len(), 1);
        assert_eq!(g.ready_tasks(Some(&id("b"))).len(), 0);
    }

    #[test]
    fn topo_order_puts_dependencies_first() {
        let order = sample().topo_order().unwrap();
        let pos = |s: &str| order.iter().position(|x| x.0 == s).unwrap();
        assert!(pos("a") < pos("b") && pos("b") < pos("m") && pos("c") < pos("m"));
    }

    #[test]
    fn remove_cleans_edges_and_resolve_uses_prefixes() {
        let mut g = sample();
        g.remove(&id("b")).unwrap();
        assert_eq!(g.get(&id("m")).unwrap().depends_on, vec![id("c")]);
        assert_eq!(g.roots().len(), 2);

        let g = Graph::from_nodes([node("abc", Kind::Task, &[]), node("abd", Kind::Task, &[])]).unwrap();
        assert_eq!(g.resolve("abc").unwrap(), id("abc"));
        assert!(matches!(g.resolve("ab"), Err(Error::Ambiguous { .. })));
        assert!(matches!(g.resolve("z"), Err(Error::NotFound(_))));
    }
}
