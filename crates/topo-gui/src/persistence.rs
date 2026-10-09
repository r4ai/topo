//! One serial persistence lane per editor. The visible graph is the confirmed
//! workspace plus ordered patches; I/O never borrows the editor on the UI thread.

use std::collections::{BTreeMap, VecDeque};

use gpui::Context;
use topo_core::{Graph, Node, NodeId, Op, Workspace, ops, wire::Snapshot};

use crate::TopoApp;
use crate::notes::Then;

#[derive(Clone)]
struct Patch {
    ops: Vec<Op>,
    key: String,
    additions: BTreeMap<NodeId, Node>,
    /// apply succeeded but fetching the authoritative timestamps failed.
    committed: bool,
    minimum_version: Option<u64>,
}

impl Patch {
    fn between(before: &Graph, after: &Graph) -> Self {
        static SERIAL: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let key =
            format!("{:016x}{:08x}{serial:08x}", jiff::Timestamp::now().as_nanosecond() as u64, std::process::id());
        Self {
            key,
            ops: ops::diff(&before.clone().into_nodes(), &after.clone().into_nodes()),
            additions: after
                .nodes()
                .filter(|n| before.get(&n.id).is_none())
                .map(|n| (n.id.clone(), n.clone()))
                .collect(),
            committed: false,
            minimum_version: None,
        }
    }

    /// Replaying after an ambiguous response must not create a second node.
    /// Fixed IDs and field-level patches also preserve unrelated writers' edits.
    fn replay(&self, graph: &Graph) -> Result<Graph, topo_core::Error> {
        let mut graph = graph.clone();
        for operation in &self.ops {
            let already_applied = match operation {
                Op::Add { id: Some(id), .. } => graph.get(id).is_some_and(|node| self.additions[id].same_content(node)),
                Op::Remove { id } => graph.get(&NodeId(id.clone())).is_none(),
                _ => false,
            };
            if !already_applied {
                ops::apply(&mut graph, std::slice::from_mut(&mut operation.clone()))?;
            }
        }
        Ok(graph)
    }
}

pub(crate) struct Persistence {
    confirmed: Workspace,
    pending: VecDeque<Patch>,
    running: bool,
    pub(crate) error: Option<String>,
    snapshot: Option<Snapshot>,
    reload: bool,
}

impl Persistence {
    pub(crate) fn new(confirmed: Workspace) -> Self {
        Self { confirmed, pending: VecDeque::new(), running: false, error: None, snapshot: None, reload: false }
    }

    pub(crate) fn pending_count(&self) -> usize {
        self.pending.len()
    }
    #[cfg(test)]
    pub(crate) fn set_busy_for_test(&mut self, busy: bool) {
        self.running = busy;
    }

