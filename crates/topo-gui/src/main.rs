//! `topo-gui`: native graph view and editor of a topo workspace.
//!
//! The canvas shows the dependency DAG left to right; the inspector on the
//! right edits the selected node. Everything is reachable from the keyboard
//! (press `?` for the list), every edit can be undone, and edits made
//! elsewhere (CLI, agents) appear live.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod args;
mod branding;
mod chrome;
mod clipboard;
mod combobox;
mod config;
mod dates;
mod gesture;
mod graph_view;
mod inline;
mod inspector;
mod layout;
mod markdown;
mod notes;
#[cfg(test)]
mod perf_tests;
mod persistence;
mod polling;
mod repository;
#[cfg(feature = "screenshot")]
mod screenshot;
mod text_input;
mod theme;

use futures::channel::mpsc;
use std::cell::Cell;
use std::collections::BTreeSet;
#[cfg(feature = "screenshot")]
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::Result;
use gpui::{
    App, Application, Bounds, Context, Entity, FocusHandle, Focusable, KeyBinding, KeyDownEvent, Menu, MenuItem,
    MouseButton, OsAction, Pixels, Point, ScrollHandle, SharedString, Subscription, Window, WindowBounds,
    WindowOptions, actions, div, point, prelude::*, px, rgb, size,
};
use notify::{RecursiveMode, Watcher};
use topo_core::wire::Snapshot;
use topo_core::{Edit, Graph, Kind, Node, NodeId, Priority, Status, Workspace};
use topo_jev::organize::{self, Proposal};
use topo_jev::{Client, Config};

use futures::StreamExt;

use crate::combobox::{Choice, ComboEvent, Combobox};
use crate::gesture::Gesture;
use crate::inline::{Field, InlineEdit};
use crate::text_input::TextInput;

/// Grid pitch and card size in canvas units (multiplied by the zoom on screen).
const CELL_W: f32 = 280.0;
const CELL_H: f32 = 92.0;
const NODE_W: f32 = 232.0;
const NODE_H: f32 = 66.0;
const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 2.5;
const UNDO_LIMIT: usize = 200;
/// How often a cloud workspace is asked whether it changed.
const REMOTE_POLL: Duration = Duration::from_secs(5);

actions!(topo, [Quit, CloseWindow]);

#[derive(Clone, Copy)]
enum OrganizeKind {
    Deps,
    Place,
}

enum Drag {
    /// The canvas dragged by its background or by a card; `last` is the
    /// previous mouse position. A background press that never moved is a
    /// click, which clears the selection.
    Pan { last: Point<Pixels>, moved: bool, on_card: bool, button: MouseButton },
    /// Shift-drag or handle-drag from `source`; `mouse` is in window coordinates.
    Link { source: NodeId, mouse: Point<Pixels> },
    /// The inspector's left border dragged to resize it.
    Resize,
}

/// How a node picked from a list relates to the node it is picked for.
#[derive(Clone, Copy, PartialEq)]
enum Relation {
    /// The node requires the picked one.
    Requires,
    /// The picked one requires the node.
    NeededBy,
    /// The (task) node joins the picked milestone.
    InMilestone,
    /// The picked task joins the (milestone) node.
    Member,
}

/// A node about to be created from the prompt.
#[derive(Clone)]
struct NewNode {
    kind: Kind,
    milestones: Vec<NodeId>,
    depends_on: Vec<NodeId>,
    /// Existing node that will depend on the new one.
    required_by: Option<NodeId>,
}

/// What the floating text prompt is currently asking for.
#[derive(Clone)]
enum Prompt {
    Search,
    /// Connect `node` to one of `candidates`, the nodes it can be connected to that way.
    Pick {
        node: NodeId,
        relation: Relation,
        candidates: Vec<NodeId>,
    },
    Create(NewNode),
}

impl Prompt {
    fn title(&self) -> &'static str {
        match self {
            Prompt::Search => "Jump to",
            Prompt::Pick { relation: Relation::Requires, .. } => "Add a prerequisite of",
            Prompt::Pick { relation: Relation::NeededBy, .. } => "Add a node that requires",
            Prompt::Pick { relation: Relation::InMilestone, .. } => "Add to a milestone",
            Prompt::Pick { relation: Relation::Member, .. } => "Add a member to",
            Prompt::Create(NewNode { kind: Kind::Milestone, .. }) => "New milestone",
            Prompt::Create(NewNode { required_by: Some(_), .. }) => "New prerequisite",
            Prompt::Create(NewNode { depends_on, .. }) if !depends_on.is_empty() => "New follow-up task",
            Prompt::Create(_) => "New task",
        }
    }

    fn placeholder(&self) -> &'static str {
        match self {
            Prompt::Search | Prompt::Pick { .. } => "Search by title, #tag or id…",
            Prompt::Create(NewNode { kind: Kind::Milestone, .. }) => "Milestone title",
            Prompt::Create(_) => "What needs to be done?",
        }
    }
}

struct Toast {
    text: String,
    error: bool,
}

/// An in-flight camera move towards `to` (offset, zoom).
struct Anim {
    from: (Point<Pixels>, f32),
    to: (Point<Pixels>, f32),
    start: Instant,
}

#[derive(Clone, Copy)]
enum Direction {
    Left,
    Right,
    Up,
    Down,
}

struct TopoApp {
    ws: Workspace,
    persistence: persistence::Persistence,
    focus: FocusHandle,
    offset: Point<Pixels>,
    zoom: f32,
    anim: Option<Anim>,
    /// Bounds of the canvas in window coordinates, recorded while painting.
    area: Rc<Cell<Bounds<Pixels>>>,
    /// Fit the whole graph once the canvas size is known.
    pending_fit: bool,
    selected: Option<NodeId>,
    selected_nodes: BTreeSet<NodeId>,
    /// The card the selection was last put on by the pointer or the arrow keys: with
    /// grouping a node has one card per group, and arrow keys move from this one.
    placed: Option<layout::Cell>,
    graph_cache: graph_view::GraphCache,
    hovered: Option<NodeId>,
    drag: Option<Drag>,
    prompt: Option<Prompt>,
    /// The field and list of the prompt.
    palette: Entity<Combobox>,
    /// The node whose notes are being edited in the inspector.
    notes: Option<NodeId>,
    notes_input: Entity<TextInput>,
    /// The notes as the editor opened with them, to tell whether they changed.
    notes_base: String,
    /// The question of what to do with unsaved notes, while it is open.
    notes_ask: Option<notes::Then>,
    /// The scroll position of the inspector, which the notes editor keeps its cursor within.
    inspector_scroll: ScrollHandle,
    /// The property of the selected node being edited in its inspector row.
    inline: Option<InlineEdit>,
    combo: Entity<Combobox>,
    /// Nodes below this priority, and nodes without one, are dimmed.
    priority_filter: Option<Priority>,
    /// Which nodes the canvas shows and how it groups them.
    view: layout::View,
    show_help: bool,
    /// Narrow window: the toolbar and inspector drop secondary text.
    compact: bool,
    /// Width of the inspector in pixels; `None` follows the window proportion.
    inspector_width: Option<f32>,
    /// The window width the inspector width was last clamped against.
    viewport_width: f32,
    undo: Vec<Graph>,
    redo: Vec<Graph>,
    proposals: Vec<Proposal>,
    busy: bool,
    toast: Option<Toast>,
    toast_serial: u64,
    /// Watches the Markdown files. A cloud workspace is polled instead.
    _watcher: Option<notify::RecommendedWatcher>,
    /// Dropping a workspace cancels its scheduler, including inactive waits.
    _sync_task: Option<gpui::Task<()>>,
    _date_task: Option<gpui::Task<()>>,
    retired: bool,
    poll_active: bool,
    poll_visible: bool,
    poll_wake: Option<futures::channel::mpsc::UnboundedSender<()>>,
    _subscriptions: Vec<Subscription>,
    _gesture_monitor: Option<gesture::Monitor>,
    _gesture_task: Option<gpui::Task<()>>,
}

