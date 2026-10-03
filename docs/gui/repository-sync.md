# Repository Selection and Synchronization Architecture

Internal lifecycle, repository switching, and background synchronization architecture in `topo-gui`.

## Target Switching Lifecycle

The window hosts a single `TopoApp` entity at any given time:

- **Background Target Preparation**: Loading a target workspace runs asynchronously on a background task. The existing editor stays alive but hidden behind the chooser, with its polling paused
- **Generation Tracking**: Switching requests carry monotonic generation IDs. Cancellation discards the background completion without affecting the active editor
- **Explicit Editor Retirement**: Once a target successfully initializes and loads, the previous editor is explicitly retired, dismantling its file watchers, periodic schedulers, and gesture monitors
- **State Cleanup**: A successful switch clears unsubmitted drafts, resets selection, and frames the new graph (large graphs are anchored at their start rather than shrunk to fit). Failed loads, cancellations, and re-selecting the current workspace preserve the current state

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
