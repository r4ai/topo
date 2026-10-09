//! Interaction tests: a headless window that takes simulated keys and clicks.

use std::ops::Range;

use gpui::{
    Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, WindowAppearance, point, px, size,
};
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
    open_full(cx, nodes, "", false)
}

/// Like [`open`], with a `[jev]` table written so the organize controls are shown.
fn open_jev<'a>(cx: &'a mut TestAppContext, nodes: &[(&str, Kind, &[&str], &[&str])]) -> Ui<'a> {
    open_full(cx, nodes, "", true)
}

fn open_full<'a>(
    cx: &'a mut TestAppContext,
    nodes: &[(&str, Kind, &[&str], &[&str])],
    name: &str,
    jev: bool,
) -> Ui<'a> {
    let dir = tempfile::tempdir().unwrap();
    let root = match name {
        "" => dir.path().to_owned(),
        name => {
            let root = dir.path().join(name);
            std::fs::create_dir(&root).unwrap();
            root
        }
    };
    let mut ws = Workspace::init(&root).unwrap();
    let nodes = nodes.iter().map(|(name, kind, deps, milestones)| {
        let mut node = Node::new(id(name), *kind, (*name).into());
        node.depends_on = deps.iter().map(|d| id(d)).collect();
        node.milestones = milestones.iter().map(|m| id(m)).collect();
        node
    });
    ws.graph = Graph::from_nodes(nodes).unwrap();
    ws.save().unwrap();
    if jev {
        std::fs::write(ws.dir().join("config.toml"), "[jev]\nbase_url = \"http://127.0.0.1:8000\"\n").unwrap();
    }
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

#[gpui::test]
fn virtual_inspector_rows_fill_the_panel_and_keep_its_inset(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let panel = ui.cx.debug_bounds("inspector").unwrap_or_else(|| {
        let area = ui.read(|app| app.area.get());
        gpui::Bounds::new(
            point(area.right(), area.top()),
            size(px(ui.read(|app| app.inspector_width())), area.size.height),
        )
    });
    let milestone = ui.cx.debug_bounds("overview-m").expect("overview milestone");
    assert!((milestone.left() - panel.left() - px(16.)).abs() < px(1.5), "milestone={milestone:?}, panel={panel:?}");
    assert!((panel.right() - milestone.right() - px(16.)).abs() < px(1.5), "milestone={milestone:?}, panel={panel:?}");
    ui.app.update(ui.cx, |app, cx| app.select_nodes(&[id("a"), id("b")], cx));
    ui.redraw();
    let row = ui.cx.debug_bounds("row-selected-a").expect("selected row");
    assert!((row.left() - panel.left() - px(16.)).abs() < px(1.5), "row={row:?}, panel={panel:?}");
    assert!((panel.right() - row.right() - px(16.)).abs() < px(1.5), "row={row:?}, panel={panel:?}");
}

#[gpui::test]
fn large_overview_and_selection_lists_keep_all_rows_reachable(cx: &mut TestAppContext) {
    let names: Vec<_> = (0..200).map(|i| format!("task{i:04}")).collect();
    let nodes: Vec<_> = names.iter().map(|name| (name.as_str(), Kind::Task, &[][..], &[][..])).collect();
    let mut ui = open(cx, &nodes);
    assert!(ui.cx.debug_bounds("row-ready-task0000").is_some());
    assert!(ui.cx.debug_bounds("row-ready-task0199").is_none());
    // Scrolling reaches the final row; it retains the same click and status actions.
    ui.app.update(ui.cx, |app, cx| {
        app.graph_cache.overview.list.scroll_to_end();
        cx.notify();
    });
    ui.redraw();
    let last = ui.cx.debug_bounds("row-ready-task0199").expect("last ready task is visible");
    assert!(ui.cx.debug_bounds("row-ready-task0000").is_none());
    ui.click(last.center());
    assert_eq!(ui.selected().as_deref(), Some("task0199"));
    ui.keys("x");
    ui.keys("escape");
    ui.redraw();
    assert!(ui.cx.debug_bounds("row-ready-task0199").is_none(), "closed task leaves ready list");

    // Metadata edits reorder the overview even though they retain canvas layout.
    ui.app.update(ui.cx, |app, cx| {
        app.mutate(cx, |graph| {
            graph.edit(&id("task0198"), Edit { priority: Some(Some(Priority::High)), ..Default::default() })
        });
        app.graph_cache.overview.list.scroll_to(gpui::ListOffset { item_ix: 0, offset_in_item: px(0.) });
        cx.notify();
    });
    ui.redraw();
    let high = ui.cx.debug_bounds("row-ready-task0198").expect("high priority task moves to top");
    let first = ui.cx.debug_bounds("row-ready-task0000").unwrap();
    assert!(high.top() < first.top());

    ui.app.update(ui.cx, |app, cx| {
        let ids: Vec<_> = app.graph().nodes().map(|n| n.id.clone()).collect();
        app.select_nodes(&ids, cx);
        app.anim = None;
    });
    ui.redraw();
    assert!(ui.cx.debug_bounds("row-selected-task0199").is_none());
    ui.app.update(ui.cx, |app, cx| {
        app.graph_cache.selection_scroll.scroll_to_item(199, gpui::ScrollStrategy::Top);
        cx.notify();
    });
    ui.redraw();
    let last = ui.cx.debug_bounds("row-selected-task0199").expect("last selected task is visible");
    assert!(ui.cx.debug_bounds("row-selected-task0000").is_none());
    ui.click(last.center());
    assert_eq!(ui.selection(), ["task0199"]);
}

/// Turns motion on until it is dropped; tests otherwise run without it.
struct Motion;

impl Motion {
    fn on() -> Self {
        crate::animation::set_enabled(true);
        Self
    }
}

impl Drop for Motion {
    fn drop(&mut self) {
        crate::animation::set_enabled(false);
    }
}

/// The value and velocity of the camera's x, y and zoom now.
fn camera(app: &TopoApp) -> [(f32, f32); 3] {
    let now = app.clock.now();
    app.springs(now).map(|spring| spring.sample(now))
}

/// Puts the camera at rest at `offset` and `zoom`.
fn place(ui: &mut Ui, offset: Point<Pixels>, zoom: f32) {
    ui.app.update(ui.cx, |app, _| (app.anim, app.offset, app.zoom) = (None, offset, zoom));
    ui.redraw();
}

fn gesture(ui: &mut Ui, gesture: Gesture) {
    ui.app.update_in(ui.cx, |app, window, cx| app.on_gesture(gesture, window, cx));
    ui.redraw();
}

#[gpui::test]
fn camera_retargeting_keeps_velocity_and_disabling_motion_finishes_the_move(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let _motion = Motion::on();
    place(&mut ui, point(px(0.), px(0.)), 1.);
    ui.app.update(ui.cx, |app, _| app.animate_to(point(px(1000.), px(400.)), 1.5));
    ui.advance(80);
    let before = ui.read(camera);
    let [(x, vx), (_, vy), (zoom, vz)] = before;
    assert!(0. < x && x < 1000. && 1. < zoom && zoom < 1.5);
    assert!(vx > 0. && vy > 0. && vz > 0.);
    assert_eq!(ui.read(|app| (f32::from(app.offset.x), app.zoom)), (x, zoom), "the frame draws the springs");

    // Retargeted between frames, the camera is where it was and as fast.
    ui.app.update(ui.cx, |app, _| app.animate_to(point(px(-100.), px(50.)), 0.75));
    for ((value, velocity), (old_value, old_velocity)) in ui.read(camera).into_iter().zip(before) {
        assert!((value - old_value).abs() <= old_value.abs() * 1e-6, "{value} != {old_value}");
        assert!((velocity - old_velocity).abs() <= old_velocity.abs() * 1e-4, "{velocity} != {old_velocity}");
    }
    // It still coasts the old way before it turns round.
    ui.advance(8);
    assert!(ui.read(|app| f32::from(app.offset.x)) > x);
    assert_eq!(ui.read(|app| app.target()), (point(px(-100.), px(50.)), 0.75));

    crate::animation::set_enabled(false);
    ui.redraw();
    assert!(ui.read(|app| app.anim.is_none()));
    assert_eq!(ui.read(|app| (app.offset, app.zoom)), (point(px(-100.), px(50.)), 0.75));
}

