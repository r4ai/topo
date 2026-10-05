//! Text field with IME support, adapted from gpui's `input` example.
//!
//! Enter, Escape, Up, Down, Tab and Shift-Tab are not handled here; they are
//! emitted as [`InputEvent`]s so the owner decides what they mean.
//!
//! A field made with [`TextInput::multiline`] holds several lines and wraps
//! them to its width: Enter breaks the line, Up and Down move between rows,
//! and ⌘Enter submits. Home and End (and ⌘←, ⌘→, ⌃A, ⌃E) go to the start and
//! end of the line between line breaks; ⌘↑ and ⌘↓ go to those of the text.
//! One made with [`TextInput::markdown`] also colors Markdown, indents with Tab
//! and carries lists on with Enter, and keeps its cursor in view of a scroller.
//!
//! The standard editing shortcuts ([`SelectAll`], [`Copy`], [`Cut`], [`Paste`],
//! [`Undo`], [`Redo`]) are bound for the whole window. A focused field handles
//! them for its text and lets none through, so they reach the canvas only when
//! no field has the focus.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Bounds, ClipboardItem, ContentMask, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, FontStyle, FontWeight, GlobalElementId, KeyBinding,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ScrollHandle,
    SharedString, Style, Task, TextRun, UTF16Selection, UnderlineStyle, Window, WrappedLine, actions, div, fill, point,
    prelude::*, px, relative, size,
};
use smallvec::SmallVec;
use unicode_segmentation::UnicodeSegmentation;

use crate::markdown::{self, Change, Span};
use crate::theme;

actions!(
    text_input,
    [
        Backspace,
        Delete,
        DeleteWord,
        DeleteToStart,
        Left,
        Right,
        WordLeft,
        WordRight,
        SelectLeft,
        SelectRight,
        SelectHome,
        SelectEnd,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
        Undo,
        Redo,
        ShowCharacterPalette,
        Submit,
        Cancel,
        Up,
        Down,
        Next,
        Previous,
        Newline,
        /// A line break that does not continue the list the line is in.
        PlainNewline,
        DocStart,
        DocEnd,
        SelectDocStart,
        SelectDocEnd,
        SelectWordLeft,
        SelectWordRight,
        Indent,
        Outdent,
    ]
);

const CONTEXT: &str = "TextInput";
/// Key context of a multiline field.
const AREA: &str = "TextArea";
/// The modifier of the standard editing shortcuts.
const MOD: &str = if cfg!(target_os = "macos") { "cmd" } else { "ctrl" };
/// Text states kept for undo.
const HISTORY: usize = 200;

