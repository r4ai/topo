//! The right-hand panel: the selected node, or an overview when nothing is selected.

use std::cmp::Reverse;

use gpui::{
    AnyElement, Context, CursorStyle, Div, ElementId, FontWeight, MouseButton, MouseDownEvent, SharedString, Stateful,
    div, prelude::*, px,
};
use jiff::Timestamp;
use jiff::tz::TimeZone;
use topo_core::model::pr_label;
use topo_core::{Graph, Kind, Node, NodeId, Priority, Status};
use topo_jev::organize::Proposal;

use crate::inline::Field;
use crate::theme::metrics::{R_LG, R_MD, R_SM, R_XS, T_BODY, T_BODY_LG, T_HEADING, T_SMALL};
use crate::theme::{self, metrics};
use crate::ui::{self, Icon, button, button_icon, icon_button_svg, kbd, section_label};
use crate::{Drag, Relation, TopoApp, dates};

type Remove = fn(&mut Graph, &NodeId, &NodeId) -> Result<(), topo_core::Error>;
/// The two ends of a relation and how to remove it.
type Removal = Option<(NodeId, NodeId, Remove)>;

impl TopoApp {
    pub(crate) fn inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current();
        let content: Vec<AnyElement> = match self.selected_nodes.len() {
            n if n > 1 => self.selection_details(cx),
            _ => match self.selected_node() {
                Some(node) => self.node_details(node, cx),
                None => self.overview(cx),
            },
        };
        div().relative().flex_shrink_0().h_full().w(px(self.inspector_width())).child(self.resize_handle(cx)).child(
            ui::panel()
                .id("inspector")
                .track_scroll(&self.inspector_scroll)
                .size_full()
                .flex()
                .flex_col()
                .border_l_1()
                .border_color(t.hairline)
                .text_size(px(T_BODY_LG))
                .text_color(t.fg)
                .overflow_y_scroll()
                .children(self.proposals_view(cx))
                .child(div().flex().flex_col().p_4().children(content)),
        )
    }

    /// A strip on the inspector's left border that drags its width.
    fn resize_handle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current();
        div()
            .id("inspector-resize")
            .debug_selector(|| "inspector-resize".to_owned())
            .absolute()
            .left_neg_1()
            .top_0()
            .h_full()
            .w(px(7.))
            .cursor(CursorStyle::ResizeLeftRight)
            .hover(move |s| s.bg(t.border_strong))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    app.drag = Some(Drag::Resize);
                    cx.notify();
                }),
            )
    }

    fn selection_details(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let t = theme::current();
        let nodes: Vec<_> = self.selected_nodes.iter().filter_map(|id| self.graph().get(id)).collect();
        let common_status = nodes.first().map(|n| n.status).filter(|s| nodes.iter().all(|n| n.status == *s));
        let mut out = vec![
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .text_size(px(T_HEADING))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("{} nodes selected", nodes.len())),
                )
                .child(icon_button_svg("deselect", Icon::Close).on_click(cx.listener(|app, _, _, cx| {
                    app.select(None, false);
                    cx.notify();
                })))
                .into_any_element(),
            div()
                .mt_1()
                .text_size(px(T_BODY))
                .text_color(t.fg_faint)
                .child("Change status for all selected nodes, or click a row to edit one.")
                .into_any_element(),
            self.status_control(common_status, cx).into_any_element(),
            section_label("SELECTED NODES").into_any_element(),
        ];
        for node in nodes {
            out.push(self.node_row("selected", node, Some(node.id.to_string()), None, cx).into_any_element());
        }
        out
    }

    /// Every segment reserves its border, keeping labels still as the active status changes.
    fn status_control(&self, current: Option<Status>, cx: &mut Context<Self>) -> Div {
        let mut segments = ui::segmented().mt_3();
        for (i, status) in [Status::Todo, Status::Doing, Status::Done, Status::Dropped].into_iter().enumerate() {
            let label = div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().text_color(theme::status_color(status)).child(theme::status_icon(status)))
                .child(theme::status_label(status));
            segments = segments.child(
                ui::segment(format!("status-{i}"), label, current == Some(status))
                    .flex_1()
                    .h(px(metrics::H_BUTTON))
                    .on_click(cx.listener(move |app, _, _, cx| app.set_selected_status(status, cx))),
            );
        }
        segments
    }

    /// A clickable row naming a node, with an optional button that removes the relation.
    /// The status icon of a task toggles it done, like the one on its card.
    fn node_row(
        &self,
        key: &str,
        node: &Node,
        detail: Option<String>,
        remove: Removal,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let t = theme::current();
        let (icon, color) = theme::node_icon(node);
        let group = SharedString::from(format!("row-{key}-{}", node.id));
        let id = node.id.clone();
        let toggle_id = node.id.clone();
        let icon = div()
            .id(ElementId::Name(format!("{group}-check").into()))
            .debug_selector(|| format!("{group}-check"))
            .w(px(18.))
            .h(px(18.))
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .rounded(px(R_XS))
            .text_color(color)
            .child(icon)
            .when(node.kind == Kind::Task, |d| {
                d.hover(move |s| s.bg(t.control_active)).on_click(cx.listener(move |app, _, _, cx| {
                    cx.stop_propagation();
                    app.toggle_done(toggle_id.clone(), cx);
                }))
            });
        ui::list_row(group.clone(), false)
            .debug_selector(|| group.to_string())
            .group(group.clone())
            .flex()
            .items_center()
            .gap_2()
            .min_h(px(30.))
            .mx_neg_2()
            .child(icon)
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .when(node.status.is_closed(), |d| d.text_color(t.fg_faint).line_through())
                    .child(node.title.clone()),
            )
            .children(detail.map(|d| div().flex_shrink_0().text_size(px(T_BODY)).text_color(t.fg_faint).child(d)))
            .children(remove.map(|(a, b, remove)| {
                icon_button_svg(ElementId::Name(format!("{group}-remove").into()).to_string(), Icon::Close)
                    .opacity(0.)
                    .group_hover(group.clone(), |s| s.opacity(1.))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        cx.stop_propagation();
                        let (a, b) = (a.clone(), b.clone());
                        app.mutate(cx, move |graph| remove(graph, &a, &b));
                    }))
            }))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.select(Some(id.clone()), true);
                cx.notify();
            }))
    }

    fn overview(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let t = theme::current();
        let graph = self.graph();
        let today = dates::today();
        let mut out: Vec<AnyElement> = Vec::new();
        out.push(div().text_size(px(T_HEADING)).font_weight(FontWeight::SEMIBOLD).child("Overview").into_any_element());
        out.push(
            div()
                .text_size(px(T_BODY))
                .text_color(t.fg_muted)
                .child("Select a node to edit it. Edits from the CLI or agents appear live.")
                .into_any_element(),
        );

        // What matters most comes first; a priority is named, not only colored.
        let detail = |node: &Node| {
            let parts = [node.priority.map(theme::priority_text), node.due.map(|d| dates::short(d, today))];
            Some(parts.into_iter().flatten().collect::<Vec<_>>().join(" · ")).filter(|d| !d.is_empty())
        };
        let mut doing: Vec<&Node> = self.graph_cache.doing.iter().filter_map(|id| graph.get(id)).collect();
        doing.sort_by_key(|n| Reverse(n.priority));
        if !doing.is_empty() {
            out.push(section_label("IN PROGRESS").into_any_element());
            for node in doing {
                let who = node.assignee.as_ref().map(|a| format!("@{a}"));
                out.push(self.node_row("doing", node, who.or_else(|| detail(node)), None, cx).into_any_element());
            }
        }

        let mut ready: Vec<&Node> = self.graph_cache.ready.iter().filter_map(|id| graph.get(id)).collect();
        ready.sort_by_key(|n| (Reverse(n.priority), n.due.is_none(), n.due));
        out.push(section_label(format!("READY NOW · {}", ready.len())).into_any_element());
        if ready.is_empty() {
            out.push(
                div()
                    .text_size(px(T_BODY))
                    .text_color(t.fg_faint)
                    .child("Nothing is unblocked right now.")
                    .into_any_element(),
            );
        }
        for node in ready {
            out.push(self.node_row("ready", node, detail(node), None, cx).into_any_element());
        }

        let milestones: Vec<&Node> = graph.nodes().filter(|n| n.kind == Kind::Milestone).collect();
        if !milestones.is_empty() {
            out.push(section_label("MILESTONES").into_any_element());
        }
        for m in milestones {
            let (done, total) = self.graph_cache.progress[&m.id];
            let left = self.graph_cache.critical_lengths[&m.id];
            let fraction = if total == 0 { 0. } else { done as f32 / total as f32 };
            let complete = m.status.is_closed() || (total > 0 && done == total);
            let id = m.id.clone();
            let (glyph, glyph_color) = theme::node_icon(m);
            let due = m.due.map(|d| {
                let color = if m.status.is_closed() { t.fg_faint } else { dates::urgency_color(d, today) };
                div().text_color(color).child(format!("due {} · {}", dates::short(d, today), dates::relative(d, today)))
            });
            out.push(
                div()
                    .id(ElementId::Name(format!("overview-{id}").into()))
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .p_2p5()
                    .mb_2()
                    .rounded(px(R_LG))
                    .bg(t.card)
                    .border_1()
                    .border_color(t.hairline)
                    .cursor_pointer()
                    .hover(|s| s.border_color(t.border_strong))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().text_color(glyph_color).child(glyph))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .truncate()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(m.title.clone()),
                            )
                            .child(div().text_size(px(T_BODY)).text_color(t.fg_muted).child(format!("{done}/{total}"))),
                    )
                    .child(div().flex().child(ui::progress_bar(fraction, complete, 4.)))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_size(px(T_BODY))
                            .text_color(t.fg_muted)
                            .child(match left {
                                0 => "all requirements closed".to_owned(),
                                1 => "1 step left".to_owned(),
                                n => format!("{n} steps left"),
                            })
                            .children(due),
                    )
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.select(Some(id.clone()), true);
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        out
    }

    fn node_details(&self, node: &Node, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let t = theme::current();
        let graph = self.graph();
        let today = dates::today();
        let id = node.id.clone();
        let milestone = node.kind == Kind::Milestone;
        let mut out: Vec<AnyElement> = Vec::new();

        // Header: kind, id and title.
        let (icon, color) = theme::node_icon(node);
        out.push(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_size(px(T_BODY))
                .text_color(t.fg_faint)
                .child(ui::chip_outline(if milestone { "MILESTONE" } else { "TASK" }))
                .child(id.to_string())
                .child(div().flex_1())
                .child(icon_button_svg("deselect", Icon::Close).on_click(cx.listener(|app, _, _, cx| {
                    app.select(None, false);
                    cx.notify();
                })))
                .into_any_element(),
        );
        let title = div().flex().items_start().gap_2().mt_2().p_1().mx_neg_1().rounded(px(R_SM));
        let glyph = div().pt_0p5().text_color(color).text_size(px(T_HEADING)).child(icon);
        out.push(match self.editing(&id, Field::Title) {
            // Edited where it stands, at its own size.
            true => title
                .child(glyph)
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_size(px(T_HEADING))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(self.combo().clone()),
                )
                .into_any_element(),
            false => title
                .id("title")
                .debug_selector(|| "title".to_owned())
                .cursor_pointer()
                .hover(move |s| s.bg(t.control_hover))
                .child(glyph)
                .child(title_text(node.title.clone()))
                .on_click(cx.listener(|app, _, window, cx| app.start_inline(Field::Title, window, cx)))
                .into_any_element(),
        });

        // State line.
        let open_reqs: Vec<&Node> = graph
            .requirements(node)
            .into_iter()
            .map(|r| graph.get(r).expect("graph invariant"))
            .filter(|n| !n.status.is_closed())
            .collect();
        let (state, state_color) = match (node.status, open_reqs.len()) {
            (Status::Done, _) => ("Done".to_owned(), t.fg_faint),
            (Status::Dropped, _) => ("Dropped".to_owned(), t.fg_faint),
            (_, 0) if milestone => ("Reached — every requirement is closed".to_owned(), t.ready),
            (Status::Doing, _) => ("In progress".to_owned(), t.accent),
            (_, 0) => ("Ready to start".to_owned(), t.ready),
            (_, n) if milestone => (format!("{n} requirement{} still open", if n == 1 { "" } else { "s" }), t.fg_muted),
            (_, n) => (format!("Blocked by {n} open requirement{}", if n == 1 { "" } else { "s" }), t.fg_muted),
        };
        out.push(
            div()
                .mt_1()
                .min_w(px(0.))
                .truncate()
                .text_size(px(T_BODY))
                .text_color(state_color)
                .child(state)
                .into_any_element(),
        );

        // Status control.
        out.push(self.status_control(Some(node.status), cx).into_any_element());

        // Properties, each edited in place.
        let none = || div().text_color(t.fg_faint).child("None");
        let priority = match node.priority {
            Some(p) => div()
                .truncate()
                .text_color(theme::priority_color(p))
                .when(p == Priority::High, |d| d.font_weight(FontWeight::SEMIBOLD))
                .child(theme::priority_text(p)),
            None => none(),
        };
        let assignee = match &node.assignee {
            Some(name) => div().truncate().child(format!("@{name}")),
            None => div().text_color(t.fg_faint).child("Unassigned"),
        };
        let due = match node.due {
            Some(d) => {
                let c = if node.status.is_closed() { t.fg_muted } else { dates::urgency_color(d, today) };
                div().truncate().text_color(c).child(format!(
                    "{} · {}",
                    dates::short(d, today),
                    dates::relative(d, today)
                ))
            }
            None => none(),
        };
        let tags = match node.tags.is_empty() {
            true => none(),
            false => div().flex().flex_wrap().gap_1().children(node.tags.iter().map(|tag| ui::chip(format!("#{tag}")))),
        };
        out.push(section_label("DETAILS").into_any_element());
        out.push(self.property(node, Field::Priority, "Priority", "P", priority, cx));
        out.push(self.property(node, Field::Assignee, "Assignee", "A", assignee, cx));
        out.push(self.property(node, Field::Due, "Due", "D", due, cx));
        out.push(self.property(node, Field::Tags, "Tags", "T", tags, cx));
        let tz = TimeZone::system();
        out.push(moment_row("created", "Created", node.created_at, "Unknown", &tz));
        out.push(moment_row("updated", "Updated", node.updated_at, "Unknown", &tz));
        let not_done = if node.status == Status::Done { "Unknown" } else { "Not completed" };
        out.push(moment_row("completed", "Completed", node.completed_at, not_done, &tz));
        out.extend(self.pull_requests(node, cx));

        // Milestone progress and critical path.
        if milestone {
            let (done, total) = self.graph_cache.progress[&id];
            let fraction = if total == 0 { 0. } else { done as f32 / total as f32 };
            let path = &self.graph_cache.critical_paths[&id];
            out.push(section_label("PROGRESS").into_any_element());
            out.push(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(ui::progress_bar(fraction, total > 0 && done == total, 6.))
                    .child(
                        div()
                            .text_size(px(T_BODY))
                            .text_color(t.fg_muted)
                            .child(format!("{done}/{total} · {:.0}%", fraction * 100.)),
                    )
                    .into_any_element(),
            );
            if !path.is_empty() {
                out.push(section_label(format!("CRITICAL PATH · {} STEPS", path.len())).into_any_element());
                for (i, step) in path.iter().enumerate() {
                    let step = graph.get(step).expect("graph invariant");
                    out.push(self.node_row("critical", step, Some(format!("{}", i + 1)), None, cx).into_any_element());
                }
            }
        }

        out.extend(self.notes_section(node, cx));

        // Relations: each section lists what is connected and offers to connect more.
        let section = |label: &'static str, items: Vec<(&Node, Removal)>, add: Relation, cx: &mut Context<Self>| {
            let heading = match items.len() {
                0 => label.to_owned(),
                n => format!("{label} · {n}"),
            };
            let add_button = icon_button_svg(format!("add-{label}"), Icon::Plus)
                .on_click(cx.listener(move |app, _, window, cx| app.prompt_pick(add, window, cx)));
            let header = div().flex().items_end().justify_between().child(section_label(heading)).child(add_button);
            let rows =
                items.into_iter().map(|(n, remove)| self.node_row(label, n, None, remove, cx).into_any_element());
            std::iter::once(header.into_any_element()).chain(rows).collect::<Vec<_>>()
        };
        let get = |id: &NodeId| graph.get(id).expect("graph invariant");
        let deps: Vec<_> =
            node.depends_on.iter().map(|d| (get(d), Some((id.clone(), d.clone(), Graph::unlink as Remove)))).collect();
        let needed_by: Vec<_> =
            graph.dependents(&id).map(|n| (n, Some((n.id.clone(), id.clone(), Graph::unlink as Remove)))).collect();
        match node.kind {
            Kind::Milestone => {
                let members: Vec<_> =
                    graph.members(&id).map(|m| (m, Some((m.id.clone(), id.clone(), Graph::leave as Remove)))).collect();
                out.extend(section("MEMBERS", members, Relation::Member, cx));
            }
            Kind::Task => {
                let memberships: Vec<_> = node
                    .milestones
                    .iter()
                    .map(|m| (get(m), Some((id.clone(), m.clone(), Graph::leave as Remove))))
                    .collect();
                out.extend(section("IN MILESTONES", memberships, Relation::InMilestone, cx));
            }
        }
        out.extend(section("REQUIRES", deps, Relation::Requires, cx));
        out.extend(section("NEEDED BY", needed_by, Relation::NeededBy, cx));

        // Actions.
        let (follow_id, convert_id, delete_id) = (id.clone(), id.clone(), id.clone());
        out.push(ui::divider().mt_5().into_any_element());
        out.push(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .pt_4()
                .when(!milestone, |d| {
                    d.child(button_icon("add-follow", Icon::Plus, "Follow-up").child(kbd("Tab")).on_click(cx.listener(
                        move |app, _, window, cx| {
                            app.select(Some(follow_id.clone()), false);
                            app.prompt_create(Kind::Task, Some(crate::Direction::Right), window, cx);
                        },
                    )))
                })
                .when(!milestone, |d| {
                    d.child(button_icon("add-pre", Icon::Plus, "Prerequisite").child(kbd("⇧Tab")).on_click(
                        cx.listener(|app, _, window, cx| {
                            app.prompt_create(Kind::Task, Some(crate::Direction::Left), window, cx)
                        }),
                    ))
                })
                .when(milestone, |d| {
                    d.child(button_icon("add-member", Icon::Plus, "Task").child(kbd("N")).on_click(cx.listener(
                        |app, _, window, cx| {
                            app.prompt_create(Kind::Task, None, window, cx);
                        },
                    )))
                })
                .when(self.convertible(node), |d| {
                    d.child(
                        button("convert", if milestone { "Make it a task" } else { "Make it a milestone" })
                            .on_click(cx.listener(move |app, _, _, cx| app.convert(convert_id.clone(), cx))),
                    )
                })
                .child(
                    ui::button_danger("delete", "Delete")
                        .ml_auto()
                        .child(kbd("⌫"))
                        .on_click(cx.listener(move |app, _, _, cx| app.delete(delete_id.clone(), cx))),
                )
                .into_any_element(),
        );
        out
    }

    /// A row of DETAILS. A click, or its key, turns the value into a text field.
    fn property(
        &self,
        node: &Node,
        field: Field,
        label: &'static str,
        shortcut: &'static str,
        value: Div,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme::current();
        let key = format!("prop-{}", label.to_lowercase());
        if self.editing(&node.id, field) {
            return self.inline_field(Some(label));
        }
        ui::list_row(key.clone(), false)
            .debug_selector(move || key)
            .flex()
            .items_center()
            .gap_2()
            .min_h(px(30.))
            .mx_neg_2()
            .child(property_label(label))
            .child(div().flex_1().min_w(px(0.)).overflow_hidden().text_size(px(T_BODY)).text_color(t.fg).child(value))
            .child(kbd(shortcut))
            .on_click(cx.listener(move |app, _, window, cx| app.start_inline(field, window, cx)))
            .into_any_element()
    }

    /// The field that replaces a row while its value is edited. It is as high
    /// as the row, and its list floats, so nothing below moves.
    fn inline_field(&self, label: Option<&'static str>) -> AnyElement {
        div()
            .id("inline-edit")
            .debug_selector(|| "inline-edit".to_owned())
            .flex()
            .items_center()
            .gap_2()
            .min_h(px(30.))
            .children(label.map(property_label))
            .child(div().flex_1().min_w(px(0.)).text_size(px(T_BODY)).child(self.combo().clone()))
            .into_any_element()
    }

    /// The notes: shown as text, and edited in place. A workspace of files can
    /// also open the file; a cloud workspace has none.
    fn notes_section(&self, node: &Node, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let t = theme::current();
        let editing = self.notes.as_ref() == Some(&node.id);
        let action = |id: &'static str, lead: Option<Icon>, label: &'static str, key: &'static str| {
            let button = match lead {
                Some(lead) => ui::button_ghost_icon(id, lead, label),
                None => ui::button_ghost(id, label),
            };
            button.h(px(metrics::H_ICON)).px_1().mb_0p5().child(kbd(key))
        };
        let file_id = node.id.clone();
        let actions = div()
            .flex()
            .items_center()
            .gap_1()
            .when(!editing, |d| {
                d.child(action("edit-notes", None, "Edit", "E").on_click(cx.listener(|app, _, window, cx| {
                    app.start_notes(window, cx);
                })))
            })
            .when(self.ws.remote().is_none(), |d| {
                d.child(
                    action("open-file", Some(Icon::External), "File", "O")
                        .on_click(cx.listener(move |app, _, _, cx| app.open_file(&file_id, cx))),
                )
            });
        let state = match (editing, self.notes_dirty(cx)) {
            (true, true) => {
                Some(ui::chip_tinted("● Unsaved changes", t.warn).debug_selector(|| "notes-unsaved".to_owned()))
            }
            (true, false) => Some(ui::chip_tinted("Saved", t.success).debug_selector(|| "notes-saved".to_owned())),
            (false, _) => None,
        };
        let header = div()
            .flex()
            .items_end()
            .justify_between()
            .child(div().flex().items_end().gap_2().child(section_label("NOTES")).children(state.map(|s| s.mb_1())))
            .child(actions);
        let text = |focused: bool| {
            ui::input_frame(focused, false).min_w(px(0.)).p_3().text_size(px(T_BODY)).line_height(px(18.))
        };
        let body = match (editing, node.body.trim().is_empty()) {
            (true, _) => div()
                .flex()
                .flex_col()
                .gap_1()
                // The editor keeps its own clicks: a click anywhere else saves and closes it.
                .on_mouse_down(MouseButton::Left, |_, _, cx: &mut gpui::App| cx.stop_propagation())
                .child(text(true).debug_selector(|| "notes-editor".to_owned()).child(self.notes_input().clone()))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_x_3()
                        .gap_y_1()
                        .text_size(px(T_SMALL))
                        .text_color(t.fg_faint)
                        .children(
                            [("⌘↵", "save"), ("esc", "close"), ("tab", "indent")]
                                .map(|(key, what)| div().flex().items_center().gap_1().child(kbd(key)).child(what)),
                        )
                        .child("unsaved changes ask first"),
                )
                .into_any_element(),
            (false, empty) => text(false)
                .id("notes")
                .debug_selector(|| "notes".to_owned())
                .cursor_pointer()
                .hover(move |s| s.border_color(t.border_strong))
                .text_color(if empty { t.fg_faint } else { t.fg })
                .child(if empty { "No notes — click to write".to_owned() } else { node.body.trim_end().to_owned() })
                .on_click(cx.listener(|app, _, window, cx| app.start_notes(window, cx)))
                .into_any_element(),
        };
        vec![header.into_any_element(), body]
    }

    /// The related pull requests: each opens in the browser and can be unlinked.
    fn pull_requests(&self, node: &Node, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let t = theme::current();
        let heading = match node.prs.len() {
            0 => "PULL REQUESTS".to_owned(),
            n => format!("PULL REQUESTS · {n}"),
        };
        let add = div().flex().items_center().gap_1().child(kbd("G")).child(
            icon_button_svg("add-pr", Icon::Plus).on_click(cx.listener(|app, _, window, cx| {
                app.start_inline(Field::Pr, window, cx);
            })),
        );
        let mut out = vec![
            div().flex().items_end().justify_between().child(section_label(heading)).child(add).into_any_element(),
        ];
        for (index, url) in node.prs.iter().enumerate() {
            let group = SharedString::from(format!("pr-{index}"));
            let (id, open, unlink) = (node.id.clone(), url.clone(), url.clone());
            out.push(
                ui::list_row(group.clone(), false)
                    .debug_selector(|| group.to_string())
                    .group(group.clone())
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_h(px(30.))
                    .mx_neg_2()
                    .child(ui::icon(Icon::External, t.fg_muted))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(T_BODY))
                            .text_color(t.md_link)
                            .child(pr_label(url)),
                    )
                    .child(
                        icon_button_svg(format!("{group}-remove"), Icon::Close)
                            .opacity(0.)
                            .group_hover(group.clone(), |s| s.opacity(1.))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                cx.stop_propagation();
                                app.unlink_pr(id.clone(), &unlink, cx);
                            })),
                    )
                    .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&open)))
                    .into_any_element(),
            );
        }
        if self.editing(&node.id, Field::Pr) {
            out.push(self.inline_field(None));
        } else if node.prs.is_empty() {
            out.push(div().text_size(px(T_BODY)).text_color(t.fg_faint).child("None linked").into_any_element());
        }
        out
    }

    fn proposals_view(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let t = theme::current();
        if self.proposals.is_empty() {
            return None;
        }
        let actionable: Vec<usize> =
            (0..self.proposals.len()).filter(|&i| !matches!(self.proposals[i], Proposal::Duplicate { .. })).collect();
        let cards = self.proposals.iter().enumerate().map(|(index, proposal)| {
            let title = |id: &NodeId| self.title_of(id);
            let (kind, text, p) = match proposal {
                Proposal::Link { from, to, probability } => {
                    ("Dependency", format!("“{}” requires “{}”", title(from), title(to)), probability)
                }
                Proposal::Join { task, milestone, probability } => {
                    ("Milestone", format!("Add “{}” to ◆ {}", title(task), title(milestone)), probability)
                }
                Proposal::SetKind { id, kind, probability } => {
                    let kind = match kind {
                        Kind::Task => "task",
                        Kind::Milestone => "milestone",
                    };
                    ("Kind", format!("Make “{}” a {kind}", title(id)), probability)
                }
                Proposal::Duplicate { a, b, probability } => {
                    ("Possible duplicate", format!("“{}” ≈ “{}”", title(a), title(b)), probability)
                }
            };
            let duplicate = matches!(proposal, Proposal::Duplicate { .. });
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .p_2p5()
                .rounded(px(R_MD))
                .bg(t.control)
                .border_1()
                .border_color(t.hairline)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(T_BODY))
                        .child(div().text_color(if duplicate { t.warn } else { t.fg_faint }).child(kind))
                        .child(div().w(px(40.)).flex().child(ui::progress_bar(*p as f32, false, 3.)))
                        .child(div().text_color(t.fg_muted).child(format!("{:.0}%", p * 100.)))
                        .child(div().flex_1())
                        .when(!duplicate, |d| {
                            d.child(
                                ui::button_primary_icon(format!("accept-{index}"), Icon::Check, "")
                                    .on_click(cx.listener(move |app, _, _, cx| app.accept(vec![index], cx))),
                            )
                        })
                        .child(ui::button_ghost_icon(format!("reject-{index}"), Icon::Close, "").on_click(
                            cx.listener(move |app, _, _, cx| {
                                app.proposals.remove(index);
                                cx.notify();
                            }),
                        )),
                )
                .child(div().min_w(px(0.)).truncate().text_size(px(T_BODY)).text_color(t.fg).child(text))
        });
        Some(
            div()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_4()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(ui::icon(Icon::Sparkle, t.fg_muted))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .truncate()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(format!("Suggestions · {}", self.proposals.len())),
                                )
                                .when(!actionable.is_empty(), |d| {
                                    d.child(
                                        ui::button_primary("accept-all", "Accept all").on_click(
                                            cx.listener(move |app, _, _, cx| app.accept(actionable.clone(), cx)),
                                        ),
                                    )
                                })
                                .child(ui::button_ghost("dismiss-all", "Dismiss").on_click(cx.listener(
                                    |app, _, _, cx| {
                                        app.proposals.clear();
                                        cx.notify();
                                    },
                                ))),
                        )
                        .children(cards),
                )
                .child(ui::divider()),
        )
    }
}

