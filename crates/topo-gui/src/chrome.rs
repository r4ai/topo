//! Everything around the graph: toolbar, prompt, toasts, zoom controls and help.

use gpui::{
    AnyElement, App, Context, Div, ElementId, MouseButton, SharedString, Stateful, div, hsla, prelude::*, px, rgb,
};
use topo_core::{Kind, NodeId, Priority, Status};

use crate::theme::{self, button, icon_button, kbd};
use crate::{NewNode, OrganizeKind, Prompt, TopoApp, dates};

/// Entries of a search or pick list shown at once; the rest scrolls with the highlight.
const LIST_ROWS: usize = 8;

/// Swallows clicks so they do not reach the canvas underneath.
fn stop_click<E: InteractiveElement>(element: E) -> E {
    element.on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| cx.stop_propagation())
}

impl TopoApp {
    fn stat(
        &self,
        id: &'static str,
        icon: &'static str,
        text: String,
        color: u32,
        ids: Vec<NodeId>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let clickable = !ids.is_empty();
        div()
            .id(id)
            .debug_selector(|| id.to_owned())
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_1()
            .whitespace_nowrap()
            .h(px(24.))
            .px_2()
            .rounded_md()
            .text_xs()
            .text_color(rgb(theme::MUTED))
            .child(div().text_color(rgb(color)).child(icon))
            .child(text)
            .when(clickable, |d| {
                d.cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::RAISED)).text_color(rgb(theme::TEXT)))
                    .on_click(cx.listener(move |app, _, _, cx| app.select_nodes(&ids, cx)))
            })
    }

    fn stat_text(&self, count: usize, what: &str) -> String {
        match self.compact {
            true => count.to_string(),
            false => format!("{count} {what}"),
        }
    }

    pub(crate) fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let graph = self.graph();
        let today = dates::today();
        let tasks: Vec<_> = graph.nodes().filter(|n| n.kind == Kind::Task).collect();
        let ready: Vec<NodeId> =
            graph.ready_tasks(None).iter().filter(|n| n.status == Status::Todo).map(|n| n.id.clone()).collect();
        let doing: Vec<NodeId> = tasks.iter().filter(|n| n.status == Status::Doing).map(|n| n.id.clone()).collect();
        let overdue: Vec<NodeId> = graph
            .nodes()
            .filter(|n| !n.status.is_closed() && n.due.is_some_and(|d| d < today))
            .map(|n| n.id.clone())
            .collect();
        let closed = tasks.iter().filter(|n| n.status.is_closed()).count();
        let fraction = if tasks.is_empty() { 0. } else { closed as f32 / tasks.len() as f32 };
        let name = self
            .ws
            .dir()
            .parent()
            .and_then(|p| p.file_name())
            .map_or("workspace".into(), |n| n.to_string_lossy().into_owned());

        let organize = |id: &'static str, label: &'static str, what: OrganizeKind, cx: &mut Context<Self>| {
            button(id, if self.busy { "✦ Thinking…" } else { label })
                .when(self.busy, |b| b.opacity(0.6).cursor_default())
                .on_click(cx.listener(move |app, _, _, cx| app.run_organize(what, cx)))
        };
        let history = |id: &'static str, glyph: &'static str, undo: bool, enabled: bool, cx: &mut Context<Self>| {
            icon_button(id, glyph)
                .when(!enabled, |b| b.opacity(0.35).cursor_default())
                .on_click(cx.listener(move |app, _, _, cx| app.restore(undo, cx)))
        };

        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .when_else(self.compact, |d| d.gap_2(), |d| d.gap_3())
            .h(px(46.))
            .px_3()
            .bg(rgb(theme::SURFACE))
            .border_b_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w(px(0.))
                    .child(
                        div()
                            .debug_selector(|| "brand".into())
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap_2()
                            .child(div().text_color(rgb(theme::AMBER)).child("◆"))
                            .child(div().font_weight(gpui::FontWeight::SEMIBOLD).text_sm().child("topo")),
                    )
                    // The workspace name is the first thing to give way in a narrow window.
                    .child(div().text_color(rgb(theme::FAINT)).text_sm().child("/"))
                    .child(div().min_w(px(0.)).text_sm().text_color(rgb(theme::MUTED)).truncate().child(name)),
            )
            .child(div().flex_shrink_0().w(px(1.)).h(px(20.)).bg(rgb(theme::BORDER)))
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_1()
                    .child(self.stat("stat-ready", "●", self.stat_text(ready.len(), "ready"), theme::GREEN, ready, cx))
                    .when(!doing.is_empty(), |d| {
                        d.child(self.stat(
                            "stat-doing",
                            "◐",
                            self.stat_text(doing.len(), "in progress"),
                            theme::ACCENT,
                            doing,
                            cx,
                        ))
                    })
                    .when(!overdue.is_empty(), |d| {
                        d.child(self.stat(
                            "stat-overdue",
                            "⚠",
                            self.stat_text(overdue.len(), "overdue"),
                            theme::RED,
                            overdue,
                            cx,
                        ))
                    })
                    .when(!self.compact, |d| {
                        d.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_2()
                                .text_xs()
                                .text_color(rgb(theme::MUTED))
                                .child(div().w(px(64.)).flex().child(theme::progress_bar(fraction, theme::GREEN, 4.)))
                                .child(format!("{closed}/{} done", tasks.len())),
                        )
                    }),
            )
            .child(div().flex_1())
            .child(
                button("search", "Search")
                    .when(!self.compact, |b| b.child(kbd("⌘K")))
                    .on_click(cx.listener(|app, _, window, cx| app.open_prompt(Prompt::Search, "", window, cx))),
            )
            .child(
                button("new-task", "+ Task")
                    .when(!self.compact, |b| b.child(kbd("N")))
                    .on_click(cx.listener(|app, _, window, cx| app.prompt_create(Kind::Task, None, window, cx))),
            )
            .child(
                button("new-milestone", "+ Milestone")
                    .when(!self.compact, |b| b.child(kbd("M")))
                    .on_click(cx.listener(|app, _, window, cx| app.prompt_create(Kind::Milestone, None, window, cx))),
            )
            .child(div().w(px(1.)).h(px(20.)).bg(rgb(theme::BORDER)))
            .child(organize(
                "org-deps",
                if self.compact { "✦ Links" } else { "✦ Suggest links" },
                OrganizeKind::Deps,
                cx,
            ))
            .child(organize(
                "org-place",
                if self.compact { "✦ Place" } else { "✦ Place tasks" },
                OrganizeKind::Place,
                cx,
            ))
            .child(div().w(px(1.)).h(px(20.)).bg(rgb(theme::BORDER)))
            .child(history("undo", "↩\u{fe0e}", true, !self.undo.is_empty(), cx))
            .child(history("redo", "↪\u{fe0e}", false, !self.redo.is_empty(), cx))
            .child(icon_button("help", "?").on_click(cx.listener(|app, _, _, cx| {
                app.show_help = !app.show_help;
                cx.notify();
            })))
    }

    pub(crate) fn zoom_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let hint = match (&self.hovered, &self.selected) {
            (Some(_), _) => {
                "Double-click to rename · drag ● onto a node to connect, or onto empty space for a follow-up"
            }
            (None, Some(_)) if self.selected_nodes.len() > 1 => {
                "Space status · 1–4 set status · ⌘C copy · ⌫ delete selection · click a row to edit one"
            }
            (None, Some(_)) => "Tab follow-up · ⇧Tab prerequisite · L link · Space status · ⌫ delete · ←→↑↓ move",
            (None, None) => "N task · M milestone · / search · F fit · drag to pan · ? shortcuts",
        };
        let pill = || {
            div()
                .flex()
                .items_center()
                .gap_0p5()
                .p_0p5()
                .rounded_lg()
                .bg(rgb(theme::SURFACE))
                .border_1()
                .border_color(rgb(theme::BORDER))
                .shadow(theme::shadow())
        };
        div()
            .absolute()
            .bottom(px(14.))
            .left(px(14.))
            .right(px(14.))
            .flex()
            .items_end()
            .justify_between()
            .gap_4()
            .child(div().flex_1().min_w(px(0.)).truncate().text_xs().text_color(rgb(theme::FAINT)).child(hint))
            .child(stop_click(
                pill().child(
                    div()
                        .id("priority-filter")
                        .debug_selector(|| "priority-filter".to_owned())
                        .flex()
                        .items_center()
                        .gap_1()
                        .h(px(24.))
                        .px_2()
                        .rounded_md()
                        .whitespace_nowrap()
                        .text_xs()
                        .cursor_pointer()
                        .text_color(rgb(if self.priority_filter.is_some() { theme::TEXT } else { theme::MUTED }))
                        .hover(|s| s.bg(rgb(theme::RAISED)).text_color(rgb(theme::TEXT)))
                        .child(match self.priority_filter {
                            None => "Priority: all".to_owned(),
                            Some(Priority::Urgent) => "Priority: urgent".to_owned(),
                            Some(Priority::Low) => "Priority: any set".to_owned(),
                            Some(least) => format!("Priority: {least} and up"),
                        })
                        .when(!self.compact, |d| d.child(kbd("⇧P")))
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.cycle_priority_filter();
                            cx.notify();
                        })),
                ),
            ))
            .child(stop_click(
                pill()
                    .id("zoom")
                    .child(icon_button("zoom-out", "−").on_click(cx.listener(|app, _, _, cx| {
                        app.zoom_by(0.8, app.canvas_center(), true);
                        cx.notify();
                    })))
                    .child(
                        div()
                            .id("zoom-reset")
                            .w(px(44.))
                            .flex()
                            .justify_center()
                            .text_xs()
                            .text_color(rgb(theme::MUTED))
                            .cursor_pointer()
                            .hover(|s| s.text_color(rgb(theme::TEXT)))
                            .child(format!("{:.0}%", self.zoom * 100.))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.zoom_to_actual_size(app.canvas_center());
                                cx.notify();
                            })),
                    )
                    .child(icon_button("zoom-in", "+").on_click(cx.listener(|app, _, _, cx| {
                        app.zoom_by(1.25, app.canvas_center(), true);
                        cx.notify();
                    })))
                    .child(div().w(px(1.)).h(px(14.)).mx_0p5().bg(rgb(theme::BORDER)))
                    .child(icon_button("fit", "⤢").on_click(cx.listener(|app, _, _, cx| {
                        app.fit(false);
                        cx.notify();
                    }))),
            ))
    }

    fn node_chip(&self, prefix: &str, id: &NodeId) -> Div {
        let node = self.graph().get(id);
        let (icon, color) = node.map_or(("?", theme::FAINT), theme::node_icon);
        div()
            .flex()
            .items_center()
            .gap_1()
            .max_w(px(200.))
            .px_1p5()
            .py_0p5()
            .rounded_md()
            .bg(rgb(theme::CARD))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .text_xs()
            .text_color(rgb(theme::MUTED))
            .child(prefix.to_owned())
            .child(div().text_color(rgb(color)).child(icon))
            .child(div().truncate().text_color(rgb(theme::TEXT)).child(self.title_of(id)))
    }

    pub(crate) fn prompt_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let prompt = self.prompt.as_ref()?;
        let mut context: Vec<Div> = Vec::new();
        match prompt {
            Prompt::Create(NewNode { milestones, depends_on, required_by, .. }) => {
                context.extend(depends_on.iter().map(|id| self.node_chip("after", id)));
                context.extend(required_by.iter().map(|id| self.node_chip("before", id)));
                context.extend(milestones.iter().map(|id| self.node_chip("in", id)));
            }
            Prompt::Rename(id) | Prompt::Pick { node: id, .. } => context.push(self.node_chip("", id)),
            Prompt::Search => {}
        }
        let icon = match prompt {
            Prompt::Search | Prompt::Pick { .. } => "⌕",
            Prompt::Create(_) => "+",
            Prompt::Rename(_) => "✎",
        };
        let footer = match prompt {
            Prompt::Search => vec![("↵", "jump"), ("↑↓", "choose"), ("esc", "close")],
            Prompt::Pick { .. } => vec![("↵", "connect"), ("↑↓", "choose"), ("esc", "cancel")],
            Prompt::Create(_) => vec![("↵", "create"), ("esc", "cancel")],
            _ => vec![("↵", "save"), ("esc", "cancel")],
        };

        let body: Option<AnyElement> = match prompt {
            Prompt::Search | Prompt::Pick { .. } => {
                let listed = self.listed(cx);
                // The window of rows that keeps the highlighted one in view.
                let first = (self.search_index + 1).saturating_sub(LIST_ROWS);
                let rows: Vec<AnyElement> = listed
                    .iter()
                    .enumerate()
                    .skip(first)
                    .take(LIST_ROWS)
                    .map(|(i, id)| {
                        let node = self.graph().get(id).expect("the list comes from the graph");
                        let (icon, color) = theme::node_icon(node);
                        div()
                            .id(ElementId::Name(format!("result-{id}").into()))
                            .flex()
                            .items_center()
                            .gap_2()
                            .h(px(32.))
                            .px_3()
                            .rounded_md()
                            .when(i == self.search_index, |d| d.bg(rgb(theme::RAISED)))
                            .hover(|s| s.bg(rgb(theme::CARD_HOVER)))
                            .cursor_pointer()
                            .child(div().w(px(14.)).text_color(rgb(color)).child(icon))
                            .child(div().flex_1().min_w(px(0.)).truncate().text_sm().child(node.title.clone()))
                            .children(
                                node.tags
                                    .first()
                                    .map(|t| div().text_xs().text_color(rgb(theme::FAINT)).child(format!("#{t}"))),
                            )
                            .child(div().text_xs().text_color(rgb(theme::FAINT)).child(id.to_string()))
                            .on_click(cx.listener(move |app, _, window, cx| {
                                app.search_index = i;
                                app.submit_prompt(window, cx);
                            }))
                            .into_any_element()
                    })
                    .collect();
                let note = |text: String| div().px_3().py_1p5().text_xs().text_color(rgb(theme::FAINT)).child(text);
                let more = (listed.len() > LIST_ROWS)
                    .then(|| note(format!("{}–{} of {}", first + 1, first + rows.len(), listed.len())));
                Some(match rows.is_empty() {
                    true => note("No matches".to_owned()).into_any_element(),
                    false => div().flex().flex_col().p_1().children(rows).children(more).into_any_element(),
                })
            }
            _ => None,
        };

        let card = div()
            .id("prompt")
            .w(px(560.))
            .flex()
            .flex_col()
            .rounded_xl()
            .bg(rgb(theme::SURFACE))
            .border_1()
            .border_color(rgb(theme::BORDER_STRONG))
            .shadow(theme::shadow())
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1p5()
                    .px_4()
                    .pt_3()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::MUTED))
                            .child(prompt.title()),
                    )
                    .children(context),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_2p5()
                    .text_size(px(16.))
                    .child(div().flex_shrink_0().text_color(rgb(theme::ACCENT)).child(icon))
                    .child(self.input.clone()),
            )
            .children(body.map(|b| div().border_t_1().border_color(rgb(theme::BORDER)).child(b)))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .px_4()
                    .py_2()
                    .bg(rgb(theme::CANVAS))
                    .border_t_1()
                    .border_color(rgb(theme::BORDER))
                    .text_xs()
                    .text_color(rgb(theme::FAINT))
                    .children(
                        footer
                            .into_iter()
                            .map(|(k, what)| div().flex().items_center().gap_1().child(kbd(k)).child(what)),
                    ),
            );
        Some(
            div()
                .absolute()
                .top(px(18.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(stop_click(card.occlude()))
                .into_any_element(),
        )
    }

    pub(crate) fn toast_view(&self) -> Option<impl IntoElement> {
        let toast = self.toast.as_ref()?;
        let (icon, color) = if toast.error { ("⚠", theme::RED) } else { ("✓", theme::GREEN) };
        Some(
            div().absolute().bottom(px(58.)).left_0().right_0().flex().justify_center().child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .max_w(px(640.))
                    .px_3()
                    .py_2()
                    .rounded_lg()
                    .bg(rgb(theme::RAISED))
                    .border_1()
                    .border_color(if toast.error {
                        theme::alpha(theme::RED, 0.6)
                    } else {
                        rgb(theme::BORDER_STRONG).into()
                    })
                    .shadow(theme::shadow())
                    .text_sm()
                    .child(div().text_color(rgb(color)).child(icon))
                    .child(toast.text.clone()),
            ),
        )
    }

    pub(crate) fn empty_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div().absolute().size_full().flex().items_center().justify_center().child(stop_click(
            div()
                .id("empty")
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .child(div().text_size(px(40.)).text_color(rgb(theme::BORDER_STRONG)).child("◇"))
                .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Nothing planned yet"))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(theme::MUTED))
                        .child("Add a milestone for the goal, then the tasks it needs."),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .pt_2()
                        .child(button("empty-milestone", "+ Milestone").child(kbd("M")).on_click(
                            cx.listener(|app, _, window, cx| app.prompt_create(Kind::Milestone, None, window, cx)),
                        ))
                        .child(button("empty-task", "+ Task").child(kbd("N")).on_click(
                            cx.listener(|app, _, window, cx| app.prompt_create(Kind::Task, None, window, cx)),
                        )),
                ),
        ))
    }

    pub(crate) fn help_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let groups: [(&'static str, &'static [(&'static str, &'static str)]); 7] = [
            (
                "Create",
                &[
                    ("N", "New task (joins the selected milestone)"),
                    ("M", "New milestone"),
                    ("Tab", "Follow-up of the selection"),
                    ("⇧Tab", "Prerequisite of the selection"),
                ],
            ),
            (
                "Edit selection",
                &[
                    ("↵", "Rename (single selection)"),
                    ("Space", "Next status for selected nodes"),
                    ("X", "Toggle done for selected nodes"),
                    ("1–4", "Todo / Doing / Done / Dropped"),
                    ("P  A", "Priority / assignee, in the inspector"),
                    ("D  T", "Due date / tags, in the inspector"),
                    ("G", "Link a pull request"),
                    ("O", "Open the file (notes)"),
                    ("⌫", "Delete selected nodes"),
                ],
            ),
            (
                "Connect selection",
                &[
                    ("L", "Add a prerequisite"),
                    ("⇧L", "Add a node that requires it"),
                    ("I", "Add to a milestone / add a member"),
                ],
            ),
            (
                "Move around",
                &[
                    ("←→", "Select a prerequisite / dependent"),
                    ("↑↓", "Select a node in the same column"),
                    ("/  ⌘K", "Search"),
                    ("C", "Center / fit selection"),
                    ("Esc", "Clear selection"),
                ],
            ),
            ("View", &[("F", "Fit graph"), ("+ −", "Zoom"), ("0", "Actual size"), ("⇧P", "Dim below a priority")]),
            (
                "Standard shortcuts",
                &[
                    ("⌘A", "Canvas: select all nodes · field: all text"),
                    ("⌘C  ⌘X", "Canvas: copy / cut nodes · field: text"),
                    ("⌘V", "Canvas: paste nodes as new · field: text"),
                    ("⌘Z  ⇧⌘Z", "Canvas: undo / redo edits · field: typing"),
                    ("↵  Esc", "Field: save / cancel"),
                ],
            ),
            (
                "Mouse",
                &[
                    ("Drag ●", "Connect; on empty space: new follow-up"),
                    ("⇧Drag", "Connect from anywhere on a card"),
                    ("Double-click", "Rename"),
                    ("Drag / middle drag", "Pan canvas"),
                    ("⌘/Ctrl-click", "Add / remove from selection"),
                    ("Wheel", "Zoom at pointer"),
                    ("⇧Wheel", "Pan horizontally"),
                    ("⌘/Ctrl-wheel", "Pan vertically"),
                    ("Trackpad scroll", "Pan; ⌘/Ctrl to zoom"),
                    ("Pinch", "Zoom at gesture center"),
                    ("2-finger double-tap", "Fit / actual size"),
                ],
            ),
        ];
        let group = |(title, rows): (&'static str, &'static [(&'static str, &'static str)])| {
            div().flex().flex_col().gap_1p5().child(theme::section_label(title.to_uppercase()).pt_0()).children(
                rows.iter().map(|(keys, what)| {
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .child(div().w(px(132.)).flex_shrink_0().flex().child(kbd(*keys)))
                        .child(div().flex_1().min_w(px(0.)).text_color(rgb(theme::MUTED)).child(*what))
                }),
            )
        };
        let [create, edit, connect, nav, view, standard, mouse] = groups;
        div()
            .id("help")
            .absolute()
            .size_full()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(hsla(0., 0., 0., 0.55))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _, _, cx| {
                    cx.stop_propagation();
                    app.show_help = false;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("help-card")
                    .debug_selector(|| "help-card".to_owned())
                    .w(px(760.))
                    .max_h(gpui::relative(0.92))
                    .overflow_y_scroll()
                    .max_w(gpui::relative(0.92))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_6()
                    .rounded_xl()
                    .bg(rgb(theme::SURFACE))
                    .border_1()
                    .border_color(rgb(theme::BORDER_STRONG))
                    .shadow(theme::shadow())
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Keyboard shortcuts"))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(theme::FAINT))
                                    .child(SharedString::from("? or Esc to close")),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_8()
                            .when(self.compact, |d| d.flex_col())
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .flex()
                                    .flex_col()
                                    .gap_4()
                                    .child(group(create))
                                    .child(group(edit))
                                    .child(group(connect))
                                    .child(group(standard)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap_4()
                                    .child(group(nav))
                                    .child(group(view))
                                    .child(group(mouse)),
                            ),
                    ),
            )
    }
}
