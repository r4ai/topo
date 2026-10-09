//! The canvas: dot grid, edges, node cards and the gestures on them.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::RangeInclusive;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, Context, CursorStyle, ElementId, Hsla, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PathBuilder, Pixels, Point, Rgba, ScrollDelta, ScrollWheelEvent, Transformation, Window, canvas, div,
    fill, point, prelude::*, px, radians, size,
};
use jiff::civil::Date;
use topo_core::{Graph, Kind, Node, NodeId, Status};

use crate::animation::{Pivot, Preset, preset};
use crate::inline::Field;
use crate::layout::Band;
use crate::theme::{Theme, metrics};
use crate::ui;
use crate::{Drag, NODE_H, NODE_W, TopoApp, animation, dates, layout, theme};

/// How a card is emphasized in the current frame.
struct CardState {
    selected: bool,
    hovered: bool,
    dimmed: bool,
    critical: bool,
    /// The highlighted entry of the open list is this node.
    cursor: bool,
    /// `Some(allowed)` while a link is being dragged over this card.
    drop: Option<bool>,
    /// The open requirements of the node.
    open_requirements: usize,
    /// The closed and all members of a milestone.
    progress: (usize, usize),
}

impl CardState {
    /// The opacity the card of a `closed` node rests at.
    fn opacity(&self, closed: bool) -> f32 {
        match () {
            _ if self.dimmed => 0.22,
            _ if closed && !self.selected => 0.6,
            _ => 1.0,
        }
    }
}

/// A card as its springs have it in the current frame.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CardMotion {
    /// The top-left corner, in canvas units.
    pub(crate) at: [f32; 2],
    /// The scale of the whole card: below 1 while it enters or leaves.
    pub(crate) scale: f32,
    pub(crate) opacity: f32,
    /// How far the border and shadow of a selected card have faded in.
    pub(crate) selected: f32,
    /// How far the colours of a done card have faded in.
    pub(crate) done: f32,
    /// The scale of the status glyph, which pops when the card is completed.
    pub(crate) check: f32,
}

/// Runs the springs of the card `key`. `place` is `[x, y, shrink]` in canvas units and moves
/// on `reflow`; `fade` is `[opacity, selected, done]`. A card whose springs are new starts at
/// `from` instead of at rest.
fn card_motion(
    window: &mut Window,
    cx: &App,
    key: ElementId,
    from: Option<([f32; 3], [f32; 3])>,
    place: [f32; 3],
    fade: [f32; 3],
    reflow: Option<Preset>,
) -> CardMotion {
    window.with_id(key, |window| {
        let start = from.map(|(place, _)| place);
        let [x, y, shrink] = animation::springs_within(window, cx, "place", start, place, reflow, PLACE_REST);
        let start = from.map(|(_, fade)| fade);
        let [opacity, selected, done] = animation::springs(window, cx, "fade", start, fade, Some(preset::FADE));
        // Completing pops the glyph; taking it back only lets the new one grow.
        let completed = fade[2] == 1.;
        let pop = if completed { preset::BOUNCY } else { preset::SMOOTH };
        let [check] = animation::springs(window, cx, "check", None, [fade[2]], Some(pop));
        let check = 1. - 0.4 * if completed { 1. - check } else { check };
        CardMotion { at: [x, y], scale: 1. - shrink / NODE_W, opacity, selected, done, check }
    })
}

/// `element` drawn at `scale` about its centre. At rest it is the element itself.
fn scaled(element: impl IntoElement, scale: f32) -> AnyElement {
    match scale == 1. {
        true => element.into_any_element(),
        false => animation::surface(element).scale(scale, Pivot::Center).into_any_element(),
    }
}

/// The canvas point of the top-left corner of `cell`.
fn spot(cell: layout::Cell) -> [f32; 2] {
    [cell.0 as f32 * crate::CELL_W, cell.1 as f32 * crate::CELL_H]
}

/// The point `left` of the way back from `at` to `from`.
fn back(at: [f32; 2], from: [f32; 2], left: f32) -> [f32; 2] {
    [at[0] + (from[0] - at[0]) * left, at[1] + (from[1] - at[1]) * left]
}

/// `from` crossfaded to `to`. The ends are exact, so a card at rest is drawn in its own colours.
fn mix(from: Hsla, to: Hsla, t: f32) -> Hsla {
    match t {
        t if t <= 0. => from,
        t if t >= 1. => to,
        t => animation::lerp(from.into(), to.into(), t).into(),
    }
}

/// The most cards a layout change moves on springs; more of them snap.
const REFLOW_CARDS: usize = 300;
/// The most cards drawn leaving the canvas.
const GHOSTS: usize = 40;
/// How close a card comes to its cell before it rests, in canvas units.
const PLACE_REST: f32 = 0.1;
/// How much narrower a card is drawn as it enters or leaves, in canvas units: a scale of 0.9.
const SHRINK: f32 = NODE_W * 0.1;

/// A card leaving the canvas: what it showed, and where.
#[derive(Clone)]
struct Ghost {
    node: Node,
    cell: layout::Cell,
    open_requirements: usize,
    progress: (usize, usize),
}

/// The layout change the cards are still moving through.
#[derive(Default)]
struct Reflow {
    /// Counts the layout changes, so that each one starts its own progress.
    serial: usize,
    /// Where each card was before the change; empty once the cards have arrived.
    previous: BTreeMap<NodeId, layout::Cell>,
    /// The most columns and rows any card is away from its cell.
    reach: layout::Cell,
    /// The cards of the nodes the last edit removed from the graph.
    departed: Vec<Ghost>,
    /// The cards that left with the change and are drawn fading out.
    ghosts: Vec<Ghost>,
}

impl Reflow {
    /// Ends the change: every card is where the layout has it.
    fn settle(&mut self) {
        self.previous.clear();
        self.reach = (0, 0);
        self.departed.clear();
        self.ghosts.clear();
    }
}

/// A drawn edge with the cells it joins; a node shown in several groups has one per group.
struct PlacedEdge {
    from: NodeId,
    to: NodeId,
    membership: bool,
    from_cell: layout::Cell,
    to_cell: layout::Cell,
}

struct EdgePaint {
    from: Point<Pixels>,
    to: Point<Pixels>,
    color: Hsla,
    width: f32,
    dashed: bool,
}

/// Traverse each direction independently: changing direction at a shared requirement
/// would include unrelated siblings in the selection's dependency/dependent chains.
fn reachable(adjacency: &BTreeMap<NodeId, Vec<NodeId>>, seeds: &BTreeSet<NodeId>) -> BTreeSet<NodeId> {
    let mut visited = BTreeSet::new();
    let mut pending: Vec<_> = seeds.iter().cloned().collect();
    while let Some(id) = pending.pop() {
        let Some(neighbors) = adjacency.get(&id) else { continue };
        if !visited.insert(id.clone()) {
            continue;
        }
        pending.extend(neighbors.iter().filter(|id| !visited.contains(*id)).cloned());
    }
    visited
}

/// Derived canvas data. Camera and hover updates reuse it; structural/status edits invalidate it.
pub(crate) struct GraphCache {
    dirty: bool,
    layout_dirty: bool,
    columns: BTreeMap<usize, BTreeMap<usize, NodeId>>,
    edge_tiles: BTreeMap<(i32, i32), Vec<usize>>,
    long_edges: Vec<usize>,
    edges: Vec<PlacedEdge>,
    /// Every card of each node with its band; only kept while grouped.
    placements: BTreeMap<NodeId, Vec<(usize, layout::Cell)>>,
    /// Nodes referencing each node, by dependency (`false`) or milestone membership (`true`); only kept while grouped.
    referrers: BTreeMap<NodeId, Vec<(NodeId, bool)>>,
    /// The selection's edges whose ends share no band, drawn in addition to `edges`.
    cross: Vec<PlacedEdge>,
    /// The card the selection was last clicked or navigated on.
    active: Option<layout::Cell>,
    view: layout::View,
    bands: Vec<layout::Band>,
    /// Rightmost column and bottom row, group headers included.
    extent: layout::Cell,
    pub(crate) ready: Rc<Vec<NodeId>>,
    pub(crate) overdue: Rc<Vec<NodeId>>,
    today: Option<Date>,
    pub(crate) doing: Rc<Vec<NodeId>>,
    pub(crate) overview: crate::inspector::Overview,
    overview_dirty: bool,
    pub(crate) task_count: usize,
    pub(crate) closed_count: usize,
    pub(crate) critical_paths: BTreeMap<NodeId, Vec<NodeId>>,
    pub(crate) critical_lengths: BTreeMap<NodeId, usize>,
    path_next: BTreeMap<NodeId, Option<NodeId>>,
    cells: BTreeMap<NodeId, layout::Cell>,
    reflow: Reflow,
    /// The cards of the last frame, ghosts included, as they were drawn.
    #[cfg(test)]
    pub(crate) drawn: std::cell::RefCell<BTreeMap<NodeId, CardMotion>>,
    order: Vec<NodeId>,
    selected: BTreeSet<NodeId>,
    pub(crate) selected_ids: Vec<NodeId>,
    pub(crate) selected_status: Option<Status>,
    pub(crate) selection_scroll: gpui::UniformListScrollHandle,
    focus: Option<BTreeSet<NodeId>>,
    critical: BTreeSet<NodeId>,
    critical_edges: BTreeMap<NodeId, BTreeSet<NodeId>>,
    requirements: BTreeMap<NodeId, Vec<NodeId>>,
    dependents: BTreeMap<NodeId, Vec<NodeId>>,
    open_requirements: BTreeMap<NodeId, usize>,
    pub(crate) progress: BTreeMap<NodeId, (usize, usize)>,
}

impl Default for GraphCache {
    fn default() -> Self {
        Self {
            dirty: true,
            layout_dirty: true,
            columns: BTreeMap::new(),
            edge_tiles: BTreeMap::new(),
            long_edges: Vec::new(),
            edges: Vec::new(),
            placements: BTreeMap::new(),
            referrers: BTreeMap::new(),
            cross: Vec::new(),
            active: None,
            view: layout::View::default(),
            bands: Vec::new(),
            extent: (0, 0),
            ready: Rc::default(),
            overdue: Rc::default(),
            today: None,
            doing: Rc::default(),
            overview: crate::inspector::Overview::default(),
            overview_dirty: true,
            task_count: 0,
            closed_count: 0,
            critical_paths: BTreeMap::new(),
            critical_lengths: BTreeMap::new(),
            path_next: BTreeMap::new(),
            cells: BTreeMap::new(),
            reflow: Reflow::default(),
            #[cfg(test)]
            drawn: Default::default(),
            order: Vec::new(),
            selected: BTreeSet::new(),
            selected_ids: Vec::new(),
            selected_status: None,
            selection_scroll: gpui::UniformListScrollHandle::new(),
            focus: None,
            critical: BTreeSet::new(),
            critical_edges: BTreeMap::new(),
            requirements: BTreeMap::new(),
            dependents: BTreeMap::new(),
            open_requirements: BTreeMap::new(),
            progress: BTreeMap::new(),
        }
    }
}

