//! Batch, headless visual QA: every GUI state in one process.
//!
//! Where `--screenshot` draws one state named by flags, `--screenshot-all`
//! builds its own fixture workspace and walks a table of states, so a full
//! review is one command and one process (one Metal context, one font scan).
//! It renders both themes and both Monotone finishes, then writes an
//! `index.html` to browse the result.
//!
//! Every state is a fresh offscreen window shaped before its first frame and
//! closed after it is captured, so the states never leak into each other.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use gpui::{AppContext as _, Context, Entity, VisualTestAppContext, Window, WindowHandle, px, size};
use jiff::ToSpan as _;
use topo_core::{Graph, Kind, Node, NodeId, Priority, Status, Workspace};
use topo_jev::organize::Proposal;

use crate::args::Args;
use crate::config::UserConfig;
use crate::repository::{RepositoryWindow, Selection};
use crate::theme::{self, ThemeMode};
use crate::{Field, Prompt, TopoApp, layout, notes, screenshot, text_input};

/// A theme and finish the whole state table is rendered in.
struct Variant {
    name: &'static str,
    mode: ThemeMode,
    monotone: bool,
}

const VARIANTS: [Variant; 4] = [
    Variant { name: "dark", mode: ThemeMode::Dark, monotone: false },
    Variant { name: "light", mode: ThemeMode::Light, monotone: false },
    Variant { name: "dark-mono", mode: ThemeMode::Dark, monotone: true },
    Variant { name: "light-mono", mode: ThemeMode::Light, monotone: true },
];

/// Which generated workspace a state is rendered against.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fixture {
    /// Tasks, a milestone, tags, priorities, due dates, statuses and notes.
    Rich,
    /// A workspace with no nodes at all.
    Empty,
    /// Tasks without tags, so grouping shows one untagged band.
    Untagged,
}

/// One named editor state: how to shape an editor before its frame.
struct EditorState {
    name: &'static str,
    fixture: Fixture,
    shape: fn(&mut TopoApp, &mut Window, &mut Context<TopoApp>),
    /// Frames the layout again after shaping, like a view toggle does.
    refit: bool,
}

/// One named shell state: what the repository window shows before a workspace.
#[derive(Clone, Copy)]
enum ShellState {
    /// Nothing is open and nothing was opened before.
    Welcome,
    /// A folder that has no `.topo` yet.
    Uninitialized,
    /// The switcher over a loaded workspace, with a missing recent.
    Switcher,
}

impl ShellState {
    fn start(self, fixtures: &Fixtures) -> (Selection, UserConfig, bool) {
        // These are exact fixture folders. Startup's ancestor discovery would
        // find the user's workspace when the output directory is inside one.
        match self {
            Self::Welcome => (fixtures.plain.clone().into(), UserConfig::default(), true),
            Self::Uninitialized => {
                let config = UserConfig { recent_workspaces: vec![fixtures.rich_dir()], ..Default::default() };
                (fixtures.plain.clone().into(), config, false)
            }
            Self::Switcher => {
                let config = UserConfig {
                    recent_workspaces: vec![fixtures.rich_dir(), fixtures.missing_dir()],
                    ..Default::default()
                };
                (fixtures.rich_folder().into(), config, true)
            }
        }
    }
}

fn id(value: &str) -> NodeId {
    NodeId(value.into())
}

