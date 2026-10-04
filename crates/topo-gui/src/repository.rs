//! The window owns one editor at a time. Preparing a new target never mutates
//! the current editor. Replacing it drops its watcher, scheduler and observers;
//! asynchronous callbacks hold weak entities and cannot affect the new editor.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use gpui::{
    App, Context, Entity, EventEmitter, Focusable, KeyBinding, KeyDownEvent, PathPromptOptions, ScrollHandle,
    Subscription, Task, Window, actions, div, point, prelude::*, px,
};
use topo_core::Workspace;

use crate::text_input::{InputEvent, TextInput};
use crate::{CloseWindow, Quit, TopoApp, chrome, config::UserConfig, layout, theme};

#[path = "repository_view.rs"]
mod view;

actions!(topo, [OpenRepository, ForgetRepository]);
impl EventEmitter<OpenRepository> for TopoApp {}

/// Placeholder of the switcher's filter field.
const FILTER_PLACEHOLDER: &str = "Switch repository — type to filter or paste a path";

/// Binds the keys of the repository switcher; the filter field would otherwise take ⌘⌫ as "delete to start".
pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-backspace", ForgetRepository, Some("RepositorySwitcher > TextInput")),
        KeyBinding::new("ctrl-backspace", ForgetRepository, Some("RepositorySwitcher > TextInput")),
    ]);
}

/// What the switcher knows about a recent workspace, read once when it opens.
#[derive(Clone, Copy)]
struct RecentInfo {
    exists: bool,
    cloud: bool,
}

impl RecentInfo {
    fn probe(dir: &Path) -> Self {
        let exists = dir.is_dir();
        let cloud = exists && matches!(topo_cloud::config::load(dir), Ok(Some(_)));
        Self { exists, cloud }
    }
}

pub(crate) enum Target {
    Workspace(Workspace),
    Uninitialized(PathBuf),
}

pub(crate) struct Selection {
    path: PathBuf,
    exact_workspace: bool,
}

impl Selection {
    /// The folder the selection stands for: the parent of a workspace directory.
    fn folder(&self) -> &Path {
        match self.exact_workspace || self.path.file_name().is_some_and(|name| name == ".topo") {
            true => folder_of(&self.path),
            false => &self.path,
        }
    }
}

/// The project folder of a workspace directory.
fn folder_of(dir: &Path) -> &Path {
    dir.parent().unwrap_or(dir)
}

/// The last component of `folder`, or all of it for a root.
fn folder_name(folder: &Path) -> String {
    folder.file_name().map_or_else(|| folder.display().to_string(), |name| name.to_string_lossy().into_owned())
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
    /// The switcher's filter field, which has the focus while the switcher is open.
    filter: Entity<TextInput>,
    /// The row Enter opens, an index into the visible rows.
    highlight: usize,
    /// The directory the filter text names, when it is a path to one.
    typed: Option<PathBuf>,
    recent_info: HashMap<PathBuf, RecentInfo>,
    list_scroll: ScrollHandle,
    /// Whether changes to `config` are written to the user's file.
    persistent: bool,
    chooser: bool,
    pending: Option<PathBuf>,
    error: Option<String>,
    loading: bool,
    /// The name of the folder being opened, for the status line.
    opening: Option<String>,
    load_generation: u64,
    /// Cancels preparation, and serializes selection / initialization requests.
    load_task: Option<Task<()>>,
    subscription: Option<Subscription>,
    _filter_subscription: Subscription,
}

impl RepositoryWindow {
    pub(crate) fn new(
        start: impl Into<Selection>,
        config: UserConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::start(start.into(), config, cfg!(not(test)), window, cx)
    }

    /// A window for a capture: it never writes the user's configuration.
    #[cfg(feature = "screenshot")]
    pub(crate) fn capture(start: Selection, config: UserConfig, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::start(start, config, false, window, cx)
    }