/// Width of the label column of DETAILS.
const PROPERTY_LABEL_W: f32 = 64.;

fn property_label(label: &'static str) -> Div {
    let t = theme::current();
    div().w(px(PROPERTY_LABEL_W)).flex_shrink_0().text_size(px(T_BODY)).text_color(t.fg_muted).child(label)
}

/// A read-only row of DETAILS showing a recorded moment in the time zone `tz`,
/// or `absent` when there is none. The value is the part that gives way in a
/// narrow panel, and it stays readable there: it has the row to itself.
fn moment_row(
    key: &'static str,
    label: &'static str,
    at: Option<Timestamp>,
    absent: &'static str,
    tz: &TimeZone,
) -> AnyElement {
    let t = theme::current();
    let value = match at {
        Some(at) => div().truncate().text_color(t.fg).child(dates::moment(at, tz.clone())),
        None => div().truncate().text_color(t.fg_faint).child(absent),
    };
    div()
        .debug_selector(move || format!("prop-{key}"))
        .flex()
        .items_center()
        .gap_2()
        .min_h(px(24.))
        .child(property_label(label))
        .child(div().flex_1().min_w(px(0.)).text_size(px(T_BODY)).child(value))
        .into_any_element()
}

/// The node title, clamped to three lines that end in `…`. `line_clamp` alone
/// limits the lines but clips the last one mid-character.
fn title_text(title: impl Into<SharedString>) -> Stateful<Div> {
    let t = theme::current();
    div()
        .id("title-text")
        .debug_selector(|| "title-text".to_owned())
        .flex_1()
        .min_w(px(0.))
        // Reserve the combobox's inset and border before editing, so the text
        // wraps at the same width and the rows below it do not move.
        .px_1p5()
        .py_0p5()
        .border_1()
        .border_color(theme::alpha(t.fg, 0.))
        .line_clamp(crate::inline::TITLE_ROWS)
        .text_ellipsis()
        .text_size(px(T_HEADING))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.fg)
        .child(title.into())
}

#[cfg(test)]
mod tests {
    use gpui::TextOverflow;

    use super::*;

    #[test]
    fn the_title_clamp_ends_in_an_ellipsis() {
        let mut title = title_text("title");
        let style = title.text_style();
        assert_eq!(style.line_clamp, Some(3));
        assert!(
            matches!(&style.text_overflow, Some(TextOverflow::Truncate(affix)) if affix.as_ref() == "…"),
            "the clamped title must truncate with an ellipsis, got {:?}",
            style.text_overflow
        );
    }
}
