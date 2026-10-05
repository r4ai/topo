//! Everything around the graph: toolbar, prompt, toasts, zoom controls and help.

use gpui::{
    Anchor, AnyElement, App, Context, Div, MouseButton, Pixels, Rgba, SharedString, Stateful, Window,
    WindowControlArea, anchored, deferred, div, prelude::*, px,
};
use topo_core::{Kind, NodeId, Priority};

use crate::theme::{
    self,
    metrics::{H_BUTTON, H_ICON, R_LG, R_SM, R_XL, T_BODY, T_BODY_LG, T_HEADING, T_SMALL, T_TITLE},
};
use crate::ui::{self, Elevation};
use crate::{NewNode, OrganizeKind, Prompt, TopoApp};

/// Swallows clicks so they do not reach the canvas underneath.
fn stop_click<E: InteractiveElement>(element: E) -> E {
    element.on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| cx.stop_propagation())
}

const TOOLBAR_H: f32 = 46.;
/// The macOS traffic lights occupy the leftmost part of the toolbar. The wordmark keeps clear of
/// them with this much room, so the two never crowd each other on a real window.
const TRAFFIC_LIGHTS: f32 = 94.;

/// Space the toolbar and the repository chooser leave for the macOS traffic lights; zero when there are none
/// (other platforms, fullscreen) and under test, so layout tests do not depend on the host.
pub(crate) fn titlebar_inset(window: &Window) -> Pixels {
    #[cfg(test)]
    let _ = window;
    #[cfg(test)]
    let inset = test_inset::get();
    #[cfg(not(test))]
    let inset = if cfg!(target_os = "macos") && !window.is_fullscreen() { TRAFFIC_LIGHTS } else { 0. };
    px(inset)
}

/// An empty strip that moves the window, standing in for the titlebar above the repository chooser.
pub(crate) fn drag_strip() -> Div {
    div().flex_shrink_0().w_full().h(px(TOOLBAR_H)).window_control_area(WindowControlArea::Drag).on_mouse_down(
        MouseButton::Left,
        |event, window, _| match event.click_count {
            2 => window.titlebar_double_click(),
            _ => window.start_window_move(),
        },
    )
}

impl TopoApp {
    fn stat(
        &self,
        id: &'static str,
        icon: &'static str,
        text: String,
        color: Rgba,
        ids: Vec<NodeId>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let t = theme::current();
        let clickable = !ids.is_empty();
        stop_click(div().id(id))
            .debug_selector(|| id.to_owned())
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_1()
            .whitespace_nowrap()
            .h(px(24.))
            .px_2()
            .rounded(px(R_SM))
            .text_size(px(T_SMALL))
            .text_color(if color == t.danger { t.danger } else { t.fg_muted })
            .child(div().text_color(color).child(icon))
            .child(text)
            .when(clickable, |d| {
                d.cursor_pointer()
                    .hover(|s| s.bg(t.control_hover).text_color(if color == t.danger { t.danger } else { t.fg }))
                    .on_click(cx.listener(move |app, _, _, cx| app.select_nodes(&ids, cx)))
            })
    }

    fn stat_text(&self, count: usize, what: &str) -> String {
        match self.compact {
            true => count.to_string(),
            false => format!("{count} {what}"),
        }
    }

    pub(crate) fn toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current();
        let ready = self.graph_cache.ready.clone();
        let doing = self.graph_cache.doing.clone();
        let overdue = self.graph_cache.overdue.clone();
        let tasks = self.graph_cache.task_count;
        let closed = self.graph_cache.closed_count;
        let fraction = if tasks == 0 { 0. } else { closed as f32 / tasks as f32 };
        let name = self
            .ws
            .dir()
            .parent()
            .and_then(|p| p.file_name())
            .map_or("workspace".into(), |n| n.to_string_lossy().into_owned());