    fn start(
        start: Selection,
        config: UserConfig,
        persistent: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let filter = cx.new(TextInput::new);
        filter.update(cx, |input, cx| input.reset("", FILTER_PLACEHOLDER, cx));
        window.focus(&filter.focus_handle(cx), cx);
        let filter_subscription = cx.subscribe_in(&filter, window, Self::on_filter);
        let mut app = Self {
            editor: None,
            config,
            filter,
            highlight: 0,
            typed: None,
            recent_info: HashMap::new(),
            list_scroll: ScrollHandle::new(),
            persistent,
            chooser: true,
            pending: None,
            error: None,
            loading: false,
            opening: None,
            load_generation: 0,
            load_task: None,
            subscription: None,
            _filter_subscription: filter_subscription,
        };
        app.refresh_recents();
        // `System` follows the OS: redraw when its appearance flips.
        cx.observe_window_appearance(window, |_, window, cx| {
            if theme::mode() == theme::ThemeMode::System {
                theme::apply(theme::ThemeMode::System, theme::monotone(), window.appearance());
                cx.refresh_windows();
            }
        })
        .detach();
        app.open_selection(start, false, window, cx);
        app
    }

    fn show_chooser(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        if let Some(editor) = &self.editor
            && !editor.update(cx, |app, cx| app.can_leave(cx))
        {
            return;
        }
        self.chooser = true;
        if let Some(editor) = &self.editor {
            editor.update(cx, |app, _| app.set_poll_visible(false));
        }
        self.pending = None;
        self.error = None;
        self.refresh_recents();
        self.highlight = 0;
        self.typed = None;
        self.filter.update(cx, |input, cx| input.set_text("", cx));
        self.focus_filter(window, cx);
        cx.notify();
    }

    /// Opens the switcher over the editor for a capture, or bare when no workspace loaded.
    #[cfg(feature = "screenshot")]
    pub(crate) fn present(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.editor {
            Some(_) => self.show_chooser(window, cx),
            None => self.cancel(window, cx),
        }
    }

    fn focus_filter(&self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.switcher_open() {
            return;
        }
        window.focus(&self.filter.focus_handle(cx), cx);
    }

    /// Reads once what the rows of the recent workspaces show: whether the folder is still there, and a cloud link.
    fn refresh_recents(&mut self) {
        self.recent_info =
            self.config.recent_workspaces.iter().map(|dir| (dir.clone(), RecentInfo::probe(dir))).collect();
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
        self.opening = None;
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
                app.focus_filter(window, cx);
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
        if let Some(editor) = &self.editor
            && !editor.update(cx, |app, cx| app.can_leave(cx))
        {
            return;
        }
        self.loading = true;
        self.opening = Some(folder_name(selection.folder()));
        self.load_generation += 1;
        let generation = self.load_generation;
        self.chooser = true;
        if let Some(editor) = &self.editor {
            editor.update(cx, |app, _| app.set_poll_visible(false));
        }
        self.pending = None;
        self.error = None;
        self.focus_filter(window, cx);
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
                app.focus_filter(window, cx);
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
        let source = TopoApp::updates(&ws)?;
        // The toggles carry over to the next workspace; its collapsed groups are its own.
        let view = match &self.editor {
            Some(editor) => layout::View { collapsed: Default::default(), ..editor.read(cx).view.clone() },
            None => layout::View {
                hide_completed: self.config.hide_completed,
                group_by_tag: self.config.group_by_tag,
                ..Default::default()
            },
        };
        let new = cx.new(|cx| TopoApp::with_source(ws, width, view.clone(), source, window, cx));
        self.subscription = Some(cx.subscribe_in(&new, window, |app, _, _: &OpenRepository, window, cx| {
            app.show_chooser(window, cx);
        }));
        window.set_window_title(&format!("topo — {}", dir.parent().unwrap_or(&dir).display()));
        if let Some(old) = &self.editor {
            old.update(cx, |app, _| app.retire());
        }
        self.editor = Some(new);
        self.config.inspector_width = width;
        (self.config.hide_completed, self.config.group_by_tag) = (view.hide_completed, view.group_by_tag);
        self.config.remember(dir);
        self.save_config(cx);
        self.chooser = false;
        self.pending = None;
        Ok(())
    }