pub fn bind_keys(cx: &mut App) {
    let standard = |key: &str| format!("{MOD}-{key}");
    cx.bind_keys([
        KeyBinding::new(&standard("a"), SelectAll, None),
        KeyBinding::new(&standard("c"), Copy, None),
        KeyBinding::new(&standard("x"), Cut, None),
        KeyBinding::new(&standard("v"), Paste, None),
        KeyBinding::new(&standard("z"), Undo, None),
        KeyBinding::new(&standard("shift-z"), Redo, None),
    ]);
    // Emacs-style line start, where Control is not the editing modifier.
    #[cfg(target_os = "macos")]
    cx.bind_keys([KeyBinding::new("ctrl-a", Home, Some(CONTEXT)), KeyBinding::new("ctrl-a", Home, Some(AREA))]);
    for context in [CONTEXT, AREA] {
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, Some(context)),
            KeyBinding::new("delete", Delete, Some(context)),
            KeyBinding::new("alt-backspace", DeleteWord, Some(context)),
            KeyBinding::new("cmd-backspace", DeleteToStart, Some(context)),
            KeyBinding::new("left", Left, Some(context)),
            KeyBinding::new("right", Right, Some(context)),
            KeyBinding::new("alt-left", WordLeft, Some(context)),
            KeyBinding::new("alt-right", WordRight, Some(context)),
            KeyBinding::new("shift-left", SelectLeft, Some(context)),
            KeyBinding::new("shift-right", SelectRight, Some(context)),
            KeyBinding::new("shift-home", SelectHome, Some(context)),
            KeyBinding::new("cmd-shift-left", SelectHome, Some(context)),
            KeyBinding::new("shift-end", SelectEnd, Some(context)),
            KeyBinding::new("cmd-shift-right", SelectEnd, Some(context)),
            KeyBinding::new("home", Home, Some(context)),
            KeyBinding::new("cmd-left", Home, Some(context)),
            KeyBinding::new("end", End, Some(context)),
            KeyBinding::new("cmd-right", End, Some(context)),
            KeyBinding::new("ctrl-e", End, Some(context)),
            KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some(context)),
            KeyBinding::new("escape", Cancel, Some(context)),
            KeyBinding::new("up", Up, Some(context)),
            KeyBinding::new("down", Down, Some(context)),
        ]);
    }
    cx.bind_keys([
        KeyBinding::new("enter", Submit, Some(CONTEXT)),
        KeyBinding::new("tab", Next, Some(CONTEXT)),
        KeyBinding::new("shift-tab", Previous, Some(CONTEXT)),
        KeyBinding::new("enter", Newline, Some(AREA)),
        KeyBinding::new("shift-enter", PlainNewline, Some(AREA)),
        KeyBinding::new(&format!("{MOD}-enter"), Submit, Some(AREA)),
        KeyBinding::new("tab", Indent, Some(AREA)),
        KeyBinding::new("shift-tab", Outdent, Some(AREA)),
        KeyBinding::new("cmd-up", DocStart, Some(AREA)),
        KeyBinding::new("ctrl-home", DocStart, Some(AREA)),
        KeyBinding::new("cmd-down", DocEnd, Some(AREA)),
        KeyBinding::new("ctrl-end", DocEnd, Some(AREA)),
        KeyBinding::new("cmd-shift-up", SelectDocStart, Some(AREA)),
        KeyBinding::new("ctrl-shift-home", SelectDocStart, Some(AREA)),
        KeyBinding::new("cmd-shift-down", SelectDocEnd, Some(AREA)),
        KeyBinding::new("ctrl-shift-end", SelectDocEnd, Some(AREA)),
        KeyBinding::new("alt-shift-left", SelectWordLeft, Some(AREA)),
        KeyBinding::new("alt-shift-right", SelectWordRight, Some(AREA)),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Changed,
    Submit,
    Cancel,
    Up,
    Down,
    /// Tab and Shift-Tab.
    Next,
    Previous,
    /// Backspace with nothing left to delete.
    BackspaceEmpty,
}

pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    /// A completion shown faintly after the text while the cursor is at its end.
    ghost: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    /// Several lines: Enter breaks the line.
    multiline: bool,
    /// The text is Markdown: it is colored, and Enter and Tab know about lists.
    markdown: bool,
    /// The scroller around the field, which moves to keep the cursor in view.
    scroll: Option<ScrollHandle>,
    /// The cursor moved since the last frame and is to be brought into view.
    reveal: bool,
    /// The text wraps to the width of the field instead of scrolling sideways.
    wrap: bool,
    /// When wrapping, the most rows the field keeps; longer text scrolls to the cursor.
    max_rows: Option<usize>,
    /// Whether the caret is in its visible half of the blink.
    caret_on: bool,
    /// Drives the caret blink while the field is focused; cancelled when it is not.
    blink: Option<Task<()>>,
    /// The shaped text of the last frame and where it was painted.
    layout: Option<Layout>,
    is_selecting: bool,
    /// Earlier states of the text and redone ones, newest last.
    undo: Vec<TextState>,
    redo: Vec<TextState>,
    /// The last edit typed text at the cursor; more typing joins its undo step.
    typing: bool,
    /// The Markdown spans of the text they were computed for, so that frames
    /// that draw unchanged text do not tokenize it again.
    spans: std::cell::RefCell<Option<(SharedString, Rc<[Span]>)>>,
}

/// The content and the selection.
type TextState = (SharedString, Range<usize>);

impl EventEmitter<InputEvent> for TextInput {}