/// A watcher (absent when inert) and the channel it signals.
pub(crate) type Updates = (Option<notify::RecommendedWatcher>, mpsc::Receiver<()>);

impl TopoApp {
    #[cfg(test)]
    fn new(ws: Workspace, window: &mut Window, cx: &mut Context<Self>) -> Result<Self> {
        Self::with_inspector_width(ws, None, window, cx)
    }

    #[cfg(any(test, feature = "screenshot"))]
    fn with_inspector_width(
        ws: Workspace,
        inspector_width: Option<f32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Self> {
        let source = Self::updates(&ws)?;
        Ok(Self::with_source(ws, inspector_width, layout::View::default(), source, window, cx))
    }

    fn with_source(
        ws: Workspace,
        inspector_width: Option<f32>,
        view: layout::View,
        source: Option<Updates>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor = cx.entity().downgrade();
        window.on_window_should_close(cx, move |_, cx| {
            editor.update(cx, |app, cx| app.can_leave_to(notes::Then::CloseWindow, cx)).unwrap_or(true)
        });
        let active = window.is_window_active();
        let (watcher, sync_task, poll_wake) = match source {
            None => {
                let (wake, task) = Self::poll_remote(cx);
                (None, task, Some(wake))
            }
            Some((watcher, rx)) => (watcher, Self::watch_files(rx, cx), None),
        };
        let (mut gestures, gesture_monitor) = gesture::watch();
        let gesture_task = cx.spawn_in(window, async move |this, cx| {
            while let Some(gesture) = gestures.next().await {
                if this.update_in(cx, |app, window, cx| app.on_gesture(gesture, window, cx)).is_err() {
                    break;
                }
            }
        });
        let palette = cx.new(Combobox::palette);
        let inspector_scroll = ScrollHandle::new();
        let notes_input = cx.new(|cx| TextInput::markdown(inspector_scroll.clone(), cx));
        let combo = cx.new(Combobox::new);
        let subscriptions = vec![
            cx.subscribe_in(&palette, window, Self::on_palette_event),
            cx.subscribe_in(&notes_input, window, Self::on_notes_event),
            cx.subscribe_in(&combo, window, Self::on_combo_event),
            cx.observe_window_activation(window, |app, window, _| {
                app.set_poll_active(window.is_window_active());
            }),
        ];
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            persistence: persistence::Persistence::new(ws.clone()),
            ws,
            focus,
            offset: point(px(40.), px(40.)),
            zoom: 1.0,
            anim: None,
            area: Rc::new(Cell::new(Bounds::default())),
            pending_fit: true,
            selected: None,
            selected_nodes: BTreeSet::new(),
            placed: None,
            graph_cache: graph_view::GraphCache::default(),
            hovered: None,
            drag: None,
            prompt: None,
            palette,
            notes: None,
            notes_input,
            notes_base: String::new(),
            notes_ask: None,
            inspector_scroll,
            inline: None,
            combo,
            priority_filter: None,
            view,
            show_help: false,
            compact: false,
            inspector_width,
            viewport_width: 0.,
            undo: Vec::new(),
            redo: Vec::new(),
            proposals: Vec::new(),
            busy: false,
            toast: None,
            toast_serial: 0,
            _watcher: watcher,
            _sync_task: Some(sync_task),
            _date_task: Some(Self::date_clock(cx)),
            retired: false,
            poll_active: active,
            poll_visible: true,
            poll_wake,
            _subscriptions: subscriptions,
            _gesture_monitor: Some(gesture_monitor),
            _gesture_task: Some(gesture_task),
        }
    }

    /// Invalidates the old target immediately, even if a previous frame still owns it.
    fn retire(&mut self) {
        self.retired = true;
        self._sync_task.take();
        self._date_task.take();
        self._watcher.take();
        self._gesture_task.take();
        self._gesture_monitor.take();
        self._subscriptions.clear();
    }

    fn graph(&self) -> &Graph {
        &self.ws.graph
    }

    // ---- inspector width ----------------------------------------------

    /// The current inspector width: the saved value, or the window proportion.
    pub(crate) fn inspector_width(&self) -> f32 {
        self.inspector_width.unwrap_or_else(|| config::default_inspector_width(self.viewport_width))
    }

    /// Makes an explicit width take effect, clamped to the current window.
    fn set_inspector_width(&mut self, width: f32) {
        self.inspector_width = Some(config::clamp_inspector_width(width, self.viewport_width));
    }

    /// The width a pointer at `position` asks for while dragging the left border:
    /// the distance from the pointer to the window's right edge, which the
    /// inspector is flush against.
    fn resized_width(&self, position: Point<Pixels>) -> f32 {
        self.viewport_width - f32::from(position.x)
    }

    /// Saves the width so the next launch starts with the same panel.
    #[cfg_attr(test, allow(unused_variables))]
    fn persist_inspector_width(&self) {
        #[cfg(not(test))]
        if let Some(width) = self.inspector_width {
            let mut config = config::UserConfig::load();
            config.inspector_width = Some(width);
            let _ = config.save();
        }
    }

    /// Saves the display toggles so the next launch starts with the same view.
    /// Collapsed groups are not saved: they belong to one graph's tags.
    #[cfg_attr(test, allow(unused_variables))]
    fn persist_view(&self) {
        #[cfg(not(test))]
        {
            let mut config = config::UserConfig::load();
            config.hide_completed = self.view.hide_completed;
            config.group_by_tag = self.view.group_by_tag;
            let _ = config.save();
        }
    }

    /// Clamps an explicit width when the window shrinks below what it needs.
    fn clamp_inspector_width_to_viewport(&mut self) {
        if let Some(width) = self.inspector_width {
            self.inspector_width = Some(config::clamp_inspector_width(width, self.viewport_width));
        }
    }

    fn selected_node(&self) -> Option<&Node> {
        if self.selected_nodes.len() != 1 {
            return None;
        }
        self.selected.as_ref().and_then(|id| self.ws.graph.get(id))
    }

    fn title_of(&self, id: &NodeId) -> String {
        self.ws.graph.get(id).map_or_else(|| id.to_string(), |n| n.title.clone())
    }

    // ---- persistence ---------------------------------------------------

    /// How the view hears about changes: file events for a local workspace,
    /// `None` (polling) for a cloud one.
    pub(crate) fn updates(ws: &Workspace) -> Result<Option<Updates>> {
        if ws.remote().is_some() {
            return Ok(None);
        }
        // gpui's test scheduler must not see the OS watcher thread, so tests
        // get a source that never fires. `file_watcher` has its own test.
        #[cfg(test)]
        return Ok(Some((None, mpsc::channel(1).1)));
        #[cfg(not(test))]
        Self::file_watcher(ws).map(|(watcher, rx)| Some((Some(watcher), rx)))
    }

