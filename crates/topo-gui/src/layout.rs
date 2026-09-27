//! Layered graph layout: prerequisites on the left, dependents on the right.

use std::collections::BTreeMap;

use topo_core::{Graph, NodeId};

/// Grid cell of a node: `(column, row)`.
pub type Cell = (usize, usize);

/// Column = length of the longest dependency chain below the node, so every
/// edge points rightwards. Rows within a column are ordered by the mean row of
/// the node's dependencies, which keeps most edges short and uncrossed.
pub fn layout(graph: &Graph) -> BTreeMap<NodeId, Cell> {
    let order = graph.topo_order().expect("workspace graphs are acyclic");
    let mut column: BTreeMap<&NodeId, usize> = BTreeMap::new();
    for id in &order {
        let deps = &graph.get(id).expect("ids come from the graph").depends_on;
        column.insert(id, deps.iter().map(|d| column[d] + 1).max().unwrap_or(0));
    }

    let mut cells: BTreeMap<NodeId, Cell> = BTreeMap::new();
    let columns = column.values().max().map_or(0, |max| max + 1);
    for col in 0..columns {
        let mut members: Vec<(f32, &NodeId)> = order
            .iter()
            .filter(|id| column[id] == col)
            .map(|id| {
                let deps = &graph.get(id).expect("ids come from the graph").depends_on;
                let mean = match deps.is_empty() {
                    true => 0.0,
                    false => deps.iter().map(|d| cells[d].1 as f32).sum::<f32>() / deps.len() as f32,
                };
                (mean, id)
            })
            .collect();
        members.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (row, (_, id)) in members.into_iter().enumerate() {
            cells.insert(id.clone(), (col, row));
        }
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use topo_core::{Kind, Node};

    #[test]
    fn edges_point_to_later_columns() {
        let mut graph = Graph::default();
        for (id, deps) in [("a", vec![]), ("b", vec!["a"]), ("c", vec![]), ("m", vec!["b", "c"])] {
            let mut node = Node::new(NodeId(id.into()), Kind::Task, id.into());
            node.depends_on = deps.into_iter().map(|d| NodeId(d.into())).collect();
            graph.insert(node).unwrap();
        }
        let cells = layout(&graph);
        let cell = |s: &str| cells[&NodeId(s.into())];
        assert_eq!(cell("a").0, 0);
        assert_eq!(cell("c").0, 0);
        assert_eq!(cell("b").0, 1);
        assert_eq!(cell("m"), (2, 0));
    }
}
