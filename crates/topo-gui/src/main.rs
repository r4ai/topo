//! `topo-gui`: native graph view of a topo workspace.
//!
//! Click a node to inspect it, shift-drag from a prerequisite onto a dependent
//! to link them, drag the background to pan, scroll to pan (cmd/ctrl+scroll to
//! zoom). Edits made elsewhere (CLI, agents) appear live.

mod layout;

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use gpui::{
    App, Application, Bounds, Context, Hsla, MouseButton, MouseDownEvent, MouseMoveEvent, PathBuilder, Pixels, Point,
    Rgba, ScrollWheelEvent, SharedString, Window, WindowBounds, WindowOptions, canvas, div, point, prelude::*, px, rgb,
    size,
};
use notify::{RecursiveMode, Watcher};
use topo_core::{Graph, Kind, Node, NodeId, Status, Workspace};
use topo_jev::organize::{self, Proposal};
use topo_jev::{Client, Config};

const CELL_W: f32 = 260.0;
const CELL_H: f32 = 84.0;
const NODE_W: f32 = 210.0;
const NODE_H: f32 = 60.0;

const BG: u32 = 0x1e1f24;
const PANEL: u32 = 0x26272e;
const TEXT: u32 = 0xe6e6e6;
const MUTED: u32 = 0x8b8d98;
const EDGE: u32 = 0x5a5d6b;
const ACCENT: u32 = 0x5b9cff;
const MILESTONE: u32 = 0xf2b84b;
const DONE: u32 = 0x4caf7a;

#[derive(Clone, Copy)]
enum OrganizeKind {
    Deps,
    Place,
}

struct TopoApp {
    ws: Workspace,
    offset: Point<Pixels>,
    zoom: f32,
    selected: Option<NodeId>,
    /// Last mouse position while panning with the background dragged.
    panning: Option<Point<Pixels>>,
    /// Source node and current mouse position while shift-dragging a new edge.
    linking: Option<(NodeId, Point<Pixels>)>,
    proposals: Vec<Proposal>,
    busy: bool,
    message: String,
    _watcher: notify::RecommendedWatcher,
}

