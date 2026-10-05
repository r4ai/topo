//! Shared components of the design system; views compose these instead of styling a `div` by hand.

use gpui::{Div, FontWeight, Rgba, SharedString, Stateful, Styled, Svg, div, prelude::*, px, relative};

pub use crate::icons::{Icon, icon};
use crate::theme::{
    alpha, current,
    metrics::{self, H_BUTTON, H_ICON, H_INPUT, R_LG, R_MD, R_SM, R_XL, R_XS, S1, S3, S4, S5, S6, T_BODY, T_CAPTION},
};

/// How high a glass surface floats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevation {
    Popover,
    Dialog,
}

/// The common frame of a text button; the variants set colors. An empty label leaves the lead alone.
fn base_button(id: impl Into<SharedString>, lead: Option<Svg>, label: impl Into<SharedString>) -> Stateful<Div> {
    let id = id.into();
    div()
        .id(id.clone())
        .debug_selector(|| id.to_string())
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap_1p5()
        .whitespace_nowrap()
        .h(px(H_BUTTON))
        .px_2p5()
        .rounded(px(R_MD))
        .border_1()
        .text_size(px(T_BODY))
        .cursor_pointer()
        .children(lead)
        .children(Some(label.into()).filter(|label: &SharedString| !label.is_empty()))
}

/// A small text button.
pub fn button(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    plain_button(id, None, label)
}

/// A [`button`] led by an icon.
pub fn button_icon(id: impl Into<SharedString>, lead: Icon, label: impl Into<SharedString>) -> Stateful<Div> {
    plain_button(id, Some(icon(lead, current().fg_muted)), label)
}

fn plain_button(id: impl Into<SharedString>, lead: Option<Svg>, label: impl Into<SharedString>) -> Stateful<Div> {
    let t = current();
    let (hover, hover_border, active) = (t.control_hover, t.border_strong, t.control_active);
    base_button(id, lead, label)
        .bg(t.control)
        .border_color(t.hairline)
        .text_color(t.fg)
        .hover(move |s| s.bg(hover).border_color(hover_border))
        .active(move |s| s.bg(active))
}

/// The primary action: filled with the emphasis color.
pub fn button_primary(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    primary_button(id, None, label)
}

/// A [`button_primary`] led by an icon.
pub fn button_primary_icon(id: impl Into<SharedString>, lead: Icon, label: impl Into<SharedString>) -> Stateful<Div> {
    primary_button(id, Some(icon(lead, current().on_emphasis)), label)
}

fn primary_button(id: impl Into<SharedString>, lead: Option<Svg>, label: impl Into<SharedString>) -> Stateful<Div> {
    let t = current();
    base_button(id, lead, label)
        .bg(t.emphasis)
        .border_color(t.emphasis)
        .text_color(t.on_emphasis)
        .font_weight(FontWeight::SEMIBOLD)
        .hover(|s| s.opacity(0.9))
}

/// A destructive action, tinted with the danger color.
pub fn button_danger(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    let t = current();
    let danger = t.danger;
    base_button(id, None, label)
        .bg(t.control)
        .border_color(t.hairline)
        .text_color(danger)
        .hover(move |s| s.bg(alpha(danger, 0.14)).border_color(alpha(danger, 0.5)))
}

/// A button with no fill until it is hovered.
pub fn button_ghost(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    let t = current();
    let (hover, fg) = (t.control_hover, t.fg);
    base_button(id, None, label)
        .border_color(gpui::transparent_black())
        .text_color(t.fg_muted)
        .hover(move |s| s.bg(hover).text_color(fg))
}

/// A [`button_ghost`] led by an icon.
pub fn button_ghost_icon(id: impl Into<SharedString>, lead: Icon, label: impl Into<SharedString>) -> Stateful<Div> {
    let t = current();
    let (hover, fg) = (t.control_hover, t.fg);
    let lead = icon(lead, t.fg_muted).group_hover("button", move |s| s.text_color(fg));
    base_button(id, Some(lead), label)
        .group("button")
        .border_color(gpui::transparent_black())
        .text_color(t.fg_muted)
        .hover(move |s| s.bg(hover).text_color(fg))
}

/// A borderless square button showing a single glyph.
pub fn icon_button(id: impl Into<SharedString>, glyph: impl Into<SharedString>) -> Stateful<Div> {
    icon_frame(id).child(glyph.into())
}

/// An [`icon_button`] showing one of the app's icons.
pub fn icon_button_svg(id: impl Into<SharedString>, shown: Icon) -> Stateful<Div> {
    let t = current();
    let fg = t.fg;
    icon_frame(id)
        .group("icon-button")
        .child(icon(shown, t.fg_muted).group_hover("icon-button", move |s| s.text_color(fg)))
}

fn icon_frame(id: impl Into<SharedString>) -> Stateful<Div> {
    let id = id.into();
    let t = current();
    let (hover, fg) = (t.control_hover, t.fg);
    div()
        .id(id.clone())
        .debug_selector(|| id.to_string())
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(H_ICON))
        .rounded(px(R_SM))
        .text_color(t.fg_muted)
        .text_sm()
        .hover(move |s| s.bg(hover).text_color(fg))
        .cursor_pointer()
}