    /// Writes the configuration, naming a failure in the editor's toast or, with no editor, in the switcher.
    fn save_config(&mut self, cx: &mut Context<Self>) {
        // Another window may have changed the mode since this copy was loaded.
        (self.config.theme, self.config.monotone) = (theme::mode(), theme::monotone());
        if !self.persistent {
            return;
        }
        if let Err(e) = self.config.save() {
            let message = format!("Could not save preferences: {e}");
            match &self.editor {
                Some(editor) => editor.update(cx, |app, cx| app.toast(message, true, cx)),
                None => self.error = Some(message),
            }
        }
    }

    /// The directory of the workspace in the editor.
    fn current_dir(&self, cx: &App) -> Option<PathBuf> {
        self.editor.as_ref().map(|editor| editor.read(cx).ws.dir().to_owned())
    }

    /// Whether the switcher is showing, over the editor or alone.
    fn switcher_open(&self) -> bool {
        self.chooser || self.editor.is_none()
    }

    fn on_filter(&mut self, _: &Entity<TextInput>, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Changed => {
                self.typed = typed_directory(self.filter.read(cx).text(), dirs::home_dir().as_deref());
                self.highlight = 0;
                self.list_scroll.set_offset(point(px(0.), px(0.)));
                cx.notify();
            }
            InputEvent::Up => self.move_highlight(-1, cx),
            InputEvent::Down => self.move_highlight(1, cx),
            InputEvent::Submit => self.submit(window, cx),
            InputEvent::Cancel => self.escape(window, cx),
            InputEvent::Next | InputEvent::Previous | InputEvent::BackspaceEmpty => {}
        }
    }

    /// Moves the highlight `by` rows, wrapping at both ends.
    fn move_highlight(&mut self, by: isize, cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        if rows.is_empty() {
            return;
        }
        let at = self.highlight.min(rows.len() - 1) as isize;
        self.highlight = (at + by).rem_euclid(rows.len() as isize) as usize;
        self.list_scroll.scroll_to_item(view::slot(&rows, self.highlight));
        cx.notify();
    }

    /// Enter: initializes the folder the banner offers, or opens the highlighted row.
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        if self.pending.is_some() && self.filter.read(cx).text().is_empty() {
            return self.initialize(window, cx);
        }
        let rows = self.rows(cx);
        if let Some(row) = rows.get(self.highlight.min(rows.len().saturating_sub(1))) {
            self.activate(row, window, cx);
        }
    }

    /// Escape: clears the filter first, then closes the switcher.
    fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.filter.read(cx).text().is_empty() {
            self.cancel(window, cx);
        } else {
            self.filter.update(cx, |input, cx| input.set_text("", cx));
        }
    }

    /// Opens what `row` stands for. A recent folder that is gone opens nothing.
    fn activate(&mut self, row: &view::Row, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        match row {
            view::Row::Open(dir) => self.open_path(dir.clone(), false, window, cx),
            view::Row::Current(_) => self.cancel(window, cx),
            view::Row::Recent(index) => {
                let dir = self.config.recent_workspaces[*index].clone();
                if self.recent_info.get(&dir).is_none_or(|info| info.exists) {
                    self.open_selection(Selection { path: dir, exact_workspace: true }, false, window, cx);
                }
            }
        }
    }

    /// Creates the workspace in the uninitialized folder the banner offers.
    fn initialize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(dir) = self.pending.clone() {
            self.open_path(dir, true, window, cx);
        }
    }

    /// Removes a recent workspace from the history.
    fn forget(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.config.recent_workspaces.len() {
            return;
        }
        let dir = self.config.recent_workspaces.remove(index);
        self.recent_info.remove(&dir);
        self.save_config(cx);
        cx.notify();
    }

    /// ⌘⌫ on a recent row; on any other row it keeps its meaning of deleting the filter text.
    fn forget_highlighted(&mut self, cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        match rows.get(self.highlight.min(rows.len().saturating_sub(1))) {
            Some(view::Row::Recent(index)) => self.forget(*index, cx),
            _ => cx.propagate(),
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let control = keystroke.modifiers.control && !keystroke.modifiers.platform;
        match (keystroke.key.as_str(), control) {
            ("escape", _) => self.escape(window, cx),
            ("up", false) | ("p", true) => self.move_highlight(-1, cx),
            ("down", false) | ("n", true) => self.move_highlight(1, cx),
            ("enter", false) => self.submit(window, cx),
            _ => {}
        }
    }
}

