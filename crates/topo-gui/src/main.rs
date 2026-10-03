//! `topo-gui`: native graph view and editor of a topo workspace.
//!
//! The canvas shows the dependency DAG left to right; the inspector on the
//! right edits the selected node. Everything is reachable from the keyboard
//! (press `?` for the list), every edit can be undone, and edits made
//! elsewhere (CLI, agents) appear live.

mod args;
mod branding;
mod chrome;
mod config;
mod dates;
mod gesture;
mod graph_view;
mod inspector;
mod layout;
#[cfg(feature = "screenshot")]
mod screenshot;
mod text_input;
mod theme;

use std::cell::Cell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use gpui::{
    App, Application, Bounds, Context, Entity, FocusHandle, Focusable, KeyBinding, KeyDownEvent, Menu, MenuItem,
    MouseButton, Pixels, Point, SharedString, Subscription, Window, WindowBounds, WindowOptions, actions, div, point,
    prelude::*, px, rgb, size,
};
use notify::{RecursiveMode, Watcher};
use topo_core::wire::Snapshot;
use topo_core::{Edit, Graph, Kind, Node, NodeId, Status, Workspace};
use topo_jev::organize::{self, Proposal};
use topo_jev::{Client, Config};

use futures::StreamExt;

use crate::gesture::Gesture;
use crate::text_input::{InputEvent, TextInput};

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
    Rename(NodeId),
    Due(NodeId),
    Tags(NodeId),
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
            Prompt::Rename(_) => "Rename",
            Prompt::Due(_) => "Due date",
            Prompt::Tags(_) => "Tags",
        }
    }

    fn placeholder(&self) -> &'static str {
        match self {
            Prompt::Search | Prompt::Pick { .. } => "Search by title, #tag or id…",
            Prompt::Create(NewNode { kind: Kind::Milestone, .. }) => "Milestone title",
            Prompt::Create(_) => "What needs to be done?",
            Prompt::Rename(_) => "Title",
            Prompt::Due(_) => "2026-10-31, 10-31, +3d, +2w, tomorrow — empty clears",
            Prompt::Tags(_) => "Space- or comma-separated tags — empty clears",
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
    graph_cache: graph_view::GraphCache,
    hovered: Option<NodeId>,
    drag: Option<Drag>,
    prompt: Option<Prompt>,
    input: Entity<TextInput>,
    /// Position of the highlighted entry in the list of a search or pick prompt.
    search_index: usize,
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
    _subscriptions: Vec<Subscription>,
}

impl TopoApp {
    #[cfg(test)]
    fn new(ws: Workspace, window: &mut Window, cx: &mut Context<Self>) -> Result<Self> {
        Self::with_inspector_width(ws, None, window, cx)
    }

