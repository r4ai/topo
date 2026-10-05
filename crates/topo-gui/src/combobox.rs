//! A text field with a list of values to choose from, which behaves like the
//! completion of a code editor.
//!
//! The owner supplies the choices for what is typed ([`Combobox::set_choices`])
//! whenever the field reports [`ComboEvent::Changed`]. Once something is typed,
//! the first choice is highlighted: Enter and Tab take it, the field completes
//! it faintly after the typed text, and the list shows which part of each
//! choice the typed text matched. Down and Up move the highlight. To submit
//! the typed text as it is, Escape closes the list (a second Escape cancels
//! the field), or Up from the first choice leaves it. An untouched field
//! highlights the value it opened with, which the list also marks.
//!
//! With [`Combobox::open`] given chips, the field holds several values: a
//! space or comma finishes one, a taken choice becomes one, and Backspace in
//! the empty field removes the last.
//!
//! The list floats over what is below the field, so opening it moves nothing.
//! It holds every choice and scrolls, with the highlight kept in view.
//!
//! [`Combobox::palette`] is the same field as a command palette: the list is
//! part of it instead of floating, its first entry is highlighted even before
//! anything is typed, and Escape cancels at once.

use gpui::{
    AnyElement, App, Bounds, Context, Div, ElementId, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    HighlightStyle, MouseButton, Rgba, ScrollStrategy, Stateful, StyledText, UniformListScrollHandle, Window, anchored,
    canvas, deferred, div, fill, point, prelude::*, px, size, uniform_list,
};
use std::collections::HashSet;
use std::ops::Range;
use std::rc::Rc;

use crate::animation::{self, Motion as _};
use crate::text_input::{InputEvent, TextInput};
use crate::theme::{self, metrics};
use crate::ui;

/// A value offered in the list, and what tells it apart there.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Choice {
    pub value: String,
    pub detail: String,
    /// What the choice stands for, when that is not its text: an id.
    pub key: Option<String>,
    /// A glyph before the value, and its color.
    pub icon: Option<(&'static str, Rgba)>,
}

impl Choice {
    pub fn new(value: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { value: value.into(), detail: detail.into(), key: None, icon: None }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ComboEvent {
    /// The text or the chips changed; the owner updates the choices.
    Changed,
    /// Another choice is highlighted.
    Highlighted,
    /// Enter, after a highlighted choice was taken as the text ([`Combobox::picked`]).
    Submit,
    Cancel,
    /// Tab and Shift-Tab, after a highlighted choice was taken.
    Next,
    Previous,
}

/// Entries the list shows at once; it scrolls through the rest.
const ROWS: usize = 8;
/// The width of the bar that shows how far the list is scrolled.
const SCROLLBAR: f32 = 3.;
/// The shortest that bar gets, however long the list.
const THUMB_MIN: f32 = 16.;
/// Characters that finish a chip.
const SEPARATORS: [char; 3] = [',', ' ', '　'];

pub(crate) struct Combobox {
    input: Entity<TextInput>,
    /// A command palette rather than a form field.
    palette: bool,
    /// A glyph before the field of a palette.
    lead: ui::Icon,
    /// The text the field opened with: the current value, which is not a query.
    initial: String,
    /// The finished values of a field of several, or `None` for a field of one.
    chips: Option<Vec<String>>,
    choices: Vec<Choice>,
    keys: Rc<HashSet<String>>,
    widest: Option<usize>,
    scroll: UniformListScrollHandle,
    /// The choice Enter takes.
    highlight: Option<usize>,
    /// The choice that was taken as the text, until the text changes again.
    picked: Option<Choice>,
    /// Escape closed the list; it stays closed until the text changes or Down reopens it.
    dismissed: bool,
    /// What Enter would save for the typed text, and its color.
    preview: (String, Rgba),
    /// Why the last submission was refused.
    error: Option<String>,
}

impl EventEmitter<ComboEvent> for Combobox {}

impl Combobox {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let t = theme::current();
        let input = cx.new(TextInput::new);
        cx.subscribe(&input, Self::on_input_event).detach();
        Self {
            input,
            palette: false,
            lead: ui::Icon::Search,
            initial: String::new(),
            chips: None,
            choices: Vec::new(),
            keys: Rc::default(),
            widest: None,
            scroll: UniformListScrollHandle::new(),
            highlight: None,
            picked: None,
            dismissed: false,
            preview: (String::new(), t.fg_faint),
            error: None,
        }
    }

    pub fn palette(cx: &mut Context<Self>) -> Self {
        Self { palette: true, ..Self::new(cx) }
    }

    /// Starts over with `text` selected, so typing replaces it. `chips` makes it a field of several values.
    pub fn open(&mut self, text: &str, placeholder: &str, chips: Option<Vec<String>>, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.reset(text, placeholder, cx));
        self.initial = text.to_owned();
        self.chips = chips;
        self.choices.clear();
        self.keys = Rc::default();
        self.widest = None;
        (self.highlight, self.picked, self.dismissed, self.error) = (None, None, false, None);
        cx.notify();
    }

