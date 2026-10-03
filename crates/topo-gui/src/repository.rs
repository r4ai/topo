//! The window owns one editor at a time. Preparing a new target never mutates
//! the current editor. Replacing it drops its watcher, scheduler and observers;
//! asynchronous callbacks hold weak entities and cannot affect the new editor.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use gpui::{
    Context, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent, PathPromptOptions, Subscription, Task, Window,
    actions, div, prelude::*, rgb,
};
use topo_core::Workspace;

use crate::{CloseWindow, TopoApp, config::UserConfig, theme};

#[path = "repository_view.rs"]
mod view;

actions!(topo, [OpenRepository]);
impl EventEmitter<OpenRepository> for TopoApp {}

pub(crate) enum Target {
    Workspace(Workspace),
    Uninitialized(PathBuf),
}

pub(crate) struct Selection {
    path: PathBuf,
    exact_workspace: bool,
}

impl From<PathBuf> for Selection {
    fn from(path: PathBuf) -> Self {
        Self { path, exact_workspace: false }
    }
}

/// TOPO_DIR > positional argument > last successful target > cwd.
/// TOPO_DIR and history are exact workspace directories, even with custom names.
pub(crate) fn startup_target(
    dir: Option<PathBuf>,
    arg: Option<PathBuf>,
    config: &UserConfig,
    cwd: PathBuf,
) -> Selection {
    if let Some(path) = dir {
        return Selection { path, exact_workspace: true };
    }
    if let Some(start) = arg {
        return Workspace::find_dir(&start).unwrap_or(start).into();
    }
    if let Some(path) = config.recent_workspaces.first() {
        return Selection { path: path.clone(), exact_workspace: true };
    }
    Workspace::find_dir(&cwd).unwrap_or(cwd).into()
}

/// Selected folders use their own `.topo`, never an unrelated ancestor's.
/// Startup cwd alone retains CLI-style ancestor discovery (resolved by `new`).
pub(crate) fn load_target(path: &Path, initialize: bool) -> Result<Target> {
    let path = path.canonicalize().with_context(|| format!("Cannot open {}", path.display()))?;
    anyhow::ensure!(path.is_dir(), "Select a repository or folder");
    let dir = if path.file_name().is_some_and(|name| name == ".topo") { path.clone() } else { path.join(".topo") };
    if dir.is_dir() {
        Ok(Target::Workspace(topo_cloud::open(dir.canonicalize()?)?))
    } else if initialize {
        Ok(Target::Workspace(Workspace::init(&path)?))
    } else {
        Ok(Target::Uninitialized(path))
    }
}

pub(crate) struct RepositoryWindow {
    pub(crate) editor: Option<Entity<TopoApp>>,
    pub(crate) config: UserConfig,
    focus: FocusHandle,
    chooser: bool,
    pending: Option<PathBuf>,
    error: Option<String>,
    loading: bool,
    load_generation: u64,
    /// Cancels preparation, and serializes selection / initialization requests.
    load_task: Option<Task<()>>,
    subscription: Option<Subscription>,
}

impl RepositoryWindow {
    pub(crate) fn new(
        start: impl Into<Selection>,
        config: UserConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let mut app = Self {
            editor: None,
            config,
            focus,
            chooser: true,
            pending: None,
            error: None,
            loading: false,
            load_generation: 0,
            load_task: None,
            subscription: None,
        };
        app.open_selection(start.into(), false, window, cx);
        app
    }

    fn show_chooser(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.chooser = true;
        if let Some(editor) = &self.editor {
            editor.update(cx, |app, _| app.set_poll_visible(false));
        }
        self.pending = None;
        self.error = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.load_generation += 1;
        self.load_task.take();
        self.loading = false;
        if self.editor.is_none() {
            self.pending = None;
            self.error = None;
            cx.notify();
            return;
        }
        self.chooser = false;
        self.pending = None;
        self.error = None;
        if let Some(editor) = &self.editor {
            editor.update(cx, |app, cx| {
                app.set_poll_visible(true);
                // Restore the exact draft that was open before choosing.
                let focus = if app.notes.is_some() {
                    app.notes_input.focus_handle(cx)
                } else if app.inline.is_some() {
                    app.combo.focus_handle(cx)
                } else if app.prompt.is_some() {
                    app.palette.focus_handle(cx)
                } else {
                    app.focus.clone()
                };
                window.focus(&focus, cx);
            });
        }
        cx.notify();
    }