#[gpui::test]
fn a_long_camera_move_settles_without_a_final_snap(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let _motion = Motion::on();
    place(&mut ui, point(px(0.), px(0.)), 1.);
    let to = (point(px(100_000.), px(-100_000.)), 0.5);
    ui.app.update(ui.cx, |app, _| app.animate_to(to.0, to.1));
    let mut last = ui.read(|app| (app.offset, app.zoom));
    let mut frames = 0;
    while ui.read(|app| app.anim.is_some()) {
        last = ui.read(|app| (app.offset, app.zoom));
        ui.advance(8);
        frames += 1;
    }
    assert!(frames > 45, "a move this long takes its time: {frames} frames");
    assert_eq!(ui.read(|app| (app.offset, app.zoom)), to);
    // The frame before the camera rested was already there to the eye.
    assert!((last.0.x - to.0.x).abs() < px(0.5) && (last.0.y - to.0.y).abs() < px(0.5), "{last:?}");
    assert!((last.1 - to.1).abs() < 1e-5, "{last:?}");
}

#[gpui::test]
fn without_motion_every_camera_move_lands_at_once(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let moved = |ui: &mut Ui, f: fn(&mut TopoApp, &mut gpui::Context<TopoApp>)| {
        ui.app.update(ui.cx, |app, cx| {
            f(app, cx);
            assert!(app.anim.is_none());
            assert_eq!(app.target(), (app.offset, app.zoom));
        });
        ui.read(|app| (app.offset, app.zoom))
    };
    place(&mut ui, point(px(5000.), px(5000.)), 2.);
    let framed = moved(&mut ui, |app, _| app.fit(false));
    assert!(framed.0.x < px(5000.) && framed.1 < 2.);
    assert_eq!(moved(&mut ui, |app, _| app.zoom_by(1.25, app.canvas_center(), true)).1, framed.1 * 1.25);
    place(&mut ui, point(px(5000.), px(5000.)), 2.);
    assert_ne!(moved(&mut ui, |app, _| app.reveal(&id("a"), true)).0, point(px(5000.), px(5000.)));
    assert_ne!(moved(&mut ui, |app, cx| app.select_nodes(&[id("a"), id("b")], cx)).1, 2.);
    place(&mut ui, point(px(5000.), px(5000.)), 2.);
    assert_ne!(moved(&mut ui, |app, _| app.toggle_hide_completed()).0, point(px(5000.), px(5000.)));

    // A fast drag ends where it is released, and a pinch past the limit returns at once.
    let before = ui.read(|app| app.offset);
    let from = ui.read(|app| app.area.get().origin) + point(px(30.), px(300.));
    ui.fast_drag(from, point(px(20.), px(0.)), 10);
    ui.release(from + point(px(200.), px(0.)));
    assert_eq!(ui.read(|app| (app.anim.is_none(), app.offset)), (true, before + point(px(200.), px(0.))));
    ui.move_to(from, false);
    gesture(&mut ui, Gesture::Pinch(3.));
    assert!(ui.read(|app| app.zoom) > crate::MAX_ZOOM);
    gesture(&mut ui, Gesture::PinchEnd);
    assert_eq!(ui.read(|app| (app.anim.is_none(), app.zoom)), (true, crate::MAX_ZOOM));
}

#[gpui::test]
fn a_pinch_takes_over_a_camera_flight_without_a_jump(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let _motion = Motion::on();
    place(&mut ui, point(px(900.), px(-300.)), 2.);
    let pointer = ui.read(|app| app.area.get().origin) + point(px(300.), px(200.));
    ui.move_to(pointer, false);
    ui.app.update(ui.cx, |app, _| app.fit(false));
    ui.advance(80);
    let (offset, zoom, target) = ui.read(|app| (app.offset, app.zoom, app.target()));
    assert!(ui.read(|app| app.anim.is_some()) && (zoom - target.1).abs() > 0.1);

    gesture(&mut ui, Gesture::Pinch(0.01));
    let (pinched_offset, pinched_zoom) = ui.read(|app| (app.offset, app.zoom));
    assert!(ui.read(|app| app.anim.is_none()), "the pinch holds the camera");
    assert!((pinched_zoom - zoom * 1.01).abs() < 1e-4, "{pinched_zoom} continues from {zoom}");
    // The offset moves only as far as zooming by a hundredth around the pointer takes it.
    let shift = (point(px(300.), px(200.)) - offset) * -0.01;
    assert!((pinched_offset.x - offset.x - shift.x).abs() < px(0.01), "{pinched_offset:?} continues from {offset:?}");
    assert!((pinched_offset.y - offset.y - shift.y).abs() < px(0.01), "{pinched_offset:?} continues from {offset:?}");
    // Nothing carries the camera on once the pinch holds it.
    ui.advance(500);
    assert_eq!(ui.read(|app| (app.offset, app.zoom)), (pinched_offset, pinched_zoom));
}

#[gpui::test]
fn a_pinch_past_a_zoom_limit_is_resisted_and_springs_back_to_it(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let _motion = Motion::on();
    let pointer = ui.card("a", 0., 0.);
    ui.move_to(pointer, false);
    for (limit, magnification) in [(crate::MAX_ZOOM, 0.5), (crate::MIN_ZOOM, -0.4)] {
        let offset = ui.read(|app| app.offset);
        place(&mut ui, offset, 1.);
        let mut asked = 1.;
        for _ in 0..6 {
            gesture(&mut ui, Gesture::Pinch(magnification));
            asked *= 1. + magnification;
        }
        let zoom = ui.read(|app| app.zoom);
        let past = |zoom: f32| (zoom / limit).ln().abs();
        assert!((asked / limit).ln() * (zoom / limit).ln() > 0., "{zoom} is past {limit}");
        assert!(past(zoom) < past(asked) / 2., "{zoom} gives much less than the {asked} asked for");
        let held = ui.card("a", 0., 0.);

        gesture(&mut ui, Gesture::PinchEnd);
        assert!(ui.read(|app| app.anim.is_some()));
        ui.advance(100);
        assert!(past(ui.read(|app| app.zoom)) < past(zoom), "the zoom is on its way back");
        // The point under the fingers stays under them on the way.
        let corner = ui.card("a", 0., 0.);
        assert!((corner.x - held.x).abs() < px(0.5) && (corner.y - held.y).abs() < px(0.5), "{corner:?} left {held:?}");
        ui.advance(3000);
        assert_eq!(ui.read(|app| (app.anim.is_none(), app.zoom)), (true, limit));
        let corner = ui.card("a", 0., 0.);
        assert!((corner.x - held.x).abs() < px(0.01) && (corner.y - held.y).abs() < px(0.01));
    }
}

