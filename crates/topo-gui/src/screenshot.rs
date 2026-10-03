//! Offscreen rendering: `topo-gui --screenshot <path>` writes a PNG of the
//! window without opening it on screen.
//!
//! The renderer is gpui's real Metal backend driven by a headless platform, so
//! this needs neither a visible display nor the OS screen-recording permission.
//! It is compiled only with the `screenshot` feature.

use std::path::Path;

use anyhow::Result;
use gpui::{AppContext as _, Entity, VisualTestAppContext, px, size};
use topo_core::Workspace;

use crate::{TopoApp, config, text_input};

/// Options that steer what the captured frame shows.
pub struct Options {
    /// Output PNG path.
    pub path: std::path::PathBuf,
    /// Window size in points.
    pub width: f32,
    pub height: f32,
    /// Inspector width override in points, mirroring a saved user preference.
    pub inspector_width: Option<f32>,
    /// Nodes to select before rendering (comma-separated), so their DETAILS are visible.
    pub select: Option<String>,
    /// Open the keyboard-shortcuts overlay.
    pub help: bool,
}

/// Renders one frame of the editor for `ws` to `options.path`.
///
/// Must run on the macOS main thread, like every AppKit interaction.
pub fn render(ws: Workspace, options: &Options) -> Result<()> {
    let platform = gpui_platform::current_platform(false);
    let mut cx = VisualTestAppContext::new(platform);
    cx.update(text_input::bind_keys);

    let inspector_width = options.inspector_width;
    let handle = cx.open_offscreen_window(size(px(options.width), px(options.height)), |window, cx| {
        // `TopoApp::new` starts the file watcher, which is harmless offscreen.
        cx.new(|cx| {
            TopoApp::with_inspector_width(ws, inspector_width, window, cx).expect("the workspace opens for rendering")
        })
    })?;
    let entity: Entity<TopoApp> = handle.entity(&cx)?;

    // Selection and the help overlay are plain state; set them before the frame.
    let select = options.select.clone();
    let help = options.help;
    cx.update(|app| {
        entity.update(app, |app, cx| {
            if let Some(ids) = select.as_deref() {
                let resolved: Vec<_> = ids.split(',').filter_map(|id| app.graph().resolve(id.trim()).ok()).collect();
                match resolved.as_slice() {
                    [] => {}
                    [one] => app.select(Some(one.clone()), false),
                    many => app.select_nodes(many, cx),
                }
            }
            app.show_help = help;
            cx.notify();
        });
        app.notify(entity.entity_id());
    });

    // The first frames settle layout and the camera fit.
    for _ in 0..4 {
        cx.run_until_parked();
        cx.update(|app| app.notify(entity.entity_id()));
    }
    cx.run_until_parked();

    let image = cx.capture_screenshot(handle.into())?;
    write_png(&image, &options.path)?;
    Ok(())
}

fn write_png(image: &image::RgbaImage, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    image.save_with_format(path, image::ImageFormat::Png)?;
    Ok(())
}

/// Parses `--screenshot` / `--screenshot=<path>` and the capture options from
/// argv, returning the remaining positional argument (the workspace path).
pub struct ParsedArgs {
    pub screenshot: Option<Options>,
    pub workspace: Option<std::path::PathBuf>,
}

pub fn parse_args(args: impl Iterator<Item = String>) -> Result<ParsedArgs> {
    let mut screenshot: Option<Options> = None;
    let mut workspace = None;
    let mut iter = args;
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--screenshot" => {
                let path = iter.next().ok_or_else(|| anyhow::anyhow!("--screenshot needs a path"))?;
                screenshot = Some(default_options(path.into()));
            }
            "--width" => {
                let value = iter.next().ok_or_else(|| anyhow::anyhow!("--width needs a number"))?;
                width_mut(&mut screenshot)?.width = value.parse()?;
            }
            "--height" => {
                let value = iter.next().ok_or_else(|| anyhow::anyhow!("--height needs a number"))?;
                width_mut(&mut screenshot)?.height = value.parse()?;
            }
            "--select" => {
                let value = iter.next().ok_or_else(|| anyhow::anyhow!("--select needs an id"))?;
                width_mut(&mut screenshot)?.select = Some(value);
            }
            "--inspector-width" => {
                let value = iter.next().ok_or_else(|| anyhow::anyhow!("--inspector-width needs a number"))?;
                width_mut(&mut screenshot)?.inspector_width = Some(value.parse()?);
            }
            "--help-overlay" => width_mut(&mut screenshot)?.help = true,
            other if other.starts_with("--") => {
                return Err(anyhow::anyhow!("unknown option `{other}`"));
            }
            other => workspace = Some(other.into()),
        }
    }
    Ok(ParsedArgs { screenshot, workspace })
}

fn default_options(path: std::path::PathBuf) -> Options {
    Options {
        path,
        width: 1360.,
        height: 860.,
        inspector_width: config::UserConfig::load().inspector_width,
        select: None,
        help: false,
    }
}

fn width_mut(options: &mut Option<Options>) -> Result<&mut Options> {
    options.as_mut().ok_or_else(|| anyhow::anyhow!("pass --screenshot <path> first"))
}
