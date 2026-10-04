//! Interaction tests: a headless window that takes simulated keys and clicks.

use std::ops::Range;

use gpui::{Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px, size};
use tempfile::TempDir;
use topo_core::{Edit, Graph, Kind, Node, NodeId, Priority, Status, Workspace};

use crate::gesture::Gesture;
use crate::inline::Field;
use crate::{NODE_H, NODE_W, Prompt, TopoApp, layout::Group, text_input, theme};

fn id(s: &str) -> NodeId {
    NodeId(s.into())
}

struct Ui<'a> {
    app: Entity<TopoApp>,
    cx: &'a mut VisualTestContext,
    _dir: TempDir,
}

/// Opens a window on a workspace with the given `(id, kind, depends_on, milestones)` nodes.
fn open<'a>(cx: &'a mut TestAppContext, nodes: &[(&str, Kind, &[&str], &[&str])]) -> Ui<'a> {
    let dir = tempfile::tempdir().unwrap();
    let mut ws = Workspace::init(dir.path()).unwrap();
    let nodes = nodes.iter().map(|(name, kind, deps, milestones)| {
        let mut node = Node::new(id(name), *kind, (*name).into());
        node.depends_on = deps.iter().map(|d| id(d)).collect();
        node.milestones = milestones.iter().map(|m| id(m)).collect();
        node
    });
    ws.graph = Graph::from_nodes(nodes).unwrap();
    ws.save().unwrap();
    cx.update(|cx| {
        text_input::bind_keys(cx);
        theme::init(cx);
    });
    let (app, cx) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
    let mut ui = Ui { app, cx, _dir: dir };
    ui.resize(1360., 860.);
    ui
}

/// a → b → m, with c a second member of m.
const SAMPLE: &[(&str, Kind, &[&str], &[&str])] = &[
    ("a", Kind::Task, &[], &[]),
    ("b", Kind::Task, &["a"], &["m"]),
    ("c", Kind::Task, &[], &["m"]),
    ("m", Kind::Milestone, &[], &[]),
];

impl Ui<'_> {
    fn resize(&mut self, width: f32, height: f32) {
        self.cx.simulate_resize(size(px(width), px(height)));
        // The first frame records the canvas size, the second one uses it.
        self.redraw();
        self.redraw();
    }

    fn redraw(&mut self) {
        self.app.update(self.cx, |_, cx| cx.notify());
        self.cx.run_until_parked();
    }

    fn read<T>(&mut self, f: impl FnOnce(&TopoApp) -> T) -> T {
        self.app.read_with(self.cx, |app, _| f(app))
    }

    fn keys(&mut self, keystrokes: &str) {
        // Test the platform's standard modifier, just like text_input::bind_keys.
        let keys = if cfg!(target_os = "macos") { keystrokes.to_owned() } else { keystrokes.replace("cmd-", "ctrl-") };
        self.cx.simulate_keystrokes(&keys);
    }

    fn type_text(&mut self, text: &str) {
        self.cx.simulate_input(text);
    }

    /// Window position of the point `(fx, fy)` of the card of `node`, as fractions of its size.
    fn card(&mut self, node: &str, fx: f32, fy: f32) -> Point<Pixels> {
        self.read(|app| {
            let cell = app.graph_cache.cells()[&id(node)];
            let origin = app.area.get().origin + app.to_screen(cell);
            origin + point(px(NODE_W * app.zoom * fx), px(NODE_H * app.zoom * fy))
        })
    }

    fn move_to(&mut self, position: Point<Pixels>, pressed: bool) {
        self.cx.simulate_mouse_move(position, pressed.then_some(MouseButton::Left), Modifiers::none());
    }

    fn press(&mut self, position: Point<Pixels>) {
        self.move_to(position, false);
        self.cx.simulate_mouse_down(position, MouseButton::Left, Modifiers::none());
    }

    fn release(&mut self, position: Point<Pixels>) {
        self.cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
    }

    fn click(&mut self, position: Point<Pixels>) {
        self.press(position);
        self.release(position);
    }

    fn drag(&mut self, from: Point<Pixels>, to: Point<Pixels>) {
        self.press(from);
        self.move_to(from + (to - from) * 0.5, true);
        self.move_to(to, true);
        self.release(to);
    }

    /// Drags the link handle of `source` onto the card of `target`.
    fn drag_handle(&mut self, source: &str, target: &str) {
        let hover = self.card(source, 0.5, 0.5);
        self.move_to(hover, false);
        let (handle, target) = (self.card(source, 1.0, 0.5), self.card(target, 0.5, 0.5));
        self.drag(handle, target);
    }

    /// Clicks the middle of the element with debug selector `selector`.
    fn click_on(&mut self, selector: &'static str) {
        let bounds = self.cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is rendered"));
        self.click(bounds.center());
    }

    /// The text of the notes editor.
    fn notes(&mut self) -> String {
        self.app.read_with(self.cx, |app, cx| app.notes_input.read(cx).text().to_owned())
    }

    /// The selection of the notes editor; an empty range is the cursor.
    fn sel(&mut self) -> Range<usize> {
        self.app.read_with(self.cx, |app, cx| app.notes_input.read(cx).selection())
    }

    fn selected(&mut self) -> Option<String> {
        self.read(|app| app.selected.as_ref().map(|id| id.to_string()))
    }

    fn node(&mut self, name: &str) -> Node {
        self.read(|app| app.graph().get(&id(name)).expect("node exists").clone())
    }

    /// The field being edited in the inspector, and the text typed into it.
    fn inline(&mut self) -> Option<(Field, String)> {
        self.app
            .read_with(self.cx, |app, cx| app.inline.as_ref().map(|edit| (edit.field, app.combo.read(cx).value(cx))))
    }

    fn select(&mut self, node: &str) {
        let card = self.card(node, 0.5, 0.5);
        self.click(card);
    }

    fn selection(&mut self) -> Vec<String> {
        self.read(|app| app.selected_nodes.iter().map(|id| id.to_string()).collect())
    }

    fn titled(&mut self, title: &str) -> Node {
        self.read(|app| app.graph().nodes().find(|n| n.title == title).expect("node exists").clone())
    }
}

#[gpui::test]
fn keyboard_creates_edits_and_undoes(cx: &mut TestAppContext) {
    let mut ui = open(cx, &[]);
    ui.keys("m");
    ui.type_text("Release");
    ui.keys("enter");
    let release = ui.titled("Release");
    assert_eq!(release.kind, Kind::Milestone);
    assert_eq!(ui.selected(), Some(release.id.to_string()));

    // A task created with a milestone selected joins it; Tab and Shift-Tab chain around a task.
    ui.keys("n");
    ui.type_text("Build");
    ui.keys("enter");
    ui.keys("tab");
    ui.type_text("Ship");
    ui.keys("enter");
    ui.keys("shift-tab");
    ui.type_text("Test");
    ui.keys("enter");
    let (build, ship, test) = (ui.titled("Build"), ui.titled("Ship"), ui.titled("Test"));
    assert_eq!(build.milestones, std::slice::from_ref(&release.id));
    assert_eq!(ship.depends_on, [build.id.clone(), test.id.clone()]);
    assert_eq!(ship.milestones, std::slice::from_ref(&release.id));

    ui.keys("space");
    assert_eq!(ui.titled("Test").status, Status::Doing);
    ui.keys("x");
    assert_eq!(ui.titled("Test").status, Status::Done);
    ui.keys("enter");
    ui.type_text("Verify");
    ui.keys("enter");
    assert_eq!(ui.read(|app| app.graph().get(&test.id).unwrap().title.clone()), "Verify");

    ui.keys("backspace");
    assert!(ui.read(|app| app.graph().get(&test.id).is_none()));
    ui.keys("cmd-z");
    assert_eq!(ui.titled("Ship").depends_on.len(), 2);
    ui.keys("cmd-shift-z");
    assert_eq!(ui.titled("Ship").depends_on, [build.id]);

    // What is on disk is what is on screen.
    let on_disk = ui.read(|app| Workspace::open(app.ws.dir().to_owned()).unwrap().graph);
    assert!(ui.read(|app| *app.graph() == on_disk));
}

#[gpui::test]
fn clicking_selects_and_the_checkbox_completes(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let title = ui.card("a", 0.6, 0.3);
    ui.click(title);
    assert_eq!(ui.selected().as_deref(), Some("a"));
    let checkbox = ui.card("a", 0.08, 0.3);
    ui.click(checkbox);
    assert_eq!(ui.node("a").status, Status::Done);
    let empty = ui.read(|app| app.area.get().bottom_left()) + point(px(200.), px(-120.));
    ui.click(empty);
    assert_eq!(ui.selected(), None);
}

#[gpui::test]
fn dragging_the_handle_connects(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    // Wide enough for the chain that the links below stretch the graph into.
    ui.resize(1900., 900.);
    ui.drag_handle("c", "a");
    assert_eq!(ui.node("a").depends_on, [id("c")]);
    assert_eq!(ui.selected().as_deref(), Some("a"));
    // b already requires a, so a cannot require b back.
    ui.drag_handle("b", "a");
    assert!(ui.node("a").depends_on == [id("c")]);
    assert_eq!(ui.read(|app| app.toast.as_ref().map(|t| t.text.clone())).as_deref(), Some("Would create a cycle"));
    // A task dropped on a milestone joins it.
    ui.drag_handle("a", "m");
    assert_eq!(ui.node("a").milestones, [id("m")]);
}

#[gpui::test]
fn dropping_a_link_on_empty_canvas_adds_a_follow_up(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let hover = ui.card("b", 0.5, 0.5);
    ui.move_to(hover, false);
    let handle = ui.card("b", 1.0, 0.5);
    let empty = ui.read(|app| app.area.get().bottom_left()) + point(px(200.), px(-120.));
    ui.drag(handle, empty);
    ui.type_text("next");
    ui.keys("enter");
    let next = ui.titled("next");
    assert_eq!((next.depends_on, next.milestones), (vec![id("b")], vec![id("m")]));
}

#[gpui::test]
fn picking_from_the_list_connects_only_what_is_allowed(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let card = ui.card("a", 0.5, 0.5);
    ui.click(card);
    // b and m already require a; only c can become its prerequisite.
    ui.keys("l");
    assert_eq!(ui.app.read_with(ui.cx, |app, cx| app.listed(cx)), [id("c")]);
    ui.keys("enter");
    assert_eq!(ui.node("a").depends_on, [id("c")]);
    assert_eq!(ui.selected().as_deref(), Some("a"));

    ui.keys("i");
    ui.type_text("m");
    ui.keys("enter");
    assert_eq!(ui.node("a").milestones, [id("m")]);
    // Every other task is a member of m by now.
    ui.keys("right");
    assert_eq!(ui.selected().as_deref(), Some("b"));
    ui.keys("right");
    assert_eq!(ui.selected().as_deref(), Some("m"));
    ui.keys("i");
    assert!(ui.read(|app| app.prompt.is_none()));
}

#[gpui::test]
fn the_link_handle_stays_while_the_pointer_is_on_it(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let inside = ui.card("a", 0.9, 0.5);
    ui.move_to(inside, false);
    assert_eq!(ui.read(|app| app.hovered.clone()), Some(id("a")));
    // The handle straddles the right border; its outer half is outside the card.
    let outer_half = ui.card("a", 1.0, 0.5) + point(px(4.), px(0.));
    ui.move_to(outer_half, false);
    assert_eq!(ui.read(|app| app.hovered.clone()), Some(id("a")));
}