#[gpui::test]
fn wheel_notches_add_up_and_stay_anchored_all_the_way(cx: &mut TestAppContext) {
    use gpui::{ScrollDelta, ScrollWheelEvent};
    let mut ui = open(cx, SAMPLE);
    let _motion = Motion::on();
    // Start in a flight, so the zoom takes over a camera that is already moving.
    place(&mut ui, point(px(700.), px(500.)), 0.6);
    ui.app.update(ui.cx, |app, _| app.fit(false));
    ui.advance(60);
    let position = ui.card("b", 0.5, 0.5);
    let zoom = ui.read(|app| app.target().1);
    let notch = ScrollWheelEvent { position, delta: ScrollDelta::Lines(point(0., 1.)), ..Default::default() };
    let anchored = |ui: &mut Ui, within: f32| {
        let card = ui.card("b", 0.5, 0.5);
        assert!((card.x - position.x).abs() < px(within) && (card.y - position.y).abs() < px(within), "{card:?}");
    };
    ui.cx.simulate_event(notch.clone());
    for _ in 0..3 {
        ui.advance(16);
        assert!(ui.read(|app| app.anim.is_some()));
        anchored(&mut ui, 0.5);
    }
    // A second notch before the first has landed zooms on from where that one was heading.
    ui.cx.simulate_event(notch);
    assert!((ui.read(|app| app.target().1) - zoom * 1.12 * 1.12).abs() < 1e-4);
    for _ in 0..3 {
        ui.advance(16);
        anchored(&mut ui, 0.5);
    }
    ui.advance(3000);
    assert!(ui.read(|app| app.anim.is_none()));
    assert!((ui.read(|app| app.zoom) - zoom * 1.12 * 1.12).abs() < 1e-4);
    anchored(&mut ui, 0.01);
}

#[gpui::test]
fn a_thrown_canvas_glides_to_its_projection_and_a_press_stops_it(cx: &mut TestAppContext) {
    use crate::animation::{preset::GLIDE, projection};
    let mut ui = open(cx, SAMPLE);
    let _motion = Motion::on();
    let from = ui.read(|app| app.area.get().origin) + point(px(30.), px(300.));
    let glide = |speed| px(projection(speed, GLIDE));

    // 10 px every 10 ms: the smoothed speed approaches 1000 px/s from below.
    let end = ui.fast_drag(from, point(px(10.), px(0.)), 30);
    let released = ui.read(|app| app.offset);
    ui.release(end);
    let landing = ui.read(|app| app.target().0);
    assert!(ui.read(|app| app.anim.is_some()));
    assert!(released.x + glide(950.) < landing.x && landing.x <= released.x + glide(1000.), "{landing:?}");
    assert_eq!(landing.y, released.y);
    // It leaves the hand at the speed it was dragged.
    ui.advance(10);
    let coasted = ui.read(|app| app.offset.x) - released.x;
    assert!(px(8.5) < coasted && coasted < px(10.), "{coasted:?}");
    ui.advance(5000);
    assert_eq!(ui.read(|app| (app.anim.is_none(), app.offset)), (true, landing));

    // A press catches the next throw where it is.
    let end = ui.fast_drag(from, point(px(10.), px(0.)), 30);
    ui.release(end);
    ui.advance(100);
    let caught = ui.read(|app| app.offset);
    assert!(caught.x < ui.read(|app| app.target().0.x) - px(50.));
    ui.press(from);
    assert!(ui.read(|app| app.anim.is_none()));
    ui.release(from);
    ui.advance(1000);
    assert_eq!(ui.read(|app| app.offset), caught);

    // A pointer that rests before the release lets go without a throw, and so does a slow one.
    let end = ui.fast_drag(from, point(px(10.), px(0.)), 30);
    ui.advance(100);
    ui.release(end);
    assert!(ui.read(|app| app.anim.is_none()));
    let end = ui.fast_drag(from, point(px(0.5), px(0.)), 30);
    ui.release(end);
    assert!(ui.read(|app| app.anim.is_none()));
}

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

    /// Lets `milliseconds` pass and draws the frame after them.
    fn advance(&mut self, milliseconds: u64) {
        self.cx.executor().advance_clock(std::time::Duration::from_millis(milliseconds));
        self.redraw();
    }

    /// Presses at `from` and drags by `step` every 10 ms, `steps` times; returns where the pointer is held.
    fn fast_drag(&mut self, from: Point<Pixels>, step: Point<Pixels>, steps: usize) -> Point<Pixels> {
        self.press(from);
        for moved in 1..=steps {
            self.cx.executor().advance_clock(std::time::Duration::from_millis(10));
            self.move_to(from + step * moved as f32, true);
        }
        from + step * steps as f32
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
    assert_eq!(ui.read(|app| app.toast.get().map(|t| t.text.clone())).as_deref(), Some("Would create a cycle"));
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
    assert!(ui.read(|app| app.prompt.get().is_none()));
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
    assert!(matches!(ui.read(|app| app.prompt.get().cloned()), Some(Prompt::Search)));
    let empty = ui.read(|app| app.area.get().bottom_left()) + point(px(200.), px(-120.));
    ui.click(empty);
    assert!(ui.read(|app| app.prompt.get().is_none()));
    assert_eq!(ui.selected().as_deref(), Some("a"));
    assert_eq!(ui.node("a").title, "a");
}

