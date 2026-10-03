//! User-level settings that are not part of a workspace.
//!
//! Workspace data lives in `.topo` and is committed to Git. Window geometry
//! such as the inspector width is a property of this machine and user, so it
//! lives under the platform configuration directory instead.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Inspector width limits, shared by the field default and the resize drag.
pub const INSPECTOR_DEFAULT_FRACTION: f32 = 0.26;
pub const INSPECTOR_MIN_WIDTH: f32 = 240.;
pub const INSPECTOR_MAX_FRACTION: f32 = 0.6;
pub const INSPECTOR_COMPACT_WIDTH: f32 = 300.;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UserConfig {
    /// Saved inspector width in pixels, or `None` to derive it from the window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspector_width: Option<f32>,
}

/// `topo-gui/config.toml` under the platform configuration directory.
pub fn path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("TOPO_CONFIG_DIR") {
        return Some(PathBuf::from(dir).join("config.toml"));
    }
    dirs::config_dir().map(|dir| dir.join("topo-gui").join("config.toml"))
}

impl UserConfig {
    /// Loads the saved settings, or the defaults when none are readable.
    pub fn load() -> Self {
        path().and_then(|path| Self::load_from(&path)).unwrap_or_default()
    }

    fn load_from(path: &Path) -> Option<Self> {
        let text = fs::read_to_string(path).ok()?;
        toml::from_str(&text).ok()
    }

    /// Writes the settings, creating the directory when needed. Best effort:
    /// a failure to persist a window size must not break the app.
    #[cfg_attr(test, allow(dead_code))]
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = path() else { return Ok(()) };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, toml::to_string_pretty(self).expect("config serializes to TOML"))
    }
}

/// The inspector width to use for a window `viewport` wide.
pub fn default_inspector_width(viewport: f32) -> f32 {
    let compact = viewport < 1180.;
    if compact {
        return INSPECTOR_COMPACT_WIDTH;
    }
    (viewport * INSPECTOR_DEFAULT_FRACTION).clamp(INSPECTOR_MIN_WIDTH, viewport * INSPECTOR_MAX_FRACTION)
}

/// Clamps a width to what the current window can show; a wide window keeps a
/// proportion of its space for the canvas.
pub fn clamp_inspector_width(width: f32, viewport: f32) -> f32 {
    let max = (viewport * INSPECTOR_MAX_FRACTION).max(INSPECTOR_MIN_WIDTH);
    width.clamp(INSPECTOR_MIN_WIDTH, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_width_scales_with_the_window_and_has_a_floor() {
        assert_eq!(default_inspector_width(1360.), 1360. * INSPECTOR_DEFAULT_FRACTION);
        // Narrow windows fall back to the compact width.
        assert_eq!(default_inspector_width(1000.), INSPECTOR_COMPACT_WIDTH);
    }

    #[test]
    fn width_is_clamped_between_minimum_and_a_share_of_the_window() {
        assert_eq!(clamp_inspector_width(10., 1360.), INSPECTOR_MIN_WIDTH);
        assert_eq!(clamp_inspector_width(2000., 1360.), 1360. * INSPECTOR_MAX_FRACTION);
        // A window too small still keeps the minimum, never a negative width.
        assert_eq!(clamp_inspector_width(500., 200.), INSPECTOR_MIN_WIDTH);
    }

    #[test]
    fn config_round_trips_and_ignores_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        assert_eq!(UserConfig::load_from(&path), None);
        let config = UserConfig { inspector_width: Some(412.) };
        fs::write(&path, toml::to_string_pretty(&config).unwrap()).unwrap();
        assert_eq!(UserConfig::load_from(&path), Some(config));
    }
}
