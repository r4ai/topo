# Promo Video Production

Sources and rendering pipeline for the 58-second, 1080p60 promo video.

## Rendering Pipeline

Run from the `promo/` directory:

```bash
# Generate soundtrack (create out/ first; it is gitignored)
mkdir -p out
node audio/synth.mjs out/audio.wav

# Render complete MP4 video (headless Chrome + ffmpeg)
node render.mjs

# Launch live local preview at http://127.0.0.1:8765/
node render.mjs --serve

# Render still frames
node render.mjs --stills 8.2,28.05 --dir out/stills
```

Requirements: Node 22+, ffmpeg, and Google Chrome (`CHROME=<path>` to override the binary path; the default is the macOS install location).

The video is written to `out/topo-pv.mp4`. If `out/audio.wav` is missing, the video renders without sound.

## Architecture

- `src/shared.js`: Shared 120 BPM timeline synchronizing visual keyframes and audio triggers
- `src/engine.js`: Canvas 2D drawing toolkit: easing, typography, card and edge drawing, screenshot placement, and the post pass
- `src/scenes_*.js`: Modular scene definitions (`scenes_product.js` showcases real UI)
- `audio/synth.mjs`: Audio synthesis and interface sound effects
- `assets/shots/`: Captured application window frames and real CLI/TUI outputs
- `assets/moves/*.mp4`: Captured GUI motion clips; `render.mjs` extracts their frames into `assets/shots/seq/`
- `assets/data.json`: Data bundle loaded by the page

## Capturing Product Footage

`capture/promo-hook.patch` is a source patch for `crates/topo-gui` that adds a scripted capture mode. The patched app opens a real window and grabs it through macOS window capture, so this step is macOS-only and not headless.

> The patch predates the repository-switching changes and no longer applies to the current `crates/topo-gui`; port it before recapturing.

Apply the patch to a copy of `crates/topo-gui`, build it, and put that `topo-gui` next to a `topo` binary:

```bash
TOPO_BIN=<dir-with-topo-and-patched-topo-gui> WORK=<scratch-dir> node capture/capture.mjs
```

The script also needs `ffmpeg`, `git`, and `uv`. It deletes and recreates both `$WORK` and `assets/shots`. After recapturing, update `CRIT_PATH` and `READY_ROWS` in `src/scenes_product.js` to match the new capture.
