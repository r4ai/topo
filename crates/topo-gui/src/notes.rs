//! Editing the notes of the selected node in the inspector.
//!
//! Notes are longer than a field and worth more: ⌘Enter saves them and closes
//! the editor. Anything else that leaves the editor (Escape, a click elsewhere,
//! another selection, closing the window) closes it without a word if nothing
//! changed since it opened, and otherwise asks whether to save, discard or keep
//! editing. A cloud workspace has no file to open in an editor, so this is how
//! its notes are written.

use std::collections::BTreeSet;

use gpui::{AnyElement, App, Context, Entity, Focusable, KeyDownEvent, MouseButton, Window, div, prelude::*, px};
use topo_core::{Edit, NodeId};

use crate::TopoApp;
use crate::text_input::{InputEvent, TextInput};
use crate::theme::{self, button, kbd, tinted_button};

/// What happens once unsaved notes are settled.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Then {
    /// Nothing more: the editor closes.
    Stay,
    /// Another selection that was asked for, which the question held back.
    Select {
        selected: Option<NodeId>,
        nodes: BTreeSet<NodeId>,
    },
    CloseWindow,
    Quit,
}

impl TopoApp {
    pub(crate) fn notes_input(&self) -> &Entity<TextInput> {
        &self.notes_input
    }

    /// Opens the notes of the selected node for editing, with the cursor at their end.
    pub(crate) fn start_notes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(node) = self.selected_node() else { return };
        let (id, body) = (node.id.clone(), node.body.trim_end().to_owned());
        self.notes_input.update(cx, |input, cx| {
            input.reset("", "Markdown notes…", cx);
            input.set_text(&body, cx);
        });
        self.notes = Some(id);
        self.notes_base = body;
        self.notes_ask = None;
        self.show_help = false;
        window.focus(&self.notes_input.focus_handle(cx), cx);
        cx.notify();
    }

    /// Whether the editor holds text that differs from what it opened with.
    /// Trailing whitespace does not count: saving drops it.
    pub(crate) fn notes_dirty(&self, cx: &App) -> bool {
        self.notes.is_some() && self.notes_input.read(cx).text().trim_end() != self.notes_base
    }

    /// Closes the editor, saving what it holds unless told to discard it.
    pub(crate) fn finish_notes(&mut self, save: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.notes.take() else { return };
        self.notes_ask = None;
        window.focus(&self.focus, cx);
        cx.notify();
        if !save || self.graph().get(&id).is_none() {
            return;
        }
        // A file ends with a line break; empty notes are no text at all.
        let body = match self.notes_input.read(cx).text().trim_end() {
            "" => String::new(),
            text => format!("{text}\n"),
        };
        self.mutate(cx, |graph| graph.edit(&id, Edit { body: Some(body), ..Edit::default() }));
    }

    /// Leaves the editor: at once if nothing changed, otherwise after asking.
    pub(crate) fn leave_notes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.notes_dirty(cx) {
            true => self.ask_notes(Then::Stay, cx),
            false => self.finish_notes(false, window, cx),
        }
    }

    /// Puts the question of what to do with the unsaved notes, to be followed by `then`.
    /// The question takes the keyboard when the window is next drawn.
    pub(crate) fn ask_notes(&mut self, then: Then, cx: &mut Context<Self>) {
        self.notes_ask = Some(then);
        cx.notify();
    }

    /// Answers the question: `Some(true)` saves, `Some(false)` discards, `None` goes on editing.
    pub(crate) fn answer_notes(&mut self, save: Option<bool>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(then) = self.notes_ask.take() else { return };
        let Some(save) = save else {
            window.focus(&self.notes_input.focus_handle(cx), cx);
            return cx.notify();
        };
        self.finish_notes(save, window, cx);
        match then {
            Then::Stay => {}
            Then::Select { selected, nodes } => {
                self.selected = selected;
                self.selected_nodes = nodes;
            }
            Then::CloseWindow => {
                if self.can_leave(cx) {
                    window.remove_window();
                }
            }
            Then::Quit => {
                if self.can_leave(cx) {
                    cx.quit();
                }
            }
        }
    }

    /// Keeps the editor and the selection in step each frame: the editor belongs
    /// to one selected node and to the focus. Leaving it with changes asks first,
    /// and a selection that moved away is put back until the question is answered.
    pub(crate) fn sync_notes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.notes.clone() else { return };
        if self.graph().get(&id).is_none() {
            if self.notes_dirty(cx) {
                self.toast("The node was deleted, so its unsaved notes were discarded", true, cx);
            }
            return self.finish_notes(false, window, cx);
        }
        let writing = self.notes_input.focus_handle(cx).is_focused(window);
        let moved = self.selected_node().is_none_or(|n| n.id != id);
        let asking = self.notes_ask.is_some();
        if !asking && writing && !moved {
            return;
        }
        if !asking && !self.notes_dirty(cx) {
            return self.finish_notes(false, window, cx);
        }
        if moved && matches!(self.notes_ask, None | Some(Then::Stay)) {
            let then = Then::Select {
                selected: self.selected.replace(id.clone()),
                nodes: std::mem::replace(&mut self.selected_nodes, BTreeSet::from([id])),
            };
            self.ask_notes(then, cx);
        } else if !asking {
            self.ask_notes(Then::Stay, cx);
        }
        // The question has the keyboard, not the text it is about.
        if !self.focus.is_focused(window) {
            window.focus(&self.focus, cx);
        }
    }

    /// Keys while the question is open.
    pub(crate) fn on_notes_ask_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let answer = match (keystroke.key.as_str(), keystroke.modifiers.modified()) {
            ("enter" | "s", _) => Some(Some(true)),
            ("d", false) => Some(Some(false)),
            ("escape", false) => Some(None),
            _ => None,
        };
        if let Some(answer) = answer {
            self.answer_notes(answer, window, cx);
        }
    }

    /// The question over the whole window, which holds the click and the keys until it is answered.
    pub(crate) fn notes_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current();
        let title =
            self.notes.as_ref().and_then(|id| self.graph().get(id)).map_or_else(String::new, |n| n.title.clone());
        let choice = |button: gpui::Stateful<gpui::Div>, key: &'static str, save: Option<bool>| -> AnyElement {
            button
                .child(kbd(key))
                .on_click(cx.listener(move |app, _, window, cx| app.answer_notes(save, window, cx)))
                .into_any_element()
        };
        div()
            .id("notes-ask")
            .absolute()
            .size_full()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(t.scrim)
            .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| cx.stop_propagation()))
            .child(
                div()
                    .debug_selector(|| "notes-dialog".to_owned())
                    .w(px(380.))
                    .max_w(gpui::relative(0.92))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_5()
                    .rounded_xl()
                    .bg(t.surface)
                    .border_1()
                    .border_color(t.border_strong)
                    .shadow(theme::shadow())
                    .child(div().text_base().font_weight(gpui::FontWeight::SEMIBOLD).child("Save the notes?"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(t.fg_muted)
                            .child(format!("The notes of “{title}” have changes that are not saved.")),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .pt_1()
                            .child(choice(tinted_button("notes-discard", "Discard", t.danger), "D", Some(false)))
                            .child(choice(button("notes-keep", "Keep editing"), "Esc", None))
                            .child(choice(tinted_button("notes-save", "Save", t.accent), "↵", Some(true))),
                    ),
            )
    }

    pub(crate) fn on_notes_event(
        &mut self,
        _: &Entity<TextInput>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Submit => self.finish_notes(true, window, cx),
            InputEvent::Cancel => self.leave_notes(window, cx),
            _ => {}
        }
        cx.notify();
    }
}