#[gpui::test]
fn the_toolbar_fits_the_smallest_window(cx: &mut TestAppContext) {
    let mut ui = open_jev(cx, SAMPLE);
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
fn the_organize_controls_show_only_with_a_jev_config(cx: &mut TestAppContext) {
    // A workspace without a `[jev]` table hides the Jev-backed controls.
    {
        let mut ui = open(cx, SAMPLE);
        ui.redraw();
        assert!(ui.cx.debug_bounds("toolbar-organize").is_none());
        assert!(ui.cx.debug_bounds("org-deps").is_none());
        assert!(ui.cx.debug_bounds("org-place").is_none());
    }

    // One that opts into a model shows them.
    let mut ui = open_jev(cx, SAMPLE);
    ui.redraw();
    assert!(ui.cx.debug_bounds("toolbar-organize").is_some());
    assert!(ui.cx.debug_bounds("org-deps").is_some());
    assert!(ui.cx.debug_bounds("org-place").is_some());
}

#[gpui::test]
fn search_emphasizes_every_match_and_jumps(cx: &mut TestAppContext) {
    let tasks: Vec<String> = (0..12).map(|i| format!("task{i:02}")).collect();
    let nodes: Vec<(&str, Kind, &[&str], &[&str])> =
        tasks.iter().map(|t| (t.as_str(), Kind::Task, &[][..], &[][..])).collect();
    let mut ui = open(cx, &nodes);
    ui.keys("/");
    ui.type_text("task");
    assert_eq!(ui.app.read_with(ui.cx, |app, cx| app.palette.read(cx).choices().len()), 12);
    assert!(ui.app.read_with(ui.cx, |app, cx| app.palette.read(cx).contains_key("task11")));
    // The list scrolls past the rows it shows at once.
    ui.keys("down down down down down down down down down down enter");
    assert_eq!(ui.selected().as_deref(), Some("task10"));

    // Only the rows in view exist, and the highlight stays among them.
    ui.keys("/");
    ui.type_text("task");
    assert!(ui.cx.debug_bounds("choice-11").is_none());
    ui.keys("down down down down down down down down down down down");
    ui.redraw();
    assert!(ui.cx.debug_bounds("choice-11").is_some(), "the list scrolled to the highlight");
    assert!(ui.cx.debug_bounds("choice-0").is_none());
}

#[gpui::test]
fn a_field_list_scrolls_through_every_choice(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let tags: Vec<String> = (0..12).map(|i| format!("tag{i:02}")).collect();
    ui.app.update(ui.cx, |app, cx| {
        let edit = Edit { tags: Some(tags), ..Edit::default() };
        app.mutate(cx, |graph| graph.edit(&id("c"), edit));
    });
    ui.select("a");
    ui.keys("t");
    ui.redraw();
    let count = |ui: &mut Ui| ui.app.read_with(ui.cx, |app, cx| app.combo.read(cx).choices().len());
    assert_eq!(count(&mut ui), 12, "nothing is cut off");
    let list = ui.cx.debug_bounds("combo-list").expect("the list is open");
    assert!(ui.cx.debug_bounds("choice-7").is_some() && ui.cx.debug_bounds("choice-8").is_none());
    // Down past the last visible entry scrolls it into view.
    ui.keys("down down down down down down down down down down down down");
    ui.redraw();
    let last = ui.cx.debug_bounds("choice-11").expect("the highlighted choice is rendered");
    assert!(last.top() >= list.top() && last.bottom() <= list.bottom(), "and within the list");
    assert!(ui.cx.debug_bounds("choice-0").is_none());
    // Typing starts over at the top.
    ui.type_text("tag0");
    ui.redraw();
    assert!(ui.cx.debug_bounds("choice-0").is_some());
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
    gesture(&mut ui, Gesture::Pinch(0.5));
    assert!((ui.read(|app| app.zoom) - 1.5).abs() < 1e-5);
    // The corner of the card under the pointer stays under it.
    let corner = ui.card("a", 0.0, 0.0);
    assert!((corner.x - pointer.x).abs() < px(0.01) && (corner.y - pointer.y).abs() < px(0.01));
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
    assert!(ui.read(|app| app.prompt.get().is_none()));
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
    assert!(ui.read(|app| app.prompt.get().is_none()), "the value is edited in its row, not in the prompt");
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
    assert!(ui.read(|app| app.toast.get().is_some_and(|t| t.error)));
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
    assert!(ui.read(|app| app.prompt.get().is_none()));
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
    assert!(matches!(ui.read(|app| app.prompt.get().cloned()), Some(Prompt::Create(_))));
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
        for title in [
            "Alpha",
            "日本語タイトルの折り返しを確認するための少し長い名前",
            "非常に長いタイトルでも編集中に右パネルの行数と幅が変わらないことを確認するための、折り返しが四行以上になる十分な長さを持たせたタイトルです。",
        ] {
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
    assert!(ui.read(|app| app.prompt.get().is_none()));
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
    assert!(ui.read(|app| app.notes_ask.get().is_some()));
    ui.keys("d");
    assert_eq!(ui.read(|app| (app.notes.clone(), app.notes_ask.get().map(|ask| ask.then.clone()))), (None, None));
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
    assert_eq!(ui.read(|app| (app.notes.clone(), app.notes_ask.get().map(|ask| ask.then.clone()))), (None, None));

    // A change shows, and the question over it has three answers.
    ui.keys("e");
    ui.type_text("draft");
    assert!(ui.cx.debug_bounds("notes-unsaved").is_some() && ui.cx.debug_bounds("notes-saved").is_none());
    ui.keys("escape");
    assert!(ui.cx.debug_bounds("notes-dialog").is_some());
    assert_eq!(ui.node("a").body, "");
    // Keep editing: the text and the cursor are as they were, and the keys are text again.
    ui.click_on("notes-keep");
    assert_eq!(ui.read(|app| app.notes_ask.get().map(|ask| ask.then.clone())), None);
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
    assert!(ui.read(|app| app.notes_ask.get().is_some()));
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
    assert_eq!(ui.read(|app| app.notes_ask.get().map(|ask| ask.then.clone())), Some(Then::CloseWindow));
    assert!(ui.cx.debug_bounds("notes-dialog").is_some());
    // The question, not the text, has the keyboard.
    ui.type_text("y");
    assert_eq!(ui.notes(), "x");
    ui.keys("escape");
    assert_eq!(ui.read(|app| app.notes_ask.get().map(|ask| ask.then.clone())), None);
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
    assert!(ui.read(|app| app.toast.get().is_none_or(|t| !t.error)));
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
        assert!(ui.read(|app| app.notes_ask.get().is_some()), "{answer}: the notes are asked about");
        assert_eq!(ui.node("b").body, "");
        ui.keys(answer);
        let saved = ui.node("b").body;
        assert_eq!(saved, if answer == "enter" { "keep me\n" } else { "" }, "{answer}");
        assert_eq!(ui.read(|app| (app.notes.clone(), app.notes_ask.get().map(|ask| ask.then.clone()))), (None, None));
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
    assert_eq!(ui.read(|app| app.notes_ask.get().map(|ask| ask.then.clone())), Some(Then::Quit));
    // Keep editing does not quit.
    ui.keys("escape");
    assert!(ui.read(|app| app.notes.is_some() && app.notes_ask.get().is_none()));
    assert!(!ui.app.update(ui.cx, |app, cx| app.can_leave_to(Then::Quit, cx)));
    ui.redraw();
    // Saving settles the notes and then quits; the test platform's quit only flags the app.
    ui.keys("enter");
    assert_eq!(ui.node("a").body, "x\n");
    assert_eq!(ui.read(|app| app.notes.clone()), None);
}

#[gpui::test]
fn the_toolbar_button_opens_a_menu_that_sets_the_mode_and_monotone(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    // The test platform reports a light appearance, so `System` resolves to the light theme.
    assert_eq!((theme::mode(), theme::monotone()), (theme::ThemeMode::Dark, false));
    assert!(ui.cx.debug_bounds("theme-menu").is_none());
    ui.click_on("theme");
    assert!(ui.cx.debug_bounds("theme-menu").is_some());
    // The Monotone choice is a switch, not a check mark.
    assert!(ui.cx.debug_bounds("theme-monotone-switch").is_some());
    ui.click_on("theme-light");
    assert_eq!((theme::mode(), theme::current()), (theme::ThemeMode::Light, &theme::LIGHT));
    assert!(ui.cx.debug_bounds("theme-menu").is_some(), "choosing a mode keeps the menu open");
    ui.click_on("theme-system");
    assert_eq!((theme::mode(), theme::current()), (theme::ThemeMode::System, &theme::LIGHT));
    ui.click_on("theme-monotone");
    assert!(theme::monotone());
    assert_eq!(theme::current(), &theme::LIGHT_MONO);
    ui.click_on("theme-dark");
    assert_eq!(theme::current(), &theme::DARK_MONO);
    ui.click_on("theme-monotone");
    assert!(!theme::monotone());
    assert_eq!(theme::current(), &theme::DARK);
    ui.keys("escape");
    ui.redraw();
    assert!(ui.cx.debug_bounds("theme-menu").is_none());
    // The button toggles it, and a click elsewhere closes it without acting on what is under it.
    ui.click_on("theme");
    ui.click_on("theme");
    assert!(ui.cx.debug_bounds("theme-menu").is_none());
    ui.click_on("theme");
    ui.click_on("help");
    assert!(ui.cx.debug_bounds("theme-menu").is_none());
    assert!(!ui.read(|app| app.show_help));
}

#[gpui::test]
fn a_long_error_toast_wraps_inside_the_window(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let message = "request to http://127.0.0.1:8000/v1/systemone failed (is a Jev-compatible server running? see `jev.base_url` in .topo/config.toml)";
    ui.app.update(ui.cx, |app, cx| app.toast(message, true, cx));
    ui.redraw();
    let frame = ui.cx.debug_bounds("toast").expect("the toast is drawn");
    let text = ui.cx.debug_bounds("toast-text").expect("the toast text is drawn");
    assert!(frame.right() <= px(1360.), "the toast stays in the window");
    assert!(text.right() <= frame.right(), "the text wraps inside the toast");
    assert!(frame.size.height > px(20.), "the long text takes more than one row");
}

/// Drives the app through its main states and returns each state's name with a selector that must be drawn.
fn drive_states(ui: &mut Ui) -> Vec<(&'static str, &'static str)> {
    let mut seen = Vec::new();
    let mut check = |ui: &mut Ui, state: &'static str, selector: &'static str| {
        ui.redraw();
        assert!(ui.cx.debug_bounds(selector).is_some(), "{state}: {selector} is drawn");
        seen.push((state, selector));
    };
    check(ui, "overview", "brand");
    ui.select("a");
    check(ui, "task", "title-text");
    ui.select("m");
    check(ui, "milestone", "title-text");
    ui.keys("shift-g");
    check(ui, "grouped", "group-untagged");
    ui.keys("shift-g");
    ui.keys("?");
    check(ui, "help", "help-card");
    ui.keys("escape");
    ui.keys("/");
    check(ui, "search", "prompt-card");
    ui.keys("escape");
    ui.select("a");
    ui.keys("t");
    check(ui, "inline edit", "inline-edit");
    ui.keys("escape");
    ui.keys("e");
    check(ui, "notes", "notes-editor");
    ui.type_text("draft");
    ui.keys("escape");
    check(ui, "unsaved dialog", "notes-dialog");
    ui.keys("d");
    ui.app.update(ui.cx, |app, cx| app.toast("Saved", false, cx));
    check(ui, "toast", "toast");
    seen
}

#[gpui::test]
fn every_main_state_draws_in_every_palette(cx: &mut TestAppContext) {
    let palettes = [
        (theme::ThemeMode::Dark, false, &theme::DARK),
        (theme::ThemeMode::Light, false, &theme::LIGHT),
        (theme::ThemeMode::Dark, true, &theme::DARK_MONO),
        (theme::ThemeMode::Light, true, &theme::LIGHT_MONO),
    ];
    for (mode, monotone, expected) in palettes {
        theme::apply(mode, monotone, WindowAppearance::Light);
        let mut ui = open(cx, SAMPLE);
        theme::apply(mode, monotone, WindowAppearance::Light);
        ui.redraw();
        assert!(std::ptr::eq(theme::current(), expected));
        assert_eq!(drive_states(&mut ui).len(), 10);
    }
    theme::apply(theme::ThemeMode::Dark, false, WindowAppearance::Light);
}

#[gpui::test]
fn switching_the_theme_is_purely_visual(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    theme::apply(theme::ThemeMode::Dark, false, WindowAppearance::Light);
    ui.select("b");
    ui.keys("t");
    let selectors = ["brand", "title-text", "inline-edit", "node-a", "node-b", "node-m", "zoom"];
    let state = |ui: &mut Ui| {
        let bounds: Vec<_> = selectors.iter().map(|s| ui.cx.debug_bounds(s)).collect();
        (ui.selection(), ui.inline(), bounds)
    };
    ui.redraw();
    let before = state(&mut ui);
    assert!(before.2.iter().all(Option::is_some));
    theme::apply(theme::ThemeMode::Light, false, WindowAppearance::Light);
    ui.redraw();
    assert!(std::ptr::eq(theme::current(), &theme::LIGHT));
    assert_eq!(state(&mut ui), before);
    theme::apply(theme::ThemeMode::Dark, false, WindowAppearance::Light);
}

#[gpui::test]
fn nothing_in_the_toolbar_overlaps_at_any_width(cx: &mut TestAppContext) {
    // Task and milestone statuses give the toolbar every stat there is.
    let mut doing = Node::new(id("d"), Kind::Task, "d".into());
    doing.status = Status::Doing;
    let mut overdue = Node::new(id("o"), Kind::Task, "o".into());
    overdue.due = Some("2000-01-01".parse().unwrap());
    let mut ui = open_full(cx, SAMPLE, "a-repository-with-a-really-very-long-name-indeed", true);
    ui.app.update(ui.cx, |app, cx| {
        app.mutate(cx, |graph| {
            graph.insert(doing)?;
            graph.insert(overdue)
        });
    });
    // Parts that always show, and the parts of a group that clips whole items it has no room for.
    let fixed = ["brand", "repository", "search", "new-task", "new-milestone", "undo", "redo", "theme", "help"];
    let clipped = [
        ("toolbar-stats", ["stat-ready", "stat-doing", "stat-overdue"].as_slice()),
        ("toolbar-organize", ["org-deps", "org-place"].as_slice()),
    ];
    for inset in [0., crate::chrome::TEST_TRAFFIC_LIGHTS] {
        crate::chrome::test_inset::set(inset);
        for width in [720., 900., 1100., 1179., 1180., 1360., 1900.] {
            ui.resize(width, 500.);
            let toolbar = ui.cx.debug_bounds("toolbar").expect("the toolbar is rendered");
            let bounds = |ui: &mut Ui, name: &'static str| {
                ui.cx.debug_bounds(name).unwrap_or_else(|| panic!("{name} is rendered at {width} (inset {inset})"))
            };
            let mut boxes: Vec<_> = fixed.iter().map(|n| (n.to_string(), bounds(&mut ui, n))).collect();
            for (group, items) in clipped {
                let group_box = bounds(&mut ui, group);
                boxes.push((group.to_string(), group_box));
                for item in items {
                    let b = bounds(&mut ui, item);
                    let shown = b.left() >= group_box.left()
                        && b.right() <= group_box.right()
                        && b.top() >= group_box.top()
                        && b.bottom() <= group_box.bottom();
                    // An item is whole inside its group or wrapped out of sight; only the first one, which a
                    // wrapping row cannot move, may be clipped by the group's edge.
                    assert!(
                        shown || b.top() >= group_box.bottom() || b.left() == group_box.left(),
                        "{item} is cut by {group} at {width} (inset {inset}): {b:?} {group_box:?}"
                    );
                }
            }
            for (name, b) in &boxes {
                assert!(
                    b.left() >= toolbar.left() + px(inset) && b.right() <= toolbar.right(),
                    "{name} leaves the toolbar at {width} (inset {inset}): {b:?} in {toolbar:?}"
                );
            }
            for (i, (a, ab)) in boxes.iter().enumerate() {
                for (b, bb) in &boxes[i + 1..] {
                    assert!(!ab.intersects(bb), "{a} overlaps {b} at {width} (inset {inset}): {ab:?} {bb:?}");
                }
            }
        }
    }
    crate::chrome::test_inset::set(0.);
}

/// The top edge of the element with debug selector `selector`, which a surface's travel moves.
fn top_of(ui: &mut Ui, selector: &'static str) -> Pixels {
    ui.cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn")).top()
}

#[gpui::test]
fn a_closed_prompt_fades_out_inert_and_a_reopening_continues_from_where_it_was(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let _motion = Motion::on();
    ui.keys("/");
    ui.advance(3000);
    let rest = top_of(&mut ui, "prompt-card");
    ui.keys("escape");
    ui.advance(40);
    assert!(ui.read(|app| app.prompt.get().is_none()), "the prompt is closed as far as the app is concerned");
    let leaving = top_of(&mut ui, "prompt-card");
    assert!(leaving > rest, "but its card is still painted, sinking: {leaving:?} below {rest:?}");

    // A click where the card is reaches the canvas under it, which starts a pan.
    let at = ui.cx.debug_bounds("prompt-card").unwrap().center();
    ui.press(at);
    assert!(ui.read(|app| app.drag.is_some()), "a leaving prompt takes no clicks");
    ui.release(at);

    // Reopening in mid-exit carries on from the card's place instead of starting over.
    let leaving = top_of(&mut ui, "prompt-card");
    ui.keys("/");
    assert!(ui.read(|app| app.prompt.get().is_some()));
    let reopened = top_of(&mut ui, "prompt-card");
    assert!((reopened - leaving).abs() < px(0.01), "{reopened:?} continues from {leaving:?}");
    assert!(reopened > rest && reopened < rest + px(7.), "{reopened:?} is neither at rest nor back at the start");
    ui.advance(3000);
    assert_eq!(top_of(&mut ui, "prompt-card"), rest);

    ui.keys("escape");
    ui.advance(3000);
    assert!(ui.cx.debug_bounds("prompt-card").is_none(), "the card is gone once it has faded");
}

#[gpui::test]
fn a_toast_replaced_while_visible_keeps_its_place_and_leaves_when_it_expires(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let _motion = Motion::on();
    ui.app.update(ui.cx, |app, cx| app.toast("one", false, cx));
    ui.advance(30);
    let rising = top_of(&mut ui, "toast");
    ui.app.update(ui.cx, |app, cx| app.toast("two", false, cx));
    ui.redraw();
    assert_eq!(ui.read(|app| app.toast.get().map(|toast| toast.text.clone())).as_deref(), Some("two"));
    assert_eq!(top_of(&mut ui, "toast"), rising, "the new text does not replay the entrance");
    ui.advance(1500);
    let rest = top_of(&mut ui, "toast");
    assert!(rest < rising);
    ui.app.update(ui.cx, |app, cx| app.toast("three", false, cx));
    ui.redraw();
    assert_eq!(top_of(&mut ui, "toast"), rest, "nor does a replacement of a toast at rest");

    // The timer of the latest toast hides it; it sinks and is removed after the exit.
    ui.advance(2400);
    assert!(ui.read(|app| app.toast.get().is_some()));
    ui.advance(200);
    assert!(ui.read(|app| app.toast.get().is_none()), "the expiry hides the toast");
    ui.advance(40);
    let sinking = top_of(&mut ui, "toast");
    assert!(sinking > rest, "and it is still painted, on its way out");

    // A toast that arrives during the exit continues from where the old one was.
    ui.app.update(ui.cx, |app, cx| app.toast("four", false, cx));
    ui.redraw();
    assert_eq!(top_of(&mut ui, "toast"), sinking);
    ui.advance(100);
    assert!(top_of(&mut ui, "toast") < sinking, "and rises again");

    ui.advance(3000 + 2600);
    assert!(ui.cx.debug_bounds("toast").is_none());
    assert!(ui.read(|app| app.toast.shown().is_none()), "the text is dropped after the exit");
}

#[gpui::test]
fn the_theme_menu_leaves_inert_and_reverses_when_toggled_in_flight(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let _motion = Motion::on();
    ui.click_on("theme");
    ui.advance(3000);
    assert!(ui.cx.debug_bounds("theme-menu").is_some());
    let monotone = theme::monotone();

    ui.click_on("theme");
    ui.advance(30);
    assert!(!ui.read(|app| app.theme_menu));
    assert!(ui.cx.debug_bounds("theme-menu").is_some(), "the menu is still drawn while it leaves");
    ui.click_on("theme-monotone");
    assert_eq!(theme::monotone(), monotone, "a leaving menu takes no clicks");

    // Toggled again before it has gone, the same menu comes back and works.
    ui.click_on("theme");
    assert!(ui.read(|app| app.theme_menu));
    ui.advance(20);
    ui.click_on("theme-monotone");
    assert_eq!(theme::monotone(), !monotone);
    ui.click_on("theme-monotone");
    assert_eq!(theme::monotone(), monotone);
    ui.advance(3000);
    assert!(ui.cx.debug_bounds("theme-menu").is_some());

    ui.keys("escape");
    ui.advance(3000);
    assert!(ui.cx.debug_bounds("theme-menu").is_none());
}

#[gpui::test]
fn the_help_sheet_and_the_unsaved_question_leave_inert_and_the_keys_follow_the_logical_state(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let _motion = Motion::on();
    ui.keys("?");
    ui.advance(3000);
    assert!(ui.cx.debug_bounds("help-card").is_some());
    ui.keys("escape");
    ui.advance(40);
    assert!(!ui.read(|app| app.show_help));
    assert!(ui.cx.debug_bounds("help-card").is_some(), "the sheet is still drawn while it fades");
    // Its scrim no longer holds the canvas: a press on it starts a pan.
    let corner = ui.read(|app| app.area.get().origin) + point(px(4.), px(4.));
    ui.press(corner);
    assert!(ui.read(|app| app.drag.is_some()));
    ui.release(corner);
    // And the keys are the canvas's again.
    ui.keys("n");
    assert!(ui.read(|app| app.prompt.get().is_some()));
    ui.keys("escape");
    ui.advance(3000);
    assert!(ui.cx.debug_bounds("help-card").is_none());

    ui.select("a");
    ui.keys("e");
    ui.type_text("draft");
    ui.keys("escape");
    ui.advance(3000);
    assert!(ui.cx.debug_bounds("notes-dialog").is_some());
    ui.keys("d");
    ui.advance(40);
    assert!(ui.read(|app| app.notes.is_none() && app.notes_ask.get().is_none()));
    assert!(ui.cx.debug_bounds("notes-dialog").is_some(), "the question is still drawn while it fades");
    ui.keys("n");
    assert!(ui.read(|app| app.prompt.get().is_some()), "the question no longer takes the keys");
    ui.keys("escape");
    ui.advance(3000);
    assert!(ui.cx.debug_bounds("notes-dialog").is_none());
}

#[gpui::test]
fn without_motion_every_floating_surface_appears_and_disappears_at_once(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    ui.keys("?");
    assert!(ui.cx.debug_bounds("help-card").is_some());
    ui.keys("escape");
    assert!(ui.cx.debug_bounds("help-card").is_none());
    ui.keys("/");
    assert!(ui.cx.debug_bounds("prompt-card").is_some());
    ui.keys("escape");
    assert!(ui.cx.debug_bounds("prompt-card").is_none());
    ui.app.update(ui.cx, |app, cx| app.toast("Saved", false, cx));
    ui.redraw();
    assert!(ui.cx.debug_bounds("toast").is_some());
    ui.cx.executor().advance_clock(std::time::Duration::from_secs(3));
    ui.redraw();
    assert!(ui.cx.debug_bounds("toast").is_none());
}

// ---- reflow and card motion ---------------------------------------------

impl Ui<'_> {
    /// How the card of `node` was drawn in the last frame, if it was.
    fn drawn(&mut self, node: &str) -> Option<crate::graph_view::CardMotion> {
        self.read(|app| app.graph_cache.drawn.borrow().get(&id(node)).copied())
    }

    /// The canvas point of the cell of `node`.
    fn rest(&mut self, node: &str) -> [f32; 2] {
        let (column, row) = self.read(|app| app.graph_cache.cells()[&id(node)]);
        [column as f32 * crate::CELL_W, row as f32 * crate::CELL_H]
    }

    /// Changes the view or the graph and draws the frame after the change.
    fn change(&mut self, f: impl FnOnce(&mut TopoApp, &mut gpui::Context<TopoApp>)) {
        self.app.update(self.cx, f);
        self.redraw();
    }

    /// Whether the card of `node` is laid out on its cell, to the pixel layout rounds to.
    fn on_cell(&mut self, node: &str) -> bool {
        let offset = self.cx.debug_bounds(format!("node-{node}").leak()).unwrap().origin - self.card(node, 0., 0.);
        offset.x.abs() <= px(0.5) && offset.y.abs() <= px(0.5)
    }

    fn reflowing(&mut self) -> bool {
        self.read(|app| app.graph_cache.reflowing())
    }
}

