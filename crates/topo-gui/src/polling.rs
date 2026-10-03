//! One scheduler per workspace view. Activation wakes it; inactive windows
//! have no periodic timer or requests. A view owns its task and subscriptions,
//! so replacing the view also invalidates every old completion.

use std::time::Duration;

use futures::{FutureExt, StreamExt, channel::mpsc, select_biased};
use gpui::{Context, Task};

use crate::{REMOTE_POLL, TopoApp};

const MAX_IDLE: Duration = Duration::from_secs(30);

const MAX_RETRY: Duration = Duration::from_secs(60);

impl TopoApp {
    pub(crate) fn set_poll_visible(&mut self, visible: bool) {
        if self.poll_visible != visible {
            self.poll_visible = visible;
            if let Some(wake) = &self.poll_wake {
                let _ = wake.unbounded_send(());
            }
        }
    }

    pub(crate) fn set_poll_active(&mut self, active: bool) {
        if self.poll_active != active {
            self.poll_active = active;
            if let Some(wake) = &self.poll_wake {
                let _ = wake.unbounded_send(());
            }
        }
    }

    pub(crate) fn poll_remote(cx: &mut Context<Self>) -> (mpsc::UnboundedSender<()>, Task<()>) {
        let (wake, mut events) = mpsc::unbounded();
        let task = cx.spawn(async move |this, cx| {
            let mut delay = REMOTE_POLL;
            let mut failed = false;
            loop {
                let Ok(active) = this.read_with(cx, |app, _| app.poll_active && app.poll_visible) else { break };
                let woke = if active {
                    let timer = cx.background_executor().timer(delay).fuse();
                    let event = events.next().fuse();
                    futures::pin_mut!(timer, event);
                    select_biased! {
                        event = event => {
                            if event.is_none() { break; }
                            true
                        },
                        _ = timer => false,
                    }
                } else {
                    if events.next().await.is_none() {
                        break;
                    }
                    true
                };
                if woke {
                    delay = REMOTE_POLL;
                }
                // Coalesce activation bursts, including events received during a fetch.
                while events.try_recv().is_ok() {}
                let Ok(request) = this.read_with(cx, |app, _| {
                    (app.poll_active && app.poll_visible && !app.persistence.busy()).then(|| app.ws.remote()).flatten()
                }) else {
                    break;
                };
                let Some((remote, version)) = request else {
                    delay = REMOTE_POLL;
                    continue;
                };
                let fetched = cx.background_executor().spawn(async move { remote.fetch(Some(version)) }).await;
                delay = if fetched.is_err() {
                    if failed { (delay.max(REMOTE_POLL) * 2).min(MAX_RETRY) } else { REMOTE_POLL * 2 }
                } else if matches!(fetched, Ok(None)) && !woke && !failed {
                    (delay.max(REMOTE_POLL) * 2).min(MAX_IDLE)
                } else {
                    REMOTE_POLL
                };
                let error = fetched.is_err();
                if this
                    .update(cx, |app, cx| {
                        // One toast per outage; an unchanged response never notifies the view.
                        if !error || !failed {
                            app.fetched(fetched, cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
                failed = error;
                // Returning during an in-flight request may miss a newer server edit.
                // Coalesce those events into one follow-up, only after completion.
                if events.try_recv().is_ok() {
                    while events.try_recv().is_ok() {}
                    delay = Duration::ZERO;
                }
            }
        });
        (wake, task)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    use topo_core::{
        Error, Op, Remote, Workspace,
        store::MemoryRemote,
        wire::{ApplyResult, Snapshot},
    };

    #[derive(Default)]
    struct CountingRemote {
        inner: MemoryRemote,
        calls: AtomicUsize,
        offline: AtomicBool,
    }
    impl Remote for CountingRemote {
        fn fetch(&self, known: Option<u64>) -> Result<Option<Snapshot>, Error> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.offline.load(Ordering::SeqCst) {
                Err(Error::Remote(std::io::Error::other("offline").into()))
            } else {
                self.inner.fetch(known)
            }
        }
        fn apply(&self, ops: &[Op]) -> Result<ApplyResult, Error> {
            self.inner.apply(ops)
        }
    }
    fn calls(remote: &CountingRemote) -> usize {
        remote.calls.load(Ordering::SeqCst)
    }

    #[gpui::test]
    fn quiet_polling_slows_down_and_skips_persistence_work(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let remote = Arc::new(CountingRemote::default());
        let ws = Workspace::open_remote(dir.path().to_owned(), remote.clone()).unwrap();
        let executor = cx.executor();
        let (app, cx) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        let initial = calls(&remote);
        for (wait, extra) in [(5, 1), (10, 2), (20, 3), (30, 4), (30, 5)] {
            executor.advance_clock(Duration::from_secs(wait));
            cx.run_until_parked();
            assert_eq!(calls(&remote), initial + extra);
        }
        // A persistence read/write already confirms the graph; polling must not overlap it.
        app.update(cx, |app, _| app.persistence.set_busy_for_test(true));
        executor.advance_clock(MAX_IDLE);
        cx.run_until_parked();
        assert_eq!(calls(&remote), initial + 5);
        app.update(cx, |app, _| app.persistence.set_busy_for_test(false));
        executor.advance_clock(REMOTE_POLL);
        cx.run_until_parked();
        assert_eq!(calls(&remote), initial + 6);
        remote
            .apply(&[serde_json::from_value(serde_json::json!({"op":"add","id":"new","title":"new"})).unwrap()])
            .unwrap();
        executor.advance_clock(REMOTE_POLL * 2);
        cx.run_until_parked();
        assert_eq!(calls(&remote), initial + 7);
        executor.advance_clock(REMOTE_POLL);
        cx.run_until_parked();
        assert_eq!(calls(&remote), initial + 8);
    }

    #[gpui::test]
    fn inactive_windows_make_zero_requests_and_resume_once(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let remote = Arc::new(CountingRemote::default());
        let ws = Workspace::open_remote(dir.path().to_owned(), remote.clone()).unwrap();
        let executor = cx.executor();
        let (app, cx) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
        cx.run_until_parked();
        assert_eq!(calls(&remote), 1);
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        assert_eq!(calls(&remote), 2);
        // Unchanged snapshots must not notify the editor.
        let notices = Arc::new(AtomicUsize::new(0));
        let observed = notices.clone();
        let _subscription = cx.update(|_, cx| {
            cx.observe(&app, move |_, _| {
                observed.fetch_add(1, Ordering::SeqCst);
            })
        });
        executor.advance_clock(REMOTE_POLL);
        cx.run_until_parked();
        assert_eq!(calls(&remote), 3);
        assert_eq!(notices.load(Ordering::SeqCst), 0);
        cx.deactivate_window();
        executor.advance_clock(Duration::from_secs(120));
        cx.run_until_parked();
        assert_eq!(calls(&remote), 3);
        let op: Op =
            serde_json::from_value(serde_json::json!({"op":"add", "id":"remote", "title":"from another writer"}))
                .unwrap();
        remote.apply(&[op]).unwrap();
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        assert_eq!(calls(&remote), 4);
        assert!(app.read_with(cx, |app, _| app.graph().get(&topo_core::NodeId("remote".into())).is_some()));
        // Input focus stays inside the active window, so it does not pause polling.
        app.update_in(cx, |app, window, cx| {
            app.select(Some(topo_core::NodeId("remote".into())), false);
            app.start_notes(window, cx);
        });
        executor.advance_clock(REMOTE_POLL);
        cx.run_until_parked();
        assert_eq!(calls(&remote), 5);
        app.update(cx, |app, _| {
            app.set_poll_active(false);
            app.set_poll_active(true);
            app.set_poll_active(false);
            app.set_poll_active(true);
        });
        cx.run_until_parked();
        assert_eq!(calls(&remote), 6);
        app.update(cx, |app, _| app.set_poll_visible(false));
        cx.run_until_parked();
        executor.advance_clock(Duration::from_secs(120));
        cx.run_until_parked();
        assert_eq!(calls(&remote), 6);
        app.update(cx, |app, _| app.set_poll_visible(true));
        cx.run_until_parked();
        assert_eq!(calls(&remote), 7);
        app.update(cx, |app, _| app.retire());
        executor.advance_clock(Duration::from_secs(120));
        cx.run_until_parked();
        assert_eq!(calls(&remote), 7);
        app.update(cx, |app, cx| {
            let known = app.ws.remote().unwrap().1;
            app.fetched(Ok(Some(Snapshot { version: known + 100, nodes: vec![] })), cx);
            assert_eq!(app.ws.remote().unwrap().1, known);
            assert!(app.graph().get(&topo_core::NodeId("remote".into())).is_some());
        });
    }

    #[gpui::test]
    fn offline_retries_back_off_and_activation_retries_immediately(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let remote = Arc::new(CountingRemote::default());
        let ws = Workspace::open_remote(dir.path().to_owned(), remote.clone()).unwrap();
        let executor = cx.executor();
        let (app, cx) = cx.add_window_view(|window, cx| TopoApp::new(ws, window, cx).unwrap());
        cx.run_until_parked();
        remote.offline.store(true, Ordering::SeqCst);
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        assert_eq!(calls(&remote), 2);
        let serial = app.read_with(cx, |app, _| app.toast_serial);
        for (wait, expected) in [(5, 2), (5, 3), (20, 4), (40, 5), (60, 6), (60, 7)] {
            executor.advance_clock(Duration::from_secs(wait));
            cx.run_until_parked();
            assert_eq!(calls(&remote), expected);
            assert_eq!(app.read_with(cx, |app, _| app.toast_serial), serial);
        }
        cx.deactivate_window();
        executor.advance_clock(Duration::from_secs(120));
        cx.run_until_parked();
        assert_eq!(calls(&remote), 7);
        remote.offline.store(false, Ordering::SeqCst);
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        assert_eq!(calls(&remote), 8);
        executor.advance_clock(REMOTE_POLL);
        cx.run_until_parked();
        assert_eq!(calls(&remote), 9);
        remote.offline.store(true, Ordering::SeqCst);
        executor.advance_clock(REMOTE_POLL * 2);
        cx.run_until_parked();
        assert_eq!(calls(&remote), 10);
        assert_eq!(app.read_with(cx, |app, _| app.toast_serial), serial + 1);
    }
}