    /// Makes a long text wrap into rows instead of scrolling sideways.
    pub fn set_wrap(&mut self, wrap: bool, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.set_wrap(wrap, cx));
    }

    /// Caps a wrapping field at `rows` rows.
    pub fn set_max_rows(&mut self, rows: Option<usize>, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.set_max_rows(rows, cx));
    }

    /// Sets the icon before the field of a palette.
    pub fn set_lead(&mut self, icon: ui::Icon) {
        self.lead = icon;
    }

    /// Replaces the typed text, as if it had been typed.
    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.set_text(text, cx));
    }

    /// The typed text.
    pub fn text<'a>(&self, cx: &'a App) -> &'a str {
        self.input.read(cx).text()
    }

    /// Whether the field still shows the value it opened with.
    fn untouched(&self, cx: &App) -> bool {
        self.text(cx) == self.initial
    }

    /// What narrows the choices: the typed text, or nothing while the field still shows the value it opened with.
    pub fn query<'a>(&self, cx: &'a App) -> &'a str {
        if self.untouched(cx) { "" } else { self.text(cx) }
    }

    pub fn chips(&self) -> &[String] {
        self.chips.as_deref().unwrap_or_default()
    }

    /// Everything the field holds as one text: the chips, then the typed text.
    pub fn value(&self, cx: &App) -> String {
        match &self.chips {
            Some(chips) => format!("{} {}", chips.join(" "), self.text(cx)).trim().to_owned(),
            None => self.text(cx).to_owned(),
        }
    }

    #[cfg(test)]
    pub fn choices(&self) -> &[Choice] {
        &self.choices
    }

    #[cfg(test)]
    pub fn contains_key(&self, key: &str) -> bool {
        self.keys.contains(key)
    }

    /// Share query results with canvas highlighting without rebuilding a set per frame.
    pub fn matching_keys(&self) -> Rc<HashSet<String>> {
        self.keys.clone()
    }

    #[cfg(test)]
    pub fn highlight(&self) -> Option<usize> {
        self.highlight
    }

    /// The choice Enter would take.
    pub fn highlighted(&self) -> Option<&Choice> {
        self.highlight.map(|index| &self.choices[index])
    }

    /// The choice Enter or a click took, if the submitted text came from one.
    pub fn picked(&self) -> Option<&Choice> {
        self.picked.as_ref()
    }

    /// Replaces the list and highlights what Enter should take: the value the
    /// field opened with while it is untouched, otherwise the first choice.
    /// Nothing typed highlights nothing, so Enter on an emptied field clears
    /// the value; a palette always highlights its first choice.
    pub fn set_choices(&mut self, choices: Vec<Choice>, cx: &mut Context<Self>) {
        self.keys = Rc::new(choices.iter().filter_map(|choice| choice.key.clone()).collect());
        self.widest =
            (0..choices.len()).max_by_key(|&i| choices[i].value.chars().count() + choices[i].detail.chars().count());
        let text = self.text(cx);
        self.highlight = match () {
            _ if self.palette => (!choices.is_empty()).then_some(0),
            _ if self.untouched(cx) => choices.iter().position(|c| !text.is_empty() && c.value == text),
            _ if text.trim().is_empty() || choices.is_empty() => None,
            _ => Some(0),
        };
        self.choices = choices;
        self.reveal_highlight();
        self.show_completion(cx);
    }

    /// Sets what the list says when no choice is highlighted, and the refusal of the last submission.
    pub fn set_feedback(&mut self, preview: (String, Rgba), error: Option<String>, cx: &mut Context<Self>) {
        (self.preview, self.error) = (preview, error);
        cx.notify();
    }

    /// Scrolls the highlighted choice into view, or back to the top when there is none.
    fn reveal_highlight(&self) {
        self.scroll.scroll_to_item(self.highlight.unwrap_or(0), ScrollStrategy::Nearest);
    }

    /// Completes the highlighted choice after the typed text, when it continues it.
    fn show_completion(&mut self, cx: &mut Context<Self>) {
        let text = self.text(cx);
        let rest = self.highlighted().and_then(|choice| {
            let continues = !self.untouched(cx) && choice.value.to_lowercase().starts_with(&text.to_lowercase());
            choice.value.get(text.len()..).filter(|_| continues).map(str::to_owned)
        });
        self.input.update(cx, |input, cx| input.set_ghost(rest.as_deref().unwrap_or_default(), cx));
        cx.notify();
    }

    /// Takes the highlighted choice. Returns whether the field still wants input: a chip was added.
    fn take_highlighted(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(choice) = self.highlight.take().map(|i| self.choices[i].clone()) else { return false };
        match &mut self.chips {
            Some(chips) => {
                chips.push(choice.value);
                self.set_text("", cx);
                true
            }
            None => {
                self.set_text(&choice.value, cx);
                self.picked = Some(choice);
                false
            }
        }
    }

    fn pick(&mut self, index: usize, cx: &mut Context<Self>) {
        self.highlight = Some(index);
        if !self.take_highlighted(cx) {
            cx.emit(ComboEvent::Submit);
        }
    }

    fn remove_chip(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(chips) = &mut self.chips {
            chips.remove(index);
            cx.emit(ComboEvent::Changed);
        }
    }

    /// Moves finished values from the text to the chips.
    fn collect_chips(&mut self, cx: &mut Context<Self>) {
        let Some(chips) = &mut self.chips else { return };
        let input = self.input.read(cx);
        // A composition may still turn its space into something else.
        if input.composing() {
            return;
        }
        let Some((finished, typing)) = input.text().rsplit_once(SEPARATORS) else { return };
        let typing = typing.to_owned();
        for value in finished.split(SEPARATORS).map(|v| v.trim_start_matches('#')).filter(|v| !v.is_empty()) {
            if !chips.iter().any(|chip| chip == value) {
                chips.push(value.to_owned());
            }
        }
        self.set_text(&typing, cx);
    }

    fn on_input_event(&mut self, _: Entity<TextInput>, event: &InputEvent, cx: &mut Context<Self>) {
        match event {
            // Taking a choice sets the text; that change keeps what was taken.
            InputEvent::Changed if self.picked.as_ref().is_some_and(|c| c.value == self.text(cx)) => {}
            InputEvent::Changed => {
                (self.picked, self.error, self.dismissed) = (None, None, false);
                self.collect_chips(cx);
                cx.emit(ComboEvent::Changed);
            }
            InputEvent::Up | InputEvent::Down => {
                let last = self.choices.len().checked_sub(1);
                self.dismissed = false;
                self.highlight = match (event, self.highlight, last) {
                    (_, _, None) => None,
                    (InputEvent::Down, None, _) => Some(0),
                    (InputEvent::Down, Some(i), Some(last)) => Some((i + 1).min(last)),
                    (_, Some(0), _) if self.palette => Some(0),
                    (_, Some(0) | None, _) => None,
                    (_, Some(i), _) => Some(i - 1),
                };
                self.reveal_highlight();
                self.show_completion(cx);
                cx.emit(ComboEvent::Highlighted);
            }
            InputEvent::BackspaceEmpty => {
                if self.chips.as_mut().is_some_and(|chips| chips.pop().is_some()) {
                    cx.emit(ComboEvent::Changed);
                }
            }
            // The first Escape closes a list that typing opened, leaving the typed text to submit.
            InputEvent::Cancel if self.highlight.is_some() && !self.untouched(cx) && !self.palette => {
                (self.highlight, self.dismissed) = (None, true);
                self.show_completion(cx);
            }
            InputEvent::Cancel => cx.emit(ComboEvent::Cancel),
            InputEvent::Submit => {
                if !self.take_highlighted(cx) {
                    cx.emit(ComboEvent::Submit);
                }
            }
            InputEvent::Next | InputEvent::Previous => {
                self.take_highlighted(cx);
                cx.emit(if *event == InputEvent::Next { ComboEvent::Next } else { ComboEvent::Previous });
            }
        }
        cx.notify();
    }

    /// A choice with the part the typed text matched set off.
    fn matched(&self, value: &str, cx: &App) -> StyledText {
        let t = theme::current();
        let query = self.query(cx).trim().trim_start_matches('#').to_lowercase();
        let start = value.to_lowercase().find(&query).filter(|_| !query.is_empty());
        let range = start.map(|start| start..start + query.len());
        // Lowercasing can move byte offsets in rare scripts; then nothing is set off.
        let range = range.filter(|r| value.is_char_boundary(r.start) && value.is_char_boundary(r.end));
        let style = HighlightStyle {
            color: Some(t.fg.into()),
            font_weight: Some(FontWeight::BOLD),
            ..HighlightStyle::default()
        };
        StyledText::new(value.to_owned()).with_highlights(range.map(|range| (range, style)))
    }

    /// The height of an entry of the list.
    fn row_height(&self) -> f32 {
        if self.palette { 32. } else { 24. }
    }

    /// The entries `range` of the list.
    fn rows(&self, range: Range<usize>, cx: &mut Context<Self>) -> Vec<Stateful<Div>> {
        let t = theme::current();
        let palette = self.palette;
        range
            .map(|index| {
                let choice = &self.choices[index];
                let highlighted = self.highlight == Some(index);
                let current = !self.initial.is_empty() && choice.value == self.initial;
                let mark = match choice.icon {
                    Some((glyph, color)) => div().w(px(14.)).text_color(color).child(glyph),
                    // The value the field has now.
                    None => {
                        div().w(px(12.)).when(current, |d| d.child(ui::icon(ui::Icon::Check, t.success).size(px(12.))))
                    }
                };
                ui::list_row(format!("choice-{index}"), highlighted)
                    .debug_selector(move || format!("choice-{index}"))
                    .flex()
                    .items_center()
                    .gap_2()
                    .w_full()
                    .h(px(self.row_height()))
                    .when_else(palette, |d| d.px_3().rounded(px(metrics::R_MD)), |d| d.px_1p5())
                    .text_color(t.fg)
                    .whitespace_nowrap()
                    .child(mark.flex_shrink_0())
                    .child(
                        div()
                            .when_else(palette, |d| d.flex_1().min_w(px(0.)).overflow_hidden(), |d| d.flex_shrink_0())
                            .child(self.matched(&choice.value, cx)),
                    )
                    .when(!palette, |d| d.child(div().flex_1()))
                    .child(div().flex_shrink_0().text_xs().text_color(t.fg_muted).child(choice.detail.clone()))
                    .on_click(cx.listener(move |combo, _, _, cx| combo.pick(index, cx)))
            })
            .collect()
    }

    /// The list: the choices, which scroll, then the preview or the error.
    fn list(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = theme::current();
        let note = match (&self.error, self.highlight) {
            (Some(error), _) => Some((error.clone(), t.danger)),
            (None, None) if !self.preview.0.is_empty() => Some(self.preview.clone()),
            _ => None,
        };
        let choices = if self.dismissed { &[][..] } else { &self.choices[..] };
        if choices.is_empty() && note.is_none() {
            return None;
        }
        let palette = self.palette;
        // The floating list is as wide as its widest entry, which is the only one measured.
        let widest = self.widest;
        let rows = (!choices.is_empty()).then(|| {
            let count = choices.len();
            let rows = uniform_list("combo-rows", count, cx.processor(|this, range, _, cx| this.rows(range, cx)))
                .track_scroll(&self.scroll)
                .with_width_from_item(widest)
                .h(px(self.row_height() * count.min(ROWS) as f32));
            // Painted after the list is laid out, so it shows where a move of the highlight scrolled to.
            let scroll = self.scroll.clone();
            let scrollbar = canvas(
                |_, _, _| (),
                move |track, (), window, _| {
                    let scrolled = -scroll.0.borrow().base_handle.offset().y / (track.size.height / ROWS as f32);
                    let (top, height) = scroll_thumb(count, scrolled, track.size.height.into());
                    let thumb = Bounds::new(track.origin + point(px(0.), px(top)), size(track.size.width, px(height)));
                    window.paint_quad(fill(thumb, t.fg_faint).corner_radii(track.size.width / 2.));
                },
            )
            .absolute()
            .top_0()
            .right(px(2.))
            .w(px(SCROLLBAR))
            .h_full();
            div().relative().child(rows).when(count > ROWS, |d| d.child(scrollbar))
        });
        let faint = |text: String, color: Rgba| {
            div()
                .when_else(palette, |d| d.px_3().py_1p5(), |d| d.px_1p5().py_1())
                .whitespace_nowrap()
                .text_xs()
                .text_color(color)
                .child(text)
        };
        let separate = rows.is_some() && !palette;
        let note = note.map(|(text, color)| {
            faint(text, color)
                .debug_selector(|| "combo-note".to_owned())
                .when(separate, |d| d.mt_0p5().border_t_1().border_color(t.hairline))
        });
        let list = div().flex().flex_col().children(rows).children(note);
        Some(match palette {
            true => list.w_full().p_1().text_sm().into_any_element(),
            false => ui::glass(ui::Elevation::Popover)
                .id("combo-list")
                .debug_selector(|| "combo-list".to_owned())
                .occlude()
                .min_w(px(200.))
                .p_0p5()
                .text_xs()
                .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| cx.stop_propagation())
                .child(list)
                .rise_in("combo-list-motion", animation::FAST),
        })
    }
}

