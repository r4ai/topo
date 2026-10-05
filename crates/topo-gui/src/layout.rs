//! Layered graph layout: prerequisites on the left, dependents on the right.

use std::collections::{BTreeMap, BTreeSet};

use topo_core::{Graph, Node, NodeId};

/// Grid cell of a node: `(column, row)`.
pub type Cell = (usize, usize);

/// A tag group. Tags are sets: a node belongs to the group of every tag it
/// has, and to `Untagged` only when it has none.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Tag(String),
    Untagged,
}

impl Group {
    pub fn label(&self) -> String {
        match self {
            Group::Tag(tag) => format!("#{tag}"),
            Group::Untagged => "untagged".to_owned(),
        }
    }

    fn of(node: &Node) -> BTreeSet<Group> {
        match node.tags.is_empty() {
            true => BTreeSet::from([Group::Untagged]),
            false => node.tags.iter().cloned().map(Group::Tag).collect(),
        }
    }
}

/// Which nodes the canvas shows and how it arranges them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct View {
    /// Hide every closed node (done or dropped) together with its edges.
    pub hide_completed: bool,
    /// Lay the nodes out in one band per tag group instead of one graph.
    pub group_by_tag: bool,
    pub collapsed: BTreeSet<Group>,
}

/// A node card placed in the band `band` (always 0 without grouping).
pub struct Slot {
    pub id: NodeId,
    pub cell: Cell,
    pub band: usize,
}

/// A group header; the group's cards follow in the rows below it.
pub struct Band {
    pub group: Group,
    pub row: usize,
    pub count: usize,
}

#[derive(Default)]
pub struct Layout {
    pub slots: Vec<Slot>,
    pub bands: Vec<Band>,
}

/// Column = length of the longest requirement chain below the node among the
/// shown nodes, so every edge points rightwards. Hidden nodes are not
/// bridged: their dependents simply lose the edge. Rows within a column are
/// ordered by the mean row of the node's dependencies, which keeps most edges
/// short and uncrossed. With grouping, each group is a band of rows under its
/// header and a node is placed once in every band of its groups.
pub fn layout(graph: &Graph, view: &View) -> Layout {
    let topo = graph.topo_order().expect("workspace graphs are acyclic");
    let order: Vec<&NodeId> = topo
        .iter()
        .filter(|id| !(view.hide_completed && graph.get(id).is_some_and(|n| n.status.is_closed())))
        .collect();
    let shown: BTreeSet<&NodeId> = order.iter().copied().collect();
    // Index memberships once. Calling graph.requirements for every milestone
    // scans all nodes, twice per layout and again for every tag band.
    let mut requirements: BTreeMap<&NodeId, Vec<&NodeId>> = order
        .iter()
        .map(|id| {
            let node = graph.get(id).expect("ids come from the graph");
            (*id, node.depends_on.iter().filter(|d| shown.contains(d)).collect())
        })
        .collect();
    for id in &order {
        for milestone in &graph.get(id).expect("ids come from the graph").milestones {
            if let Some(members) = requirements.get_mut(milestone) {
                members.push(id);
            }
        }
    }
    let requirements = |id: &NodeId| requirements[id].clone();
    let mut column: BTreeMap<&NodeId, usize> = BTreeMap::new();
    for id in &order {
        column.insert(id, requirements(id).iter().map(|d| column[d] + 1).max().unwrap_or(0));
    }

    if !view.group_by_tag {
        let (placed, _) = place(&order, &column, &requirements);
        return Layout {
            slots: placed.into_iter().map(|(id, cell)| Slot { id, cell, band: 0 }).collect(),
            bands: vec![],
        };
    }
    let mut groups: BTreeMap<Group, Vec<&NodeId>> = BTreeMap::new();
    for id in &order {
        for group in Group::of(graph.get(id).expect("ids come from the graph")) {
            groups.entry(group).or_default().push(id);
        }
    }
    let mut result = Layout::default();
    let mut header = 0;
    for (group, members) in groups {
        let band = result.bands.len();
        let collapsed = view.collapsed.contains(&group);
        let mut rows = 0;
        if !collapsed {
            let (placed, used) = place(&members, &column, &requirements);
            rows = used;
            let slots = placed.into_iter().map(|(id, (col, row))| Slot { id, cell: (col, header + 1 + row), band });
            result.slots.extend(slots);
        }
        result.bands.push(Band { group, row: header, count: members.len() });
        header += 1 + rows;
    }
    result
}