#[gpui::test]
fn dragging_a_card_pans_and_keeps_it_selected(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let before = ui.read(|app| app.offset);
    let from = ui.card("a", 0.5, 0.5);
    ui.drag(from, from + point(px(60.), px(40.)));
    assert_eq!(ui.read(|app| app.offset), before + point(px(60.), px(40.)));
    assert_eq!(ui.selected().as_deref(), Some("a"));
}

#[gpui::test]
fn clicking_away_from_a_prompt_only_closes_it(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let card = ui.card("a", 0.5, 0.5);
    ui.click(card);
    ui.keys("/");
    assert!(matches!(ui.read(|app| app.prompt.clone()), Some(Prompt::Search)));
    let empty = ui.read(|app| app.area.get().bottom_left()) + point(px(200.), px(-120.));
    ui.click(empty);
    assert!(ui.read(|app| app.prompt.is_none()));
    assert_eq!(ui.selected().as_deref(), Some("a"));
    assert_eq!(ui.node("a").title, "a");
}

#[gpui::test]
fn the_toolbar_fits_the_smallest_window(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    // Work in progress and overdue work add their counters to the toolbar.
    ui.app.update(ui.cx, |app, cx| {
        let overdue = Edit { due: Some(Some(jiff::civil::date(2020, 1, 1))), ..Edit::default() };
        app.mutate(cx, |graph| graph.set_status(&id("a"), Status::Doing).and_then(|()| graph.edit(&id("c"), overdue)));
    });
    ui.resize(720., 480.);
    for selector in [
        "brand",
        "stat-ready",
        "stat-doing",
        "stat-overdue",
        "search",
        "new-task",
        "new-milestone",
        "org-deps",
        "org-place",
        "undo",
        "redo",
        "help",
    ] {
        let bounds = ui.cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is rendered"));
        assert!(bounds.right() <= px(720.), "{selector} ends at {:?}", bounds.right());
        assert!(bounds.size.height <= px(26.), "{selector} is {:?} high", bounds.size.height);
    }
}

#[gpui::test]
fn search_emphasizes_every_match_and_jumps(cx: &mut TestAppContext) {
    let tasks: Vec<String> = (0..12).map(|i| format!("task{i:02}")).collect();
    let nodes: Vec<(&str, Kind, &[&str], &[&str])> =
        tasks.iter().map(|t| (t.as_str(), Kind::Task, &[][..], &[][..])).collect();
    let mut ui = open(cx, &nodes);
    ui.keys("/");
    ui.type_text("task");
    assert_eq!(ui.app.read_with(ui.cx, |app, cx| app.search_matches(cx).map(|m| m.len())), Some(12));
    // The list scrolls past the rows it shows at once.
    ui.keys("down down down down down down down down down down enter");
    assert_eq!(ui.selected().as_deref(), Some("task10"));
}

#[gpui::test]
fn the_inspector_connects_converts_and_completes(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let card = ui.card("c", 0.5, 0.5);
    ui.click(card);
    ui.click_on("add-REQUIRES");
    ui.type_text("a");
    ui.keys("enter");
    assert_eq!(ui.node("c").depends_on, [id("a")]);

    // The icon of a listed task completes it without leaving the selection.
    ui.click_on("row-REQUIRES-a-check");
    assert_eq!(ui.node("a").status, Status::Done);
    assert_eq!(ui.selected().as_deref(), Some("c"));

    // A member cannot become a milestone; a free task can, and back.
    assert_eq!(ui.cx.debug_bounds("convert"), None);
    ui.click_on("row-REQUIRES-a");
    assert_eq!(ui.selected().as_deref(), Some("a"));
    ui.click_on("convert");
    assert_eq!(ui.node("a").kind, Kind::Milestone);
    ui.click_on("convert");
    assert_eq!(ui.node("a").kind, Kind::Task);
}

#[gpui::test]
fn trackpad_gestures_zoom_around_the_pointer(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let pointer = ui.card("a", 0.0, 0.0);
    ui.move_to(pointer, false);
    let gesture = |ui: &mut Ui, gesture| ui.app.update_in(ui.cx, |app, window, cx| app.on_gesture(gesture, window, cx));
    gesture(&mut ui, Gesture::Pinch(0.5));
    assert_eq!(ui.read(|app| app.zoom), 1.5);
    // The corner of the card under the pointer stays under it.
    assert_eq!(ui.card("a", 0.0, 0.0), pointer);
    // A double-tap returns to actual size, and from there shows the whole graph.
    gesture(&mut ui, Gesture::SmartZoom);
    assert_eq!(ui.read(|app| app.target().1), 1.0);
}

#[gpui::test]
fn multiple_selection_bulk_edits_are_single_undo_steps(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.app.update(ui.cx, |app, cx| app.select_nodes(&[id("a"), id("c")], cx));
    assert_eq!(ui.read(|app| app.selected_nodes.len()), 2);
    ui.keys("2");
    assert_eq!(ui.node("a").status, Status::Doing);
    assert_eq!(ui.node("c").status, Status::Doing);
    ui.keys("cmd-z");
    assert_eq!(ui.node("a").status, Status::Todo);
    assert_eq!(ui.node("c").status, Status::Todo);
    ui.keys("backspace");
    assert!(ui.read(|app| app.graph().get(&id("a")).is_none() && app.graph().get(&id("c")).is_none()));
    assert!(ui.read(|app| app.selected_nodes.is_empty()));
    ui.keys("cmd-z");
    assert_eq!(ui.node("b").depends_on, [id("a")]);
    assert_eq!(ui.node("c").milestones, [id("m")]);
}

#[gpui::test]
fn multiple_selection_does_not_rename_or_connect_only_one_node(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.app.update(ui.cx, |app, cx| app.select_nodes(&[id("a"), id("c")], cx));
    ui.keys("enter l tab");
    assert!(ui.read(|app| app.prompt.is_none()));
    ui.keys("right");
    assert_eq!(ui.selected().as_deref(), Some("b"));
    assert_eq!(ui.read(|app| app.selected_nodes.len()), 1);
}

#[gpui::test]
fn toolbar_selects_all_matches_and_inspector_narrows_selection(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.click_on("stat-ready");
    assert_eq!(ui.read(|app| app.selected_nodes.clone()), [id("a"), id("c")].into_iter().collect());
    assert!(ui.cx.debug_bounds("deselect").is_some());
    ui.click_on("row-selected-c");
    assert_eq!(ui.selected().as_deref(), Some("c"));
    assert_eq!(ui.read(|app| app.selected_nodes.len()), 1);
}

#[gpui::test]
fn graph_reload_removes_only_missing_selected_nodes(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.app.update(ui.cx, |app, cx| {
        app.select_nodes(&[id("a"), id("c")], cx);
        let mut disk = Workspace::open(app.ws.dir().to_owned()).unwrap();
        disk.graph.remove(&id("a")).unwrap();
        disk.save().unwrap();
        app.reload(cx);
    });
    ui.cx.run_until_parked();
    assert_eq!(ui.selected().as_deref(), Some("c"));
    assert_eq!(ui.read(|app| app.selected_nodes.clone()), [id("c")].into_iter().collect());
}

#[gpui::test]
fn cloud_workspace_merges_other_writers_and_drops_stale_undo_steps(cx: &mut TestAppContext) {
    use std::sync::Arc;
    use topo_core::store::MemoryRemote;
    use topo_core::{Op, Remote};

    let add = |name: &str| -> Op {
        serde_json::from_value(serde_json::json!({ "op": "add", "id": name, "title": name })).unwrap()
    };
    let status = |name: &str, status| Op::Status { id: name.into(), status, if_status: None };
    let dir = tempfile::tempdir().unwrap();
    let remote = Arc::new(MemoryRemote::default());
    remote.apply(&[add("a"), add("b")]).unwrap();
    let ws = Workspace::open_remote(dir.path().to_owned(), remote.clone()).unwrap();
    let (app, cx) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
    let server = |remote: &MemoryRemote| -> Vec<(String, Status)> {
        let nodes = remote.fetch(None).unwrap().unwrap().nodes;
        nodes.into_iter().map(Node::from).map(|n| (n.id.0, n.status)).collect()
    };

    // An edit goes to the server and can be undone.
    app.update(cx, |app, cx| assert!(app.mutate(cx, |g| g.set_status(&id("a"), Status::Done))));
    cx.run_until_parked();
    assert_eq!(server(&remote), [("a".into(), Status::Done), ("b".into(), Status::Todo)]);
    assert_eq!(app.read_with(cx, |app, _| app.undo.len()), 1);

    // A poll brings in what another writer did. Undoing past it would undo their change.
    remote.apply(&[add("c")]).unwrap();
    app.update(cx, |app, cx| {
        let (remote, version) = app.ws.remote().unwrap();
        app.fetched(remote.fetch(Some(version)), cx);
    });
    cx.run_until_parked();
    app.update(cx, |app, cx| {
        assert!(app.graph().get(&id("c")).is_some());
        assert!(app.undo.is_empty());
        // A poll that was overtaken by a save of ours is ignored.
        let stale = topo_core::wire::Snapshot { version: 1, nodes: Vec::new() };
        app.fetched(Ok(Some(stale)), cx);
        assert!(app.graph().get(&id("c")).is_some());
    });

    // Another writer gets in between two polls: our edit is applied on top of theirs.
    remote.apply(&[status("b", Status::Doing)]).unwrap();
    app.update(cx, |app, cx| {
        assert!(app.mutate(cx, |g| g.set_status(&id("a"), Status::Todo)));
    });
    cx.run_until_parked();
    app.update(cx, |app, cx| {
        assert_eq!(app.graph().get(&id("b")).unwrap().status, Status::Doing);
        assert!(app.undo.is_empty());
        assert!(app.mutate(cx, |g| g.set_status(&id("c"), Status::Done)));
        assert_eq!(app.undo.len(), 1);
        app.restore(true, cx);
        assert_eq!((app.undo.len(), app.redo.len()), (0, 1));
    });
    let expected = [("a".into(), Status::Todo), ("b".into(), Status::Doing), ("c".into(), Status::Todo)];
    cx.run_until_parked();
    assert_eq!(server(&remote), expected);
}

#[gpui::test]
fn modifier_click_toggles_selection_and_middle_drag_preserves_it(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let a = ui.card("a", 0.5, 0.5);
    let c = ui.card("c", 0.5, 0.5);
    ui.click(a);
    let additive = Modifiers { platform: true, ..Modifiers::none() };
    ui.cx.simulate_mouse_down(c, MouseButton::Left, additive);
    ui.cx.simulate_mouse_up(c, MouseButton::Left, additive);
    assert_eq!(ui.read(|app| app.selected_nodes.len()), 2);
    let delta = point(px(60.), px(30.));
    let before = ui.read(|app| app.offset);
    ui.cx.simulate_mouse_down(a, MouseButton::Middle, Modifiers::none());
    // A different button's release must not terminate the middle drag.
    ui.cx.simulate_mouse_up(a, MouseButton::Left, Modifiers::none());
    ui.cx.simulate_mouse_move(a + delta, Some(MouseButton::Middle), Modifiers::none());
    ui.cx.simulate_mouse_up(a + delta, MouseButton::Middle, Modifiers::none());
    assert_eq!(ui.read(|app| app.offset), before + delta);
    assert_eq!(ui.read(|app| app.selected_nodes.len()), 2);
    let c = ui.card("c", 0.5, 0.5);
    ui.cx.simulate_mouse_down(c, MouseButton::Left, additive);
    ui.cx.simulate_mouse_up(c, MouseButton::Left, additive);
    assert_eq!(ui.selected().as_deref(), Some("a"));
}

