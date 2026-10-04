//! The repository switcher: a palette-style glass dialog with a filter field, the current and recent
//! workspaces, and the status of an opening. It sits over the editor, or alone before any workspace is open.

use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, App, Context, Div, FontWeight, Hsla, IntoElement, MouseButton, Stateful, Window, div, prelude::*, px,
};

use super::{RecentInfo, RepositoryWindow, folder_name, folder_of};
use crate::chrome::on_primary_kbd;
use crate::theme::{
    self,
    metrics::{H_ROW, R_XL, S3, S4, S5, S6, S8, T_BODY, T_BODY_LG, T_HEADING, T_SMALL, T_TITLE, W_PALETTE},
};
use crate::ui::{self, Elevation};

/// A row of the list, by what it opens.
#[derive(Clone)]
pub(super) enum Row {
    /// The folder the filter text names.
    Open(PathBuf),
    /// The workspace in the editor.
    Current(PathBuf),
    /// An index into `config.recent_workspaces`.
    Recent(usize),
}

/// The position among the list's children of row `index`: the section labels count as children.
pub(super) fn slot(rows: &[Row], index: usize) -> usize {
    let seen = &rows[..=index];
    let labels = usize::from(seen.iter().any(|row| matches!(row, Row::Current(_))))
        + usize::from(seen.iter().any(|row| matches!(row, Row::Recent(_))));
    index + labels
}

/// `path` with the home directory written as `~`.
pub(super) fn abbreviate_home(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => Path::new("~").join(rest).display().to_string(),
        None => path.display().to_string(),
    }
}

/// `text` cut to `max` characters by replacing its middle with `…`; the end, which names the folder, keeps more.
pub(super) fn truncate_middle(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_owned();
    }
    let Some(kept) = max.checked_sub(1) else { return String::new() };
    let head = kept / 3;
    let tail = kept - head;
    let start: String = text.chars().take(head).collect();
    let end: String = text.chars().skip(count - tail).collect();
    format!("{start}…{end}")
}

/// How many characters of a path fit in a row of a card as wide as `card` points.
fn path_budget(card: f32) -> usize {
    let room = card - 2. * (S4 + S4) - 96.;
    (room / (T_SMALL * 0.48)).max(8.) as usize
}

impl RepositoryWindow {
    /// The rows the filter leaves, in the order they are listed.
    pub(super) fn rows(&self, cx: &App) -> Vec<Row> {
        let query = self.filter.read(cx).text().trim().to_lowercase();
        let home = dirs::home_dir();
        let matches = |dir: &Path| {
            let folder = folder_of(dir);
            query.is_empty()
                || folder_name(folder).to_lowercase().contains(&query)
                || abbreviate_home(folder, home.as_deref()).to_lowercase().contains(&query)
        };
        let current = self.current_dir(cx);
        let mut rows: Vec<Row> = self.typed.iter().cloned().map(Row::Open).collect();
        rows.extend(current.iter().filter(|dir| matches(dir)).cloned().map(Row::Current));
        rows.extend(
            self.config
                .recent_workspaces
                .iter()
                .enumerate()
                .filter(|(_, dir)| current.as_ref() != Some(*dir) && matches(dir))
                .map(|(index, _)| Row::Recent(index)),
        );
        rows
    }

    /// The switcher: the scrim over an open editor and the card in the upper third of the window.
    pub(super) fn switcher(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let height = f32::from(window.viewport_size().height);
        let top = (height * 0.15).clamp(56., 140.);
        let layer = match self.editor {
            Some(_) => {
                ui::scrim().on_mouse_down(MouseButton::Left, cx.listener(|app, _, window, cx| app.cancel(window, cx)))
            }
            None => div().absolute().size_full().flex(),
        };
        layer.items_start().justify_center().px(px(S6)).pt(px(top)).child(self.card(window, height - top - S6, cx))
    }

    fn card(&self, window: &Window, max_height: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.rows(cx);
        let width = (f32::from(window.viewport_size().width) - 2. * S6).min(W_PALETTE);
        ui::glass(Elevation::Dialog)
            .key_context("RepositorySwitcher")
            .id("repository-switcher")
            .debug_selector(|| "repository-switcher".into())
            .w_full()
            .max_w(px(W_PALETTE))
            .max_h(px(max_height))
            .flex()
            .flex_col()
            .rounded(px(R_XL))
            .overflow_hidden()
            // Clicks inside the card neither close the switcher nor take the focus from the filter.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(self.filter_field())
            .children(self.banner(cx))
            .child(match self.first_launch() {
                true => self.welcome(cx).into_any_element(),
                false => self.list(&rows, path_budget(width), cx).into_any_element(),
            })
            .child(self.footer(cx))
    }

    /// Nothing is open and nothing was opened before.
    fn first_launch(&self) -> bool {
        self.editor.is_none() && self.config.recent_workspaces.is_empty()
    }

