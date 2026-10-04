//! Colors, glyphs and small styled building blocks shared by every view.

use std::cell::Cell;

use gpui::{BoxShadow, Div, Hsla, Rgba, SharedString, Stateful, Styled, div, prelude::*, px};
use topo_core::{Kind, Node, Priority, Status};

use crate::markdown::Style;
use crate::ui;

pub mod metrics;
mod mode;
mod palette;
#[cfg(test)]
pub(crate) use palette::{DARK, LIGHT};

pub use mode::{ThemeDark, ThemeLight, ThemeMode, ThemeSystem, apply, init, mode, set_mode};

/// Every color the UI draws with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub bg: Rgba,
    pub glass_alpha: f32,
    pub chrome: Rgba,
    pub card: Rgba,
    pub card_hover: Rgba,
    pub card_milestone: Rgba,
    pub card_milestone_hover: Rgba,
    pub overlay: Rgba,
    pub control: Rgba,
    pub control_hover: Rgba,
    pub control_active: Rgba,
    pub field: Rgba,
    pub hairline: Rgba,
    pub border_strong: Rgba,
    pub highlight: Rgba,
    pub fg: Rgba,
    pub fg_muted: Rgba,
    pub fg_faint: Rgba,
    pub emphasis: Rgba,
    pub on_emphasis: Rgba,
    pub selection: Rgba,
    pub danger: Rgba,
    pub warn: Rgba,
    pub success: Rgba,
    pub edge: Rgba,
    pub edge_closed: Rgba,
    pub edge_dim: Rgba,
    pub grid_dot: Rgba,
    pub scrim: Rgba,
    pub shadow: Rgba,
    // Legacy names, removed once every view is migrated.
    pub surface: Rgba,
    pub raised: Rgba,
    pub border: Rgba,
    pub accent: Rgba,
    pub link: Rgba,
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
    current().emphasis
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
        Style::Heading => Some(t.emphasis),
        Style::Code => Some(t.fg_muted),
        Style::Link => Some(t.fg),
        Style::Marker => Some(t.fg_faint),
        Style::Quote => Some(t.fg_muted),
    }
}

pub fn status_color(status: Status) -> Rgba {
    let t = current();
    match status {
        Status::Todo => t.fg_muted,
        Status::Doing => t.emphasis,
        Status::Done => t.fg_faint,
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
        Priority::High => t.fg,
        Priority::Medium => t.fg_muted,
        Priority::Low => t.fg_faint,
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
        Kind::Milestone if node.status.is_closed() => ("◆", t.fg_faint),
        Kind::Milestone => ("◆", t.fg),
        Kind::Task => (status_icon(node.status), status_color(node.status)),
    }
}

/// The shadow of a selected card; an alias of `metrics::e1`.
pub fn shadow() -> Vec<BoxShadow> {
    metrics::e1()
}

/// A small text button.
pub fn button(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    ui::button(id, label)
}

/// A small text button tinted with `color`, e.g. for destructive actions.
pub fn tinted_button(id: impl Into<SharedString>, label: impl Into<SharedString>, color: Rgba) -> Stateful<Div> {
    match color == current().danger {
        true => ui::button_danger(id, label),
        false => ui::button(id, label).text_color(color),
    }
}

/// A borderless square button showing a single glyph.
pub fn icon_button(id: impl Into<SharedString>, glyph: impl Into<SharedString>) -> Stateful<Div> {
    ui::icon_button(id, glyph)
}

/// A keyboard shortcut hint such as `⌘Z`.
pub fn kbd(keys: impl Into<SharedString>) -> Div {
    ui::kbd(keys)
}

/// Small uppercase heading above a group of rows.
pub fn section_label(label: impl Into<SharedString>) -> Div {
    ui::section_label(label)
}

/// A rounded label; alarm colors tint it, the emphasis color fills it, any other color only sets the text.
pub fn chip(label: impl Into<SharedString>, color: Rgba) -> Div {
    let t = current();
    match color {
        c if c == t.danger || c == t.warn || c == t.success => ui::chip_tinted(label, c),
        c if c == t.emphasis => ui::chip_emphasis(label),
        c => ui::chip(label).text_color(c),
    }
}

/// Horizontal progress bar filled to `fraction` (0..=1).
pub fn progress_bar(fraction: f32, color: Rgba, height: f32) -> Div {
    div()
        .flex_1()
        .h(px(height))
        .rounded_full()
        .bg(current().control)
        .overflow_hidden()
        .child(div().h_full().w(gpui::relative(fraction.clamp(0., 1.))).rounded_full().bg(color))
}
