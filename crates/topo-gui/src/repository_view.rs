//! Repository chooser presentation. Fixed header/footer and a scrolling body
//! keep the chooser usable even in a small window.

use gpui::{Context, IntoElement, Window, div, prelude::*, px, rgb};

use super::{RepositoryWindow, Selection};
use crate::theme;

impl RepositoryWindow {
    pub(super) fn chooser_view(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current();
        let current = self.editor.as_ref().map(|editor| editor.read(cx).ws.dir().to_owned());
        let recent_count = self.config.recent_workspaces.iter().filter(|path| current.as_ref() != Some(*path)).count();
        let height = (f32::from(window.viewport_size().height) - 48.).clamp(300., 680.);
        let caption = |text: &'static str| {
            div().text_xs().font_weight(gpui::FontWeight::SEMIBOLD).text_color(t.fg_muted).child(text)
        };
        div().size_full().flex().items_center().justify_center().p_6().child(
            div()
                .w_full()
                .max_w(px(640.))
                .h(px(height))
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded_xl()
                .border_1()
                .border_color(t.border)
                .bg(t.surface)
                .shadow(theme::shadow())
                .child(
                    div()
                        .flex_shrink_0()
                        .px_6()
                        .pt_6()
                        .pb_4()
                        .flex()
                        .items_start()
                        .gap_3()
                        .child(
                            div()
                                .w(px(40.))
                                .h(px(40.))
                                .flex_shrink_0()
                                .rounded_lg()
                                .bg(t.card)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_lg()
                                .text_color(t.warn)
                                .child("◆"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(div().text_xl().font_weight(gpui::FontWeight::SEMIBOLD).child("Open repository"))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(t.fg_muted)
                                        .child("Choose a folder, or continue in a recent repository."),
                                ),
                        ),
                )
                .child(
                    div()
                        .id("repository-chooser-body")
                        .flex_1()
                        .min_h(px(0.))
                        .overflow_y_scroll()
                        .px_6()
                        .pb_4()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .when_some(current.as_ref(), |d, path| {
                            let folder = path.parent().unwrap_or(path);
                            let name = folder
                                .file_name()
                                .map_or_else(|| folder.display().to_string(), |n| n.to_string_lossy().into_owned());
                            d.child(
                                div()
                                    .p_3()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(t.border)
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .flex_shrink_0()
                                    .child(caption("CURRENT REPOSITORY"))
                                    .child(
                                        div().text_sm().font_weight(gpui::FontWeight::SEMIBOLD).truncate().child(name),
                                    )
                                    .child(
                                        div()
                                            .id("current-repository-path")
                                            .overflow_x_scroll()
                                            .whitespace_nowrap()
                                            .text_xs()
                                            .text_color(t.fg_muted)
                                            .child(folder.display().to_string()),
                                    ),
                            )
                        })
                        .child(
                            div()
                                .id("open-folder")
                                .debug_selector(|| "open-folder".into())
                                .h(px(48.))
                                .flex_shrink_0()
                                .px_4()
                                .rounded_lg()
                                .flex()
                                .items_center()
                                .gap_3()
                                .bg(t.accent)
                                .text_color(t.bg)
                                .text_sm()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .when_else(
                                    self.loading,
                                    |d| d.opacity(0.65).cursor_default(),
                                    |d| d.cursor_pointer().hover(|s| s.bg(rgb(0x8abbff))),
                                )
                                .child(div().text_lg().child(if self.loading { "◌" } else { "+" }))
                                .child(if self.loading { "Opening repository…" } else { "Open folder…" })
                                .child(div().flex_1())
                                .child("→")
                                .on_click(cx.listener(|app, _, window, cx| app.choose_folder(window, cx))),
                        )
                        .when_some(self.error.as_ref(), |d, error| {
                            d.child(
                                div()
                                    .flex_shrink_0()
                                    .p_3()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(theme::alpha(t.danger, 0.4))
                                    .bg(theme::alpha(t.danger, 0.06))
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(t.danger)
                                            .child("Couldn't open repository"),
                                    )
                                    .child(
                                        div()
                                            .id("repository-error-detail")
                                            .overflow_x_scroll()
                                            .whitespace_nowrap()
                                            .text_xs()
                                            .text_color(t.fg_muted)
                                            .child(error.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(t.fg_muted)
                                            .child("Choose another folder or try a recent repository."),
                                    ),
                            )
                        })
                        .when_some(self.pending.as_ref(), |d, path| {
                            let folder = path.clone();
                            let name = path
                                .file_name()
                                .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
                            d.child(
                                div()
                                    .flex_shrink_0()
                                    .p_4()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(theme::alpha(t.warn, 0.4))
                                    .bg(theme::alpha(t.warn, 0.05))
                                    .flex()
                                    .flex_col()
                                    .items_start()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child(format!("Set up {name}")),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(t.fg_muted)
                                            .child("This folder doesn't have a topo workspace yet."),
                                    )
                                    .child(
                                        div()
                                            .id("pending-repository-path")
                                            .w_full()
                                            .overflow_x_scroll()
                                            .whitespace_nowrap()
                                            .text_xs()
                                            .text_color(t.fg_muted)
                                            .child(path.display().to_string()),
                                    )
                                    .child(
                                        theme::button("initialize-repository", "Initialize workspace")
                                            .when(self.loading, |b| b.opacity(0.5).cursor_default())
                                            .on_click(cx.listener(move |app, _, window, cx| {
                                                app.open_path(folder.clone(), true, window, cx)
                                            })),
                                    ),
                            )
                        })
                        .child(
                            div()
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(caption("RECENT REPOSITORIES"))
                                .child(
                                    div()
                                        .px_1p5()
                                        .rounded_md()
                                        .bg(t.raised)
                                        .text_xs()
                                        .text_color(t.fg_muted)
                                        .child(recent_count.to_string()),
                                )
                                .child(div().flex_1())
                                .when(recent_count > 3, |d| {
                                    d.child(div().text_xs().text_color(t.fg_muted).child("Scroll ↓"))
                                }),
                        )
                        .child(
                            div()
                                .id("recent-repositories")
                                .flex_shrink_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .when(recent_count == 0, |d| {
                                    d.child(
                                        div()
                                            .p_4()
                                            .rounded_lg()
                                            .border_1()
                                            .border_color(t.border)
                                            .text_sm()
                                            .text_color(t.fg_muted)
                                            .child(if current.is_some() {
                                                "Open a folder to add another repository."
                                            } else {
                                                "Repositories you open will appear here."
                                            }),
                                    )
                                })
                                .children(
                                    self.config
                                        .recent_workspaces
                                        .iter()
                                        .enumerate()
                                        .filter(|(_, path)| current.as_ref() != Some(*path))
                                        .map(|(index, path)| {
                                            let path = path.clone();
                                            let folder = path.parent().unwrap_or(&path);
                                            let name = folder.file_name().map_or_else(
                                                || folder.display().to_string(),
                                                |n| n.to_string_lossy().into_owned(),
                                            );
                                            div()
                                                .id(("recent-repository", index))
                                                .debug_selector(move || format!("recent-repository-{index}"))
                                                .flex()
                                                .flex_shrink_0()
                                                .items_center()
                                                .gap_3()
                                                .p_3()
                                                .rounded_lg()
                                                .border_1()
                                                .border_color(t.border)
                                                .bg(t.card)
                                                .when_else(
                                                    self.loading,
                                                    |d| d.opacity(0.5).cursor_default(),
                                                    |d| {
                                                        d.cursor_pointer()
                                                            .hover(|s| s.bg(t.raised).border_color(t.border_strong))
                                                    },
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w(px(0.))
                                                        .flex()
                                                        .flex_col()
                                                        .gap_1()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                                .truncate()
                                                                .child(name),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(t.fg_muted)
                                                                .truncate()
                                                                .child(folder.display().to_string()),
                                                        ),
                                                )
                                                .child(div().text_sm().text_color(t.fg_muted).child("→"))
                                                .on_click(cx.listener(move |app, _, window, cx| {
                                                    app.open_selection(
                                                        Selection { path: path.clone(), exact_workspace: true },
                                                        false,
                                                        window,
                                                        cx,
                                                    )
                                                }))
                                        }),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .border_t_1()
                        .border_color(t.border)
                        .px_6()
                        .py_3()
                        .flex()
                        .items_center()
                        .gap_4()
                        .child(div().flex_1().text_xs().text_color(t.fg_muted).child(if current.is_some() {
                            "Unsubmitted edits are discarded only when you switch."
                        } else {
                            "Git repositories and ordinary folders are supported."
                        }))
                        .when(current.is_some() || self.loading, |d| {
                            d.child(
                                theme::button("cancel-repository", "Cancel")
                                    .on_click(cx.listener(|app, _, window, cx| app.cancel(window, cx))),
                            )
                        }),
                ),
        )
    }
}
