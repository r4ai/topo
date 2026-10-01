# topo promo video

A 58-second, 1080p60 promo. The picture is drawn on a canvas as a pure function of time; the soundtrack is synthesized from the same timeline (`src/shared.js`), so every hit in the audio is an event the picture animates.

```bash
node audio/synth.mjs out/audio.wav   # soundtrack
node render.mjs                      # out/topo-pv.mp4 (headless Chrome + ffmpeg)
node render.mjs --serve              # live preview at http://127.0.0.1:8765/
node render.mjs --stills 8.2,28.05 --dir out/stills
```

Requires Node 22+, ffmpeg and Google Chrome (`CHROME=<path>` to override).

## Layout

- `src/shared.js`: timeline (120 BPM) and event times shared by picture and sound.
- `src/engine.js`: easing, type, the app's cards and edges, screenshot windows, post pass.
- `src/scenes_*.js`: the scenes. `scenes_product.js` (16–40 s) shows the real product.
- `audio/synth.mjs`: music and UI sound effects.
- `assets/shots/`: frames the real GUI wrote of its own window, plus real CLI and TUI output (`cli.json`, `tui.json`). `assets/data.json` bundles their metadata.

## Re-capturing the product footage

The GUI frames come from the app itself: `capture/promo-hook.patch` adds a script-driven capture mode (`TOPO_PROMO=<script>`) that replays state changes and writes the app's own window to PNG, with node positions next to each frame. It is not part of the product; apply it to a scratch copy of `crates/topo-gui`, build, then:

```bash
TOPO_BIN=<dir with topo and the patched topo-gui> WORK=<scratch dir> node capture/capture.mjs
```

Node ids are random, so a new capture can order rows differently; `CRIT_PATH` and `READY_ROWS` in `src/scenes_product.js` name what the inspector lists and must match the capture.
