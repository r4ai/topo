# topo-gui Motion

Motion makes a change easy to follow and keeps interaction responsive. The approach is inspired by [Apple's Motion guidance](https://developer.apple.com/design/human-interface-guidelines/motion) and [Animate with springs](https://developer.apple.com/videos/play/wwdc2023/10158/): short travel, a non-bouncy spring, and continuity when a move is interrupted. The parameters here are topo's own, not a reproduction of Apple's private animation presets.

## Timing

| Token | Value | Use |
| :--- | :--- | :--- |
| `FAST` | 160ms | Combobox dropdowns |
| `STANDARD` | 240ms | Theme popovers, toasts, save status |
| `SLOW` | 320ms | Prompts, help, notes confirmation, repository switcher |
| `CAMERA` | 360ms nominal | Fit, reveal, zoom; longer moves settle until the remaining error is subpixel |
| `FADE` | 120ms | Opacity and backdrop fade |
| `RISE` | 6pt | Maximum entering travel |

Position follows a critically damped spring. It starts at rest, moves promptly, and eases gently into place without bouncing. A surface's opacity reaches its final value within 120ms, while its position continues settling. Backdrops fade independently; fading a dialog does not multiply the opacity of its contents through its parent.

Camera moves use the analytical position and velocity of the same kind of spring. Retargeting first samples the running move and carries its velocity into the next one, including when an input arrives between rendered frames. Interactive zoom stays within its limits, and a completed move arrives exactly at its destination.

## Surfaces

| Surface | Motion |
| :--- | :--- |
| Prompt / search | Fade + small upward settling |
| Help, notes confirmation, repository switcher | Independent backdrop fade + card settling |
| Theme popover, combobox dropdown | Fade + small upward settling |
| Toast, save status | Fade + small upward settling |
| Inspector selection, empty / all-hidden canvas | Immediate; persistent content stays fully visible |
| Canvas camera | Continuous spring movement |

Hover, press, focus, theme changes, layout regrouping, and direct manipulation remain immediate. Entrance motion belongs to newly appearing floating surfaces. Changing the content of an existing panel or visible toast does not replay its entrance or briefly make it transparent. Closing a surface removes it at once, so it never holds input while disappearing. Motion does not convey information on its own.

## Layout and hit testing

`rise_in` translates a surface in the prepaint phase using GPUI's element offset. Its measured size, existing margins, sibling positions, scroll extent, and final anchored position are unchanged. Painting, hitboxes and accessibility bounds use the translated position together.

The offset is limited to the available room below the surface in the viewport. A dropdown next to the bottom edge travels less, or simply fades, instead of pushing its last row outside the window. A bottom-anchored toast moves visibly without changing its parent's size.

## Reduced motion and captures

`enabled()` checks `TOPO_REDUCE_MOTION=1` (or `true`) and macOS `NSWorkspace.accessibilityDisplayShouldReduceMotion` whenever motion is used. Changes to the OS preference apply on the next rendered frame; a camera move in progress finishes immediately if motion becomes disabled.

Tests and screenshot / batch QA entry points disable motion by default. Disabled motion draws the final state on the first frame. Regression tests explicitly enable motion to check viewport bounds, translated click targets, preserved margins, stable sibling layout, and camera retargeting.

## Implementation

`crates/topo-gui/src/animation.rs` owns the timing, spring response, policy, and the `Motion` extension. Views use `rise_in(id, duration)` or `fade_background(id, color)` for a scrim with independently animated children. Stable IDs preserve a running animation across renders, including when a visible toast's text changes. Once a surface is removed, its next appearance starts a new entrance.
