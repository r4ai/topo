//! The canvas: dot grid, edges, node cards and the gestures on them.

use std::collections::{BTreeMap, BTreeSet};

use gpui::{
    AnyElement, Bounds, Context, CursorStyle, ElementId, Hsla, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PathBuilder, Pixels, Point, ScrollDelta, ScrollWheelEvent, Window, canvas, div, fill, point,
    prelude::*, px, rgb, size,
};
use jiff::civil::Date;
use topo_core::{Graph, Kind, Node, NodeId, Status};

use crate::{Drag, NODE_H, NODE_W, Prompt, TopoApp, dates, layout, theme};

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
    cells: BTreeMap<NodeId, layout::Cell>,
    selected: BTreeSet<NodeId>,
    focus: Option<BTreeSet<NodeId>>,
    critical: BTreeSet<NodeId>,
    critical_edges: BTreeMap<NodeId, BTreeSet<NodeId>>,
    requirements: BTreeMap<NodeId, Vec<NodeId>>,
    dependents: BTreeMap<NodeId, Vec<NodeId>>,
    open_requirements: BTreeMap<NodeId, usize>,
    progress: BTreeMap<NodeId, (usize, usize)>,
}

impl Default for GraphCache {
    fn default() -> Self {
        Self {
            dirty: true,
            cells: BTreeMap::new(),
            selected: BTreeSet::new(),
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
    pub(crate) fn invalidate(&mut self) {
        self.dirty = true;
    }

    pub(crate) fn cells(&self) -> &BTreeMap<NodeId, layout::Cell> {
        &self.cells
    }

    pub(crate) fn refresh(&mut self, graph: &Graph, selected: &BTreeSet<NodeId>) {
        let unchanged = !self.dirty;
        if !unchanged {
            self.dirty = false;
            self.cells = layout::layout(graph);
            self.requirements = graph.nodes().map(|n| (n.id.clone(), n.depends_on.clone())).collect();
            self.dependents = graph.nodes().map(|n| (n.id.clone(), Vec::new())).collect();
            for node in graph.nodes() {
                for milestone in &node.milestones {
                    self.requirements.get_mut(milestone).expect("graph invariant").push(node.id.clone());
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
            self.progress = graph
                .nodes()
                .filter(|n| n.kind == Kind::Milestone)
                .map(|n| (n.id.clone(), graph.progress(&n.id)))
                .collect();
        }
        if !unchanged || self.selected != *selected {
            self.selected.clone_from(selected);
            self.focus = (!selected.is_empty()).then(|| {
                let mut focus = reachable(&self.requirements, selected);
                focus.extend(reachable(&self.dependents, selected));
                focus
            });
            self.critical.clear();
            self.critical_edges.clear();
            for id in selected.iter().filter(|id| graph.get(id).is_some_and(|n| n.kind == Kind::Milestone)) {
                let path = graph.critical_path(id);
                self.critical.extend(path.iter().cloned());
                for pair in path.windows(2) {
                    self.critical_edges.entry(pair[0].clone()).or_default().insert(pair[1].clone());
                }
                if let Some(last) = path.last() {
                    self.critical_edges.entry(last.clone()).or_default().insert(id.clone());
                }
            }
        }
    }
}

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
        self.graph_cache.refresh(&self.ws.graph, &self.selected_nodes);
    }

    /// The node whose card contains `local` (canvas coordinates).
    fn node_at(&self, local: Point<Pixels>, cells: &BTreeMap<NodeId, layout::Cell>) -> Option<NodeId> {
        let size = size(px(NODE_W * self.zoom), px(NODE_H * self.zoom));
        cells
            .iter()
            .find(|(_, cell)| Bounds::new(self.to_screen(**cell), size).contains(&local))
            .map(|(id, _)| id.clone())
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
        match &mut self.drag {
            Some(Drag::Pan { last, moved, .. }) => {
                let delta = event.position - *last;
                *moved |= delta.x.abs() + delta.y.abs() > px(2.);
                *last = event.position;
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
            Some(Drag::Resize) => self.persist_inspector_width(),
            Some(Drag::Link { source, .. }) => {
                let area = self.area.get();
                self.ensure_graph_cache();
                let cells = self.graph_cache.cells();
                match self.node_at(event.position - area.origin, cells) {
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

    pub(crate) fn graph_view(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_graph_cache();
        let graph = &self.ws.graph;
        let cells = self.graph_cache.cells();
        let viewport = Bounds::new(point(px(0.), px(0.)), self.area.get().size);
        let today = dates::today();
        let z = self.zoom;
        let origin = self.area.get().origin;

        let focus = &self.graph_cache.focus;
        let matches: Option<BTreeSet<NodeId>> = self.search_matches(cx).map(|ids| ids.into_iter().collect());
        let cursor = self.list_cursor(cx);
        let critical = &self.graph_cache.critical;
        let filtered = |id: &NodeId| self.passes_filter(graph.get(id).expect("graph invariant"));
        let emphasized = |id: &NodeId| match (&matches, &focus) {
            (Some(m), _) => m.contains(id),
            (None, Some(f)) => f.contains(id) && filtered(id),
            (None, None) => filtered(id),
        };

        let link = match &self.drag {
            Some(Drag::Link { source, mouse }) => Some((source.clone(), *mouse - origin)),
            _ => None,
        };
        let over = link.as_ref().and_then(|(_, mouse)| self.node_at(*mouse, cells));
        let drop = link.as_ref().zip(over.as_ref()).filter(|((source, _), target)| source != *target).map(
            |((source, _), target)| {
                let (label, allowed) = self.connect_preview(source, target);
                (target.clone(), label, allowed)
            },
        );

        // Edges run from the right side of a requirement to the left side of
        // the node requiring it: prerequisite → dependent, member → milestone.
        let anchor = |id: &NodeId, right: bool| {
            let p = self.to_screen(cells[id]);
            point(p.x + px(if right { NODE_W * z } else { 0. }), p.y + px(NODE_H * z / 2.))
        };
        let edge = |from: &NodeId, to: &NodeId, membership: bool| {
            let closed = graph.get(from).expect("graph invariant").status.is_closed();
            let on_focus = focus.as_ref().is_some_and(|f| f.contains(from) && f.contains(to));
            let on_critical = self.graph_cache.critical_edges.get(from).is_some_and(|targets| targets.contains(to));
            let (color, width) = match () {
                _ if !emphasized(from) || !emphasized(to) => (theme::alpha(theme::BORDER_STRONG, 0.25), 1.),
                _ if on_critical => (rgb(theme::AMBER).into(), 2.5),
                _ if on_focus && membership => (theme::alpha(theme::AMBER, 0.9), 2.),
                _ if on_focus => (rgb(theme::ACCENT).into(), 2.),
                _ if closed => (theme::alpha(theme::BORDER_STRONG, 0.7), 1.25),
                _ if membership => (theme::alpha(theme::AMBER, 0.45), 1.25),
                _ => (rgb(0x565a68).into(), 1.5),
            };
            EdgePaint { from: anchor(from, true), to: anchor(to, false), color, width, dashed: membership }
        };
        let mut edges: Vec<EdgePaint> = graph
            .nodes()
            .flat_map(|n| {
                let deps = n.depends_on.iter().map(|d| edge(d, &n.id, false));
                deps.chain(n.milestones.iter().map(|m| edge(&n.id, m, true))).collect::<Vec<_>>()
            })
            .filter(|e| edge_visible(e.from, e.to, viewport, z))
            .collect();
        edges.sort_by(|a, b| a.width.total_cmp(&b.width));
        let pending = link.as_ref().map(|(source, mouse)| {
            let color = match &drop {
                Some((_, _, false)) => theme::RED,
                Some(_) => theme::GREEN,
                None => theme::ACCENT,
            };
            (anchor(source, true), *mouse, color)
        });

        let nodes: Vec<AnyElement> = graph
            .nodes()
            .filter(|n| {
                viewport.size.width <= px(0.)
                    || Bounds::new(self.to_screen(cells[&n.id]), size(px((NODE_W + 8.) * z), px(NODE_H * z)))
                        .intersects(&viewport)
            })
            .map(|n| {
                let state = CardState {
                    selected: self.selected_nodes.contains(&n.id),
                    hovered: self.hovered.as_ref() == Some(&n.id) && self.drag.is_none(),
                    dimmed: !emphasized(&n.id),
                    critical: critical.contains(&n.id),
                    cursor: cursor.as_ref() == Some(&n.id),
                    drop: drop.as_ref().filter(|(t, ..)| *t == n.id).map(|(_, _, allowed)| *allowed),
                };
                self.node_card(n, self.to_screen(cells[&n.id]), state, today, cx)
            })
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
                    paint_edge(window, e.from + o, e.to + o, e.color, e.width, e.dashed, z);
                }
                if let Some((from, mouse, color)) = pending {
                    let mut path = PathBuilder::stroke(px(2.)).dash_array(&[px(6.), px(4.)]);
                    path.move_to(from + o);
                    let (from, to) = (from + o, mouse + o);
                    let dx = ((to.x - from.x) / 2.).max(px(30.));
                    path.cubic_bezier_to(to, point(from.x + dx, from.y), point(to.x - dx, to.y));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, rgb(color));
                    }
                    window.paint_quad(
                        fill(Bounds::centered_at(to, size(px(8.), px(8.))), rgb(color)).corner_radii(px(4.)),
                    );
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
                (Some((_, label, true)), _) => (format!("↳ {label}"), theme::GREEN),
                (Some((_, label, false)), _) => (format!("✕ {label}"), theme::RED),
                (None, Some(_)) => ("Drop on a node to connect, or on empty space".to_owned(), theme::MUTED),
                (None, None) => ("+ New follow-up task".to_owned(), theme::ACCENT),
            };
            div()
                .absolute()
                .left(mouse.x + px(14.))
                .top(mouse.y + px(14.))
                .px_2()
                .py_1()
                .rounded_md()
                .bg(rgb(theme::RAISED))
                .border_1()
                .border_color(theme::alpha(color, 0.6))
                .text_color(rgb(color))
                .text_xs()
                .whitespace_nowrap()
                .shadow(theme::shadow())
                .child(text)
        });

        div()
            .id("canvas")
            .relative()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .bg(rgb(theme::CANVAS))
            .cursor(cursor)
            .child(painter)
            .children(nodes)
            .when(self.graph().nodes().next().is_none(), |d| d.child(self.empty_state(cx)))
            .children(link_label)
            .child(self.zoom_controls(cx))
            .children(self.prompt_overlay(cx))
            .children(self.toast_view())
            .when(self.show_help, |d| d.child(self.help_overlay(cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, ev: &MouseDownEvent, _, cx| {
                    // A click away from an open prompt only dismisses it (the window does that).
                    if app.prompt.is_none() {
                        app.drag = Some(Drag::Pan {
                            last: ev.position,
                            moved: false,
                            on_card: false,
                            button: MouseButton::Left,
                        });
                        cx.notify();
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|app, ev: &MouseDownEvent, _, cx| {
                    if app.prompt.is_none() {
                        app.drag = Some(Drag::Pan {
                            last: ev.position,
                            moved: false,
                            on_card: true,
                            button: MouseButton::Middle,
                        });
                        cx.notify();
                    }
                }),
            )
            .on_scroll_wheel(cx.listener(|app, ev: &ScrollWheelEvent, _, cx| {
                app.anim = None;
                match ev.delta {
                    ScrollDelta::Lines(delta) if ev.modifiers.shift => {
                        app.offset.x += px((delta.y + delta.x) * 40.);
                    }
                    ScrollDelta::Lines(delta) if ev.modifiers.platform || ev.modifiers.control => {
                        app.offset.y += px(delta.y * 40.);
                        app.offset.x += px(delta.x * 40.);
                    }
                    ScrollDelta::Lines(delta) => {
                        app.zoom_by(1.12_f32.powf(delta.y), ev.position - app.area.get().origin, false);
                    }
                    ScrollDelta::Pixels(delta) if ev.modifiers.platform || ev.modifiers.control => {
                        app.zoom_by((f32::from(delta.y) / 300.).exp(), ev.position - app.area.get().origin, false);
                    }
                    ScrollDelta::Pixels(delta) => app.offset += delta,
                }
                cx.notify();
            }))
    }

    fn node_card(
        &self,
        node: &Node,
        pos: Point<Pixels>,
        state: CardState,
        today: Date,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let z = self.zoom;
        let id = node.id.clone();
        let milestone = node.kind == Kind::Milestone;
        let closed = node.status.is_closed();
        let (icon, icon_color) = theme::node_icon(node);
        let border: Hsla = match (state.drop, state.selected || state.cursor) {
            (Some(true), _) => rgb(theme::GREEN).into(),
            (Some(false), _) => rgb(theme::RED).into(),
            (None, true) => rgb(theme::ACCENT).into(),
            _ if state.critical => theme::alpha(theme::AMBER, 0.8),
            _ if milestone => theme::alpha(theme::AMBER, 0.45),
            _ if state.hovered => rgb(theme::BORDER_STRONG).into(),
            _ => rgb(theme::BORDER).into(),
        };
        let bg = match (milestone, state.hovered) {
            (true, false) => rgb(0x221f1b),
            (true, true) => rgb(0x2a2620),
            (false, false) => rgb(theme::CARD),
            (false, true) => rgb(theme::CARD_HOVER),
        };
        let opacity = match () {
            _ if state.dimmed => 0.22,
            _ if closed && !state.selected => 0.6,
            _ => 1.0,
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
                .text_color(rgb(icon_color))
                .text_size(px(13. * z))
                .child(icon)
                .when(!milestone, |d| {
                    d.hover(|s| s.bg(rgb(theme::RAISED))).on_mouse_down(
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
            .when(milestone, |d| d.font_weight(gpui::FontWeight::SEMIBOLD))
            .when(node.status == Status::Done, |d| d.line_through().text_color(rgb(theme::MUTED)))
            .when(node.status == Status::Dropped, |d| d.line_through().text_color(rgb(theme::FAINT)))
            .child(node.title.clone());

        let mini = |label: String, color: u32| {
            div()
                .flex_shrink_0()
                .px(px(5. * z))
                .rounded(px(3. * z))
                .bg(theme::alpha(color, 0.15))
                .text_color(rgb(color))
                .child(label)
        };
        let due = node.due.map(|d| {
            let color = if closed { theme::FAINT } else { dates::urgency_color(d, today) };
            div().flex_shrink_0().text_color(rgb(color)).child(format!("⏱ {}", dates::short(d, today)))
        });
        // Named with a glyph as well as colored. Tags give way to it and to the assignee.
        let priority = node.priority.map(|p| mini(theme::priority_text(p), theme::priority_color(p)));
        let assignee = node.assignee.as_ref().map(|a| div().min_w(px(0.)).truncate().child(format!("@{a}")));
        let tags = 2usize.saturating_sub(usize::from(priority.is_some()) + usize::from(assignee.is_some()));
        let meta = div()
            .flex()
            .items_center()
            .overflow_hidden()
            .gap(px(6. * z))
            .pl(px(24. * z))
            .text_size(px(10.5 * z))
            .text_color(rgb(theme::FAINT));
        let meta = match node.kind {
            Kind::Milestone => {
                let (done, total) = self.graph_cache.progress[&node.id];
                let fraction = if total == 0 { 0. } else { done as f32 / total as f32 };
                let color = if closed || (total > 0 && done == total) { theme::GREEN } else { theme::AMBER };
                meta.child(theme::progress_bar(fraction, color, 4. * z))
                    .child(div().flex_shrink_0().text_color(rgb(theme::MUTED)).child(format!("{done}/{total}")))
                    .children(priority)
                    .children(due)
            }
            Kind::Task => {
                let open_reqs = self.graph_cache.open_requirements[&node.id];
                let status = match node.status {
                    Status::Doing => mini("In progress".into(), theme::ACCENT),
                    Status::Done => mini("Done".into(), theme::GREEN),
                    Status::Dropped => mini("Dropped".into(), theme::FAINT),
                    Status::Todo if open_reqs == 0 => mini("Ready".into(), theme::GREEN),
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
                .bg(rgb(theme::CANVAS))
                .border_2()
                .border_color(rgb(theme::ACCENT))
                .cursor(CursorStyle::Crosshair)
                .hover(|s| s.bg(rgb(theme::ACCENT)))
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
            .rounded(px(if milestone { 14. } else { 8. } * z))
            .border_2()
            .border_color(border)
            .bg(bg)
            .when(state.selected, |d| d.shadow(theme::shadow()))
            .text_size(px(13. * z))
            .cursor_pointer()
            .child(div().flex().items_center().gap(px(6. * z)).child(check).child(title))
            .when(z >= 0.55, |d| d.child(meta))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |app, ev: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    if app.prompt.is_some() {
                        app.close_prompt(window, cx);
                    }
                    window.focus(&app.focus, cx);
                    if ev.modifiers.platform || ev.modifiers.control {
                        app.toggle_selection(down_id.clone());
                        cx.notify();
                        return;
                    }
                    app.select(Some(down_id.clone()), false);
                    app.drag = match () {
                        _ if ev.modifiers.shift => Some(Drag::Link { source: down_id.clone(), mouse: ev.position }),
                        _ if ev.click_count >= 2 => {
                            let title = app.title_of(&down_id);
                            app.open_prompt(Prompt::Rename(down_id.clone()), &title, window, cx);
                            None
                        }
                        // Cards are placed by the layout, so dragging one moves the canvas.
                        _ => Some(Drag::Pan {
                            last: ev.position,
                            moved: false,
                            on_card: true,
                            button: MouseButton::Left,
                        }),
                    };
                    cx.notify();
                }),
            );

        // Wider than the card, so the pointer can reach all of the handle without leaving.
        div()
            .id(ElementId::Name(format!("node-{id}").into()))
            .debug_selector(|| format!("node-{id}"))
            .absolute()
            .left(pos.x)
            .top(pos.y)
            .w(px((NODE_W + 8.) * z))
            .h(px(NODE_H * z))
            .opacity(opacity)
            .child(card)
            .when(state.hovered || state.selected, |d| d.child(handle))
            .on_hover(cx.listener(move |app, hovered: &bool, _, cx| {
                match *hovered {
                    true => app.hovered = Some(id.clone()),
                    false if app.hovered.as_ref() == Some(&id) => app.hovered = None,
                    false => return,
                }
                cx.notify();
            }))
            .into_any_element()
    }
}

/// Dots every 28 canvas units, thinned to a minimum 28 screen pixels at low zoom.
fn paint_grid(window: &mut Window, bounds: Bounds<Pixels>, offset: Point<Pixels>, zoom: f32) {
    paint_grid_at_density(window, bounds, offset, zoom, 28.);
}

fn paint_grid_at_density(window: &mut Window, bounds: Bounds<Pixels>, offset: Point<Pixels>, zoom: f32, minimum: f32) {
    let mut step = 28. * zoom;
    while step < minimum {
        step *= 2.;
    }
    let dot = if zoom > 1.2 { 2. } else { 1.5 };
    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let start = |o: Pixels| f32::from(o).rem_euclid(step);
    let color = rgb(theme::GRID_DOT);
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

fn paint_edge(
    window: &mut Window,
    from: Point<Pixels>,
    to: Point<Pixels>,
    color: Hsla,
    width: f32,
    dashed: bool,
    zoom: f32,
) {
    let head = (7. * zoom).clamp(4., 10.);
    let end = point(to.x - px(head), to.y);
    let dx = ((end.x - from.x) / 2.).max(px(24. * zoom));
    let mut path = PathBuilder::stroke(px(width));
    if dashed {
        path = path.dash_array(&[px(5.), px(4.)]);
    }
    path.move_to(from);
    path.cubic_bezier_to(end, point(from.x + dx, from.y), point(end.x - dx, end.y));
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
        cache.refresh(&graph, &selected);
        assert_eq!(cache.open_requirements[&NodeId("n0025".into())], 1);
        assert!(cache.focus.as_ref().unwrap().contains(&NodeId("n0000".into())));
        assert!(cache.focus.as_ref().unwrap().contains(&NodeId("n0026".into())));
        graph.set_status(&NodeId("n0000".into()), Status::Done).unwrap();
        cache.invalidate();
        cache.refresh(&graph, &selected);
        assert_eq!(cache.open_requirements[&NodeId("n0025".into())], 0);
        let mut extra = Node::new(NodeId("extra".into()), Kind::Task, "Extra".into());
        extra.depends_on.push(NodeId("n0025".into()));
        graph.insert(extra).unwrap();
        cache.invalidate();
        cache.refresh(&graph, &selected);
        assert_eq!(cache.cells[&NodeId("extra".into())].0, 2);
        assert!(cache.focus.as_ref().unwrap().contains(&NodeId("extra".into())));
        assert_eq!(cache.dependents[&NodeId("n0025".into())], vec![NodeId("extra".into())]);
        cache.refresh(&graph, &BTreeSet::new());
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
        cache.refresh(&graph, &selected);
        assert_eq!(cache.focus.as_ref(), Some(&selected));
        assert_eq!(cache.requirements.values().map(Vec::len).sum::<usize>(), 999);
        assert_eq!(cache.dependents.values().map(Vec::len).sum::<usize>(), 999);
    }

    #[test]
    fn selection_focus_ignores_nodes_removed_from_the_graph() {
        let graph = fixture(1);
        let mut cache = GraphCache::default();
        cache.refresh(&graph, &BTreeSet::from([id("missing"), id("n0000")]));
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
        cache.refresh(&graph, &BTreeSet::from([id("left")]));
        assert_eq!(cache.focus, Some(BTreeSet::from([id("root"), id("left"), id("end")])));
        cache.refresh(&graph, &BTreeSet::from([id("left"), id("right")]));
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
        cache.refresh(&graph, &BTreeSet::from([id("a")]));
        assert_eq!(cache.focus, Some(BTreeSet::from([id("a"), id("m"), id("next")])));
        assert_eq!(cache.open_requirements[&id("m")], 2);
        cache.refresh(&graph, &BTreeSet::from([id("m")]));
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
        cache.refresh(&graph, &selected);
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
        cache.refresh(&graph, &selected);
        assert!(!cache.critical_edges.contains_key(&id("b")));
        assert_eq!(cache.critical_edges[&id("c")], BTreeSet::from([id("d"), id("m1")]));
        assert_eq!(cache.open_requirements[&id("m1")], 1);
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
            cache.refresh(&graph, &BTreeSet::new());
            let start = Instant::now();
            let legacy: BTreeSet<_> = selected
                .iter()
                .flat_map(|id| graph.descendants(id).into_iter().chain(graph.ancestors(id)).chain([id.clone()]))
                .collect();
            let legacy_ms = start.elapsed().as_secs_f64() * 1000.;
            let mut samples = Vec::new();
            for _ in 0..40 {
                cache.refresh(&graph, &BTreeSet::new());
                let start = Instant::now();
                cache.refresh(&graph, black_box(&selected));
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