impl TopoApp {
    fn new(ws: Workspace, cx: &mut Context<Self>) -> Result<Self> {
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_ok() {
                let _ = tx.send(());
            }
        })?;
        watcher.watch(&ws.nodes_dir(), RecursiveMode::NonRecursive)?;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(300)).await;
                if rx.try_iter().count() > 0 && this.update(cx, |app, cx| app.reload(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        Ok(Self {
            ws,
            offset: point(px(40.), px(40.)),
            zoom: 1.0,
            selected: None,
            panning: None,
            linking: None,
            proposals: Vec::new(),
            busy: false,
            message: String::new(),
            _watcher: watcher,
        })
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        match Workspace::open(self.ws.dir().to_owned()) {
            Ok(ws) => self.ws = ws,
            Err(e) => self.message = format!("reload failed: {e}"),
        }
        cx.notify();
    }

    /// Applies a graph mutation and saves it, reporting failures in the status line.
    fn mutate(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Graph) -> Result<(), topo_core::Error>) {
        let result = f(&mut self.ws.graph).and_then(|()| self.ws.save());
        self.message = match result {
            Ok(()) => String::new(),
            Err(e) => e.to_string(),
        };
        cx.notify();
    }

    fn run_organize(&mut self, what: OrganizeKind, cx: &mut Context<Self>) {
        let config = match Config::load(self.ws.dir()) {
            Ok(config) => config,
            Err(e) => {
                self.message = e.to_string();
                return;
            }
        };
        let graph = self.ws.graph.clone();
        self.busy = true;
        self.message = "asking the decision model…".into();
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            let client = Client::new(config);
            match what {
                OrganizeKind::Deps => organize::dependencies(&graph, &client),
                OrganizeKind::Place => organize::placement(&graph, &client, &organize::unplaced_tasks(&graph)),
            }
            .map_err(|e| e.to_string())
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |app, cx| {
                app.busy = false;
                match result {
                    Ok(proposals) => {
                        app.message = format!("{} proposal(s)", proposals.len());
                        app.proposals = proposals;
                    }
                    Err(e) => app.message = e,
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn accept(&mut self, index: usize, cx: &mut Context<Self>) {
        let proposal = self.proposals.remove(index);
        let applied = organize::apply(&mut self.ws.graph, vec![proposal]);
        let result = match applied[0].skipped.clone() {
            Some(why) => Err(why),
            None => self.ws.save().map_err(|e| e.to_string()),
        };
        self.message = result.err().unwrap_or_default();
        cx.notify();
    }

    fn to_screen(&self, cell: layout::Cell) -> Point<Pixels> {
        let (col, row) = cell;
        point(self.offset.x + px(col as f32 * CELL_W * self.zoom), self.offset.y + px(row as f32 * CELL_H * self.zoom))
    }
}

fn status_color(status: Status) -> Rgba {
    match status {
        Status::Todo => rgb(MUTED),
        Status::Doing => rgb(ACCENT),
        Status::Done => rgb(DONE),
        Status::Dropped => rgb(EDGE),
    }
}

fn button(id: impl Into<SharedString>, label: impl Into<SharedString>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.into())
        .px_2()
        .py_1()
        .rounded_md()
        .bg(rgb(0x363843))
        .hover(|s| s.bg(rgb(0x444757)))
        .cursor_pointer()
        .text_sm()
        .child(label.into())
}

impl TopoApp {
    fn node_view(&self, node: &Node, origin: Point<Pixels>, cx: &mut Context<Self>) -> impl IntoElement {
        let id = node.id.clone();
        let (link_id, up_id) = (id.clone(), id.clone());
        let selected = self.selected.as_ref() == Some(&node.id);
        let border = match (selected, node.kind) {
            (true, _) => rgb(TEXT),
            (false, Kind::Milestone) => rgb(MILESTONE),
            (false, Kind::Task) => status_color(node.status),
        };
        let subtitle = match node.kind {
            Kind::Milestone => {
                let (done, total) = self.ws.graph.progress(&node.id);
                format!("◆ {done}/{total}{}", node.due.map(|d| format!(" · due {d}")).unwrap_or_default())
            }
            Kind::Task => format!("{:?}{}", node.status, node.due.map(|d| format!(" · due {d}")).unwrap_or_default()),
        };
        let z = self.zoom;
        div()
            .absolute()
            .left(origin.x)
            .top(origin.y)
            .w(px(NODE_W * z))
            .h(px(NODE_H * z))
            .p(px(6. * z))
            .overflow_hidden()
            .rounded(px(if node.kind == Kind::Milestone { 16. } else { 6. } * z))
            .border_2()
            .border_color(border)
            .bg(rgb(PANEL))
            .opacity(if node.status.is_closed() { 0.5 } else { 1.0 })
            .cursor_pointer()
            .text_size(px(13. * z))
            .text_color(rgb(TEXT))
            .child(div().child(node.title.clone()))
            .child(div().text_size(px(11. * z)).text_color(status_color(node.status)).child(subtitle))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |app, ev: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    match ev.modifiers.shift {
                        true => app.linking = Some((link_id.clone(), ev.position)),
                        false => app.selected = Some(id.clone()),
                    }
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |app, _, _, cx| {
                    if let Some((source, _)) = app.linking.take()
                        && source != up_id
                    {
                        app.mutate(cx, |graph| graph.link(&up_id, &source));
                    }
                }),
            )
    }

    fn panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let graph = &self.ws.graph;
        let mut panel = div()
            .w(px(340.))
            .h_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .bg(rgb(PANEL))
            .text_color(rgb(TEXT))
            .text_sm()
            .overflow_hidden();

        panel = panel.child(
            div()
                .flex()
                .gap_2()
                .child(
                    button("org-deps", "Organize deps")
                        .on_click(cx.listener(|app, _, _, cx| app.run_organize(OrganizeKind::Deps, cx))),
                )
                .child(
                    button("org-place", "Place tasks")
                        .on_click(cx.listener(|app, _, _, cx| app.run_organize(OrganizeKind::Place, cx))),
                )
                .child(button("reset", "Reset view").on_click(cx.listener(|app, _, _, cx| {
                    app.offset = point(px(40.), px(40.));
                    app.zoom = 1.0;
                    cx.notify();
                }))),
        );
        if !self.message.is_empty() || self.busy {
            panel = panel.child(div().text_color(rgb(MUTED)).child(self.message.clone()));
        }

        if let Some(node) = self.selected.as_ref().and_then(|id| graph.get(id)) {
            let id = node.id.clone();
            let mut statuses = div().flex().gap_1();
            for status in [Status::Todo, Status::Doing, Status::Done, Status::Dropped] {
                let id = id.clone();
                let label = format!("{status:?}");
                let b = button(SharedString::from(format!("status-{label}")), label)
                    .when(node.status == status, |b| b.border_1().border_color(status_color(status)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        let id = id.clone();
                        app.mutate(cx, move |graph| graph.set_status(&id, status));
                    }));
                statuses = statuses.child(b);
            }
            panel = panel
                .child(div().text_lg().child(node.title.clone()))
                .child(div().text_color(rgb(MUTED)).child(format!(
                    "{} · {:?}{}{}",
                    node.id,
                    node.kind,
                    node.due.map(|d| format!(" · due {d}")).unwrap_or_default(),
                    node.tags.iter().map(|t| format!(" #{t}")).collect::<String>()
                )))
                .child(statuses);
            if node.kind == Kind::Milestone {
                let (done, total) = graph.progress(&node.id);
                let path: Vec<String> = graph
                    .critical_path(&node.id)
                    .iter()
                    .map(|p| graph.get(p).expect("graph invariant").title.clone())
                    .collect();
                panel = panel
                    .child(format!("progress {done}/{total}"))
                    .child(div().text_color(rgb(MUTED)).child(format!("critical path: {}", path.join(" → "))));
            }
            if !node.body.is_empty() {
                panel = panel.child(div().p_2().rounded_md().bg(rgb(BG)).child(node.body.trim_end().to_owned()));
            }
            panel = panel.child(div().text_color(rgb(MUTED)).child("depends on"));
            for dep in &node.depends_on {
                let (from, to) = (id.clone(), dep.clone());
                let title = graph.get(dep).expect("graph invariant").title.clone();
                panel = panel.child(div().flex().gap_2().items_center().child(div().flex_1().child(title)).child(
                    button(SharedString::from(format!("unlink-{dep}")), "×").on_click(cx.listener(
                        move |app, _, _, cx| {
                            let (from, to) = (from.clone(), to.clone());
                            app.mutate(cx, move |graph| graph.unlink(&from, &to));
                        },
                    )),
                ));
            }
            let needed_by: Vec<String> = graph.dependents(&node.id).map(|n| n.title.clone()).collect();
            if !needed_by.is_empty() {
                panel = panel
                    .child(div().text_color(rgb(MUTED)).child("needed by"))
                    .child(div().child(needed_by.join(", ")));
            }
        } else {
            panel = panel.child(div().text_color(rgb(MUTED)).child(
                "Click a node for details. Shift-drag from a prerequisite onto a dependent to link them. \
                 Add nodes with the `topo` CLI or an agent; changes appear here live.",
            ));
        }

        if !self.proposals.is_empty() {
            panel = panel.child(div().mt_2().text_color(rgb(MUTED)).child("proposals"));
            for (index, proposal) in self.proposals.iter().enumerate() {
                let title = |id: &NodeId| graph.get(id).map_or("?".to_owned(), |n| n.title.clone());
                let text = match proposal {
                    Proposal::Link { from, to, probability } => {
                        format!("{probability:.2} {} ← {}", title(from), title(to))
                    }
                    Proposal::SetKind { id, kind, probability } => format!("{probability:.2} {} → {kind:?}", title(id)),
                    Proposal::Duplicate { a, b, probability } => {
                        format!("{probability:.2} {} ≈ {}", title(a), title(b))
                    }
                };
                panel = panel.child(
                    div()
                        .flex()
                        .gap_1()
                        .items_center()
                        .child(div().flex_1().child(text))
                        .child(
                            button(SharedString::from(format!("accept-{index}")), "✓")
                                .on_click(cx.listener(move |app, _, _, cx| app.accept(index, cx))),
                        )
                        .child(button(SharedString::from(format!("reject-{index}")), "×").on_click(cx.listener(
                            move |app, _, _, cx| {
                                app.proposals.remove(index);
                                cx.notify();
                            },
                        ))),
                );
            }
        }
        panel
    }
}

