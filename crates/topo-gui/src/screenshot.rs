//! Offscreen rendering: `topo-gui --screenshot <path>` writes a PNG of the
//! window without opening it on screen.
//!
//! The renderer is gpui's real Metal backend driven by a headless platform, so
//! this needs neither a visible display nor the OS screen-recording permission.
//! It is compiled only with the `screenshot` feature.

use std::path::Path;

use anyhow::{Context as _, Result};
use gpui::{AppContext as _, Entity, VisualTestAppContext, px, size};
use topo_core::Workspace;

use crate::args::Args;
use crate::{TopoApp, text_input};

/// Renders one frame of the editor for `ws` to `path`, as `options` steers it.
///
/// Must run on the macOS main thread, like every AppKit interaction.
pub fn render(ws: Workspace, path: &Path, options: &Args) -> Result<()> {
    let platform = gpui_platform::current_platform(false);
    let mut cx = VisualTestAppContext::new(platform);
    cx.update(text_input::bind_keys);

    let requested = size(px(options.width as f32), px(options.height as f32));
    let inspector_width = options.inspector_width.map(|width| width as f32);
    let handle = cx.open_offscreen_window(requested, |window, cx| {
        // `TopoApp::new` starts the file watcher, which is harmless offscreen.
        cx.new(|cx| {
            TopoApp::with_inspector_width(ws, inspector_width, window, cx).expect("the workspace opens for rendering")
        })
    })?;
    let entity: Entity<TopoApp> = handle.entity(&cx)?;

    // The platform caps a window at the display size; a smaller frame than
    // asked for must not pass for the requested one.
    let actual = cx.update_window(handle.into(), |_, window, _| window.viewport_size())?;
    if actual != requested {
        anyhow::bail!(
            "the window was limited to {}x{} points instead of {}x{}",
            f32::from(actual.width),
            f32::from(actual.height),
            options.width,
            options.height
        );
    }

    // Selection and the help overlay are plain state; set them before the frame.
    let help = options.help_overlay;
    cx.update(|app| {
        entity.update(app, |app, cx| {
            let selected: Vec<_> = options.select.iter().map(|id| app.graph().resolve(id)).collect::<Result<_, _>>()?;
            match selected.as_slice() {
                [] => {}
                [one] => app.select(Some(one.clone()), false),
                many => app.select_nodes(many, cx),
            }
            app.show_help = help;
            app.view = crate::layout::View {
                hide_completed: options.hide_completed,
                group_by_tag: options.group_by_tag,
                collapsed: options
                    .collapse
                    .iter()
                    .map(|tag| match tag.as_str() {
                        "untagged" => crate::layout::Group::Untagged,
                        tag => crate::layout::Group::Tag(tag.to_owned()),
                    })
                    .collect(),
            };
            cx.notify();
            anyhow::Ok(())
        })
    })
    .context("--select")?;

    if let Some(field) = options.edit {
        cx.update_window(handle.into(), |_, window, cx| {
            entity.update(cx, |app, cx| {
                app.start_inline(field, window, cx);
                if let Some(text) = &options.typed {
                    app.combo().update(cx, |combo, cx| combo.set_text(text, cx));
                }
            })
        })?;
    }

    if options.edit_notes || options.search.is_some() {
        cx.update_window(handle.into(), |_, window, cx| {
            entity.update(cx, |app, cx| match &options.search {
                Some(query) => {
                    app.open_prompt(crate::Prompt::Search, window, cx);
                    app.palette.update(cx, |palette, cx| palette.set_text(query, cx));
                }
                None => {
                    app.start_notes(window, cx);
                    if let Some(text) = &options.notes_text {
                        app.notes_input().update(cx, |input, cx| input.set_text(text, cx));
                    }
                    if options.unsaved_dialog {
                        app.ask_notes(crate::notes::Then::Stay, cx);
                    }
                }
            })
        })?;
    }

    // The first frames settle layout and the camera fit.
    for _ in 0..4 {
        cx.run_until_parked();
        cx.update(|app| app.notify(entity.entity_id()));
    }
    cx.run_until_parked();

    // Toggling a view mode in the app frames the new layout; do the same here.
    if options.hide_completed || options.group_by_tag {
        cx.update(|app| {
            entity.update(app, |app, cx| {
                app.refit();
                cx.notify();
            })
        });
        for _ in 0..2 {
            cx.run_until_parked();
            cx.update(|app| app.notify(entity.entity_id()));
        }
        cx.run_until_parked();
    }

    let image = cx.capture_screenshot(handle.into())?;
    write_png(&image, path)?;
    Ok(())
}

fn write_png(image: &image::RgbaImage, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    image.save_with_format(path, image::ImageFormat::Png)?;
    Ok(())
}