/// Places `members` (in topological order) into columns, rows counted from 0.
/// Returns the cells and the number of rows used.
fn place<'a>(
    members: &[&'a NodeId],
    column: &BTreeMap<&NodeId, usize>,
    requirements: &impl Fn(&NodeId) -> Vec<&'a NodeId>,
) -> (Vec<(NodeId, Cell)>, usize) {
    let mut cells: BTreeMap<&NodeId, Cell> = BTreeMap::new();
    let columns = members.iter().map(|id| column[*id] + 1).max().unwrap_or(0);
    let mut grouped = vec![Vec::new(); columns];
    for id in members {
        grouped[column[*id]].push(*id);
    }
    for (col, group) in grouped.into_iter().enumerate() {
        let mut ranked: Vec<(f32, &NodeId)> = group
            .into_iter()
            .map(|id| {
                // Only requirements in this band have a row to align with.
                let rows: Vec<usize> = requirements(id).iter().filter_map(|d| cells.get(d)).map(|c| c.1).collect();
                let mean = match rows.is_empty() {
                    true => 0.0,
                    false => rows.iter().sum::<usize>() as f32 / rows.len() as f32,
                };
                (mean, id)
            })
            .collect();
        ranked.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (row, (_, id)) in ranked.into_iter().enumerate() {
            cells.insert(id, (col, row));
        }
    }
    let rows = cells.values().map(|c| c.1 + 1).max().unwrap_or(0);
    (members.iter().map(|id| ((*id).clone(), cells[*id])).collect(), rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use topo_core::{Kind, Status};

    fn graph(nodes: &[(&str, &[&str], &[&str], Status)]) -> Graph {
        let mut graph = Graph::default();
        for (id, deps, tags, status) in nodes {
            let mut node = Node::new(NodeId((*id).into()), Kind::Task, (*id).into());
            node.depends_on = deps.iter().map(|d| NodeId((*d).into())).collect();
            node.tags = tags.iter().map(|t| (*t).to_owned()).collect();
            node.status = *status;
            graph.insert(node).unwrap();
        }
        graph
    }

    fn cell(layout: &Layout, id: &str, band: usize) -> Cell {
        layout.slots.iter().find(|s| s.id.as_str() == id && s.band == band).expect("placed").cell
    }

    #[test]
    fn edges_point_to_later_columns() {
        let mut graph = Graph::default();
        for (id, deps) in [("a", vec![]), ("b", vec!["a"]), ("c", vec![]), ("m", vec!["b", "c"])] {
            let mut node = Node::new(NodeId(id.into()), Kind::Task, id.into());
            node.depends_on = deps.into_iter().map(|d| NodeId(d.into())).collect();
            graph.insert(node).unwrap();
        }
        let layout = layout(&graph, &View::default());
        assert!(layout.bands.is_empty());
        assert_eq!(cell(&layout, "a", 0).0, 0);
        assert_eq!(cell(&layout, "c", 0).0, 0);
        assert_eq!(cell(&layout, "b", 0).0, 1);
        assert_eq!(cell(&layout, "m", 0), (2, 0));
    }

    #[test]
    fn hidden_nodes_are_neither_placed_nor_bridged() {
        let graph = graph(&[
            ("a", &[], &[], Status::Done),
            ("b", &["a"], &[], Status::Todo),
            ("c", &["b"], &[], Status::Todo),
            ("d", &[], &[], Status::Dropped),
        ]);
        let view = View { hide_completed: true, ..View::default() };
        let layout = layout(&graph, &view);
        let ids: Vec<&str> = layout.slots.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["b", "c"]);
        // b lost its only requirement, so it starts the graph.
        assert_eq!(cell(&layout, "b", 0), (0, 0));
        assert_eq!(cell(&layout, "c", 0), (1, 0));
    }

    #[test]
    fn a_node_is_placed_in_every_group_of_its_tags() {
        let graph = graph(&[
            ("a", &[], &["ui", "core"], Status::Todo),
            ("b", &["a"], &["ui"], Status::Todo),
            ("c", &[], &[], Status::Todo),
        ]);
        let view = View { group_by_tag: true, ..View::default() };
        let layout = layout(&graph, &view);
        let groups: Vec<(&Group, usize)> = layout.bands.iter().map(|b| (&b.group, b.row)).collect();
        // core: header 0, a at row 1. ui: header 2, a at row 3, b right of a. untagged last.
        assert_eq!(groups, [(&Group::Tag("core".into()), 0), (&Group::Tag("ui".into()), 2), (&Group::Untagged, 4)]);
        assert_eq!(cell(&layout, "a", 0), (0, 1));
        assert_eq!(cell(&layout, "a", 1), (0, 3));
        assert_eq!(cell(&layout, "b", 1), (1, 3));
        assert_eq!(cell(&layout, "c", 2), (0, 5));
        assert_eq!(layout.slots.len(), 4);
    }

    #[test]
    fn a_collapsed_group_keeps_its_header_and_hides_its_cards() {
        let graph = graph(&[("a", &[], &["x"], Status::Todo), ("b", &[], &["y"], Status::Todo)]);
        let view = View { group_by_tag: true, collapsed: BTreeSet::from([Group::Tag("x".into())]), ..View::default() };
        let layout = layout(&graph, &view);
        assert_eq!(layout.bands.iter().map(|b| (b.row, b.count)).collect::<Vec<_>>(), [(0, 1), (1, 1)]);
        assert_eq!(layout.slots.len(), 1);
        assert_eq!(cell(&layout, "b", 1), (0, 2));
    }
}
