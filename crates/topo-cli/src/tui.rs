//! `topo tui`: browse views, change status, and follow external edits live.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use notify::{RecursiveMode, Watcher};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, List, ListState, Paragraph, Tabs, Wrap};
use ratatui::{DefaultTerminal, Frame};
use topo_core::{Kind, Node, NodeId, Op, Status, Workspace};

use crate::render;

#[derive(Clone, Copy, PartialEq)]
enum View {
    Ready,
    Milestones,
    Open,
    All,
}

impl View {
    const ALL: [View; 4] = [View::Ready, View::Milestones, View::Open, View::All];

    fn title(self) -> &'static str {
        match self {
            View::Ready => "1 Ready",
            View::Milestones => "2 Milestones",
            View::Open => "3 Open",
            View::All => "4 All",
        }
    }
}

struct App {
    ws: Workspace,
    view: View,
    list: ListState,
    message: String,
}

impl App {
    fn visible(&self) -> Vec<&Node> {
        let graph = &self.ws.graph;
        match self.view {
            View::Ready => graph.ready_tasks(None),
            View::Milestones => graph.nodes().filter(|n| n.kind == Kind::Milestone).collect(),
            View::Open => graph.nodes().filter(|n| !n.status.is_closed()).collect(),
            View::All => graph.nodes().collect(),
        }
    }

    fn selected(&self) -> Option<NodeId> {
        self.list.selected().and_then(|i| self.visible().get(i).map(|n| n.id.clone()))
    }

    fn set_status(&mut self, status: Status) -> Result<()> {
        if let Some(id) = self.selected() {
            self.ws.apply(vec![Op::Status { id: id.0, status, if_status: None }])?;
        }
        Ok(())
    }

    /// Picks up edits made outside the TUI; its own saves reload an identical graph.
    fn reload(&mut self) {
        match self.ws.reload() {
            Ok(false) => {}
            Ok(true) => self.message = "reloaded".into(),
            Err(e) => self.message = format!("reload failed: {e}"),
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let [tabs_area, main, help] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());
        let [left, right] = Layout::horizontal([Constraint::Percentage(45), Constraint::Min(0)]).areas(main);

        let index = View::ALL.iter().position(|v| *v == self.view).expect("view is listed");
        frame.render_widget(Tabs::new(View::ALL.map(View::title)).select(index), tabs_area);

        let len = self.visible().len();
        self.list.select(match len {
            0 => None,
            _ => Some(self.list.selected().unwrap_or(0).min(len - 1)),
        });
        let nodes = self.visible();
        let items: Vec<String> = nodes
            .iter()
            .map(|n| match n.kind {
                Kind::Milestone => render::milestone_line(&self.ws.graph, n),
                Kind::Task => render::node_line(n),
            })
            .collect();
        let detail = self
            .list
            .selected()
            .and_then(|i| nodes.get(i))
            .map(|n| render::detail_text(&self.ws.graph, n))
            .unwrap_or_default();
        let list = List::new(items)
            .block(Block::bordered().title(self.view.title()))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED));
        frame.render_stateful_widget(list, left, &mut self.list);
        frame.render_widget(Paragraph::new(detail).wrap(Wrap { trim: false }).block(Block::bordered()), right);
        let keys = "q quit  1-4/tab view  j/k move  space next status  x done  d drop  u todo";
        frame.render_widget(Line::from(format!("{keys}   {}", self.message)).dim(), help);
    }
}

/// How often a cloud workspace is asked whether it changed.
const POLL: Duration = Duration::from_secs(5);

/// Tells when the nodes may have changed outside the TUI.
enum Outside {
    /// The Markdown files are watched.
    Files { events: mpsc::Receiver<()>, _watcher: notify::RecommendedWatcher },
    /// The server is asked every [`POLL`].
    Remote { asked: Instant },
}

impl Outside {
    fn watch(ws: &Workspace) -> Result<Self> {
        if ws.remote().is_some() {
            return Ok(Outside::Remote { asked: Instant::now() });
        }
        let (tx, events) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_ok() {
                let _ = tx.send(());
            }
        })?;
        watcher.watch(&ws.nodes_dir(), RecursiveMode::NonRecursive)?;
        Ok(Outside::Files { events, _watcher: watcher })
    }

    fn may_have_changed(&mut self) -> bool {
        match self {
            Outside::Files { events, .. } => events.try_iter().count() > 0,
            Outside::Remote { asked } => {
                let due = asked.elapsed() >= POLL;
                if due {
                    *asked = Instant::now();
                }
                due
            }
        }
    }
}

pub fn run(ws: Workspace) -> Result<()> {
    let mut outside = Outside::watch(&ws)?;
    let mut app = App { ws, view: View::Ready, list: ListState::default(), message: String::new() };
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app, &mut outside);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut DefaultTerminal, app: &mut App, outside: &mut Outside) -> Result<()> {
    loop {
        if outside.may_have_changed() {
            app.reload();
        }
        terminal.draw(|f| app.draw(f))?;
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(key) = event::read()? else { continue };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let result = match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
            KeyCode::Char(c @ '1'..='4') => {
                app.view = View::ALL[c as usize - '1' as usize];
                Ok(())
            }
            KeyCode::Tab => {
                let index = View::ALL.iter().position(|v| *v == app.view).expect("view is listed");
                app.view = View::ALL[(index + 1) % View::ALL.len()];
                Ok(())
            }
            KeyCode::Char('j') | KeyCode::Down => {
                app.list.select_next();
                Ok(())
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.list.select_previous();
                Ok(())
            }
            KeyCode::Char(' ') => match app.selected().and_then(|id| app.ws.graph.get(&id).map(|n| n.status)) {
                Some(Status::Todo) => app.set_status(Status::Doing),
                Some(Status::Doing) => app.set_status(Status::Done),
                Some(Status::Done | Status::Dropped) => app.set_status(Status::Todo),
                None => Ok(()),
            },
            KeyCode::Char('x') => app.set_status(Status::Done),
            KeyCode::Char('d') => app.set_status(Status::Dropped),
            KeyCode::Char('u') => app.set_status(Status::Todo),
            _ => Ok(()),
        };
        if let Err(e) = result {
            app.message = format!("error: {e}");
        }
    }
}
