//! The right-hand panel: the selected node, or an overview when nothing is selected.

use gpui::{
    AnyElement, Context, CursorStyle, Div, ElementId, MouseButton, MouseDownEvent, SharedString, Stateful, div,
    prelude::*, px, rgb,
};
use topo_core::{Graph, Kind, Node, NodeId, Status};
use topo_jev::organize::Proposal;

use crate::theme::{self, button, chip, icon_button, kbd, section_label};
use crate::{Drag, Prompt, Relation, TopoApp, dates};

type Remove = fn(&mut Graph, &NodeId, &NodeId) -> Result<(), topo_core::Error>;
/// The two ends of a relation and how to remove it.
type Removal = Option<(NodeId, NodeId, Remove)>;

impl TopoApp {
    pub(crate) fn inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let content: Vec<AnyElement> = match self.selected_nodes.len() {
            n if n > 1 => self.selection_details(cx),
            _ => match self.selected_node() {
                Some(node) => self.node_details(node, cx),
                None => self.overview(cx),
            },
        };
        div().relative().flex_shrink_0().h_full().w(px(self.inspector_width())).child(self.resize_handle(cx)).child(
            div()
                .id("inspector")
                .size_full()
                .flex()
                .flex_col()
                .bg(rgb(theme::SURFACE))
                .border_l_1()
                .border_color(rgb(theme::BORDER))
                .text_sm()
                .overflow_y_scroll()
                .children(self.proposals_view(cx))
                .child(div().flex().flex_col().p_4().children(content)),
        )
    }

    /// A strip on the inspector's left border that drags its width.
    fn resize_handle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("inspector-resize")
            .debug_selector(|| "inspector-resize".to_owned())
            .absolute()
            .left_neg_1()
            .top_0()
            .h_full()
            .w(px(7.))
            .cursor(CursorStyle::ResizeLeftRight)
            .hover(|s| s.bg(theme::alpha(theme::ACCENT, 0.5)))
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
                        .text_lg()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(format!("{} nodes selected", nodes.len())),
                )
                .child(icon_button("deselect", "×").on_click(cx.listener(|app, _, _, cx| {
                    app.select(None, false);
                    cx.notify();
                })))
                .into_any_element(),
            div()
                .mt_1()
                .text_xs()
                .text_color(rgb(theme::FAINT))
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
        let mut segments = div()
            .flex()
            .mt_3()
            .p_0p5()
            .gap_0p5()
            .rounded_lg()
            .bg(rgb(theme::CANVAS))
            .border_1()
            .border_color(rgb(theme::BORDER));
        for (i, status) in [Status::Todo, Status::Doing, Status::Done, Status::Dropped].into_iter().enumerate() {
            let active = current == Some(status);
            let c = theme::status_color(status);
            segments = segments.child(
                div()
                    .id(ElementId::Name(format!("status-{i}").into()))
                    .debug_selector(|| format!("status-{i}"))
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .h(px(28.))
                    .rounded_md()
                    .border_1()
                    .border_color(theme::alpha(c, if active { 0.5 } else { 0. }))
                    .text_xs()
                    .cursor_pointer()
                    .text_color(rgb(if active { theme::TEXT } else { theme::MUTED }))
                    .when(active, |d| d.bg(theme::alpha(c, 0.18)))
                    .when(!active, |d| d.hover(|s| s.bg(rgb(theme::CARD)).text_color(rgb(theme::TEXT))))
                    .child(div().text_color(rgb(c)).child(theme::status_icon(status)))
                    .child(theme::status_label(status))
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
            .rounded_sm()
            .text_color(rgb(color))
            .child(icon)
            .when(node.kind == Kind::Task, |d| {
                d.hover(|s| s.bg(rgb(theme::RAISED))).on_click(cx.listener(move |app, _, _, cx| {
                    cx.stop_propagation();
                    app.toggle_done(toggle_id.clone(), cx);
                }))
            });
        div()
            .id(ElementId::Name(group.clone()))
            .debug_selector(|| group.to_string())
            .group(group.clone())
            .flex()
            .items_center()
            .gap_2()
            .min_h(px(30.))
            .px_2()
            .mx_neg_2()
            .rounded_md()
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::CARD_HOVER)))
            .child(icon)
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .when(node.status.is_closed(), |d| d.text_color(rgb(theme::FAINT)).line_through())
                    .child(node.title.clone()),
            )
            .children(detail.map(|d| div().flex_shrink_0().text_xs().text_color(rgb(theme::FAINT)).child(d)))
            .children(remove.map(|(a, b, remove)| {
                icon_button(ElementId::Name(format!("{group}-remove").into()).to_string(), "×")
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
        let graph = self.graph();
        let today = dates::today();
        let mut out: Vec<AnyElement> = Vec::new();
        out.push(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Overview").into_any_element());
        out.push(
            div()
                .text_xs()
                .text_color(rgb(theme::FAINT))
                .child("Select a node to edit it. Edits from the CLI or agents appear live.")
                .into_any_element(),
        );

        let doing: Vec<&Node> = graph.nodes().filter(|n| n.kind == Kind::Task && n.status == Status::Doing).collect();
        if !doing.is_empty() {
            out.push(section_label("IN PROGRESS").into_any_element());
            for node in doing {
                out.push(self.node_row("doing", node, None, None, cx).into_any_element());
            }
        }

        let mut ready: Vec<&Node> = graph.ready_tasks(None).into_iter().filter(|n| n.status == Status::Todo).collect();
        ready.sort_by_key(|n| (n.due.is_none(), n.due));
        out.push(section_label(format!("READY NOW · {}", ready.len())).into_any_element());
        if ready.is_empty() {
            out.push(
                div()
                    .text_xs()
                    .text_color(rgb(theme::FAINT))
                    .child("Nothing is unblocked right now.")
                    .into_any_element(),
            );
        }
        for node in ready {
            let due = node.due.map(|d| dates::short(d, today));
            out.push(self.node_row("ready", node, due, None, cx).into_any_element());
        }

        let milestones: Vec<&Node> = graph.nodes().filter(|n| n.kind == Kind::Milestone).collect();
        if !milestones.is_empty() {
            out.push(section_label("MILESTONES").into_any_element());
        }
        for m in milestones {
            let (done, total) = graph.progress(&m.id);
            let left = graph.critical_path(&m.id).len();
            let fraction = if total == 0 { 0. } else { done as f32 / total as f32 };
            let color = if m.status.is_closed() || (total > 0 && done == total) { theme::GREEN } else { theme::AMBER };
            let id = m.id.clone();
            let due = m.due.map(|d| {
                let color = if m.status.is_closed() { theme::FAINT } else { dates::urgency_color(d, today) };
                div().text_color(rgb(color)).child(format!(
                    "due {} · {}",
                    dates::short(d, today),
                    dates::relative(d, today)
                ))
            });
            out.push(
                div()
                    .id(ElementId::Name(format!("overview-{id}").into()))
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .p_2p5()
                    .mb_2()
                    .rounded_lg()
                    .bg(rgb(theme::CARD))
                    .border_1()
                    .border_color(rgb(theme::BORDER))
                    .cursor_pointer()
                    .hover(|s| s.border_color(rgb(theme::BORDER_STRONG)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().text_color(rgb(color)).child("◆"))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .truncate()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child(m.title.clone()),
                            )
                            .child(div().text_xs().text_color(rgb(theme::MUTED)).child(format!("{done}/{total}"))),
                    )
                    .child(div().flex().child(theme::progress_bar(fraction, color, 4.)))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_xs()
                            .text_color(rgb(theme::FAINT))
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
                .text_xs()
                .text_color(rgb(theme::FAINT))
                .child(chip(
                    if milestone { "MILESTONE" } else { "TASK" },
                    if milestone { theme::AMBER } else { theme::MUTED },
                ))
                .child(id.to_string())
                .child(div().flex_1())
                .child(icon_button("deselect", "×").on_click(cx.listener(|app, _, _, cx| {
                    app.select(None, false);
                    cx.notify();
                })))
                .into_any_element(),
        );
        let rename_id = id.clone();
        out.push(
            div()
                .id("title")
                .flex()
                .items_start()
                .gap_2()
                .mt_2()
                .p_1()
                .mx_neg_1()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(rgb(theme::CARD)))
                .child(div().pt_0p5().text_color(rgb(color)).text_lg().child(icon))
                .child(title_text(node.title.clone()))
                .on_click(cx.listener(move |app, _, window, cx| {
                    let title = app.title_of(&rename_id);
                    app.open_prompt(Prompt::Rename(rename_id.clone()), &title, window, cx);
                }))
                .into_any_element(),
        );

        // State line.
        let open_reqs: Vec<&Node> = graph
            .requirements(node)
            .into_iter()
            .map(|r| graph.get(r).expect("graph invariant"))
            .filter(|n| !n.status.is_closed())
            .collect();
        let (state, state_color) = match (node.status, open_reqs.len()) {
            (Status::Done, _) => ("Done".to_owned(), theme::GREEN),
            (Status::Dropped, _) => ("Dropped".to_owned(), theme::FAINT),
            (_, 0) if milestone => ("Reached — every requirement is closed".to_owned(), theme::GREEN),
            (Status::Doing, _) => ("In progress".to_owned(), theme::ACCENT),
            (_, 0) => ("Ready to start".to_owned(), theme::GREEN),
            (_, n) if milestone => {
                (format!("{n} requirement{} still open", if n == 1 { "" } else { "s" }), theme::AMBER)
            }
            (_, n) => (format!("Blocked by {n} open requirement{}", if n == 1 { "" } else { "s" }), theme::AMBER),
        };
        out.push(
            div()
                .mt_1()
                .min_w(px(0.))
                .truncate()
                .text_xs()
                .text_color(rgb(state_color))
                .child(state)
                .into_any_element(),
        );

        // Status control.
        out.push(self.status_control(Some(node.status), cx).into_any_element());

        // Properties.
        let due_id = id.clone();
        let tags_id = id.clone();
        let due_text = match node.due {
            Some(d) => {
                let c = if node.status.is_closed() { theme::MUTED } else { dates::urgency_color(d, today) };
                div().truncate().text_color(rgb(c)).child(format!(
                    "{} · {}",
                    dates::short(d, today),
                    dates::relative(d, today)
                ))
            }
            None => div().text_color(rgb(theme::FAINT)).child("None"),
        };
        let tags = match node.tags.is_empty() {
            true => div().text_color(rgb(theme::FAINT)).child("None"),
            false => div()
                .flex()
                .flex_wrap()
                .gap_1()
                .children(node.tags.iter().map(|t| chip(format!("#{t}"), theme::ACCENT))),
        };
        let property = |key: &'static str, label: &'static str, shortcut: &'static str, value: Div| {
            div()
                .id(key)
                .flex()
                .items_center()
                .gap_2()
                .min_h(px(30.))
                .px_2()
                .mx_neg_2()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(rgb(theme::CARD_HOVER)))
                .child(div().w(px(56.)).flex_shrink_0().text_xs().text_color(rgb(theme::FAINT)).child(label))
                .child(div().flex_1().min_w(px(0.)).overflow_hidden().text_xs().child(value))
                .child(kbd(shortcut))
        };
        out.push(section_label("DETAILS").into_any_element());
        out.push(
            property("prop-due", "Due", "D", due_text)
                .on_click(cx.listener(move |app, _, window, cx| {
                    let due = app.graph().get(&due_id).and_then(|n| n.due).map(|d| d.to_string()).unwrap_or_default();
                    app.open_prompt(Prompt::Due(due_id.clone()), &due, window, cx);
                }))
                .into_any_element(),
        );
        out.push(
            property("prop-tags", "Tags", "T", tags)
                .on_click(cx.listener(move |app, _, window, cx| {
                    let tags = app.graph().get(&tags_id).map(|n| n.tags.join(" ")).unwrap_or_default();
                    app.open_prompt(Prompt::Tags(tags_id.clone()), &tags, window, cx);
                }))
                .into_any_element(),
        );

        // Milestone progress and critical path.
        if milestone {
            let (done, total) = graph.progress(&id);
            let fraction = if total == 0 { 0. } else { done as f32 / total as f32 };
            let path = graph.critical_path(&id);
            out.push(section_label("PROGRESS").into_any_element());
            out.push(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(theme::progress_bar(fraction, theme::GREEN, 6.))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme::MUTED))
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

        // Notes.
        let notes_id = id.clone();
        out.push(
            div()
                .flex()
                .items_end()
                .justify_between()
                .child(section_label("NOTES"))
                .child(
                    div()
                        .id("open-file")
                        .flex()
                        .items_center()
                        .gap_1()
                        .px_1()
                        .mb_0p5()
                        .rounded_sm()
                        .text_xs()
                        .text_color(rgb(theme::MUTED))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(theme::RAISED)).text_color(rgb(theme::TEXT)))
                        .child("Edit file ↗")
                        .child(kbd("O"))
                        .on_click(cx.listener(move |app, _, _, cx| app.open_file(&notes_id, cx))),
                )
                .into_any_element(),
        );
        out.push(match node.body.trim().is_empty() {
            true => div().text_xs().text_color(rgb(theme::FAINT)).child("No notes").into_any_element(),
            false => div()
                .min_w(px(0.))
                .p_3()
                .rounded_lg()
                .bg(rgb(theme::CANVAS))
                .border_1()
                .border_color(rgb(theme::BORDER))
                .text_xs()
                .text_color(rgb(theme::MUTED))
                .line_height(px(18.))
                .child(node.body.trim_end().to_owned())
                .into_any_element(),
        });

        // Relations: each section lists what is connected and offers to connect more.
        let section = |label: &'static str, items: Vec<(&Node, Removal)>, add: Relation, cx: &mut Context<Self>| {
            let heading = match items.len() {
                0 => label.to_owned(),
                n => format!("{label} · {n}"),
            };
            let add_button = icon_button(format!("add-{label}"), "+")
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
        out.push(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .mt_5()
                .pt_4()
                .border_t_1()
                .border_color(rgb(theme::BORDER))
                .when(!milestone, |d| {
                    d.child(button("add-follow", "+ Follow-up").child(kbd("Tab")).on_click(cx.listener(
                        move |app, _, window, cx| {
                            app.select(Some(follow_id.clone()), false);
                            app.prompt_create(Kind::Task, Some(crate::Direction::Right), window, cx);
                        },
                    )))
                })
                .when(!milestone, |d| {
                    d.child(button("add-pre", "+ Prerequisite").child(kbd("⇧Tab")).on_click(cx.listener(
                        |app, _, window, cx| app.prompt_create(Kind::Task, Some(crate::Direction::Left), window, cx),
                    )))
                })
                .when(milestone, |d| {
                    d.child(button("add-member", "+ Task").child(kbd("N")).on_click(cx.listener(
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
                    theme::tinted_button("delete", "Delete", theme::RED)
                        .ml_auto()
                        .child(kbd("⌫"))
                        .on_click(cx.listener(move |app, _, _, cx| app.delete(delete_id.clone(), cx))),
                )
                .into_any_element(),
        );
        out
    }

    fn proposals_view(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if self.proposals.is_empty() {
            return None;
        }
        let actionable: Vec<usize> =
            (0..self.proposals.len()).filter(|&i| !matches!(self.proposals[i], Proposal::Duplicate { .. })).collect();
        let cards = self.proposals.iter().enumerate().map(|(index, proposal)| {
            let t = |id: &NodeId| self.title_of(id);
            let (kind, text, p) = match proposal {
                Proposal::Link { from, to, probability } => {
                    ("Dependency", format!("“{}” requires “{}”", t(from), t(to)), probability)
                }
                Proposal::Join { task, milestone, probability } => {
                    ("Milestone", format!("Add “{}” to ◆ {}", t(task), t(milestone)), probability)
                }
                Proposal::SetKind { id, kind, probability } => {
                    let kind = match kind {
                        Kind::Task => "task",
                        Kind::Milestone => "milestone",
                    };
                    ("Kind", format!("Make “{}” a {kind}", t(id)), probability)
                }
                Proposal::Duplicate { a, b, probability } => {
                    ("Possible duplicate", format!("“{}” ≈ “{}”", t(a), t(b)), probability)
                }
            };
            let duplicate = matches!(proposal, Proposal::Duplicate { .. });
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .p_2p5()
                .rounded_lg()
                .bg(rgb(theme::CARD))
                .border_1()
                .border_color(rgb(theme::BORDER))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .child(div().text_color(rgb(theme::FAINT)).child(kind))
                        .child(div().w(px(40.)).flex().child(theme::progress_bar(*p as f32, theme::ACCENT, 3.)))
                        .child(div().text_color(rgb(theme::MUTED)).child(format!("{:.0}%", p * 100.)))
                        .child(div().flex_1())
                        .when(!duplicate, |d| {
                            d.child(
                                icon_button(format!("accept-{index}"), "✓")
                                    .text_color(rgb(theme::GREEN))
                                    .on_click(cx.listener(move |app, _, _, cx| app.accept(vec![index], cx))),
                            )
                        })
                        .child(icon_button(format!("reject-{index}"), "×").on_click(cx.listener(
                            move |app, _, _, cx| {
                                app.proposals.remove(index);
                                cx.notify();
                            },
                        ))),
                )
                .child(div().min_w(px(0.)).truncate().text_xs().child(text))
        });
        Some(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_4()
                .border_b_1()
                .border_color(rgb(theme::BORDER))
                .bg(theme::alpha(theme::ACCENT, 0.05))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().text_color(rgb(theme::ACCENT)).child("✦"))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .truncate()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(format!("Suggestions · {}", self.proposals.len())),
                        )
                        .when(!actionable.is_empty(), |d| {
                            d.child(
                                button("accept-all", "Accept all")
                                    .on_click(cx.listener(move |app, _, _, cx| app.accept(actionable.clone(), cx))),
                            )
                        })
                        .child(button("dismiss-all", "Dismiss").on_click(cx.listener(|app, _, _, cx| {
                            app.proposals.clear();
                            cx.notify();
                        }))),
                )
                .children(cards),
        )
    }
}

/// The node title, clamped to three lines that end in `…`. `line_clamp` alone
/// limits the lines but clips the last one mid-character.
fn title_text(title: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id("title-text")
        .debug_selector(|| "title-text".to_owned())
        .flex_1()
        .min_w(px(0.))
        .line_clamp(3)
        .text_ellipsis()
        .text_lg()
        .font_weight(gpui::FontWeight::SEMIBOLD)
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