/// A small on/off switch. `on` fills the track with the accent and moves the knob to the end.
pub fn switch(on: bool) -> Div {
    let t = current();
    div()
        .flex_shrink_0()
        .flex()
        .items_center()
        .w(px(30.))
        .h(px(17.))
        .px(px(2.))
        .rounded_full()
        .border_1()
        .border_color(if on { t.accent } else { t.border_strong })
        .bg(if on { t.accent } else { t.control })
        .map(|d| if on { d.justify_end() } else { d.justify_start() })
        .child(div().size(px(11.)).rounded_full().bg(if on { t.on_accent } else { t.fg_muted }))
}

/// The track that holds a row of segments.
pub fn segmented() -> Div {
    let t = current();
    div()
        .flex()
        .items_center()
        .gap(px(S1))
        .p(px(S1))
        .rounded(px(R_MD))
        .bg(t.control)
        .border_1()
        .border_color(t.hairline)
}

/// One choice inside `segmented`.
pub fn segment(id: impl Into<SharedString>, label: impl IntoElement, selected: bool) -> Stateful<Div> {
    let id = id.into();
    let t = current();
    let hover = t.control_hover;
    div()
        .id(id.clone())
        .debug_selector(|| id.to_string())
        .flex()
        .items_center()
        .justify_center()
        .px_2p5()
        .py(px(S1))
        .rounded(px(R_SM))
        .text_size(px(T_BODY))
        .cursor_pointer()
        .map(|d| match selected {
            true => d.bg(t.control_active).text_color(t.fg).font_weight(FontWeight::MEDIUM),
            false => d.text_color(t.fg_muted).hover(move |s| s.bg(hover)),
        })
        .child(label)
}

/// The well of a text input; the border shows focus or an error.
pub fn input_frame(focused: bool, error: bool) -> Div {
    let t = current();
    let border = match (error, focused) {
        (true, _) => t.danger,
        (false, true) => t.accent,
        (false, false) => t.hairline,
    };
    div().min_h(px(H_INPUT)).rounded(px(R_MD)).bg(t.field).border_1().border_color(border)
}

/// A selectable row in a list.
pub fn list_row(id: impl Into<SharedString>, selected: bool) -> Stateful<Div> {
    let t = current();
    let hover = t.control_hover;
    div().id(id.into()).rounded(px(R_SM)).px(px(S4)).cursor_pointer().map(|d| match selected {
        true => d.bg(t.control_active),
        false => d.hover(move |s| s.bg(hover)),
    })
}

/// A side or top bar of chrome.
pub fn panel() -> Div {
    div().bg(current().chrome)
}

/// Floating glass at `level`.
pub fn glass(level: Elevation) -> Div {
    let t = current();
    let shadow = match level {
        Elevation::Popover => metrics::e2(),
        Elevation::Dialog => metrics::e3(),
    };
    div().bg(t.overlay).border_1().border_color(t.hairline).rounded(px(R_LG)).shadow(shadow)
}

/// The backdrop of a modal: covers its parent and centers its child.
pub fn scrim() -> Div {
    div().absolute().size_full().occlude().flex().items_center().justify_center().bg(current().scrim)
}

/// The glass card of a modal.
pub fn dialog() -> Div {
    glass(Elevation::Dialog).p(px(S6)).rounded(px(R_XL))
}

/// A neutral filled label.
pub fn chip(label: impl Into<SharedString>) -> Div {
    let t = current();
    chip_frame(label).bg(t.control).text_color(t.fg_muted)
}

/// A label with a hairline outline and no fill.
pub fn chip_outline(label: impl Into<SharedString>) -> Div {
    let t = current();
    chip_frame(label).border_1().border_color(t.hairline).text_color(t.fg_muted)
}

/// A label tinted with a signal or alarm `color`.
pub fn chip_tinted(label: impl Into<SharedString>, color: Rgba) -> Div {
    chip_frame(label).bg(alpha(color, 0.14)).text_color(color)
}

fn chip_frame(label: impl Into<SharedString>) -> Div {
    div().flex_shrink_0().px(px(S3)).rounded(px(R_XS)).text_size(px(T_CAPTION)).child(label.into())
}

/// A keyboard shortcut hint such as `⌘Z`.
pub fn kbd(keys: impl Into<SharedString>) -> Div {
    let t = current();
    div()
        .flex_shrink_0()
        .px_1()
        .min_w(px(16.))
        .flex()
        .justify_center()
        .rounded(px(R_XS))
        .border_1()
        .border_color(t.hairline)
        .text_color(t.fg_faint)
        .text_size(px(T_CAPTION))
        .child(keys.into())
}

/// Small uppercase heading above a group of rows.
pub fn section_label(label: impl Into<SharedString>) -> Div {
    div()
        .pt_3()
        .pb_1()
        .text_size(px(T_CAPTION))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(current().fg_faint)
        .child(label.into())
}

/// A 1px rule.
pub fn divider() -> Div {
    div().h(px(1.)).w_full().flex_shrink_0().bg(current().hairline)
}

/// Horizontal progress bar filled to `fraction` (0..=1); `complete` takes the completed color.
pub fn progress_bar(fraction: f32, complete: bool, height: f32) -> Div {
    let t = current();
    let fill = if complete { t.progress_complete } else { t.progress };
    div()
        .flex_1()
        .h(px(height))
        .rounded_full()
        .bg(t.control)
        .overflow_hidden()
        .child(div().h_full().w(relative(fraction.clamp(0., 1.))).rounded_full().bg(fill))
}

/// The glass pill of a toast.
pub fn toast_frame() -> Div {
    glass(Elevation::Popover).px(px(S5)).py(px(S4))
}
