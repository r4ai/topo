//! Comparable release CPU workloads; intentionally independent of cache internals.
use crate::{Prompt, TopoApp, inline::Field, text_input};
use gpui::{TestAppContext, point, px, size};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use topo_core::{
    Edit, Error, Graph, Kind, Node, NodeId, Op, Remote, Workspace,
    store::MemoryRemote,
    wire::{ApplyResult, Snapshot},
};

#[cfg(target_os = "macos")]
fn thread_cpu_time() -> Option<u64> {
    unsafe extern "C" {
        fn clock_gettime_nsec_np(clock_id: u32) -> u64;
    }
    // CLOCK_THREAD_CPUTIME_ID in the macOS SDK's _time.h.
    Some(unsafe { clock_gettime_nsec_np(16) })
}

#[cfg(not(target_os = "macos"))]
fn thread_cpu_time() -> Option<u64> {
    None
}

struct Sample {
    wall_ms: f64,
    cpu_ms: Option<f64>,
}
struct FrameClock {
    wall: Instant,
    cpu: Option<u64>,
}
impl FrameClock {
    fn now() -> Self {
        Self { wall: Instant::now(), cpu: thread_cpu_time() }
    }
    fn sample(&self) -> Sample {
        Sample {
            wall_ms: self.wall.elapsed().as_secs_f64() * 1000.,
            cpu_ms: self.cpu.zip(thread_cpu_time()).map(|(start, end)| (end - start) as f64 / 1_000_000.),
        }
    }
}

fn fixture(count: usize, density: usize) -> Graph {
    let mut nodes: Vec<_> = (0..count)
        .map(|i| {
            let mut n = Node::new(NodeId(format!("n{i:05}")), Kind::Task, format!("Task {i} with metadata"));
            n.tags = vec!["gui".into(), "performance".into()];
            n.assignee = Some("codex".into());
            n.prs = vec!["https://github.com/r4ai/topo/pull/1".into()];
            if i >= 25 {
                n.depends_on = (0..density.min(25)).map(|d| NodeId(format!("n{:05}", i - 25 + d))).collect();
            }
            if i % 20 == 0 {
                n.milestones = vec![NodeId("m".into())];
            }
            n
        })
        .collect();
    nodes.push(Node::new(NodeId("m".into()), Kind::Milestone, "Release".into()));
    Graph::from_nodes(nodes).unwrap()
}
fn report(label: &str, samples: Vec<Sample>) {
    let print = |prefix: &str, mut times: Vec<f64>| {
        times.sort_by(f64::total_cmp);
        eprintln!(
            "{prefix} {label}: median={:.3}ms p95={:.3}ms",
            times[times.len() / 2],
            times[times.len() * 95 / 100]
        );
    };
    print("PERF", samples.iter().map(|sample| sample.wall_ms).collect());
    let cpu: Vec<_> = samples.iter().filter_map(|sample| sample.cpu_ms).collect();
    if !cpu.is_empty() {
        print("PERF_CPU", cpu);
    }
}

#[gpui::test]
#[ignore]
fn responsiveness_benchmark(cx: &mut TestAppContext) {
    cx.update(text_input::bind_keys);
    for (count, density) in [(100, 1), (5000, 1), (1000, 12), (5000, 0), (20000, 1)] {
        if std::env::var("TOPO_PERF_CASE").ok().is_some_and(|case| case != format!("{count}/{density}")) {
            continue;
        }
        let dir = tempfile::tempdir().unwrap();
        let mut ws = Workspace::init(dir.path()).unwrap();
        ws.graph = fixture(count, density);
        ws.save().unwrap();
        let (app, visual) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
        visual.simulate_resize(size(px(1360.), px(860.)));
        visual.run_until_parked();
        for motion in ["pan", "zoom", "hover", "selection", "search", "input"] {
            app.update_in(visual, |app, window, cx| {
                app.anim = None;
                app.offset = point(px(40.), px(40.));
                app.zoom = 1.;
                if motion == "search" {
                    app.open_prompt(Prompt::Search, window, cx);
                }
                if motion == "input" {
                    app.close_prompt(window, cx);
                    app.select(Some(NodeId("n00000".into())), false);
                    app.start_inline(Field::Title, window, cx);
                }
            });
            visual.run_until_parked();
            let mut samples = Vec::new();
            for frame in 0..40 {
                let start = FrameClock::now();
                app.update(visual, |app, cx| {
                    app.anim = None;
                    match motion {
                        "pan" => app.offset.x += px(if frame % 2 == 0 { 8. } else { -8. }),
                        "zoom" => app.zoom = if frame % 2 == 0 { 0.9 } else { 1. },
                        "hover" => app.hovered = Some(NodeId(format!("n{:05}", frame % 25))),
                        "selection" => app.select(Some(NodeId(format!("n{:05}", frame % 25))), false),
                        "search" => app.palette.update(cx, |palette, cx| {
                            palette.set_text(if frame % 2 == 0 { "Task 42" } else { "Task 4" }, cx)
                        }),
                        "input" => app.combo.update(cx, |combo, cx| {
                            combo.set_text(if frame % 2 == 0 { "edited" } else { "editing" }, cx)
                        }),
                        _ => unreachable!(),
                    }
                    cx.notify();
                });
                visual.run_until_parked();
                samples.push(start.sample());
            }
            report(&format!("nodes={count} density={density} {motion}"), samples);
        }
        // Explicitly close each fixture's native event source before the next workload.
        app.update(visual, |app, _| app.retire());
    }
}

