# Repository Selection and Synchronization Architecture

Internal lifecycle, repository switching, and background synchronization architecture in `topo-gui`.

## Target Switching Lifecycle

The window hosts a single `TopoApp` entity at any given time:

- **Background Target Preparation**: Loading a target workspace runs asynchronously on a background task. The existing editor stays alive but hidden behind the chooser, with its polling paused
- **Generation Tracking**: Switching requests carry monotonic generation IDs. Cancellation discards the background completion without affecting the active editor
- **Explicit Editor Retirement**: Once a target successfully initializes and loads, the previous editor is explicitly retired, dismantling its file watchers, periodic schedulers, and gesture monitors
- **State Cleanup**: A successful switch clears unsubmitted drafts, resets selection, and frames the new graph (large graphs are anchored at their start rather than shrunk to fit). Failed loads, cancellations, and re-selecting the current workspace preserve the current state

## Repository Switcher

The window shows the switcher whenever it has no editor or the user asks to change repository (toolbar repository name, `File > Open Repository…`, `Cmd+O`). It is a palette-style card (`ui::glass(Dialog)`, radius `R_XL`, up to 560px wide, 16px window margin, in the upper third). Over an editor it sits on `ui::scrim()`, with the canvas dimmed beneath; with no editor it sits on the plain window background. On macOS a drag strip stays above the scrim so the titlebar moves the window.

### Structure

- **Filter field**: frameless, focused on open. Typing filters the rows by a case-insensitive substring of the folder name and of the displayed path (home written as `~`). A text that is an absolute path or starts with `~` and names an existing folder adds a first row "Open <path>", highlighted
- **Status banner**: between the field and the list, one of: *loading* ("Opening <name>…", rows dimmed and inert), *error* (`danger` glyph, "Couldn't open repository" and up to three lines of detail), or *pending* (the folder has no workspace yet, with an "Initialize workspace" button)
- **List**: a `CURRENT` section with the workspace in the editor (a `✓`; Enter closes the switcher) and a `RECENT` section with `recent_workspaces` other than the current one. A row shows the folder name and its path, the middle of a long path cut to fit. A workspace whose `.topo/config.toml` has a `[cloud]` link carries a `cloud` chip. A folder that no longer exists is faint, carries a `missing` chip and a visible remove button, and opens nothing. The remove button (`✕`) shows on hover and on the highlighted row. No match shows "No matching repository"
- **First launch**: with no editor and no recents, a welcome block with the "Open folder…" primary button replaces the list
- **Footer**: "Open folder…" with its shortcut, the note that unsubmitted edits are discarded only on a switch, and the key hints

Whether a recent folder exists and whether it is cloud-linked are read once when the switcher opens, never per frame.

### Keyboard and Mouse

| Input | Action |
| :--- | :--- |
| `↑` / `↓`, `Ctrl+P` / `Ctrl+N` | Move the highlight, wrapping at the ends |
| `Enter` | Initialize the pending folder while its banner shows and the filter is empty; otherwise open the highlighted row |
| `Cmd+O` | Open the system folder picker (`Ctrl+O` off macOS) |
| `Cmd+Backspace` | Forget the highlighted recent (`Ctrl+Backspace` off macOS); on another row it edits the filter text |
| `Esc` | Clear a non-empty filter; otherwise close the switcher and return to the editor, restoring its drafts |
| Click a row | Open it |
| Click the scrim | Close the switcher (only over an editor) |

Forgetting writes the history at once and keeps the highlight on a row. While the switcher covers the editor, the editor's drafts are not synchronized with the focus, so closing the switcher restores them as they were.

## Startup and Discovery Precedence

Workspace discovery resolves in the following order:

1. `TOPO_DIR` environment variable (explicit workspace directory)
2. Positional CLI argument (walks up parent hierarchy to locate `.topo`)
3. Most recently opened canonical workspace from history
4. Current working directory (ancestor `.topo` search)

The folder chooser always mounts the selected directory's own `.topo` folder directly, preventing accidental discovery of a parent repository. History tracks up to 10 canonical workspace paths under `recent_workspaces` in `topo-gui/config.toml`, located in the platform config directory (or `$TOPO_CONFIG_DIR`).

## Background Synchronization Engine

### Polling Behavior

- **Local Workspaces**: Watch the `.topo` directory for file changes; no polling
- **Cloud-Linked Workspaces**: Poll the remote endpoint every 5 seconds using `If-None-Match: "<version>"`
- **Paused Polling**: Polling stops while the window is not the active window and while the repository chooser is open. In-editor overlays such as the notes editor do not pause it
- **Wakeup Coalescing**: Window focus re-activation triggers an immediate poll, coalescing any queued events into a single request
- **Single Flight**: Only one sync request is in flight per editor at any time

### Outage and Retry Strategy

Network request failures enter exponential backoff intervals of 10s, 20s, 40s, and up to 60s max. Error notifications are deduplicated to avoid visual toast spam during prolonged disconnects. Focus re-activation immediately resets and retries.