/// Whether `value` lies strictly between `a` and `b`.
fn between(value: f32, a: f32, b: f32) -> bool {
    a.min(b) < value && value < a.max(b)
}

#[gpui::test]
fn a_layout_change_moves_cards_on_springs_and_retargets_them_mid_flight(cx: &mut TestAppContext) {
    let mut ui = open(cx, VIEW_SAMPLE);
    view_fixture(&mut ui);
    ui.redraw();
    let _motion = Motion::on();
    // Hiding a moves b and m one column to the left.
    let shown = ui.rest("b");
    ui.change(|app, _| app.toggle_hide_completed());
    let hidden = ui.rest("b");
    assert_eq!((shown, hidden), ([crate::CELL_W, 0.], [0., 0.]));
    assert_eq!(ui.drawn("b").unwrap().at, shown, "the change starts where the card was");
    assert!(ui.reflowing());

    ui.advance(100);
    let midway = ui.drawn("b").unwrap().at[0];
    assert!(between(midway, hidden[0], shown[0]), "{midway}");
    // The card is laid out where it is drawn, and the camera's own move does not disturb it.
    let (bounds, origin, zoom) = (ui.cx.debug_bounds("node-b").unwrap(), ui.card("b", 0., 0.), ui.read(|app| app.zoom));
    assert!((f32::from(bounds.left() - origin.x) - midway * zoom).abs() <= 0.5, "layout rounds to pixels");

    // Showing a again turns the card round where it is, still moving the old way at first.
    ui.change(|app, _| app.toggle_hide_completed());
    let turned = ui.drawn("b").unwrap().at[0];
    assert!((turned - midway).abs() < 1e-3, "{turned} != {midway}");
    ui.advance(8);
    let coasting = ui.drawn("b").unwrap().at[0];
    assert!(coasting < turned && turned - coasting < 20., "{coasting} after {turned}");
    ui.advance(100);
    assert!(between(ui.drawn("b").unwrap().at[0], coasting, shown[0]));

    ui.advance(3000);
    assert_eq!(ui.drawn("b").unwrap().at, shown, "a settled card sits exactly on its cell");
    assert!(ui.on_cell("b") && !ui.reflowing());
}

