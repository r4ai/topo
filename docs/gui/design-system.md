# topo-gui Design System

The visual language of `topo-gui`: principles, tokens, themes and shared components. Code lives in `crates/topo-gui/src/theme/` (tokens) and `crates/topo-gui/src/ui.rs` (components).

## Principles

1. **Monotone.** The interface is achromatic. Hierarchy comes from luminance, weight, fill versus outline, and glyphs, never from hue.
2. **Colour is an alarm.** Only three hues exist, and each means one thing: `danger` (overdue, error, urgent, destructive), `warn` (due soon, unsaved), `success` (saved, a valid drop target). Nothing decorative is coloured.
3. **Minimal.** One border weight (a hairline), few surfaces, generous radii, no ornament. A control is quiet until hovered.
4. **Glass.** Chrome reads as translucent material above the content: tinted fills with alpha, a hairline edge, a faint top highlight and a soft shadow. Where the OS can blur the window background, the window itself is glass.
5. **Meaning lives in one place.** Views name a role (`t.fg_muted`, `theme::status_color`), never a literal colour or size.

## Problems this replaces

The previous look, audited before the redesign:

- Sixteen `u32` colour constants, dark only, with no theme type and no way to switch.
- Hue carried most meaning (blue for doing, selection, tags and medium priority at once; amber for milestones, critical path, high priority and markers at once), so one colour meant several unrelated things.
- Colour literals bypassed the tokens for edges, milestone cards, the modal scrim and the selection tint.
- No tokens for radius, spacing, type size or elevation; each view chose its own.
- Two hand-built modals, and no shared dialog, list row, input frame or segmented control.

## Themes

Three modes: **System** (default, follows the OS appearance live), **Light**, **Dark**.

- **Storage.** `theme` in the per-user `topo-gui/config.toml` (`UserConfig`), as `"system"`, `"light"` or `"dark"`. The default is omitted from the file. It is never stored in a repository's `.topo/`.
- **Switching.** `View > Appearance` in the native menu, and the theme button in the toolbar, which cycles System, Light, Dark and names the new mode in a toast. A switch repaints every window at once and is saved immediately.
- **Implementation.** The active theme is a `&'static Theme` in a thread-local cell, read with `theme::current()`. `theme::apply` sets it and refreshes the windows. Views take `let t = theme::current();` once per render function.
- **Screenshots and tests** never read the saved mode. They render Dark unless `--theme light` is given, and are always opaque.

## Colour roles

Fills with alpha are tints, so they work on both an opaque and a blurred window.

| Role | Dark | Light | Use |
| :--- | :--- | :--- | :--- |
| `bg` | `#0f0f11` | `#f2f2f4` | Window root and canvas |
| `glass_alpha` | 0.78 | 0.72 | Alpha of `bg` when the OS blurs the window |
| `chrome` | white 3.5% | white 55% | Toolbar, inspector, persistence bar, band headers |
| `card` / `card_hover` | `#1a1a1d` / `#212125` | `#ffffff` / `#fafafb` | Task cards (opaque, edges pass beneath) |
| `card_milestone` / `card_milestone_hover` | `#222226` / `#2a2a2f` | `#ececef` / `#e4e4e8` | Milestone cards |
| `overlay` | `#1e1e22` 97% | `#ffffff` 97% | Floating glass: palette, help, lists, pills, toast, dialog |
| `control` / `control_hover` / `control_active` | white 6% / 10% / 16% | black 4% / 7% / 12% | Buttons, rows, chips |
| `field` | black 25% | white 90% | Input wells |
| `hairline` / `border_strong` | white 8% / 16% | black 8% / 16% | Borders |
| `highlight` | white 10% | white 90% | Inset top edge of glass |
| `fg` / `fg_muted` / `fg_faint` | `#f2f2f3` / `#a0a0a8` / `#6a6a73` | `#18181b` / `#5c5c66` / `#8a8a93` | Text levels |
| `emphasis` / `on_emphasis` | `#ffffff` / `#0f0f11` | `#111113` / `#ffffff` | The monotone accent: selection, focus, primary fill, critical path, doing |
| `selection` | white 22% | black 16% | Text selection |
| `danger` / `warn` / `success` | `#ff6b6b` / `#f0b445` / `#5fd39a` | `#d92d2d` / `#b7791f` / `#1a8f59` | Alarm states only |
| `edge` / `edge_closed` / `edge_dim` | white 28% / 14% / 7% | black 30% / 14% / 7% | Canvas edges |
| `grid_dot` | white 5.5% | black 8% | Canvas grid |
| `scrim` | black 50% | black 25% | Modal backdrop |
| `shadow` | black 50% | black 14% | Shadow colour |

Contrast floors, checked by a test for both themes on `bg`, `card` and `overlay`: `fg` 7:1, `fg_muted` 4.5:1, `fg_faint` 3:1, and `danger`, `warn`, `success` 3:1.