/// The directory a filter text names, when it is an absolute path or one starting with `~` to a folder that exists.
fn typed_directory(text: &str, home: Option<&Path>) -> Option<PathBuf> {
    let text = text.trim();
    let path = match text.strip_prefix('~') {
        Some("") => home?.to_owned(),
        Some(rest) if rest.starts_with(['/', std::path::MAIN_SEPARATOR]) => {
            home?.join(rest.trim_start_matches(['/', std::path::MAIN_SEPARATOR]))
        }
        Some(_) => return None,
        None => PathBuf::from(text),
    };
    // Components drop a trailing separator.
    (path.is_absolute() && path.is_dir()).then(|| path.components().collect())
}

impl Render for RepositoryWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current();
        let switcher = self.switcher_open();
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(theme::window_bg())
            .text_color(t.fg)
            .font_family(".SystemUIFont")
            .on_action(cx.listener(|app, _: &OpenRepository, window, cx| match app.switcher_open() {
                true => app.choose_folder(window, cx),
                false => app.show_chooser(window, cx),
            }))
            .on_action(cx.listener(|app, _: &ForgetRepository, _, cx| app.forget_highlighted(cx)))
            .on_action(cx.listener(|app, _: &CloseWindow, window, cx| {
                if app.editor.as_ref().is_none_or(|editor| editor.update(cx, |app, cx| app.can_leave(cx))) {
                    window.remove_window();
                }
            }))
            .on_action(cx.listener(|app, _: &Quit, _, cx| {
                if app.editor.as_ref().is_none_or(|editor| editor.update(cx, |app, cx| app.can_leave(cx))) {
                    cx.quit();
                }
            }))
            .children(self.editor.clone())
            .when(switcher, |d| d.on_key_down(cx.listener(Self::key_down)).child(self.switcher(window, cx)))
            // Above the scrim, so the titlebar still moves the window.
            .when(switcher && chrome::titlebar_inset(window) > px(0.), |d| {
                d.child(chrome::drag_strip().absolute().top_0().left_0())
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
        cx.run_until_parked();
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
        cx.run_until_parked();
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
        // Changed notes are settled before the chooser covers the editor.
        shell.update_in(cx, |app, window, cx| app.show_chooser(window, cx));
        assert!(!shell.read_with(cx, |app, _| app.chooser));
        assert!(editor.read_with(cx, |app, _| app.notes_ask.is_some()));
        editor.update_in(cx, |app, window, cx| app.answer_notes(None, window, cx));
        editor.read_with(cx, |app, cx| assert_eq!(app.notes_input.read(cx).text(), "unsaved draft"));
        // The editor stays open while the chooser comes and goes, once it has nothing unsaved.
        editor.update_in(cx, |app, _, cx| {
            app.notes_input.update(cx, |input, cx| input.set_text("", cx));
            assert!(!app.notes_dirty(cx));
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
            assert_eq!(app.notes_input.read(cx).text(), "");
        });
        // A draft typed since blocks every way of switching repositories, and survives the question.
        editor.update_in(cx, |app, _, cx| app.notes_input.update(cx, |input, cx| input.set_text("unsaved draft", cx)));
        shell.update_in(cx, |app, window, cx| app.open_path(uninitialized.path().to_owned(), true, window, cx));
        cx.run_until_parked();
        assert!(!uninitialized.path().join(".topo").exists());
        assert!(shell.read_with(cx, |app, _| !app.loading && app.error.is_none() && app.pending.is_none()));
        editor.update_in(cx, |app, window, cx| {
            assert!(app.notes_ask.is_some());
            app.answer_notes(None, window, cx);
            assert!(app.notes.is_some());
            assert_eq!(app.notes_input.read(cx).text(), "unsaved draft");
            app.notes_input.update(cx, |input, cx| input.set_text("", cx));
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
        let config =
            UserConfig { inspector_width: Some(412.), recent_workspaces: vec![custom.clone()], ..Default::default() };
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
    fn installing_a_workspace_keeps_the_chosen_theme_and_monotone_in_the_config(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        workspace(dir.path(), "first");
        let (shell, cx) = cx.add_window_view(|window, cx| {
            RepositoryWindow::new(dir.path().to_owned(), UserConfig::default(), window, cx)
        });
        cx.run_until_parked();
        // The window's copy still says `System` when the user picks another mode.
        theme::apply(theme::ThemeMode::Light, true, gpui::WindowAppearance::Dark);
        let next = tempfile::tempdir().unwrap();
        let ws = workspace(next.path(), "next");
        shell.update_in(cx, |app, window, cx| app.install(ws, window, cx).unwrap());
        assert_eq!(
            shell.read_with(cx, |app, _| (app.config.theme, app.config.monotone)),
            (theme::ThemeMode::Light, true)
        );
        theme::apply(theme::ThemeMode::Dark, false, gpui::WindowAppearance::Dark);
    }

    #[gpui::test]
    fn view_toggles_come_from_the_config_and_follow_the_user_to_the_next_workspace(cx: &mut TestAppContext) {
        let (first, second) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        workspace(first.path(), "first");
        let config = UserConfig { hide_completed: true, ..Default::default() };
        let (shell, cx) =
            cx.add_window_view(|window, cx| RepositoryWindow::new(first.path().to_owned(), config, window, cx));
        cx.run_until_parked();
        let editor = shell.read_with(cx, |app, _| app.editor.clone().unwrap());
        assert!(editor.read_with(cx, |app, _| app.view.hide_completed && !app.view.group_by_tag));
        editor.update(cx, |app, _| {
            app.toggle_group_by_tag();
            app.toggle_group(&layout::Group::Untagged);
        });
        shell.update_in(cx, |app, window, cx| app.install(workspace(second.path(), "second"), window, cx).unwrap());
        let next = shell.read_with(cx, |app, _| app.editor.clone().unwrap());
        let view = next.read_with(cx, |app, _| app.view.clone());
        assert!(view.hide_completed && view.group_by_tag);
        // Collapsed groups belong to one graph's tags.
        assert!(view.collapsed.is_empty());
        let config = shell.read_with(cx, |app, _| app.config.clone());
        assert!(config.hide_completed && config.group_by_tag);
    }
    /// Binds the keys a window of this shell needs.
    fn bind(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::text_input::bind_keys(cx);
            bind_keys(cx);
        });
    }

    /// `cmd-` on macOS, `ctrl-` elsewhere, like the app's standard shortcuts.
    fn platform_keys(keys: &str) -> String {
        if cfg!(target_os = "macos") { keys.to_owned() } else { keys.replace("cmd-", "ctrl-") }
    }

    /// Workspaces called `names` under one canonical parent; returns the parent and their `.topo` paths.
    fn named(names: &[&str]) -> (tempfile::TempDir, Vec<PathBuf>) {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().canonicalize().unwrap();
        let dirs = names
            .iter()
            .map(|name| {
                let folder = root.join(name);
                std::fs::create_dir(&folder).unwrap();
                workspace(&folder, name).dir().canonicalize().unwrap()
            })
            .collect();
        (parent, dirs)
    }

    fn editor_dir(shell: &Entity<RepositoryWindow>, cx: &mut gpui::VisualTestContext) -> PathBuf {
        shell.read_with(cx, |app, cx| app.current_dir(cx).unwrap())
    }

    #[gpui::test]
    fn toolbar_and_recent_repository_controls_switch_the_window(cx: &mut TestAppContext) {
        use gpui::{Modifiers, MouseButton, VisualTestContext, px, size};
        bind(cx);
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
        assert!(cx.debug_bounds("open-folder").is_some() && cx.debug_bounds("repository-switcher").is_some());
        click(cx, "recent-repository-1");
        assert_eq!(
            shell.read_with(cx, |app, cx| app.editor.as_ref().unwrap().read(cx).ws.dir().to_owned()),
            second.path().join(".topo").canonicalize().unwrap()
        );
        assert!(cx.window_title().unwrap().contains(&second.path().canonicalize().unwrap().display().to_string()));
        cx.dispatch_action(OpenRepository);
        cx.run_until_parked();
        assert!(cx.debug_bounds("open-folder").is_some());
        // Escape closes the switcher: the old Cancel button is gone.
        cx.simulate_keystrokes("escape");
        assert!(!shell.read_with(cx, |app, _| app.chooser));
        assert!(cx.debug_bounds("repository-switcher").is_none());
        // A click on the scrim closes it too.
        cx.dispatch_action(OpenRepository);
        cx.run_until_parked();
        cx.simulate_mouse_down(gpui::point(px(5.), px(300.)), MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(gpui::point(px(5.), px(300.)), MouseButton::Left, Modifiers::none());
        assert!(!shell.read_with(cx, |app, _| app.chooser));
    }

    #[gpui::test]
    fn a_full_history_scrolls_inside_a_card_that_fits_the_window(cx: &mut TestAppContext) {
        use gpui::{Modifiers, MouseButton, ScrollDelta, ScrollWheelEvent, point, px, size};
        bind(cx);
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
        let card = cx.debug_bounds("repository-switcher").unwrap();
        assert!(card.left() >= px(0.) && card.right() <= px(720.) && card.top() >= px(0.) && card.bottom() <= px(480.));
        let first = cx.debug_bounds("recent-repository-1").unwrap();
        assert_eq!(first.size.height, px(44.));
        // The footer stays visible while the list scrolls.
        let footer = cx.debug_bounds("open-folder").unwrap();
        assert!(footer.bottom() <= card.bottom());
        cx.simulate_event(ScrollWheelEvent {
            position: first.center(),
            delta: ScrollDelta::Pixels(point(px(0.), px(-10000.))),
            ..Default::default()
        });
        cx.run_until_parked();
        let last = cx.debug_bounds("recent-repository-9").unwrap();
        assert_eq!(last.size.height, px(44.));
        assert!(last.top() >= card.top() && last.bottom() <= footer.top());
        let position = last.center();
        cx.simulate_mouse_down(position, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            shell.read_with(cx, |app, cx| app.editor.as_ref().unwrap().read(cx).ws.dir().to_owned()),
            folders[9].path().join(".topo").canonicalize().unwrap()
        );
    }

    #[gpui::test]
    fn the_filter_narrows_the_rows_and_the_keys_open_the_highlighted_one(cx: &mut TestAppContext) {
        bind(cx);
        let (parent, dirs) = named(&["alpha", "beta", "gamma"]);
        let config = UserConfig { recent_workspaces: dirs[1..].to_vec(), ..Default::default() };
        let start = parent.path().canonicalize().unwrap().join("alpha");
        let (shell, cx) = cx.add_window_view(|window, cx| RepositoryWindow::new(start, config, window, cx));
        cx.run_until_parked();
        cx.dispatch_action(OpenRepository);
        cx.run_until_parked();
        // Current is alpha; the history reads [alpha, beta, gamma].
        assert!(cx.debug_bounds("repository-current").is_some());
        cx.simulate_input("GAM");
        cx.run_until_parked();
        assert!(cx.debug_bounds("recent-repository-2").is_some());
        assert!(cx.debug_bounds("recent-repository-1").is_none() && cx.debug_bounds("repository-current").is_none());
        // Escape clears the filter before it closes the switcher.
        cx.simulate_keystrokes("escape");
        assert!(shell.read_with(cx, |app, cx| app.chooser && app.filter.read(cx).text().is_empty()));
        assert!(cx.debug_bounds("recent-repository-1").is_some());
        cx.simulate_input("zzz");
        cx.run_until_parked();
        assert!(cx.debug_bounds("recent-repository-1").is_none() && cx.debug_bounds("recent-repository-2").is_none());
        cx.simulate_keystrokes("escape escape");
        assert!(!shell.read_with(cx, |app, _| app.chooser));
        // Down moves from the current row to the next one, Enter opens it.
        cx.dispatch_action(OpenRepository);
        cx.run_until_parked();
        cx.simulate_keystrokes("down enter");
        cx.run_until_parked();
        assert_eq!(editor_dir(&shell, cx), dirs[1]);
        // Up wraps from the first row to the last.
        cx.dispatch_action(OpenRepository);
        cx.run_until_parked();
        cx.simulate_keystrokes("up enter");
        cx.run_until_parked();
        assert_eq!(editor_dir(&shell, cx), dirs[2]);
        // Enter on the current row only closes the switcher.
        cx.dispatch_action(OpenRepository);
        cx.run_until_parked();
        let editor = shell.read_with(cx, |app, _| app.editor.clone().unwrap());
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert!(!shell.read_with(cx, |app, _| app.chooser));
        assert_eq!(shell.read_with(cx, |app, _| app.editor.clone().unwrap().entity_id()), editor.entity_id());
    }

    #[gpui::test]
    fn a_typed_path_to_a_folder_is_the_first_row(cx: &mut TestAppContext) {
        bind(cx);
        let (parent, dirs) = named(&["alpha", "beta"]);
        let start = parent.path().canonicalize().unwrap().join("alpha");
        let (shell, cx) =
            cx.add_window_view(|window, cx| RepositoryWindow::new(start, UserConfig::default(), window, cx));
        cx.run_until_parked();
        cx.dispatch_action(OpenRepository);
        cx.run_until_parked();
        cx.simulate_input(&parent.path().canonicalize().unwrap().join("beta").display().to_string());
        cx.run_until_parked();
        assert!(cx.debug_bounds("repository-open").is_some());
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(editor_dir(&shell, cx), dirs[1]);
    }

    #[gpui::test]
    fn forgetting_a_recent_removes_it_from_the_history(cx: &mut TestAppContext) {
        bind(cx);
        let (parent, dirs) = named(&["alpha", "beta", "gamma"]);
        let config = UserConfig { recent_workspaces: dirs[1..].to_vec(), ..Default::default() };
        let start = parent.path().canonicalize().unwrap().join("alpha");
        let (shell, cx) = cx.add_window_view(|window, cx| RepositoryWindow::new(start, config, window, cx));
        cx.run_until_parked();
        cx.dispatch_action(OpenRepository);
        cx.run_until_parked();
        // The highlight is on the current row: the key keeps its meaning of editing the text.
        cx.simulate_input("ab");
        cx.simulate_keystrokes(&platform_keys("cmd-backspace"));
        assert!(shell.read_with(cx, |app, cx| app.filter.read(cx).text().is_empty()));
        assert_eq!(shell.read_with(cx, |app, _| app.config.recent_workspaces.len()), 3);
        // On a recent row it forgets that row.
        cx.simulate_keystrokes("down");
        cx.simulate_keystrokes(&platform_keys("cmd-backspace"));
        cx.run_until_parked();
        assert_eq!(
            shell.read_with(cx, |app, _| app.config.recent_workspaces.clone()),
            [dirs[0].clone(), dirs[2].clone()]
        );
        // The button does the same, and the highlight stays on a row.
        let button = cx.debug_bounds("forget-repository-1").expect("the highlighted row offers it");
        cx.simulate_mouse_down(button.center(), gpui::MouseButton::Left, gpui::Modifiers::none());
        cx.simulate_mouse_up(button.center(), gpui::MouseButton::Left, gpui::Modifiers::none());
        cx.run_until_parked();
        assert_eq!(shell.read_with(cx, |app, _| app.config.recent_workspaces.clone()), [dirs[0].clone()]);
        assert!(shell.read_with(cx, |app, _| app.chooser));
        // Choosing a folder in the same window still works with the history changed.
        assert!(cx.debug_bounds("repository-current").is_some());
    }

    #[gpui::test]
    fn a_missing_folder_is_marked_and_does_not_start_a_load(cx: &mut TestAppContext) {
        bind(cx);
        let (parent, dirs) = named(&["alpha"]);
        let gone = parent.path().join("gone").join(".topo");
        let config = UserConfig { recent_workspaces: vec![gone.clone()], ..Default::default() };
        let start = parent.path().canonicalize().unwrap().join("alpha");
        let (shell, cx) = cx.add_window_view(|window, cx| RepositoryWindow::new(start, config, window, cx));
        cx.run_until_parked();
        cx.dispatch_action(OpenRepository);
        cx.run_until_parked();
        assert!(shell.read_with(cx, |app, _| !app.recent_info[&gone].exists));
        // Its remove button is shown without hovering or highlighting it.
        assert!(cx.debug_bounds("forget-repository-1").is_some());
        let row = cx.debug_bounds("recent-repository-1").unwrap();
        cx.simulate_mouse_down(row.center(), gpui::MouseButton::Left, gpui::Modifiers::none());
        cx.simulate_mouse_up(row.center(), gpui::MouseButton::Left, gpui::Modifiers::none());
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert!(shell.read_with(cx, |app, _| !app.loading && app.chooser && app.error.is_none()));
        assert_eq!(editor_dir(&shell, cx), dirs[0]);
    }

    #[gpui::test]
    fn the_first_launch_welcomes_with_one_open_folder_button(cx: &mut TestAppContext) {
        use gpui::{px, size};
        bind(cx);
        let dir = tempfile::tempdir().unwrap();
        let (shell, cx) = cx.add_window_view(|window, cx| {
            RepositoryWindow::new(dir.path().join("missing"), UserConfig::default(), window, cx)
        });
        cx.simulate_resize(size(px(720.), px(480.)));
        cx.run_until_parked();
        assert!(cx.debug_bounds("repository-error").is_some());
        shell.update_in(cx, |app, window, cx| app.cancel(window, cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("repository-error").is_none());
        let card = cx.debug_bounds("repository-switcher").unwrap();
        let button = cx.debug_bounds("open-folder").unwrap();
        // It is the primary button in the middle of the card, not the one in the footer.
        assert!((button.center().x - card.center().x).abs() < px(1.));
        assert!(button.bottom() < card.bottom() - px(30.));
        // Without an editor there is nothing to return to.
        assert!(cx.debug_bounds("repository-current").is_none());
        cx.simulate_keystrokes("escape");
        assert!(cx.debug_bounds("repository-switcher").is_some());
    }

    #[gpui::test]
    fn enter_initializes_the_folder_the_banner_offers(cx: &mut TestAppContext) {
        bind(cx);
        let dir = tempfile::tempdir().unwrap();
        let (shell, cx) = cx.add_window_view(|window, cx| {
            RepositoryWindow::new(dir.path().to_owned(), UserConfig::default(), window, cx)
        });
        cx.run_until_parked();
        assert!(shell.read_with(cx, |app, _| app.pending.is_some()));
        assert!(cx.debug_bounds("repository-pending").is_some() && cx.debug_bounds("initialize-repository").is_some());
        assert!(!dir.path().join(".topo").exists());
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert!(dir.path().join(".topo/nodes").is_dir());
        assert!(shell.read_with(cx, |app, _| app.editor.is_some() && app.pending.is_none()));
    }

    #[test]
    fn typed_paths_name_existing_folders_only() {
        let home = tempfile::tempdir().unwrap();
        let home_dir = home.path().canonicalize().unwrap();
        std::fs::create_dir(home_dir.join("src")).unwrap();
        assert_eq!(typed_directory("~/src/", Some(&home_dir)), Some(home_dir.join("src")));
        assert_eq!(typed_directory(" ~ ", Some(&home_dir)), Some(home_dir.clone()));
        assert_eq!(typed_directory(&home_dir.display().to_string(), None), Some(home_dir.clone()));
        assert_eq!(typed_directory("~/missing", Some(&home_dir)), None);
        assert_eq!(typed_directory("~other", Some(&home_dir)), None);
        assert_eq!(typed_directory("~", None), None);
        assert_eq!(typed_directory("src", Some(&home_dir)), None);
        assert_eq!(typed_directory("", Some(&home_dir)), None);
    }
}
