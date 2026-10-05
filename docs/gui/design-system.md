# topo-gui Design System

The visual language of `topo-gui`: principles, tokens, themes and shared components. Code lives in `crates/topo-gui/src/theme/` (tokens) and `crates/topo-gui/src/ui.rs` (components).

## Principles

1. **Quiet.** Surfaces, text, borders and controls are achromatic. Hierarchy comes from luminance, weight and fill versus outline.
2. **Hue is a signal.** A small fixed set of hues, each with one job: `accent` (blue) for selection, focus, in-progress and links; amber for the critical path, milestones and high priority; green for done and ready; plus the alarms `danger`, `warn` and `success`. Nothing decorative is coloured.
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

Three modes: **System** (default, follows the OS appearance live), **Light**, **Dark**. Each has a hued palette (`DARK`, `LIGHT`) and a monotone one (`DARK_MONO`, `LIGHT_MONO`) that differ only in the signal roles.

- **Storage.** `theme` in the per-user `topo-gui/config.toml` (`UserConfig`), as `"system"`, `"light"` or `"dark"`, and `monotone` as a boolean. Defaults are omitted from the file. Neither is ever stored in a repository's `.topo/`.
- **Switching.** `View > Appearance` in the native menu: System, Light, Dark, then a checkable Monotone item. The theme button in the toolbar toggles a popover (Escape or a click outside closes it) holding the three modes as a segmented control and a Monotone switch. Choosing a mode names it in a toast. A switch repaints every window at once and is saved immediately.
- **Implementation.** The active theme is a `&'static Theme` in a thread-local cell, read with `theme::current()`. `theme::apply(mode, monotone, appearance)` sets it and `theme::set_mode` / `theme::set_monotone` also save and refresh the windows. Views take `let t = theme::current();` once per render function.
- **Screenshots and tests** never read the saved choice. They render Dark and hued unless `--theme light` or `--monotone` is given, and are always opaque.

### Monotone

An opt-in option, off by default and independent of the mode. It maps every signal role to a neutral (the table below), leaving only the alarms `danger`, `warn` and `success` coloured. It is stored as `monotone` in the user config and toggled from `View > Appearance` or the toolbar theme popover's switch.

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
| `emphasis` / `on_emphasis` | `#ffffff` / `#0f0f11` | `#111113` / `#ffffff` | The neutral accent: primary button fill, and the neutral stand-in for several signals in Monotone |
| `selection` | `accent` 30% | `accent` 22% | Text selection (Monotone: white 22% / black 16%) |
| `danger` / `warn` / `success` | `#ff6b6b` / `#f0b445` / `#5fd39a` | `#d92d2d` / `#b7791f` / `#1a8f59` | Alarm states only |
| `edge` / `edge_closed` / `edge_dim` | white 28% / 14% / 7% | black 30% / 14% / 7% | Canvas edges |
| `grid_dot` | white 5.5% | black 8% | Canvas grid |
| `scrim` | black 50% | black 25% | Modal backdrop |
| `shadow` | black 50% | black 14% | Shadow colour |

### Signal roles

Hue is used only through these roles. In Monotone each takes the neutral in the last column.

| Role | Dark | Light | Monotone |
| :--- | :--- | :--- | :--- |
| `accent` | `#7aa7ff` | `#2f6fed` | `emphasis` |
| `on_accent` | `#0f0f11` | `#ffffff` | `on_emphasis` |
| `critical` | `#f0b445` | `#b7791f` | `emphasis` |
| `milestone` | `#f0b445` | `#b7791f` | `fg` |
| `status_done` | `#5fd39a` | `#1a8f59` | `fg_faint` |
| `ready` | `#5fd39a` | `#1a8f59` | `fg` |
| `priority_high` | `#f0b445` | `#b7791f` | `fg` |
| `md_heading` | `accent` | `accent` | `emphasis` |
| `md_link` | `accent` | `accent` | `fg` |
| `progress` | `accent` | `accent` | `fg_muted` |
| `progress_complete` | `#5fd39a` | `#1a8f59` | `fg` |

Contrast floors, checked by a test for all four palettes on `bg`, `card` and `overlay`: `fg` 7:1, `fg_muted` 4.5:1, `fg_faint` 3:1, and `danger`, `warn`, `success`, `accent`, `critical`, `milestone`, `status_done`, `ready`, `priority_high` 3:1; `on_accent` on `accent` 4.5:1. A second test requires every signal role to be achromatic in the monotone palettes and `accent`, `critical`, `milestone`, `status_done` to be chromatic in the others.

## Meaning

The mapping functions in `theme/mod.rs` are the single source. Each row states the default expression; Monotone replaces the hue by the neutral in the signal table. Glyphs and weight carry the meaning in both.

| Meaning | Expression |
| :--- | :--- |
| Status | Glyph first: `○` todo, `◐` doing, `✓` done, `⊘` dropped. Todo `fg_muted`; doing `accent` with a solid chip (`accent` fill, `on_accent` text); done `status_done` with strikethrough and 60% card opacity; dropped `fg_faint` |
| Priority | Glyph first: `!!`, `↑`, `=`, `↓`. Urgent `danger`; high `priority_high` semibold; medium `fg_muted`; low `fg_faint`. Urgent and high are tinted chips on the canvas |
| Selection | Card: 2px `accent` border and `e1` shadow. Text selection: `accent` tint. Rows and segments: `control_active` fill with `fg` text |
| Critical path | Edge `critical` at width 2.5; card border `critical` at 60% |
| Focused edge | `accent` at width 2 (dashed when it is a membership edge) |
| Milestone | `◆` in `milestone` (`status_done` once closed), `card_milestone` fill, border `milestone` at 45%, larger radius, dashed membership edges in `milestone` at 50% |
| Tag | Neutral chip: `control` fill, `fg_muted` text |
| Progress | Bar `progress` on a `control` track; `progress_complete` when complete |
| Due | Overdue `danger`; within three days `warn`; otherwise `fg_muted`. Overdue and soon are tinted chips |
| Readiness | Ready `ready`; in progress `accent`; blocked `fg_muted`; closed `fg_faint`. On a card, "Ready" and "Done" are tinted chips |
| Link drag | Allowed `success`; refused `danger`; undecided `accent` |
| Focus | Input caret, focused input border, the link handle and the palette lead glyph in `accent` |
| Toast | Neutral glass; a `danger` or `success` glyph and hairline only |
| Markdown | Heading `md_heading` (semibold); code and quote `fg_muted`; link `md_link`; marker `fg_faint` |