impl GraphCache {
    #[cfg(test)]
    pub(crate) fn invalidate(&mut self) {
        self.dirty = true;
        self.layout_dirty = true;
    }

    /// Switches what the canvas shows; the layout is rebuilt once, not per frame.
    pub(crate) fn set_view(&mut self, view: &layout::View) {
        if self.view != *view {
            self.view.clone_from(view);
            self.dirty = true;
            self.layout_dirty = true;
        }
    }

    /// Classify changes at the mutation boundary; metadata edits keep layout and metrics.
    pub(crate) fn changed(&mut self, before: &Graph, after: &Graph) {
        // Overview order and variable row heights also depend on metadata.
        self.overview_dirty = true;
        let structure = before.nodes().count() != after.nodes().count()
            || after.nodes().any(|n| {
                before.get(&n.id).is_none_or(|old| old.depends_on != n.depends_on || old.milestones != n.milestones)
            });
        let metrics = structure
            || after.nodes().any(|n| before.get(&n.id).is_none_or(|old| old.status != n.status || old.kind != n.kind));
        // Closing a node moves cards only while closed nodes are hidden; tags only while grouped.
        let reshaped = after.nodes().any(|n| {
            before.get(&n.id).is_some_and(|old| {
                (self.view.hide_completed && old.status.is_closed() != n.status.is_closed())
                    || (self.view.group_by_tag && old.tags != n.tags)
            })
        });
        if structure {
            // The graph forgets a removed node, and its card still has to be drawn leaving.
            let removed = before.nodes().filter(|n| after.get(&n.id).is_none());
            let departed = removed
                .filter_map(|n| {
                    Some(Ghost {
                        cell: *self.cells.get(&n.id)?,
                        open_requirements: self.open_requirements[&n.id],
                        progress: self.progress.get(&n.id).copied().unwrap_or_default(),
                        node: n.clone(),
                    })
                })
                .collect::<Vec<_>>();
            self.reflow.departed.extend(departed);
        }
        self.layout_dirty |= structure || reshaped;
        self.dirty |= metrics || reshaped;
        if structure
            || after.nodes().any(|n| before.get(&n.id).is_none_or(|old| old.status != n.status || old.due != n.due))
        {
            self.today = None;
        }
    }

    fn index_layout(&mut self, graph: &Graph, slots: &[layout::Slot]) {
        self.columns.clear();
        let mut placed: BTreeMap<&NodeId, Vec<(usize, layout::Cell)>> = BTreeMap::new();
        for slot in slots {
            self.columns.entry(slot.cell.0).or_default().insert(slot.cell.1, slot.id.clone());
            placed.entry(&slot.id).or_default().push((slot.band, slot.cell));
        }
        // An edge is drawn inside each band that shows both ends; edges to hidden nodes are dropped.
        self.edges.clear();
        for n in graph.nodes() {
            let links =
                n.depends_on.iter().map(|d| (d, &n.id, false)).chain(n.milestones.iter().map(|m| (&n.id, m, true)));
            for (from, to, membership) in links {
                for &(band, from_cell) in placed.get(from).into_iter().flatten() {
                    let target = placed.get(to).and_then(|cells| cells.iter().find(|(b, _)| *b == band));
                    if let Some(&(_, to_cell)) = target {
                        self.edges.push(PlacedEdge {
                            from: from.clone(),
                            to: to.clone(),
                            membership,
                            from_cell,
                            to_cell,
                        });
                    }
                }
            }
        }
        self.placements.clear();
        if self.view.group_by_tag {
            self.placements = placed.into_iter().map(|(id, cells)| (id.clone(), cells)).collect();
        }
        self.edge_tiles.clear();
        self.long_edges.clear();
        for (i, PlacedEdge { from_cell: (fc, fr), to_cell: (tc, tr), .. }) in self.edges.iter().enumerate() {
            let (fc, fr, tc, tr) = (*fc, *fr, *tc, *tr);
            // Covers the control hull, arrowhead and stroke at every supported zoom.
            let x0 = ((fc as f32 * crate::CELL_W + NODE_W - 48.) / TILE).floor() as i32;
            let x1 = ((tc as f32 * crate::CELL_W + 48.) / TILE).floor() as i32;
            let y0 = ((fr.min(tr) as f32 * crate::CELL_H - 32.) / TILE).floor() as i32;
            let y1 = ((fr.max(tr) as f32 * crate::CELL_H + NODE_H + 32.) / TILE).floor() as i32;
            if (x1 - x0 + 1) as i64 * (y1 - y0 + 1) as i64 > 256 {
                self.long_edges.push(i);
            } else {
                for x in x0..=x1 {
                    for y in y0..=y1 {
                        self.edge_tiles.entry((x, y)).or_default().push(i);
                    }
                }
            }
        }
    }

    /// The columns and rows whose cards can be in the viewport; `None` when none can.
    fn visible_cells(
        offset: Point<Pixels>,
        zoom: f32,
        viewport: Bounds<Pixels>,
    ) -> Option<(RangeInclusive<usize>, RangeInclusive<usize>)> {
        let left = (-f32::from(offset.x) / zoom - NODE_W - 8.).max(0.) / crate::CELL_W;
        let right = ((f32::from(viewport.size.width - offset.x)) / zoom).max(0.) / crate::CELL_W;
        let top = (-f32::from(offset.y) / zoom - NODE_H).max(0.) / crate::CELL_H;
        let bottom = (f32::from(viewport.size.height - offset.y) / zoom).max(0.) / crate::CELL_H;
        (left.ceil() <= right.floor() && top.ceil() <= bottom.floor())
            .then(|| (left.ceil() as usize..=right.floor() as usize, top.ceil() as usize..=bottom.floor() as usize))
    }

    fn visible_nodes(&self, offset: Point<Pixels>, zoom: f32, viewport: Bounds<Pixels>) -> Vec<(NodeId, layout::Cell)> {
        if viewport.size.width <= px(0.) {
            return self
                .columns
                .iter()
                .flat_map(|(col, rows)| rows.iter().map(|(row, id)| (id.clone(), (*col, *row))))
                .collect();
        }
        let Some((columns, rows)) = Self::visible_cells(offset, zoom, viewport) else { return Vec::new() };
        self.columns
            .range(columns)
            .flat_map(|(col, placed)| placed.range(rows.clone()).map(|(row, id)| (id.clone(), (*col, *row))))
            .collect()
    }

    /// How much of the layout change in flight is left: 1 as it starts, 0 once the cards
    /// have arrived. Call it every frame, from where [`animation::springs`] may be called.
    fn reflow_progress(&mut self, window: &mut Window, cx: &App) -> f32 {
        let left = match self.reflow.previous.is_empty() {
            true => 0.,
            false => {
                let id = ("reflow", self.reflow.serial);
                animation::springs(window, cx, id, Some([1.]), [0.], Some(preset::SMOOTH))[0]
            }
        };
        if left == 0. {
            self.reflow.settle();
        }
        left
    }

    /// Picks the cards that left with the layout change and are drawn fading out: those
    /// whose cell was in the viewport, and no more than [`GHOSTS`].
    fn leave(&mut self, graph: &Graph, offset: Point<Pixels>, zoom: f32, viewport: Bounds<Pixels>) {
        let Reflow { previous, departed, ghosts, .. } = &mut self.reflow;
        let Some((columns, rows)) = Self::visible_cells(offset, zoom, viewport) else { return ghosts.clear() };
        let left = previous.iter().filter(|(id, (column, row))| {
            !self.cells.contains_key(*id) && columns.contains(column) && rows.contains(row)
        });
        *ghosts = left
            .take(GHOSTS)
            .filter_map(|(id, cell)| match graph.get(id) {
                // A node hidden by the view is still in the graph.
                Some(node) => Some(Ghost {
                    node: node.clone(),
                    cell: *cell,
                    open_requirements: self.open_requirements[id],
                    progress: self.progress.get(id).copied().unwrap_or_default(),
                }),
                // One removed without `changed` hearing of it leaves at once.
                None => departed.iter().find(|ghost| ghost.node.id == *id).cloned(),
            })
            .collect();
    }

    /// Whether cards are still moving through a layout change.
    #[cfg(test)]
    pub(crate) fn reflowing(&self) -> bool {
        !self.reflow.previous.is_empty()
    }

    /// The nodes whose cards are drawn leaving.
    #[cfg(test)]
    pub(crate) fn ghosts(&self) -> Vec<&str> {
        self.reflow.ghosts.iter().map(|ghost| ghost.node.id.as_str()).collect()
    }

    fn visible_edges(&self, offset: Point<Pixels>, zoom: f32, viewport: Bounds<Pixels>) -> BTreeSet<usize> {
        if viewport.size.width <= px(0.) {
            return (0..self.edges.len()).collect();
        }
        let x0 = (-f32::from(offset.x) / zoom / TILE).floor() as i32;
        let y0 = (-f32::from(offset.y) / zoom / TILE).floor() as i32;
        let x1 = (f32::from(viewport.size.width - offset.x) / zoom / TILE).floor() as i32;
        let y1 = (f32::from(viewport.size.height - offset.y) / zoom / TILE).floor() as i32;
        let mut result: BTreeSet<_> = self.long_edges.iter().copied().collect();
        for x in x0..=x1 {
            for y in y0..=y1 {
                if let Some(edges) = self.edge_tiles.get(&(x, y)) {
                    result.extend(edges);
                }
            }
        }
        result
    }

    /// Where each shown node is; one cell per node, its first group's with grouping.
    pub(crate) fn cells(&self) -> &BTreeMap<NodeId, layout::Cell> {
        &self.cells
    }

    #[cfg(test)]
    pub(crate) fn placements(&self, id: &NodeId) -> usize {
        self.columns.values().flat_map(|rows| rows.values()).filter(|placed| *placed == id).count()
    }

    /// The cells of every card of `id`, one per band while grouped.
    #[cfg(test)]
    pub(crate) fn cards(&self, id: &NodeId) -> Vec<layout::Cell> {
        self.placements.get(id).into_iter().flatten().map(|(_, cell)| *cell).collect()
    }

    /// The selection's cross-band edges as `(from, to, from_cell, to_cell)`.
    #[cfg(test)]
    pub(crate) fn cross_edges(&self) -> Vec<(&str, &str, bool, layout::Cell, layout::Cell)> {
        self.cross.iter().map(|e| (e.from.as_str(), e.to.as_str(), e.membership, e.from_cell, e.to_cell)).collect()
    }