impl Render for TopoApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let cells = layout::layout(&self.ws.graph);
        let z = self.zoom;
        let anchor = |cell: layout::Cell, right: bool| {
            let p = self.to_screen(cell);
            point(p.x + px(if right { NODE_W * z } else { 0. }), p.y + px(NODE_H * z / 2.))
        };
        // Edges run from the right side of a prerequisite to the left side of its dependent.
        let edges: Vec<(Point<Pixels>, Point<Pixels>, bool)> = self
            .ws
            .graph
            .nodes()
            .flat_map(|n| {
                n.depends_on.iter().map(|d| {
                    (
                        anchor(cells[d], true),
                        anchor(cells[&n.id], false),
                        self.ws.graph.get(d).unwrap().status.is_closed(),
                    )
                })
            })
            .collect();
        let pending_link = self.linking.as_ref().map(|(source, mouse)| (anchor(cells[source], true), *mouse));

        let nodes: Vec<gpui::AnyElement> = self
            .ws
            .graph
            .nodes()
            .map(|n| self.node_view(n, self.to_screen(cells[&n.id]), cx).into_any_element())
            .collect();

        let graph_area = div()
            .relative()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .bg(rgb(BG))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds: Bounds<Pixels>, _, window, _| {
                        let o = bounds.origin;
                        for (from, to, closed) in edges {
                            let (from, to) = (from + o, to + o);
                            let dx = (to.x - from.x) / 2.;
                            let mut path = PathBuilder::stroke(px(1.5));
                            path.move_to(from);
                            path.cubic_bezier_to(to, point(from.x + dx, from.y), point(to.x - dx, to.y));
                            if let Ok(path) = path.build() {
                                let color: Hsla = rgb(if closed { 0x3a3c46 } else { EDGE }).into();
                                window.paint_path(path, color);
                            }
                            let mut head = PathBuilder::fill();
                            head.add_polygon(
                                &[to, point(to.x - px(8.), to.y - px(4.)), point(to.x - px(8.), to.y + px(4.))],
                                true,
                            );
                            if let Ok(head) = head.build() {
                                window.paint_path(head, rgb(EDGE));
                            }
                        }
                        if let Some((from, mouse)) = pending_link {
                            let mut path = PathBuilder::stroke(px(2.)).dash_array(&[px(6.), px(4.)]);
                            path.move_to(from + o);
                            path.line_to(mouse);
                            if let Ok(path) = path.build() {
                                window.paint_path(path, rgb(ACCENT));
                            }
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .children(nodes)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, ev: &MouseDownEvent, _, cx| {
                    app.panning = Some(ev.position);
                    app.selected = None;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|app, ev: &MouseMoveEvent, _, cx| {
                if let Some((_, mouse)) = &mut app.linking {
                    *mouse = ev.position;
                    cx.notify();
                } else if let Some(last) = app.panning {
                    app.offset += ev.position - last;
                    app.panning = Some(ev.position);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|app, _, _, cx| {
                    app.panning = None;
                    app.linking = None;
                    cx.notify();
                }),
            )
            .on_scroll_wheel(cx.listener(|app, ev: &ScrollWheelEvent, _, cx| {
                let delta = ev.delta.pixel_delta(px(20.));
                if ev.modifiers.platform || ev.modifiers.control {
                    let factor = (1.0 + f32::from(delta.y) / 200.0).clamp(0.5, 2.0);
                    let zoom = (app.zoom * factor).clamp(0.3, 2.5);
                    // Keep the point under the cursor fixed while zooming.
                    let anchor = ev.position;
                    app.offset = anchor - (anchor - app.offset) * (zoom / app.zoom);
                    app.zoom = zoom;
                } else {
                    app.offset += delta;
                }
                cx.notify();
            }));

        div().size_full().flex().bg(rgb(BG)).font_family(".SystemUIFont").child(graph_area).child(self.panel(cx))
    }
}

fn open_workspace() -> Result<Workspace> {
    Ok(match (std::env::var_os("TOPO_DIR"), std::env::args_os().nth(1)) {
        (Some(dir), _) => Workspace::open(PathBuf::from(dir))?,
        (None, Some(start)) => Workspace::discover(&PathBuf::from(start))?,
        (None, None) => Workspace::discover(&std::env::current_dir()?)?,
    })
}

fn main() -> Result<()> {
    let ws = open_workspace()?;
    let title: SharedString = format!("topo — {}", ws.dir().parent().unwrap_or(ws.dir()).display()).into();
    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui::TitlebarOptions { title: Some(title), ..Default::default() }),
            ..Default::default()
        };
        cx.open_window(options, |_, cx| cx.new(|cx| TopoApp::new(ws, cx).expect("failed to watch the workspace")))
            .expect("failed to open window");
        cx.on_window_closed(|cx| cx.quit()).detach();
        cx.activate(true);
    });
    Ok(())
}