#[gpui::test]
fn a_long_inspector_title_is_clamped_to_three_lines(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let card = ui.card("a", 0.5, 0.5);
    ui.click(card);
    let sentence = "とても長い日本語タイトルのマイルストーンで折り返しと省略を確認する。";
    let height = |ui: &mut Ui, text: String| {
        ui.app.update(ui.cx, |app, cx| {
            let id = id("a");
            app.mutate(cx, |graph| graph.edit(&id, Edit { title: Some(text), ..Edit::default() }));
        });
        ui.redraw();
        ui.cx.debug_bounds("title-text").expect("the title is rendered").size.height
    };
    let three_lines = height(&mut ui, sentence.repeat(3));
    // Twelve times the text must not add a fourth line.
    let twelve_lines = height(&mut ui, sentence.repeat(12));
    assert!(
        twelve_lines <= three_lines,
        "a longer title must not exceed the clamp: {three_lines:?} then {twelve_lines:?}"
    );
    assert!(three_lines <= px(96.), "the clamp shows at most three text_lg lines, got {three_lines:?}");
}

#[gpui::test]
fn dragging_the_inspector_border_resizes_and_clamps(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let handle = ui.cx.debug_bounds("inspector-resize").expect("resize handle is rendered");
    let before = ui.read(|app| app.inspector_width());
    // Drag the border left, widening the panel.
    let to = handle.center() - point(px(80.), px(0.));
    ui.drag(handle.center(), to);
    assert!((ui.read(|app| app.inspector_width()) - (before + 80.)).abs() < 1.5);

    // Dragging the border to the far left clamps to a share of the window.
    let handle = ui.cx.debug_bounds("inspector-resize").unwrap();
    ui.drag(handle.center(), point(px(20.), handle.center().y));
    let max = 1360. * crate::config::INSPECTOR_MAX_FRACTION;
    assert!((ui.read(|app| app.inspector_width()) - max).abs() < 1.5);

    // Dragging it to the far right clamps the other way.
    let handle = ui.cx.debug_bounds("inspector-resize").unwrap();
    ui.drag(handle.center(), point(px(1340.), handle.center().y));
    assert!((ui.read(|app| app.inspector_width()) - crate::config::INSPECTOR_MIN_WIDTH).abs() < 1.5);

    // The canvas never overlaps the panel it sits next to.
    let area = ui.read(|app| app.area.get());
    let inspector = ui.read(|app| app.inspector_width());
    assert!((f32::from(area.right()) - (1360. - inspector)).abs() < 1.5);
}

#[gpui::test]
fn the_inspector_starts_with_the_window_proportion(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    assert_eq!(ui.read(|app| app.inspector_width()), 1360. * crate::config::INSPECTOR_DEFAULT_FRACTION);
    // A narrow window falls back to the compact width.
    ui.resize(900., 700.);
    assert_eq!(ui.read(|app| app.inspector_width()), crate::config::INSPECTOR_COMPACT_WIDTH);
}

#[gpui::test]
fn wheel_zoom_is_anchored_and_modified_wheel_pans(cx: &mut TestAppContext) {
    use gpui::{ScrollDelta, ScrollWheelEvent};
    let mut ui = open(cx, SAMPLE);
    let position = ui.card("a", 0., 0.);
    let scroll = |ui: &mut Ui, delta, modifiers| {
        ui.cx.simulate_event(ScrollWheelEvent { position, delta, modifiers, ..Default::default() });
    };
    let zoom = ui.read(|app| app.zoom);
    scroll(&mut ui, ScrollDelta::Lines(point(0., 1.)), Modifiers::none());
    assert!((ui.read(|app| app.zoom) - zoom * 1.12).abs() < 0.0001);
    let corner = ui.card("a", 0., 0.);
    assert!((corner.x - position.x).abs() < px(0.01) && (corner.y - position.y).abs() < px(0.01));
    let before = ui.read(|app| app.offset);
    scroll(&mut ui, ScrollDelta::Lines(point(0., 1.)), Modifiers { shift: true, ..Modifiers::none() });
    assert_eq!(ui.read(|app| app.offset), before + point(px(40.), px(0.)));
    scroll(&mut ui, ScrollDelta::Lines(point(0., 1.)), Modifiers { control: true, ..Modifiers::none() });
    assert_eq!(ui.read(|app| app.offset), before + point(px(40.), px(40.)));
    let delta = point(px(15.), px(-20.));
    let before = ui.read(|app| app.offset);
    scroll(&mut ui, ScrollDelta::Pixels(delta), Modifiers::none());
    assert_eq!(ui.read(|app| app.offset), before + delta);
}

#[gpui::test]
fn bulk_inspector_status_changes_have_stable_bounds(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.app.update(ui.cx, |app, cx| app.select_nodes(&[id("a"), id("c")], cx));
    ui.redraw();
    let bounds = ui.cx.debug_bounds("status-1").unwrap();
    ui.click_on("status-1");
    assert_eq!(ui.node("a").status, Status::Doing);
    assert_eq!(ui.node("c").status, Status::Doing);
    assert_eq!(ui.cx.debug_bounds("status-1"), Some(bounds));
    ui.keys("cmd-z");
    assert_eq!(ui.node("a").status, Status::Todo);
    assert_eq!(ui.node("c").status, Status::Todo);
}

#[gpui::test]
fn canvas_help_remains_within_the_smallest_window(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.resize(720., 480.);
    ui.keys("?");
    let help = ui.cx.debug_bounds("help-card").expect("help rendered");
    let area = ui.read(|app| app.area.get());
    assert!(help.left() >= area.left() && help.right() <= area.right());
    assert!(help.top() >= area.top() && help.bottom() <= area.bottom());
}

#[gpui::test]
fn toolbar_frames_large_selections_in_both_dimensions(cx: &mut TestAppContext) {
    let names: Vec<String> = (0..50).map(|i| format!("task{i:02}")).collect();
    for chained in [false, true] {
        let dependencies: Vec<Vec<&str>> =
            (0..names.len()).map(|i| if chained && i > 0 { vec![names[i - 1].as_str()] } else { vec![] }).collect();
        let nodes: Vec<(&str, Kind, &[&str], &[&str])> = names
            .iter()
            .enumerate()
            .map(|(i, name)| (name.as_str(), Kind::Task, dependencies[i].as_slice(), &[][..]))
            .collect();
        let mut ui = open(cx, &nodes);
        if chained {
            ui.app.update(ui.cx, |app, cx| {
                let ids: Vec<_> = app.graph().nodes().map(|node| node.id.clone()).collect();
                app.mutate(cx, |graph| {
                    for id in ids {
                        graph.set_status(&id, Status::Doing)?;
                    }
                    Ok(())
                });
            });
            ui.redraw();
            ui.click_on("stat-doing");
        } else {
            ui.click_on("stat-ready");
        }
        ui.read(|app| {
            assert_eq!(app.selected_nodes.len(), names.len());
            let (offset, zoom) = app.target();
            assert!(zoom > 0. && zoom < crate::MIN_ZOOM);
            let area = app.area.get().size;
            for id in &app.selected_nodes {
                let (col, row) = app.graph_cache.cells()[id];
                let left = offset.x + px(col as f32 * crate::CELL_W * zoom);
                let top = offset.y + px(row as f32 * crate::CELL_H * zoom);
                assert!(left >= px(0.) && top >= px(0.));
                assert!(left + px(NODE_W * zoom) <= area.width);
                assert!(top + px(NODE_H * zoom) <= area.height);
            }
        });
        ui.app.update(ui.cx, |app, cx| {
            let (offset, zoom) = app.target();
            app.offset = offset;
            app.zoom = zoom;
            app.anim = None;
            let anchor = app.canvas_center();
            app.zoom_by(1.12, anchor, false);
            assert!((app.zoom - zoom * 1.12).abs() < 0.0001);
            app.zoom_by(1. / 1.12, anchor, false);
            assert!((app.zoom - zoom).abs() < 0.0001);
            app.zoom_by(0.8, anchor, false);
            assert!((app.zoom - zoom * 0.8).abs() < 0.0001);
            cx.notify();
        });
    }
}

#[gpui::test]
fn details_are_edited_in_place(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.select("a");

    // A key opens the row as a field holding the current value; Enter saves.
    ui.keys("d");
    assert_eq!(ui.inline(), Some((Field::Due, String::new())));
    assert!(ui.read(|app| app.prompt.is_none()), "the value is edited in its row, not in the prompt");
    assert!(ui.cx.debug_bounds("inline-edit").is_some());
    ui.type_text("2026-12-24");
    ui.keys("enter");
    assert_eq!(ui.inline(), None);
    assert_eq!(ui.node("a").due, Some(jiff::civil::date(2026, 12, 24)));

    // Input that cannot be saved keeps the field open and says why; typing clears the message.
    ui.keys("d");
    assert_eq!(ui.inline(), Some((Field::Due, "2026-12-24".into())));
    ui.type_text("someday");
    ui.keys("enter");
    assert!(ui.read(|app| app.inline.as_ref().unwrap().error.as_ref().unwrap().contains("not a date")));
    assert_eq!(ui.node("a").due, Some(jiff::civil::date(2026, 12, 24)));
    ui.type_text("!");
    assert!(ui.read(|app| app.inline.as_ref().unwrap().error.is_none()));
    // Escape cancels, and the keyboard is back on the canvas.
    ui.keys("escape");
    assert_eq!(ui.inline(), None);
    assert_eq!(ui.node("a").due, Some(jiff::civil::date(2026, 12, 24)));
    // An empty field clears the date.
    ui.keys("d backspace enter");
    assert_eq!(ui.node("a").due, None);

    // A click on the row does the same as its key.
    ui.click_on("prop-tags");
    assert_eq!(ui.inline(), Some((Field::Tags, String::new())));
    ui.type_text("#core ui core");
    ui.keys("enter");
    assert_eq!(ui.node("a").tags, ["core", "ui"]);
    ui.keys("t");
    assert_eq!(ui.inline(), Some((Field::Tags, "core ui".into())));
    ui.keys("escape");

    ui.keys("p");
    ui.type_text("hi");
    ui.keys("enter");
    assert_eq!(ui.node("a").priority, Some(Priority::High));
    ui.keys("a");
    ui.type_text(" claude ");
    ui.keys("enter");
    assert_eq!(ui.node("a").assignee.as_deref(), Some("claude"));
    ui.click_on("prop-assignee");
    ui.keys("backspace enter");
    assert_eq!(ui.node("a").assignee, None);
    ui.keys("p backspace enter");
    assert_eq!(ui.node("a").priority, None);

    // Every saved field is one undo step.
    ui.keys("cmd-z");
    assert_eq!(ui.node("a").priority, Some(Priority::High));
}