    fn filter_field(&self) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(S5))
            .px(px(S6))
            .py(px(S5))
            .text_size(px(T_TITLE))
            .child(div().flex_shrink_0().text_color(theme::current().milestone).child("◆"))
            .child(
                div().flex_1().min_w(px(64.)).debug_selector(|| "repository-filter".into()).child(self.filter.clone()),
            )
    }

    /// The status of the last request: an opening, a failure, or a folder that needs a workspace.
    fn banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = theme::current();
        if self.loading {
            let text = match &self.opening {
                Some(name) => format!("◌ Opening {name}…"),
                None => "◌ Choosing a folder…".to_owned(),
            };
            return Some(
                banner_frame(gpui::transparent_black())
                    .debug_selector(|| "repository-loading".into())
                    .text_color(t.fg_muted)
                    .child(text)
                    .into_any_element(),
            );
        }
        if let Some(error) = &self.error {
            return Some(
                banner_frame(theme::alpha(t.danger, 0.08))
                    .debug_selector(|| "repository-error".into())
                    .child(div().flex_shrink_0().text_color(t.danger).child("⚠"))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(S3 / 2.))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Couldn't open repository"))
                            .child(
                                div().text_size(px(T_SMALL)).text_color(t.fg_muted).line_clamp(3).child(error.clone()),
                            ),
                    )
                    .into_any_element(),
            );
        }
        let folder = self.pending.as_ref()?;
        let home = dirs::home_dir();
        Some(
            banner_frame(t.control.into())
                .debug_selector(|| "repository-pending".into())
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .gap(px(S3 / 2.))
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .truncate()
                                .child(format!("{} has no topo workspace yet", folder_name(folder))),
                        )
                        .child(
                            div()
                                .text_size(px(T_SMALL))
                                .text_color(t.fg_muted)
                                .truncate()
                                .child(abbreviate_home(folder, home.as_deref())),
                        ),
                )
                .child(
                    ui::button_primary("initialize-repository", "Initialize workspace")
                        .child(on_primary_kbd("↵"))
                        .on_click(cx.listener(|app, _, window, cx| app.initialize(window, cx))),
                )
                .into_any_element(),
        )
    }

    /// The scrolling list of rows under their section labels.
    fn list(&self, rows: &[Row], budget: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current();
        let mut children: Vec<AnyElement> = Vec::new();
        let (mut current, mut recent) = (false, false);
        for (position, row) in rows.iter().enumerate() {
            match row {
                Row::Current(_) if !current => {
                    current = true;
                    children.push(section("CURRENT"));
                }
                Row::Recent(_) if !recent => {
                    recent = true;
                    children.push(section("RECENT"));
                }
                _ => {}
            }
            let selected = position == self.highlight.min(rows.len() - 1);
            children.push(self.row(row, position, selected, budget, cx).into_any_element());
        }
        if rows.is_empty() {
            children.push(
                div().px(px(S4)).py(px(S5)).text_color(t.fg_faint).child("No matching repository").into_any_element(),
            );
        }
        div()
            .id("repository-list")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .track_scroll(&self.list_scroll)
            .px(px(S4))
            .pb(px(S4))
            .text_size(px(T_BODY))
            .children(children)
    }

    fn row(&self, row: &Row, position: usize, selected: bool, budget: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let t = theme::current();
        let home = dirs::home_dir();
        let shown = |dir: &Path| truncate_middle(&abbreviate_home(folder_of(dir), home.as_deref()), budget);
        let mut gone = false;
        let (id, title, subtitle, trailing) = match row {
            Row::Open(dir) => {
                let title = format!("Open {}", abbreviate_home(dir, home.as_deref()));
                ("repository-open".to_owned(), title, "Use this folder as a repository".to_owned(), Vec::new())
            }
            Row::Current(dir) => {
                let ready = div().flex_shrink_0().text_color(t.ready).child("✓").into_any_element();
                ("repository-current".to_owned(), folder_name(folder_of(dir)), shown(dir), vec![ready])
            }
            Row::Recent(index) => {
                let index = *index;
                let dir = &self.config.recent_workspaces[index];
                let info = self.recent_info.get(dir).copied().unwrap_or(RecentInfo { exists: true, cloud: false });
                gone = !info.exists;
                let id = format!("recent-repository-{index}");
                let mut trailing = Vec::new();
                if info.cloud {
                    trailing.push(ui::chip_outline("cloud").into_any_element());
                }
                if gone {
                    trailing.push(ui::chip_outline("missing").into_any_element());
                }
                let forget = ui::icon_button(format!("forget-repository-{index}"), "✕")
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |app, _, _, cx| app.forget(index, cx)));
                // The button shows while the pointer is on the row, and always on the row Enter would open.
                trailing.push(
                    div()
                        .flex_shrink_0()
                        .when(!(selected || gone), |d| d.opacity(0.).group_hover(id.clone(), |s| s.opacity(1.)))
                        .child(forget)
                        .into_any_element(),
                );
                (id, folder_name(folder_of(dir)), shown(dir), trailing)
            }
        };
        let (name_color, path_color) = if gone { (t.fg_faint, t.fg_faint) } else { (t.fg, t.fg_muted) };
        let row = row.clone();
        ui::list_row(id.clone(), selected)
            .debug_selector({
                let id = id.clone();
                move || id
            })
            .group(id)
            .h(px(H_ROW))
            .flex()
            .items_center()
            .gap(px(S5))
            .when(self.loading, |d| d.opacity(0.5).cursor_default())
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(T_BODY_LG))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(name_color)
                            .truncate()
                            .child(title),
                    )
                    .child(div().text_size(px(T_SMALL)).text_color(path_color).truncate().child(subtitle)),
            )
            .children(trailing)
            .when(!self.loading, |d| {
                d.on_click(cx.listener(move |app, _, window, cx| {
                    app.highlight = position;
                    app.activate(&row, window, cx);
                }))
            })
    }

    /// What the card says before any repository was opened.
    fn welcome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current();
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(S5))
            .px(px(S8))
            .py(px(S8))
            .child(div().text_size(px(T_HEADING * 2.)).text_color(t.milestone).child("◆"))
            .child(div().text_size(px(T_HEADING)).font_weight(FontWeight::SEMIBOLD).child("Open a folder to start"))
            .child(div().max_w(px(380.)).text_center().text_color(t.fg_muted).child(
                "topo keeps tasks in a .topo folder inside your project. \
                     Git repositories and ordinary folders both work.",
            ))
            .child(
                ui::button_primary("open-folder", "Open folder…")
                    .child(on_primary_kbd("⌘O"))
                    .when(self.loading, |b| b.opacity(0.5).cursor_default())
                    .on_click(cx.listener(|app, _, window, cx| app.choose_folder(window, cx))),
            )
    }

    fn footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current();
        let hints = match self.editor {
            Some(_) => "↑↓ select · ↵ open · ⌘⌫ forget · esc close",
            None => "↑↓ select · ↵ open · ⌘⌫ forget",
        };
        div()
            .flex_shrink_0()
            .border_t_1()
            .border_color(t.hairline)
            .text_size(px(T_SMALL))
            .text_color(t.fg_faint)
            .when(self.editor.is_some(), |d| {
                d.child(div().px(px(S6)).pt(px(S4)).child("Unsubmitted edits are discarded only when you switch."))
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(S5))
                    .p(px(S4))
                    .when(!self.first_launch(), |d| {
                        d.child(
                            ui::button_ghost("open-folder", "Open folder…")
                                .child(ui::kbd("⌘O"))
                                .when(self.loading, |b| b.opacity(0.5).cursor_default())
                                .on_click(cx.listener(|app, _, window, cx| app.choose_folder(window, cx))),
                        )
                    })
                    .child(div().flex_1())
                    .child(div().min_w(px(0.)).px(px(S4)).truncate().child(hints)),
            )
    }
}