#[gpui::test]
fn a_new_card_grows_in_and_a_deleted_one_fades_out_as_an_inert_ghost(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let _motion = Motion::on();
    ui.change(|app, cx| {
        app.mutate(cx, |graph| graph.insert(Node::new(id("new"), Kind::Task, "new".into())));
    });
    let entering = ui.drawn("new").unwrap();
    assert_eq!((entering.opacity, entering.at), (0., ui.rest("new")));
    assert!((entering.scale - 0.9).abs() < 1e-6, "{}", entering.scale);
    ui.advance(60);
    let growing = ui.drawn("new").unwrap();
    assert!(between(growing.opacity, 0., 1.) && between(growing.scale, 0.9, 1.), "{growing:?}");
    ui.advance(3000);
    let grown = ui.drawn("new").unwrap();
    assert_eq!((grown.opacity, grown.scale), (1., 1.));

    let spot = ui.card("c", 0.5, 0.5);
    ui.change(|app, cx| app.delete(id("c"), cx));
    assert_eq!(ui.read(|app| app.graph_cache.ghosts().len()), 1);
    ui.advance(40);
    let ghost = ui.drawn("c").expect("the deleted card is still drawn");
    assert!(between(ghost.opacity, 0., 1.) && between(ghost.scale, 0.9, 1.), "{ghost:?}");
    assert!(ui.cx.debug_bounds("node-c").is_some());
    // It is a picture of the card: a click goes through it to the canvas.
    ui.click(spot);
    assert_eq!(ui.selected(), None);
    assert!(ui.read(|app| app.hovered.is_none()));
    ui.advance(3000);
    assert!(ui.drawn("c").is_none() && ui.cx.debug_bounds("node-c").is_none());
    assert!(ui.read(|app| app.graph_cache.ghosts().is_empty()) && !ui.reflowing());
}