## Meaning without hue

The mapping functions in `theme/mod.rs` are the single source.

| Meaning | Expression |
| :--- | :--- |
| Status | Glyph first: `○` todo, `◐` doing, `✓` done, `⊘` dropped. Todo `fg_muted`; doing `emphasis` with a filled chip (`emphasis` fill, `on_emphasis` text); done `fg_faint` with strikethrough and 60% card opacity; dropped `fg_faint` |
| Priority | Glyph first: `!!`, `↑`, `=`, `↓`. Urgent `danger`; high `fg` semibold; medium `fg_muted`; low `fg_faint` |
| Selection | Card: 2px `emphasis` border and `e1` shadow. Rows and segments: `control_active` fill with `fg` text |
| Critical path | Edge `emphasis` at width 2.5; card border `border_strong` |
| Focused edge | `fg` at width 2 (dashed when it is a membership edge) |
| Milestone | `◆` in `fg`, `card_milestone` fill, `border_strong` border, larger radius, dashed membership edges in `edge` |
| Tag | Neutral chip: `control` fill, `fg_muted` text |
| Progress | Bar `fg_muted` on a `control` track; `fg` when complete |
| Due | Overdue `danger`; within three days `warn`; otherwise `fg_muted` |
| Readiness | Ready `fg`; in progress `emphasis`; blocked `fg_muted`; closed `fg_faint` |
| Link drag | Allowed `success`; refused `danger`; undecided `fg` |
| Toast | Neutral glass; a `danger` or `success` glyph and hairline only |
| Markdown | Heading `emphasis` (semibold); code and quote `fg_muted`; link `fg`; marker `fg_faint` |

## Metrics

Constants in `theme/metrics.rs`, in points.

| Group | Tokens |
| :--- | :--- |
| Radius | `R_XS 4`, `R_SM 6`, `R_MD 8`, `R_LG 12`, `R_XL 16`. Canvas cards: `10 × zoom` for tasks, `16 × zoom` for milestones |
| Space | `S1 2`, `S2 4`, `S3 6`, `S4 8`, `S5 12`, `S6 16`, `S7 24`, `S8 32` |
| Type | `T_CAPTION 10`, `T_SMALL 11`, `T_BODY 12`, `T_BODY_LG 13`, `T_TITLE 15`, `T_HEADING 18`. Weights: regular, medium, semibold |
| Control height | `H_ICON 22`, `H_BUTTON 26`, `H_INPUT 28`, `H_TOOLBAR 46` |

Elevation, as functions returning shadows in the theme's `shadow` colour:

| Level | Shadow | Use |
| :--- | :--- | :--- |
| `e1()` | y 2, blur 8 | A selected card |
| `e2()` | y 8, blur 24, plus an inset 1px `highlight` on the top edge | Pills, popovers, toasts |
| `e3()` | y 16, blur 48, plus the same inset | Dialogs, the palette |

## Components

All in `ui.rs`. A view composes these instead of styling a `div` by hand.

| Component | Notes |
| :--- | :--- |
| `button` | Variants: plain (`control`), primary (`emphasis` fill), danger (`danger` text and tint), ghost (no fill until hover). Height `H_BUTTON`, radius `R_MD` |
| `icon_button` | `H_ICON` square, ghost |
| `segmented` | One `control` track, the chosen segment in `control_active` |
| `input_frame` | `field` fill, hairline; `emphasis` border on focus, `danger` on error |
| `list_row` | Hover `control_hover`, selected `control_active` |
| `panel` | `chrome` fill with a hairline on the content side |
| `glass(level)` | `overlay` fill, hairline, radius `R_LG`, elevation `e2` or `e3` |
| `dialog` / `scrim` | `scrim` backdrop and a centred `glass` at `e3` |
| `chip` | Filled (`control`) or outline (hairline), text `T_CAPTION` |
| `kbd`, `section_label`, `divider`, `progress_bar`, `toast_frame` | Small fixed recipes |

## Window

| Platform | Background | Titlebar |
| :--- | :--- | :--- |
| macOS | OS blur behind the window; the root paints `bg` at `glass_alpha` | Transparent, traffic lights inset into the toolbar, which is the drag area |
| Windows 11 | Mica backdrop; the root paints `bg` at `glass_alpha` | Standard |
| Linux | Opaque `bg` | Standard |

## Limits

- GPUI has no per-element backdrop blur. Floating surfaces are simulated glass (a near-opaque fill, a hairline, a highlight and a shadow); only the window background is really blurred.
- GPUI cannot set the window's native appearance, so choosing Light on a dark OS leaves the blur material dark. `glass_alpha` is high enough to keep the theme legible.
- The offscreen renderer cannot capture the OS blur, so screenshots and tests are always opaque.
- The repository chooser (`repository_view.rs`) inherits the tokens but keeps its layout until it is rebuilt.