#[gpui::test]
fn an_edited_field_keeps_the_keyboard_from_the_canvas(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.select("a");
    ui.keys("x");
    assert_eq!(ui.node("a").status, Status::Done);

    ui.keys("a");
    ui.type_text("one two");
    // Keys that are canvas commands are text or text editing here.
    ui.type_text("x");
    ui.keys("backspace cmd-a");
    assert_eq!(ui.selection(), ["a"], "⌘A selects the text, not the nodes");
    assert_eq!(ui.node("a").status, Status::Done);
    assert_eq!(ui.inline(), Some((Field::Assignee, "one two".into())));
    ui.keys("cmd-x");
    assert_eq!(ui.inline(), Some((Field::Assignee, String::new())));
    assert!(ui.read(|app| app.graph().get(&id("a")).is_some()), "⌘X cuts the text, not the node");
    ui.keys("cmd-v cmd-v");
    assert_eq!(ui.inline(), Some((Field::Assignee, "one twoone two".into())));

    // ⌘Z undoes typing, never the graph, even with nothing left to undo.
    ui.keys("cmd-z cmd-z cmd-z cmd-z cmd-z cmd-z cmd-z");
    assert_eq!(ui.inline(), Some((Field::Assignee, String::new())));
    assert_eq!(ui.node("a").status, Status::Done);
    ui.keys("cmd-shift-z cmd-shift-z");
    assert_eq!(ui.inline(), Some((Field::Assignee, "one two".into())));
    ui.keys("enter");
    assert_eq!(ui.node("a").assignee.as_deref(), Some("one two"));

    // Back on the canvas the same key undoes the graph.
    ui.keys("cmd-z");
    assert_eq!(ui.node("a").assignee, None);
}

#[gpui::test]
fn a_field_that_loses_the_focus_is_cancelled(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.select("a");
    ui.keys("t");
    ui.type_text("draft");
    // A click inside the field keeps it open.
    ui.click_on("inline-edit");
    assert_eq!(ui.inline(), Some((Field::Tags, "draft".into())));
    // A click on another card selects it and drops the field unsaved.
    ui.select("c");
    assert_eq!(ui.inline(), None);
    assert_eq!(ui.selected().as_deref(), Some("c"));
    assert!(ui.node("a").tags.is_empty());
    // The keyboard works on the canvas again.
    ui.keys("x");
    assert_eq!(ui.node("c").status, Status::Done);

    // So does a click on the same card, on empty canvas, and on another row.
    ui.keys("t");
    ui.select("c");
    assert_eq!(ui.inline(), None);
    ui.keys("t");
    let empty = ui.read(|app| app.area.get().bottom_left()) + point(px(200.), px(-120.));
    ui.click(empty);
    assert_eq!(ui.inline(), None);
    ui.select("c");
    ui.keys("t");
    ui.click_on("prop-due");
    assert_eq!(ui.inline(), Some((Field::Due, String::new())));
}

#[gpui::test]
fn pull_requests_are_linked_opened_and_unlinked(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.select("a");
    ui.keys("g");
    ui.type_text("r4ai/topo#12");
    ui.keys("enter");
    ui.click_on("add-pr");
    ui.type_text("https://github.com/r4ai/topo/pull/7/files");
    ui.keys("enter");
    let prs = ["https://github.com/r4ai/topo/pull/12", "https://github.com/r4ai/topo/pull/7"];
    assert_eq!(ui.node("a").prs, prs);
    // Linking is not progress.
    assert_eq!(ui.node("a").status, Status::Todo);

    // The same pull request in another spelling, and text that is no URL, are refused in the row.
    for refused in ["https://github.com/r4ai/topo/pull/12/", "not a url"] {
        ui.keys("g");
        ui.type_text(refused);
        ui.keys("enter");
        assert!(ui.read(|app| app.inline.as_ref().unwrap().error.is_some()), "{refused}");
        ui.keys("escape");
    }
    // Enter on an empty field adds nothing.
    ui.keys("g enter");
    assert_eq!(ui.inline(), None);
    assert_eq!(ui.node("a").prs, prs);

    ui.click_on("pr-0");
    assert_eq!(ui.cx.opened_url().as_deref(), Some(prs[0]));
    ui.click_on("pr-0-remove");
    assert_eq!(ui.node("a").prs, [prs[1]]);
    ui.keys("cmd-z");
    assert_eq!(ui.node("a").prs, prs);
}

#[gpui::test]
fn the_canvas_selects_all_copies_and_pastes_nodes(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.keys("cmd-a");
    assert_eq!(ui.selection(), ["a", "b", "c", "m"]);
    // The multiple-selection commands apply to it.
    ui.keys("2");
    assert_eq!(ui.node("m").status, Status::Doing);
    ui.keys("cmd-z escape");

    ui.select("a");
    let more = Modifiers { platform: true, ..Modifiers::none() };
    let b = ui.card("b", 0.5, 0.5);
    ui.cx.simulate_mouse_down(b, MouseButton::Left, more);
    ui.cx.simulate_mouse_up(b, MouseButton::Left, more);
    ui.keys("cmd-c");
    let text = ui.cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(text.as_deref(), Some("- [ ] a\n- [ ] b"), "other applications get a list");

    ui.keys("cmd-v");
    assert_eq!(ui.read(|app| app.graph().nodes().count()), 6);
    let pasted = ui.selection();
    assert_eq!(pasted.len(), 2);
    let copies: Vec<Node> = pasted.iter().map(|id| ui.node(id)).collect();
    let (a2, b2) = match copies[0].title.as_str() {
        "a" => (&copies[0], &copies[1]),
        _ => (&copies[1], &copies[0]),
    };
    assert_eq!((b2.depends_on.clone(), b2.milestones.clone()), (vec![a2.id.clone()], vec![id("m")]));
    assert!(a2.created_at.is_some(), "a copy is created when it is pasted");

    // The paste is one undo step, and the cut is another.
    ui.keys("cmd-z");
    assert_eq!(ui.read(|app| app.graph().nodes().count()), 4);
    ui.app.update(ui.cx, |app, cx| {
        app.select(Some(id("c")), false);
        cx.notify();
    });
    ui.keys("cmd-x");
    assert!(ui.read(|app| app.graph().get(&id("c")).is_none()));
    ui.keys("cmd-v");
    assert_eq!(ui.titled("c").milestones, [id("m")]);
    ui.keys("cmd-z cmd-z");
    assert_eq!(ui.node("c").milestones, [id("m")]);

    // Text that is not a clip of nodes creates nothing.
    ui.cx.write_to_clipboard(gpui::ClipboardItem::new_string("- [ ] a".into()));
    ui.keys("cmd-v");
    assert_eq!(ui.read(|app| app.graph().nodes().count()), 4);
    assert!(ui.read(|app| app.toast.as_ref().is_some_and(|t| t.error)));
}

#[gpui::test]
fn the_priority_filter_dims_and_limits_select_all(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.app.update(ui.cx, |app, cx| {
        let set = |p| Edit { priority: Some(Some(p)), ..Edit::default() };
        app.mutate(cx, |g| g.edit(&id("a"), set(Priority::Urgent)).and_then(|()| g.edit(&id("c"), set(Priority::Low))));
    });
    ui.keys("shift-p");
    assert_eq!(ui.read(|app| app.priority_filter), Some(Priority::Urgent));
    ui.keys("cmd-a");
    assert_eq!(ui.selection(), ["a"]);
    ui.keys("escape");
    ui.click_on("priority-filter");
    ui.click_on("priority-filter");
    ui.click_on("priority-filter");
    assert_eq!(ui.read(|app| app.priority_filter), Some(Priority::Low));
    ui.keys("cmd-a");
    assert_eq!(ui.selection(), ["a", "c"]);
    ui.click_on("priority-filter");
    assert_eq!(ui.read(|app| app.priority_filter), None);
}

#[gpui::test]
fn the_prompt_field_has_the_standard_text_shortcuts(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let text = |ui: &mut Ui| ui.app.read_with(ui.cx, |app, cx| app.palette.read(cx).text(cx).to_owned());
    ui.keys("/");
    ui.type_text("日本語 text");
    ui.keys("cmd-a cmd-c right");
    ui.cx.write_to_clipboard(gpui::ClipboardItem::new_string("two\nlines".into()));
    ui.keys("cmd-v");
    assert_eq!(text(&mut ui), "日本語 texttwo lines");
    assert!(ui.selection().is_empty(), "⌘A in the search field selects no nodes");
    ui.keys("cmd-z");
    assert_eq!(text(&mut ui), "日本語 text");
    ui.keys("cmd-shift-z shift-left shift-left cmd-x");
    assert_eq!(text(&mut ui), "日本語 texttwo lin");
    assert_eq!(ui.cx.read_from_clipboard().and_then(|item| item.text()).as_deref(), Some("es"));
    // An empty clipboard pastes nothing.
    ui.cx.write_to_clipboard(gpui::ClipboardItem::new_string(String::new()));
    ui.keys("cmd-v escape");
    assert!(ui.read(|app| app.prompt.is_none()));
    assert_eq!(ui.read(|app| app.graph().nodes().count()), 4);
}

#[gpui::test]
fn recorded_times_show_in_the_narrowest_inspector(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.resize(720., 480.);
    ui.app.update(ui.cx, |app, _| app.select(Some(id("a")), false));
    ui.redraw();
    let created = ui.node("a").created_at.expect("saving records the creation");
    assert_eq!(ui.node("a").completed_at, None);
    ui.keys("x");
    assert!(ui.node("a").completed_at.is_some_and(|at| at >= created));
    ui.redraw();
    let inspector = ui.read(|app| app.inspector_width());
    for row in
        ["prop-priority", "prop-assignee", "prop-due", "prop-tags", "prop-created", "prop-updated", "prop-completed"]
    {
        let bounds = ui.cx.debug_bounds(row).unwrap_or_else(|| panic!("{row} is rendered"));
        assert!(bounds.left() >= px(720. - inspector) && bounds.right() <= px(720.), "{row} at {bounds:?}");
    }
    // Reopening forgets the completion.
    ui.keys("x");
    assert_eq!(ui.node("a").completed_at, None);
}

#[gpui::test]
fn tab_saves_a_field_and_opens_the_next(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.select("a");
    ui.keys("p");
    ui.type_text("u");
    ui.keys("tab");
    assert_eq!(ui.node("a").priority, Some(Priority::Urgent));
    assert_eq!(ui.inline(), Some((Field::Assignee, String::new())));
    ui.type_text("me");
    ui.keys("tab tab");
    assert_eq!(ui.node("a").assignee.as_deref(), Some("me"));
    assert_eq!(ui.inline(), Some((Field::Tags, String::new())));
    ui.type_text("gui");
    ui.keys("tab");
    assert_eq!(ui.node("a").tags, ["gui"]);
    // An empty pull request field is passed over, and the order wraps around.
    assert_eq!(ui.inline(), Some((Field::Pr, String::new())));
    ui.keys("tab");
    assert_eq!(ui.inline(), Some((Field::Title, "a".into())));
    ui.keys("tab");
    assert_eq!(ui.inline(), Some((Field::Priority, "urgent".into())));
    ui.keys("shift-tab shift-tab shift-tab shift-tab");
    assert_eq!(ui.inline(), Some((Field::Due, String::new())));

    // A field that cannot be saved keeps the focus.
    ui.type_text("someday");
    ui.keys("tab");
    assert_eq!(ui.inline(), Some((Field::Due, "someday".into())));
    assert!(ui.read(|app| app.inline.as_ref().unwrap().error.is_some()));
    ui.keys("escape");
    // Tab is still the follow-up key on the canvas.
    ui.keys("tab");
    assert!(matches!(ui.read(|app| app.prompt.clone()), Some(Prompt::Create(_))));
}

