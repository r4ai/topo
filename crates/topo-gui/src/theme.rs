//! Colors, glyphs and small styled building blocks shared by every view.

use gpui::{BoxShadow, Div, Hsla, Rgba, SharedString, Stateful, Styled, div, hsla, point, prelude::*, px, rgb, rgba};
use topo_core::{Kind, Node, Priority, Status};

use crate::markdown::Style;

pub const CANVAS: u32 = 0x111216;
pub const SURFACE: u32 = 0x17181d;
pub const CARD: u32 = 0x1e1f26;
pub const CARD_HOVER: u32 = 0x25262f;
pub const RAISED: u32 = 0x2a2c36;
pub const BORDER: u32 = 0x2b2d37;
pub const BORDER_STRONG: u32 = 0x3b3e4a;
pub const TEXT: u32 = 0xe8e9ee;
pub const MUTED: u32 = 0x9b9dab;
pub const FAINT: u32 = 0x626574;
pub const ACCENT: u32 = 0x6ea8fe;
pub const GREEN: u32 = 0x4cc38a;
pub const AMBER: u32 = 0xf5b949;
pub const RED: u32 = 0xf2555a;
pub const GRID_DOT: u32 = 0x24262e;
const LINK: u32 = 0x5ad1e6;

pub fn faint() -> Rgba {
    rgb(FAINT)
}

pub fn accent() -> Rgba {
    rgb(ACCENT)
}

pub fn selection() -> Rgba {
    rgba(0x6ea8fe44)
}

/// `color` with its alpha replaced by `alpha`.
pub fn alpha(color: u32, alpha: f32) -> Hsla {
    let mut c: Hsla = rgb(color).into();
    c.a = alpha;
    c
}

/// The color of Markdown text in `style`; plain text keeps the color it inherits.
pub fn markdown_color(style: Style) -> Option<Rgba> {
    match style {
        Style::Plain | Style::Emphasis | Style::Strong => None,
        Style::Heading => Some(rgb(ACCENT)),
        Style::Code => Some(rgb(GREEN)),
        Style::Link => Some(rgb(LINK)),
        Style::Marker => Some(rgb(AMBER)),
        Style::Quote => Some(rgb(MUTED)),
    }
}

pub fn status_color(status: Status) -> u32 {
    match status {
        Status::Todo => MUTED,
        Status::Doing => ACCENT,
        Status::Done => GREEN,
        Status::Dropped => FAINT,
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

pub fn priority_color(priority: Priority) -> u32 {
    match priority {
        Priority::Urgent => RED,
        Priority::High => AMBER,
        Priority::Medium => ACCENT,
        Priority::Low => MUTED,
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
pub fn node_icon(node: &Node) -> (&'static str, u32) {
    match node.kind {
        Kind::Milestone if node.status.is_closed() => ("◆", GREEN),
        Kind::Milestone => ("◆", AMBER),
        Kind::Task => (status_icon(node.status), status_color(node.status)),
    }
}

pub fn shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: hsla(0., 0., 0., 0.45),
        offset: point(px(0.), px(8.)),
        blur_radius: px(24.),
        spread_radius: px(0.),
        inset: false,
    }]
}

/// A small text button.
pub fn button(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    styled_button(id, label, rgb(TEXT).into(), rgb(RAISED).into(), rgb(BORDER_STRONG).into())
}

/// A small text button tinted with `color`, e.g. for destructive actions.
pub fn tinted_button(id: impl Into<SharedString>, label: impl Into<SharedString>, color: u32) -> Stateful<Div> {
    styled_button(id, label, rgb(color).into(), alpha(color, 0.15), alpha(color, 0.5))
}

fn styled_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    text: Hsla,
    hover_bg: Hsla,
    hover_border: Hsla,
) -> Stateful<Div> {
    let id = id.into();
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
        .border_color(rgb(BORDER))
        .bg(rgb(CARD))
        .text_color(text)
        .text_xs()
        .hover(move |s| s.bg(hover_bg).border_color(hover_border))
        .active(|s| s.bg(rgb(BORDER_STRONG)))
        .cursor_pointer()
        .child(label.into())
}

/// A borderless square button showing a single glyph.
pub fn icon_button(id: impl Into<SharedString>, glyph: impl Into<SharedString>) -> Stateful<Div> {
    let id = id.into();
    div()
        .id(id.clone())
        .debug_selector(|| id.to_string())
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(22.))
        .rounded_md()
        .text_color(rgb(MUTED))
        .text_sm()
        .hover(|s| s.bg(rgb(RAISED)).text_color(rgb(TEXT)))
        .cursor_pointer()
        .child(glyph.into())
}

/// A keyboard shortcut hint such as `⌘Z`.
pub fn kbd(keys: impl Into<SharedString>) -> Div {
    div()
        .flex_shrink_0()
        .px_1()
        .min_w(px(16.))
        .flex()
        .justify_center()
        .rounded_sm()
        .border_1()
        .border_color(rgb(BORDER_STRONG))
        .bg(rgb(SURFACE))
        .text_color(rgb(MUTED))
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
        .text_color(rgb(FAINT))
        .child(label.into())
}

/// A rounded tinted label.
pub fn chip(label: impl Into<SharedString>, color: u32) -> Div {
    div()
        .flex_shrink_0()
        .px_1p5()
        .rounded_sm()
        .bg(alpha(color, 0.14))
        .text_color(rgb(color))
        .text_size(px(10.))
        .child(label.into())
}

/// Horizontal progress bar filled to `fraction` (0..=1).
pub fn progress_bar(fraction: f32, color: u32, height: f32) -> Div {
    div()
        .flex_1()
        .h(px(height))
        .rounded_full()
        .bg(rgb(BORDER))
        .overflow_hidden()
        .child(div().h_full().w(gpui::relative(fraction.clamp(0., 1.))).rounded_full().bg(rgb(color)))
}