    /// Reloads the graph whenever a Markdown file changes.
    fn file_watcher(ws: &Workspace) -> Result<(notify::RecommendedWatcher, mpsc::Receiver<()>)> {
        let (mut tx, rx) = mpsc::channel(1);
        let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_ok() {
                let _ = tx.try_send(());
            }
        })?;
        watcher.watch(&ws.nodes_dir(), RecursiveMode::NonRecursive)?;
        Ok((watcher, rx))
    }

    fn watch_files(mut rx: mpsc::Receiver<()>, cx: &mut Context<Self>) -> gpui::Task<()> {
        cx.spawn(async move |this, cx| {
            while rx.next().await.is_some() {
                cx.background_executor().timer(Duration::from_millis(300)).await;
                while rx.try_recv().is_ok() {}
                if this.update(cx, |app, cx| app.reload(cx)).is_err() {
                    break;
                }
            }
        })
    }

    /// Takes over what a poll of the server returned.
    fn fetched(&mut self, fetched: Result<Option<Snapshot>, topo_core::Error>, cx: &mut Context<Self>) {
        if self.retired {
            return;
        }
        match fetched {
            Ok(Some(snapshot)) => self.receive_snapshot(snapshot, cx),
            Ok(None) => {}
            Err(e) => self.toast(format!("Reload failed: {e}"), true, cx),
        }
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.reload_background(cx);
    }

    fn prune_selection(&mut self) {
        self.selected_nodes.retain(|id| self.ws.graph.get(id).is_some());
        if self.selected.as_ref().is_none_or(|id| !self.selected_nodes.contains(id)) {
            self.selected = self.selected_nodes.first().cloned();
        }
        self.drag = None;
    }

    /// Records `before` as an undo step, unless the edit changed nothing.
    fn record_undo(&mut self, before: Graph) {
        if before.same_content(&self.ws.graph) {
            return;
        }
        self.graph_cache.changed(&before, &self.ws.graph);
        self.undo.push(before);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Validate and display an edit immediately, then enqueue its persistence.
    fn mutate(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Graph) -> Result<(), topo_core::Error>) -> bool {
        let before = self.ws.graph.clone();
        if let Err(e) = f(&mut self.ws.graph) {
            self.ws.graph = before;
            self.toast(e.to_string(), true, cx);
            return false;
        }
        self.enqueue_save(&before, cx);
        self.record_undo(before);
        if self.prompt.is_some() {
            self.refresh_palette(cx);
        }
        cx.notify();
        true
    }

    /// Undo/redo are ordered edits, including when the original write is pending.
    fn restore(&mut self, undo: bool, cx: &mut Context<Self>) {
        let Some(graph) = (if undo { &mut self.undo } else { &mut self.redo }).pop() else {
            return self.toast(if undo { "Nothing to undo" } else { "Nothing to redo" }, false, cx);
        };
        let current = std::mem::replace(&mut self.ws.graph, graph);
        self.graph_cache.changed(&current, &self.ws.graph);
        self.enqueue_save(&current, cx);
        (if undo { &mut self.redo } else { &mut self.undo }).push(current);
        self.prune_selection();
        self.toast(if undo { "Undone" } else { "Redone" }, false, cx);
    }

    fn toast(&mut self, text: impl Into<String>, error: bool, cx: &mut Context<Self>) {
        self.toast = Some(Toast { text: text.into(), error });
        self.toast_serial += 1;
        let serial = self.toast_serial;
        let delay = Duration::from_millis(if error { 6000 } else { 2500 });
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            let _ = this.update(cx, |app, cx| {
                if app.toast_serial == serial {
                    app.toast = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    // ---- node edits ----------------------------------------------------

    fn set_status(&mut self, id: NodeId, status: Status, cx: &mut Context<Self>) {
        self.mutate(cx, |graph| graph.set_status(&id, status));
    }

    fn toggle_done(&mut self, id: NodeId, cx: &mut Context<Self>) {
        let Some(status) = self.graph().get(&id).map(|n| n.status) else { return };
        self.set_status(id, if status == Status::Done { Status::Todo } else { Status::Done }, cx);
    }

    fn delete(&mut self, id: NodeId, cx: &mut Context<Self>) {
        let title = self.title_of(&id);
        if self.mutate(cx, |graph| graph.remove(&id).map(drop)) {
            self.clear_selection();
            self.hovered = None;
            self.toast(format!("Deleted “{title}” — ⌘Z to undo"), false, cx);
        }
    }

    /// Turns a task into a milestone or back.
    fn convert(&mut self, id: NodeId, cx: &mut Context<Self>) {
        let Some(kind) = self.graph().get(&id).map(|n| n.kind) else { return };
        let kind = match kind {
            Kind::Task => Kind::Milestone,
            Kind::Milestone => Kind::Task,
        };
        self.mutate(cx, |graph| graph.edit(&id, Edit { kind: Some(kind), ..Edit::default() }));
    }

    /// Whether [`Self::convert`] is possible: membership ties a node to its kind.
    fn convertible(&self, node: &Node) -> bool {
        node.milestones.is_empty() && self.graph().members(&node.id).next().is_none()
    }

    /// Opens the Markdown file of `id` in the default editor; saving there shows up live.
    fn open_file(&mut self, id: &NodeId, cx: &mut Context<Self>) {
        if self.ws.remote().is_some() {
            return self.toast("A cloud workspace has no files to open", true, cx);
        }
        cx.open_with_system(&self.ws.node_path(id));
        self.toast("Opened the file — edits appear here when saved", false, cx);
    }

    fn create(&mut self, new: NewNode, title: String, cx: &mut Context<Self>) -> Option<NodeId> {
        let id = self.graph().fresh_id();
        let mut node = Node::new(id.clone(), new.kind, title);
        node.depends_on = new.depends_on;
        node.milestones = new.milestones;
        let ok = self.mutate(cx, |graph| {
            graph.insert(node)?;
            match &new.required_by {
                Some(later) => graph.link(later, &id),
                None => Ok(()),
            }
        });
        ok.then_some(id)
    }

    /// Links `source` to `target` the way a drag from one onto the other means:
    /// a task dropped on a milestone joins it, otherwise the target starts to
    /// depend on the source.
    fn connect(graph: &mut Graph, source: &NodeId, target: &NodeId) -> Result<(), topo_core::Error> {
        let is = |id: &NodeId, kind| graph.get(id).is_some_and(|n| n.kind == kind);
        match is(source, Kind::Task) && is(target, Kind::Milestone) {
            true => graph.join(source, target),
            false => graph.link(target, source),
        }
    }

    /// Text describing what dropping `source` on `target` would do, and whether it is allowed.
    fn connect_preview(&self, source: &NodeId, target: &NodeId) -> (String, bool) {
        let mut graph = self.graph().clone();
        let joins = graph.get(target).is_some_and(|n| n.kind == Kind::Milestone)
            && graph.get(source).is_some_and(|n| n.kind == Kind::Task);
        let label = match joins {
            true => "Add to milestone".to_owned(),
            false => format!("Requires “{}”", self.title_of(source)),
        };
        match Self::connect(&mut graph, source, target) {
            Ok(()) if graph == self.ws.graph => ("Already connected".into(), false),
            Ok(()) => (label, true),
            Err(topo_core::Error::WouldCycle { .. }) => ("Would create a cycle".into(), false),
            Err(e) => (e.to_string(), false),
        }
    }

    fn finish_link(&mut self, source: NodeId, target: NodeId, cx: &mut Context<Self>) {
        match self.connect_preview(&source, &target) {
            (_, true) => {
                self.mutate(cx, |graph| Self::connect(graph, &source, &target));
                // The layout just changed under the pointer; keep the result in view.
                self.select(Some(target), true);
            }
            (why, false) => self.toast(why, true, cx),
        }
    }

    /// Nodes that `node` can be connected to by `relation` without a duplicate or a cycle.
    fn candidates(&self, node: &Node, relation: Relation) -> Vec<NodeId> {
        let graph = self.graph();
        let (below, above) = (graph.descendants(&node.id), graph.ancestors(&node.id));
        graph
            .nodes()
            .filter(|n| n.id != node.id)
            .filter(|n| match relation {
                Relation::Requires => !node.depends_on.contains(&n.id) && !above.contains(&n.id),
                Relation::NeededBy => !n.depends_on.contains(&node.id) && !below.contains(&n.id),
                Relation::InMilestone => {
                    n.kind == Kind::Milestone && !node.milestones.contains(&n.id) && !below.contains(&n.id)
                }
                Relation::Member => n.kind == Kind::Task && !n.milestones.contains(&node.id) && !above.contains(&n.id),
            })
            .map(|n| n.id.clone())
            .collect()
    }

    fn relate(graph: &mut Graph, node: &NodeId, relation: Relation, picked: &NodeId) -> Result<(), topo_core::Error> {
        match relation {
            Relation::Requires => graph.link(node, picked),
            Relation::NeededBy => graph.link(picked, node),
            Relation::InMilestone => graph.join(node, picked),
            Relation::Member => graph.join(picked, node),
        }
    }

    /// Opens the list of nodes that the selection can be connected to by `relation`.
    fn prompt_pick(&mut self, relation: Relation, window: &mut Window, cx: &mut Context<Self>) {
        let Some(node) = self.selected_node() else { return };
        let candidates = self.candidates(node, relation);
        if candidates.is_empty() {
            return self.toast("Nothing left to connect this to", false, cx);
        }
        let prompt = Prompt::Pick { node: node.id.clone(), relation, candidates };
        self.open_prompt(prompt, window, cx);
    }

    // ---- prompt --------------------------------------------------------

    fn open_prompt(&mut self, prompt: Prompt, window: &mut Window, cx: &mut Context<Self>) {
        let (placeholder, lead) = (prompt.placeholder(), if matches!(prompt, Prompt::Create(_)) { "+" } else { "⌕" });
        self.palette.update(cx, |palette, cx| {
            palette.set_lead(lead);
            palette.open("", placeholder, None, cx);
        });
        self.prompt = Some(prompt);
        self.show_help = false;
        self.refresh_palette(cx);
        window.focus(&self.palette.focus_handle(cx), cx);
        cx.notify();
    }

    fn close_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Opens the prompt for a new node placed relative to the selection.
    fn prompt_create(&mut self, kind: Kind, relation: Option<Direction>, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.selected_node().cloned();
        let mut new = NewNode { kind, milestones: Vec::new(), depends_on: Vec::new(), required_by: None };
        if kind == Kind::Task
            && let Some(sel) = &selected
        {
            // Tasks created around a selection land in the same milestones.
            new.milestones = match sel.kind {
                Kind::Milestone => vec![sel.id.clone()],
                Kind::Task => sel.milestones.clone(),
            };
            match relation {
                Some(Direction::Right) if sel.kind == Kind::Task => new.depends_on = vec![sel.id.clone()],
                // A milestone already requires its members; linking them too would be redundant.
                Some(Direction::Left) if sel.kind == Kind::Task => new.required_by = Some(sel.id.clone()),
                _ => {}
            }
        }
        if relation.is_some() && selected.is_none() {
            return self.toast("Select a node first", false, cx);
        }
        self.open_prompt(Prompt::Create(new), window, cx);
    }

    /// Opens the prompt for a task that comes after `source`, in the same
    /// milestones: what dragging its handle onto empty canvas asks for.
    pub(crate) fn prompt_follow_up(&mut self, source: &NodeId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.graph().get(source) else { return };
        let new = NewNode {
            kind: Kind::Task,
            milestones: source.milestones.clone(),
            depends_on: vec![source.id.clone()],
            required_by: None,
        };
        self.open_prompt(Prompt::Create(new), window, cx);
    }

    fn on_palette_event(
        &mut self,
        _: &Entity<Combobox>,
        event: &ComboEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ComboEvent::Changed => self.refresh_palette(cx),
            // The node the highlighted entry stands for comes into view.
            ComboEvent::Highlighted => {
                if let Some(id) = self.list_cursor(cx) {
                    self.reveal(&id, false);
                }
            }
            ComboEvent::Cancel => self.close_prompt(window, cx),
            ComboEvent::Submit => self.submit_prompt(window, cx),
            // The prompt has one field.
            ComboEvent::Next | ComboEvent::Previous => {}
        }
        cx.notify();
    }

    /// Gives the prompt the nodes that match what is typed.
    fn refresh_palette(&mut self, cx: &mut Context<Self>) {
        let choices: Vec<Choice> = self
            .listed(cx)
            .iter()
            .map(|id| {
                let node = self.graph().get(id).expect("the list comes from the graph");
                let tag = node.tags.first().map(|t| format!("#{t}  ")).unwrap_or_default();
                Choice {
                    key: Some(id.to_string()),
                    icon: Some(theme::node_icon(node)),
                    ..Choice::new(node.title.clone(), format!("{tag}{id}"))
                }
            })
            .collect();
        let listing = matches!(self.prompt, Some(Prompt::Search | Prompt::Pick { .. }));
        let empty = if listing && choices.is_empty() { "No matches" } else { "" };
        self.palette.update(cx, |palette, cx| {
            palette.set_choices(choices, cx);
            palette.set_feedback((empty.to_owned(), theme::FAINT), None, cx);
        });
    }

    pub(crate) fn submit_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(prompt) = self.prompt.clone() else { return };
        let palette = self.palette.read(cx);
        let text = palette.text(cx).trim().to_owned();
        let picked = palette.picked().and_then(|choice| choice.key.clone()).map(NodeId);
        match prompt {
            Prompt::Search => {
                if let Some(id) = picked {
                    self.select(Some(id), true);
                }
            }
            Prompt::Pick { node, relation, .. } => {
                // Nothing matches: keep the prompt open so the query can be corrected.
                let Some(picked) = picked else { return };
                self.mutate(cx, |graph| Self::relate(graph, &node, relation, &picked));
            }
            Prompt::Create(_) if text.is_empty() => {}
            Prompt::Create(new) => {
                if let Some(id) = self.create(new, text, cx) {
                    self.select(Some(id), true);
                }
            }
        }
        self.close_prompt(window, cx);
    }

    /// The list of the open search or pick prompt: the nodes it offers that
    /// match the typed query by title, `#tag` or id prefix; open work first.
    pub(crate) fn listed(&self, cx: &App) -> Vec<NodeId> {
        // Nodes the canvas hides cannot be jumped to; linking may still offer them.
        let cells = self.graph_cache.cells();
        let offered = |n: &&Node| match &self.prompt {
            Some(Prompt::Search) => cells.contains_key(&n.id),
            Some(Prompt::Pick { candidates, .. }) => candidates.contains(&n.id),
            _ => false,
        };
        let query = self.palette.read(cx).text(cx).trim().to_lowercase();
        let tag = query.strip_prefix('#');
        let mut hits: Vec<(bool, usize, &Node)> = self
            .graph()
            .nodes()
            .filter(offered)
            .filter_map(|n| {
                let title = n.title.to_lowercase();
                let rank = match tag {
                    Some(tag) => n.tags.iter().any(|t| t.to_lowercase().starts_with(tag)).then_some(0),
                    None if query.is_empty() => Some(0),
                    None if n.id.as_str().starts_with(&query) => Some(0),
                    None => title.find(&query).map(|i| i + 1),
                }?;
                Some((n.status.is_closed(), rank, n))
            })
            .collect();
        hits.sort_by(|a, b| (a.0, a.1, &a.2.title).cmp(&(b.0, b.1, &b.2.title)));
        hits.into_iter().map(|(_, _, n)| n.id.clone()).collect()
    }

    /// Nodes to emphasize while a list is open, or `None` when the whole graph is of interest.
    fn search_matches(&self, cx: &App) -> Option<Vec<NodeId>> {
        let ids =
            || self.palette.read(cx).choices().iter().filter_map(|c| c.key.clone().map(NodeId)).collect::<Vec<_>>();
        match &self.prompt {
            Some(Prompt::Search) if !self.palette.read(cx).text(cx).trim().is_empty() => Some(ids()),
            Some(Prompt::Pick { node, .. }) => Some(ids().into_iter().chain([node.clone()]).collect()),
            _ => None,
        }
    }

    /// The node the highlighted list entry stands for.
    fn list_cursor(&self, cx: &App) -> Option<NodeId> {
        self.prompt.as_ref()?;
        self.palette.read(cx).highlighted()?.key.clone().map(NodeId)
    }

    // ---- AI organizing -------------------------------------------------

    fn run_organize(&mut self, what: OrganizeKind, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let config = match Config::load(self.ws.dir()) {
            Ok(config) => config,
            Err(e) => return self.toast(e.to_string(), true, cx),
        };
        let graph = self.ws.graph.clone();
        self.busy = true;
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
                if app.retired {
                    return;
                }
                app.busy = false;
                match result {
                    Ok(proposals) if proposals.is_empty() => {
                        app.toast("No suggestions — the graph looks complete", false, cx)
                    }
                    Ok(proposals) => {
                        app.toast(format!("{} suggestion(s) in the inspector", proposals.len()), false, cx);
                        app.proposals = proposals;
                        app.clear_selection();
                    }
                    Err(e) => app.toast(e, true, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn accept(&mut self, indices: Vec<usize>, cx: &mut Context<Self>) {
        let mut picked = Vec::new();
        for index in indices.into_iter().rev() {
            picked.push(self.proposals.remove(index));
        }
        let before = self.ws.graph.clone();
        let applied = organize::apply(&mut self.ws.graph, picked);
        let skipped: Vec<&String> = applied.iter().filter_map(|a| a.skipped.as_ref()).collect();
        self.enqueue_save(&before, cx);
        self.record_undo(before);
        let done = applied.len() - skipped.len();
        match skipped.first() {
            None => self.toast(format!("Applied {done}"), false, cx),
            Some(why) => self.toast(format!("Applied {done}, skipped {}: {why}", skipped.len()), true, cx),
        }
        cx.notify();
    }

    // ---- camera --------------------------------------------------------

    fn to_screen(&self, cell: layout::Cell) -> Point<Pixels> {
        let (col, row) = cell;
        point(self.offset.x + px(col as f32 * CELL_W * self.zoom), self.offset.y + px(row as f32 * CELL_H * self.zoom))
    }

    fn animate_to(&mut self, offset: Point<Pixels>, zoom: f32) {
        self.anim = Some(Anim { from: (self.offset, self.zoom), to: (offset, zoom), start: Instant::now() });
    }

    /// Advances the camera animation; returns whether it needs another frame.
    fn step_anim(&mut self) -> bool {
        let Some(anim) = &self.anim else { return false };
        let t = (anim.start.elapsed().as_secs_f32() / 0.22).min(1.0);
        let ease = 1.0 - (1.0 - t).powi(3);
        let (from, to) = (anim.from, anim.to);
        self.offset = from.0 + (to.0 - from.0) * ease;
        self.zoom = from.1 + (to.1 - from.1) * ease;
        if t >= 1.0 {
            self.anim = None;
        }
        t < 1.0
    }

    /// Frames the whole graph in the canvas. The initial framing keeps text
    /// readable and anchors large graphs at their start (the left) instead.
    fn fit(&mut self, initial: bool) {
        let area = self.area.get().size;
        self.ensure_graph_cache();
        if area.width <= px(0.) || self.graph_cache.cells().is_empty() {
            return;
        }
        let (cols, rows) = self.graph_cache.extent();
        let (cols, rows) = (cols as f32, rows as f32);
        let (w, h) = (cols * CELL_W + NODE_W, rows * CELL_H + NODE_H);
        let (aw, ah) = (f32::from(area.width), f32::from(area.height));
        let fit = ((aw - 120.) / w).min((ah - 160.) / h);
        let zoom = match initial {
            true => fit.clamp(0.85, 1.0),
            false => fit.clamp(MIN_ZOOM, 1.15),
        };
        let center = |available: f32, content: f32| px(((available - content * zoom) / 2.).max(48.));
        let offset = point(center(aw, w), center(ah, h) + px(20.));
        match initial {
            true => (self.offset, self.zoom) = (offset, zoom),
            false => self.animate_to(offset, zoom),
        }
    }

    /// The (offset, zoom) the camera is at, or is moving to.
    fn target(&self) -> (Point<Pixels>, f32) {
        self.anim.as_ref().map_or((self.offset, self.zoom), |a| a.to)
    }

    /// Zooms by `factor` keeping the canvas point `anchor` (local coordinates) fixed.
    fn zoom_by(&mut self, factor: f32, anchor: Point<Pixels>, animate: bool) {
        let (offset, zoom) = self.target();
        // A large selection can fit below the usual interactive zoom limit.
        // Continue smoothly from that overview instead of jumping to MIN_ZOOM.
        let min_zoom = if zoom < MIN_ZOOM { f32::MIN_POSITIVE } else { MIN_ZOOM };
        let new_zoom = (zoom * factor).clamp(min_zoom, MAX_ZOOM);
        let new_offset = anchor - (anchor - offset) * (new_zoom / zoom);
        match animate {
            true => self.animate_to(new_offset, new_zoom),
            false => {
                self.anim = None;
                (self.offset, self.zoom) = (new_offset, new_zoom);
            }
        }
    }

    /// Pinch zooms around the pointer; a two-finger double-tap toggles
    /// between actual size and the whole graph.
    fn on_gesture(&mut self, gesture: Gesture, window: &mut Window, cx: &mut Context<Self>) {
        let area = self.area.get();
        let mouse = window.mouse_position();
        if !area.contains(&mouse) {
            return;
        }
        let anchor = mouse - area.origin;
        match gesture {
            Gesture::Pinch(magnification) => self.zoom_by(1.0 + magnification, anchor, false),
            Gesture::SmartZoom if (self.target().1 - 1.0).abs() < 0.05 => self.fit(false),
            Gesture::SmartZoom => self.zoom_to_actual_size(anchor),
        }
        cx.notify();
    }

    fn zoom_to_actual_size(&mut self, anchor: Point<Pixels>) {
        self.zoom_by(1.0 / self.target().1, anchor, true);
    }

    fn canvas_center(&self) -> Point<Pixels> {
        let size = self.area.get().size;
        point(size.width / 2., size.height / 2.)
    }

    /// Moves the camera so `id` is visible; `force` centers it even when it already is.
    fn reveal(&mut self, id: &NodeId, force: bool) {
        self.ensure_graph_cache();
        let cells = self.graph_cache.cells();
        let placed = self.placed.filter(|cell| self.graph_cache.at(*cell) == Some(id));
        let Some(cell) = placed.or_else(|| cells.get(id).copied()) else { return };
        let area = self.area.get().size;
        // Where the node will be once a camera move in flight has finished.
        let (offset, z) = self.target();
        let p = point(offset.x + px(cell.0 as f32 * CELL_W * z), offset.y + px(cell.1 as f32 * CELL_H * z));
        let margin = px(40.);
        let visible = p.x >= margin
            && p.y >= margin + px(40.)
            && p.x + px(NODE_W * z) <= area.width - margin
            && p.y + px(NODE_H * z) <= area.height - margin;
        if force || !visible {
            let center = point(p.x + px(NODE_W * z / 2.), p.y + px(NODE_H * z / 2.));
            self.animate_to(offset + (self.canvas_center() - center), z);
        }
    }

    fn select(&mut self, id: Option<NodeId>, reveal: bool) {
        if reveal && let Some(id) = &id {
            self.reveal(id, false);
        }
        self.selected_nodes = id.iter().cloned().collect();
        self.selected = id;
        self.placed = None;
    }

    /// Whether the priority filter leaves `node` undimmed.
    pub(crate) fn passes_filter(&self, node: &Node) -> bool {
        self.priority_filter.is_none_or(|least| node.priority.is_some_and(|p| p >= least))
    }

    /// Steps the priority filter: everything, urgent only, high and up, medium and up, any priority.
    pub(crate) fn cycle_priority_filter(&mut self) {
        self.priority_filter = match self.priority_filter {
            None => Some(Priority::Urgent),
            Some(Priority::Urgent) => Some(Priority::High),
            Some(Priority::High) => Some(Priority::Medium),
            Some(Priority::Medium) => Some(Priority::Low),
            Some(Priority::Low) => None,
        };
    }

    /// Frames the new layout at once: a different set of cards is not a camera move to animate.
    fn refit(&mut self) {
        self.fit(false);
        if let Some(anim) = self.anim.take() {
            (self.offset, self.zoom) = anim.to;
        }
    }

    pub(crate) fn toggle_hide_completed(&mut self) {
        self.view.hide_completed = !self.view.hide_completed;
        self.refit();
        self.persist_view();
    }

    pub(crate) fn toggle_group_by_tag(&mut self) {
        self.view.group_by_tag = !self.view.group_by_tag;
        self.refit();
        self.persist_view();
    }

    pub(crate) fn toggle_group(&mut self, group: &layout::Group) {
        if !self.view.collapsed.remove(group) {
            self.view.collapsed.insert(group.clone());
        }
    }

    fn clear_selection(&mut self) {
        self.selected = None;
        self.selected_nodes.clear();
    }

    fn toggle_selection(&mut self, id: NodeId) {
        if self.graph().get(&id).is_none() {
            return;
        }
        if self.selected_nodes.remove(&id) {
            if self.selected.as_ref() == Some(&id) {
                self.selected = self.selected_nodes.first().cloned();
            }
        } else {
            self.selected_nodes.insert(id.clone());
            self.selected = Some(id);
        }
    }

    fn select_nodes(&mut self, ids: &[NodeId], cx: &mut Context<Self>) {
        self.selected_nodes = ids.iter().filter(|id| self.graph().get(id).is_some()).cloned().collect();
        self.selected = self.selected_nodes.first().cloned();
        self.frame_selection();
        cx.notify();
    }

    fn frame_selection(&mut self) {
        self.ensure_graph_cache();
        let cells = self.graph_cache.cells();
        let selected: Vec<_> = self.selected_nodes.iter().filter_map(|id| cells.get(id)).collect();
        if selected.is_empty() {
            return;
        }
        let min_col = selected.iter().map(|c| c.0).min().unwrap() as f32;
        let max_col = selected.iter().map(|c| c.0).max().unwrap() as f32;
        let min_row = selected.iter().map(|c| c.1).min().unwrap() as f32;
        let max_row = selected.iter().map(|c| c.1).max().unwrap() as f32;
        let (width, height) = ((max_col - min_col) * CELL_W + NODE_W, (max_row - min_row) * CELL_H + NODE_H);
        let area = self.area.get().size;
        let (aw, ah) = (f32::from(area.width), f32::from(area.height));
        if aw <= 80. || ah <= 120. {
            return;
        }
        let zoom = ((aw - 80.) / width).min((ah - 120.) / height).min(1.15);
        let offset = point(
            px((aw - width * zoom) / 2. - min_col * CELL_W * zoom),
            px((ah - height * zoom) / 2. - min_row * CELL_H * zoom),
        );
        self.animate_to(offset, zoom);
    }

    fn set_selected_status(&mut self, status: Status, cx: &mut Context<Self>) {
        self.change_selected_status(|_| status, cx);
    }

    fn change_selected_status(&mut self, next: impl Fn(Status) -> Status, cx: &mut Context<Self>) {
        let ids = self.selected_nodes.clone();
        self.mutate(cx, |graph| {
            for id in ids {
                let status = graph.get(&id).expect("selection exists").status;
                graph.set_status(&id, next(status))?;
            }
            Ok(())
        });
    }

    fn delete_selected(&mut self, cx: &mut Context<Self>) {
        if self.selected_nodes.len() == 1 {
            self.delete(self.selected.clone().unwrap(), cx);
            return;
        }
        let ids = self.selected_nodes.clone();
        if self.mutate(cx, |graph| {
            for id in &ids {
                graph.remove(id)?;
            }
            Ok(())
        }) {
            self.clear_selection();
            self.hovered = None;
            self.toast(format!("Deleted {} nodes — ⌘Z to undo", ids.len()), false, cx);
        }
    }

    /// Selects the neighbor of the selection in `direction`: requirements to
    /// the left, dependents to the right, the same column up and down.
    fn navigate(&mut self, direction: Direction) {
        self.ensure_graph_cache();
        let graph = &self.ws.graph;
        let cells = self.graph_cache.cells();
        let Some(current) = self.selected.as_ref().and_then(|id| graph.get(id)) else {
            let first = (graph.ready_tasks(None).into_iter().map(|n| &n.id))
                .find(|id| cells.contains_key(*id))
                .or(cells.keys().next())
                .cloned();
            return self.select(first, true);
        };
        // Hidden nodes have no card to move to, whatever the graph says about them.
        let shown = |id: &&NodeId| cells.contains_key(*id);
        let cache = &self.graph_cache;
        let from = self.placed.filter(|cell| cache.at(*cell) == Some(&current.id)).unwrap_or(cells[&current.id]);
        let (col, row) = from;
        let column = |c: usize| cells.iter().filter(move |(_, cell)| cell.0 == c).map(|(id, _)| id.clone());
        let candidates: Vec<NodeId> = match direction {
            Direction::Left => {
                let reqs: Vec<NodeId> = graph.requirements(current).into_iter().filter(shown).cloned().collect();
                match (reqs.is_empty(), col) {
                    (false, _) => reqs,
                    (true, 0) => Vec::new(),
                    (true, c) => column(c - 1).collect(),
                }
            }
            Direction::Right => {
                let deps: Vec<NodeId> = (graph.dependents(&current.id).map(|n| &n.id))
                    .chain(current.milestones.iter())
                    .filter(shown)
                    .cloned()
                    .collect();
                match deps.is_empty() {
                    false => deps,
                    true => column(col + 1).collect(),
                }
            }
            Direction::Up | Direction::Down => {
                let neighbour = cache.neighbour(from, matches!(direction, Direction::Up));
                if let Some((cell, id)) = neighbour {
                    let id = id.clone();
                    self.select(Some(id.clone()), false);
                    self.placed = Some(cell);
                    self.reveal(&id, false);
                }
                return;
            }
        };
        let next = candidates.into_iter().min_by_key(|id| cells[id].1.abs_diff(row));
        if next.is_some() {
            self.select(next, true);
        }
    }

    // ---- keyboard ------------------------------------------------------

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.notes_ask.is_some() {
            return self.on_notes_ask_key(event, window, cx);
        }
        // A text field has the keyboard; its keys are text, not canvas commands.
        if self.prompt.is_some() || self.inline.is_some() || self.notes.is_some() {
            return;
        }
        let keystroke = &event.keystroke;
        let m = keystroke.modifiers;
        let cmd = m.platform || m.control;
        let key = keystroke.key.as_str();
        let char = keystroke.key_char.as_deref().unwrap_or_default();
        let selected = self.selected.clone();
        match (key, cmd) {
            ("k" | "f", true) => self.open_prompt(Prompt::Search, window, cx),
            ("=" | "+", true) => self.zoom_by(1.25, self.canvas_center(), true),
            ("-", true) => self.zoom_by(0.8, self.canvas_center(), true),
            ("0", true) => self.zoom_to_actual_size(self.canvas_center()),
            (_, true) => return,
            _ if char == "?" || key == "?" => self.show_help = !self.show_help,
            _ if char == "+" || key == "+" || key == "=" => self.zoom_by(1.25, self.canvas_center(), true),
            ("-", _) => self.zoom_by(0.8, self.canvas_center(), true),
            ("0", _) => self.zoom_to_actual_size(self.canvas_center()),
            ("/", _) => self.open_prompt(Prompt::Search, window, cx),
            ("n", _) => self.prompt_create(Kind::Task, None, window, cx),
            ("m", _) => self.prompt_create(Kind::Milestone, None, window, cx),
            ("tab", _) => {
                let relation = if m.shift { Direction::Left } else { Direction::Right };
                self.prompt_create(Kind::Task, Some(relation), window, cx)
            }
            ("f", _) => self.fit(false),
            ("p", _) if m.shift => self.cycle_priority_filter(),
            ("h", _) if m.shift => self.toggle_hide_completed(),
            ("g", _) if m.shift => self.toggle_group_by_tag(),
            ("escape", _) => match () {
                _ if self.drag.is_some() => self.drag = None,
                _ if self.show_help => self.show_help = false,
                _ => self.clear_selection(),
            },
            ("left", _) => self.navigate(Direction::Left),
            ("right", _) => self.navigate(Direction::Right),
            ("up", _) => self.navigate(Direction::Up),
            ("down", _) => self.navigate(Direction::Down),
            _ if !self.selected_nodes.is_empty()
                && matches!(key, "space" | "x" | "1" | "2" | "3" | "4" | "c" | "backspace" | "delete") =>
            {
                match key {
                    "space" => self.change_selected_status(next_status, cx),
                    "x" => self.change_selected_status(
                        |status| if status == Status::Done { Status::Todo } else { Status::Done },
                        cx,
                    ),
                    "1" => self.set_selected_status(Status::Todo, cx),
                    "2" => self.set_selected_status(Status::Doing, cx),
                    "3" => self.set_selected_status(Status::Done, cx),
                    "4" => self.set_selected_status(Status::Dropped, cx),
                    "c" if self.selected_nodes.len() == 1 => {
                        let id = self.selected.clone().unwrap();
                        self.reveal(&id, true);
                    }
                    "c" => self.frame_selection(),
                    "backspace" | "delete" => self.delete_selected(cx),
                    _ => unreachable!(),
                }
            }
            _ => {
                if self.selected_nodes.len() != 1 {
                    return;
                }
                let Some(id) = selected else { return };
                let node = self.graph().get(&id).expect("selection exists").clone();
                match key {
                    "l" if m.shift => self.prompt_pick(Relation::NeededBy, window, cx),
                    "l" => self.prompt_pick(Relation::Requires, window, cx),
                    "i" if node.kind == Kind::Task => self.prompt_pick(Relation::InMilestone, window, cx),
                    "i" => self.prompt_pick(Relation::Member, window, cx),
                    // A cloud workspace has no file; its notes are edited here.
                    "o" if self.ws.remote().is_none() => self.open_file(&id, cx),
                    "o" | "e" => self.start_notes(window, cx),
                    "enter" | "f2" | "r" => self.start_inline(Field::Title, window, cx),
                    "d" => self.start_inline(Field::Due, window, cx),
                    "t" => self.start_inline(Field::Tags, window, cx),
                    "p" => self.start_inline(Field::Priority, window, cx),
                    "a" => self.start_inline(Field::Assignee, window, cx),
                    "g" => self.start_inline(Field::Pr, window, cx),
                    _ => return,
                }
            }
        }
        cx.stop_propagation();
        cx.notify();
    }
}

impl Focusable for TopoApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TopoApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_graph_cache();
        if self.pending_fit {
            match self.area.get().size.width > px(0.) {
                true => {
                    self.fit(true);
                    self.pending_fit = false;
                }
                false => window.request_animation_frame(),
            }
        }
        let viewport = window.viewport_size().width;
        self.viewport_width = f32::from(viewport);
        self.compact = viewport < px(1180.);
        self.clamp_inspector_width_to_viewport();
        if self.step_anim() {
            window.request_animation_frame();
        }
        // The field belongs to the one selected node and lives while it has the
        // focus. Leaving the window does not move the focus, so it survives that.
        let focused = self.combo.focus_handle(cx).is_focused(window);
        match &self.inline {
            Some(edit) if !focused || self.selected_node().is_none_or(|n| n.id != edit.node) => {
                self.inline = None;
                window.focus(&self.focus, cx);
            }
            None if focused => window.focus(&self.focus, cx),
            _ => {}
        }
        self.sync_notes(window, cx);
        // The canvas takes the standard editing commands only while no text field is
        // open. A command that cannot run has no handler, which disables its menu item.
        let canvas = self.prompt.is_none() && self.inline.is_none() && self.notes.is_none();
        let (has_selection, has_nodes) = (!self.selected_nodes.is_empty(), self.graph().nodes().next().is_some());
        let (can_undo, can_redo) = (!self.undo.is_empty(), !self.redo.is_empty());
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::CANVAS))
            .text_color(rgb(theme::TEXT))
            .font_family(".SystemUIFont")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_action(cx.listener(|app, _: &CloseWindow, window, cx| {
                if app.can_leave_to(notes::Then::CloseWindow, cx) {
                    window.remove_window();
                }
            }))
            .on_action(cx.listener(|app, _: &Quit, _, cx| {
                if app.can_leave_to(notes::Then::Quit, cx) {
                    cx.quit();
                }
            }))
            .when(canvas && can_undo, |d| {
                d.on_action(cx.listener(|app, _: &text_input::Undo, _, cx| app.restore(true, cx)))
            })
            .when(canvas && can_redo, |d| {
                d.on_action(cx.listener(|app, _: &text_input::Redo, _, cx| app.restore(false, cx)))
            })
            .when(canvas && has_nodes, |d| {
                d.on_action(cx.listener(|app, _: &text_input::SelectAll, _, cx| app.select_all(cx)))
            })
            .when(canvas && has_selection, |d| {
                d.on_action(cx.listener(|app, _: &text_input::Copy, _, cx| {
                    app.copy_selection(cx);
                }))
                .on_action(cx.listener(|app, _: &text_input::Cut, _, cx| app.cut_selection(cx)))
            })
            .when(canvas, |d| d.on_action(cx.listener(|app, _: &text_input::Paste, _, cx| app.paste_nodes(cx))))
            // A click outside the prompt or the edited field dismisses it; they keep their own clicks.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _, window, cx| {
                    if app.prompt.is_some() {
                        app.close_prompt(window, cx);
                    }
                    app.cancel_inline(window, cx);
                    if app.notes.is_some() && app.notes_ask.is_none() {
                        app.leave_notes(window, cx);
                    }
                }),
            )
            .on_mouse_move(cx.listener(Self::on_drag_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_drag_end))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_drag_end))
            .child(self.toolbar(cx))
            .when(self.persistence.pending_count() > 0, |d| d.child(self.persistence_bar(cx)))
            .child(div().flex_1().min_h(px(0.)).flex().child(self.graph_view(cx)).child(self.inspector(cx)))
            .when(self.notes_ask.is_some(), |d| d.child(self.notes_dialog(cx)))
    }
}