fn editor_states() -> [EditorState; 22] {
    [
        EditorState { name: "default", fixture: Fixture::Rich, shape: |_, _, _| {}, refit: false },
        EditorState { name: "select", fixture: Fixture::Rich, shape: select_api, refit: false },
        EditorState { name: "milestone", fixture: Fixture::Rich, shape: select_milestone, refit: false },
        EditorState { name: "multi-select", fixture: Fixture::Rich, shape: select_many, refit: false },
        EditorState { name: "grouped", fixture: Fixture::Rich, shape: grouped, refit: true },
        EditorState { name: "grouped-collapsed", fixture: Fixture::Rich, shape: grouped_collapsed, refit: true },
        EditorState { name: "hide-completed", fixture: Fixture::Rich, shape: hide_completed, refit: true },
        EditorState { name: "priority-filter", fixture: Fixture::Rich, shape: priority_filter, refit: false },
        EditorState { name: "notes", fixture: Fixture::Rich, shape: notes_open, refit: false },
        EditorState { name: "inline-edit", fixture: Fixture::Rich, shape: inline_edit, refit: false },
        EditorState { name: "unsaved-dialog", fixture: Fixture::Rich, shape: unsaved_dialog, refit: false },
        EditorState { name: "search", fixture: Fixture::Rich, shape: search, refit: false },
        EditorState { name: "help", fixture: Fixture::Rich, shape: help, refit: false },
        EditorState { name: "theme-menu", fixture: Fixture::Rich, shape: theme_menu, refit: false },
        EditorState { name: "save-saving", fixture: Fixture::Rich, shape: save_saving, refit: false },
        EditorState { name: "save-error", fixture: Fixture::Rich, shape: save_error, refit: false },
        EditorState { name: "toast-success", fixture: Fixture::Rich, shape: toast_success, refit: false },
        EditorState { name: "toast-error", fixture: Fixture::Rich, shape: toast_error, refit: false },
        EditorState { name: "jev-proposals", fixture: Fixture::Rich, shape: jev_proposals, refit: false },
        EditorState { name: "jev-busy", fixture: Fixture::Rich, shape: jev_busy, refit: false },
        EditorState { name: "empty", fixture: Fixture::Empty, shape: |_, _, _| {}, refit: false },
        EditorState { name: "untagged-grouped", fixture: Fixture::Untagged, shape: grouped, refit: true },
    ]
}

const SHELL_STATES: [(&str, ShellState); 3] = [
    ("shell-welcome", ShellState::Welcome),
    ("shell-uninitialized", ShellState::Uninitialized),
    ("shell-switcher", ShellState::Switcher),
];

fn select_api(app: &mut TopoApp, _: &mut Window, _: &mut Context<TopoApp>) {
    app.select(Some(id(API)), false);
}

fn select_milestone(app: &mut TopoApp, _: &mut Window, _: &mut Context<TopoApp>) {
    app.select(Some(id(RELEASE)), false);
}

fn select_many(app: &mut TopoApp, _: &mut Window, cx: &mut Context<TopoApp>) {
    app.select_nodes(&[id(API), id(WEB)], cx);
}

fn grouped(app: &mut TopoApp, _: &mut Window, _: &mut Context<TopoApp>) {
    app.view.group_by_tag = true;
}

fn grouped_collapsed(app: &mut TopoApp, _: &mut Window, _: &mut Context<TopoApp>) {
    app.view.group_by_tag = true;
    app.view.collapsed.insert(layout::Group::Tag("infra".into()));
}

fn hide_completed(app: &mut TopoApp, _: &mut Window, _: &mut Context<TopoApp>) {
    app.view.hide_completed = true;
}

fn priority_filter(app: &mut TopoApp, _: &mut Window, _: &mut Context<TopoApp>) {
    app.priority_filter = Some(Priority::High);
}

fn notes_open(app: &mut TopoApp, window: &mut Window, cx: &mut Context<TopoApp>) {
    app.select(Some(id(DESIGN)), false);
    app.start_notes(window, cx);
}

fn inline_edit(app: &mut TopoApp, window: &mut Window, cx: &mut Context<TopoApp>) {
    app.select(Some(id(API)), false);
    app.start_inline(Field::Tags, window, cx);
    app.combo().update(cx, |combo, cx| combo.set_text("#backend", cx));
}

fn unsaved_dialog(app: &mut TopoApp, window: &mut Window, cx: &mut Context<TopoApp>) {
    app.select(Some(id(DESIGN)), false);
    app.start_notes(window, cx);
    app.notes_input().update(cx, |input, cx| input.set_text("A change that is not saved yet", cx));
    app.ask_notes(notes::Then::Stay, cx);
}

fn search(app: &mut TopoApp, window: &mut Window, cx: &mut Context<TopoApp>) {
    app.open_prompt(Prompt::Search, window, cx);
    app.palette.update(cx, |palette, cx| palette.set_text("release", cx));
}

fn help(app: &mut TopoApp, _: &mut Window, _: &mut Context<TopoApp>) {
    app.show_help = true;
}