    /// The selection's edges that no band draws because their ends share none, each joined
    /// between one card per end: the selected end's active card, the other end's nearest.
    fn cross_band_edges(&self, graph: &Graph) -> Vec<PlacedEdge> {
        let mut seen = BTreeSet::new();
        let mut edges = Vec::new();
        for id in &self.selected {
            let Some(node) = graph.get(id) else { continue };
            let referrers = self.referrers.get(id).into_iter().flatten();
            let links = node
                .depends_on
                .iter()
                .map(|d| (d, id, false))
                .chain(node.milestones.iter().map(|m| (id, m, true)))
                .chain(referrers.map(|(n, membership)| if *membership { (n, id, true) } else { (id, n, false) }));
            for (from, to, membership) in links {
                let (Some(a), Some(b)) = (self.placements.get(from), self.placements.get(to)) else { continue };
                if a.iter().any(|(band, _)| b.iter().any(|(other, _)| other == band))
                    || !seen.insert((from, to, membership))
                {
                    continue;
                }
                let active = |id: &NodeId, cells: &[(usize, layout::Cell)]| {
                    self.active.filter(|cell| self.at(*cell) == Some(id)).unwrap_or(cells[0].1)
                };
                let nearest = |cells: &[(usize, layout::Cell)], to: layout::Cell| {
                    cells.iter().map(|(_, cell)| *cell).min_by_key(|cell| cell.1.abs_diff(to.1)).expect("placed")
                };
                let (from_cell, to_cell) = match (self.selected.contains(from), self.selected.contains(to)) {
                    (true, false) => (active(from, a), nearest(b, active(from, a))),
                    (false, true) => (nearest(a, active(to, b)), active(to, b)),
                    _ => (active(from, a), active(to, b)),
                };
                edges.push(PlacedEdge { from: from.clone(), to: to.clone(), membership, from_cell, to_cell });
            }
        }
        edges
    }

    #[cfg(test)]
    pub(crate) fn edge_count(&self) -> usize {
        self.edges.len()
    }

    #[cfg(test)]
    pub(crate) fn group_labels(&self) -> Vec<String> {
        self.bands.iter().map(|band| band.group.label()).collect()
    }

    #[cfg(test)]
    pub(crate) fn group_counts(&self) -> Vec<usize> {
        self.bands.iter().map(|band| band.count).collect()
    }

    /// The node drawn in `cell`; with grouping a node has a card in each of its groups.
    pub(crate) fn at(&self, cell: layout::Cell) -> Option<&NodeId> {
        self.columns.get(&cell.0)?.get(&cell.1)
    }

    /// The nearest card above (`up`) or below `cell` in its column, skipping group headers.
    pub(crate) fn neighbour(&self, cell: layout::Cell, up: bool) -> Option<(layout::Cell, &NodeId)> {
        let rows = self.columns.get(&cell.0)?;
        let found = match up {
            true => rows.range(..cell.1).next_back(),
            false => rows.range(cell.1 + 1..).next(),
        };
        found.map(|(row, id)| ((cell.0, *row), id))
    }

    pub(crate) fn extent(&self) -> layout::Cell {
        self.extent
    }

    pub(crate) fn refresh(&mut self, graph: &Graph, selected: &BTreeSet<NodeId>, active: Option<layout::Cell>) {
        let unchanged = !self.dirty;
        let reselected = self.selected != *selected || self.active != active;
        if !unchanged {
            self.dirty = false;
            if self.layout_dirty {
                self.order = graph.topo_order().expect("graph invariant");
                let layout = layout::layout(graph, &self.view);
                let previous = std::mem::take(&mut self.cells);
                for slot in &layout.slots {
                    self.cells.entry(slot.id.clone()).or_insert(slot.cell);
                }
                // The first layout has nothing to move from, and an unchanged one nothing to move.
                if !previous.is_empty() && previous != self.cells {
                    // A card still on its way from an earlier change is at most that much further off.
                    let (columns, rows) = self.reflow.reach;
                    let moves = self.cells.iter().filter_map(|(id, cell)| Some((cell, previous.get(id)?)));
                    self.reflow.reach = moves.fold((columns, rows), |reach, (cell, old)| {
                        (reach.0.max(columns + cell.0.abs_diff(old.0)), reach.1.max(rows + cell.1.abs_diff(old.1)))
                    });
                    self.reflow.serial += 1;
                    self.reflow.previous = previous;
                }
                self.extent = layout
                    .slots
                    .iter()
                    .map(|slot| slot.cell)
                    .chain(layout.bands.iter().map(|band| (0, band.row)))
                    .fold((0, 0), |(col, row), cell| (col.max(cell.0), row.max(cell.1)));
                self.index_layout(graph, &layout.slots);
                self.bands = layout.bands;
                self.layout_dirty = false;
            }
            self.ready = Rc::new(
                graph
                    .ready_tasks(None)
                    .into_iter()
                    .filter(|n| n.status == Status::Todo)
                    .map(|n| n.id.clone())
                    .collect(),
            );
            self.doing = Rc::new(
                graph
                    .nodes()
                    .filter(|n| n.kind == Kind::Task && n.status == Status::Doing)
                    .map(|n| n.id.clone())
                    .collect(),
            );
            self.task_count = graph.nodes().filter(|n| n.kind == Kind::Task).count();
            self.closed_count = graph.nodes().filter(|n| n.kind == Kind::Task && n.status.is_closed()).count();
            self.requirements = graph.nodes().map(|n| (n.id.clone(), n.depends_on.clone())).collect();
            self.dependents = graph.nodes().map(|n| (n.id.clone(), Vec::new())).collect();
            for node in graph.nodes() {
                for milestone in &node.milestones {
                    self.requirements.get_mut(milestone).expect("graph invariant").push(node.id.clone());
                }
            }
            self.referrers.clear();
            if self.view.group_by_tag {
                self.referrers = graph.nodes().map(|n| (n.id.clone(), Vec::new())).collect();
                for node in graph.nodes() {
                    let links =
                        node.depends_on.iter().map(|d| (d, false)).chain(node.milestones.iter().map(|m| (m, true)));
                    for (target, membership) in links {
                        self.referrers.get_mut(target).expect("graph invariant").push((node.id.clone(), membership));
                    }
                }
            }
            for (id, requirements) in &self.requirements {
                for requirement in requirements {
                    self.dependents.get_mut(requirement).expect("graph invariant").push(id.clone());
                }
            }
            self.open_requirements = self
                .requirements
                .iter()
                .map(|(id, requirements)| {
                    (
                        id.clone(),
                        requirements
                            .iter()
                            .filter(|id| !graph.get(id).expect("graph invariant").status.is_closed())
                            .count(),
                    )
                })
                .collect();
            self.progress =
                graph.nodes().filter(|n| n.kind == Kind::Milestone).map(|n| (n.id.clone(), (0, 0))).collect();
            for node in graph.nodes() {
                for milestone in &node.milestones {
                    let (closed, total) = self.progress.get_mut(milestone).expect("graph invariant");
                    *total += 1;
                    *closed += usize::from(node.status.is_closed());
                }
            }
            // Compute all critical paths in one shared topological pass.
            let mut paths: BTreeMap<NodeId, (usize, Option<NodeId>)> = BTreeMap::new();
            for id in &self.order {
                let node = graph.get(id).expect("graph invariant");
                let best = self.requirements[id]
                    .iter()
                    .filter(|dep| !graph.get(dep).expect("graph invariant").status.is_closed())
                    .max_by_key(|dep| paths[*dep].0)
                    .cloned();
                let length = best.as_ref().map_or(0, |dep| paths[dep].0)
                    + usize::from(node.kind == Kind::Task && !node.status.is_closed());
                paths.insert(id.clone(), (length, best));
            }
            self.critical_lengths =
                graph.nodes().filter(|n| n.kind == Kind::Milestone).map(|n| (n.id.clone(), paths[&n.id].0)).collect();
            self.path_next = paths.into_iter().map(|(id, (_, next))| (id, next)).collect();
        }
        let today = dates::today();
        if !unchanged || self.today != Some(today) {
            self.today = Some(today);
            self.overdue = Rc::new(
                graph
                    .nodes()
                    .filter(|n| !n.status.is_closed() && n.due.is_some_and(|d| d < today))
                    .map(|n| n.id.clone())
                    .collect(),
            );
        }
        if !unchanged || self.overview_dirty {
            self.overview.refresh(graph, &self.ready, &self.doing);
            self.overview_dirty = false;
        }
        if !unchanged || reselected {
            self.selected.clone_from(selected);
            self.selected_ids = selected.iter().cloned().collect();
            self.selected_status = selected
                .first()
                .and_then(|id| graph.get(id))
                .map(|n| n.status)
                .filter(|status| selected.iter().all(|id| graph.get(id).is_some_and(|n| n.status == *status)));
            self.active = active;
            self.cross = self.cross_band_edges(graph);
            self.focus = (!selected.is_empty()).then(|| {
                let mut focus = reachable(&self.requirements, selected);
                focus.extend(reachable(&self.dependents, selected));
                focus
            });
            self.critical.clear();
            self.critical_edges.clear();
            self.critical_paths.clear();
            for id in selected.iter().filter(|id| graph.get(id).is_some_and(|n| n.kind == Kind::Milestone)) {
                let mut path = Vec::with_capacity(self.critical_lengths[id]);
                let mut next = Some(id);
                while let Some(current) = next {
                    let n = graph.get(current).expect("graph invariant");
                    if n.kind == Kind::Task && !n.status.is_closed() {
                        path.push(current.clone());
                    }
                    next = self.path_next[current].as_ref();
                }
                path.reverse();
                self.critical.extend(path.iter().cloned());
                for pair in path.windows(2) {
                    self.critical_edges.entry(pair[0].clone()).or_default().insert(pair[1].clone());
                }
                if let Some(last) = path.last() {
                    self.critical_edges.entry(last.clone()).or_default().insert(id.clone());
                }
                self.critical_paths.insert(id.clone(), path);
            }
        }
    }
}

/// The time over which a pan's pointer speed is averaged, in seconds.
const PAN_SMOOTHING: f32 = 0.04;
/// A pointer that has been still this long before the release does not throw the canvas.
const PAN_PAUSE: Duration = Duration::from_millis(80);

const TILE: f32 = 560.;

/// Conservative Bezier bounds include both control points, including curves that cross the viewport.
fn edge_visible(from: Point<Pixels>, to: Point<Pixels>, viewport: Bounds<Pixels>, zoom: f32) -> bool {
    if viewport.size.width <= px(0.) {
        return true;
    }
    let head = (7. * zoom).clamp(4., 10.);
    let end_x = to.x - px(head);
    let dx = ((end_x - from.x) / 2.).max(px(24. * zoom));
    let min_x = from.x.min(to.x).min(end_x - dx) - px(4.);
    let max_x = from.x.max(to.x).max(from.x + dx) + px(4.);
    let min_y = from.y.min(to.y) - px(head + 4.);
    let max_y = from.y.max(to.y) + px(head + 4.);
    Bounds::new(point(min_x, min_y), size(max_x - min_x, max_y - min_y)).intersects(&viewport)
}