#[gpui::test]
fn a_field_offers_values_to_choose_from(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.app.update(ui.cx, |app, cx| {
        let edit = Edit {
            assignee: Some(Some("claude".into())),
            tags: Some(vec!["gui".into(), "core".into()]),
            ..Edit::default()
        };
        app.mutate(cx, |graph| graph.edit(&id("c"), edit));
    });
    let choices = |ui: &mut Ui| -> Vec<String> {
        ui.app.read_with(ui.cx, |app, cx| app.combo.read(cx).choices().iter().map(|c| c.value.clone()).collect())
    };
    ui.select("a");

    // The arrow keys choose and Enter takes the choice.
    ui.keys("p");
    assert_eq!(choices(&mut ui), ["urgent", "high", "medium", "low"]);
    ui.keys("down down up down enter");
    assert_eq!(ui.node("a").priority, Some(Priority::High));
    // The list says what Enter does: a highlighted choice, or else a note on the typed text or the refusal.
    let highlight = |ui: &mut Ui| ui.app.read_with(ui.cx, |app, cx| app.combo.read(cx).highlight());
    ui.keys("d");
    assert!(ui.cx.debug_bounds("choice-0").is_some() && ui.cx.debug_bounds("combo-note").is_some());
    assert_eq!(highlight(&mut ui), None, "Enter on an untouched empty field clears");
    ui.type_text("2026-12-24");
    assert_eq!(choices(&mut ui), ["2026-12-24"]);
    assert_eq!(highlight(&mut ui), Some(0));
    assert!(ui.cx.debug_bounds("combo-note").is_none());
    // Escape first closes the list, leaving the typed text; the second one cancels.
    ui.keys("escape");
    assert_eq!((ui.inline(), highlight(&mut ui)), (Some((Field::Due, "2026-12-24".into())), None));
    assert!(ui.cx.debug_bounds("choice-0").is_none());
    ui.keys("escape");
    assert_eq!(ui.inline(), None);
    ui.keys("p");
    ui.type_text("zz");
    ui.keys("enter");
    ui.redraw();
    assert!(ui.read(|app| app.inline.as_ref().unwrap().error.is_some()));
    assert!(ui.cx.debug_bounds("combo-note").is_some(), "an error is always shown");
    ui.keys("escape");

    // The list floats: opening a field moves no row below it.
    let below = ui.cx.debug_bounds("prop-tags").unwrap();
    ui.keys("p");
    ui.redraw();
    let list = ui.cx.debug_bounds("combo-list").expect("the list is open");
    assert_eq!(ui.cx.debug_bounds("prop-tags"), Some(below));
    assert!(list.bottom() > below.top(), "the list covers the rows below instead of pushing them");
    // The field completes the highlighted choice, and reopened it marks the current value.
    assert_eq!(highlight(&mut ui), Some(1), "the current priority, high");
    ui.type_text("u");
    assert_eq!((choices(&mut ui), highlight(&mut ui)), (vec!["urgent".to_owned()], Some(0)));
    ui.keys("escape escape");
    // Reopened, the list is whole again although the field holds a value; typing narrows it.
    ui.keys("p");
    assert_eq!(choices(&mut ui).len(), 4);
    ui.type_text("l");
    assert_eq!(choices(&mut ui), ["low"]);
    // Up from the first entry returns to the typed text, which Enter saves.
    ui.keys("down up enter");
    assert_eq!(ui.node("a").priority, Some(Priority::Low));

    // A click takes a choice too.
    ui.keys("a");
    assert_eq!(choices(&mut ui), ["claude"]);
    ui.click_on("choice-0");
    assert_eq!(ui.node("a").assignee.as_deref(), Some("claude"));
    assert_eq!(ui.inline(), None);

    // Tags: a space finishes a chip, a choice becomes one, Backspace and × remove one.
    ui.keys("t");
    assert_eq!(choices(&mut ui), ["core", "gui"]);
    ui.type_text("new ");
    assert_eq!(ui.app.read_with(ui.cx, |app, cx| app.combo.read(cx).chips().to_vec()), ["new"]);
    ui.type_text("g");
    assert_eq!(choices(&mut ui), ["gui"]);
    ui.keys("down enter");
    assert_eq!(ui.inline(), Some((Field::Tags, "new gui".into())));
    assert_eq!(choices(&mut ui), ["core"]);
    ui.type_text("x, y");
    ui.keys("backspace backspace");
    assert_eq!(ui.inline(), Some((Field::Tags, "new gui".into())));
    ui.click_on("chip-0-remove");
    assert_eq!(ui.inline(), Some((Field::Tags, "gui".into())));
    ui.type_text("last");
    ui.keys("enter");
    assert_eq!(ui.node("a").tags, ["gui", "last"]);

    ui.keys("d");
    ui.type_text("tom");
    ui.keys("down enter");
    assert_eq!(ui.node("a").due, Some(crate::dates::today().tomorrow().unwrap()));
}

#[gpui::test]
fn editing_the_title_keeps_its_bounds_and_the_details_still(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.select("a");
    for width in [1360., 800.] {
        ui.resize(width, 860.);
        for title in ["Alpha", "日本語タイトルの折り返しを確認するための少し長い名前"] {
            ui.app.update(ui.cx, |app, cx| {
                app.mutate(cx, |graph| graph.edit(&id("a"), Edit { title: Some(title.into()), ..Edit::default() }));
            });
            ui.redraw();
            let title_bounds = ui.cx.debug_bounds("title-text").unwrap();
            let status_bounds = ui.cx.debug_bounds("status-1").unwrap();
            let details_bounds = ui.cx.debug_bounds("prop-priority").unwrap();
            ui.keys("enter");
            assert_eq!(ui.cx.debug_bounds("combobox"), Some(title_bounds));
            assert_eq!(ui.cx.debug_bounds("status-1"), Some(status_bounds));
            assert_eq!(ui.cx.debug_bounds("prop-priority"), Some(details_bounds));
            ui.keys("escape");
            assert_eq!(ui.cx.debug_bounds("title-text"), Some(title_bounds));
        }
    }
}