fn theme_menu(app: &mut TopoApp, _: &mut Window, _: &mut Context<TopoApp>) {
    app.theme_menu = true;
}

fn save_saving(app: &mut TopoApp, _: &mut Window, _: &mut Context<TopoApp>) {
    app.persistence.show_for_capture(false);
}

fn save_error(app: &mut TopoApp, _: &mut Window, _: &mut Context<TopoApp>) {
    app.persistence.show_for_capture(true);
}

fn toast_success(app: &mut TopoApp, _: &mut Window, cx: &mut Context<TopoApp>) {
    app.toast("Saved", false, cx);
}

fn toast_error(app: &mut TopoApp, _: &mut Window, cx: &mut Context<TopoApp>) {
    app.toast("Would create a cycle", true, cx);
}

fn jev_proposals(app: &mut TopoApp, _: &mut Window, _: &mut Context<TopoApp>) {
    app.select(Some(id(API)), false);
    app.proposals = vec![
        Proposal::Link { from: id(WEB), to: id(DOCS), probability: 0.82 },
        Proposal::Join { task: id(DOCS), milestone: id(RELEASE), probability: 0.74 },
        Proposal::SetKind { id: id(LEGACY), kind: Kind::Milestone, probability: 0.61 },
        Proposal::Duplicate { a: id(API), b: id(WEB), probability: 0.55 },
    ];
}

fn jev_busy(app: &mut TopoApp, _: &mut Window, _: &mut Context<TopoApp>) {
    app.select(Some(id(API)), false);
    app.busy = true;
}

/// Renders every QA state into `out`, in one process.
///
/// `only` filters the state names; an unknown name is an error that lists the
/// known ones, so a typo never passes for an empty run.
pub(crate) fn render_all(out: &Path, options: &Args, only: &[String]) -> Result<()> {
    let names = state_names();
    for requested in only {
        anyhow::ensure!(
            names.contains(&requested.as_str()),
            "unknown QA state `{requested}`; known states: {}",
            names.join(", ")
        );
    }
    let want = |name: &str| only.is_empty() || only.iter().any(|s| s == name);
    let states = editor_states();
    let editors: Vec<&EditorState> = states.iter().filter(|s| want(s.name)).collect();
    let shells: Vec<(&'static str, ShellState)> = SHELL_STATES.iter().copied().filter(|(n, _)| want(n)).collect();

    std::fs::create_dir_all(out)?;
    let fixtures = Fixtures::build(out)?;

    let platform = gpui_platform::current_platform(false);
    let mut cx = VisualTestAppContext::with_asset_source(platform, std::sync::Arc::new(crate::icons::Assets));
    cx.update(text_input::bind_keys);

    let mut entries: Vec<Entry> = Vec::new();
    for variant in &VARIANTS {
        theme::apply(variant.mode, variant.monotone, gpui::WindowAppearance::default());
        let dir = out.join(variant.name);
        std::fs::create_dir_all(&dir)?;
        for state in &editors {
            let path = dir.join(format!("{}.png", state.name));
            render_editor(&mut cx, fixtures.open(state.fixture), state, options, &path)?;
            entries.push(Entry {
                variant: variant.name,
                state: state.name.to_owned(),
                file: format!("{}/{}.png", variant.name, state.name),
            });
        }
        for (name, shell) in &shells {
            let path = dir.join(format!("{name}.png"));
            render_shell(&mut cx, &fixtures, *shell, options, &path)?;
            entries.push(Entry {
                variant: variant.name,
                state: (*name).to_owned(),
                file: format!("{}/{}.png", variant.name, name),
            });
        }
    }
    write_index(out, &entries)?;
    println!("Wrote {} captures to {}", entries.len(), out.display());
    Ok(())
}

/// The names `--screenshot-all` renders, for `--list-states` and `--state`.
pub(crate) fn state_names() -> Vec<&'static str> {
    editor_states().iter().map(|s| s.name).chain(SHELL_STATES.iter().map(|(n, _)| *n)).collect()
}