impl TextInput {
    /// The Markdown spans of the content, computed once per text.
    fn spans(&self) -> Rc<[Span]> {
        let mut cache = self.spans.borrow_mut();
        match &*cache {
            Some((text, spans)) if *text == self.content => spans.clone(),
            _ => {
                let spans: Rc<[Span]> = markdown::highlight(&self.content).into();
                *cache = Some((self.content.clone(), spans.clone()));
                spans
            }
        }
    }

    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: SharedString::default(),
            placeholder: SharedString::default(),
            ghost: SharedString::default(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            multiline: false,
            markdown: false,
            scroll: None,
            reveal: false,
            wrap: false,
            max_rows: None,
            caret_on: true,
            blink: None,
            layout: None,
            is_selecting: false,
            undo: Vec::new(),
            redo: Vec::new(),
            typing: false,
            spans: Default::default(),
        }
    }

    /// A field of several lines.
    pub fn multiline(cx: &mut Context<Self>) -> Self {
        Self { multiline: true, wrap: true, ..Self::new(cx) }
    }

    /// A field of several lines of Markdown, inside `scroll`.
    pub fn markdown(scroll: ScrollHandle, cx: &mut Context<Self>) -> Self {
        Self { markdown: true, scroll: Some(scroll), ..Self::multiline(cx) }
    }

    /// Makes one line of text wrap into rows as wide as the field, or scroll sideways.
    pub fn set_wrap(&mut self, wrap: bool, cx: &mut Context<Self>) {
        self.wrap = wrap || self.multiline;
        cx.notify();
    }

    /// Caps a wrapping field at `rows` rows; longer text scrolls to keep the cursor in view.
    pub fn set_max_rows(&mut self, rows: Option<usize>, cx: &mut Context<Self>) {
        self.max_rows = rows.filter(|rows| *rows > 0);
        cx.notify();
    }

    /// Runs the caret blink while the field is focused. Tests and captures keep the caret solid
    /// so their frames stay deterministic.
    #[cfg(any(test, feature = "screenshot"))]
    fn sync_blink(&mut self, _window: &Window, _cx: &mut Context<Self>) {
        let _ = self.blink.take();
        self.caret_on = true;
    }

    #[cfg(all(not(test), not(feature = "screenshot")))]
    fn sync_blink(&mut self, window: &Window, cx: &mut Context<Self>) {
        if !self.focus_handle.is_focused(window) {
            if self.blink.take().is_some() {
                self.caret_on = true;
            }
            return;
        }
        if self.blink.is_none() {
            self.blink = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(std::time::Duration::from_millis(530)).await;
                    if this
                        .update(cx, |input, cx| {
                            input.caret_on = !input.caret_on;
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    /// The selected bytes; an empty range is the cursor.
    #[cfg(test)]
    pub fn selection(&self) -> Range<usize> {
        self.selected_range.clone()
    }

    /// Replaces the content and selects all of it, so typing overwrites it.
    pub fn reset(&mut self, text: &str, placeholder: &str, cx: &mut Context<Self>) {
        self.content = text.to_owned().into();
        self.placeholder = placeholder.to_owned().into();
        self.selected_range = 0..self.content.len();
        self.selection_reversed = false;
        self.marked_range = None;
        self.undo.clear();
        self.redo.clear();
        self.typing = false;
        self.caret_on = true;
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    /// Replaces the content, with the cursor at its end.
    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let placeholder = self.placeholder.clone();
        self.reset(text, &placeholder, cx);
        self.selected_range = self.content.len()..self.content.len();
    }

    /// Sets the completion shown after the text. It is display only: the owner
    /// decides what accepts it.
    pub fn set_ghost(&mut self, ghost: &str, cx: &mut Context<Self>) {
        if self.ghost.as_ref() != ghost {
            self.ghost = ghost.to_owned().into();
            cx.notify();
        }
    }

    /// Whether an input method is composing text that is not final yet.
    pub fn composing(&self) -> bool {
        self.marked_range.is_some()
    }

    /// Records the current text as an undo step, before an edit replaces it.
    fn checkpoint(&mut self) {
        self.undo.push((self.content.clone(), self.selected_range.clone()));
        if self.undo.len() > HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Swaps the text with the newest state of `undo` (or `redo`). The key is
    /// consumed even when there is none, so it never reaches the graph's history.
    fn restore(&mut self, undo: bool, cx: &mut Context<Self>) {
        let (from, to) = match undo {
            true => (&mut self.undo, &mut self.redo),
            false => (&mut self.redo, &mut self.undo),
        };
        let Some((content, selection)) = from.pop() else { return };
        to.push((self.content.clone(), self.selected_range.clone()));
        self.content = content;
        self.selected_range = selection;
        self.selection_reversed = false;
        self.marked_range = None;
        self.typing = false;
        self.reveal = true;
        self.caret_on = true;
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    /// Replaces part of the text as one undo step. Text typed right after it
    /// joins the step if `typing`.
    fn apply(&mut self, change: Change, typing: bool, cx: &mut Context<Self>) {
        self.checkpoint();
        self.content =
            (self.content[..change.range.start].to_owned() + &change.text + &self.content[change.range.end..]).into();
        self.selected_range = change.selection;
        self.selection_reversed = false;
        self.marked_range = None;
        self.typing = typing;
        self.reveal = true;
        self.caret_on = true;
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        match self.selected_range.is_empty() {
            true => self.move_to(self.previous_boundary(self.cursor_offset()), cx),
            false => self.move_to(self.selected_range.start, cx),
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        match self.selected_range.is_empty() {
            true => self.move_to(self.next_boundary(self.selected_range.end), cx),
            false => self.move_to(self.selected_range.end, cx),
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.previous_word(self.cursor_offset()), cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.next_word(self.cursor_offset()), cx);
    }

    fn word_select_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_word(self.cursor_offset()), cx);
    }

    fn word_select_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_word(self.cursor_offset()), cx);
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.line_start(), cx);
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.line_end(), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.line_start(), cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.line_end(), cx);
    }

    /// Where Home goes: the start of the line the cursor is on, or of the one line of a single-line field.
    fn line_start(&self) -> usize {
        match self.multiline {
            true => markdown::line_start(&self.content, self.cursor_offset()),
            false => 0,
        }
    }

    fn line_end(&self) -> usize {
        match self.multiline {
            true => markdown::line_end(&self.content, self.cursor_offset()),
            false => self.content.len(),
        }
    }

    /// Enter: a line break, which in Markdown carries on the list the line is in.
    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        match self.markdown {
            true => self.apply(markdown::newline(&self.content, &self.selected_range), true, cx),
            false => self.replace_text_in_range(None, "\n", window, cx),
        }
    }

    fn indent(&mut self, _: &Indent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(change) = markdown::indent(&self.content, &self.selected_range) {
            self.apply(change, false, cx);
        }
    }

    fn outdent(&mut self, _: &Outdent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(change) = markdown::outdent(&self.content, &self.selected_range) {
            self.apply(change, false, cx);
        }
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.content.is_empty() {
            return cx.emit(InputEvent::BackspaceEmpty);
        }
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete_word(&mut self, _: &DeleteWord, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_word(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete_to_start(&mut self, _: &DeleteToStart, window: &mut Window, cx: &mut Context<Self>) {
        let cursor = self.cursor_offset();
        // At the start of a line the line break before it goes.
        let start = match self.line_start() {
            start if start == cursor && self.multiline => self.previous_boundary(cursor),
            start => start,
        };
        self.selected_range = start..cursor;
        self.replace_text_in_range(None, "", window, cx)
    }

    fn show_character_palette(&mut self, _: &ShowCharacterPalette, window: &mut Window, _: &mut Context<Self>) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let text = match self.multiline {
                true => text.replace("\r\n", "\n"),
                // The field has one line.
                false => text.lines().collect::<Vec<_>>().join(" "),
            };
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(self.content[self.selected_range.clone()].to_string()));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(self.content[self.selected_range.clone()].to_string()));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn on_mouse_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.is_selecting = true;
        match event.modifiers.shift {
            true => self.select_to(self.index_for_mouse_position(event.position), cx),
            false => self.move_to(self.index_for_mouse_position(event.position), cx),
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.typing = false;
        self.reveal = true;
        self.caret_on = true;
        self.selected_range = offset..offset;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        match self.selection_reversed {
            true => self.selected_range.start,
            false => self.selected_range.end,
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let Some(layout) = self.layout.as_ref().filter(|_| !self.content.is_empty()) else { return 0 };
        // The completion after the text is not part of it.
        layout.offset_at(position - layout.origin).min(self.content.len())
    }

    /// Up and Down: in a field of several lines they move the cursor a row, keeping its column.
    fn vertical(&mut self, down: bool, cx: &mut Context<Self>) {
        let Some(layout) = self.layout.as_ref().filter(|_| self.multiline) else {
            return cx.emit(if down { InputEvent::Down } else { InputEvent::Up });
        };
        let at = layout.position(self.cursor_offset());
        let row = layout.line_height;
        let y = at.y + if down { row } else { -row };
        let offset = match () {
            _ if y < px(0.) => 0,
            _ if y >= layout.height() => self.content.len(),
            _ => layout.offset_at(point(at.x, y + row / 2.)).min(self.content.len()),
        };
        self.move_to(offset, cx);
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.reveal = true;
        self.caret_on = true;
        match self.selection_reversed {
            true => self.selected_range.start = offset,
            false => self.selected_range.end = offset,
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        utf8_offset(&self.content, offset)
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content.grapheme_indices(true).rev().find_map(|(idx, _)| (idx < offset).then_some(idx)).unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    /// Start of the word before `offset`.
    fn previous_word(&self, offset: usize) -> usize {
        self.content.unicode_word_indices().rev().map(|(start, _)| start).find(|start| *start < offset).unwrap_or(0)
    }

    /// End of the word after `offset`.
    fn next_word(&self, offset: usize) -> usize {
        self.content
            .unicode_word_indices()
            .map(|(start, word)| start + word.len())
            .find(|end| *end > offset)
            .unwrap_or(self.content.len())
    }
}

/// Byte offset in `text` of the UTF-16 offset `utf16`, clamped to the end.
fn utf8_offset(text: &str, utf16: usize) -> usize {
    let mut utf8_offset = 0;
    let mut utf16_count = 0;
    for ch in text.chars() {
        if utf16_count >= utf16 {
            break;
        }
        utf16_count += ch.len_utf16();
        utf8_offset += ch.len_utf8();
    }
    utf8_offset
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: self.range_to_utf16(&self.selected_range), reversed: self.selection_reversed })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range.as_ref().map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        if range.is_empty() && new_text.is_empty() {
            return;
        }
        // A composition was recorded when it started; typing extends one step.
        let typed = range.is_empty();
        if self.marked_range.is_none() && !(typed && self.typing) {
            self.checkpoint();
        }
        self.typing = typed;
        self.content = (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..]).into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        self.reveal = true;
        self.caret_on = true;
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        if self.marked_range.is_none() {
            self.checkpoint();
        }
        self.typing = false;
        self.content = (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..]).into();
        self.marked_range = (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        // The IME reports the selection relative to the text it just inserted.
        let within =
            new_selected_range_utf16.unwrap_or(new_text.encode_utf16().count()..new_text.encode_utf16().count());
        self.selected_range =
            range.start + utf8_offset(new_text, within.start)..range.start + utf8_offset(new_text, within.end);
        self.reveal = true;
        self.caret_on = true;
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let (start, end) = (layout.position(range.start), layout.position(range.end));
        Some(Bounds::from_corners(layout.origin + start, layout.origin + end + point(px(0.), layout.line_height)))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let layout = self.layout.as_ref()?;
        Some(self.offset_to_utf16(layout.offset_at(point - layout.origin).min(self.content.len())))
    }
}

