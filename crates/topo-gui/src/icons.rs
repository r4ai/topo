//! The app's own icons: SVGs compiled into the binary and drawn in a theme color.

use std::borrow::Cow;

use gpui::{AssetSource, Hsla, SharedString, Styled, Svg, px, svg};

use crate::theme::metrics::ICON;

/// An icon of the design system. Each is drawn on a 16 point grid with a 1.5 point stroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Search,
    Plus,
    Minus,
    Close,
    Check,
    ChevronDown,
    ChevronRight,
    Undo,
    Redo,
    Sun,
    Moon,
    System,
    Help,
    Fit,
    Sparkle,
    External,
    Warning,
}

impl Icon {
    const ALL: [Icon; 17] = [
        Icon::Search,
        Icon::Plus,
        Icon::Minus,
        Icon::Close,
        Icon::Check,
        Icon::ChevronDown,
        Icon::ChevronRight,
        Icon::Undo,
        Icon::Redo,
        Icon::Sun,
        Icon::Moon,
        Icon::System,
        Icon::Help,
        Icon::Fit,
        Icon::Sparkle,
        Icon::External,
        Icon::Warning,
    ];

    fn path(self) -> &'static str {
        match self {
            Icon::Search => "icons/search.svg",
            Icon::Plus => "icons/plus.svg",
            Icon::Minus => "icons/minus.svg",
            Icon::Close => "icons/close.svg",
            Icon::Check => "icons/check.svg",
            Icon::ChevronDown => "icons/chevron-down.svg",
            Icon::ChevronRight => "icons/chevron-right.svg",
            Icon::Undo => "icons/undo.svg",
            Icon::Redo => "icons/redo.svg",
            Icon::Sun => "icons/sun.svg",
            Icon::Moon => "icons/moon.svg",
            Icon::System => "icons/system.svg",
            Icon::Help => "icons/help.svg",
            Icon::Fit => "icons/fit.svg",
            Icon::Sparkle => "icons/sparkle.svg",
            Icon::External => "icons/external.svg",
            Icon::Warning => "icons/warning.svg",
        }
    }

    fn bytes(self) -> &'static [u8] {
        match self {
            Icon::Search => include_bytes!("../../../assets/icons/search.svg"),
            Icon::Plus => include_bytes!("../../../assets/icons/plus.svg"),
            Icon::Minus => include_bytes!("../../../assets/icons/minus.svg"),
            Icon::Close => include_bytes!("../../../assets/icons/close.svg"),
            Icon::Check => include_bytes!("../../../assets/icons/check.svg"),
            Icon::ChevronDown => include_bytes!("../../../assets/icons/chevron-down.svg"),
            Icon::ChevronRight => include_bytes!("../../../assets/icons/chevron-right.svg"),
            Icon::Undo => include_bytes!("../../../assets/icons/undo.svg"),
            Icon::Redo => include_bytes!("../../../assets/icons/redo.svg"),
            Icon::Sun => include_bytes!("../../../assets/icons/sun.svg"),
            Icon::Moon => include_bytes!("../../../assets/icons/moon.svg"),
            Icon::System => include_bytes!("../../../assets/icons/system.svg"),
            Icon::Help => include_bytes!("../../../assets/icons/help.svg"),
            Icon::Fit => include_bytes!("../../../assets/icons/fit.svg"),
            Icon::Sparkle => include_bytes!("../../../assets/icons/sparkle.svg"),
            Icon::External => include_bytes!("../../../assets/icons/external.svg"),
            Icon::Warning => include_bytes!("../../../assets/icons/warning.svg"),
        }
    }
}

/// `icon` at the size it has beside a label, in `color`.
pub fn icon(icon: Icon, color: impl Into<Hsla>) -> Svg {
    svg().path(icon.path()).flex_shrink_0().size(px(ICON)).text_color(color)
}

/// Serves the icons to gpui's SVG renderer.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(Icon::ALL.into_iter().find(|icon| icon.path() == path).map(|icon| Cow::Borrowed(icon.bytes())))
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Icon::ALL.into_iter().map(Icon::path).filter(|icon| icon.starts_with(path)).map(Into::into).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_served_as_an_svg() {
        for icon in Icon::ALL {
            let bytes = Assets.load(icon.path()).unwrap().expect("the icon is served");
            assert!(bytes.starts_with(b"<svg"), "{icon:?}");
        }
        assert_eq!(Assets.list("icons/").unwrap().len(), Icon::ALL.len());
        assert!(Assets.load("icons/missing.svg").unwrap().is_none());
    }
}