A coloured chip has one recipe: text in the colour on a fill of that colour at 14%. The one solid chip is "In progress".

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
| `button_icon`, `button_primary_icon`, `button_ghost_icon` | A `button`, `button_primary` or `button_ghost` led by an `icon`; an empty label leaves the icon alone |
| `icon_button`, `icon_button_svg` | `H_ICON` square, ghost; showing a text glyph or an `icon` |
| `icon` | One of the app's own SVGs (`assets/icons/`, the `Icon` enum in `icons.rs`): a 16 point grid with a 1.5 point round stroke, drawn at `ICON` (14) beside a label and `ICON_LG` (16) leading a palette, in a theme color. Every control and status mark in the chrome uses one; node status and kind marks, and keys in `kbd`, stay text. Add an icon as an SVG file and an `Icon` variant rather than a Unicode glyph, whose size and baseline vary with the fallback font |
| `segmented` | One `control` track, the chosen segment in `control_active`; a segment's label may be any element, so a glyph can carry its own colour |
| `switch` | An on/off toggle: `accent` track with an `on_accent` knob when on, `control` track with an `fg_muted` knob when off |
| `input_frame` | `field` fill, hairline; `accent` border on focus, `danger` on error |
| `list_row` | Hover `control_hover`, selected `control_active` |
| `panel` | `chrome` fill with a hairline on the content side |
| `glass(level)` | `overlay` fill, hairline, radius `R_LG`, elevation `e2` or `e3` |
| `dialog` / `scrim` | `scrim` backdrop and a centred `glass` at `e3` |
| `chip` | Filled (`control`) or outline (hairline), text `T_CAPTION` |
| Repository switcher | Palette-style glass dialog; see [repository-sync.md](repository-sync.md) |
| `kbd`, `section_label`, `divider`, `progress_bar`, `toast_frame` | Small fixed recipes |

## Window

| Platform | Background | Titlebar |
| :--- | :--- | :--- |
| macOS | OS blur behind the window; the root paints `bg` at `glass_alpha` | Transparent, traffic lights inset into the toolbar, which is the drag area |
| Windows 11 | Mica backdrop; the root paints `bg` at `glass_alpha` | Standard |
| Linux | Opaque `bg` | Standard |

On macOS 26 and later, `TOPO_GLASS=1` swaps the blur for AppKit's `NSGlassEffectView` behind the window as an experiment (`glass.rs`).

## Limits

- GPUI has no per-element backdrop blur. Floating surfaces are simulated glass (a near-opaque fill, a hairline, a highlight and a shadow); only the window background is really blurred.
- GPUI cannot set the window's native appearance, so choosing Light on a dark OS leaves the blur material dark. `glass_alpha` is high enough to keep the theme legible.
- The offscreen renderer cannot capture the OS blur, so screenshots and tests are always opaque.

## Visual QA

Capture one state in a theme with the screenshot build (macOS):

```bash
cargo run -p topo-gui --features screenshot -- <workspace> --screenshot out.png --theme <dark|light> [--monotone] ...
```

Or capture the whole table below in both themes and both Monotone finishes in one process, generating its own fixture workspace, and write a contact sheet:

```bash
cargo run -p topo-gui --features screenshot -- --screenshot-all qa/
cargo run -p topo-gui --features screenshot -- --list-states   # the names below, plus the shell states
```

The table lives in `crates/topo-gui/src/qa.rs`: `--screenshot-all` walks it, and `--list-states` prints every name, so a new state is added in one place. Beyond the states below it also captures the multi-selection inspector, the priority filter, the Jev suggestions and busy toolbar, an empty workspace, an untagged grouping, and the three shell states (welcome, uninitialized folder, switcher with a missing recent).

| State | Flags |
| :--- | :--- |
| Default | none |
| Select | `--select <id>` |
| Milestone | `--select <milestone-id>` |
| Milestone, grouped | `--select <milestone-id> --group-by-tag` |
| Help | `--help-overlay` |
| Theme popover | `--theme-menu` (add `--monotone` to capture the switch on) |
| Save status | `--save-status saving` |
| Save failure | `--save-status error` |
| Toast | `--toast <text>` |
| Search | `--search <query>` |
| Notes | `--select <id> --edit-notes` |
| Inline edit | `--select <id> --edit tags --type <text>` |
| Unsaved dialog | `--select <id> --edit-notes --notes-text <text> --unsaved-dialog` |

Screenshots are opaque and never show the OS blur or the window chrome, so these are checked by hand:

- Switching the theme or Monotone, from the menu or the toolbar popover, repaints every window immediately, and the choice survives a restart.
- The toolbar popover closes on Escape and on a click outside it.
- System mode follows the OS appearance live.
- macOS: the titlebar drags the window, a double-click zooms it, fullscreen works, and the traffic lights align with the toolbar.
- Text stays legible over a bright and a dark desktop, in both themes.
- Windows 11: the Mica backdrop shows and the text stays legible.