/// The shaped lines of the text, each wrapped into one or more rows, and
/// where their top left corner was painted.
struct Layout {
    lines: SmallVec<[WrappedLine; 1]>,
    line_height: Pixels,
    origin: Point<Pixels>,
}

impl Layout {
    fn rows(line: &WrappedLine) -> usize {
        line.wrap_boundaries().len() + 1
    }

    fn height(&self) -> Pixels {
        self.line_height * self.lines.iter().map(Self::rows).sum::<usize>().max(1) as f32
    }

    /// Top left corner of the character at byte `offset`, relative to the origin.
    fn position(&self, offset: usize) -> Point<Pixels> {
        let (mut start, mut top) = (0, px(0.));
        for line in &self.lines {
            if offset <= start + line.len() {
                let within = line.position_for_index(offset - start, self.line_height).unwrap_or_default();
                return point(within.x, top + within.y);
            }
            // The line break after it.
            start += line.len() + 1;
            top += self.line_height * Self::rows(line) as f32;
        }
        point(px(0.), top)
    }

    /// Byte offset of the character boundary closest to `position`, relative to the origin.
    fn offset_at(&self, position: Point<Pixels>) -> usize {
        let (mut start, mut top) = (0, px(0.));
        for (index, line) in self.lines.iter().enumerate() {
            let height = self.line_height * Self::rows(line) as f32;
            if position.y < top + height || index + 1 == self.lines.len() {
                let within = point(position.x, (position.y - top).clamp(px(0.), height - px(1.)));
                let (Ok(offset) | Err(offset)) = line.closest_index_for_position(within, self.line_height);
                return start + offset;
            }
            start += line.len() + 1;
            top += height;
        }
        0
    }