/// A section label with the inset of the rows' text.
fn section(label: &'static str) -> AnyElement {
    ui::section_label(label).px(px(S4)).into_any_element()
}

/// The frame of a status line between the field and the list.
fn banner_frame(tint: Hsla) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(S5))
        .px(px(S6))
        .py(px(S5))
        .border_t_1()
        .border_color(theme::current().hairline)
        .bg(tint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_home_folder_is_written_as_a_tilde() {
        let home = Path::new("/Users/me");
        assert_eq!(abbreviate_home(Path::new("/Users/me"), Some(home)), "~");
        assert_eq!(abbreviate_home(Path::new("/Users/me/src/app"), Some(home)), "~/src/app");
        // A sibling that merely shares the prefix is not inside the home folder.
        assert_eq!(abbreviate_home(Path::new("/Users/meow/app"), Some(home)), "/Users/meow/app");
        assert_eq!(abbreviate_home(Path::new("/srv/app"), Some(home)), "/srv/app");
        assert_eq!(abbreviate_home(Path::new("/Users/me/app"), None), "/Users/me/app");
    }

    #[test]
    fn long_text_loses_its_middle_and_keeps_more_of_the_end() {
        assert_eq!(truncate_middle("short", 10), "short");
        assert_eq!(truncate_middle("exactly10!", 10), "exactly10!");
        let cut = truncate_middle("~/a/very/long/path/to/project-name", 20);
        assert_eq!(cut.chars().count(), 20);
        assert_eq!(cut, "~/a/ve…/project-name");
        assert_eq!(truncate_middle("abcdef", 1), "…");
        assert_eq!(truncate_middle("abcdef", 0), "");
        // Characters, not bytes.
        assert_eq!(truncate_middle("日本語のフォルダー名", 5), "日…ダー名");
    }
}