    pub(crate) fn busy(&self) -> bool {
        self.running || (!self.pending.is_empty() && self.error.is_none())
    }
    pub(crate) fn unsettled(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Shows the save-status overlay in a capture without any I/O.
    #[cfg(feature = "screenshot")]
    pub(crate) fn show_for_capture(&mut self, error: bool) {
        self.pending.push_back(Patch {
            ops: Vec::new(),
            key: "capture".into(),
            additions: BTreeMap::new(),
            committed: false,
            minimum_version: None,
        });
        self.error = error.then(|| "the capture shows the failure state".to_owned());
    }
}

enum Work {
    Save(Patch),
    Snapshot(Snapshot),
    Reload,
}

impl TopoApp {
    /// One wake at the local date boundary; no periodic whole-view redraws.
    pub(crate) fn date_clock(cx: &mut Context<Self>) -> gpui::Task<()> {
        cx.spawn(async move |this, cx| {
            let mut today = crate::dates::today();
            loop {
                let now = jiff::Zoned::now();
                let seconds = now
                    .tomorrow()
                    .and_then(|next| next.start_of_day())
                    .map(|next| (next.timestamp().as_second() - now.timestamp().as_second()).max(1) as u64)
                    .unwrap_or(3600)
                    .min(3600);
                cx.background_executor().timer(std::time::Duration::from_secs(seconds)).await;
                let next = crate::dates::today();
                if next != today {
                    today = next;
                    if this
                        .update(cx, |app, cx| {
                            if app.poll_visible && app.poll_active {
                                cx.notify();
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }
        })
    }

    pub(crate) fn enqueue_save(&mut self, before: &Graph, cx: &mut Context<Self>) {
        let patch = Patch::between(before, &self.ws.graph);
        if patch.ops.is_empty() {
            return;
        }
        // The front request is immutable once dispatched (including its retry key).
        // Batch only later, unsent edits, bounding both HTTP payloads and replay work.
        if self.persistence.pending.len() > 1
            && self.persistence.pending.back().is_some_and(|tail| tail.ops.len() + patch.ops.len() <= 128)
        {
            let tail = self.persistence.pending.back_mut().expect("pending tail");
            tail.ops.extend(patch.ops);
            tail.additions.extend(patch.additions);
        } else {
            self.persistence.pending.push_back(patch);
        }
        self.start_io(cx);
    }

    pub(crate) fn retry_save(&mut self, cx: &mut Context<Self>) {
        self.persistence.error = None;
        self.start_io(cx);
        cx.notify();
    }

    pub(crate) fn discard_pending(&mut self, cx: &mut Context<Self>) {
        if self.persistence.running {
            return;
        }
        self.persistence.pending.clear();
        self.persistence.error = None;
        let before = self.ws.graph.clone();
        self.ws = self.persistence.confirmed.clone();
        self.graph_cache.changed(&before, &self.ws.graph);
        self.prune_selection();
        self.undo.clear();
        self.redo.clear();
        self.reload_background(cx);
        cx.notify();
    }

    pub(crate) fn receive_snapshot(&mut self, snapshot: Snapshot, cx: &mut Context<Self>) {
        let version = self.persistence.confirmed.remote().map(|(_, v)| v).unwrap_or(0);
        if snapshot.version <= version {
            return;
        }
        if self.persistence.snapshot.as_ref().is_none_or(|old| snapshot.version > old.version) {
            self.persistence.snapshot = Some(snapshot);
        }
        self.start_io(cx);
    }

    pub(crate) fn reload_background(&mut self, cx: &mut Context<Self>) {
        if self.retired {
            return;
        }
        self.persistence.reload = true;
        self.start_io(cx);
    }

    fn start_io(&mut self, cx: &mut Context<Self>) {
        if self.retired || self.persistence.running || self.persistence.error.is_some() {
            return;
        }
        let work = if let Some(patch) = self.persistence.pending.front() {
            Work::Save(patch.clone())
        } else if let Some(snapshot) = self.persistence.snapshot.take() {
            Work::Snapshot(snapshot)
        } else if std::mem::take(&mut self.persistence.reload) {
            Work::Reload
        } else {
            return;
        };
        self.persistence.running = true;
        let mut ws = self.persistence.confirmed.clone();
        let saving = matches!(work, Work::Save(_));
        let task = cx.background_executor().spawn(async move {
            let original = ws.clone();
            let mut committed = false;
            let mut minimum_version = None;
            let result = (|| -> Result<(), topo_core::Error> {
                match work {
                    Work::Save(patch) => {
                        committed = patch.committed;
                        minimum_version = patch.minimum_version;
                        // Remote apply merges against the server's current graph. Only local
                        // replay needs a fresh read before writing; acknowledged retries just fetch.
                        if ws.remote().is_none() || committed {
                            ws.reload()?;
                        }
                        if !committed {
                            if let Some((remote, _)) = ws.remote() {
                                minimum_version = Some(remote.apply_with_key(&patch.ops, &patch.key)?.version);
                                committed = true;
                                ws.reload()?;
                            } else {
                                ws.graph = patch.replay(&ws.graph)?;
                                ws.save()?;
                                committed = true;
                            }
                        }
                    }
                    Work::Snapshot(snapshot) => {
                        if ws.remote().is_some_and(|(_, v)| snapshot.version > v) {
                            ws.install(snapshot)?;
                        }
                    }
                    Work::Reload => {
                        ws.reload()?;
                    }
                }
                if let Some(minimum) = minimum_version
                    && ws.remote().is_none_or(|(_, version)| version < minimum)
                {
                    return Err(topo_core::Error::Remote(
                        std::io::Error::other("Waiting for the acknowledged version").into(),
                    ));
                }
                Ok(())
            })()
            .map_err(|e| e.to_string());
            if result.is_err() && ws.remote().is_none() && ws.reload().is_err() {
                ws = original.clone();
            }
            if ws.remote().is_some_and(|(_, v)| original.remote().is_some_and(|(_, old)| v < old)) {
                ws = original;
            }
            (ws, result, committed, minimum_version)
        });
        cx.spawn(async move |this, cx| {
            let (ws, result, committed, minimum_version) = task.await;
            let _ = this.update(cx, |app, cx| {
                if app.retired {
                    return;
                }
                app.persistence.running = false;
                let before = app.ws.graph.clone();
                // This lane serializes installs and writes, so an older poll cannot replace a save.
                app.persistence.confirmed = ws;
                if saving {
                    match result {
                        Ok(()) => {
                            app.persistence.pending.pop_front();
                        }
                        Err(ref e) => {
                            if let Some(patch) = app.persistence.pending.front_mut() {
                                patch.committed = committed;
                                patch.minimum_version = minimum_version;
                            }
                            app.persistence.error = Some(e.clone());
                            app.toast(format!("Changes need confirmation: {e}"), true, cx);
                        }
                    }
                } else if let Err(ref e) = result {
                    app.toast(format!("Reload failed: {e}"), true, cx);
                }
                let mut visible = app.persistence.confirmed.graph.clone();
                for patch in &app.persistence.pending {
                    match patch.replay(&visible) {
                        Ok(graph) => visible = graph,
                        Err(e) => {
                            app.persistence.error = Some(format!("Pending edit conflicts: {e}"));
                            // Keep the draft and every queued operation available for retry/discard.
                            visible = before.clone();
                            break;
                        }
                    }
                }
                let changed = before != visible;
                let external = !before.same_content(&visible);
                app.ws = app.persistence.confirmed.clone();
                app.ws.graph = visible;
                app.graph_cache.changed(&before, &app.ws.graph);
                if external {
                    app.undo.clear();
                    app.redo.clear();
                    app.prune_selection();
                }
                if changed && app.prompt.get().is_some() {
                    app.refresh_palette(cx);
                }
                if saving || changed {
                    cx.notify();
                }
                app.start_io(cx);
            });
        })
        .detach();
    }

    pub(crate) fn can_leave(&mut self, cx: &mut Context<Self>) -> bool {
        self.can_leave_to(Then::Stay, cx)
    }

    /// Whether the window may be left now. Unsaved notes stop it and ask what to
    /// do with them first, after which `then` happens.
    pub(crate) fn can_leave_to(&mut self, then: Then, cx: &mut Context<Self>) -> bool {
        if self.notes_dirty(cx) {
            self.ask_notes(then, cx);
            false
        } else if self.persistence.unsettled() {
            self.toast("Finish saving or resolve unsaved changes before closing or switching repositories", true, cx);
            false
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    use topo_core::{Edit, Error, Kind, Remote, Status, store::MemoryRemote, wire::ApplyResult};

    #[derive(Default)]
    struct ControlledRemote {
        inner: MemoryRemote,
        offline: AtomicBool,
        denied: AtomicBool,
        stale_after_apply: AtomicBool,
        stale: Mutex<Option<Snapshot>>,
        lose_reply: AtomicBool,
        lose_fetch: AtomicBool,
        lose_fetch_after_apply: AtomicBool,
        writes: AtomicUsize,
        fetches: AtomicUsize,
        requests: Mutex<BTreeMap<String, (Vec<Op>, ApplyResult)>>,
    }
    impl Remote for ControlledRemote {
        fn fetch(&self, known: Option<u64>) -> Result<Option<Snapshot>, Error> {
            self.fetches.fetch_add(1, Ordering::SeqCst);
            if let Some(stale) = self.stale.lock().unwrap().take() {
                return Ok(Some(stale));
            }
            if self.offline.load(Ordering::SeqCst) || self.lose_fetch.swap(false, Ordering::SeqCst) {
                return Err(Error::Remote(std::io::Error::other("offline").into()));
            }
            self.inner.fetch(known)
        }
        fn apply(&self, ops: &[Op]) -> Result<ApplyResult, Error> {
            self.inner.apply(ops)
        }
        fn apply_with_key(&self, ops: &[Op], key: &str) -> Result<ApplyResult, Error> {
            if self.offline.load(Ordering::SeqCst) {
                return Err(Error::Remote(std::io::Error::other("offline").into()));
            }
            if self.denied.load(Ordering::SeqCst) {
                return Err(Error::Remote(
                    std::io::Error::new(std::io::ErrorKind::PermissionDenied, "read-only workspace").into(),
                ));
            }
            let mut requests = self.requests.lock().unwrap();
            if let Some((original, result)) = requests.get(key) {
                assert_eq!(original, ops, "a retry must send exactly the same body");
                return Ok(result.clone());
            }
            self.writes.fetch_add(1, Ordering::SeqCst);
            if self.stale_after_apply.swap(false, Ordering::SeqCst) {
                *self.stale.lock().unwrap() = self.inner.fetch(None)?;
            }
            let result = self.inner.apply(ops)?;
            requests.insert(key.into(), (ops.to_vec(), result.clone()));
            if self.lose_fetch_after_apply.swap(false, Ordering::SeqCst) {
                self.lose_fetch.store(true, Ordering::SeqCst);
            }
            if self.lose_reply.swap(false, Ordering::SeqCst) {
                Err(Error::Remote(std::io::Error::other("reply lost after commit").into()))
            } else {
                Ok(result)
            }
        }
    }

    fn id(s: &str) -> NodeId {
        NodeId(s.into())
    }
    fn add(s: &str) -> Op {
        // Reuse the graph diff contract, including explicit stable node IDs.
        let mut graph = Graph::default();
        graph.insert(Node::new(id(s), Kind::Task, s.into())).unwrap();
        ops::diff(&BTreeMap::new(), &graph.into_nodes()).remove(0)
    }

    #[gpui::test]
    fn remote_save_uses_one_write_and_one_confirmation_fetch(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let remote = Arc::new(ControlledRemote::default());
        remote.apply(&[add("a")]).unwrap();
        let ws = Workspace::open_remote(dir.path().to_owned(), remote.clone()).unwrap();
        let (app, cx) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
        cx.run_until_parked();
        let fetched = remote.fetches.load(Ordering::SeqCst);
        app.update(cx, |app, cx| {
            app.mutate(cx, |g| g.set_status(&id("a"), Status::Done));
        });
        cx.run_until_parked();
        assert_eq!(remote.writes.load(Ordering::SeqCst), 1);
        assert_eq!(remote.fetches.load(Ordering::SeqCst) - fetched, 1);
        assert!(!app.read_with(cx, |app, _| app.persistence.unsettled()));
    }

    #[gpui::test]
    fn edits_are_visible_before_io_and_ordered_with_create_edit_delete_and_undo(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let remote = Arc::new(ControlledRemote::default());
        remote.apply(&[add("a"), add("other")]).unwrap();
        let ws = Workspace::open_remote(dir.path().to_owned(), remote.clone()).unwrap();
        let (app, cx) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
        cx.run_until_parked();
        app.update(cx, |app, cx| {
            assert!(app.mutate(cx, |g| g.set_status(&id("a"), Status::Done)));
            assert_eq!(app.graph().get(&id("a")).unwrap().status, Status::Done);
            assert_eq!(remote.writes.load(Ordering::SeqCst), 0);
            app.mutate(cx, |g| g.insert(Node::new(id("new"), Kind::Task, "draft".into())));
            app.mutate(cx, |g| g.edit(&id("new"), Edit { title: Some("edited".into()), ..Edit::default() }));
            app.mutate(cx, |g| g.remove(&id("new")).map(drop));
            app.restore(true, cx); // undo delete while its save is pending
            assert_eq!(app.graph().get(&id("new")).unwrap().title, "edited");
            assert!(!app.can_leave(cx));
            app.offset.x += gpui::px(25.); // camera remains usable while saving
            app.select(Some(id("a")), false);
        });
        // A concurrent writer and an older poll must survive/preserve pending edits.
        remote.apply(&[Op::Status { id: "other".into(), status: Status::Doing, if_status: None }]).unwrap();
        let snapshot = remote.fetch(None).unwrap().unwrap();
        app.update(cx, |app, cx| app.fetched(Ok(Some(snapshot)), cx));
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert_eq!(app.persistence.pending_count(), 0);
            assert!(app.persistence.error.is_none());
            assert_eq!(app.graph().get(&id("a")).unwrap().status, Status::Done);
            assert_eq!(app.graph().get(&id("other")).unwrap().status, Status::Doing);
            assert_eq!(app.graph().get(&id("new")).unwrap().title, "edited");
            assert!(app.graph().get(&id("new")).unwrap().created_at.is_some());
            let server =
                Graph::from_nodes(remote.fetch(None).unwrap().unwrap().nodes.into_iter().map(Node::from)).unwrap();
            assert_eq!(app.graph(), &server);
        });
    }

    #[gpui::test]
    fn failure_keeps_later_drafts_and_retry_deduplicates_an_ambiguous_write(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let remote = Arc::new(ControlledRemote::default());
        remote.apply(&[add("a")]).unwrap();
        let ws = Workspace::open_remote(dir.path().to_owned(), remote.clone()).unwrap();
        let (app, cx) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
        cx.run_until_parked();
        remote.lose_reply.store(true, Ordering::SeqCst);
        app.update(cx, |app, cx| {
            app.mutate(cx, |g| g.insert(Node::new(id("new"), Kind::Task, "new".into())));
            app.mutate(cx, |g| g.edit(&id("a"), Edit { title: Some("later".into()), ..Edit::default() }));
        });
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert!(app.persistence.error.is_some());
            assert_eq!(app.persistence.pending_count(), 2);
            assert!(app.graph().get(&id("new")).is_some());
            assert_eq!(app.graph().get(&id("a")).unwrap().title, "later");
        });
        remote.apply(&[add("collab")]).unwrap();
        app.update(cx, |app, cx| app.retry_save(cx));
        cx.run_until_parked();
        assert_eq!(remote.writes.load(Ordering::SeqCst), 2); // two edits, never a second creation
        app.read_with(cx, |app, _| {
            assert!(!app.persistence.unsettled());
            assert!(app.persistence.error.is_none());
            assert!(app.graph().get(&id("collab")).is_some());
            assert_eq!(app.graph().get(&id("a")).unwrap().title, "later");
        });
    }

    #[gpui::test]
    fn acknowledged_writes_retry_only_the_failed_fetch(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let remote = Arc::new(ControlledRemote::default());
        remote.apply(&[add("a")]).unwrap();
        let ws = Workspace::open_remote(dir.path().to_owned(), remote.clone()).unwrap();
        let (app, cx) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
        cx.run_until_parked();
        remote.lose_fetch_after_apply.store(true, Ordering::SeqCst);
        app.update(cx, |app, cx| {
            app.mutate(cx, |g| g.set_status(&id("a"), Status::Done));
        });
        cx.run_until_parked();
        assert!(app.read_with(cx, |app, _| app.persistence.error.is_some()));
        assert_eq!(remote.writes.load(Ordering::SeqCst), 1);
        app.update(cx, |app, cx| app.retry_save(cx));
        cx.run_until_parked();
        assert_eq!(remote.writes.load(Ordering::SeqCst), 1);
        assert!(!app.read_with(cx, |app, _| app.persistence.unsettled()));
        assert!(app.read_with(cx, |app, _| app.graph().get(&id("a")).unwrap().completed_at.is_some()));
    }

    #[gpui::test]
    fn stale_confirmation_and_permission_errors_preserve_the_draft(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let remote = Arc::new(ControlledRemote::default());
        remote.apply(&[add("a")]).unwrap();
        let ws = Workspace::open_remote(dir.path().to_owned(), remote.clone()).unwrap();
        let (app, cx) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
        cx.run_until_parked();
        remote.stale_after_apply.store(true, Ordering::SeqCst);
        app.update(cx, |app, cx| {
            app.mutate(cx, |g| g.set_status(&id("a"), Status::Done));
        });
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert!(app.persistence.error.is_some());
            assert!(app.persistence.pending.front().unwrap().committed);
            assert_eq!(app.graph().get(&id("a")).unwrap().status, Status::Done);
        });
        app.update(cx, |app, cx| app.retry_save(cx));
        cx.run_until_parked();
        assert_eq!(remote.writes.load(Ordering::SeqCst), 1);
        remote.denied.store(true, Ordering::SeqCst);
        app.update(cx, |app, cx| {
            app.mutate(cx, |g| g.edit(&id("a"), Edit { title: Some("retained draft".into()), ..Edit::default() }));
        });
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert!(app.persistence.error.as_ref().unwrap().contains("read-only"));
            assert_eq!(app.graph().get(&id("a")).unwrap().title, "retained draft");
        });
        remote.denied.store(false, Ordering::SeqCst);
        app.update(cx, |app, cx| app.retry_save(cx));
        cx.run_until_parked();
        assert!(!app.read_with(cx, |app, _| app.persistence.unsettled()));
    }

    #[gpui::test]
    fn offline_drafts_can_be_discarded_without_losing_other_writers(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let remote = Arc::new(ControlledRemote::default());
        remote.apply(&[add("a")]).unwrap();
        let ws = Workspace::open_remote(dir.path().to_owned(), remote.clone()).unwrap();
        let (app, cx) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
        cx.run_until_parked();
        cx.simulate_resize(gpui::size(gpui::px(900.), gpui::px(600.)));
        app.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        let zoom = cx.debug_bounds("zoom").unwrap();
        remote.offline.store(true, Ordering::SeqCst);
        app.update(cx, |app, cx| {
            app.mutate(cx, |g| g.set_status(&id("a"), Status::Done));
        });
        cx.run_until_parked();
        assert!(app.read_with(cx, |app, _| app.persistence.error.is_some()));
        // The save status is an overlay: it does not resize the canvas under it.
        assert_eq!(cx.debug_bounds("zoom"), Some(zoom));
        cx.simulate_resize(gpui::size(gpui::px(720.), gpui::px(480.)));
        app.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        assert!(cx.debug_bounds("save-status").is_some());
        for selector in ["retry-save", "discard-save"] {
            let bounds = cx.debug_bounds(selector).expect("failure remains actionable in a narrow window");
            assert!(bounds.right() <= gpui::px(720.));
        }
        remote.offline.store(false, Ordering::SeqCst);
        remote.apply(&[add("collab")]).unwrap();
        app.update(cx, |app, cx| app.discard_pending(cx));
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert!(!app.persistence.unsettled());
            assert_eq!(app.graph().get(&id("a")).unwrap().status, Status::Todo);
            assert!(app.graph().get(&id("collab")).is_some());
        });
    }

    #[gpui::test]
    fn local_external_edits_are_merged_before_a_background_save(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let mut ws = Workspace::init(dir.path()).unwrap();
        ws.graph.insert(Node::new(id("a"), Kind::Task, "a".into())).unwrap();
        ws.save().unwrap();
        let (app, cx) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
        cx.run_until_parked();
        app.update(cx, |app, cx| {
            app.mutate(cx, |g| g.set_status(&id("a"), Status::Done));
        });
        let mut disk = Workspace::discover(dir.path()).unwrap();
        disk.graph.edit(&id("a"), Edit { title: Some("CLI title".into()), ..Edit::default() }).unwrap();
        disk.save().unwrap();
        cx.run_until_parked();
        let disk = Workspace::discover(dir.path()).unwrap();
        assert_eq!(disk.graph.get(&id("a")).unwrap().title, "CLI title");
        assert_eq!(disk.graph.get(&id("a")).unwrap().status, Status::Done);
        assert_eq!(app.read_with(cx, |app, _| app.graph().clone()), disk.graph);
    }
}