/// `lanes` chains of `length` tasks, named `t{lane}{step}`, all behind the task `root`.
fn lanes(lanes: usize, length: usize) -> Vec<(String, Vec<String>)> {
    let lane = |lane: usize| {
        (0..length).map(move |step| {
            let before = if step == 0 { "root".to_owned() } else { format!("t{lane:02}{:02}", step - 1) };
            (format!("t{lane:02}{step:02}"), vec![before])
        })
    };
    std::iter::once(("root".to_owned(), vec![])).chain((0..lanes).flat_map(lane)).collect()
}

fn open_lanes<'a>(cx: &'a mut TestAppContext, nodes: &[(String, Vec<String>)]) -> Ui<'a> {
    let dependencies: Vec<Vec<&str>> =
        nodes.iter().map(|(_, deps)| deps.iter().map(String::as_str).collect()).collect();
    let nodes: Vec<_> = nodes
        .iter()
        .zip(&dependencies)
        .map(|((name, _), deps)| (name.as_str(), Kind::Task, &deps[..], &[][..]))
        .collect();
    open(cx, &nodes)
}

#[gpui::test]
fn no_more_ghosts_are_drawn_than_the_cap_and_only_from_the_viewport(cx: &mut TestAppContext) {
    let mut ui = open_lanes(cx, &lanes(20, 4));
    ui.change(|app, cx| {
        let ids: Vec<_> = app.graph().nodes().map(|n| n.id.clone()).collect();
        app.mutate(cx, |graph| ids.iter().try_for_each(|id| graph.set_status(id, Status::Done)));
    });
    place(&mut ui, point(px(40.), px(40.)), 0.4);
    let _motion = Motion::on();
    ui.change(|app, _| app.toggle_hide_completed());
    assert_eq!(ui.shown(), [""; 0]);
    assert_eq!(ui.read(|app| app.graph_cache.ghosts().len()), 40, "81 cards left, all of them on screen");
    ui.advance(3000);
    assert!(ui.read(|app| app.graph_cache.ghosts().is_empty()));

    // Cards that leave outside the viewport are not drawn leaving at all.
    ui.change(|app, _| app.toggle_hide_completed());
    ui.advance(3000);
    place(&mut ui, point(px(-4000.), px(40.)), 0.4);
    ui.change(|app, _| app.toggle_hide_completed());
    assert!(ui.reflowing() && ui.read(|app| app.graph_cache.ghosts().is_empty()));
}