impl TopoApp {
    pub(crate) fn ensure_graph_cache(&mut self) {
        self.graph_cache.set_view(&self.view);
        self.graph_cache.refresh(&self.ws.graph, &self.selected_nodes, self.placed);
        // A selection never points at a node the canvas does not show.
        let cells = self.graph_cache.cells();
        if self.selected_nodes.iter().any(|id| !cells.contains_key(id)) {
            self.selected_nodes.retain(|id| cells.contains_key(id));
            if self.selected.as_ref().is_none_or(|id| !self.selected_nodes.contains(id)) {
                self.selected = self.selected_nodes.first().cloned();
            }
            self.graph_cache.refresh(&self.ws.graph, &self.selected_nodes, self.placed);
        }
    }

    /// The node whose card contains `local` (canvas coordinates).
    fn node_at(&self, local: Point<Pixels>) -> Option<NodeId> {
        self.card_at(local).map(|(id, _)| id)
    }

    /// The node whose card contains `local`, and the cell of that card.
    fn card_at(&self, local: Point<Pixels>) -> Option<(NodeId, layout::Cell)> {
        let x = f32::from(local.x - self.offset.x) / self.zoom;
        let y = f32::from(local.y - self.offset.y) / self.zoom;
        if x < 0. || y < 0. {
            return None;
        }
        let cell = ((x / crate::CELL_W) as usize, (y / crate::CELL_H) as usize);
        let id = self.graph_cache.at(cell)?;
        Bounds::new(self.to_screen(cell), size(px(NODE_W * self.zoom), px(NODE_H * self.zoom)))
            .contains(&local)
            .then(|| (id.clone(), cell))
    }