fn open_editor(
    cx: &mut VisualTestAppContext,
    ws: Workspace,
    options: &Args,
) -> Result<(WindowHandle<TopoApp>, Entity<TopoApp>)> {
    let requested = size(px(options.width as f32), px(options.height as f32));
    let inspector_width = options.inspector_width.map(|width| width as f32);
    let handle = cx.open_offscreen_window(requested, |window, cx| {
        cx.new(|cx| {
            TopoApp::with_inspector_width(ws, inspector_width, window, cx).expect("the workspace opens for rendering")
        })
    })?;
    let actual = cx.update_window(handle.into(), |_, window, _| window.viewport_size())?;
    anyhow::ensure!(
        actual == requested,
        "the window was limited to {}x{} points instead of {}x{}",
        f32::from(actual.width),
        f32::from(actual.height),
        options.width,
        options.height
    );
    let entity = handle.entity(cx)?;
    Ok((handle, entity))
}

fn render_editor(
    cx: &mut VisualTestAppContext,
    ws: Workspace,
    state: &EditorState,
    options: &Args,
    path: &Path,
) -> Result<()> {
    let (handle, entity) = open_editor(cx, ws, options)?;
    cx.update_window(handle.into(), |_, window, cx| {
        entity.update(cx, |app, cx| {
            (state.shape)(app, window, cx);
            cx.notify();
        })
    })?;
    settle(cx, &entity);
    // A view toggle frames the new layout again, like the toolbar does.
    if state.refit {
        cx.update(|app| {
            entity.update(app, |app, cx| {
                app.refit();
                cx.notify();
            })
        });
        for _ in 0..2 {
            cx.run_until_parked();
            cx.update(|app| app.notify(entity.entity_id()));
        }
        cx.run_until_parked();
    }
    write_png(cx, handle, path)
}

fn render_shell(
    cx: &mut VisualTestAppContext,
    fixtures: &Fixtures,
    state: ShellState,
    options: &Args,
    path: &Path,
) -> Result<()> {
    let requested = size(px(options.width as f32), px(options.height as f32));
    let (start, config, present) = state.start(fixtures);
    let handle = cx.open_offscreen_window(requested, |window, cx| {
        cx.new(|cx| RepositoryWindow::capture(start, config, window, cx))
    })?;
    let shell = handle.entity(cx)?;
    cx.run_until_parked();
    // `present` shows the switcher over an editor, and with no editor it clears
    // the pending banner: that is the welcome card on a first launch.
    if present {
        cx.update_window(handle.into(), |_, window, cx| shell.update(cx, |app, cx| app.present(window, cx)))?;
    }
    settle(cx, &shell);
    write_png(cx, handle, path)
}

/// Runs the frames that settle layout and the camera fit.
fn settle<T: gpui::Render>(cx: &mut VisualTestAppContext, root: &Entity<T>) {
    for _ in 0..4 {
        cx.run_until_parked();
        cx.update(|app| app.notify(root.entity_id()));
    }
    cx.run_until_parked();
}

fn write_png(cx: &mut VisualTestAppContext, handle: WindowHandle<impl gpui::Render>, path: &Path) -> Result<()> {
    let image = cx.capture_screenshot(handle.into())?;
    screenshot::write_png(&image, path)?;
    cx.update_window(handle.into(), |_, window, _| window.remove_window())?;
    cx.run_until_parked();
    Ok(())
}

/// The generated workspaces every state is rendered against, under `out`.
struct Fixtures {
    root: PathBuf,
    rich: Workspace,
    empty: Workspace,
    untagged: Workspace,
    plain: PathBuf,
}

impl Fixtures {
    fn build(out: &Path) -> Result<Self> {
        let root = out.join("_fixtures");
        if root.exists() {
            std::fs::remove_dir_all(&root).with_context(|| format!("cannot clear {}", root.display()))?;
        }
        std::fs::create_dir_all(&root)?;
        // History and the installed editor must identify the same workspace,
        // including relative output paths and symlinked parent directories.
        let root = root.canonicalize().context("cannot resolve the fixture directory")?;
        let plain = root.join("plain");
        std::fs::create_dir_all(&plain)?;
        Ok(Self {
            rich: write_fixture(&root.join("rich"), Fixture::Rich)?,
            empty: write_fixture(&root.join("empty"), Fixture::Empty)?,
            untagged: write_fixture(&root.join("untagged"), Fixture::Untagged)?,
            plain,
            root,
        })
    }

