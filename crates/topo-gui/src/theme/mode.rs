//! Which theme the UI uses: a fixed one, or the one the OS asks for.

use std::cell::Cell;

use gpui::{App, WindowAppearance, actions};
use serde::{Deserialize, Serialize};

use super::{CURRENT, Theme, palette};

/// The user's choice, as saved in the config. `System` follows the OS appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::System => "System",
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
        }
    }

    /// The mode a toolbar click cycles to.
    pub fn next(self) -> Self {
        match self {
            ThemeMode::System => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::System,
        }
    }

    /// The theme this mode draws with when the OS is in `appearance`.
    pub fn resolve(self, appearance: WindowAppearance) -> &'static Theme {
        match (self, appearance) {
            (ThemeMode::Light, _) | (ThemeMode::System, WindowAppearance::Light | WindowAppearance::VibrantLight) => {
                &palette::LIGHT
            }
            _ => &palette::DARK,
        }
    }
}

thread_local! {
    // Dark until the app applies the saved mode, matching `CURRENT`, so tests and captures need no setup.
    static MODE: Cell<ThemeMode> = const { Cell::new(ThemeMode::Dark) };
}

/// The mode the user chose.
pub fn mode() -> ThemeMode {
    MODE.with(Cell::get)
}

/// Makes `mode` the chosen mode and draws with the theme it resolves to under `appearance`.
pub fn apply(mode: ThemeMode, appearance: WindowAppearance) {
    MODE.with(|cell| cell.set(mode));
    CURRENT.with(|cell| cell.set(mode.resolve(appearance)));
}

actions!(theme, [ThemeSystem, ThemeLight, ThemeDark]);

/// Registers the menu actions that choose a mode.
pub fn init(cx: &mut App) {
    cx.on_action(|_: &ThemeSystem, cx| set_mode(ThemeMode::System, cx));
    cx.on_action(|_: &ThemeLight, cx| set_mode(ThemeMode::Light, cx));
    cx.on_action(|_: &ThemeDark, cx| set_mode(ThemeMode::Dark, cx));
}

/// Switches to `mode`: saves it, moves the menu check mark and redraws every window.
pub fn set_mode(mode: ThemeMode, cx: &mut App) {
    apply(mode, cx.window_appearance());
    persist(mode);
    cx.set_menus(crate::menus(mode));
    cx.refresh_windows();
}

/// Saves the chosen mode so the next launch starts with the same one.
#[cfg_attr(test, allow(unused_variables))]
fn persist(mode: ThemeMode) {
    #[cfg(not(test))]
    {
        let mut config = crate::config::UserConfig::load();
        config.theme = mode;
        let _ = config.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use WindowAppearance::{Dark, Light, VibrantDark, VibrantLight};

    #[test]
    fn modes_resolve_to_a_theme_for_every_appearance() {
        for appearance in [Light, VibrantLight, Dark, VibrantDark] {
            assert_eq!(ThemeMode::Light.resolve(appearance), &palette::LIGHT);
            assert_eq!(ThemeMode::Dark.resolve(appearance), &palette::DARK);
        }
        assert_eq!(ThemeMode::System.resolve(Light), &palette::LIGHT);
        assert_eq!(ThemeMode::System.resolve(VibrantLight), &palette::LIGHT);
        assert_eq!(ThemeMode::System.resolve(Dark), &palette::DARK);
        assert_eq!(ThemeMode::System.resolve(VibrantDark), &palette::DARK);
    }

    #[test]
    fn the_toolbar_cycles_through_every_mode() {
        let cycle: Vec<_> = std::iter::successors(Some(ThemeMode::System), |m| Some(m.next())).take(4).collect();
        assert_eq!(cycle, [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark, ThemeMode::System]);
    }

    #[test]
    fn apply_sets_the_mode_and_the_resolved_theme() {
        apply(ThemeMode::System, Light);
        assert_eq!((mode(), super::super::current()), (ThemeMode::System, &palette::LIGHT));
        apply(ThemeMode::System, Dark);
        assert_eq!(super::super::current(), &palette::DARK);
    }
}