#[gpui::test]
fn the_title_is_edited_in_the_inspector(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.select("a");
    ui.keys("enter");
    assert_eq!(ui.inline(), Some((Field::Title, "a".into())));
    assert!(ui.read(|app| app.prompt.is_none()));
    ui.type_text("Alpha");
    ui.keys("enter");
    assert_eq!(ui.node("a").title, "Alpha");
    // A click on the title, and a double-click on the card, open it too.
    ui.click_on("title");
    assert_eq!(ui.inline(), Some((Field::Title, "Alpha".into())));
    // A title cannot be emptied.
    ui.keys("backspace enter");
    assert!(ui.read(|app| app.inline.as_ref().unwrap().error.is_some()));
    ui.keys("escape");
    assert_eq!(ui.node("a").title, "Alpha");
    let card = ui.card("c", 0.6, 0.3);
    ui.click(card);
    ui.cx.simulate_event(gpui::MouseDownEvent {
        position: card,
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    ui.release(card);
    assert_eq!(ui.inline(), Some((Field::Title, "c".into())));
}

#[gpui::test]
fn notes_are_written_in_the_inspector(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.select("a");
    ui.keys("e");
    assert_eq!(ui.read(|app| app.notes.clone()), Some(id("a")));
    // Enter breaks the line and the canvas keys are text; ⌘Enter saves.
    ui.type_text("first x");
    ui.keys("enter");
    ui.type_text("second");
    assert_eq!(ui.notes(), "first x\nsecond");
    assert_eq!(ui.node("a").status, Status::Todo);
    ui.keys("cmd-enter");
    assert_eq!(ui.read(|app| app.notes.clone()), None);
    assert_eq!(ui.node("a").body, "first x\nsecond\n");

    // Up and Down move between the lines, keeping the column.
    ui.click_on("notes");
    ui.keys("up");
    ui.type_text("!");
    ui.keys("down");
    ui.type_text("?");
    assert_eq!(ui.notes(), "first !x\nsecond?");
    // Escape asks about the changes, and discarding them keeps the saved notes.
    ui.keys("escape");
    assert_eq!(ui.node("a").body, "first x\nsecond\n");
    assert!(ui.read(|app| app.notes_ask.is_some()));
    ui.keys("d");
    assert_eq!(ui.read(|app| (app.notes.clone(), app.notes_ask.clone())), (None, None));
    assert_eq!(ui.node("a").body, "first x\nsecond\n");

    // Another selection asks too, and saving it goes on to select.
    ui.click_on("edit-notes");
    ui.type_text(" more");
    ui.select("c");
    assert_eq!(ui.node("a").body, "first x\nsecond\n");
    assert_eq!(ui.selected().as_deref(), Some("a"));
    ui.keys("enter");
    assert_eq!(ui.node("a").body, "first x\nsecond more\n");
    assert_eq!(ui.selected().as_deref(), Some("c"));
    // Each save is one undo step, and emptied notes are no text at all.
    ui.keys("cmd-z");
    assert_eq!(ui.node("a").body, "first x\nsecond\n");
    ui.select("a");
    ui.keys("e cmd-a backspace cmd-enter");
    assert_eq!(ui.node("a").body, "");
}

#[gpui::test]
fn unsaved_notes_ask_before_they_are_left(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.select("a");
    ui.keys("e");
    // Nothing changed: the editor says so, and leaving it is silent.
    assert!(ui.cx.debug_bounds("notes-saved").is_some() && ui.cx.debug_bounds("notes-unsaved").is_none());
    ui.keys("escape");
    assert_eq!(ui.read(|app| (app.notes.clone(), app.notes_ask.clone())), (None, None));

    // A change shows, and the question over it has three answers.
    ui.keys("e");
    ui.type_text("draft");
    assert!(ui.cx.debug_bounds("notes-unsaved").is_some() && ui.cx.debug_bounds("notes-saved").is_none());
    ui.keys("escape");
    assert!(ui.cx.debug_bounds("notes-dialog").is_some());
    assert_eq!(ui.node("a").body, "");
    // Keep editing: the text and the cursor are as they were, and the keys are text again.
    ui.click_on("notes-keep");
    assert_eq!(ui.read(|app| app.notes_ask.clone()), None);
    ui.type_text("!");
    assert_eq!(ui.notes(), "draft!");
    // Typing the change away is no change.
    ui.keys("backspace backspace backspace backspace backspace backspace");
    assert!(ui.cx.debug_bounds("notes-saved").is_some());
    ui.keys("escape");
    assert_eq!(ui.read(|app| app.notes.clone()), None);

    // Discard.
    ui.keys("e");
    ui.type_text("gone");
    ui.keys("escape");
    ui.click_on("notes-discard");
    assert_eq!((ui.read(|app| app.notes.clone()), ui.node("a").body.as_str()), (None, ""));
    assert!(ui.cx.debug_bounds("notes-dialog").is_none());

    // Save, from a click elsewhere on the canvas.
    ui.keys("e");
    ui.type_text("kept");
    let empty = ui.card("a", 0.5, 0.5) + point(px(0.), px(300.));
    ui.click(empty);
    assert!(ui.read(|app| app.notes_ask.is_some()));
    ui.click_on("notes-save");
    assert_eq!((ui.read(|app| app.notes.clone()), ui.node("a").body.as_str()), (None, "kept\n"));
    // The keys the canvas knows are the canvas's again.
    ui.keys("2");
    assert_eq!(ui.node("a").status, Status::Doing);
}

#[gpui::test]
fn another_selection_waits_for_the_answer(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    for (answer, body, selected) in [("escape", "", "a"), ("d", "", "c"), ("enter", "x\n", "c")] {
        ui.select("a");
        ui.keys("e");
        ui.type_text("x");
        ui.select("c");
        // The selection is held back: the inspector keeps showing the notes.
        assert_eq!(ui.selected().as_deref(), Some("a"));
        assert!(ui.cx.debug_bounds("notes-editor").is_some());
        ui.keys(answer);
        assert_eq!((ui.selected().as_deref(), ui.node("a").body.as_str()), (Some(selected), body), "{answer}");
        // Put the state back for the next round.
        if ui.read(|app| app.notes.is_some()) {
            ui.keys("escape d");
        }
        ui.app.update(ui.cx, |app, cx| {
            app.mutate(cx, |g| g.edit(&id("a"), Edit { body: Some(String::new()), ..Edit::default() }));
        });
    }
}

#[gpui::test]
fn closing_the_window_with_unsaved_notes_asks(cx: &mut TestAppContext) {
    use crate::notes::Then;
    let mut ui = open(cx, SAMPLE);
    ui.select("a");
    ui.keys("e");
    // Nothing changed, nothing to ask.
    assert!(ui.app.update(ui.cx, |app, cx| app.can_leave_to(Then::CloseWindow, cx)));
    ui.type_text("x");
    assert!(!ui.app.update(ui.cx, |app, cx| app.can_leave_to(Then::CloseWindow, cx)));
    ui.redraw();
    assert_eq!(ui.read(|app| app.notes_ask.clone()), Some(Then::CloseWindow));
    assert!(ui.cx.debug_bounds("notes-dialog").is_some());
    // The question, not the text, has the keyboard.
    ui.type_text("y");
    assert_eq!(ui.notes(), "x");
    ui.keys("escape");
    assert_eq!(ui.read(|app| app.notes_ask.clone()), None);
    ui.type_text("y");
    assert_eq!(ui.notes(), "xy");

    // The window closes once the answer settles the notes.
    assert!(!ui.app.update(ui.cx, |app, cx| app.can_leave_to(Then::CloseWindow, cx)));
    ui.redraw();
    ui.keys("d");
    assert_eq!(ui.node("a").body, "");
    assert!(ui.cx.windows().is_empty());
}

#[gpui::test]
fn lines_start_and_end_where_the_line_breaks_are(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.select("a");
    ui.keys("e");
    ui.type_text("one two");
    ui.keys("enter");
    ui.type_text("three four");
    ui.keys("enter");
    ui.type_text("five");
    // 0..7 "one two", 8..18 "three four", 19..23 "five".
    assert_eq!(ui.sel(), 23..23);
    let moves: &[(&str, Range<usize>)] = &[
        ("home", 19..19),
        ("end", 23..23),
        ("up", 12..12),
        ("home", 8..8),
        ("cmd-right", 18..18),
        ("cmd-left", 8..8),
        ("cmd-up", 0..0),
        ("cmd-down", 23..23),
        ("ctrl-home", 0..0),
        ("ctrl-end", 23..23),
        ("left", 22..22),
        ("alt-left", 19..19),
        ("alt-right", 23..23),
    ];
    for (keys, expected) in moves {
        ui.keys(keys);
        assert_eq!(&ui.sel(), expected, "{keys}");
    }
    if cfg!(target_os = "macos") {
        ui.keys("ctrl-a");
        assert_eq!(ui.sel(), 19..19);
        ui.keys("ctrl-e");
        assert_eq!(ui.sel(), 23..23);
    }
}

#[gpui::test]
fn moving_with_shift_extends_the_selection(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.select("a");
    ui.keys("e");
    ui.type_text("one two");
    ui.keys("enter");
    ui.type_text("three four");
    // 0..7 "one two", 8..18 "three four"; the cursor is at 18.
    let extend: &[(&str, Range<usize>)] = &[
        ("shift-home", 8..18),
        ("shift-end", 18..18),
        ("cmd-shift-left", 8..18),
        ("cmd-shift-right", 18..18),
        ("cmd-shift-up", 0..18),
        ("cmd-shift-down", 18..18),
        ("ctrl-shift-home", 0..18),
        ("ctrl-shift-end", 18..18),
        ("alt-shift-left", 14..18),
        ("alt-shift-right", 18..18),
        ("shift-left", 17..18),
    ];
    for (keys, expected) in extend {
        ui.keys(keys);
        assert_eq!(&ui.sel(), expected, "{keys}");
    }
    // The selection grows from where it started, even past it.
    ui.keys("home");
    ui.keys("shift-end shift-home");
    assert_eq!(ui.sel(), 8..8);
    ui.keys("cmd-up");
    ui.keys("shift-end");
    assert_eq!(ui.sel(), 0..7);
    ui.keys("shift-end");
    assert_eq!(ui.sel(), 0..7);
    ui.keys("shift-right shift-end");
    assert_eq!(ui.sel(), 0..18);
    // Typing replaces what is selected.
    ui.type_text("x");
    assert_eq!(ui.notes(), "x");
}

#[gpui::test]
fn deleting_to_the_start_stops_at_the_line(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.select("a");
    ui.keys("e");
    ui.type_text("one");
    ui.keys("enter");
    ui.type_text("two");
    ui.keys("cmd-backspace");
    assert_eq!(ui.notes(), "one\n");
    // At the start of a line the line break goes.
    ui.keys("cmd-backspace");
    assert_eq!(ui.notes(), "one");
}

#[gpui::test]
fn tab_indents_and_shift_tab_outdents(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.select("a");
    ui.keys("e");
    ui.type_text("plain");
    // In a plain line Tab types an indentation where the cursor is, and stays in the editor.
    ui.keys("home tab");
    assert_eq!((ui.notes().as_str(), ui.sel()), ("  plain", 2..2));
    ui.keys("shift-tab");
    assert_eq!((ui.notes().as_str(), ui.sel()), ("plain", 0..0));
    ui.keys("shift-tab");
    assert_eq!(ui.notes(), "plain");
    assert!(ui.read(|app| app.notes.is_some()));

    // On a list item it indents the item, and the cursor stays in the text.
    ui.keys("cmd-a backspace");
    ui.type_text("- one");
    ui.keys("enter");
    ui.type_text("two");
    ui.keys("tab");
    assert_eq!((ui.notes().as_str(), ui.sel()), ("- one\n  - two", 13..13));
    // A selection of several lines indents each of them and stays selected.
    ui.keys("cmd-a tab");
    assert_eq!((ui.notes().as_str(), ui.sel()), ("  - one\n    - two", 2..17));
    ui.keys("shift-tab");
    assert_eq!((ui.notes().as_str(), ui.sel()), ("- one\n  - two", 0..13));
    // Each is one undo step.
    ui.keys("cmd-z");
    assert_eq!(ui.notes(), "  - one\n    - two");
    ui.keys("cmd-z");
    assert_eq!(ui.notes(), "- one\n  - two");
    ui.keys("cmd-z");
    assert_eq!(ui.notes(), "- one\n- two");
    ui.keys("cmd-shift-z");
    assert_eq!(ui.notes(), "- one\n  - two");
}

#[gpui::test]
fn enter_continues_lists_and_undo_follows_each_step(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.select("a");
    ui.keys("e");
    ui.type_text("- one");
    ui.keys("enter");
    assert_eq!(ui.notes(), "- one\n- ");
    ui.type_text("two");
    ui.keys("enter enter");
    // The empty item ends the list.
    assert_eq!((ui.notes().as_str(), ui.sel()), ("- one\n- two\n", 12..12));
    // Each is one undo step: the end of the list, the second item with what was typed in it, the first.
    let steps = ["- one\n- two\n- ", "- one\n- two", "- one", ""];
    for step in steps {
        ui.keys("cmd-z");
        assert_eq!(ui.notes(), step);
    }
    for step in steps.iter().rev().skip(1).chain(&["- one\n- two\n"]) {
        ui.keys("cmd-shift-z");
        assert_eq!(&ui.notes(), step);
    }

    // Shift-Enter breaks the line plainly; a quote and a task carry on.
    ui.keys("cmd-a backspace");
    ui.type_text("- [x] done");
    ui.keys("shift-enter");
    assert_eq!(ui.notes(), "- [x] done\n");
    ui.keys("backspace enter");
    assert_eq!(ui.notes(), "- [x] done\n- [ ] ");
    ui.keys("enter");
    ui.type_text("> q");
    ui.keys("enter");
    assert_eq!(ui.notes(), "- [x] done\n> q\n> ");
}

#[gpui::test]
fn the_inspector_follows_the_cursor_through_long_notes(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.select("a");
    ui.resize(1360., 480.);
    ui.keys("e");
    assert_eq!(ui.read(|app| app.notes.clone()), Some(id("a")));
    for line in 0..60 {
        ui.type_text(&format!("line {line}"));
        ui.keys("enter");
    }
    ui.redraw();
    // Whether the row `row` (of 18 points) of the notes is inside the panel, with a line to spare.
    let visible = |ui: &mut Ui, row: usize| {
        let editor = ui.cx.debug_bounds("notes-editor").expect("the editor is shown");
        let view = ui.read(|app| app.inspector_scroll.bounds());
        let top = editor.top() + px(13. + 18. * row as f32);
        top >= view.top() + px(18.) && top + px(18.) <= view.bottom() - px(18.)
    };
    let offset = |ui: &mut Ui| ui.read(|app| app.inspector_scroll.offset().y);
    // Typing at the end scrolled down to it.
    assert!(visible(&mut ui, 60));
    let end = offset(&mut ui);
    assert!(end < px(-300.), "the notes are longer than the panel");
    // Moving within the view does not scroll; moving out of it does.
    ui.keys("up up up");
    ui.redraw();
    assert_eq!(offset(&mut ui), end);
    ui.keys("cmd-up");
    ui.redraw();
    assert!(visible(&mut ui, 0));
    ui.keys("cmd-down");
    ui.redraw();
    assert!(visible(&mut ui, 60) && offset(&mut ui) == end);
    // A caret in the middle comes into view by scrolling no further than it takes.
    ui.keys("cmd-up");
    ui.keys(&"down ".repeat(40));
    ui.redraw();
    assert!(visible(&mut ui, 40));
    assert!(!visible(&mut ui, 0) && !visible(&mut ui, 60));
}

#[gpui::test]
fn cloud_metadata_edits_reload_server_times_and_survive_undo(cx: &mut TestAppContext) {
    use std::sync::Arc;
    use topo_core::store::MemoryRemote;
    use topo_core::{Op, Remote};

    let dir = tempfile::tempdir().unwrap();
    let remote = Arc::new(MemoryRemote::default());
    let add: Op = serde_json::from_value(serde_json::json!({ "op": "add", "id": "a", "title": "a" })).unwrap();
    remote.apply(&[add]).unwrap();
    let ws = Workspace::open_remote(dir.path().to_owned(), remote.clone()).unwrap();
    cx.update(text_input::bind_keys);
    let (app, cx) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
    let mut ui = Ui { app, cx, _dir: dir };
    ui.resize(1360., 860.);
    ui.select("a");
    assert_eq!(ui.selected().as_deref(), Some("a"));
    ui.resize(800., 640.);
    let stored = || Node::from(remote.fetch(None).unwrap().unwrap().nodes.remove(0));
    let created = stored().created_at;
    assert!(created.is_some());

    ui.keys("p");
    ui.type_text("urgent");
    ui.keys("enter");
    assert_eq!(stored().priority, Some(Priority::Urgent));
    assert_eq!(ui.node("a"), stored());
    assert_eq!(stored().created_at, created);
    for selector in ["prop-priority", "prop-created", "prop-updated", "prop-completed"] {
        assert!(ui.cx.debug_bounds(selector).is_some());
    }
    ui.keys("3");
    assert!(stored().completed_at.is_some());
    assert_eq!(ui.node("a"), stored());
    ui.keys("cmd-z");
    assert_eq!((stored().status, stored().completed_at), (Status::Todo, None));
    ui.keys("cmd-shift-z");
    assert_eq!(stored().status, Status::Done);
    assert!(stored().completed_at.is_some());
    ui.keys("p backspace enter");
    assert_eq!(stored().priority, None);
    assert_eq!(ui.node("a"), stored());
    assert!(!ui.read(|app| app.ws.nodes_dir().exists()), "cloud edits never write node files");
}

#[gpui::test]
fn a_cloud_workspace_edits_notes_in_place(cx: &mut TestAppContext) {
    use std::sync::Arc;
    use topo_core::store::MemoryRemote;
    use topo_core::{Op, Remote};

    let add: Op = serde_json::from_value(serde_json::json!({ "op": "add", "id": "a", "title": "a" })).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let remote = Arc::new(MemoryRemote::default());
    remote.apply(&[add]).unwrap();
    let ws = Workspace::open_remote(dir.path().to_owned(), remote.clone()).unwrap();
    cx.update(text_input::bind_keys);
    let (app, cx) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
    let mut ui = Ui { app, cx, _dir: dir };
    ui.resize(1360., 860.);
    ui.select("a");
    // There is no file to open, so the file key edits here and the file button is gone.
    assert_eq!(ui.cx.debug_bounds("open-file"), None);
    ui.keys("o");
    assert_eq!(ui.read(|app| app.notes.clone()), Some(id("a")));
    ui.type_text("from the gui");
    ui.keys("cmd-enter");
    let stored: Vec<Node> = remote.fetch(None).unwrap().unwrap().nodes.into_iter().map(Node::from).collect();
    assert_eq!(stored[0].body, "from the gui\n");
    assert!(ui.read(|app| app.toast.as_ref().is_none_or(|t| !t.error)));
}

/// a (done, #core #ui) → b (todo, #ui) → m (todo, no tag); c dropped, no tag; d todo, #core.
fn view_fixture(ui: &mut Ui) {
    ui.app.update(ui.cx, |app, cx| {
        let tags =
            |tags: &[&str]| Edit { tags: Some(tags.iter().map(|t| (*t).to_owned()).collect()), ..Edit::default() };
        assert!(app.mutate(cx, |g| {
            g.set_status(&id("a"), Status::Done)?;
            g.set_status(&id("c"), Status::Dropped)?;
            g.edit(&id("a"), tags(&["core", "ui"]))?;
            g.edit(&id("b"), tags(&["ui"]))?;
            g.edit(&id("d"), tags(&["core"]))
        }));
    });
}

const VIEW_SAMPLE: &[(&str, Kind, &[&str], &[&str])] = &[
    ("a", Kind::Task, &[], &[]),
    ("b", Kind::Task, &["a"], &["m"]),
    ("c", Kind::Task, &[], &["m"]),
    ("d", Kind::Task, &[], &[]),
    ("m", Kind::Milestone, &[], &[]),
];

impl Ui<'_> {
    /// Ids of the cards placed on the canvas, once per placement.
    fn shown(&mut self) -> Vec<String> {
        self.redraw();
        let mut ids = self.read(|app| {
            let mut ids: Vec<String> = app.graph_cache.cells().keys().map(|id| id.to_string()).collect();
            ids.sort();
            ids
        });
        ids.dedup();
        ids
    }

    fn placements(&mut self, node: &str) -> usize {
        self.redraw();
        self.read(|app| app.graph_cache.placements(&id(node)))
    }
}

#[gpui::test]
fn hiding_completed_removes_done_and_dropped_nodes_and_their_edges(cx: &mut TestAppContext) {
    let mut ui = open(cx, VIEW_SAMPLE);
    view_fixture(&mut ui);
    assert_eq!(ui.shown(), ["a", "b", "c", "d", "m"]);
    let edges = ui.read(|app| app.graph_cache.edge_count());
    assert_eq!(edges, 3);
    ui.keys("shift-h");
    assert!(ui.read(|app| app.view.hide_completed));
    assert_eq!(ui.shown(), ["b", "d", "m"]);
    // b → m is the only edge left; b lost its requirement a without being re-routed.
    assert_eq!(ui.read(|app| app.graph_cache.edge_count()), 1);
    assert_eq!(ui.read(|app| app.graph_cache.cells()[&id("b")]), (0, 0));
    ui.click_on("hide-completed");
    assert!(!ui.read(|app| app.view.hide_completed));
    assert_eq!(ui.shown(), ["a", "b", "c", "d", "m"]);
}

#[gpui::test]
fn hiding_the_selected_node_clears_the_selection(cx: &mut TestAppContext) {
    let mut ui = open(cx, VIEW_SAMPLE);
    view_fixture(&mut ui);
    ui.keys("shift-h");
    ui.select("b");
    ui.keys("3");
    assert_eq!(ui.shown(), ["d", "m"]);
    assert_eq!(ui.selected(), None);
    assert!(ui.read(|app| app.selected_nodes.is_empty()));
    // Turning the toggle on with a done node selected clears it too.
    ui.keys("shift-h");
    ui.select("a");
    assert_eq!(ui.selected().as_deref(), Some("a"));
    ui.keys("shift-h");
    assert_eq!(ui.selected(), None);
    // Select all only takes what is shown.
    ui.keys("cmd-a");
    assert_eq!(ui.selection(), ["d", "m"]);
}

#[gpui::test]
fn hide_completed_composes_with_the_priority_filter_and_search(cx: &mut TestAppContext) {
    let mut ui = open(cx, VIEW_SAMPLE);
    view_fixture(&mut ui);
    ui.app.update(ui.cx, |app, cx| {
        let set = |p| Edit { priority: Some(Some(p)), ..Edit::default() };
        app.mutate(cx, |g| {
            g.edit(&id("a"), set(Priority::Urgent)).and_then(|()| g.edit(&id("b"), set(Priority::Urgent)))
        });
    });
    ui.keys("shift-h shift-p");
    // Only b is both open and urgent; a is hidden even though it passes the filter.
    ui.keys("cmd-a");
    assert_eq!(ui.selection(), ["b"]);
    ui.keys("escape /");
    ui.type_text("a");
    let listed = ui.app.read_with(ui.cx, |app, cx| app.listed(cx));
    assert!(!listed.contains(&id("a")), "a hidden node is not offered by search");
}

#[gpui::test]
fn grouping_places_a_multi_tag_node_in_each_of_its_groups(cx: &mut TestAppContext) {
    let mut ui = open(cx, VIEW_SAMPLE);
    view_fixture(&mut ui);
    ui.keys("shift-g");
    assert!(ui.read(|app| app.view.group_by_tag));
    let groups = ui.read(|app| app.graph_cache.group_labels());
    // Tags are sorted; untagged comes last.
    assert_eq!(groups, ["#core", "#ui", "untagged"]);
    assert_eq!(ui.placements("a"), 2);
    assert_eq!(ui.placements("b"), 1);
    assert_eq!(ui.placements("d"), 1);
    // c and m have no tags: one card each, in the single untagged group.
    assert_eq!(ui.placements("c"), 1);
    assert_eq!(ui.placements("m"), 1);
    // Folding the untagged group removes c and m; unfolding brings them back.
    ui.click_on("group-untagged");
    assert_eq!(ui.placements("c"), 0);
    assert_eq!(ui.placements("m"), 0);
    ui.click_on("group-untagged");
    assert_eq!(ui.placements("c"), 1);
}

#[gpui::test]
fn collapsing_a_group_hides_its_cards_but_not_copies_in_other_groups(cx: &mut TestAppContext) {
    let mut ui = open(cx, VIEW_SAMPLE);
    view_fixture(&mut ui);
    ui.keys("shift-g");
    ui.click_on("group-#core");
    assert!(ui.read(|app| app.view.collapsed.contains(&Group::Tag("core".into()))));
    // a stays visible through #ui; d only lived in #core.
    assert_eq!(ui.placements("a"), 1);
    assert_eq!(ui.placements("d"), 0);
    // Selecting d is impossible while its only group is folded: select-all skips it.
    ui.keys("cmd-a");
    assert!(!ui.selection().contains(&"d".to_owned()));
    ui.click_on("group-#core");
    assert_eq!(ui.placements("d"), 1);
    assert_eq!(ui.placements("a"), 2);
}

#[gpui::test]
fn folding_the_group_of_the_selection_clears_it(cx: &mut TestAppContext) {
    let mut ui = open(cx, VIEW_SAMPLE);
    view_fixture(&mut ui);
    ui.keys("shift-g");
    ui.select("d");
    assert_eq!(ui.selected().as_deref(), Some("d"));
    ui.click_on("group-#core");
    assert_eq!(ui.selected(), None);
}

#[gpui::test]
fn grouping_and_hiding_completed_compose(cx: &mut TestAppContext) {
    let mut ui = open(cx, VIEW_SAMPLE);
    view_fixture(&mut ui);
    ui.keys("shift-g shift-h");
    // a (done) is gone from both of its groups, c (dropped) from untagged.
    assert_eq!(ui.placements("a"), 0);
    assert_eq!(ui.placements("c"), 0);
    assert_eq!(ui.read(|app| app.graph_cache.group_labels()), ["#core", "#ui", "untagged"]);
    assert_eq!(ui.read(|app| app.graph_cache.group_counts()), [1, 1, 1]);
    // Editing tags regroups while grouped.
    ui.app.update(ui.cx, |app, cx| {
        app.mutate(cx, |g| g.edit(&id("d"), Edit { tags: Some(vec!["ops".into()]), ..Edit::default() }));
    });
    assert_eq!(ui.read(|app| app.graph_cache.group_labels()), ["#ops", "#ui", "untagged"]);
}

#[gpui::test]
fn the_bottom_bar_fits_the_smallest_window_with_every_toggle_state(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.resize(720., 480.);
    ui.keys("shift-h shift-g");
    // Longest label of the priority filter last: "Priority: medium and up".
    for step in 0..5 {
        ui.redraw();
        let area = ui.read(|app| app.area.get());
        let mut previous = None;
        for selector in ["hide-completed", "group-by-tag", "priority-filter", "zoom"] {
            let bounds = ui.cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is rendered"));
            assert!(bounds.right() <= area.right(), "{selector} ends at {:?} (step {step})", bounds.right());
            assert!(bounds.size.height <= px(32.), "{selector} is {:?} high", bounds.size.height);
            if let Some(before) = previous {
                assert!(bounds.left() >= before, "{selector} overlaps its neighbour (step {step})");
            }
            previous = Some(bounds.right());
        }
        ui.keys("shift-p");
    }
}

#[gpui::test]
fn arrow_keys_skip_hidden_nodes(cx: &mut TestAppContext) {
    let mut ui = open(cx, VIEW_SAMPLE);
    view_fixture(&mut ui);
    ui.keys("shift-h");
    // b requires the hidden a; there is no card to move left to.
    ui.select("b");
    ui.keys("left");
    assert_eq!(ui.selected().as_deref(), Some("b"));
    ui.keys("right");
    assert_eq!(ui.selected().as_deref(), Some("m"));
    ui.keys("left");
    assert_eq!(ui.selected().as_deref(), Some("b"));
}

#[gpui::test]
fn arrow_keys_walk_through_group_bands_and_repeated_cards(cx: &mut TestAppContext) {
    let mut ui = open(cx, VIEW_SAMPLE);
    view_fixture(&mut ui);
    ui.keys("shift-g");
    // Column 0, top to bottom: #core (a, d), #ui (a, with b to its right), untagged (c).
    ui.select("d");
    let mut walked = vec![];
    for _ in 0..3 {
        ui.keys("down");
        walked.push(ui.selected().unwrap());
    }
    assert_eq!(walked, ["a", "c", "c"]);
    for _ in 0..2 {
        ui.keys("up");
        walked.push(ui.selected().unwrap());
    }
    // The second card of a was reached from below, so up goes on to d, not back to a's first card.
    assert_eq!(walked[3..], ["a", "d"]);
    // From a's second card the follow-up is b of the #ui band.
    ui.keys("down down up");
    assert_eq!(ui.selected().as_deref(), Some("a"));
    ui.keys("right");
    assert_eq!(ui.selected().as_deref(), Some("b"));
}

const CROSS_SAMPLE: &[(&str, Kind, &[&str], &[&str])] = &[
    ("a", Kind::Task, &[], &[]),
    ("m", Kind::Milestone, &["a"], &[]),
    ("b", Kind::Task, &["a"], &["m"]),
    ("c", Kind::Task, &[], &["m"]),
    ("w", Kind::Task, &[], &["m"]),
];

/// Bands #gui (m, b), #notes (a, b, w) and #ui (c, w); m depends on a, which shares no band with it.
fn cross_fixture(ui: &mut Ui) {
    ui.app.update(ui.cx, |app, cx| {
        let tags =
            |tags: &[&str]| Edit { tags: Some(tags.iter().map(|t| (*t).to_owned()).collect()), ..Edit::default() };
        assert!(app.mutate(cx, |g| {
            g.edit(&id("a"), tags(&["notes"]))?;
            g.edit(&id("m"), tags(&["gui"]))?;
            g.edit(&id("b"), tags(&["gui", "notes"]))?;
            g.edit(&id("c"), tags(&["ui"]))?;
            g.edit(&id("w"), tags(&["notes", "ui"]))
        }));
    });
    ui.keys("shift-g");
}

impl Ui<'_> {
    /// The selection's cross-band edges as `from>to` (`from~to` for membership), sorted.
    fn cross(&mut self) -> Vec<String> {
        self.redraw();
        let mut edges: Vec<String> = self.read(|app| {
            let edges = app.graph_cache.cross_edges();
            edges.iter().map(|(f, t, member, ..)| format!("{f}{}{t}", if *member { "~" } else { ">" })).collect()
        });
        edges.sort();
        edges
    }

    fn cards(&mut self, node: &str) -> Vec<crate::layout::Cell> {
        self.read(|app| app.graph_cache.cards(&id(node)))
    }
}

