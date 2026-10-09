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

    /// The index of this mode among the theme menu's System/Light/Dark segments.
    pub fn index(self) -> usize {
        match self {
            ThemeMode::System => 0,
            ThemeMode::Light => 1,
            ThemeMode::Dark => 2,
        }
    }

    /// The theme this mode draws with when the OS is in `appearance`; `monotone` drops the signal hues.
    pub fn resolve(self, appearance: WindowAppearance, monotone: bool) -> &'static Theme {
        let light = match self {
            ThemeMode::Light => true,
            ThemeMode::Dark => false,
            ThemeMode::System => matches!(appearance, WindowAppearance::Light | WindowAppearance::VibrantLight),
        };
        match (light, monotone) {
            (true, false) => &palette::LIGHT,
            (true, true) => &palette::LIGHT_MONO,
            (false, false) => &palette::DARK,
            (false, true) => &palette::DARK_MONO,
        }
    }
}

thread_local! {
    // Dark until the app applies the saved mode, matching `CURRENT`, so tests and captures need no setup.
    static MODE: Cell<ThemeMode> = const { Cell::new(ThemeMode::Dark) };
    static MONOTONE: Cell<bool> = const { Cell::new(false) };
}

/// The mode the user chose.
pub fn mode() -> ThemeMode {
    MODE.with(Cell::get)
}

/// Whether the signal hues are replaced by neutrals.
pub fn monotone() -> bool {
    MONOTONE.with(Cell::get)
}

/// Makes `mode` and `monotone` the choice and draws with the theme they resolve to under `appearance`.
pub fn apply(mode: ThemeMode, monotone: bool, appearance: WindowAppearance) {
    MODE.with(|cell| cell.set(mode));
    MONOTONE.with(|cell| cell.set(monotone));
    CURRENT.with(|cell| cell.set(mode.resolve(appearance, monotone)));
}

actions!(theme, [ThemeSystem, ThemeLight, ThemeDark, ToggleMonotone]);

/// Registers the menu actions that choose a mode or toggle monotone.
pub fn init(cx: &mut App) {
    cx.on_action(|_: &ThemeSystem, cx| set_mode(ThemeMode::System, cx));
    cx.on_action(|_: &ThemeLight, cx| set_mode(ThemeMode::Light, cx));
    cx.on_action(|_: &ThemeDark, cx| set_mode(ThemeMode::Dark, cx));
    cx.on_action(|_: &ToggleMonotone, cx| set_monotone(!monotone(), cx));
}

/// Switches to `mode`: saves it, moves the menu check mark and redraws every window.
pub fn set_mode(mode: ThemeMode, cx: &mut App) {
    apply(mode, monotone(), cx.window_appearance());
    settled(cx);
}

/// Turns the signal hues off or on, like `set_mode`.
pub fn set_monotone(monotone: bool, cx: &mut App) {
    apply(mode(), monotone, cx.window_appearance());
    settled(cx);
}

fn settled(cx: &mut App) {
    persist();
    cx.set_menus(crate::menus(mode(), monotone()));
    cx.refresh_windows();
}

/// Saves the choice so the next launch starts with the same one.
fn persist() {
    #[cfg(not(test))]
    {
        let mut config = crate::config::UserConfig::load();
        (config.theme, config.monotone) = (mode(), monotone());
        let _ = config.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use WindowAppearance::{Dark, Light, VibrantDark, VibrantLight};

    const PALETTES: [(bool, &Theme, &Theme); 2] =
        [(false, &palette::LIGHT, &palette::DARK), (true, &palette::LIGHT_MONO, &palette::DARK_MONO)];

    #[test]
    fn modes_resolve_to_a_theme_for_every_appearance_and_monotone_setting() {
        for appearance in [Light, VibrantLight, Dark, VibrantDark] {
            for (monotone, light, dark) in PALETTES {
                assert_eq!(ThemeMode::Light.resolve(appearance, monotone), light);
                assert_eq!(ThemeMode::Dark.resolve(appearance, monotone), dark);
            }
        }
        for (monotone, light, dark) in PALETTES {
            assert_eq!(ThemeMode::System.resolve(Light, monotone), light);
            assert_eq!(ThemeMode::System.resolve(VibrantLight, monotone), light);
            assert_eq!(ThemeMode::System.resolve(Dark, monotone), dark);
            assert_eq!(ThemeMode::System.resolve(VibrantDark, monotone), dark);
        }
    }

    #[test]
    fn apply_sets_the_mode_the_monotone_flag_and_the_resolved_theme() {
        apply(ThemeMode::System, false, Light);
        assert_eq!((mode(), monotone(), super::super::current()), (ThemeMode::System, false, &palette::LIGHT));
        apply(ThemeMode::System, true, Dark);
        assert_eq!((monotone(), super::super::current()), (true, &palette::DARK_MONO));
        apply(ThemeMode::Dark, false, Dark);
    }
}