/// Tags typed separated by spaces or commas, with or without `#`, each kept once.
fn parse_tags(text: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for tag in text.split([',', ' ', '　']).map(|t| t.trim_start_matches('#')).filter(|t| !t.is_empty()) {
        if !tags.iter().any(|t| t == tag) {
            tags.push(tag.to_owned());
        }
    }
    tags
}

fn next_status(status: Status) -> Status {
    match status {
        Status::Todo => Status::Doing,
        Status::Doing => Status::Done,
        Status::Done | Status::Dropped => Status::Todo,
    }
}

#[cfg(feature = "screenshot")]
fn open_workspace(start: Option<&Path>) -> Result<Workspace> {
    Ok(match (std::env::var_os("TOPO_DIR"), start) {
        (Some(dir), _) => topo_cloud::open(PathBuf::from(dir))?,
        (None, Some(start)) => topo_cloud::discover(start)?,
        (None, None) => topo_cloud::discover(&std::env::current_dir()?)?,
    })
}

fn main() -> Result<()> {
    let args = <args::Args as clap::Parser>::parse();
    #[cfg(not(feature = "screenshot"))]
    if args.screenshot.is_some() {
        anyhow::bail!(
            "--screenshot needs a build with the `screenshot` feature (cargo build -p topo-gui --features screenshot)"
        );
    }
    #[cfg(feature = "screenshot")]
    if let Some(path) = &args.screenshot {
        return screenshot::render(open_workspace(args.workspace.as_deref())?, path, &args);
    }
    let config = config::UserConfig::load();
    let start = repository::startup_target(
        std::env::var_os("TOPO_DIR").map(PathBuf::from),
        args.workspace,
        &config,
        std::env::current_dir()?,
    );
    run(start, config)
}

