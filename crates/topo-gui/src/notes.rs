//! Editing the notes of the selected node in the inspector.
//!
//! Notes are longer than a field and worth more: ⌘Enter saves them and so does
//! anything that leaves the editor (a click elsewhere, another selection).
//! Only Escape discards the changes. A cloud workspace has no file to open in
//! an editor, so this is how its notes are written.

use gpui::{Context, Entity, Focusable, Window};
use topo_core::Edit;

use crate::TopoApp;
use crate::text_input::{InputEvent, TextInput};

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
        self.show_help = false;
        window.focus(&self.notes_input.focus_handle(cx), cx);
        cx.notify();
    }

    /// Closes the editor, saving what it holds unless told to discard it.
    pub(crate) fn finish_notes(&mut self, save: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.notes.take() else { return };
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

    pub(crate) fn on_notes_event(
        &mut self,
        _: &Entity<TextInput>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Submit => self.finish_notes(true, window, cx),
            InputEvent::Cancel => self.finish_notes(false, window, cx),
            _ => {}
        }
        cx.notify();
    }
}