        let organize = |id: &'static str, label: &'static str, what: OrganizeKind, cx: &mut Context<Self>| {
            stop_click(ui::button_ghost(id, if self.busy { "✦ Thinking…" } else { label }))
                .when(self.busy, |b| b.opacity(0.6).cursor_default())
                .on_click(cx.listener(move |app, _, _, cx| app.run_organize(what, cx)))
        };
        let history = |id: &'static str, glyph: &'static str, undo: bool, enabled: bool, cx: &mut Context<Self>| {
            stop_click(match enabled {
                true => ui::icon_button(id, glyph),
                false => div()
                    .id(id)
                    .debug_selector(|| id.to_owned())
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .size(px(H_ICON))
                    .text_sm()
                    .text_color(t.fg_faint)
                    .child(glyph),
            })
            .on_click(cx.listener(move |app, _, _, cx| app.restore(undo, cx)))
        };

        let inset = titlebar_inset(window);
        // Only macOS has no system titlebar; the toolbar's empty space moves the window there.
        let draggable = cfg!(target_os = "macos");
        ui::panel()
            .debug_selector(|| "toolbar".into())
            .flex()
            .flex_shrink_0()
            .items_center()
            .when_else(self.compact, |d| d.gap_2(), |d| d.gap_3())
            .h(px(TOOLBAR_H))
            .px_3()
            .when(inset > px(0.), |d| d.pl(inset))
            .when(draggable, |d| {
                d.window_control_area(WindowControlArea::Drag)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|app, event: &gpui::MouseDownEvent, window, _| match event.click_count {
                            2 => window.titlebar_double_click(),
                            _ => app.toolbar_press = true,
                        }),
                    )
                    .on_mouse_move(cx.listener(|app, _, window, _| {
                        if std::mem::take(&mut app.toolbar_press) {
                            window.start_window_move();
                        }
                    }))
                    .on_mouse_up(MouseButton::Left, cx.listener(|app, _, _, _| app.toolbar_press = false))
                    .on_mouse_down_out(cx.listener(|app, _, _, _| app.toolbar_press = false))
            })
            .border_b_1()
            .border_color(t.hairline)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .child(
                        div()
                            .debug_selector(|| "brand".into())
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .child(div().font_weight(gpui::FontWeight::SEMIBOLD).text_sm().child("topo")),
                    )
                    // The workspace name is the first thing to give way in a narrow window.
                    .child(div().flex_shrink_0().text_color(t.fg_faint).text_sm().child("/"))
                    .child(
                        stop_click(
                            ui::button_ghost("repository", "")
                                .text_color(t.fg)
                                .child(div().min_w(px(0.)).truncate().child(name))
                                .child("▾"),
                        )
                        // The button is `flex_shrink_0` by default; this one gives way, down to a stub.
                        .flex_shrink(1.)
                        .min_w(px(80.))
                        .max_w(px(if self.compact { 110. } else { 220. }))
                        .overflow_hidden()
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(crate::repository::OpenRepository))),
                    ),
            )
            .child(div().flex_shrink_0().w(px(1.)).h(px(20.)).bg(t.hairline))
            .child(
                // The first to give way. A single-row wrapping box clips whole items: the ones that do not fit wrap
                // onto a second row that `overflow_hidden` cuts off. The huge shrink factor makes it yield all its
                // width before anything else starts to shrink.
                div()
                    .debug_selector(|| "toolbar-stats".into())
                    .flex()
                    .flex_wrap()
                    .content_start()
                    .items_center()
                    .gap_1()
                    .h(px(H_BUTTON))
                    .min_w(px(0.))
                    .overflow_hidden()
                    .flex_shrink(1000.)
                    .child(self.stat("stat-ready", "●", self.stat_text(ready.len(), "ready"), t.ready, ready, cx))
                    .when(!doing.is_empty(), |d| {
                        d.child(self.stat(
                            "stat-doing",
                            "◐",
                            self.stat_text(doing.len(), "in progress"),
                            t.accent,
                            doing,
                            cx,
                        ))
                    })
                    .when(!overdue.is_empty(), |d| {
                        d.child(self.stat(
                            "stat-overdue",
                            "⚠",
                            self.stat_text(overdue.len(), "overdue"),
                            t.danger,
                            overdue,
                            cx,
                        ))
                    })
                    .when(!self.compact, |d| {
                        d.child(
                            div()
                                .flex()
                                .flex_shrink_0()
                                .items_center()
                                .gap_2()
                                .px_2()
                                .whitespace_nowrap()
                                .text_size(px(T_SMALL))
                                .text_color(t.fg_muted)
                                .child(div().w(px(64.)).flex().child(ui::progress_bar(
                                    fraction,
                                    closed == tasks && tasks > 0,
                                    4.,
                                )))
                                .child(format!("{closed}/{} done", tasks)),
                        )
                    }),
            )
            .child(div().flex_1())
            .child(
                stop_click(ui::button("search", "⌕ Search"))
                    .when(!self.compact, |b| b.child(ui::kbd("⌘K")))
                    .on_click(cx.listener(|app, _, window, cx| app.open_prompt(Prompt::Search, window, cx))),
            )
            .child(
                stop_click(ui::button("new-task", "+ Task"))
                    .when(!self.compact, |b| b.child(ui::kbd("N")))
                    .on_click(cx.listener(|app, _, window, cx| app.prompt_create(Kind::Task, None, window, cx))),
            )
            .child(
                stop_click(ui::button("new-milestone", "+ Milestone"))
                    .when(!self.compact, |b| b.child(ui::kbd("M")))
                    .on_click(cx.listener(|app, _, window, cx| app.prompt_create(Kind::Milestone, None, window, cx))),
            )
            // Next to give way, the same way as the stats but later. Shown only when the workspace
            // opts into a Jev model (a `[jev]` table in `.topo/config.toml`).
            .when(self.jev_enabled, |d| {
                d.child(div().flex_shrink_0().w(px(1.)).h(px(20.)).bg(t.hairline)).child(
                    div()
                        .debug_selector(|| "toolbar-organize".into())
                        .flex()
                        .flex_wrap()
                        .content_start()
                        .items_center()
                        .gap_2()
                        .h(px(H_BUTTON))
                        .min_w(px(0.))
                        .overflow_hidden()
                        .flex_shrink(100.)
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
                        .child(div().flex_shrink_0().w(px(1.)).h(px(20.)).bg(t.hairline)),
                )
            })
            .child(history("undo", "↩\u{fe0e}", true, !self.undo.is_empty(), cx))
            .child(history("redo", "↪\u{fe0e}", false, !self.redo.is_empty(), cx))
            .child(
                div()
                    .relative()
                    .flex_shrink_0()
                    .child(stop_click(ui::icon_button("theme", theme_glyph(theme::mode()))).on_click(cx.listener(
                        |app, _, _, cx| {
                            app.theme_menu = !app.theme_menu;
                            cx.notify();
                        },
                    )))
                    .when(self.theme_menu, |d| d.child(self.theme_menu_popover(cx))),
            )
            .child(stop_click(ui::icon_button("help", "?")).on_click(cx.listener(|app, _, _, cx| {
                app.show_help = !app.show_help;
                cx.notify();
            })))
    }

    /// The popover under the theme button: the mode, and the Monotone switch.
    fn theme_menu_popover(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut modes = ui::segmented();
        for (id, mode) in [
            ("theme-system", theme::ThemeMode::System),
            ("theme-light", theme::ThemeMode::Light),
            ("theme-dark", theme::ThemeMode::Dark),
        ] {
            modes = modes.child(ui::segment(id, mode.label(), theme::mode() == mode).flex_1().on_click(cx.listener(
                move |app, _, _, cx| {
                    theme::set_mode(mode, cx);
                    app.toast(format!("Theme: {}", mode.label()), false, cx);
                },
            )));
        }
        let monotone = theme::monotone();
        let menu = ui::glass(Elevation::Popover)
            .id("theme-menu")
            .debug_selector(|| "theme-menu".to_owned())
            .occlude()
            .w(px(240.))
            .flex()
            .flex_col()
            .gap_1()
            .p_1()
            .text_size(px(T_BODY))
            .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| cx.stop_propagation())
            // A click away only closes the menu; it does not reach what is under it.
            .on_mouse_down_out(cx.listener(|app, _, _, cx| {
                app.theme_menu = false;
                cx.stop_propagation();
                cx.notify();
            }))
            .child(modes)
            .child(
                ui::list_row("theme-monotone", false)
                    .debug_selector(|| "theme-monotone".to_owned())
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(H_BUTTON))
                    .child("Monotone")
                    .child(div().debug_selector(|| "theme-monotone-switch".to_owned()).child(ui::switch(monotone)))
                    .on_click(cx.listener(|app, _, _, cx| {
                        let on = !theme::monotone();
                        theme::set_monotone(on, cx);
                        app.toast(format!("Monotone {}", if on { "on" } else { "off" }), false, cx);
                    })),
            );
        div().absolute().right(px(0.)).top_full().mt_1().child(
            deferred(anchored().anchor(Anchor::TopRight).snap_to_window_with_margin(px(8.)).child(menu))
                .with_priority(1),
        )
    }

    /// The save status as a floating overlay, so showing it never moves the canvas.
    pub(crate) fn persistence_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current();
        let failed = self.persistence.error.is_some();
        let bar = ui::toast_frame()
            .debug_selector(|| "save-status".into())
            .flex()
            .flex_wrap()
            .items_center()
            .justify_center()
            .gap_2()
            .text_size(px(T_SMALL))
            .text_color(if failed { t.danger } else { t.warn })
            .child(if failed { "Changes need confirmation" } else { "Saving changes…" })
            .when(failed, |d| {
                d.child(
                    ui::button("retry-save", "Retry")
                        .debug_selector(|| "retry-save".into())
                        .on_click(cx.listener(|app, _, _, cx| app.retry_save(cx))),
                )
                .child(
                    ui::button("discard-save", "Discard drafts")
                        .debug_selector(|| "discard-save".into())
                        .on_click(cx.listener(|app, _, _, cx| app.discard_pending(cx))),
                )
            });
        div().absolute().top(px(12.)).left_0().right_0().flex().justify_center().px_3().child(stop_click(bar))
    }

    pub(crate) fn zoom_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current();
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
        let pill = || ui::glass(Elevation::Popover).rounded(px(R_LG)).flex().items_center().gap_0p5().p_0p5();
        div()
            .absolute()
            .bottom(px(14.))
            .left(px(14.))
            .right(px(14.))
            .flex()
            .items_end()
            .justify_between()
            .gap_4()
            .child(div().flex_1().min_w(px(0.)).truncate().text_size(px(T_SMALL)).text_color(t.fg_faint).child(hint))
            .child(stop_click(
                pill()
                    .child(
                        self.view_toggle("hide-completed", ("Hide done", "Hide"), "⇧H", self.view.hide_completed)
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.toggle_hide_completed();
                                cx.notify();
                            })),
                    )
                    .child(
                        self.view_toggle("group-by-tag", ("Group by tag", "Group"), "⇧G", self.view.group_by_tag)
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.toggle_group_by_tag();
                                cx.notify();
                            })),
                    )
                    .child(div().w(px(1.)).h(px(14.)).mx_0p5().bg(t.hairline))
                    .child(
                        div()
                            .id("priority-filter")
                            .debug_selector(|| "priority-filter".to_owned())
                            .flex()
                            .items_center()
                            .gap_1()
                            .h(px(24.))
                            .px_2()
                            .rounded(px(R_SM))
                            .whitespace_nowrap()
                            .text_size(px(T_SMALL))
                            .cursor_pointer()
                            .map(|d| match self.priority_filter.is_some() {
                                true => d.bg(t.control_active).text_color(t.fg),
                                false => d.text_color(t.fg_muted).hover(|s| s.bg(t.control_hover)),
                            })
                            .child(match (self.priority_filter, self.compact) {
                                (None, true) => "Priority".to_owned(),
                                (None, false) => "Priority: all".to_owned(),
                                (Some(Priority::Urgent), true) => "Urgent".to_owned(),
                                (Some(Priority::Urgent), false) => "Priority: urgent".to_owned(),
                                (Some(Priority::Low), true) => "Any set".to_owned(),
                                (Some(Priority::Low), false) => "Priority: any set".to_owned(),
                                (Some(least), true) => format!("{least}+"),
                                (Some(least), false) => format!("Priority: {least} and up"),
                            })
                            .when(!self.compact, |d| d.child(ui::kbd("⇧P")))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.cycle_priority_filter();
                                cx.notify();
                            })),
                    ),
            ))
            .child(stop_click(
                pill()
                    .id("zoom")
                    .debug_selector(|| "zoom".to_owned())
                    .child(ui::icon_button("zoom-out", "−").on_click(cx.listener(|app, _, _, cx| {
                        app.zoom_by(0.8, app.canvas_center(), true);
                        cx.notify();
                    })))
                    .child(
                        div()
                            .id("zoom-reset")
                            .w(px(44.))
                            .flex()
                            .justify_center()
                            .text_size(px(T_SMALL))
                            .text_color(t.fg_muted)
                            .cursor_pointer()
                            .hover(|s| s.text_color(t.fg))
                            .child(format!("{:.0}%", self.zoom * 100.))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.zoom_to_actual_size(app.canvas_center());
                                cx.notify();
                            })),
                    )
                    .child(ui::icon_button("zoom-in", "+").on_click(cx.listener(|app, _, _, cx| {
                        app.zoom_by(1.25, app.canvas_center(), true);
                        cx.notify();
                    })))
                    .child(div().w(px(1.)).h(px(14.)).mx_0p5().bg(t.hairline))
                    .child(ui::icon_button("fit", "⤢").on_click(cx.listener(|app, _, _, cx| {
                        app.fit(false);
                        cx.notify();
                    }))),
            ))
    }

    /// A switch in the bottom bar; lit while `on`. `label` is the full text and the compact one.
    fn view_toggle(
        &self,
        id: &'static str,
        label: (&'static str, &'static str),
        key: &'static str,
        on: bool,
    ) -> Stateful<Div> {
        let t = theme::current();
        div()
            .id(id)
            .debug_selector(|| id.to_owned())
            .flex()
            .items_center()
            .gap_1()
            .h(px(24.))
            .px_2()
            .rounded(px(R_SM))
            .whitespace_nowrap()
            .text_size(px(T_SMALL))
            .cursor_pointer()
            .map(|d| match on {
                true => d.bg(t.control_active).text_color(t.fg),
                false => d.text_color(t.fg_muted).hover(|s| s.bg(t.control_hover)),
            })
            .child(if self.compact { label.1 } else { label.0 })
            .when(!self.compact, |d| d.child(ui::kbd(key)))
    }

    fn node_chip(&self, prefix: &str, id: &NodeId) -> Div {
        let t = theme::current();
        let node = self.graph().get(id);
        let (icon, color) = node.map_or(("?", t.fg_faint), theme::node_icon);
        div()
            .flex()
            .items_center()
            .gap_1()
            .max_w(px(200.))
            .px_1p5()
            .py_0p5()
            .rounded(px(R_SM))
            .bg(t.control)
            .text_size(px(T_SMALL))
            .text_color(t.fg_muted)
            .child(prefix.to_owned())
            .child(div().text_color(color).child(icon))
            .child(div().truncate().text_color(t.fg).child(self.title_of(id)))
    }

    pub(crate) fn prompt_overlay(&self) -> Option<AnyElement> {
        let t = theme::current();
        let prompt = self.prompt.as_ref()?;
        let mut context: Vec<Div> = Vec::new();
        match prompt {
            Prompt::Create(NewNode { milestones, depends_on, required_by, .. }) => {
                context.extend(depends_on.iter().map(|id| self.node_chip("after", id)));
                context.extend(required_by.iter().map(|id| self.node_chip("before", id)));
                context.extend(milestones.iter().map(|id| self.node_chip("in", id)));
            }
            Prompt::Pick { node: id, .. } => context.push(self.node_chip("", id)),
            Prompt::Search => {}
        }
        let footer = match prompt {
            Prompt::Search => vec![("↵", "jump"), ("↑↓", "choose"), ("esc", "close")],
            Prompt::Pick { .. } => vec![("↵", "connect"), ("↑↓", "choose"), ("esc", "cancel")],
            Prompt::Create(_) => vec![("↵", "create"), ("esc", "cancel")],
        };

        let card = ui::glass(Elevation::Dialog)
            .id("prompt")
            .debug_selector(|| "prompt-card".to_owned())
            .w(px(560.))
            .flex()
            .flex_col()
            .rounded(px(R_XL))
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
                            .text_size(px(T_SMALL))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(t.fg_muted)
                            .child(prompt.title()),
                    )
                    .children(context),
            )
            // The field and, under it, the nodes that match.
            .child(div().text_size(px(T_TITLE)).child(self.palette.clone()))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(t.hairline)
                    .text_size(px(T_SMALL))
                    .text_color(t.fg_faint)
                    .children(
                        footer
                            .into_iter()
                            .map(|(k, what)| div().flex().items_center().gap_1().child(ui::kbd(k)).child(what)),
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
        let t = theme::current();
        let toast = self.toast.as_ref()?;
        let (icon, color) = if toast.error { ("⚠", t.danger) } else { ("✓", t.success) };
        Some(
            div().absolute().bottom(px(58.)).left_0().right_0().flex().justify_center().child(
                ui::toast_frame()
                    .debug_selector(|| "toast".to_owned())
                    .flex()
                    .items_center()
                    .gap_2()
                    .max_w(px(640.))
                    .text_size(px(T_BODY_LG))
                    .text_color(t.fg)
                    .child(div().flex_shrink_0().text_color(color).child(icon))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .debug_selector(|| "toast-text".to_owned())
                            .child(toast.text.clone()),
                    ),
            ),
        )
    }

    pub(crate) fn empty_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current();
        div().absolute().size_full().flex().items_center().justify_center().child(stop_click(
            div()
                .id("empty")
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .child(div().text_size(px(40.)).text_color(t.fg_faint).child("◇"))
                .child(
                    div()
                        .text_size(px(T_HEADING))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(t.fg)
                        .child("Nothing planned yet"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(t.fg_muted)
                        .child("Add a milestone for the goal, then the tasks it needs."),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .pt_2()
                        .child(
                            ui::button_primary("empty-milestone", "+ Milestone").child(on_primary_kbd("M")).on_click(
                                cx.listener(|app, _, window, cx| app.prompt_create(Kind::Milestone, None, window, cx)),
                            ),
                        )
                        .child(ui::button("empty-task", "+ Task").child(ui::kbd("N")).on_click(
                            cx.listener(|app, _, window, cx| app.prompt_create(Kind::Task, None, window, cx)),
                        )),
                ),
        ))
    }

    /// What the canvas says when the nodes exist but all of them are hidden.
    pub(crate) fn all_hidden_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current();
        div().absolute().size_full().flex().items_center().justify_center().child(stop_click(
            div()
                .id("all-hidden")
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .text_size(px(T_HEADING))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(t.fg)
                        .child("Everything is done"),
                )
                .child(div().text_sm().text_color(t.fg_muted).child("Done and dropped nodes are hidden."))
                .child(ui::button_primary("show-completed", "Show them").child(on_primary_kbd("⇧H")).on_click(
                    cx.listener(|app, _, _, cx| {
                        app.toggle_hide_completed();
                        cx.notify();
                    }),
                )),
        ))
    }

    pub(crate) fn help_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current();
        let groups: [(&'static str, &'static [(&'static str, &'static str)]); 8] = [
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
                    ("↵", "Edit the title (single selection)"),
                    ("Space", "Next status for selected nodes"),
                    ("X", "Toggle done for selected nodes"),
                    ("1–4", "Todo / Doing / Done / Dropped"),
                    ("P  A", "Priority / assignee, in the inspector"),
                    ("D  T", "Due date / tags, in the inspector"),
                    ("G", "Link a pull request"),
                    ("E", "Edit the notes (⌘↵ saves, Esc closes)"),
                    ("O", "Open the notes file in an editor"),
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
            (
                "View",
                &[
                    ("F", "Fit graph"),
                    ("+ −", "Zoom"),
                    ("0", "Actual size"),
                    ("⇧P", "Dim below a priority"),
                    ("⇧H", "Hide done and dropped nodes"),
                    ("⇧G", "Group nodes by tag"),
                ],
            ),
            (
                "Notes editor",
                &[
                    ("⌘↵", "Save and close"),
                    ("Esc", "Close; with changes, ask to save or discard"),
                    ("Home  End", "Start / end of the line (⌘← ⌘→, ⌃A ⌃E)"),
                    ("⌘↑  ⌘↓", "Start / end of the notes"),
                    ("⌥←  ⌥→", "Previous / next word"),
                    ("⇧ + any move", "Extend the selection"),
                    ("Tab  ⇧Tab", "Indent / outdent the lines selected"),
                    ("↵  ⇧↵", "New line, continuing a list / not"),
                ],
            ),
            (
                "Standard shortcuts",
                &[
                    ("⌘O / Ctrl+O", "Open repository / recent repositories"),
                    ("⌘A", "Canvas: select all nodes · field: all text"),
                    ("⌘C  ⌘X", "Canvas: copy / cut nodes · field: text"),
                    ("⌘V", "Canvas: paste nodes as new · field: text"),
                    ("⌘Z  ⇧⌘Z", "Canvas: undo / redo edits · field: typing"),
                    ("↵  Esc", "Field: save / close the list, then cancel"),
                    ("Tab  ⇧Tab", "Inspector field: save, next / previous"),
                    ("↓ ↑", "Inspector field: choose a suggestion"),
                ],
            ),
            (
                "Mouse",
                &[
                    ("Drag ●", "Connect; on empty space: new follow-up"),
                    ("⇧Drag", "Connect from anywhere on a card"),
                    ("Double-click", "Edit the title"),
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
            div().flex().flex_col().gap_1p5().child(ui::section_label(title.to_uppercase()).pt_0()).children(
                rows.iter().map(|(keys, what)| {
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(T_SMALL))
                        .child(div().w(px(132.)).flex_shrink_0().flex().child(ui::kbd(*keys)))
                        .child(div().flex_1().min_w(px(0.)).text_color(t.fg_muted).child(*what))
                }),
            )
        };
        let [create, edit, connect, nav, view, notes, standard, mouse] = groups;
        ui::scrim()
            .id("help")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _, _, cx| {
                    cx.stop_propagation();
                    app.show_help = false;
                    cx.notify();
                }),
            )
            .child(
                ui::dialog()
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
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(T_HEADING))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("Keyboard shortcuts"),
                            )
                            .child(
                                div()
                                    .text_size(px(T_SMALL))
                                    .text_color(t.fg_faint)
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
                                    .child(group(notes))
                                    .child(group(mouse)),
                            ),
                    ),
            )
    }
}

/// A shortcut hint that stays legible on the primary fill.
pub(crate) fn on_primary_kbd(keys: &'static str) -> Div {
    let t = theme::current();
    ui::kbd(keys).text_color(t.on_emphasis).border_color(theme::alpha(t.on_emphasis, 0.3))
}

/// The toolbar glyph that shows `mode`.
fn theme_glyph(mode: theme::ThemeMode) -> &'static str {
    match mode {
        theme::ThemeMode::System => "◐",
        theme::ThemeMode::Light => "☀",
        theme::ThemeMode::Dark => "☾",
    }
}

/// The traffic-light inset a test can ask for with [`test_inset::set`].
#[cfg(test)]
pub(crate) const TEST_TRAFFIC_LIGHTS: f32 = TRAFFIC_LIGHTS;

/// Lets a test pretend the traffic lights are there; the default stays zero so tests do not depend on the host.
#[cfg(test)]
pub(crate) mod test_inset {
    use std::cell::Cell;

    thread_local!(static INSET: Cell<f32> = const { Cell::new(0.) });

    pub(crate) fn set(inset: f32) {
        INSET.with(|i| i.set(inset));
    }

    pub(crate) fn get() -> f32 {
        INSET.with(Cell::get)
    }
}