    pub(crate) fn on_drag_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let button = match self.drag {
            Some(Drag::Pan { button, .. }) => button,
            _ => MouseButton::Left,
        };
        if event.pressed_button != Some(button) {
            // The button was released outside the window.
            if self.drag.take().is_some() {
                cx.notify();
            }
            return;
        }
        let now = self.clock.now();
        match &mut self.drag {
            Some(Drag::Pan { last, at, velocity, moved, .. }) => {
                let delta = event.position - *last;
                *moved |= delta.x.abs() + delta.y.abs() > px(2.);
                *last = event.position;
                // Moves that share an instant carry no speed.
                let seconds = now.saturating_duration_since(*at).as_secs_f32();
                if seconds > 0. {
                    let weight = 1. - (-seconds / PAN_SMOOTHING).exp();
                    *velocity += (delta * (1. / seconds) - *velocity) * weight;
                    *at = now;
                }
                self.offset += delta;
                self.anim = None;
            }
            Some(Drag::Link { mouse, .. }) => *mouse = event.position,
            Some(Drag::Resize) => {
                let width = self.resized_width(event.position);
                self.set_inspector_width(width);
            }
            None => return,
        }
        cx.notify();
    }

    pub(crate) fn on_drag_end(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let button = match self.drag {
            Some(Drag::Pan { button, .. }) => button,
            Some(Drag::Link { .. } | Drag::Resize) => MouseButton::Left,
            None => return,
        };
        if event.button != button {
            return;
        }
        match self.drag.take() {
            Some(Drag::Pan { moved: false, on_card: false, button: MouseButton::Left, .. }) => self.clear_selection(),
            // A pointer that rested before the release lets go of the canvas without throwing it.
            Some(Drag::Pan { moved: true, at, velocity, .. })
                if self.clock.now().saturating_duration_since(at) < PAN_PAUSE =>
            {
                self.fling(velocity)
            }
            Some(Drag::Resize) => self.persist_inspector_width(),
            Some(Drag::Link { source, .. }) => {
                let area = self.area.get();
                self.ensure_graph_cache();
                match self.node_at(event.position - area.origin) {
                    Some(target) if target == source => {}
                    Some(target) => self.finish_link(source, target, cx),
                    // Dropped on empty canvas: the link gets a new task at its end.
                    None if area.contains(&event.position) => self.prompt_follow_up(&source, window, cx),
                    None => {}
                }
            }
            _ => {}
        }
        cx.notify();
    }

    /// Where the canvas point `at` is drawn in the canvas area.
    fn drawn_at(&self, at: [f32; 2]) -> Point<Pixels> {
        point(self.offset.x + px(at[0] * self.zoom), self.offset.y + px(at[1] * self.zoom))
    }

    pub(crate) fn graph_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current();
        self.ensure_graph_cache();
        // The overlays advance their motion, which the borrows of the graph below rule out.
        let overlays = self.overlays(window, cx);
        // The first frame has no canvas bounds yet. Use the window as a conservative
        // bound rather than constructing every offscreen card before layout settles.
        let area = self.area.get().size;
        let viewport =
            Bounds::new(point(px(0.), px(0.)), if area.width > px(0.) { area } else { window.viewport_size() });
        let today = dates::today();
        let z = self.zoom;
        let origin = self.area.get().origin;

        // While a layout change is in flight a card is drawn away from its cell, so cards
        // and edges are looked up in a viewport grown by how far that still is: at most by
        // the viewport itself, and by nothing once the cards have arrived.
        let mut progress = self.graph_cache.reflow_progress(window, cx);
        let [reach_x, reach_y] = spot(self.graph_cache.reflow.reach);
        let margin = point(
            viewport.size.width.min(px(reach_x * z * progress)),
            viewport.size.height.min(px(reach_y * z * progress)),
        );
        let grown = Bounds::new(viewport.origin - margin, viewport.size + size(margin.x * 2., margin.y * 2.));
        let placed = self.graph_cache.visible_nodes(self.offset + margin, z, grown);
        let mut reflow = Some(preset::SMOOTH);
        if progress > 0. {
            let GraphCache { cells, reflow: Reflow { previous, .. }, .. } = &self.graph_cache;
            let moves =
                |(id, cell): &&(NodeId, layout::Cell)| cells.get(id) == Some(cell) && previous.get(id) != Some(cell);
            if placed.iter().filter(moves).count() > REFLOW_CARDS {
                self.graph_cache.reflow.settle();
                (progress, reflow) = (0., None);
            } else if progress == 1. {
                // The first frame of the change.
                self.graph_cache.leave(&self.ws.graph, self.offset, z, viewport);
            }
        }
        let graph = &self.ws.graph;
        let cells = self.graph_cache.cells();
        let previous = &self.graph_cache.reflow.previous;
        let on_screen = |at: Point<Pixels>| {
            viewport.size.width <= px(0.)
                || Bounds::new(at, size(px((NODE_W + 8.) * z), px(NODE_H * z))).intersects(&viewport)
        };

        let focus = &self.graph_cache.focus;
        let cursor = self.list_cursor(cx);
        let critical = &self.graph_cache.critical;
        let filtered = |id: &NodeId| self.passes_filter(graph.get(id).expect("graph invariant"));
        let palette = self.palette.read(cx);
        let matching = matches!(self.prompt.get(), Some(crate::Prompt::Pick { .. }))
            || (matches!(self.prompt.get(), Some(crate::Prompt::Search)) && !palette.text(cx).trim().is_empty());
        let matches = matching.then(|| palette.matching_keys());
        let emphasized = |id: &NodeId| match (matching, &focus) {
            (true, _) => {
                matches.as_ref().is_some_and(|keys| keys.contains(id.as_str()))
                    || matches!(self.prompt.get(), Some(crate::Prompt::Pick { node, .. }) if node == id)
            }
            (false, Some(f)) => f.contains(id) && filtered(id),
            (false, None) => filtered(id),
        };

        let link = match &self.drag {
            Some(Drag::Link { source, mouse }) => Some((source.clone(), *mouse - origin)),
            _ => None,
        };
        let over = link.as_ref().and_then(|(_, mouse)| self.node_at(*mouse));
        let drop = link.as_ref().zip(over.as_ref()).filter(|((source, _), target)| source != *target).map(
            |((source, _), target)| {
                let (label, allowed) = self.connect_preview(source, target);
                (target.clone(), label, allowed)
            },
        );

        let card = |n: &Node| CardState {
            selected: self.selected_nodes.contains(&n.id),
            hovered: self.hovered.as_ref() == Some(&n.id) && self.drag.is_none(),
            dimmed: !emphasized(&n.id),
            critical: critical.contains(&n.id),
            cursor: cursor.as_ref() == Some(&n.id),
            drop: drop.as_ref().filter(|(t, ..)| *t == n.id).map(|(_, _, allowed)| *allowed),
            open_requirements: self.graph_cache.open_requirements[&n.id],
            progress: self.graph_cache.progress.get(&n.id).copied().unwrap_or_default(),
        };
        #[cfg(test)]
        self.graph_cache.drawn.borrow_mut().clear();
        let mut nodes: Vec<AnyElement> = Vec::new();
        // Where the cards that are away from their cells are drawn, for their edges.
        let mut moved: BTreeMap<&NodeId, [f32; 2]> = BTreeMap::new();
        for (id, cell) in &placed {
            let Some(n) = graph.get(id) else { continue };
            let state = card(n);
            let rest = spot(*cell);
            let done = f32::from(n.status == Status::Done);
            let fade = [state.opacity(n.status.is_closed()), f32::from(state.selected), done];
            // Only the first card of a node follows it between layouts; its cards in other
            // groups belong to their cells.
            let first = cells.get(id) == Some(cell);
            let key = match first {
                true => ElementId::from(id.as_str().to_owned()),
                false => ElementId::from(("card", ((cell.0 as u64) << 32) | cell.1 as u64)),
            };
            // A card that comes into view during the change joins it where it would be by now.
            let from = (first && progress > 0.).then(|| match previous.get(id) {
                Some(old) => {
                    let [x, y] = back(rest, spot(*old), progress);
                    ([x, y, 0.], fade)
                }
                None => ([rest[0], rest[1], SHRINK * progress], [0., fade[1], fade[2]]),
            });
            let motion = card_motion(window, cx, key, from, [rest[0], rest[1], 0.], fade, reflow);
            if first && motion.at != rest {
                moved.insert(id, motion.at);
            }
            if on_screen(self.drawn_at(motion.at)) {
                nodes.push(self.node_card(t, n, state, motion, today, cx));
            }
        }
        for Ghost { node, cell, open_requirements, progress } in &self.graph_cache.reflow.ghosts {
            let state = CardState {
                selected: false,
                hovered: false,
                dimmed: !self.passes_filter(node),
                critical: false,
                cursor: false,
                drop: None,
                open_requirements: *open_requirements,
                progress: *progress,
            };
            let [x, y] = spot(*cell);
            let done = f32::from(node.status == Status::Done);
            let from = ([x, y, 0.], [state.opacity(node.status.is_closed()), 0., done]);
            let key = ElementId::from(node.id.as_str().to_owned());
            let motion = card_motion(window, cx, key, Some(from), [x, y, SHRINK], [0., 0., done], reflow);
            if motion.opacity > 0. && on_screen(self.drawn_at(motion.at)) {
                let card = self.node_card(t, node, state, motion, today, cx);
                nodes.push(animation::surface(card).inert(true).into_any_element());
            }
        }

        // A card that is not drawn is taken to move in step with the change.
        let place = |id: &NodeId, cell: layout::Cell| match (moved.get(id), previous.get(id)) {
            _ if cells.get(id) != Some(&cell) => spot(cell),
            (Some(at), _) => *at,
            (None, Some(old)) => back(spot(cell), spot(*old), progress),
            (None, None) => spot(cell),
        };
        // Edges run from the right side of a requirement to the left side of
        // the node requiring it: prerequisite → dependent, member → milestone.
        let anchor = |id: &NodeId, cell: layout::Cell, right: bool| {
            let at = match moved.is_empty() && previous.is_empty() {
                true => spot(cell),
                false => place(id, cell),
            };
            let p = self.drawn_at(at);
            point(p.x + px(if right { NODE_W * z } else { 0. }), p.y + px(NODE_H * z / 2.))
        };
        let ends = |placed: &PlacedEdge| {
            (anchor(&placed.from, placed.from_cell, true), anchor(&placed.to, placed.to_cell, false))
        };
        let edge = |(placed, (start, end)): (&PlacedEdge, (Point<Pixels>, Point<Pixels>))| {
            let PlacedEdge { from, to, membership, .. } = placed;
            let closed = graph.get(from).expect("graph invariant").status.is_closed();
            let on_focus = focus.as_ref().is_some_and(|f| f.contains(from) && f.contains(to));
            let on_critical = self.graph_cache.critical_edges.get(from).is_some_and(|targets| targets.contains(to));
            let (color, width) = match () {
                _ if !emphasized(from) || !emphasized(to) => (t.edge_dim.into(), 1.),
                _ if on_critical => (t.critical.into(), 2.5),
                _ if on_focus => (t.accent.into(), 2.),
                _ if closed => (t.edge_closed.into(), 1.25),
                _ if *membership => (theme::alpha(t.milestone, 0.5), 1.25),
                _ => (t.edge.into(), 1.5),
            };
            EdgePaint { from: start, to: end, color, width, dashed: *membership }
        };
        let mut edges: Vec<EdgePaint> = self
            .graph_cache
            .visible_edges(self.offset + margin, z, grown)
            .into_iter()
            .map(|i| &self.graph_cache.edges[i])
            .chain(&self.graph_cache.cross)
            // Most edges of a tile miss the viewport: place them before working out their looks.
            .map(|placed| (placed, ends(placed)))
            .filter(|(_, (from, to))| edge_visible(*from, *to, viewport, z))
            .map(edge)
            .collect();
        edges.sort_by(|a, b| a.width.total_cmp(&b.width));
        let pending = link.as_ref().and_then(|(source, mouse)| {
            let color = match &drop {
                Some((_, _, false)) => t.danger,
                Some(_) => t.success,
                None => t.accent,
            };
            Some((anchor(source, *cells.get(source)?, true), *mouse, color))
        });

        let headers: Vec<AnyElement> = (0..self.graph_cache.bands.len())
            .filter(|&band| {
                let pos = self.to_screen((0, self.graph_cache.bands[band].row));
                viewport.size.width <= px(0.)
                    || Bounds::new(pos, size(px(NODE_W * z), px(NODE_H * z))).intersects(&grown)
            })
            .map(|band| self.group_header(t, band, reflow, window, cx))
            .collect();

        let area = self.area.clone();
        let offset = self.offset;
        let entity_id = cx.entity_id();
        let painter = canvas(
            move |bounds, window, _| {
                if area.get() != bounds {
                    area.set(bounds);
                    // Elements were culled with the previous viewport. Re-render after layout
                    // settles so growing the window also reveals newly visible cards.
                    window.on_next_frame(move |_, cx| cx.notify(entity_id));
                }
            },
            move |bounds: Bounds<Pixels>, (), window, _| {
                let o = bounds.origin;
                paint_grid(window, bounds, offset, z);
                for e in edges {
                    paint_edge(window, &e, o, bounds, z);
                }
                if let Some((from, mouse, color)) = pending {
                    let mut path = PathBuilder::stroke(px(2.)).dash_array(&[px(6.), px(4.)]);
                    path.move_to(from + o);
                    let (from, to) = (from + o, mouse + o);
                    let dx = ((to.x - from.x) / 2.).max(px(30.));
                    path.cubic_bezier_to(to, point(from.x + dx, from.y), point(to.x - dx, to.y));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                    window.paint_quad(fill(Bounds::centered_at(to, size(px(8.), px(8.))), color).corner_radii(px(4.)));
                }
            },
        )
        .absolute()
        .size_full();

        let cursor = match &self.drag {
            Some(Drag::Pan { moved: true, .. }) => CursorStyle::ClosedHand,
            Some(Drag::Link { .. }) => CursorStyle::Crosshair,
            _ => CursorStyle::Arrow,
        };
        let link_label = link.map(|(_, mouse)| {
            let (text, color) = match (&drop, &over) {
                (Some((_, label, true)), _) => (format!("↳ {label}"), t.success),
                (Some((_, label, false)), _) => (format!("✕ {label}"), t.danger),
                (None, Some(_)) => ("Drop on a node to connect, or on empty space".to_owned(), t.fg_muted),
                (None, None) => ("+ New follow-up task".to_owned(), t.fg),
            };
            div()
                .absolute()
                .left(mouse.x + px(14.))
                .top(mouse.y + px(14.))
                .px_2()
                .py_1()
                .rounded(px(metrics::R_SM))
                .bg(t.overlay)
                .border_1()
                .border_color(t.hairline)
                .text_color(color)
                .text_size(px(metrics::T_SMALL))
                .whitespace_nowrap()
                .shadow(metrics::e2())
                .child(text)
        });

        div()
            .id("canvas")
            .relative()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .cursor(cursor)
            .child(painter)
            .children(headers)
            .children(nodes)
            .when(self.graph().nodes().next().is_none(), |d| d.child(self.empty_state(cx)))
            .when(self.graph().nodes().next().is_some() && cells.is_empty() && self.graph_cache.bands.is_empty(), |d| {
                d.child(self.all_hidden_state(cx))
            })
            .children(link_label)
            .child(self.zoom_controls(cx))
            // The save status floats over the canvas, so showing it never moves the graph.
            .children(overlays)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, ev: &MouseDownEvent, _, cx| {
                    // A click away from an open prompt only dismisses it (the window does that).
                    if app.prompt.get().is_none() {
                        app.drag = Some(Drag::pan(ev, false, app.clock.now()));
                        cx.notify();
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|app, ev: &MouseDownEvent, _, cx| {
                    if app.prompt.get().is_none() {
                        app.drag = Some(Drag::pan(ev, true, app.clock.now()));
                        cx.notify();
                    }
                }),
            )
            // Any press takes hold of a camera that is moving by itself.
            .capture_any_mouse_down(cx.listener(|app, _, _, _| app.anim = None))
            .on_scroll_wheel(cx.listener(|app, ev: &ScrollWheelEvent, _, cx| {
                let anchor = ev.position - app.area.get().origin;
                let modified = ev.modifiers.platform || ev.modifiers.control;
                match ev.delta {
                    // Wheel notches add up: each zooms on from where the last one is heading.
                    ScrollDelta::Lines(delta) if !ev.modifiers.shift && !modified => {
                        let zoom = Self::stepped(app.target().1, 1.12_f32.powf(delta.y));
                        app.zoom_to(zoom, anchor, animation::preset::SNAPPY);
                    }
                    // Everything else moves the camera directly, from where it is drawn.
                    ScrollDelta::Lines(delta) => {
                        app.anim = None;
                        app.offset += match ev.modifiers.shift {
                            true => point(px((delta.y + delta.x) * 40.), px(0.)),
                            false => point(px(delta.x * 40.), px(delta.y * 40.)),
                        };
                    }
                    // The scroll's end is not reported for every device, so the zoom stops at its limits.
                    ScrollDelta::Pixels(delta) if modified => {
                        app.zoom_by((f32::from(delta.y) / 300.).exp(), anchor, false)
                    }
                    ScrollDelta::Pixels(delta) => {
                        app.anim = None;
                        app.offset += delta;
                    }
                }
                cx.notify();
            }))
    }

    /// The header of group `band`: click to fold or unfold its cards.
    fn group_header(
        &self,
        t: &'static Theme,
        band: usize,
        reflow: Option<Preset>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Band { group, row, count } = &self.graph_cache.bands[band];
        let z = self.zoom;
        let collapsed = self.graph_cache.view.collapsed.contains(group);
        let toggled = group.clone();
        let name = format!("group-{}", group.label());
        // The header moves with its cards, and its chevron turns from pointing right (0°)
        // to pointing down (90°) as the group unfolds.
        let rest = [spot((0, *row))[1], if collapsed { 0. } else { 90. }];
        let id = ElementId::from(name.clone());
        let [y, turn] = animation::springs_within(window, cx, id, None, rest, reflow, PLACE_REST);
        // At rest the chevron is the icon itself, not a turned one.
        let chevron = match turn {
            turn if turn <= 0. => ui::icon(ui::Icon::ChevronRight, t.fg_faint),
            turn if turn >= 90. => ui::icon(ui::Icon::ChevronDown, t.fg_faint),
            turn => ui::icon(ui::Icon::ChevronRight, t.fg_faint)
                .with_transformation(Transformation::rotate(radians(turn.to_radians()))),
        };
        div()
            .id(ElementId::Name(name.clone().into()))
            .debug_selector(move || name.clone())
            .absolute()
            .left(self.offset.x)
            // Sits at the bottom of its row, right above the first card of the group.
            .top(self.drawn_at([0., y]).y + px((crate::CELL_H - 6. - 28.) * z))
            .h(px(28. * z))
            .px(px(10. * z))
            .flex()
            .items_center()
            .gap(px(6. * z))
            .rounded(px(metrics::R_MD * z))
            .bg(t.chrome)
            .border_1()
            .border_color(t.hairline)
            .text_size(px(12. * z))
            .text_color(t.fg_faint)
            .whitespace_nowrap()
            .cursor_pointer()
            .hover(|s| s.border_color(t.border_strong))
            .child(chevron.size(px(12. * z)))
            .child(div().text_color(t.fg).child(group.label()))
            .child(div().text_color(t.fg_muted).child(count.to_string()))
            .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| cx.stop_propagation()))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.toggle_group(&toggled);
                cx.notify();
            }))
            .into_any_element()
    }

    fn node_card(
        &self,
        t: &'static Theme,
        node: &Node,
        state: CardState,
        motion: CardMotion,
        today: Date,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        #[cfg(test)]
        self.graph_cache.drawn.borrow_mut().insert(node.id.clone(), motion);
        let z = self.zoom;
        let pos = self.drawn_at(motion.at);
        let id = node.id.clone();
        let milestone = node.kind == Kind::Milestone;
        let closed = node.status.is_closed();
        let (icon, icon_color) = theme::node_icon(node);
        // The glyph changes at once; its colour and the title's cross over from the other state.
        let icon_color = match node.status {
            Status::Done if milestone => mix(t.milestone.into(), icon_color.into(), motion.done),
            Status::Done => mix(theme::status_color(Status::Todo).into(), icon_color.into(), motion.done),
            _ => mix(icon_color.into(), t.status_done.into(), motion.done),
        };
        let unselected: Hsla = match () {
            _ if state.critical => theme::alpha(t.critical, 0.6),
            _ if milestone => theme::alpha(t.milestone, 0.45),
            _ if state.hovered => t.border_strong.into(),
            _ => t.hairline.into(),
        };
        let border: Hsla = match (state.drop, state.cursor) {
            (Some(true), _) => t.success.into(),
            (Some(false), _) => t.danger.into(),
            (None, true) => t.accent.into(),
            (None, false) => mix(unselected, t.accent.into(), motion.selected),
        };
        let bg = match (milestone, state.hovered) {
            (true, false) => t.card_milestone,
            (true, true) => t.card_milestone_hover,
            (false, false) => t.card,
            (false, true) => t.card_hover,
        };

        let check = {
            let id = id.clone();
            div()
                .id(ElementId::Name(format!("check-{id}").into()))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .size(px(18. * z))
                .rounded(px(4. * z))
                .text_color(icon_color)
                .text_size(px(13. * z))
                .child(scaled(icon, motion.check))
                .when(!milestone, |d| {
                    d.hover(|s| s.bg(t.control_hover)).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |app, _, _, cx| {
                            cx.stop_propagation();
                            app.toggle_done(id.clone(), cx);
                        }),
                    )
                })
        };
        let title = div()
            .flex_1()
            .min_w(px(0.))
            .truncate()
            .text_color(match node.status {
                Status::Dropped => t.fg_faint.into(),
                _ => mix(t.fg.into(), t.fg_muted.into(), motion.done),
            })
            .when(milestone, |d| d.font_weight(gpui::FontWeight::SEMIBOLD))
            .when(node.status == Status::Dropped || motion.done >= 0.5, |d| d.line_through())
            .child(node.title.clone());

        let mini = |label: String, color: Rgba, fill: Hsla| {
            div().flex_shrink_0().px(px(5. * z)).rounded(px(3. * z)).bg(fill).text_color(color).child(label)
        };
        let neutral: Hsla = t.control.into();
        let tinted = |label: String, color: Rgba| mini(label, color, theme::alpha(color, 0.14));
        let due = node.due.map(|d| {
            let color = if closed { t.fg_faint } else { dates::urgency_color(d, today) };
            let label = format!("⏱ {}", dates::short(d, today));
            match color == t.danger || color == t.warn {
                true => tinted(label, color),
                false => div().flex_shrink_0().text_color(color).child(label),
            }
        });
        // Named with a glyph as well as colored. Tags give way to it and to the assignee.
        let priority = node.priority.map(|p| {
            let color = theme::priority_color(p);
            let label = theme::priority_text(p);
            let chip = match p {
                topo_core::Priority::Urgent | topo_core::Priority::High => tinted(label, color),
                _ => mini(label, color, neutral),
            };
            chip.when(p == topo_core::Priority::High, |d| d.font_weight(gpui::FontWeight::SEMIBOLD))
        });
        let assignee = node.assignee.as_ref().map(|a| div().min_w(px(0.)).truncate().child(format!("@{a}")));
        let tags = 2usize.saturating_sub(usize::from(priority.is_some()) + usize::from(assignee.is_some()));
        let meta = div()
            .flex()
            .items_center()
            .overflow_hidden()
            .gap(px(6. * z))
            .pl(px(24. * z))
            .text_size(px(10.5 * z))
            .text_color(t.fg_faint);
        let meta = match node.kind {
            Kind::Milestone => {
                let (done, total) = state.progress;
                let fraction = if total == 0 { 0. } else { done as f32 / total as f32 };
                let complete = closed || (total > 0 && done == total);
                meta.child(ui::progress_bar(fraction, complete, 4. * z))
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(if complete { t.progress_complete } else { t.progress })
                            .child(format!("{done}/{total}")),
                    )
                    .children(priority)
                    .children(due)
            }
            Kind::Task => {
                let open_reqs = state.open_requirements;
                let status = match node.status {
                    Status::Doing => mini("In progress".into(), t.on_accent, t.accent.into()),
                    Status::Done => tinted("Done".into(), t.status_done),
                    Status::Dropped => mini("Dropped".into(), t.fg_faint, neutral),
                    Status::Todo if open_reqs == 0 => tinted("Ready".into(), t.ready),
                    Status::Todo => div().flex_shrink_0().child(format!("Blocked by {open_reqs}")),
                };
                meta.child(status)
                    .children(priority)
                    .children(due)
                    .children(assignee)
                    .children(node.tags.iter().take(tags).map(|t| div().flex_shrink_0().child(format!("#{t}"))))
            }
        };

        let handle = {
            let id = id.clone();
            div()
                .id(ElementId::Name(format!("handle-{id}").into()))
                .absolute()
                .left(px((NODE_W - 7.) * z))
                .top(px((NODE_H / 2. - 7.) * z))
                .size(px(14. * z))
                .rounded_full()
                .bg(t.card)
                .border_2()
                .border_color(t.accent)
                .cursor(CursorStyle::Crosshair)
                .hover(|s| s.bg(t.accent))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |app, ev: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        app.drag = Some(Drag::Link { source: id.clone(), mouse: ev.position });
                        cx.notify();
                    }),
                )
        };

        let down_id = id.clone();
        let card = div()
            .w(px(NODE_W * z))
            .h_full()
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(6. * z))
            .pl(px(10. * z))
            .pr(px(12. * z))
            .rounded(px(if milestone { 16. } else { 10. } * z))
            .border_2()
            .border_color(border)
            .bg(bg)
            .when(motion.selected > 0., |d| {
                let mut shadows = metrics::e1();
                shadows.iter_mut().for_each(|shadow| shadow.color.a *= motion.selected);
                d.shadow(shadows)
            })
            .text_size(px(13. * z))
            .cursor_pointer()
            .child(div().flex().items_center().gap(px(6. * z)).child(check).child(title))
            .when(z >= 0.55, |d| d.child(meta))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |app, ev: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    if app.prompt.get().is_some() {
                        app.close_prompt(window, cx);
                    }
                    window.focus(&app.focus, cx);
                    if ev.modifiers.platform || ev.modifiers.control {
                        app.toggle_selection(down_id.clone());
                        cx.notify();
                        return;
                    }
                    app.select(Some(down_id.clone()), false);
                    app.placed = app.card_at(ev.position - app.area.get().origin).map(|(_, cell)| cell);
                    app.drag = match () {
                        _ if ev.modifiers.shift => Some(Drag::Link { source: down_id.clone(), mouse: ev.position }),
                        _ if ev.click_count >= 2 => {
                            app.start_inline(Field::Title, window, cx);
                            None
                        }
                        // Cards are placed by the layout, so dragging one moves the canvas.
                        _ => Some(Drag::pan(ev, true, app.clock.now())),
                    };
                    cx.notify();
                }),
            );

        // Wider than the card, so the pointer can reach all of the handle without leaving.
        let card = div()
            .id(ElementId::Name(format!("node-{id}").into()))
            .debug_selector(|| format!("node-{id}"))
            .absolute()
            .left(pos.x)
            .top(pos.y)
            .w(px((NODE_W + 8.) * z))
            .h(px(NODE_H * z))
            .opacity(motion.opacity)
            .child(card)
            .when(state.hovered || state.selected, |d| d.child(handle))
            .on_hover(cx.listener(move |app, hovered: &bool, _, cx| {
                match *hovered {
                    true if app.hovered.as_ref() == Some(&id) => return,
                    true => app.hovered = Some(id.clone()),
                    false if app.hovered.as_ref() == Some(&id) => app.hovered = None,
                    false => return,
                }
                cx.notify();
            }));
        scaled(card, motion.scale)
    }
}

