//! The canvas: dot grid, edges, node cards and the gestures on them.

use std::collections::{BTreeMap, BTreeSet};

use gpui::{
    AnyElement, Bounds, Context, CursorStyle, ElementId, Hsla, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PathBuilder, Pixels, Point, ScrollWheelEvent, Window, canvas, div, fill, point, prelude::*, px, rgb,
    size,
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

/// `id` together with everything it transitively requires and everything that requires it.
fn chain(graph: &Graph, id: &NodeId) -> BTreeSet<NodeId> {
    graph.descendants(id).into_iter().chain(graph.ancestors(id)).chain([id.clone()]).collect()
}

impl TopoApp {
    /// The node whose card contains `local` (canvas coordinates).
    fn node_at(&self, local: Point<Pixels>, cells: &BTreeMap<NodeId, layout::Cell>) -> Option<NodeId> {
        let size = size(px(NODE_W * self.zoom), px(NODE_H * self.zoom));
        cells
            .iter()
            .find(|(_, cell)| Bounds::new(self.to_screen(**cell), size).contains(&local))
            .map(|(id, _)| id.clone())
    }

    pub(crate) fn on_drag_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.pressed_button != Some(MouseButton::Left) {
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
            None => return,
        }
        cx.notify();
    }

    pub(crate) fn on_drag_end(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        match self.drag.take() {
            Some(Drag::Pan { moved: false, on_card: false, .. }) => self.selected = None,
            Some(Drag::Link { source, .. }) => {
                let area = self.area.get();
                let cells = layout::layout(self.graph());
                match self.node_at(event.position - area.origin, &cells) {
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
        let graph = &self.ws.graph;
        let cells = layout::layout(graph);
        let today = dates::today();
        let z = self.zoom;
        let origin = self.area.get().origin;

        let focus = self.selected.as_ref().filter(|id| graph.get(id).is_some()).map(|id| chain(graph, id));
        let matches: Option<BTreeSet<NodeId>> = self.search_matches(cx).map(|ids| ids.into_iter().collect());
        let cursor = self.list_cursor(cx);
        let critical: BTreeSet<NodeId> = match self.selected_node() {
            Some(n) if n.kind == Kind::Milestone => graph.critical_path(&n.id).into_iter().collect(),
            _ => BTreeSet::new(),
        };
        let emphasized = |id: &NodeId| match (&matches, &focus) {
            (Some(m), _) => m.contains(id),
            (None, Some(f)) => f.contains(id),
            (None, None) => true,
        };

        let link = match &self.drag {
            Some(Drag::Link { source, mouse }) => Some((source.clone(), *mouse - origin)),
            _ => None,
        };
        let over = link.as_ref().and_then(|(_, mouse)| self.node_at(*mouse, &cells));
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
        let selected_id = self.selected.clone();
        let edge = |from: &NodeId, to: &NodeId, membership: bool| {
            let closed = graph.get(from).expect("graph invariant").status.is_closed();
            let on_focus = focus.as_ref().is_some_and(|f| f.contains(from) && f.contains(to));
            let on_critical = critical.contains(from) && (critical.contains(to) || Some(to) == selected_id.as_ref());
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
            .map(|n| {
                let state = CardState {
                    selected: self.selected.as_ref() == Some(&n.id),
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
        let painter = canvas(
            move |bounds, _, _| area.set(bounds),
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
                        app.drag = Some(Drag::Pan { last: ev.position, moved: false, on_card: false });
                        cx.notify();
                    }
                }),
            )
            .on_scroll_wheel(cx.listener(|app, ev: &ScrollWheelEvent, _, cx| {
                let delta = ev.delta.pixel_delta(px(20.));
                if ev.modifiers.platform || ev.modifiers.control {
                    let factor = (1.0 + f32::from(delta.y) / 300.0).clamp(0.5, 2.0);
                    app.zoom_by(factor, ev.position - app.area.get().origin, false);
                } else {
                    app.anim = None;
                    app.offset += delta;
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
        let graph = self.graph();
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
        let meta = div()
            .flex()
            .items_center()
            .gap(px(6. * z))
            .pl(px(24. * z))
            .text_size(px(10.5 * z))
            .text_color(rgb(theme::FAINT));
        let meta = match node.kind {
            Kind::Milestone => {
                let (done, total) = graph.progress(&node.id);
                let fraction = if total == 0 { 0. } else { done as f32 / total as f32 };
                let color = if closed || (total > 0 && done == total) { theme::GREEN } else { theme::AMBER };
                meta.child(theme::progress_bar(fraction, color, 4. * z))
                    .child(div().flex_shrink_0().text_color(rgb(theme::MUTED)).child(format!("{done}/{total}")))
                    .children(due)
            }
            Kind::Task => {
                let open_reqs = graph
                    .requirements(node)
                    .iter()
                    .filter(|r| !graph.get(r).expect("graph invariant").status.is_closed())
                    .count();
                let status = match node.status {
                    Status::Doing => mini("In progress".into(), theme::ACCENT),
                    Status::Done => mini("Done".into(), theme::GREEN),
                    Status::Dropped => mini("Dropped".into(), theme::FAINT),
                    Status::Todo if open_reqs == 0 => mini("Ready".into(), theme::GREEN),
                    Status::Todo => div().flex_shrink_0().child(format!("Blocked by {open_reqs}")),
                };
                meta.child(status)
                    .children(due)
                    .children(node.tags.iter().take(2).map(|t| div().flex_shrink_0().child(format!("#{t}"))))
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
            .when_else(state.selected || state.cursor || state.drop.is_some(), |d| d.border_2(), |d| d.border_1())
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
                    window.focus(&app.focus);
                    app.selected = Some(down_id.clone());
                    app.drag = match () {
                        _ if ev.modifiers.shift => Some(Drag::Link { source: down_id.clone(), mouse: ev.position }),
                        _ if ev.click_count >= 2 => {
                            let title = app.title_of(&down_id);
                            app.open_prompt(Prompt::Rename(down_id.clone()), &title, window, cx);
                            None
                        }
                        // Cards are placed by the layout, so dragging one moves the canvas.
                        _ => Some(Drag::Pan { last: ev.position, moved: false, on_card: true }),
                    };
                    cx.notify();
                }),
            );

        // Wider than the card, so the pointer can reach all of the handle without leaving.
        div()
            .id(ElementId::Name(format!("node-{id}").into()))
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

/// Dots every 28 canvas units, thinned out when zoomed far out.
fn paint_grid(window: &mut Window, bounds: Bounds<Pixels>, offset: Point<Pixels>, zoom: f32) {
    let mut step = 28. * zoom;
    while step < 14. {
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