#[gpui::test]
fn a_layout_change_that_moves_too_many_cards_snaps(cx: &mut TestAppContext) {
    let mut ui = open_lanes(cx, &lanes(30, 20));
    ui.change(|app, cx| {
        app.mutate(cx, |graph| graph.set_status(&id("root"), Status::Done));
    });
    place(&mut ui, point(px(40.), px(40.)), 0.25);
    let _motion = Motion::on();
    let shown = ui.rest("t0005");
    // Hiding the root moves all 600 cards a column to the left.
    ui.change(|app, _| app.toggle_hide_completed());
    let hidden = ui.rest("t0005");
    assert_eq!(hidden[0], shown[0] - crate::CELL_W);
    assert_eq!(ui.drawn("t0005").unwrap().at, hidden);
    assert!(!ui.reflowing() && ui.read(|app| app.graph_cache.ghosts().is_empty()));
    assert!(ui.read(|app| app.graph_cache.drawn.borrow().len()) > 300);
}

#[gpui::test]
fn a_card_leaving_the_viewport_is_drawn_until_it_is_out_and_culling_is_tight_again(cx: &mut TestAppContext) {
    let chain = (0..5).map(|i| (format!("t{i}"), if i == 0 { vec![] } else { vec![format!("t{}", i - 1)] }));
    let chain: Vec<_> = chain.chain([("x".to_owned(), vec![])]).collect();
    let mut ui = open_lanes(cx, &chain);
    place(&mut ui, point(px(40.), px(40.)), 1.);
    let _motion = Motion::on();
    let width = ui.read(|app| f32::from(app.area.get().size.width));
    // Requiring the end of the chain sends x five columns to the right, out of the canvas.
    ui.change(|app, cx| {
        app.mutate(cx, |graph| graph.link(&id("x"), &id("t4")));
    });
    let rest = ui.rest("x");
    assert_eq!(rest, [5. * crate::CELL_W, 0.]);
    assert!(40. + rest[0] > width, "the cell of x is outside the canvas");
    assert_eq!(ui.drawn("x").unwrap().at[0], 0.);
    ui.advance(100);
    let midway = ui.drawn("x").expect("x is still on screen").at[0];
    assert!(between(midway, 0., rest[0]) && 40. + midway < width, "{midway}");
    assert!(ui.cx.debug_bounds("node-x").is_some());
    ui.advance(3000);
    assert!(ui.drawn("x").is_none() && ui.cx.debug_bounds("node-x").is_none());
    assert!(!ui.reflowing());
    assert_eq!(ui.read(|app| app.graph_cache.drawn.borrow().len()), 4, "only the cards in the canvas are drawn");

    // Coming back, it is drawn from the moment it enters.
    ui.change(|app, cx| {
        app.mutate(cx, |graph| graph.unlink(&id("x"), &id("t4")));
    });
    assert!(ui.drawn("x").is_none(), "x starts outside the canvas");
    ui.advance(300);
    assert!(between(ui.drawn("x").expect("x has entered").at[0], 0., rest[0]));
    ui.advance(3000);
    assert_eq!(ui.drawn("x").unwrap().at, ui.rest("x"));
}

#[gpui::test]
fn without_motion_every_layout_change_lands_at_once(cx: &mut TestAppContext) {
    let mut ui = open(cx, VIEW_SAMPLE);
    view_fixture(&mut ui);
    let landed = |ui: &mut Ui| {
        assert!(!ui.reflowing() && ui.read(|app| app.graph_cache.ghosts().is_empty()));
        for node in ui.shown() {
            let drawn = ui.drawn(&node).unwrap();
            assert_eq!((drawn.at, drawn.scale, drawn.check), (ui.rest(&node), 1., 1.), "{node}");
            assert!(ui.on_cell(&node), "{node}");
        }
        let shown = ui.shown().len();
        assert_eq!(ui.read(|app| app.graph_cache.drawn.borrow().len()), shown);
    };
    ui.change(|app, _| app.toggle_hide_completed());
    assert_eq!(ui.shown(), ["b", "d", "m"]);
    landed(&mut ui);
    ui.change(|app, _| app.toggle_hide_completed());
    landed(&mut ui);
    ui.change(|app, _| app.toggle_group_by_tag());
    ui.change(|app, _| app.toggle_group(&Group::Tag("core".into())));
    landed(&mut ui);
    ui.change(|app, _| app.toggle_group_by_tag());
    ui.change(|app, cx| {
        app.mutate(cx, |graph| graph.insert(Node::new(id("new"), Kind::Task, "new".into())));
    });
    assert_eq!(ui.drawn("new").unwrap().opacity, 1.);
    landed(&mut ui);
    ui.change(|app, cx| app.delete(id("new"), cx));
    assert!(ui.drawn("new").is_none());
    landed(&mut ui);
    ui.keys("cmd-z");
    landed(&mut ui);
    ui.change(|app, cx| app.toggle_done(id("b"), cx));
    ui.select("d");
    assert_eq!(ui.drawn("d").unwrap().selected, 1.);
    landed(&mut ui);
}

#[gpui::test]
fn selection_and_dimming_crossfade_and_completing_pops_the_check(cx: &mut TestAppContext) {
    let mut ui = open(cx, SAMPLE);
    let _motion = Motion::on();
    ui.select("a");
    let unselected = ui.drawn("a").unwrap();
    assert_eq!((unselected.selected, unselected.opacity), (0., 1.));
    ui.advance(40);
    assert!(between(ui.drawn("a").unwrap().selected, 0., 1.));
    // The cards outside the selection's chain dim over the same time.
    assert!(between(ui.drawn("c").unwrap().opacity, 0.22, 1.));
    ui.advance(1000);
    assert_eq!((ui.drawn("a").unwrap().selected, ui.drawn("c").unwrap().opacity), (1., 0.22));

    ui.change(|app, cx| app.toggle_done(id("b"), cx));
    let done = ui.drawn("b").unwrap();
    assert_eq!((done.check, done.done), (0.6, 0.));
    let mut peak: f32 = 0.;
    for _ in 0..60 {
        ui.advance(8);
        peak = peak.max(ui.drawn("b").unwrap().check);
    }
    assert!(peak > 1.01 && peak < 1.1, "the check overshoots a little: {peak}");
    ui.advance(2000);
    let done = ui.drawn("b").unwrap();
    assert_eq!((done.check, done.done), (1., 1.));

    // Taking it back lets the open glyph grow without a bounce.
    ui.change(|app, cx| app.toggle_done(id("b"), cx));
    assert_eq!(ui.drawn("b").unwrap().check, 0.6);
    let mut peak: f32 = 0.;
    for _ in 0..120 {
        ui.advance(8);
        peak = peak.max(ui.drawn("b").unwrap().check);
    }
    assert_eq!(peak, 1.);
    assert_eq!(ui.drawn("b").unwrap().done, 0.);
}

#[gpui::test]
fn folding_a_group_moves_the_headers_and_cards_below_it(cx: &mut TestAppContext) {
    let mut ui = open(cx, VIEW_SAMPLE);
    view_fixture(&mut ui);
    ui.change(|app, _| app.toggle_group_by_tag());
    ui.advance(0);
    let _motion = Motion::on();
    let header = |ui: &mut Ui| f32::from(ui.cx.debug_bounds("group-#ui").unwrap().top());
    let (open, card) = (header(&mut ui), ui.rest("b"));
    ui.change(|app, _| app.toggle_group(&Group::Tag("core".into())));
    assert_eq!(header(&mut ui), open);
    ui.advance(100);
    let (moving, folded) = (header(&mut ui), ui.rest("b"));
    assert!(moving < open && folded[1] < card[1]);
    assert!(between(ui.drawn("b").unwrap().at[1], folded[1], card[1]));
    ui.advance(3000);
    assert!(header(&mut ui) < moving);
    assert_eq!(ui.drawn("b").unwrap().at, folded);
    assert!(!ui.reflowing());
}