/// Dots every 28 canvas units, thinned to a minimum 28 screen pixels at low zoom.
fn paint_grid(window: &mut Window, bounds: Bounds<Pixels>, offset: Point<Pixels>, zoom: f32) {
    paint_grid_at_density(window, bounds, offset, zoom, 28.);
}

fn paint_grid_at_density(window: &mut Window, bounds: Bounds<Pixels>, offset: Point<Pixels>, zoom: f32, minimum: f32) {
    let t = theme::current();
    let mut step = 28. * zoom;
    while step < minimum {
        step *= 2.;
    }
    let dot = if zoom > 1.2 { 2. } else { 1.5 };
    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let start = |o: Pixels| f32::from(o).rem_euclid(step);
    let color = t.grid_dot;
    let mut y = start(offset.y);
    while y < h {
        let mut x = start(offset.x);
        while x < w {
            window.paint_quad(fill(Bounds::new(bounds.origin + point(px(x), px(y)), size(px(dot), px(dot))), color));
            x += step;
        }
        y += step;
    }
}

/// Flatten curves before dashing so offscreen dash geometry is never tessellated.
/// Arc length still advances through hidden portions, keeping dashes anchored while panning.
fn dashed_segments(curve: [Point<Pixels>; 4], viewport: Bounds<Pixels>) -> Vec<(Point<Pixels>, Point<Pixels>)> {
    fn flatten(c: [Point<f64>; 4], depth: usize, points: &mut Vec<Point<f64>>) {
        let distance = |a: Point<f64>, b: Point<f64>| (b.x - a.x).hypot(b.y - a.y);
        let chord = distance(c[0], c[3]);
        let polygon = distance(c[0], c[1]) + distance(c[1], c[2]) + distance(c[2], c[3]);
        let deviation = |p: Point<f64>| {
            if chord < 1e-9 {
                return distance(c[0], p);
            }
            ((p.x - c[0].x) * (c[3].y - c[0].y) - (p.y - c[0].y) * (c[3].x - c[0].x)).abs() / chord
        };
        if depth == 20 || (deviation(c[1]).max(deviation(c[2])) <= 0.2 && polygon - chord <= 0.2) {
            points.push(c[3]);
            return;
        }
        let mid = |a: Point<f64>, b: Point<f64>| point((a.x + b.x) / 2., (a.y + b.y) / 2.);
        let (a, b, d) = (mid(c[0], c[1]), mid(c[1], c[2]), mid(c[2], c[3]));
        let (ab, bd) = (mid(a, b), mid(b, d));
        let center = mid(ab, bd);
        flatten([c[0], a, ab, center], depth + 1, points);
        flatten([center, bd, d, c[3]], depth + 1, points);
    }
    let curve = curve.map(|p| point(f64::from(f32::from(p.x)), f64::from(f32::from(p.y))));
    let mut points = vec![curve[0]];
    flatten(curve, 0, &mut points);
    let mut segments = Vec::new();
    let mut phase = 0.;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let length = dx.hypot(dy);
        if length < 1e-9 {
            continue;
        }
        let mut enter: f64 = 0.;
        let mut exit: f64 = 1.;
        // Include the stroke/antialiasing margin so clipped ends remain outside the canvas.
        for (start, delta, min, max) in [
            (a.x, dx, f64::from(f32::from(viewport.left())) - 4., f64::from(f32::from(viewport.right())) + 4.),
            (a.y, dy, f64::from(f32::from(viewport.top())) - 4., f64::from(f32::from(viewport.bottom())) + 4.),
        ] {
            if delta.abs() < 1e-9 {
                if start < min || start > max {
                    exit = -1.;
                }
            } else {
                let (lo, hi) = ((min - start) / delta, (max - start) / delta);
                enter = enter.max(lo.min(hi));
                exit = exit.min(lo.max(hi));
            }
        }
        if enter < exit {
            let mut position = enter * length;
            let end = exit * length;
            let at = |distance: f64| {
                point(px((a.x + dx * distance / length) as f32), px((a.y + dy * distance / length) as f32))
            };
            while position < end {
                let offset = (phase + position) % 9.;
                let on = offset < 5.;
                let next = (position + ((if on { 5. } else { 9. }) - offset).max(1e-6)).min(end);
                if on {
                    segments.push((at(position), at(next)));
                }
                position = next;
            }
        }
        phase = (phase + length) % 9.;
    }
    segments
}