#[test]
#[ignore]
fn milestone_layout_benchmark() {
    for count in [1000, 5000] {
        let mut nodes = Vec::new();
        for i in 0..count {
            let mut task = Node::new(NodeId(format!("t{i:05}")), Kind::Task, format!("Task {i}"));
            task.milestones.push(NodeId(format!("m{i:05}")));
            nodes.push(task);
            nodes.push(Node::new(NodeId(format!("m{i:05}")), Kind::Milestone, format!("Milestone {i}")));
        }
        let graph = Graph::from_nodes(nodes).unwrap();
        for grouped in [false, true] {
            let mut samples = Vec::new();
            for _ in 0..10 {
                let start = FrameClock::now();
                let layout =
                    crate::layout::layout(&graph, &crate::layout::View { group_by_tag: grouped, ..Default::default() });
                assert_eq!(layout.slots.len(), count * 2);
                samples.push(start.sample());
            }
            report(&format!("{} nodes {} milestones grouped={grouped} layout", count * 2, count), samples);
        }
    }
}

#[gpui::test]
#[ignore]
fn large_selection_frame_benchmark(cx: &mut TestAppContext) {
    for count in [1000, 5000] {
        let dir = tempfile::tempdir().unwrap();
        let mut ws = Workspace::init(dir.path()).unwrap();
        ws.graph = fixture(count, 1);
        ws.save().unwrap();
        let (app, visual) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
        visual.simulate_resize(size(px(1360.), px(860.)));
        visual.run_until_parked();
        app.update(visual, |app, cx| {
            let ids: Vec<_> = app.graph().nodes().map(|n| n.id.clone()).collect();
            app.select_nodes(&ids, cx);
            app.anim = None;
            app.offset = point(px(40.), px(40.));
            app.zoom = 1.;
        });
        visual.run_until_parked();
        let mut samples = Vec::new();
        for frame in 0..40 {
            let start = FrameClock::now();
            app.update(visual, |app, cx| {
                app.offset.x += px(if frame % 2 == 0 { 8. } else { -8. });
                cx.notify();
            });
            visual.run_until_parked();
            samples.push(start.sample());
        }
        report(&format!("{count} selected nodes pan"), samples);
        app.update(visual, |app, _| app.retire());
    }
}

struct SlowRemote(MemoryRemote);
impl Remote for SlowRemote {
    fn fetch(&self, known: Option<u64>) -> Result<Option<Snapshot>, Error> {
        self.0.fetch(known)
    }
    fn apply(&self, ops: &[Op]) -> Result<ApplyResult, Error> {
        std::thread::sleep(Duration::from_millis(120));
        self.0.apply(ops)
    }
}

#[gpui::test]
#[ignore]
fn slow_save_response_benchmark(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let remote = Arc::new(SlowRemote(MemoryRemote::default()));
    remote.0.apply(&topo_core::ops::diff(&Default::default(), &fixture(500, 1).into_nodes())).unwrap();
    let ws = Workspace::open_remote(dir.path().to_owned(), remote).unwrap();
    let (app, visual) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
    visual.simulate_resize(size(px(1360.), px(860.)));
    visual.run_until_parked();
    let mut display = Vec::new();
    let mut persisted = Vec::new();
    for i in 0..20 {
        let start = FrameClock::now();
        app.update(visual, |app, cx| {
            app.mutate(cx, |graph| {
                graph.edit(&NodeId("n00000".into()), Edit { title: Some(format!("edited {i}")), ..Edit::default() })
            });
            assert_eq!(app.graph().get(&NodeId("n00000".into())).unwrap().title, format!("edited {i}"));
        });
        display.push(start.sample());
        visual.run_until_parked();
        persisted.push(start.sample());
    }
    report("500 nodes 120ms remote edit-to-visible-state", display);
    report("500 nodes 120ms remote edit-to-settled", persisted);
    app.update(visual, |app, _| app.retire());
}