    /// One rectangle per row for the bytes `range`, in a field `width` wide.
    fn selection(&self, range: &Range<usize>, width: Pixels) -> Vec<Bounds<Pixels>> {
        let mut rows = Vec::new();
        let mut start = 0;
        for line in &self.lines {
            let end = start + line.len();
            let (from, to) = (range.start.max(start), range.end.min(end));
            let mut row: Option<(Point<Pixels>, Pixels)> = None;
            // The row a character is on is known only by asking where it is.
            let boundaries = line.text.char_indices().map(|(i, _)| i).chain([line.len()]);
            for index in boundaries.filter(|i| (from..=to).contains(&(start + i))).take_while(|_| from <= to) {
                let at = self.position(start + index);
                match &mut row {
                    Some((corner, right)) if corner.y == at.y => *right = at.x,
                    None => row = Some((at, at.x)),
                    Some((corner, _)) => {
                        // The previous row ran on to its end, and this one starts at the left.
                        rows.push(Bounds::from_corners(*corner, point(width, corner.y)));
                        row = Some((point(px(0.), at.y), at.x));
                    }
                }
            }
            // A selected line break shows as a sliver after the line.
            let past = if range.end > end && from <= to { px(6.) } else { px(0.) };
            rows.extend(row.map(|(corner, right)| Bounds::from_corners(corner, point(right + past, corner.y))));
            start = end + 1;
        }
        rows.into_iter().map(|b| Bounds::new(self.origin + b.origin, size(b.size.width, self.line_height))).collect()
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    layout: Option<Layout>,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl TextElement {
    /// The text to show and its runs: the content with a composition underlined
    /// and the completion faint after it, or the placeholder.
    fn display(&self, window: &Window, cx: &App) -> (SharedString, Vec<TextRun>) {
        let input = self.input.read(cx);
        let style = window.text_style();
        let (text, color) = match input.content.is_empty() {
            true => (input.placeholder.clone(), theme::current().fg_faint.into()),
            false => (input.content.clone(), style.color),
        };
        let typed = text.len();
        let at_end = input.selected_range.is_empty()
            && input.cursor_offset() == input.content.len()
            && input.marked_range.is_none();
        let ghost = match !input.content.is_empty() && at_end && !input.wrap {
            true => input.ghost.clone(),
            false => SharedString::default(),
        };
        let run = TextRun {
            len: typed,
            font: style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let ghost_run = TextRun { len: ghost.len(), color: theme::current().fg_faint.into(), ..run.clone() };
        let spans = match input.markdown && !input.content.is_empty() {
            true => input.spans(),
            false => Rc::new([Span { len: typed, style: markdown::Style::Plain }]),
        };
        let mut runs: Vec<TextRun> = spans.iter().map(|span| styled(&run, *span)).collect();
        if let Some(marked) = input.marked_range.as_ref() {
            runs = underlined(runs, marked);
        }
        if ghost.is_empty() {
            return (text, runs);
        }
        runs.push(ghost_run);
        (format!("{text}{ghost}").into(), runs)
    }
}

/// `run` as `span` draws it: colored, bold or italic as its Markdown style says.
fn styled(run: &TextRun, span: Span) -> TextRun {
    let mut font = run.font.clone();
    match span.style {
        markdown::Style::Strong => font.weight = FontWeight::BOLD,
        markdown::Style::Heading => font.weight = FontWeight::SEMIBOLD,
        markdown::Style::Emphasis => font.style = FontStyle::Italic,
        _ => {}
    }
    let color = theme::markdown_color(span.style).map_or(run.color, Into::into);
    TextRun { len: span.len, font, color, ..run.clone() }
}

/// `runs` with the bytes `marked` underlined, as an input method shows a composition.
fn underlined(runs: Vec<TextRun>, marked: &Range<usize>) -> Vec<TextRun> {
    let mut start = 0;
    let mut out = Vec::new();
    for run in runs {
        let end = start + run.len;
        let (from, to) = (marked.start.clamp(start, end), marked.end.clamp(start, end));
        let underline = Some(UnderlineStyle { color: Some(run.color), thickness: px(1.0), wavy: false });
        for (len, underline) in [(from - start, None), (to - from, underline), (end - to, None)] {
            if len > 0 {
                out.push(TextRun { len, underline, ..run.clone() });
            }
        }
        start = end;
    }
    out
}

/// Scrolls `scroll` as little as it takes to bring `caret` into view, with a line to spare.
fn reveal(scroll: &ScrollHandle, caret: Bounds<Pixels>, window: &mut Window) {
    let view = scroll.bounds();
    let margin = caret.size.height;
    let (top, bottom) = (view.top() + margin, view.bottom() - margin);
    if top >= bottom {
        return;
    }
    let by = match () {
        _ if caret.top() < top => top - caret.top(),
        _ if caret.bottom() > bottom => bottom - caret.bottom(),
        _ => return,
    };
    let mut offset = scroll.offset();
    offset.y = (offset.y + by).clamp(-scroll.max_offset().y, px(0.));
    if offset != scroll.offset() {
        scroll.set_offset(offset);
        window.request_animation_frame();
    }
}

/// The font size of the text style in effect. It is in effect while an
/// element is laid out and painted, not when its measure function runs later.
fn font_size(window: &Window) -> Pixels {
    window.text_style().font_size.to_pixels(window.rem_size())
}

/// Shapes `text`, wrapped to `width` if given.
fn shape(
    text: SharedString,
    runs: &[TextRun],
    font_size: Pixels,
    width: Option<Pixels>,
    window: &Window,
) -> SmallVec<[WrappedLine; 1]> {
    window.text_system().shape_text(text, font_size, runs, width, None).unwrap_or_default()
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        if !self.input.read(cx).wrap {
            style.size.height = window.line_height().into();
            return (window.request_layout(style, [], cx), ());
        }
        // The height follows from the rows the text wraps into at the width it is given.
        let (text, runs) = self.display(window, cx);
        let (line_height, font_size) = (window.line_height(), font_size(window));
        let max_rows = self.input.read(cx).max_rows;
        let measure = move |known: gpui::Size<Option<Pixels>>,
                            available: gpui::Size<gpui::AvailableSpace>,
                            window: &mut Window,
                            _: &mut App| {
            let width = known.width.or(match available.width {
                gpui::AvailableSpace::Definite(width) => Some(width),
                _ => None,
            });
            let lines = shape(text.clone(), &runs, font_size, width, window);
            let layout = Layout { lines, line_height, origin: Point::default() };
            let height = match max_rows {
                Some(rows) => layout.height().min(line_height * rows as f32),
                None => layout.height(),
            };
            size(width.unwrap_or_default(), height)
        };
        (window.request_measured_layout(style, measure), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let (text, runs) = self.display(window, cx);
        let input = self.input.read(cx);
        let (selected, cursor) = (input.selected_range.clone(), input.cursor_offset());
        let (caret_on, composing) = (input.caret_on, input.marked_range.is_some());
        let lines = shape(text, &runs, font_size(window), input.wrap.then_some(bounds.size.width), window);
        let mut layout = Layout { lines, line_height: window.line_height(), origin: bounds.origin };
        let cursor_width = px(2.);
        // A capped field scrolls up so the cursor's row stays in view.
        if let Some(rows) = input.max_rows {
            let max_height = layout.line_height * rows as f32;
            let below = layout.position(cursor).y + layout.line_height - max_height;
            layout.origin.y -= below.max(px(0.));
        }
        // One line moves left so that the cursor of a long text stays inside the field.
        let overflow = (layout.position(cursor).x + cursor_width - bounds.size.width).max(px(0.));
        if !input.wrap {
            layout.origin.x -= overflow;
        }
        let caret = Bounds::new(layout.origin + layout.position(cursor), size(cursor_width, layout.line_height));
        let selection = layout.selection(&selected, bounds.size.width);
        let reveal_now = input.reveal;
        let scroll = input.scroll.clone().filter(|_| reveal_now);
        if reveal_now {
            self.input.update(cx, |input, _| input.reveal = false);
        }
        if let Some(scroll) = scroll {
            reveal(&scroll, caret, window);
        }
        // The caret is solid while composing or selected text; otherwise it blinks.
        let cursor = (selected.is_empty() && !composing && caret_on).then(|| fill(caret, theme::current().accent));
        PrepaintState {
            cursor,
            selection: selection.into_iter().map(|row| fill(row, theme::current().selection)).collect(),
            layout: Some(layout),
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        let layout = prepaint.layout.take().expect("prepaint always shapes the text");
        let text_bounds = Bounds::new(layout.origin, bounds.size);
        window.handle_input(&focus_handle, ElementInputHandler::new(text_bounds, self.input.clone()), cx);
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for row in prepaint.selection.drain(..) {
                window.paint_quad(row)
            }
            let mut origin = layout.origin;
            for line in &layout.lines {
                line.paint(origin, layout.line_height, gpui::TextAlign::Left, None, window, cx)
                    .expect("painting a shaped line");
                origin.y += layout.line_height * Layout::rows(line) as f32;
            }
            if focus_handle.is_focused(window)
                && let Some(cursor) = prepaint.cursor.take()
            {
                window.paint_quad(cursor);
            }
        });
        self.input.update(cx, |input, _| input.layout = Some(layout));
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_blink(window, cx);
        div()
            .flex()
            .w_full()
            .key_context(if self.multiline { AREA } else { CONTEXT })
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_word))
            .on_action(cx.listener(Self::delete_to_start))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::word_select_left))
            .on_action(cx.listener(Self::word_select_right))
            .on_action(cx.listener(Self::indent))
            .on_action(cx.listener(Self::outdent))
            .on_action(cx.listener(|input, _: &DocStart, _, cx| input.move_to(0, cx)))
            .on_action(cx.listener(|input, _: &DocEnd, _, cx| input.move_to(input.content.len(), cx)))
            .on_action(cx.listener(|input, _: &SelectDocStart, _, cx| input.select_to(0, cx)))
            .on_action(cx.listener(|input, _: &SelectDocEnd, _, cx| input.select_to(input.content.len(), cx)))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(|input, _: &Undo, _, cx| input.restore(true, cx)))
            .on_action(cx.listener(|input, _: &Redo, _, cx| input.restore(false, cx)))
            .on_action(cx.listener(|_, _: &Submit, _, cx| cx.emit(InputEvent::Submit)))
            .on_action(cx.listener(|_, _: &Cancel, _, cx| cx.emit(InputEvent::Cancel)))
            .on_action(cx.listener(|input, _: &Up, _, cx| input.vertical(false, cx)))
            .on_action(cx.listener(|input, _: &Down, _, cx| input.vertical(true, cx)))
            .on_action(cx.listener(Self::newline))
            .on_action(
                cx.listener(|input, _: &PlainNewline, window, cx| input.replace_text_in_range(None, "\n", window, cx)),
            )
            .on_action(cx.listener(|_, _: &Next, _, cx| cx.emit(InputEvent::Next)))
            .on_action(cx.listener(|_, _: &Previous, _, cx| cx.emit(InputEvent::Previous)))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(TextElement { input: cx.entity() })
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_offsets_map_to_byte_offsets() {
        // `か` is 1 UTF-16 unit and 3 bytes; `😀` is 2 units and 4 bytes.
        assert_eq!(utf8_offset("かa😀b", 0), 0);
        assert_eq!(utf8_offset("かa😀b", 1), 3);
        assert_eq!(utf8_offset("かa😀b", 2), 4);
        assert_eq!(utf8_offset("かa😀b", 4), 8);
        assert_eq!(utf8_offset("かa😀b", 99), 9);
    }

    fn run(len: usize) -> TextRun {
        TextRun {
            len,
            font: gpui::font("test"),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }

    #[test]
    fn highlighting_colors_the_text_without_changing_its_bytes() {
        let text = "# ヘッダ\n*強調* `code` plain";
        let runs: Vec<TextRun> = markdown::highlight(text).iter().map(|span| styled(&run(0), *span)).collect();
        assert_eq!(runs.iter().map(|run| run.len).sum::<usize>(), text.len());
        assert_eq!(runs[0].font.weight, FontWeight::SEMIBOLD);
        assert_eq!(runs[0].color, theme::current().md_heading.into());
        assert!(runs.iter().any(|run| run.font.style == FontStyle::Italic));
        assert!(runs.iter().all(|run| run.underline.is_none()));
    }

    #[test]
    fn a_composition_is_underlined_across_the_colored_runs() {
        // "ab" and "cd" differ in color; the composition covers "b" and "c".
        let runs = underlined(vec![run(2), TextRun { color: gpui::white(), ..run(2) }], &(1..3));
        let shape: Vec<_> = runs.iter().map(|run| (run.len, run.underline.is_some(), run.color)).collect();
        assert_eq!(
            shape,
            [(1, false, gpui::black()), (1, true, gpui::black()), (1, true, gpui::white()), (1, false, gpui::white())]
        );
        // Nothing marked, nothing underlined.
        assert!(underlined(vec![run(4)], &(0..0)).iter().all(|run| run.underline.is_none()));
    }

    #[gpui::test]
    fn spans_are_computed_once_per_text_and_follow_a_japanese_composition(cx: &mut gpui::TestAppContext) {
        use gpui::EntityInputHandler;
        let (input, cx) = cx.add_window_view(|_, cx| TextInput::markdown(ScrollHandle::new(), cx));
        input.update_in(cx, |input, window, cx| {
            input.set_text("# 見出し\n- 項目", cx);
            let first = input.spans();
            assert!(Rc::ptr_eq(&first, &input.spans()), "unchanged text is not tokenized again");
            // The composition is part of the content; its bytes are covered by the spans.
            input.replace_and_mark_text_in_range(None, "にほん", None, window, cx);
            let spans = input.spans();
            assert!(!Rc::ptr_eq(&first, &spans));
            assert!(input.content.ends_with("- 項目にほん"));
            assert_eq!(spans.iter().map(|span| span.len).sum::<usize>(), input.content.len());
            assert_eq!(input.marked_range, Some(input.content.len() - "にほん".len()..input.content.len()));
            input.replace_text_in_range(None, "日本", window, cx);
            assert!(input.content.ends_with("- 項目日本") && input.marked_range.is_none());
            assert_eq!(input.spans().iter().map(|span| span.len).sum::<usize>(), input.content.len());
        });
    }

    #[gpui::test]
    fn a_single_line_field_keeps_its_keys(cx: &mut gpui::TestAppContext) {
        cx.update(bind_keys);
        let (input, cx) = cx.add_window_view(|_, cx| TextInput::new(cx));
        input.update_in(cx, |input, window, cx| {
            input.set_text("ab cd", cx);
            window.focus(&input.focus_handle(cx), cx);
        });
        let selection = |input: &Entity<TextInput>, cx: &mut gpui::VisualTestContext| {
            input.read_with(cx, |input, _| input.selection())
        };
        for (keys, expected) in [
            ("home", 0..0),
            ("end", 5..5),
            ("cmd-left", 0..0),
            ("cmd-right", 5..5),
            ("alt-left", 3..3),
            ("cmd-shift-left", 0..3),
            // Not bound outside multiline fields: no movement and no new line.
            ("cmd-up", 0..3),
            ("alt-shift-left", 0..3),
            ("tab", 0..3),
        ] {
            cx.simulate_keystrokes(&if cfg!(target_os = "macos") {
                keys.to_owned()
            } else {
                keys.replace("cmd-", "ctrl-")
            });
            assert_eq!(selection(&input, cx), expected, "{keys}");
        }
        assert_eq!(input.read_with(cx, |input, _| input.text().to_owned()), "ab cd");
    }
}
