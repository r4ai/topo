# topo-gui Motion

Motion makes a change easy to follow and keeps interaction responsive. The approach is inspired by [Apple's Motion guidance](https://developer.apple.com/design/human-interface-guidelines/motion), [Animate with springs](https://developer.apple.com/videos/play/wwdc2023/10158/) and [Designing Fluid Interfaces](https://developer.apple.com/videos/play/wwdc2018/803/): short travel, springs instead of fixed durations, and continuity when a move is interrupted. The parameters here are topo's own, not a reproduction of Apple's private animation presets.

## Principles

- **Springs, not durations.** Every animated value is a damped spring described by a response and a damping ratio. Nothing has a nominal duration; a spring is done when the distance and speed it has left are both invisible.
- **Interruptible.** Any motion can be retargeted at any moment. The new spring continues from the current value and velocity, so reversing, repeating or overriding a move never jumps or restarts.
- **Symmetric.** A surface leaves the way it came: the same spring, run backwards from wherever it is. Closing is not a cut.
- **Leaving surfaces are inert.** A surface that is animating out is still drawn but takes no pointer input, so it never blocks what is behind it.
- **Direct manipulation hands over velocity.** While a finger or pointer drags, the content follows it one-to-one. On release the spring starts at the gesture's velocity, so the content keeps its momentum instead of stopping and restarting.
- **Idle costs nothing.** A frame is requested only while some spring is unsettled. A settled UI draws no extra frames.
- **Motion carries no information on its own.** Every state is also visible at rest.

## Presets

A preset is `response` (the period of the undamped spring, in seconds) and `damping` (the damping ratio; 1 never overshoots, below 1 overshoots). A *spatial* preset moves or resizes something and snaps under reduced motion.

| Preset | Response | Damping | Spatial | Used for |
| :--- | ---: | ---: | :---: | :--- |
| `FADE` | 0.14 s | 1.00 | no | Opacity and colour: surface fades, scrims, card dimming, selection and done colours |
| `SNAPPY` | 0.23 s | 0.86 | yes | Popovers, dropdowns, toasts, the save bar, the theme thumb and switch, button press feedback, wheel zoom |
| `SMOOTH` | 0.32 s | 1.00 | yes | Dialogs, the camera, reflow |
| `BOUNCY` | 0.29 s | 0.68 | yes | The completion check and the zoom's return from past its limit |
| `GLIDE` | 0.90 s | 1.00 | yes | Pan inertia |

Only `SNAPPY` and `BOUNCY` overshoot, and only slightly. Sampling is a pure function of the time since the last change, so the frame rate does not affect the path.

## Core API

Everything lives in `crates/topo-gui/src/animation.rs`.

| Item | Purpose |
| :--- | :--- |
| `Spring` | One value moving to a target. `sample(now)` gives value and velocity, `retarget` continues from the current state, `push` starts from a gesture's velocity, `set_preset` changes the feel without a jump, `is_settled` says whether it has arrived |
| `springs(window, cx, id, initial, targets, preset)` | Hook: springs kept between frames that chase `targets`. Returns the current values and requests another frame while any still moves. `initial` is the starting point of a new spring; `preset: None` snaps. `springs_within` takes a custom rest tolerance (the default is 0.002 in the value's units) |
| `presence(window, cx, id, open, preset)` | Hook for a surface that opens and closes. Returns `[opacity, travel]`, 0 when closed and 1 when open, or `None` once it is closed and has faded out. Opacity follows `FADE`, travel follows `preset` and may overshoot |
| `Presence<T>` | A payload that outlives its dismissal while its surface animates out. `show` / `hide` change it, `get` is what the app acts on (`None` as soon as it is hidden), `shown` is what to draw, `progress` runs `presence` and drops the item when it returns `None` |
| `surface(element)` | Draws an element so it can be moved, scaled, faded and made inert; see below |
| `Pivot` | The fixed point of a scale: `Center`, `TopLeft`, `TopRight`, `Bottom` |
| `projection(velocity, preset)` | How far ahead of its position to target a spring released at `velocity` so it starts without acceleration |
| `lerp(from, to, t)` | Linear interpolation between two colours |

`springs` and `presence` are hooks. Call them while drawing: from `render`, `request_layout`, `prepaint` or `paint`, including the closures of `canvas`. Their state is element state, so an `id` is scoped by the ids of the elements around the call, and the springs are forgotten after a frame that does not ask for them. Call `presence` every frame, open or not, or the surface cannot animate out.

`surface(element)` has four modifiers: `offset(point)`, `scale(factor, pivot)`, `opacity(alpha)` and `inert(bool)`. The element is laid out exactly as it would be alone, so its size, margins, flex behaviour, sibling positions and scroll extent are untouched. When the opacity is below 1 the element is painted inside a fading wrapper, so everything it paints fades together instead of each child being multiplied separately.

## Surfaces

Every surface leaves by running its enter spring back from wherever it is, inert while it does. Numbers are the closed end of the travel.

| Surface | Enter / exit | Preset | Properties |
| :--- | :--- | :--- | :--- |
| Prompt (create, pick, search) | Fade, rise, grow | `SMOOTH` | Dialog card: rises 8 pt, scales 0.96 → 1 about its centre, fades |
| Help, notes confirmation | Backdrop and card | `SMOOTH` | Dialog card as above; the scrim fades independently on `FADE` |
| Repository switcher | Backdrop and card | `SMOOTH` | Dialog card as above; the scrim only dims an open editor and fades on `FADE`; the dimming follows the switcher in and out |
| Theme popover | Fade, grows from the button | `SNAPPY` | Scales 0.92 → 1 about its top-right corner, fades |
| Theme thumb and Monotone switch | Slide | `SNAPPY` | The System / Light / Dark thumb and the switch knob chase their target and start in place when the menu opens; the label colours crossfade |
| Combobox dropdown | Fade, grows from the field | `SNAPPY` | Scales 0.92 → 1 about its top-left corner, fades; leaves when the field loses focus. A command palette's list sits inside its card and only fades |
| Toast | Rise, grow, fade | `SNAPPY` | Rises 12 pt, scales 0.96 → 1 about its bottom edge, fades; new text replaces the old in place without replaying the entrance |
| Save status bar | Drop in, fade | `SNAPPY` | Moves 8 pt down from the top edge, fades |
| Button | Press | `SNAPPY` | Shrinks to 0.97 about its centre while the left button is held on it and springs back on release, anywhere. Disabled buttons never shrink |
| Card selection | Border and shadow | `FADE` | Fade in and out with the selection |
| Card dimming | Opacity | `FADE` | Dimmed 0.22, closed and unselected 0.6, otherwise 1 |
| Card done | Colours | `FADE` | The done colours fade in |
| Completion check | Pop | `BOUNCY` | The status glyph grows from 0.6 to 1 with an overshoot when a card is completed; taking the status back grows the new glyph on `SMOOTH` |
| Card enter / exit | Grow, fade | `SMOOTH` for place, `FADE` for opacity | A new card grows from 0.9 and fades in at its cell; a removed card shrinks to 0.9 and fades out as a ghost |
| Reflow | Move | `SMOOTH` | Cards, group headers and the edges that join them travel to their new cells |
| Group chevron | Turn | `SMOOTH` (with reflow) | Turns from pointing right (0°) to pointing down (90°) as the group unfolds |
| Camera: fit, reveal, zoom keys and buttons, smart zoom | Move | `SMOOTH` | Offset and zoom are three springs that share a preset |
| Wheel zoom | Move | `SNAPPY` | Notches add up: each one zooms by 1.12 from where the last is heading, around the pointer |
| Pinch | Direct, then return | `BOUNCY` | Follows the fingers with a rubber band past the zoom limits; returns to the limit on release |
| ⌘ / ctrl pixel scroll zoom | Direct | none | Zoom follows the scroll and stops at the limits. There is no rubber band: the end of such a scroll is not reported by every device, so the zoom could not be released |
| Drag-pan | Direct, then fling | `GLIDE` | The canvas follows the pointer; on release it keeps the pointer's speed and glides to rest |

### Camera

The camera is three springs (x, y, zoom) in `TopoApp`. `fly` retargets them, `zoom_to` keeps the canvas point under an anchor fixed all the way (the offset springs start with matching velocity and share the zoom's preset), and `step_anim` samples them each frame and requests another frame while they move. A completed move arrives exactly at its destination. Offsets rest within 0.1 px and the zoom within 1e-6. Zoom stays between 0.25 and 2.5, except that a pinch that starts on an overview below 0.25 may return to it.

- **Pinch.** While the fingers are down, `pinched` applies the zoom directly. Within the limits it is free. Past a limit the zoom gives only `1 / (1 + 4 × distance)` as fast as the fingers ask (distance measured logarithmically), so it is continuous at the limit and follows the fingers back the way it came. On release `BOUNCY` returns it to the limit.
- **Drag-pan.** The pointer's speed is averaged over 40 ms. On release the canvas flings only if it was moving faster than 80 px/s and the pointer had not rested for 80 ms or more before the release. The fling targets the position `projection` computes for `GLIDE` and starts at the release velocity.
- **Taking hold.** Any mouse press stops a camera that is moving by itself. Line scroll, pixel scroll and drag-pan move the camera directly, from where it is drawn.

## Deliberately instant

- Hover, focus and pressed states of everything but buttons, and the cursor.
- Direct manipulation: pan, link and inspector-resize drags, and plain scroll.
- A theme change swaps the palette at once. The controls that chose it keep their cue (the Monotone knob and the System / Light / Dark thumb move; see above).
- Inspector selection and content, and the empty and all-hidden canvas: persistent content stays fully visible.
- Changing the content of a visible panel or toast does not replay its entrance or make it briefly transparent.

## Modes

`animation::mode()` returns how much motion runs now.

| Mode | When | Behaviour |
| :--- | :--- | :--- |
| `Full` | Normal running | Every spring animates |
| `Reduced` | `TOPO_REDUCE_MOTION=1` (or `true`), or the macOS accessibility flag *Reduce motion* (`NSWorkspace.accessibilityDisplayShouldReduceMotion`) | Spatial presets snap: nothing moves or scales, and flings and camera moves land at once. `FADE` still runs, so surfaces crossfade |
| `Off` | Tests, `--screenshot` and `--screenshot-all` | Every spring rests at its target on the first frame |

The environment variable and the OS flag are read whenever motion is used, so a change applies on the next frame; a camera move in progress finishes immediately if motion becomes reduced. Motion is on in the app and off in tests (`set_enabled`).

## Layout and hit testing

- **Offset moves hitboxes.** `surface(..).offset(..)` shifts the element in prepaint with GPUI's element offset, so painting, hitboxes and accessibility bounds move together. Travel is limited to the room between the surface and the window edges: a surface resting against an edge moves less, or only fades, instead of leaving the window.
- **Scale is paint-only.** Hitboxes keep their laid-out size and position; only what is painted is scaled about the pivot. During a small scale the clickable area is therefore slightly larger or smaller than what is drawn, which the short travel keeps unnoticeable.
- **Inert.** An inert surface is painted as usual while none of its hitboxes takes pointer input.
- **`deferred` and `anchored` content escapes a `surface`.** What the element hands to `deferred` or `anchored` is drawn later, outside the surface, so it does not move, scale or fade with it. Wrap the content of a deferred element rather than the deferred element.
- A scrim and its card are separate surfaces so that fading a dialog does not multiply the opacity of its backdrop through its parent.

## Performance

- **Reflow caps.** If more than 300 cards would move in one layout change (counted among those in the lookup area), the change snaps instead of animating. At most 40 cards leave as ghosts, taken from cells that were in the viewport when the change started.
- **Culling margin.** While cards are travelling, cards and edges are looked up in a viewport grown by how far any card still is from its cell: the most columns and rows any card is away from its cell, times the zoom and the unfinished fraction of the move, and never more than one viewport in each direction. At rest the margin is 0, so a settled canvas culls exactly as before.
- **Idle.** A frame is requested only while a spring is unsettled.
- Measurements: [canvas-performance.md](../canvas-performance.md).

## The gpui fork

Scaling a surface needs a transform that gpui does not have for styled elements. topo-gui therefore depends on a fork of Zed, [`r4ai/zed`](https://github.com/r4ai/zed), branch `topo/element-scale`, pinned by `rev` in `crates/topo-gui/Cargo.toml` (the `gpui` and `gpui_platform` dependencies and the `gpui` dev-dependency).

The patch is one commit that changes only `crates/gpui/src/window.rs`. It adds `Window::with_element_scale(scale, origin, f)`, which paints `f`'s primitives uniformly scaled about `origin`. The scale is applied on the CPU as primitives are inserted into the scene, so there are no renderer or shader changes. Layout and hitboxes are unaffected. Nested scales compose, and content masks pushed inside are scaled with the primitives. It must be called during paint.

To bump the fork:

1. Commit the change (or rebase the patch onto a newer upstream Zed) on `topo/element-scale` in `r4ai/zed`, and push the branch.
2. Replace the `rev` in all three places in `crates/topo-gui/Cargo.toml` with the new commit.
3. Run `cargo build` (or `cargo update -p gpui`) to refresh `Cargo.lock`; builds use `--locked`.
4. Run the gate in [development.md](../development.md).

Building the fork's own tests on a machine without the Xcode `metal` tool needs `--features gpui_platform/runtime_shaders`.

Known limits of the scale:

- Glyphs, emoji and SVGs are stretched tiles of the unscaled atlas, not re-rasterized, so text is slightly soft while a surface is scaled. Surface scales stay between 0.9 and 1 (0.6 for the completion check), and the softness disappears at rest, where no scale is applied.
- On Windows and Linux, subpixel text may show colour fringing during a scale.
- Cached views are not supported under a scale.
- The edges of scaled content snap to device pixels on each frame.

## Testing

Tests run with motion off, so frames are deterministic and surfaces are at their final state. A test that checks motion turns it on with a guard that restores the setting when dropped (`Motion::on()` in `ui_tests.rs`), then drives time itself: springs read the executor clock, so `cx.executor().advance_clock(duration)` followed by a redraw steps every spring by exactly that much. Tests cover viewport bounds, translated click targets, preserved margins, stable sibling layout, retargeting with preserved velocity, and surfaces that become inert while leaving.

## Implementation

`crates/topo-gui/src/animation.rs` owns the springs, the mode policy and `surface`. Views call the hooks while drawing and keep nothing but ids and targets; the motion state is element state. The dialog card and its scrim are shared as `dialog_motion` and `dimming` in `chrome.rs`.
