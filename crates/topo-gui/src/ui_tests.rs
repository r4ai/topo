//! Interaction tests: a headless window that takes simulated keys and clicks.

use gpui::{Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px, size};
use tempfile::TempDir;
use topo_core::{Edit, Graph, Kind, Node, NodeId, Status, Workspace};

use crate::gesture::Gesture;
use crate::{NODE_H, NODE_W, Prompt, TopoApp, layout, text_input};

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
    cx.update(text_input::bind_keys);
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
        self.cx.simulate_keystrokes(keystrokes);
    }

    fn type_text(&mut self, text: &str) {
        self.cx.simulate_input(text);
    }

    /// Window position of the point `(fx, fy)` of the card of `node`, as fractions of its size.
    fn card(&mut self, node: &str, fx: f32, fy: f32) -> Point<Pixels> {
        self.read(|app| {
            let cell = layout::layout(app.graph())[&id(node)];
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

    fn selected(&mut self) -> Option<String> {
        self.read(|app| app.selected.as_ref().map(|id| id.to_string()))
    }

    fn node(&mut self, name: &str) -> Node {
        self.read(|app| app.graph().get(&id(name)).expect("node exists").clone())
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
    ui.keys("enter");
    assert!(matches!(ui.read(|app| app.prompt.clone()), Some(Prompt::Rename(_))));
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
    assert_eq!(ui.selected().as_deref(), Some("c"));
    assert_eq!(ui.read(|app| app.selected_nodes.clone()), [id("c")].into_iter().collect());
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
