# Branding and Icons

Brand assets, application icons, and generation scripts for `topo`.

## Logo Assets

- `topo-logo.svg`: Canonical vector asset, 512 × 512 viewBox, transparent background
- `topo-logo.png`: 1024 × 1024 transparent PNG rendered from SVG geometry
- `topo-logo-512.png`: 512 × 512 transparent PNG
- `topo-logo-preview.png`: Preview rendered against dark canvas background (`#111216`)

## App Icons

- `topo.icon/`: Canonical layered macOS icon (Icon Composer format). Release bundles ship it compiled as `Assets.car`
- `topo.icns`: Compatibility macOS icon compiled from `topo.icon` (16 px to 1024 px). Embedded in the GUI binary for launches outside a bundle
- `topo-app-icon.png`: 512 × 512 Linux desktop launcher icon
- `topo.ico`: Multi-resolution Windows executable icon (16 px to 256 px)

## Scripts

- `generate-icons.py`: Regenerates the app icons (see [Icon Generation](#icon-generation))
- `install-linux-desktop.sh`: Installs the Linux desktop launcher
- `render-app-icon.swift`: Legacy static AppKit renderer, kept for reference; no longer used by packaging

## Linux Desktop Launcher

Linux windows register under application ID `dev.r4ai.topo`:

```bash
bash assets/branding/install-linux-desktop.sh /path/to/workspace /path/to/topo-gui
```

The workspace must already be initialized. The binary path is optional and defaults to the `topo-gui` found on `PATH`.

Under `$XDG_DATA_HOME` (default `~/.local/share`), the script installs the desktop entry to `applications/dev.r4ai.topo.desktop` and the icon to `icons/hicolor/512x512/apps/dev.r4ai.topo.png`.

## Icon Generation

Regenerate `.icns`, `.ico`, the Linux PNG, and promo favicon on macOS:

```bash
python3 assets/branding/generate-icons.py
```

The script compiles `topo.icon` with `actool` and derives the Windows and Linux icons from the result with `sips`. It requires full Xcode 26 or later; set `DEVELOPER_DIR` if Command Line Tools is the active selection.