fn paint_edge(window: &mut Window, edge: &EdgePaint, origin: Point<Pixels>, viewport: Bounds<Pixels>, zoom: f32) {
    let (from, to) = (edge.from + origin, edge.to + origin);
    let color = edge.color;
    let head = (7. * zoom).clamp(4., 10.);
    let end = point(to.x - px(head), to.y);
    let dx = ((end.x - from.x) / 2.).max(px(24. * zoom));
    let controls = [from, point(from.x + dx, from.y), point(end.x - dx, end.y), end];
    let mut path = PathBuilder::stroke(px(edge.width));
    if edge.dashed {
        for (from, to) in dashed_segments(controls, viewport) {
            path.move_to(from);
            path.line_to(to);
        }
    } else {
        path.move_to(from);
        path.cubic_bezier_to(end, controls[1], controls[2]);
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
    let mut arrow = PathBuilder::fill();
    arrow.add_polygon(
        &[to, point(to.x - px(head), to.y - px(head / 2.)), point(to.x - px(head), to.y + px(head / 2.))],
        true,
    );
    if let Ok(arrow) = arrow.build() {
        window.paint_path(arrow, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_dashed_curves_clip_geometry_and_keep_their_phase() {
        let curve = [
            point(px(0.), px(0.)),
            point(px(100.), px(0.)),
            point(px(100.), px(100000.)),
            point(px(200.), px(100000.)),
        ];
        let viewport = Bounds::new(point(px(0.), px(50000.)), size(px(300.), px(800.)));
        let clipped = dashed_segments(curve, viewport);
        assert!(!clipped.is_empty() && clipped.len() < 200, "geometry scales with the viewport, not edge length");
        for (a, b) in &clipped {
            assert!(a.y >= viewport.top() - px(4.) && b.y <= viewport.bottom() + px(4.));
            let length = f32::from(b.x - a.x).hypot(f32::from(b.y - a.y));
            assert!(length <= 5.02);
        }
        let larger = Bounds::new(point(px(0.), px(49900.)), size(px(300.), px(1000.)));
        let inner = |(a, b): &(Point<Pixels>, Point<Pixels>)| {
            a.y > viewport.top() + px(10.) && b.y < viewport.bottom() - px(10.)
        };
        assert_eq!(
            clipped.into_iter().filter(inner).collect::<Vec<_>>(),
            dashed_segments(curve, larger).into_iter().filter(inner).collect::<Vec<_>>(),
            "clipping never restarts the dash pattern"
        );
        let away = Bounds::new(point(px(500.), px(50000.)), size(px(300.), px(800.)));
        assert!(dashed_segments(curve, away).is_empty());
    }

    #[test]
    fn horizontal_dashes_retain_five_on_four_off_from_the_source() {
        let curve = [point(px(0.), px(10.)), point(px(30.), px(10.)), point(px(60.), px(10.)), point(px(90.), px(10.))];
        let viewport = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(100.)));
        let segments = dashed_segments(curve, viewport);
        assert_eq!(segments.len(), 10);
        for (i, (from, to)) in segments.iter().enumerate() {
            assert_eq!(*from, point(px(i as f32 * 9.), px(10.)));
            assert_eq!(*to, point(px(i as f32 * 9. + 5.), px(10.)));
        }
    }

    fn fixture(count: usize) -> Graph {
        Graph::from_nodes((0..count).map(|i| {
            let mut node = Node::new(NodeId(format!("n{i:04}")), Kind::Task, format!("Task {i}"));
            if i >= 25 {
                node.depends_on.push(NodeId(format!("n{:04}", i - 25)));
            }
            node
        }))
        .unwrap()
    }

    #[test]
    fn cached_metadata_refreshes_after_status_and_topology_edits() {
        let mut graph = fixture(50);
        let mut cache = GraphCache::default();
        let selected = BTreeSet::from([NodeId("n0025".into()), NodeId("n0001".into())]);
        cache.refresh(&graph, &selected, None);
        assert_eq!(cache.open_requirements[&NodeId("n0025".into())], 1);
        assert!(cache.focus.as_ref().unwrap().contains(&NodeId("n0000".into())));
        assert!(cache.focus.as_ref().unwrap().contains(&NodeId("n0026".into())));
        graph.set_status(&NodeId("n0000".into()), Status::Done).unwrap();
        cache.invalidate();
        cache.refresh(&graph, &selected, None);
        assert_eq!(cache.open_requirements[&NodeId("n0025".into())], 0);
        let mut extra = Node::new(NodeId("extra".into()), Kind::Task, "Extra".into());
        extra.depends_on.push(NodeId("n0025".into()));
        graph.insert(extra).unwrap();
        cache.invalidate();
        cache.refresh(&graph, &selected, None);
        assert_eq!(cache.cells[&NodeId("extra".into())].0, 2);
        assert!(cache.focus.as_ref().unwrap().contains(&NodeId("extra".into())));
        assert_eq!(cache.dependents[&NodeId("n0025".into())], vec![NodeId("extra".into())]);
        cache.refresh(&graph, &BTreeSet::new(), None);
        assert!(cache.focus.is_none());
    }

    fn id(value: &str) -> NodeId {
        NodeId(value.into())
    }

    fn graph(nodes: &[(&str, Kind, &[&str], &[&str])]) -> Graph {
        Graph::from_nodes(nodes.iter().map(|(name, kind, dependencies, milestones)| {
            let mut node = Node::new(id(name), *kind, (*name).into());
            node.depends_on = dependencies.iter().map(|value| id(value)).collect();
            node.milestones = milestones.iter().map(|value| id(value)).collect();
            node
        }))
        .unwrap()
    }

    fn chain_fixture(count: usize) -> Graph {
        Graph::from_nodes((0..count).map(|i| {
            let mut node = Node::new(NodeId(format!("n{i:04}")), Kind::Task, format!("Task {i}"));
            if i > 0 {
                node.depends_on.push(NodeId(format!("n{:04}", i - 1)));
            }
            node
        }))
        .unwrap()
    }

    #[test]
    fn selecting_every_node_in_a_long_chain_keeps_the_complete_focus() {
        let graph = chain_fixture(1000);
        let selected = graph.nodes().map(|node| node.id.clone()).collect();
        let mut cache = GraphCache::default();
        cache.refresh(&graph, &selected, None);
        assert_eq!(cache.focus.as_ref(), Some(&selected));
        assert_eq!(cache.requirements.values().map(Vec::len).sum::<usize>(), 999);
        assert_eq!(cache.dependents.values().map(Vec::len).sum::<usize>(), 999);
    }

    #[test]
    fn selection_focus_ignores_nodes_removed_from_the_graph() {
        let graph = fixture(1);
        let mut cache = GraphCache::default();
        cache.refresh(&graph, &BTreeSet::from([id("missing"), id("n0000")]), None);
        assert_eq!(cache.focus, Some(BTreeSet::from([id("n0000")])));
    }

    #[test]
    fn selection_chains_do_not_turn_at_diamond_requirements_or_dependents() {
        let graph = graph(&[
            ("root", Kind::Task, &[], &[]),
            ("left", Kind::Task, &["root"], &[]),
            ("right", Kind::Task, &["root"], &[]),
            ("end", Kind::Task, &["left", "right"], &[]),
        ]);
        let mut cache = GraphCache::default();
        cache.refresh(&graph, &BTreeSet::from([id("left")]), None);
        assert_eq!(cache.focus, Some(BTreeSet::from([id("root"), id("left"), id("end")])));
        cache.refresh(&graph, &BTreeSet::from([id("left"), id("right")]), None);
        assert_eq!(cache.focus, Some(graph.nodes().map(|node| node.id.clone()).collect()));
    }

    #[test]
    fn membership_is_traversed_in_both_directions_without_adding_siblings() {
        let graph = graph(&[
            ("a", Kind::Task, &[], &["m"]),
            ("b", Kind::Task, &[], &["m"]),
            ("m", Kind::Milestone, &[], &[]),
            ("next", Kind::Task, &["m"], &[]),
        ]);
        let mut cache = GraphCache::default();
        cache.refresh(&graph, &BTreeSet::from([id("a")]), None);
        assert_eq!(cache.focus, Some(BTreeSet::from([id("a"), id("m"), id("next")])));
        assert_eq!(cache.open_requirements[&id("m")], 2);
        cache.refresh(&graph, &BTreeSet::from([id("m")]), None);
        assert_eq!(cache.focus, Some(graph.nodes().map(|node| node.id.clone()).collect()));
    }

    #[test]
    fn critical_edges_end_at_each_selected_milestone_without_cross_path_edges() {
        let mut graph = graph(&[
            ("a", Kind::Task, &[], &[]),
            ("b", Kind::Task, &["a"], &["m1"]),
            // c is a shorter, off-path member of m1, but critical for m2.
            ("c", Kind::Task, &[], &["m1"]),
            ("d", Kind::Task, &["c"], &["m2"]),
            ("m1", Kind::Milestone, &[], &[]),
            ("m2", Kind::Milestone, &[], &[]),
        ]);
        let mut cache = GraphCache::default();
        let selected = BTreeSet::from([id("m1"), id("m2")]);
        cache.refresh(&graph, &selected, None);
        assert_eq!(
            cache.critical_edges,
            BTreeMap::from([
                (id("a"), BTreeSet::from([id("b")])),
                (id("b"), BTreeSet::from([id("m1")])),
                (id("c"), BTreeSet::from([id("d")])),
                (id("d"), BTreeSet::from([id("m2")])),
            ])
        );
        graph.set_status(&id("b"), Status::Done).unwrap();
        cache.invalidate();
        cache.refresh(&graph, &selected, None);
        assert!(!cache.critical_edges.contains_key(&id("b")));
        assert_eq!(cache.critical_edges[&id("c")], BTreeSet::from([id("d"), id("m1")]));
        assert_eq!(cache.open_requirements[&id("m1")], 1);
    }

    #[test]
    fn metadata_and_status_changes_keep_layout_and_refresh_only_needed_metrics() {
        let mut graph = graph(&[
            ("a", Kind::Task, &[], &["m"]),
            ("b", Kind::Task, &["a"], &["m"]),
            ("m", Kind::Milestone, &[], &[]),
        ]);
        let selected = BTreeSet::from([id("m")]);
        let mut cache = GraphCache::default();
        cache.refresh(&graph, &selected, None);
        let cells = cache.cells.clone();
        let before = graph.clone();
        graph.edit(&id("a"), topo_core::Edit { title: Some("renamed".into()), ..Default::default() }).unwrap();
        cache.changed(&before, &graph);
        assert!(!cache.layout_dirty && !cache.dirty);
        cache.refresh(&graph, &selected, None);
        assert_eq!(cache.cells, cells);
        let before = graph.clone();
        graph.set_status(&id("a"), Status::Done).unwrap();
        cache.changed(&before, &graph);
        assert!(!cache.layout_dirty && cache.dirty);
        cache.refresh(&graph, &selected, None);
        assert_eq!(cache.cells, cells);
        assert_eq!(cache.progress[&id("m")], graph.progress(&id("m")));
        assert_eq!(cache.critical_paths[&id("m")], graph.critical_path(&id("m")));
        assert_eq!(cache.open_requirements[&id("b")], 0);
        let before = graph.clone();
        graph.unlink(&id("b"), &id("a")).unwrap();
        cache.changed(&before, &graph);
        assert!(cache.layout_dirty);
        cache.refresh(&graph, &selected, None);
        assert_eq!(cache.cells.len(), graph.nodes().count());
        assert_eq!(cache.cells[&id("a")], (0, 0));
    }

    #[test]
    fn spatial_indices_never_omit_visible_cards_or_crossing_edges() {
        let mut graph = fixture(1000);
        graph.link(&id("n0999"), &id("n0000")).unwrap();
        let mut cache = GraphCache::default();
        cache.refresh(&graph, &BTreeSet::new(), None);
        let viewport = Bounds::new(point(px(0.), px(0.)), size(px(1360.), px(860.)));
        for zoom in [0.25, 0.9, 2.5] {
            for x in [-10000., -500., -32., 0., 800., 40000.] {
                for y in [-2500., -33., 0., 900.] {
                    let offset = point(px(x), px(y));
                    let screen = |id: &NodeId| {
                        let (col, row) = cache.cells[id];
                        offset + point(px(col as f32 * crate::CELL_W * zoom), px(row as f32 * crate::CELL_H * zoom))
                    };
                    let nodes: BTreeSet<_> =
                        cache.visible_nodes(offset, zoom, viewport).into_iter().map(|(id, _)| id).collect();
                    for id in cache.cells.keys() {
                        if Bounds::new(screen(id), size(px((NODE_W + 8.) * zoom), px(NODE_H * zoom)))
                            .intersects(&viewport)
                        {
                            assert!(nodes.contains(id), "missing {id} at {offset:?}, {zoom}");
                        }
                    }
                    let edges = cache.visible_edges(offset, zoom, viewport);
                    for (i, PlacedEdge { from, to, .. }) in cache.edges.iter().enumerate() {
                        let from = screen(from) + point(px(NODE_W * zoom), px(NODE_H * zoom / 2.));
                        let to = screen(to) + point(px(0.), px(NODE_H * zoom / 2.));
                        if edge_visible(from, to, viewport, zoom) {
                            assert!(edges.contains(&i), "missing crossing edge {i}");
                        }
                    }
                }
            }
        }
        let nodes = cache.visible_nodes(point(px(0.), px(0.)), 1., viewport);
        let edges = cache.visible_edges(point(px(0.), px(0.)), 1., viewport);
        assert!(nodes.len() < 200);
        assert!(edges.len() < 200);
    }

    /// Selection-only CPU benchmark; layout/adjacency construction is excluded.
    #[test]
    #[ignore]
    fn multi_selection_chain_benchmark() {
        use std::{hint::black_box, time::Instant};
        for count in [500, 1000] {
            let graph = chain_fixture(count);
            let selected: BTreeSet<_> = graph.nodes().map(|node| node.id.clone()).collect();
            let mut cache = GraphCache::default();
            cache.refresh(&graph, &BTreeSet::new(), None);
            let start = Instant::now();
            let legacy: BTreeSet<_> = selected
                .iter()
                .flat_map(|id| graph.descendants(id).into_iter().chain(graph.ancestors(id)).chain([id.clone()]))
                .collect();
            let legacy_ms = start.elapsed().as_secs_f64() * 1000.;
            let mut samples = Vec::new();
            for _ in 0..40 {
                cache.refresh(&graph, &BTreeSet::new(), None);
                let start = Instant::now();
                cache.refresh(&graph, black_box(&selected), None);
                samples.push(start.elapsed().as_secs_f64() * 1000.);
                assert_eq!(cache.focus.as_ref(), Some(&legacy));
                black_box(&cache.focus);
            }
            samples.sort_by(f64::total_cmp);
            eprintln!(
                "{count} selected chain: legacy {:.3} ms; shared traversal median {:.3} ms p95 {:.3} ms",
                legacy_ms, samples[20], samples[38]
            );
        }
    }

    #[test]
    fn culling_keeps_crossing_edges_and_rejects_distant_edges() {
        let viewport = Bounds::new(point(px(0.), px(0.)), size(px(800.), px(600.)));
        assert!(edge_visible(point(px(-300.), px(300.)), point(px(1000.), px(300.)), viewport, 1.));
        assert!(!edge_visible(point(px(900.), px(800.)), point(px(1200.), px(800.)), viewport, 1.));
        // The control hull can enter the viewport even when endpoints are outside.
        assert!(edge_visible(point(px(790.), px(-10.)), point(px(820.), px(610.)), viewport, 1.));
    }

    #[gpui::test]
    fn growing_viewport_reveals_previously_culled_cards(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let mut ws = topo_core::Workspace::init(directory.path()).unwrap();
        ws.graph = fixture(0);
        let (app, visual) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
        visual.simulate_resize(size(px(700.), px(860.)));
        visual.run_until_parked();
        app.update(visual, |app, cx| {
            app.ws.graph = fixture(1);
            app.graph_cache.invalidate();
            app.anim = None;
            app.zoom = 1.;
            app.offset = point(px(850.), px(100.));
            cx.notify();
        });
        visual.run_until_parked();
        assert!(visual.debug_bounds("node-n0000").is_none());
        visual.simulate_resize(size(px(1800.), px(860.)));
        // The test platform has no display-link callback; refresh models the two
        // scheduled display frames (bounds discovery, then culling with new bounds).
        visual.refresh().unwrap();
        visual.run_until_parked();
        // Model the on_next_frame notification, absent from the test platform.
        app.update(visual, |_, cx| cx.notify());
        visual.run_until_parked();
        assert!(visual.debug_bounds("node-n0000").is_some());
    }

    struct GridBench {
        minimum: Option<f32>,
        zoom: f32,
    }
    impl gpui::Render for GridBench {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let minimum = self.minimum;
            let zoom = self.zoom;
            div().size_full().child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, _| {
                        if let Some(minimum) = minimum {
                            paint_grid_at_density(window, bounds, point(px(0.), px(0.)), zoom, minimum);
                        }
                    },
                )
                .size_full(),
            )
        }
    }

    #[gpui::test]
    #[ignore]
    fn grid_frame_benchmark(cx: &mut gpui::TestAppContext) {
        use std::time::Instant;
        let (app, visual) = cx.add_window_view(|_, _| GridBench { minimum: None, zoom: 0.5 });
        visual.simulate_resize(size(px(1360.), px(860.)));
        for minimum in [None, Some(14.), Some(28.)] {
            app.update(visual, |app, cx| {
                app.minimum = minimum;
                cx.notify();
            });
            visual.run_until_parked();
            let mut samples = Vec::new();
            for _ in 0..80 {
                let started = Instant::now();
                app.update(visual, |_, cx| cx.notify());
                visual.run_until_parked();
                samples.push(started.elapsed().as_secs_f64() * 1000.);
            }
            samples.sort_by(f64::total_cmp);
            eprintln!("1360x860 grid zoom=0.5 min={minimum:?}: median {:.3} ms p95 {:.3} ms", samples[40], samples[76]);
        }
    }

    /// Release-mode headless CPU frame benchmark, excluding GPU/compositor time.
    /// cargo test -p topo-gui --release canvas_frame_benchmark -- --ignored --nocapture
    #[gpui::test]
    #[ignore]
    fn canvas_frame_benchmark(cx: &mut gpui::TestAppContext) {
        use std::time::Instant;
        let directory = tempfile::tempdir().unwrap();
        let mut ws = topo_core::Workspace::init(directory.path()).unwrap();
        ws.graph = fixture(500);
        let (app, visual) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
        visual.simulate_resize(size(px(1360.), px(860.)));
        for _ in 0..3 {
            app.update(visual, |_, cx| cx.notify());
            visual.run_until_parked();
        }
        for invalidate in [true, false] {
            for motion in ["pan", "zoom", "hover"] {
                let mut samples = Vec::new();
                for frame in 0..40 {
                    let started = Instant::now();
                    app.update(visual, |app, cx| {
                        app.anim = None;
                        if invalidate {
                            app.graph_cache.invalidate();
                        }
                        match motion {
                            "pan" => app.offset.x += px(if frame % 2 == 0 { 8. } else { -8. }),
                            "zoom" => app.zoom = if frame % 2 == 0 { 0.9 } else { 1.0 },
                            _ => app.hovered = Some(NodeId(format!("n{:04}", frame % 25))),
                        }
                        cx.notify();
                    });
                    visual.run_until_parked();
                    samples.push(started.elapsed().as_secs_f64() * 1000.);
                }
                samples.sort_by(f64::total_cmp);
                eprintln!(
                    "500 nodes {motion}, recompute={invalidate}: median {:.3} ms, p95 {:.3} ms",
                    samples[20], samples[38]
                );
            }
        }
    }
}