    fn choose_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open repository".into()),
        });
        self.loading = true;
        self.load_generation += 1;
        let generation = self.load_generation;
        cx.notify();
        self.load_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = paths.await;
            let _ = this.update_in(cx, |app, window, cx| {
                if app.load_generation != generation {
                    return;
                }
                app.loading = false;
                match result {
                    Ok(Ok(Some(paths))) if !paths.is_empty() => app.open_path(paths[0].clone(), false, window, cx),
                    Ok(Ok(_)) => app.cancel(window, cx),
                    Ok(Err(e)) => app.error = Some(e.to_string()),
                    Err(e) => app.error = Some(e.to_string()),
                }
                cx.notify();
            });
        }));
    }

    pub(crate) fn open_path(&mut self, path: PathBuf, initialize: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.open_selection(path.into(), initialize, window, cx);
    }

    fn open_selection(&mut self, selection: Selection, initialize: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.load_generation += 1;
        let generation = self.load_generation;
        self.chooser = true;
        if let Some(editor) = &self.editor {
            editor.update(cx, |app, _| app.set_poll_visible(false));
        }
        self.pending = None;
        self.error = None;
        window.focus(&self.focus, cx);
        let task = cx.background_executor().spawn(async move {
            let result = if selection.exact_workspace {
                selection
                    .path
                    .canonicalize()
                    .map_err(anyhow::Error::from)
                    .and_then(|dir| Ok(Target::Workspace(topo_cloud::open(dir)?)))
            } else {
                load_target(&selection.path, initialize)
            };
            result.map_err(|e| e.to_string())
        });
        self.load_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |app, window, cx| {
                if app.load_generation != generation {
                    return;
                }
                app.loading = false;
                match result {
                    Ok(Target::Workspace(ws)) => {
                        if let Err(e) = app.install(ws, window, cx) {
                            app.error = Some(e.to_string());
                        }
                    }
                    Ok(Target::Uninitialized(path)) => app.pending = Some(path),
                    Err(e) => app.error = Some(e),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(crate) fn install(&mut self, ws: Workspace, window: &mut Window, cx: &mut Context<Self>) -> Result<()> {
        let dir = ws.dir().canonicalize()?;
        if self.editor.as_ref().is_some_and(|editor| editor.read(cx).ws.dir() == dir) {
            self.cancel(window, cx);
            return Ok(());
        }
        // Preserve the user's width; all graph-specific state belongs to the new entity.
        let width =
            self.editor.as_ref().and_then(|editor| editor.read(cx).inspector_width).or(self.config.inspector_width);
        let source = if ws.remote().is_none() { Some(TopoApp::file_watcher(&ws)?) } else { None };
        let new = cx.new(|cx| TopoApp::with_source(ws, width, source, window, cx));
        self.subscription = Some(cx.subscribe_in(&new, window, |app, _, _: &OpenRepository, window, cx| {
            app.show_chooser(window, cx);
        }));
        window.set_window_title(&format!("topo — {}", dir.parent().unwrap_or(&dir).display()));
        if let Some(old) = &self.editor {
            old.update(cx, |app, _| app.retire());
        }
        self.editor = Some(new);
        self.config.inspector_width = width;
        self.config.remember(dir);
        #[cfg(not(test))]
        if let Err(e) = self.config.save() {
            self.editor
                .as_ref()
                .unwrap()
                .update(cx, |app, cx| app.toast(format!("Could not save preferences: {e}"), true, cx));
        }
        self.chooser = false;
        self.pending = None;
        Ok(())
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" {
            self.cancel(window, cx);
        }
    }
}

impl Render for RepositoryWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chooser = self.chooser || self.editor.is_none();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::CANVAS))
            .text_color(rgb(theme::TEXT))
            .font_family(".SystemUIFont")
            .on_action(cx.listener(|app, _: &OpenRepository, window, cx| app.show_chooser(window, cx)))
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
            .when(!chooser, |d| d.child(self.editor.as_ref().unwrap().clone()))
            .when(chooser, |d| {
                d.track_focus(&self.focus).on_key_down(cx.listener(Self::key_down)).child(self.chooser_view(window, cx))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use topo_core::{Graph, Kind, Node, NodeId, Status};

    fn workspace(path: &Path, title: &str) -> Workspace {
        let mut ws = Workspace::init(path).unwrap();
        ws.graph = Graph::from_nodes([Node::new(NodeId("same".into()), Kind::Task, title.into())]).unwrap();
        ws.save().unwrap();
        ws
    }

    #[test]
    fn startup_precedence_and_exact_folder_selection() {
        let dir = tempfile::tempdir().unwrap();
        workspace(dir.path(), "parent");
        let child = dir.path().join("child");
        std::fs::create_dir(&child).unwrap();
        let config = UserConfig { recent_workspaces: vec!["recent/.topo".into()], ..Default::default() };
        assert_eq!(
            startup_target(Some("override/.topo".into()), Some("argument".into()), &config, child.clone()).path,
            PathBuf::from("override/.topo")
        );
        assert_eq!(
            startup_target(None, Some("argument".into()), &config, child.clone()).path,
            PathBuf::from("argument")
        );
        assert_eq!(startup_target(None, None, &config, child.clone()).path, PathBuf::from("recent/.topo"));
        assert_eq!(startup_target(None, None, &UserConfig::default(), child.clone()).path, dir.path().join(".topo"));
        assert!(matches!(load_target(&child, false).unwrap(), Target::Uninitialized(_)));
        assert!(!child.join(".topo").exists());
        assert!(matches!(load_target(&child, true).unwrap(), Target::Workspace(_)));
        assert!(load_target(&child.join("missing"), false).is_err());
    }

    #[gpui::test]
    fn switching_resets_editor_and_cancels_old_sources(cx: &mut TestAppContext) {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        workspace(first.path(), "first");
        workspace(second.path(), "second");
        let (shell, cx) = cx.add_window_view(|window, cx| {
            RepositoryWindow::new(first.path().to_owned(), UserConfig::default(), window, cx)
        });
        cx.run_until_parked();
        let old = shell.read_with(cx, |app, _| app.editor.clone().unwrap());
        old.update(cx, |app, cx| {
            app.mutate(cx, |g| g.set_status(&NodeId("same".into()), Status::Done));
            app.selected = Some(NodeId("same".into()));
            app.selected_nodes.insert(NodeId("same".into()));
        });
        shell.update_in(cx, |app, window, cx| app.open_path(second.path().to_owned(), false, window, cx));
        cx.run_until_parked();
        let new = shell.read_with(cx, |app, _| app.editor.clone().unwrap());
        assert_ne!(old.entity_id(), new.entity_id());
        old.read_with(cx, |app, _| {
            assert!(app.retired);
            assert!(app._watcher.is_none());
            assert!(app._sync_task.is_none());
            assert!(app._subscriptions.is_empty());
        });
        new.read_with(cx, |app, _| {
            assert!(app.undo.is_empty() && app.redo.is_empty());
            assert!(app.selected_nodes.is_empty());
            assert!(app.prompt.is_none() && app.notes.is_none() && app.inline.is_none());
            assert_eq!(app.graph().get(&NodeId("same".into())).unwrap().title, "second");
            assert_eq!(app.graph().get(&NodeId("same".into())).unwrap().status, Status::Todo);
        });
        new.update(cx, |app, cx| {
            app.mutate(cx, |g| g.set_status(&NodeId("same".into()), Status::Doing));
        });
        assert_eq!(
            Workspace::discover(first.path()).unwrap().graph.get(&NodeId("same".into())).unwrap().status,
            Status::Done
        );
        assert_eq!(
            Workspace::discover(second.path()).unwrap().graph.get(&NodeId("same".into())).unwrap().status,
            Status::Doing
        );
        shell.read_with(cx, |app, _| {
            assert_eq!(
                app.config.recent_workspaces,
                [
                    second.path().join(".topo").canonicalize().unwrap(),
                    first.path().join(".topo").canonicalize().unwrap()
                ]
            );
        });
    }

    #[gpui::test]
    fn picker_cancel_failures_and_same_target_preserve_drafts(cx: &mut TestAppContext) {
        let first = tempfile::tempdir().unwrap();
        let uninitialized = tempfile::tempdir().unwrap();
        workspace(first.path(), "first");
        let (shell, cx) = cx.add_window_view(|window, cx| {
            RepositoryWindow::new(first.path().to_owned(), UserConfig::default(), window, cx)
        });
        cx.run_until_parked();
        let editor = shell.read_with(cx, |app, _| app.editor.clone().unwrap());
        editor.update_in(cx, |app, window, cx| {
            app.select(Some(NodeId("same".into())), false);
            app.start_notes(window, cx);
            app.notes_input.update(cx, |input, cx| input.set_text("unsaved draft", cx));
        });
        shell.update_in(cx, |app, window, cx| {
            app.show_chooser(window, cx);
            app.choose_folder(window, cx);
        });
        cx.simulate_path_prompt_response(|options| {
            assert!(options.directories && !options.files && !options.multiple);
            None
        });
        cx.run_until_parked();
        assert!(!shell.read_with(cx, |app, _| app.chooser));
        editor.read_with(cx, |app, cx| {
            assert!(app.notes.is_some());
            assert_eq!(app.notes_input.read(cx).text(), "unsaved draft");
        });
        shell.update_in(cx, |app, window, cx| app.open_path(first.path().join("missing"), false, window, cx));
        cx.run_until_parked();
        assert!(shell.read_with(cx, |app, _| app.error.is_some()));
        assert_eq!(shell.read_with(cx, |app, _| app.editor.as_ref().unwrap().entity_id()), editor.entity_id());
        shell.update_in(cx, |app, window, cx| app.open_path(first.path().join(".topo"), false, window, cx));
        cx.run_until_parked();
        assert_eq!(shell.read_with(cx, |app, _| app.editor.as_ref().unwrap().entity_id()), editor.entity_id());
        assert!(editor.read_with(cx, |app, _| app.notes.is_some()));
        shell.update_in(cx, |app, window, cx| app.open_path(uninitialized.path().to_owned(), false, window, cx));
        cx.run_until_parked();
        assert!(!uninitialized.path().join(".topo").exists());
        assert!(shell.read_with(cx, |app, _| app.pending.is_some()));
        shell.update_in(cx, |app, window, cx| app.open_path(uninitialized.path().to_owned(), true, window, cx));
        cx.run_until_parked();
        assert!(uninitialized.path().join(".topo/nodes").is_dir());
        assert!(editor.read_with(cx, |app, _| app.retired));
        assert_eq!(Workspace::discover(first.path()).unwrap().graph.get(&NodeId("same".into())).unwrap().body, "");
    }

    #[gpui::test]
    fn no_target_and_unreadable_cloud_link_keep_selection_available(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (shell, cx) = cx.add_window_view(|window, cx| {
            RepositoryWindow::new(dir.path().join("missing"), UserConfig::default(), window, cx)
        });
        cx.run_until_parked();
        assert!(shell.read_with(cx, |app, _| app.editor.is_none() && app.error.is_some()));
        workspace(dir.path(), "local");
        shell.update_in(cx, |app, window, cx| app.open_path(dir.path().to_owned(), false, window, cx));
        cx.run_until_parked();
        let cloud = tempfile::tempdir().unwrap();
        std::fs::create_dir(cloud.path().join(".topo")).unwrap();
        std::fs::write(cloud.path().join(".topo/config.toml"), "[cloud]\nurl = 'invalid-url'\nworkspace = 'id'\n")
            .unwrap();
        let before = shell.read_with(cx, |app, _| app.editor.as_ref().unwrap().entity_id());
        shell.update_in(cx, |app, window, cx| app.open_path(cloud.path().to_owned(), false, window, cx));
        cx.run_until_parked();
        assert!(shell.read_with(cx, |app, _| app.error.is_some()));
        assert_eq!(shell.read_with(cx, |app, _| app.editor.as_ref().unwrap().entity_id()), before);
    }
    #[gpui::test]
    fn custom_workspace_directory_restores_and_cancelled_load_cannot_switch(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        workspace(dir.path(), "custom");
        let custom = dir.path().join("tasks");
        std::fs::rename(dir.path().join(".topo"), &custom).unwrap();
        let config = UserConfig { inspector_width: Some(412.), recent_workspaces: vec![custom.clone()] };
        let start = startup_target(None, None, &config, PathBuf::from("/missing-cwd"));
        let (shell, cx) = cx.add_window_view(|window, cx| RepositoryWindow::new(start, config, window, cx));
        cx.run_until_parked();
        let before = shell.read_with(cx, |app, _| app.editor.clone().unwrap());
        assert_eq!(before.read_with(cx, |app, _| app.ws.dir().to_owned()), custom.canonicalize().unwrap());
        assert_eq!(before.read_with(cx, |app, _| app.inspector_width), Some(412.));
        shell.update_in(cx, |app, window, cx| {
            app.show_chooser(window, cx);
            app.choose_folder(window, cx);
            app.cancel(window, cx);
        });
        let next = tempfile::tempdir().unwrap();
        workspace(next.path(), "next");
        cx.simulate_path_prompt_response(|_| Some(vec![next.path().to_owned()]));
        cx.run_until_parked();
        assert_eq!(shell.read_with(cx, |app, _| app.editor.as_ref().unwrap().entity_id()), before.entity_id());
        assert!(!shell.read_with(cx, |app, _| app.loading || app.chooser));
    }

    #[gpui::test]
    fn replacing_a_cloud_editor_rejects_old_snapshots(cx: &mut TestAppContext) {
        use std::sync::Arc;
        use topo_core::{Remote, store::MemoryRemote};
        let dir = tempfile::tempdir().unwrap();
        workspace(dir.path(), "local");
        let remote = Arc::new(MemoryRemote::default());
        let add = serde_json::from_value(serde_json::json!({ "op": "add", "id": "same", "title": "cloud" })).unwrap();
        remote.apply(&[add]).unwrap();
        let cloud = tempfile::tempdir().unwrap();
        let cloud_dir = Workspace::init(cloud.path()).unwrap().dir().to_owned();
        let ws = Workspace::open_remote(cloud_dir, remote.clone()).unwrap();
        let (shell, cx) = cx.add_window_view(|window, cx| {
            RepositoryWindow::new(dir.path().to_owned(), UserConfig::default(), window, cx)
        });
        cx.run_until_parked();
        shell.update_in(cx, |app, window, cx| app.install(ws, window, cx).unwrap());
        cx.run_until_parked();
        let old = shell.read_with(cx, |app, _| app.editor.clone().unwrap());
        assert!(old.read_with(cx, |app, _| app.ws.remote().is_some()));
        shell.update_in(cx, |app, window, cx| app.open_path(dir.path().to_owned(), false, window, cx));
        cx.run_until_parked();
        let new = shell.read_with(cx, |app, _| app.editor.clone().unwrap());
        old.update(cx, |app, cx| {
            app.fetched(Ok(Some(topo_core::wire::Snapshot { version: 999, nodes: vec![] })), cx);
            assert!(app.retired && app._sync_task.is_none());
            assert_eq!(app.graph().get(&NodeId("same".into())).unwrap().title, "cloud");
        });
        assert_eq!(new.read_with(cx, |app, _| app.graph().get(&NodeId("same".into())).unwrap().title.clone()), "local");
    }
    #[gpui::test]
    fn toolbar_and_recent_repository_controls_switch_the_window(cx: &mut TestAppContext) {
        use gpui::{Modifiers, MouseButton, VisualTestContext, px, size};
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        workspace(first.path(), "first");
        workspace(second.path(), "second");
        let config = UserConfig {
            recent_workspaces: vec![second.path().join(".topo").canonicalize().unwrap()],
            ..Default::default()
        };
        let (shell, cx) =
            cx.add_window_view(|window, cx| RepositoryWindow::new(first.path().to_owned(), config, window, cx));
        cx.simulate_resize(size(px(720.), px(480.)));
        cx.run_until_parked();
        let click = |cx: &mut VisualTestContext, name: &'static str| {
            let bounds = cx.debug_bounds(name).expect("control is visible");
            assert!(bounds.left() >= px(0.) && bounds.right() <= px(720.));
            cx.simulate_mouse_down(bounds.center(), MouseButton::Left, Modifiers::none());
            cx.simulate_mouse_up(bounds.center(), MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
        };
        click(cx, "repository");
        assert!(cx.debug_bounds("open-folder").is_some());
        click(cx, "recent-repository-1");
        assert_eq!(
            shell.read_with(cx, |app, cx| app.editor.as_ref().unwrap().read(cx).ws.dir().to_owned()),
            second.path().join(".topo").canonicalize().unwrap()
        );
        assert!(cx.window_title().unwrap().contains(&second.path().canonicalize().unwrap().display().to_string()));
        cx.dispatch_action(OpenRepository);
        cx.run_until_parked();
        assert!(cx.debug_bounds("open-folder").is_some());
        cx.simulate_keystrokes("escape");
        assert!(!shell.read_with(cx, |app, _| app.chooser));
    }
    #[gpui::test]
    fn a_full_history_scrolls_without_shrinking_rows_or_hiding_cancel(cx: &mut TestAppContext) {
        use gpui::{Modifiers, MouseButton, ScrollDelta, ScrollWheelEvent, point, px, size};
        let folders: Vec<_> = (0..10)
            .map(|index| {
                let folder = tempfile::tempdir().unwrap();
                workspace(folder.path(), &format!("repository {index}"));
                folder
            })
            .collect();
        let config = UserConfig {
            recent_workspaces: folders.iter().map(|d| d.path().join(".topo").canonicalize().unwrap()).collect(),
            ..Default::default()
        };
        let (shell, cx) =
            cx.add_window_view(|window, cx| RepositoryWindow::new(folders[0].path().to_owned(), config, window, cx));
        cx.simulate_resize(size(px(720.), px(480.)));
        cx.run_until_parked();
        cx.dispatch_action(OpenRepository);
        cx.run_until_parked();
        let first = cx.debug_bounds("recent-repository-1").unwrap();
        assert!(first.size.height >= px(64.));
        let cancel = cx.debug_bounds("cancel-repository").unwrap();
        assert!(cancel.top() >= px(0.) && cancel.bottom() <= px(480.));
        cx.simulate_event(ScrollWheelEvent {
            position: first.center(),
            delta: ScrollDelta::Pixels(point(px(0.), px(-10000.))),
            ..Default::default()
        });
        cx.run_until_parked();
        let last = cx.debug_bounds("recent-repository-9").unwrap();
        assert!(last.size.height >= px(64.));
        assert!(last.top() >= px(100.) && last.bottom() < cancel.top());
        let position = last.center();
        cx.simulate_mouse_down(position, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            shell.read_with(cx, |app, cx| app.editor.as_ref().unwrap().read(cx).ws.dir().to_owned()),
            folders[9].path().join(".topo").canonicalize().unwrap()
        );
    }
}