/// Where the thumb of the scrollbar starts and how tall it is, in a track of `track` pixels
/// beside a list of `count` entries scrolled down by `scrolled` of them.
fn scroll_thumb(count: usize, scrolled: f32, track: f32) -> (f32, f32) {
    let height = (track * ROWS as f32 / count as f32).max(THUMB_MIN);
    let hidden = count.saturating_sub(ROWS).max(1) as f32;
    ((track - height) * (scrolled / hidden).clamp(0., 1.), height)
}

impl Focusable for Combobox {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for Combobox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current();
        let focused = self.focus_handle(cx).is_focused(window);
        let list = focused.then(|| self.list(cx)).flatten();
        let field = div().flex_1().min_w(px(64.)).child(self.input.clone());
        if self.palette {
            // The owner draws the card around it; the list follows the field in the flow.
            let lead = ui::icon(self.lead, t.accent).size(px(metrics::ICON_LG));
            return div()
                .id("combobox")
                .w_full()
                .flex()
                .flex_col()
                .child(div().flex().items_center().gap_3().px_4().py_2p5().child(lead).child(field))
                .children(list.map(|list| div().border_t_1().border_color(t.hairline).child(list)));
        }
        let chips: Vec<_> = self
            .chips()
            .iter()
            .enumerate()
            .map(|(index, value)| {
                ui::chip(format!("#{value}")).flex().items_center().gap_1().child(
                    div()
                        .id(ElementId::Name(format!("chip-{index}-remove").into()))
                        .debug_selector(move || format!("chip-{index}-remove"))
                        .group("chip-remove")
                        .cursor_pointer()
                        .child(
                            ui::icon(ui::Icon::Close, t.fg_muted)
                                .size(px(10.))
                                .group_hover("chip-remove", |s| s.text_color(t.fg)),
                        )
                        .on_click(cx.listener(move |combo, _, _, cx| combo.remove_chip(index, cx))),
                )
            })
            .collect();
        ui::input_frame(focused, self.error.is_some())
            .id("combobox")
            .debug_selector(|| "combobox".to_owned())
            .relative()
            .w_full()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .px_1p5()
            .py_0p5()
            // The field keeps its own clicks, so its owner can close it on a click elsewhere.
            .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| cx.stop_propagation())
            .children(chips)
            .child(field)
            .children(list.map(|list| {
                div()
                    .absolute()
                    .left(px(-1.))
                    .top_full()
                    .mt_1()
                    .child(deferred(anchored().snap_to_window_with_margin(px(8.)).child(list)).with_priority(1))
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scroll_thumb_spans_the_track_as_the_list_scrolls() {
        // Twice the rows: the thumb is half the track, and ends where the track does.
        assert_eq!(scroll_thumb(2 * ROWS, 0., 192.), (0., 96.));
        assert_eq!(scroll_thumb(2 * ROWS, ROWS as f32, 192.), (96., 96.));
        // A long list keeps a thumb that can be seen, and overscroll keeps it on the track.
        assert_eq!(scroll_thumb(1000, 2000., 192.), (192. - THUMB_MIN, THUMB_MIN));
        assert_eq!(scroll_thumb(1000, -5., 192.), (0., THUMB_MIN));
    }
}
