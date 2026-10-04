//! Colors, glyphs and small styled building blocks shared by every view.

use std::cell::Cell;

use gpui::{Hsla, Rgba};
use topo_core::{Kind, Node, Priority, Status};

use crate::markdown::Style;

pub mod metrics;
mod mode;
mod palette;
#[cfg(test)]
pub(crate) use palette::{DARK, DARK_MONO, LIGHT, LIGHT_MONO};

pub use mode::{
    ThemeDark, ThemeLight, ThemeMode, ThemeSystem, ToggleMonotone, apply, init, mode, monotone, set_mode, set_monotone,
};

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
    pub accent: Rgba,
    pub on_accent: Rgba,
    pub critical: Rgba,
    pub milestone: Rgba,
    pub status_done: Rgba,
    pub ready: Rgba,
    pub priority_high: Rgba,
    pub md_heading: Rgba,
    pub md_link: Rgba,
    pub progress: Rgba,
    pub progress_complete: Rgba,
    pub danger: Rgba,
    pub warn: Rgba,
    pub success: Rgba,
    pub edge: Rgba,
    pub edge_closed: Rgba,
    pub edge_dim: Rgba,
    pub grid_dot: Rgba,
    pub scrim: Rgba,
    pub shadow: Rgba,
}

thread_local! {
    static CURRENT: Cell<&'static Theme> = const { Cell::new(&palette::DARK) };
    static TRANSLUCENT: Cell<bool> = const { Cell::new(false) };
}

pub fn current() -> &'static Theme {
    CURRENT.with(Cell::get)
}

/// Declares that the OS blurs the window, so the root may paint `bg` translucent.
pub fn set_translucent(translucent: bool) {
    TRANSLUCENT.with(|cell| cell.set(translucent));
}

/// The fill of the window root: `bg` at `glass_alpha` over an OS blur, opaque otherwise.
pub fn window_bg() -> Hsla {
    let t = current();
    alpha(t.bg, if TRANSLUCENT.with(Cell::get) { t.glass_alpha } else { 1. })
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
        Style::Heading => Some(t.md_heading),
        Style::Code => Some(t.fg_muted),
        Style::Link => Some(t.md_link),
        Style::Marker => Some(t.fg_faint),
        Style::Quote => Some(t.fg_muted),
    }
}

pub fn status_color(status: Status) -> Rgba {
    let t = current();
    match status {
        Status::Todo => t.fg_muted,
        Status::Doing => t.accent,
        Status::Done => t.status_done,
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
        Priority::High => t.priority_high,
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
        Kind::Milestone if node.status.is_closed() => ("◆", t.status_done),
        Kind::Milestone => ("◆", t.milestone),
        Kind::Task => (status_icon(node.status), status_color(node.status)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_bg_is_translucent_only_over_a_blur() {
        set_translucent(false);
        assert_eq!(window_bg().a, 1.);
        set_translucent(true);
        assert_eq!(window_bg().a, current().glass_alpha);
        set_translucent(false);
    }
}