    fn open(&self, fixture: Fixture) -> Workspace {
        match fixture {
            Fixture::Rich => self.rich.clone(),
            Fixture::Empty => self.empty.clone(),
            Fixture::Untagged => self.untagged.clone(),
        }
    }

    fn rich_dir(&self) -> PathBuf {
        self.rich.dir().to_owned()
    }

    fn rich_folder(&self) -> PathBuf {
        self.rich.dir().parent().expect("a workspace has a folder").to_owned()
    }

    fn missing_dir(&self) -> PathBuf {
        self.root.join("missing").join(topo_core::store::DIR_NAME)
    }
}

fn write_fixture(root: &Path, fixture: Fixture) -> Result<Workspace> {
    std::fs::create_dir_all(root)?;
    let mut ws = Workspace::init(root)?;
    ws.graph = fixture.graph();
    ws.save()?;
    if fixture == Fixture::Rich {
        // The organize controls show only where Jev is configured; the capture
        // never calls the endpoint, so the value is inert.
        std::fs::write(ws.dir().join("config.toml"), "[jev]\nbase_url = \"http://127.0.0.1:8000\"\n")?;
    }
    Ok(ws)
}

const RELEASE: &str = "release";
const DESIGN: &str = "design";
const API: &str = "api";
const WEB: &str = "web";
const DOCS: &str = "docs";
const INFRA: &str = "infra";
const LEGACY: &str = "legacy";

impl Fixture {
    fn graph(self) -> Graph {
        match self {
            Fixture::Rich => rich_graph(),
            Fixture::Empty => Graph::default(),
            Fixture::Untagged => untagged_graph(),
        }
    }
}

fn task(id: &str, title: &str) -> Node {
    Node::new(NodeId(id.into()), Kind::Task, title.into())
}

/// A graph that exercises every card and inspector branch: statuses, tags,
/// priorities, due dates, a milestone, dependencies, notes and a pull request.
fn rich_graph() -> Graph {
    let today = crate::dates::today();

    let mut release = Node::new(id(RELEASE), Kind::Milestone, "v1.0 release".into());
    release.body = "Ship the **first public release**.\n\n- [x] brand\n- [ ] docs\n".into();

    let mut design = task(DESIGN, "Design the canvas");
    design.status = Status::Done;
    design.priority = Some(Priority::High);
    design.tags = vec!["design".into()];
    design.milestones = vec![id(RELEASE)];
    design.body =
        "# Notes\n\nTile the canvas on a 16pt grid.\n\n> Keep it quiet.\n\n```rust\nlet card = Card::new();\n```\n"
            .into();

    let mut api = task(API, "Wire the sync API");
    api.status = Status::Doing;
    api.priority = Some(Priority::Urgent);
    api.due = today.checked_add(2.days()).ok();
    api.tags = vec!["backend".into()];
    api.milestones = vec![id(RELEASE)];
    api.depends_on = vec![id(DESIGN)];
    api.assignee = Some("claude".into());
    api.prs = vec!["https://github.com/r4ai/topo/pull/12".into()];

    let mut web = task(WEB, "Build the activity view");
    web.priority = Some(Priority::Medium);
    web.due = today.checked_add((-2).days()).ok();
    web.tags = vec!["frontend".into(), "design".into()];
    web.milestones = vec![id(RELEASE)];
    web.depends_on = vec![id(API)];
    web.assignee = Some("r4ai".into());

    let mut docs = task(DOCS, "Write the usage guide");
    docs.priority = Some(Priority::Low);
    docs.tags = vec!["docs".into()];

    let mut infra = task(INFRA, "Provision the release runner");
    infra.priority = Some(Priority::High);
    infra.due = today.checked_add(30.days()).ok();
    infra.tags = vec!["infra".into()];
    infra.milestones = vec![id(RELEASE)];

    let mut legacy = task(LEGACY, "Retire the old exporter");
    legacy.status = Status::Dropped;
    legacy.tags = vec!["infra".into()];
    legacy.milestones = vec![id(RELEASE)];

    Graph::from_nodes(vec![release, design, api, web, docs, infra, legacy]).expect("the fixture is a valid DAG")
}