#[gpui::test]
fn a_selected_node_draws_its_edges_that_share_no_band(cx: &mut TestAppContext) {
    let mut ui = open(cx, CROSS_SAMPLE);
    cross_fixture(&mut ui);
    // b → m (#gui) and a → b (#notes) are drawn inside a band; nothing crosses without a selection.
    assert!(ui.cross().is_empty());
    let in_band = ui.read(|app| app.graph_cache.edge_count());
    // The #gui milestone depends on a task tagged only #notes; c and w are members from other bands.
    ui.select("m");
    assert_eq!(ui.cross(), ["a>m", "c~m", "w~m"]);
    // Edges already drawn within a band stay as they are and are not duplicated.
    assert_eq!(ui.read(|app| app.graph_cache.edge_count()), in_band);
    // Selecting the other end shows the same edge; a node whose edges are all in-band shows none.
    ui.select("a");
    assert_eq!(ui.cross(), ["a>m"]);
    ui.select("b");
    assert!(ui.cross().is_empty());
    // Multi-select covers both ends and every kind once.
    ui.keys("cmd-a");
    assert_eq!(ui.cross(), ["a>m", "c~m", "w~m"]);
    ui.keys("escape");
    assert!(ui.cross().is_empty());
}

#[gpui::test]
fn cross_band_edges_join_the_active_card_to_the_nearest_card(cx: &mut TestAppContext) {
    let mut ui = open(cx, CROSS_SAMPLE);
    cross_fixture(&mut ui);
    let (m, w) = (ui.cards("m")[0], ui.cards("w"));
    assert_eq!(w.len(), 2);
    // Without a clicked card the first placement is used; m's only card is the nearest of its ends.
    ui.select("w");
    let ends = ui.read(|app| app.graph_cache.cross_edges().iter().map(|e| (e.3, e.4)).collect::<Vec<_>>());
    assert_eq!(ends, [(w[0], m)]);
    // Landing on w's card in the #ui band (by click or arrow keys) makes that one the end of the line.
    ui.app.update(ui.cx, |app, _| app.placed = Some(w[1]));
    ui.redraw();
    let ends = ui.read(|app| app.graph_cache.cross_edges().iter().map(|e| (e.3, e.4)).collect::<Vec<_>>());
    assert_eq!(ends, [(w[1], m)]);
    // A node with two cards connects to the nearest of them: a's #notes card from m.
    ui.select("m");
    let a = ui.cards("a");
    let ends = ui.read(|app| app.graph_cache.cross_edges().iter().map(|e| (e.3, e.4)).find(|e| e.0 == a[0]));
    assert_eq!(ends, Some((a[0], m)));
}