fn run(start: repository::Selection, config: config::UserConfig) -> Result<()> {
    let title: SharedString = "topo — Open repository".into();
    Application::with_platform(gpui_platform::current_platform(false)).run(move |cx: &mut App| {
        branding::set_app_icon();
        text_input::bind_keys(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-w", CloseWindow, None),
            KeyBinding::new("cmd-o", repository::OpenRepository, None),
            KeyBinding::new("ctrl-o", repository::OpenRepository, None),
        ]);
        cx.set_menus(vec![
            Menu { name: "topo".into(), items: vec![MenuItem::action("Quit topo", Quit)], disabled: false },
            Menu {
                name: "File".into(),
                items: vec![MenuItem::action("Open Repository…", repository::OpenRepository)],
                disabled: false,
            },
            // Each item acts on the text of a focused field, and on the graph otherwise.
            Menu {
                name: "Edit".into(),
                items: vec![
                    MenuItem::os_action("Undo", text_input::Undo, OsAction::Undo),
                    MenuItem::os_action("Redo", text_input::Redo, OsAction::Redo),
                    MenuItem::separator(),
                    MenuItem::os_action("Cut", text_input::Cut, OsAction::Cut),
                    MenuItem::os_action("Copy", text_input::Copy, OsAction::Copy),
                    MenuItem::os_action("Paste", text_input::Paste, OsAction::Paste),
                    MenuItem::os_action("Select All", text_input::SelectAll, OsAction::SelectAll),
                ],
                disabled: false,
            },
        ]);
        let bounds = Bounds::centered(None, size(px(1360.), px(860.)), cx);
        let options = WindowOptions {
            app_id: Some("dev.r4ai.topo".into()),
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui::TitlebarOptions { title: Some(title), ..Default::default() }),
            window_min_size: Some(size(px(720.), px(480.))),
            ..Default::default()
        };
        cx.open_window(options, |window, cx| cx.new(|cx| repository::RepositoryWindow::new(start, config, window, cx)))
            .expect("failed to open window");
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        cx.activate(true);
    });
    Ok(())
}

#[cfg(test)]
mod ui_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_split_stripped_and_deduplicated() {
        assert_eq!(parse_tags("#core, io　core  #ui"), ["core", "io", "ui"]);
        assert!(parse_tags(" , ").is_empty());
    }

    /// The real OS watcher signals a change to a Markdown file. It runs outside
    /// gpui's test scheduler, which rejects the watcher thread's activity.
    #[test]
    fn file_watcher_signals_changes_to_the_nodes_directory() {
        let dir = tempfile::tempdir().unwrap();
        let ws = Workspace::init(dir.path()).unwrap();
        let (_watcher, mut rx) = TopoApp::file_watcher(&ws).unwrap();
        std::fs::write(ws.nodes_dir().join("changed.md"), "x").unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while rx.try_recv().is_err() {
            assert!(std::time::Instant::now() < deadline, "no file event arrived");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