/// A chain with no tags, so grouping yields one untagged band.
fn untagged_graph() -> Graph {
    let mut alpha = task("alpha", "Draft the outline");
    alpha.status = Status::Done;
    let mut bravo = task("bravo", "Gather the sources");
    bravo.status = Status::Doing;
    bravo.depends_on = vec![id("alpha")];
    let mut charlie = task("charlie", "Write the first pass");
    charlie.depends_on = vec![id("bravo")];
    Graph::from_nodes(vec![alpha, bravo, charlie]).expect("the fixture is a valid DAG")
}

struct Entry {
    variant: &'static str,
    state: String,
    file: String,
}

/// A minimal contact sheet: one section per variant, every state beside its name.
fn write_index(out: &Path, entries: &[Entry]) -> Result<()> {
    let mut html = String::from(
        "<!doctype html>\n<meta charset=\"utf-8\">\n<title>topo visual QA</title>\n\
         <style>body{background:#111;color:#eee;font:13px system-ui;margin:24px}\
         h1{font-weight:600}h2{margin-top:32px;border-bottom:1px solid #333;padding-bottom:4px}\
         .grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(320px,1fr));gap:16px}\
         figure{margin:0}img{width:100%;border:1px solid #333;border-radius:6px;background:#000}\
         figcaption{margin-top:6px;color:#aaa}</style>\n<h1>topo visual QA</h1>\n",
    );
    for variant in &VARIANTS {
        let _ = write!(html, "<section><h2>{}</h2><div class=\"grid\">", variant.name);
        for entry in entries.iter().filter(|e| e.variant == variant.name) {
            let _ = write!(
                html,
                "<figure><img src=\"{file}\" loading=\"lazy\"><figcaption>{state}</figcaption></figure>",
                file = entry.file,
                state = entry.state
            );
        }
        html.push_str("</div></section>\n");
    }
    std::fs::write(out.join("index.html"), html).context("cannot write index.html")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn bare_shell_states_ignore_an_ancestor_workspace(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path()).unwrap();
        let fixtures = Fixtures::build(&dir.path().join("qa")).unwrap();

        for state in [ShellState::Welcome, ShellState::Uninitialized] {
            let (start, config, present) = state.start(&fixtures);
            let expected = config.clone();
            let (shell, cx) = cx.add_window_view(|window, cx| RepositoryWindow::capture(start, config, window, cx));
            cx.run_until_parked();
            if present {
                shell.update_in(cx, |app, window, cx| app.present(window, cx));
                cx.run_until_parked();
            }
            shell.read_with(cx, |app, _| {
                assert!(app.editor.is_none(), "a bare shell must not load the ancestor workspace");
                assert_eq!(app.config, expected);
            });
            assert!(!fixtures.plain.join(topo_core::store::DIR_NAME).exists());
        }
    }

    #[gpui::test]
    fn a_relative_output_keeps_the_current_workspace_out_of_recents(cx: &mut TestAppContext) {
        let cwd = std::env::current_dir().unwrap().canonicalize().unwrap();
        let target = cwd.join("target");
        std::fs::create_dir_all(&target).unwrap();
        let dir = tempfile::tempdir_in(&target).unwrap();
        let out = dir.path().strip_prefix(&cwd).unwrap().join("qa");
        assert!(out.is_relative());
        let fixtures = Fixtures::build(&out).unwrap();
        let (start, config, present) = ShellState::Switcher.start(&fixtures);
        let (shell, cx) = cx.add_window_view(|window, cx| RepositoryWindow::capture(start, config, window, cx));
        cx.run_until_parked();
        assert!(present);
        shell.update_in(cx, |app, window, cx| app.present(window, cx));
        cx.run_until_parked();
        shell.read_with(cx, |app, cx| {
            let editor = app.editor.as_ref().expect("the rich fixture is loaded");
            assert_eq!(editor.read(cx).ws.dir(), fixtures.rich_dir().canonicalize().unwrap());
            assert_eq!(app.config.recent_workspaces, [fixtures.rich_dir(), fixtures.missing_dir()]);
        });
    }
}
