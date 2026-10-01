use std::collections::{BTreeMap, BTreeSet, HashMap};

use jiff::civil::Date;

use crate::Error;
use crate::model::{Kind, Node, NodeId, Status};

/// All nodes of a workspace.
///
/// Two relations connect nodes: `depends_on` (ordering) and a task's
/// `milestones` (set membership). Together they form the *requirement*
/// relation: a node requires its dependencies, and a milestone additionally
/// requires its member tasks.
///
/// Invariants: every reference names an existing node, with no self or
/// duplicate references; only tasks have `milestones`, and those name
/// milestones; the requirement relation is acyclic. Every mutation below
/// preserves them, so readers never re-check.
#[derive(Debug, Clone, Default, PartialEq)]
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
            graph.check_references(node)?;
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

    fn check_references(&self, node: &Node) -> Result<(), Error> {
        let mut seen = BTreeSet::new();
        for dep in &node.depends_on {
            if dep == &node.id {
                return Err(Error::Cycle(vec![node.id.clone()]));
            }
            if !self.nodes.contains_key(dep) {
                return Err(Error::DanglingReference { node: node.id.clone(), target: dep.clone() });
            }
            if !seen.insert(dep) {
                return Err(Error::DuplicateReference { node: node.id.clone(), target: dep.clone() });
            }
        }
        if node.kind == Kind::Milestone && !node.milestones.is_empty() {
            return Err(Error::MilestoneInMilestone(node.id.clone()));
        }
        let mut seen = BTreeSet::new();
        for milestone in &node.milestones {
            let target = self
                .nodes
                .get(milestone)
                .ok_or_else(|| Error::DanglingReference { node: node.id.clone(), target: milestone.clone() })?;
            if target.kind != Kind::Milestone {
                return Err(Error::NotAMilestone(milestone.clone()));
            }
            if !seen.insert(milestone) {
                return Err(Error::DuplicateReference { node: node.id.clone(), target: milestone.clone() });
            }
        }
        Ok(())
    }

    // ---- queries -------------------------------------------------------

    /// Nodes that directly depend on `id`.
    pub fn dependents<'a>(&'a self, id: &'a NodeId) -> impl Iterator<Item = &'a Node> + 'a {
        self.nodes.values().filter(move |n| n.depends_on.contains(id))
    }

    /// Tasks that belong to milestone `id`.
    pub fn members<'a>(&'a self, id: &'a NodeId) -> impl Iterator<Item = &'a Node> + 'a {
        self.nodes.values().filter(move |n| n.milestones.contains(id))
    }

    /// What `node` directly requires: its dependencies and, for a milestone, its members.
    pub fn requirements<'a>(&'a self, node: &'a Node) -> Vec<&'a NodeId> {
        node.depends_on.iter().chain(self.members(&node.id).map(|m| &m.id)).collect()
    }

    /// Everything `id` transitively requires, excluding `id` itself.
    pub fn descendants(&self, id: &NodeId) -> BTreeSet<NodeId> {
        let mut seen = BTreeSet::new();
        let mut stack = self.requirements(&self.nodes[id]);
        while let Some(next) = stack.pop() {
            if seen.insert(next.clone()) {
                stack.extend(self.requirements(&self.nodes[next]));
            }
        }
        seen
    }

    /// Everything that transitively requires `id`, excluding `id` itself.
    pub fn ancestors(&self, id: &NodeId) -> BTreeSet<NodeId> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![id];
        while let Some(next) = stack.pop() {
            // A node is required by its dependents and by the milestones it belongs to.
            let requirers = self.dependents(next).map(|n| &n.id).chain(&self.nodes[next].milestones);
            for requirer in requirers {
                if seen.insert(requirer.clone()) {
                    stack.push(requirer);
                }
            }
        }
        seen
    }

    /// Whether `from` transitively requires `to` (or they are the same node).
    pub fn reaches(&self, from: &NodeId, to: &NodeId) -> bool {
        from == to || self.descendants(from).contains(to)
    }

    /// An open node whose requirements are all closed. For a milestone this
    /// means it has been reached.
    pub fn is_ready(&self, node: &Node) -> bool {
        !node.status.is_closed() && self.requirements(node).iter().all(|d| self.nodes[*d].status.is_closed())
    }

    /// Open tasks that can be started now, optionally limited to the work `scope` requires.
    pub fn ready_tasks(&self, scope: Option<&NodeId>) -> Vec<&Node> {
        let in_scope = scope.map(|s| self.descendants(s));
        self.nodes
            .values()
            .filter(|n| n.kind == Kind::Task && self.is_ready(n))
            .filter(|n| in_scope.as_ref().is_none_or(|set| set.contains(&n.id)))
            .collect()
    }

    /// `(closed, total)` member tasks of milestone `id`.
    pub fn progress(&self, id: &NodeId) -> (usize, usize) {
        let members: Vec<&Node> = self.members(id).collect();
        (members.iter().filter(|n| n.status.is_closed()).count(), members.len())
    }

    /// The longest chain of open tasks that `id` requires (inclusive), in
    /// order of execution. Its length is the minimum number of sequential steps left.
    pub fn critical_path(&self, id: &NodeId) -> Vec<NodeId> {
        fn walk(g: &Graph, id: &NodeId, memo: &mut HashMap<NodeId, Vec<NodeId>>) -> Vec<NodeId> {
            if let Some(path) = memo.get(id) {
                return path.clone();
            }
            let node = &g.nodes[id];
            let mut path = g
                .requirements(node)
                .into_iter()
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

    /// Node ids ordered so that every node comes after its requirements.
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
            for dep in g.requirements(&g.nodes[id]) {
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

    /// Nodes that nothing requires (the tops of the graph).
    pub fn roots(&self) -> Vec<&Node> {
        let required: BTreeSet<&NodeId> = self.nodes.values().flat_map(|n| self.requirements(n)).collect();
        self.nodes.values().filter(|n| !required.contains(&n.id)).collect()
    }

    // ---- mutations -----------------------------------------------------

    /// Inserts a new node. Everything it references must already exist.
    pub fn insert(&mut self, node: Node) -> Result<(), Error> {
        if self.nodes.contains_key(&node.id) {
            return Err(Error::DuplicateId(node.id));
        }
        // A new node is required by nothing yet, so it cannot close a cycle.
        self.check_references(&node)?;
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

    /// Adds `task` to the set of `milestone`. Joining twice is a no-op.
    pub fn join(&mut self, task: &NodeId, milestone: &NodeId) -> Result<(), Error> {
        if self.node(milestone)?.kind != Kind::Milestone {
            return Err(Error::NotAMilestone(milestone.clone()));
        }
        let node = self.node(task)?;
        if node.kind == Kind::Milestone {
            return Err(Error::MilestoneInMilestone(task.clone()));
        }
        if node.milestones.contains(milestone) {
            return Ok(());
        }
        if self.reaches(task, milestone) {
            return Err(Error::WouldCycle { from: milestone.clone(), to: task.clone() });
        }
        self.node_mut(task)?.milestones.push(milestone.clone());
        Ok(())
    }

    pub fn leave(&mut self, task: &NodeId, milestone: &NodeId) -> Result<(), Error> {
        let milestones = &mut self.node_mut(task)?.milestones;
        let index = milestones
            .iter()
            .position(|m| m == milestone)
            .ok_or_else(|| Error::NotMember { task: task.clone(), milestone: milestone.clone() })?;
        milestones.remove(index);
        Ok(())
    }

    pub fn set_status(&mut self, id: &NodeId, status: Status) -> Result<(), Error> {
        self.node_mut(id)?.status = status;
        Ok(())
    }

    pub fn edit(&mut self, id: &NodeId, edit: Edit) -> Result<(), Error> {
        if let Some(kind) = edit.kind {
            let node = self.node(id)?;
            let blocked = match kind {
                Kind::Task => self.members(id).next().is_some(),
                Kind::Milestone => !node.milestones.is_empty(),
            };
            if node.kind != kind && blocked {
                return Err(Error::KindChangeBreaksMembership(id.clone()));
            }
        }
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

    /// Removes a node and every reference to it.
    pub fn remove(&mut self, id: &NodeId) -> Result<Node, Error> {
        let node = self.nodes.remove(id).ok_or_else(|| Error::NotFound(id.0.clone()))?;
        for other in self.nodes.values_mut() {
            other.depends_on.retain(|d| d != id);
            other.milestones.retain(|m| m != id);
        }
        Ok(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, kind: Kind, deps: &[&str], milestones: &[&str]) -> Node {
        let mut n = Node::new(NodeId(id.into()), kind, id.into());
        n.depends_on = deps.iter().map(|d| NodeId((*d).into())).collect();
        n.milestones = milestones.iter().map(|m| NodeId((*m).into())).collect();
        n
    }

    fn id(s: &str) -> NodeId {
        NodeId(s.into())
    }

    /// b depends on a; milestone m = {b, c}.
    fn sample() -> Graph {
        Graph::from_nodes([
            node("a", Kind::Task, &[], &[]),
            node("b", Kind::Task, &["a"], &["m"]),
            node("c", Kind::Task, &[], &["m"]),
            node("m", Kind::Milestone, &[], &[]),
        ])
        .unwrap()
    }

    #[test]
    fn rejects_invalid_graphs() {
        let invalid = |nodes: Vec<Node>| Graph::from_nodes(nodes).unwrap_err();
        assert!(matches!(invalid(vec![node("a", Kind::Task, &["x"], &[])]), Error::DanglingReference { .. }));
        assert!(matches!(
            invalid(vec![node("a", Kind::Task, &["b"], &[]), node("b", Kind::Task, &["a"], &[])]),
            Error::Cycle(_)
        ));
        assert!(matches!(invalid(vec![node("a", Kind::Task, &["a"], &[])]), Error::Cycle(_)));
        assert!(matches!(
            invalid(vec![node("a", Kind::Task, &[], &["b"]), node("b", Kind::Task, &[], &[])]),
            Error::NotAMilestone(_)
        ));
        // A member that waits for its own milestone can never be finished.
        assert!(matches!(
            invalid(vec![node("a", Kind::Task, &["m"], &["m"]), node("m", Kind::Milestone, &[], &[])]),
            Error::Cycle(_)
        ));
    }

    #[test]
    fn link_and_join_reject_cycles_and_are_idempotent() {
        let mut g = sample();
        assert!(matches!(g.link(&id("a"), &id("m")), Err(Error::WouldCycle { .. })));
        assert!(matches!(g.link(&id("a"), &id("a")), Err(Error::WouldCycle { .. })));
        g.link(&id("b"), &id("a")).unwrap();
        assert_eq!(g.get(&id("b")).unwrap().depends_on, vec![id("a")]);

        g.join(&id("b"), &id("m")).unwrap();
        assert_eq!(g.get(&id("b")).unwrap().milestones, vec![id("m")]);
        assert!(matches!(g.join(&id("a"), &id("c")), Err(Error::NotAMilestone(_))));
        assert!(matches!(g.join(&id("m"), &id("m")), Err(Error::MilestoneInMilestone(_))));

        g.insert(node("n", Kind::Milestone, &["m"], &[])).unwrap();
        g.link(&id("a"), &id("n")).unwrap_err();
        g.leave(&id("c"), &id("m")).unwrap();
        assert!(matches!(g.leave(&id("c"), &id("m")), Err(Error::NotMember { .. })));
    }

    #[test]
    fn ready_progress_and_critical_path_follow_status() {
        let mut g = sample();
        let ready = |g: &Graph| g.ready_tasks(None).iter().map(|n| n.id.0.clone()).collect::<Vec<_>>();
        assert_eq!(ready(&g), ["a", "c"]);
        assert_eq!(g.progress(&id("m")), (0, 2));
        // `a` is not a member but `b` needs it, so it is on the way to `m`.
        assert_eq!(g.critical_path(&id("m")), [id("a"), id("b")]);
        assert_eq!(g.ready_tasks(Some(&id("m"))).len(), 2);

        g.set_status(&id("a"), Status::Done).unwrap();
        assert_eq!(ready(&g), ["b", "c"]);
        assert_eq!(g.progress(&id("m")), (0, 2));
        assert_eq!(g.critical_path(&id("m")).len(), 1);

        g.set_status(&id("b"), Status::Done).unwrap();
        g.set_status(&id("c"), Status::Dropped).unwrap();
        assert!(g.is_ready(g.get(&id("m")).unwrap()));
    }

    #[test]
    fn milestone_sets_do_not_include_earlier_milestones() {
        let mut g = sample();
        g.insert(node("v2", Kind::Milestone, &["m"], &[])).unwrap();
        g.insert(node("d", Kind::Task, &[], &["v2"])).unwrap();
        assert_eq!(g.progress(&id("v2")), (0, 1));
        assert!(!g.is_ready(g.get(&id("v2")).unwrap()));
        assert!(g.descendants(&id("v2")).contains(&id("b")));
        let all = |ids: &[&str]| ids.iter().map(|s| id(s)).collect::<BTreeSet<_>>();
        assert_eq!(g.ancestors(&id("a")), all(&["b", "m", "v2"]));
        assert_eq!(g.ancestors(&id("d")), all(&["v2"]));
        assert!(g.ancestors(&id("v2")).is_empty());
    }

    #[test]
    fn topo_order_puts_requirements_first() {
        let order = sample().topo_order().unwrap();
        let pos = |s: &str| order.iter().position(|x| x.0 == s).unwrap();
        assert!(pos("a") < pos("b") && pos("b") < pos("m") && pos("c") < pos("m"));
    }

    #[test]
    fn kind_changes_keep_membership_valid() {
        let mut g = sample();
        let to = |kind| Edit { kind: Some(kind), ..Edit::default() };
        assert!(matches!(g.edit(&id("m"), to(Kind::Task)), Err(Error::KindChangeBreaksMembership(_))));
        assert!(matches!(g.edit(&id("b"), to(Kind::Milestone)), Err(Error::KindChangeBreaksMembership(_))));
        g.edit(&id("a"), to(Kind::Milestone)).unwrap();
    }

    #[test]
    fn remove_cleans_references_and_resolve_uses_prefixes() {
        let mut g = sample();
        g.remove(&id("m")).unwrap();
        assert!(g.get(&id("b")).unwrap().milestones.is_empty());
        g.remove(&id("a")).unwrap();
        assert!(g.get(&id("b")).unwrap().depends_on.is_empty());
        assert_eq!(g.roots().len(), 2);

        let g = Graph::from_nodes([node("abc", Kind::Task, &[], &[]), node("abd", Kind::Task, &[], &[])]).unwrap();
        assert_eq!(g.resolve("abc").unwrap(), id("abc"));
        assert!(matches!(g.resolve("ab"), Err(Error::Ambiguous { .. })));
        assert!(matches!(g.resolve("z"), Err(Error::NotFound(_))));
    }
}