    fn with_inspector_width(
        ws: Workspace,
        inspector_width: Option<f32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Self> {
        let watcher = match ws.remote() {
            Some(_) => {
                Self::poll_remote(cx);
                None
            }
            None => Some(Self::watch_files(&ws, cx)?),
        };
        let mut gestures = gesture::watch();
        cx.spawn_in(window, async move |this, cx| {
            while let Some(gesture) = gestures.next().await {
                if this.update_in(cx, |app, window, cx| app.on_gesture(gesture, window, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        let input = cx.new(TextInput::new);
        let subscription = cx.subscribe_in(&input, window, Self::on_input_event);
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Ok(Self {
            ws,
            focus,
            offset: point(px(40.), px(40.)),
            zoom: 1.0,
            anim: None,
            area: Rc::new(Cell::new(Bounds::default())),
            pending_fit: true,
            selected: None,
            selected_nodes: BTreeSet::new(),
            graph_cache: graph_view::GraphCache::default(),
            hovered: None,
            drag: None,
            prompt: None,
            input,
            search_index: 0,
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
            _subscriptions: vec![subscription],
        })
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
            let _ = config::UserConfig { inspector_width: Some(width) }.save();
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

    /// Reloads the graph whenever a Markdown file changes.
    fn watch_files(ws: &Workspace, cx: &mut Context<Self>) -> Result<notify::RecommendedWatcher> {
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
        Ok(watcher)
    }

    /// Asks the server every few seconds whether the workspace changed. The
    /// request runs off the main thread, so a slow network does not stall the window.
    fn poll_remote(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(REMOTE_POLL).await;
                let Ok(Some((remote, version))) = this.update(cx, |app, _| app.ws.remote()) else {
                    break;
                };
                let fetched = cx.background_executor().spawn(async move { remote.fetch(Some(version)) }).await;
                if this.update(cx, |app, cx| app.fetched(fetched, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Takes over what a poll of the server returned.
    fn fetched(&mut self, fetched: Result<Option<Snapshot>, topo_core::Error>, cx: &mut Context<Self>) {
        let known = self.ws.remote().map(|(_, version)| version);
        let result = match fetched {
            // A save of ours may have overtaken the poll; then the snapshot is the older one.
            Ok(Some(snapshot)) if Some(snapshot.version) > known => {
                let before = self.ws.graph.clone();
                self.ws.install(snapshot).map(|()| self.ws.graph != before)
            }
            Ok(_) => Ok(false),
            Err(e) => Err(e),
        };
        self.reloaded(result, "Updated from the cloud", cx);
    }

    /// Picks up edits made outside the app. Our own saves also trigger the
    /// watcher; those reload an identical graph and are ignored so the undo
    /// history survives.
    fn reload(&mut self, cx: &mut Context<Self>) {
        let result = self.ws.reload();
        self.reloaded(result, "Updated from disk", cx);
    }

    /// Finishes a reload that says whether it changed the graph.
    fn reloaded(&mut self, changed: Result<bool, topo_core::Error>, notice: &'static str, cx: &mut Context<Self>) {
        match changed {
            Ok(false) => {}
            Ok(true) => {
                self.undo.clear();
                self.redo.clear();
                self.graph_replaced();
                self.toast(notice, false, cx);
            }
            Err(e) => self.toast(format!("Reload failed: {e}"), true, cx),
        }
        cx.notify();
    }

    /// Saves the graph and returns whether it is still exactly what was saved.
    /// A cloud workspace can come back with the changes of other writers
    /// merged in. The undo history predates those, and stepping back to an
    /// older graph would undo them too, so it is dropped.
    fn save(&mut self) -> Result<bool, topo_core::Error> {
        let intended = self.ws.graph.clone();
        self.ws.save()?;
        let exact = self.ws.graph == intended;
        if !exact {
            self.undo.clear();
            self.redo.clear();
            self.graph_replaced();
        }
        Ok(exact)
    }

    /// Drops what may refer to nodes that the graph no longer has, after it
    /// was replaced as a whole.
    fn graph_replaced(&mut self) {
        self.graph_cache.invalidate();
        self.selected_nodes.retain(|id| self.ws.graph.get(id).is_some());
        if self.selected.as_ref().is_none_or(|id| !self.selected_nodes.contains(id)) {
            self.selected = self.selected_nodes.first().cloned();
        }
        self.drag = None;
    }

    /// Records `before` as an undo step, unless the edit changed nothing.
    fn record_undo(&mut self, before: Graph) {
        if before == self.ws.graph {
            return;
        }
        self.graph_cache.invalidate();
        self.undo.push(before);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Applies a graph mutation, saves it and records it for undo. On failure
    /// the graph is left untouched and the error is shown.
    fn mutate(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Graph) -> Result<(), topo_core::Error>) -> bool {
        let before = self.ws.graph.clone();
        let result = f(&mut self.ws.graph).and_then(|()| self.save());
        cx.notify();
        match result {
            Ok(exact) => {
                if exact {
                    self.record_undo(before);
                }
                true
            }
            Err(e) => {
                self.ws.graph = before;
                self.toast(e.to_string(), true, cx);
                false
            }
        }
    }

    /// Swaps the graph with the top of `from`, pushing the current one onto `to`.
    fn restore(&mut self, undo: bool, cx: &mut Context<Self>) {
        let Some(graph) = (if undo { &mut self.undo } else { &mut self.redo }).pop() else {
            return self.toast(if undo { "Nothing to undo" } else { "Nothing to redo" }, false, cx);
        };
        let current = std::mem::replace(&mut self.ws.graph, graph);
        match self.save() {
            Ok(true) => (if undo { &mut self.redo } else { &mut self.undo }).push(current),
            Ok(false) => {}
            Err(e) => {
                // Nothing was restored, so the step stays where it was.
                let graph = std::mem::replace(&mut self.ws.graph, current);
                (if undo { &mut self.undo } else { &mut self.redo }).push(graph);
                return self.toast(e.to_string(), true, cx);
            }
        }
        self.graph_replaced();
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
        self.open_prompt(prompt, "", window, cx);
    }

    // ---- prompt --------------------------------------------------------

    fn open_prompt(&mut self, prompt: Prompt, initial: &str, window: &mut Window, cx: &mut Context<Self>) {
        let placeholder = prompt.placeholder();
        self.input.update(cx, |input, cx| input.reset(initial, placeholder, cx));
        self.prompt = Some(prompt);
        self.search_index = 0;
        self.show_help = false;
        window.focus(&self.input.focus_handle(cx), cx);
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
        self.open_prompt(Prompt::Create(new), "", window, cx);
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
        self.open_prompt(Prompt::Create(new), "", window, cx);
    }

    fn on_input_event(
        &mut self,
        _: &Entity<TextInput>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Changed => self.search_index = 0,
            InputEvent::Cancel => self.close_prompt(window, cx),
            InputEvent::Submit => self.submit_prompt(window, cx),
            InputEvent::Up => self.move_in_list(self.search_index.saturating_sub(1), cx),
            InputEvent::Down => self.move_in_list(self.search_index + 1, cx),
        }
        cx.notify();
    }

    /// Highlights entry `index` of the prompt's list (clamped to it) and brings its node into view.
    fn move_in_list(&mut self, index: usize, cx: &mut Context<Self>) {
        let listed = self.listed(cx);
        self.search_index = index.min(listed.len().saturating_sub(1));
        if let Some(id) = listed.get(self.search_index) {
            self.reveal(id, false);
        }
    }

    pub(crate) fn submit_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(prompt) = self.prompt.clone() else { return };
        let text = self.input.read(cx).text().trim().to_owned();
        match prompt {
            Prompt::Search => {
                if let Some(id) = self.listed(cx).get(self.search_index).cloned() {
                    self.select(Some(id), true);
                }
            }
            Prompt::Pick { node, relation, .. } => {
                // Nothing matches: keep the prompt open so the query can be corrected.
                let Some(picked) = self.listed(cx).get(self.search_index).cloned() else { return };
                self.mutate(cx, |graph| Self::relate(graph, &node, relation, &picked));
            }
            Prompt::Create(_) if text.is_empty() => {}
            Prompt::Create(new) => {
                if let Some(id) = self.create(new, text, cx) {
                    self.select(Some(id), true);
                }
            }
            Prompt::Rename(_) if text.is_empty() => {}
            Prompt::Rename(id) => {
                self.mutate(cx, |graph| graph.edit(&id, Edit { title: Some(text), ..Edit::default() }));
            }
            Prompt::Due(id) => match dates::parse_due(&text, dates::today()) {
                Ok(due) => {
                    self.mutate(cx, |graph| graph.edit(&id, Edit { due: Some(due), ..Edit::default() }));
                }
                // Keep the prompt open so the input can be corrected.
                Err(e) => return self.toast(e, true, cx),
            },
            Prompt::Tags(id) => {
                let tags = parse_tags(&text);
                self.mutate(cx, |graph| graph.edit(&id, Edit { tags: Some(tags), ..Edit::default() }));
            }
        }
        self.close_prompt(window, cx);
    }

    /// The list of the open search or pick prompt: the nodes it offers that
    /// match the typed query by title, `#tag` or id prefix; open work first.
    pub(crate) fn listed(&self, cx: &App) -> Vec<NodeId> {
        let offered = |n: &&Node| match &self.prompt {
            Some(Prompt::Search) => true,
            Some(Prompt::Pick { candidates, .. }) => candidates.contains(&n.id),
            _ => false,
        };
        let query = self.input.read(cx).text().trim().to_lowercase();
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
        match &self.prompt {
            Some(Prompt::Search) if !self.input.read(cx).text().trim().is_empty() => Some(self.listed(cx)),
            Some(Prompt::Pick { node, .. }) => Some(self.listed(cx).into_iter().chain([node.clone()]).collect()),
            _ => None,
        }
    }

    /// The node the highlighted list entry stands for.
    fn list_cursor(&self, cx: &App) -> Option<NodeId> {
        self.listed(cx).get(self.search_index).cloned()
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
        match self.save() {
            Ok(exact) => {
                if exact {
                    self.record_undo(before);
                }
                let done = applied.len() - skipped.len();
                match skipped.first() {
                    None => self.toast(format!("Applied {done}"), false, cx),
                    Some(why) => self.toast(format!("Applied {done}, skipped {}: {why}", skipped.len()), true, cx),
                }
            }
            Err(e) => {
                self.ws.graph = before;
                self.toast(e.to_string(), true, cx);
            }
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
        let cells = self.graph_cache.cells();
        if area.width <= px(0.) || cells.is_empty() {
            return;
        }
        let cols = cells.values().map(|c| c.0).max().unwrap_or(0) as f32;
        let rows = cells.values().map(|c| c.1).max().unwrap_or(0) as f32;
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
        let Some(&cell) = cells.get(id) else { return };
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
            let first =
                graph.ready_tasks(None).first().map(|n| n.id.clone()).or(graph.nodes().next().map(|n| n.id.clone()));
            return self.select(first, true);
        };
        let (col, row) = cells[&current.id];
        let column = |c: usize| cells.iter().filter(move |(_, cell)| cell.0 == c).map(|(id, _)| id.clone());
        let candidates: Vec<NodeId> = match direction {
            Direction::Left => {
                let reqs: Vec<NodeId> = graph.requirements(current).into_iter().cloned().collect();
                match (reqs.is_empty(), col) {
                    (false, _) => reqs,
                    (true, 0) => Vec::new(),
                    (true, c) => column(c - 1).collect(),
                }
            }
            Direction::Right => {
                let deps: Vec<NodeId> = graph
                    .dependents(&current.id)
                    .map(|n| n.id.clone())
                    .chain(current.milestones.iter().cloned())
                    .collect();
                match deps.is_empty() {
                    false => deps,
                    true => column(col + 1).collect(),
                }
            }
            Direction::Up => column(col).filter(|id| cells[id].1 + 1 == row).collect(),
            Direction::Down => column(col).filter(|id| cells[id].1 == row + 1).collect(),
        };
        let next = candidates.into_iter().min_by_key(|id| cells[id].1.abs_diff(row));
        if next.is_some() {
            self.select(next, true);
        }
    }

    // ---- keyboard ------------------------------------------------------

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.prompt.is_some() {
            return;
        }
        let keystroke = &event.keystroke;
        let m = keystroke.modifiers;
        let cmd = m.platform || m.control;
        let key = keystroke.key.as_str();
        let char = keystroke.key_char.as_deref().unwrap_or_default();
        let selected = self.selected.clone();
        match (key, cmd) {
            ("z", true) => self.restore(!m.shift, cx),
            ("k" | "f", true) => self.open_prompt(Prompt::Search, "", window, cx),
            ("=" | "+", true) => self.zoom_by(1.25, self.canvas_center(), true),
            ("-", true) => self.zoom_by(0.8, self.canvas_center(), true),
            ("0", true) => self.zoom_to_actual_size(self.canvas_center()),
            (_, true) => return,
            _ if char == "?" || key == "?" => self.show_help = !self.show_help,
            _ if char == "+" || key == "+" || key == "=" => self.zoom_by(1.25, self.canvas_center(), true),
            ("-", _) => self.zoom_by(0.8, self.canvas_center(), true),
            ("0", _) => self.zoom_to_actual_size(self.canvas_center()),
            ("/", _) => self.open_prompt(Prompt::Search, "", window, cx),
            ("n", _) => self.prompt_create(Kind::Task, None, window, cx),
            ("m", _) => self.prompt_create(Kind::Milestone, None, window, cx),
            ("tab", _) => {
                let relation = if m.shift { Direction::Left } else { Direction::Right };
                self.prompt_create(Kind::Task, Some(relation), window, cx)
            }
            ("f", _) => self.fit(false),
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
                    "o" => self.open_file(&id, cx),
                    "enter" | "f2" | "r" => self.open_prompt(Prompt::Rename(id), &node.title, window, cx),
                    "d" => {
                        let due = node.due.map(|d| d.to_string()).unwrap_or_default();
                        self.open_prompt(Prompt::Due(id), &due, window, cx)
                    }
                    "t" => self.open_prompt(Prompt::Tags(id), &node.tags.join(" "), window, cx),
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
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::CANVAS))
            .text_color(rgb(theme::TEXT))
            .font_family(".SystemUIFont")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
            // A click outside the prompt dismisses it; the prompt keeps its own clicks.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _, window, cx| {
                    if app.prompt.is_some() {
                        app.close_prompt(window, cx);
                    }
                }),
            )
            .on_mouse_move(cx.listener(Self::on_drag_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_drag_end))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_drag_end))
            .child(self.toolbar(cx))
            .child(div().flex_1().min_h(px(0.)).flex().child(self.graph_view(cx)).child(self.inspector(cx)))
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
    let ws = open_workspace(args.workspace.as_deref())?;
    #[cfg(feature = "screenshot")]
    if let Some(path) = &args.screenshot {
        return screenshot::render(ws, path, &args);
    }
    run(ws)
}

fn run(ws: Workspace) -> Result<()> {
    let title: SharedString = format!("topo — {}", ws.dir().parent().unwrap_or(ws.dir()).display()).into();
    Application::with_platform(gpui_platform::current_platform(false)).run(move |cx: &mut App| {
        branding::set_app_icon();
        text_input::bind_keys(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None), KeyBinding::new("cmd-w", CloseWindow, None)]);
        cx.set_menus(vec![Menu {
            name: "topo".into(),
            items: vec![MenuItem::action("Quit topo", Quit)],
            disabled: false,
        }]);
        let bounds = Bounds::centered(None, size(px(1360.), px(860.)), cx);
        let options = WindowOptions {
            app_id: Some("dev.r4ai.topo".into()),
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui::TitlebarOptions { title: Some(title), ..Default::default() }),
            window_min_size: Some(size(px(720.), px(480.))),
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            let width = config::UserConfig::load().inspector_width;
            cx.new(|cx| TopoApp::with_inspector_width(ws, width, window, cx).expect("failed to watch the workspace"))
        })
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
}
