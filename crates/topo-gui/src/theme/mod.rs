//! Colors, glyphs and small styled building blocks shared by every view.

use std::cell::Cell;

use gpui::{BoxShadow, Div, Hsla, Rgba, SharedString, Stateful, Styled, div, point, prelude::*, px};
use topo_core::{Kind, Node, Priority, Status};

use crate::markdown::Style;

mod mode;
mod palette;
#[cfg(test)]
pub(crate) use palette::{DARK, LIGHT};

pub use mode::{ThemeDark, ThemeLight, ThemeMode, ThemeSystem, apply, init, mode, set_mode};

/// Every color the UI draws with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub bg: Rgba,
    pub surface: Rgba,
    pub card: Rgba,
    pub card_hover: Rgba,
    pub card_milestone: Rgba,
    pub card_milestone_hover: Rgba,
    pub raised: Rgba,
    pub border: Rgba,
    pub border_strong: Rgba,
    pub fg: Rgba,
    pub fg_muted: Rgba,
    pub fg_faint: Rgba,
    pub accent: Rgba,
    pub success: Rgba,
    pub warn: Rgba,
    pub danger: Rgba,
    pub grid_dot: Rgba,
    pub link: Rgba,
    pub edge: Rgba,
    pub scrim: Rgba,
    pub selection: Rgba,
    pub shadow: Rgba,
}

thread_local! {
    static CURRENT: Cell<&'static Theme> = const { Cell::new(&palette::DARK) };
}

pub fn current() -> &'static Theme {
    CURRENT.with(Cell::get)
}

pub fn faint() -> Rgba {
    current().fg_faint
}

pub fn accent() -> Rgba {
    current().accent
}

pub fn selection() -> Rgba {
    current().selection
}

/// `color` with its alpha replaced by `alpha`.
pub fn alpha(color: Rgba, alpha: f32) -> Hsla {
    let mut c: Hsla = color.into();
    c.a = alpha;
    c
}

/// The color of Markdown text in `style`; plain text keeps the color it inherits.
pub fn markdown_color(style: Style) -> Option<Rgba> {
    let t = current();
    match style {
        Style::Plain | Style::Emphasis | Style::Strong => None,
        Style::Heading => Some(t.accent),
        Style::Code => Some(t.success),
        Style::Link => Some(t.link),
        Style::Marker => Some(t.warn),
        Style::Quote => Some(t.fg_muted),
    }
}

pub fn status_color(status: Status) -> Rgba {
    let t = current();
    match status {
        Status::Todo => t.fg_muted,
        Status::Doing => t.accent,
        Status::Done => t.success,
        Status::Dropped => t.fg_faint,
    }
}

pub fn status_icon(status: Status) -> &'static str {
    match status {
        Status::Todo => "○",
        Status::Doing => "◐",
        Status::Done => "✓",
        Status::Dropped => "⊘",
    }
}

pub fn status_label(status: Status) -> &'static str {
    match status {
        Status::Todo => "Todo",
        Status::Doing => "Doing",
        Status::Done => "Done",
        Status::Dropped => "Dropped",
    }
}

pub fn priority_color(priority: Priority) -> Rgba {
    let t = current();
    match priority {
        Priority::Urgent => t.danger,
        Priority::High => t.warn,
        Priority::Medium => t.accent,
        Priority::Low => t.fg_muted,
    }
}

pub fn priority_label(priority: Priority) -> &'static str {
    match priority {
        Priority::Urgent => "Urgent",
        Priority::High => "High",
        Priority::Medium => "Medium",
        Priority::Low => "Low",
    }
}

/// The priority as a glyph and its name, so it reads without its color.
pub fn priority_text(priority: Priority) -> String {
    let glyph = match priority {
        Priority::Urgent => "!!",
        Priority::High => "↑",
        Priority::Medium => "=",
        Priority::Low => "↓",
    };
    format!("{glyph} {}", priority_label(priority))
}

/// Glyph and color that identify a node at a glance.
pub fn node_icon(node: &Node) -> (&'static str, Rgba) {
    let t = current();
    match node.kind {
        Kind::Milestone if node.status.is_closed() => ("◆", t.success),
        Kind::Milestone => ("◆", t.warn),
        Kind::Task => (status_icon(node.status), status_color(node.status)),
    }
}

pub fn shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: current().shadow.into(),
        offset: point(px(0.), px(8.)),
        blur_radius: px(24.),
        spread_radius: px(0.),
        inset: false,
    }]
}

/// A small text button.
pub fn button(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    let t = current();
    styled_button(id, label, t.fg.into(), t.raised.into(), t.border_strong.into())
}

/// A small text button tinted with `color`, e.g. for destructive actions.
pub fn tinted_button(id: impl Into<SharedString>, label: impl Into<SharedString>, color: Rgba) -> Stateful<Div> {
    styled_button(id, label, color.into(), alpha(color, 0.15), alpha(color, 0.5))
}

fn styled_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    text: Hsla,
    hover_bg: Hsla,
    hover_border: Hsla,
) -> Stateful<Div> {
    let id = id.into();
    let t = current();
    div()
        .id(id.clone())
        .debug_selector(|| id.to_string())
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap_1p5()
        .whitespace_nowrap()
        .h(px(26.))
        .px_2p5()
        .rounded_md()
        .border_1()
        .border_color(t.border)
        .bg(t.card)
        .text_color(text)
        .text_xs()
        .hover(move |s| s.bg(hover_bg).border_color(hover_border))
        .active(move |s| s.bg(t.border_strong))
        .cursor_pointer()
        .child(label.into())
}

/// A borderless square button showing a single glyph.
pub fn icon_button(id: impl Into<SharedString>, glyph: impl Into<SharedString>) -> Stateful<Div> {
    let id = id.into();
    let t = current();
    div()
        .id(id.clone())
        .debug_selector(|| id.to_string())
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(22.))
        .rounded_md()
        .text_color(t.fg_muted)
        .text_sm()
        .hover(move |s| s.bg(t.raised).text_color(t.fg))
        .cursor_pointer()
        .child(glyph.into())
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
        .rounded_sm()
        .border_1()
        .border_color(t.border_strong)
        .bg(t.surface)
        .text_color(t.fg_muted)
        .text_size(px(10.))
        .child(keys.into())
}

/// Small uppercase heading above a group of rows.
pub fn section_label(label: impl Into<SharedString>) -> Div {
    div()
        .pt_3()
        .pb_1()
        .text_size(px(10.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(current().fg_faint)
        .child(label.into())
}

/// A rounded tinted label.
pub fn chip(label: impl Into<SharedString>, color: Rgba) -> Div {
    div()
        .flex_shrink_0()
        .px_1p5()
        .rounded_sm()
        .bg(alpha(color, 0.14))
        .text_color(color)
        .text_size(px(10.))
        .child(label.into())
}

/// Horizontal progress bar filled to `fraction` (0..=1).
pub fn progress_bar(fraction: f32, color: Rgba, height: f32) -> Div {
    let t = current();
    div()
        .flex_1()
        .h(px(height))
        .rounded_full()
        .bg(t.border)
        .overflow_hidden()
        .child(div().h_full().w(gpui::relative(fraction.clamp(0., 1.))).rounded_full().bg(color))
}