#[gpui::test]
fn cross_band_edges_skip_folded_bands_and_hidden_nodes(cx: &mut TestAppContext) {
    let mut ui = open(cx, CROSS_SAMPLE);
    cross_fixture(&mut ui);
    ui.select("m");
    assert_eq!(ui.cross(), ["a>m", "c~m", "w~m"]);
    // c lives only in #ui; folding the band leaves it without a card, but w keeps its #notes one.
    ui.click_on("group-#ui");
    assert_eq!(ui.cross(), ["a>m", "w~m"]);
    // Folding #notes too leaves only the lines to cards that are still shown.
    ui.click_on("group-#notes");
    assert_eq!(ui.cross(), Vec::<String>::new());
    ui.click_on("group-#notes");
    ui.click_on("group-#ui");
    assert_eq!(ui.cross(), ["a>m", "c~m", "w~m"]);
    // A done task is hidden with its edges, cross-band ones included.
    ui.app.update(ui.cx, |app, cx| assert!(app.mutate(cx, |g| g.set_status(&id("a"), Status::Done))));
    ui.keys("shift-h");
    assert_eq!(ui.cross(), ["c~m", "w~m"]);
}

#[gpui::test]
fn cross_band_edges_are_only_drawn_while_grouped(cx: &mut TestAppContext) {
    let mut ui = open(cx, CROSS_SAMPLE);
    cross_fixture(&mut ui);
    ui.select("m");
    assert!(!ui.cross().is_empty());
    ui.keys("shift-g");
    assert!(ui.cross().is_empty());
    assert_eq!(ui.read(|app| app.graph_cache.edge_count()), 5);
    ui.keys("shift-g");
    assert_eq!(ui.cross(), ["a>m", "c~m", "w~m"]);
}

#[gpui::test]
fn the_canvas_can_be_emptied_by_hiding_and_still_works(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.app.update(ui.cx, |app, cx| {
        app.mutate(cx, |g| {
            for name in ["a", "b", "c", "m"] {
                g.set_status(&id(name), Status::Done)?;
            }
            Ok(())
        });
    });
    ui.keys("shift-h");
    assert!(ui.shown().is_empty());
    // The empty canvas says why, and offers the way back.
    assert!(ui.cx.debug_bounds("all-hidden").is_some() || ui.cx.debug_bounds("show-completed").is_some());
    ui.keys("down left right cmd-a f shift-g");
    assert_eq!(ui.selected(), None);
    assert!(ui.read(|app| app.graph_cache.group_labels().is_empty()));
    ui.keys("shift-g shift-h");
    assert_eq!(ui.shown(), ["a", "b", "c", "m"]);
}

#[gpui::test]
fn a_workspace_without_tags_groups_into_one_untagged_band(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.keys("shift-g");
    assert_eq!(ui.read(|app| app.graph_cache.group_labels()), ["untagged"]);
    assert_eq!(ui.read(|app| app.graph_cache.group_counts()), [4]);
}

#[gpui::test]
fn view_shortcuts_typed_into_the_notes_editor_are_text(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.select("a");
    ui.keys("e");
    ui.keys("shift-h shift-g");
    assert_eq!(ui.notes(), "HG");
    assert_eq!(ui.read(|app| app.view.clone()), Default::default());
}

#[gpui::test]
fn notes_of_a_node_that_gets_hidden_are_asked_about_not_lost(cx: &mut TestAppContext) {
    for answer in ["enter", "d"] {
        let mut ui = open(cx, SAMPLE);
        ui.keys("shift-h");
        ui.select("b");
        ui.keys("e");
        ui.type_text("keep me");
        // Marked done elsewhere (the CLI, an agent): the card goes away under the editor.
        ui.app.update(ui.cx, |app, cx| {
            app.mutate(cx, |g| g.set_status(&id("b"), Status::Done));
        });
        ui.redraw();
        assert!(ui.read(|app| app.notes_ask.is_some()), "{answer}: the notes are asked about");
        assert_eq!(ui.node("b").body, "");
        ui.keys(answer);
        let saved = ui.node("b").body;
        assert_eq!(saved, if answer == "enter" { "keep me\n" } else { "" }, "{answer}");
        assert_eq!(ui.read(|app| (app.notes.clone(), app.notes_ask.clone())), (None, None));
        assert!(ui.cx.debug_bounds("notes-dialog").is_none());
    }
}

#[gpui::test]
fn keep_editing_a_node_that_got_hidden_keeps_the_notes(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.keys("shift-h");
    ui.select("b");
    ui.keys("e");
    ui.type_text("draft");
    ui.app.update(ui.cx, |app, cx| {
        app.mutate(cx, |g| g.set_status(&id("b"), Status::Done));
    });
    ui.redraw();
    ui.keys("escape");
    ui.redraw();
    // Whatever the editor does now, the draft is still there to save.
    assert_eq!(ui.notes(), "draft");
    assert!(ui.read(|app| app.notes.is_some()));
}

#[gpui::test]
fn quitting_with_unsaved_notes_asks_and_then_quits(cx: &mut TestAppContext) {
    use crate::notes::Then;
    let mut ui = open(cx, SAMPLE);
    ui.select("a");
    ui.keys("e");
    ui.type_text("x");
    assert!(!ui.app.update(ui.cx, |app, cx| app.can_leave_to(Then::Quit, cx)));
    ui.redraw();
    assert_eq!(ui.read(|app| app.notes_ask.clone()), Some(Then::Quit));
    // Keep editing does not quit.
    ui.keys("escape");
    assert!(ui.read(|app| app.notes.is_some() && app.notes_ask.is_none()));
    assert!(!ui.app.update(ui.cx, |app, cx| app.can_leave_to(Then::Quit, cx)));
    ui.redraw();
    // Saving settles the notes and then quits; the test platform's quit only flags the app.
    ui.keys("enter");
    assert_eq!(ui.node("a").body, "x\n");
    assert_eq!(ui.read(|app| app.notes.clone()), None);
}

#[gpui::test]
fn the_toolbar_button_cycles_the_theme_mode(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    // The test platform reports a light appearance, so `System` resolves to the light theme.
    assert_eq!(theme::mode(), theme::ThemeMode::Dark);
    ui.click_on("theme");
    assert_eq!((theme::mode(), theme::current()), (theme::ThemeMode::System, &theme::LIGHT));
    ui.click_on("theme");
    assert_eq!((theme::mode(), theme::current()), (theme::ThemeMode::Light, &theme::LIGHT));
    ui.click_on("theme");
    assert_eq!((theme::mode(), theme::current()), (theme::ThemeMode::Dark, &theme::DARK));
}
