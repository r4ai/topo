# topo logo

The adopted logo is concept 01: a lowercase `t` with a diagonal split.
The SVG redraws the selected concept as clean curves in solid white.

- `topo-logo.svg`: canonical vector asset, 512 × 512 viewBox, transparent background. Two editable paths; no embedded raster image or font dependency.
- `topo-logo.png`: 1024 × 1024 transparent PNG rendered from the SVG geometry.
- `topo-logo-512.png`: 512 × 512 transparent PNG rendered from the SVG geometry.
- `topo-logo-preview.png`: preview on the app's dark canvas color (`#111216`).
- `topo.icns`: macOS app icon, using the dark preview with sizes from 16 to 512 pixels. Embedded in the GUI for direct launches and included in release app bundles.
- `topo.ico`: Windows executable and native window icon, using resource ID 1 expected by GPUI, with 16, 32, 64, 128, and 256 pixel images.

The promo page uses the same SVG geometry in `promo/assets/favicon.svg`, with a dark background so the white mark remains visible on light browser tabs.

Linux windows use the application ID `dev.r4ai.topo` on both X11 and Wayland.
Register the matching desktop launcher and icon for a source-built GUI:

```sh
bash assets/branding/install-linux-desktop.sh /path/to/workspace /path/to/topo-gui
```

The launcher opens the specified initialized workspace. It is installed for the
current user in `$XDG_DATA_HOME` (default `~/.local/share`).

Regenerate `.icns`, `.ico`, and the favicon on macOS with
`python3 assets/branding/generate-icons.py`. This uses the existing dark preview
for native icons and the canonical SVG for the favicon; no imaging dependency
is required.

Use white on a dark background and preserve the diagonal gap and proportions.
