# Repository selection and synchronization

The window owns one `TopoApp` entity at a time. Preparing a target uses a background task and keeps the previous editor alive, hidden, with its drafts intact. Only a successful open and watcher preparation installs the new editor. The old editor is explicitly retired, so even a previous rendered frame retaining it cannot keep a watcher, scheduler or gesture monitor active. Old asynchronous suggestion completions and fetched snapshots are ignored. Preparation requests have a generation; cancellation invalidates their completion without applying a new target. Initialization is an explicit action and may finish creating the workspace even if subsequent loading is cancelled.

Startup preserves `TOPO_DIR` precedence, including custom directory names. Explicit arguments and cwd retain ancestor discovery. Interactive folder selection opens the selected folder's own `.topo`; it never silently opens a parent repository. History stores exact canonical workspace paths, capped at ten, alongside inspector width. A successful switch discards unsubmitted drafts and resets graph-specific state; failure, cancellation and selecting the same workspace preserve it. PR URLs remain explicit node metadata.

Each editor owns one scheduler and its activation subscription. GPUI window activation reflects operating-system focus, so minimized, hidden and background windows stop periodic fetching without mistaking input-field focus changes for inactivity. The repository chooser also pauses its hidden editor. An inactive scheduler waits on a channel, without periodic timer wakeups. On return, queued events are coalesced into one immediate fetch. Activation during an in-flight request schedules at most one follow-up after completion. Requests use the known version; a snapshot overtaken by a save is ignored. Unchanged responses do not notify the view. Failure retries use 10/20/40/60 seconds and one toast per outage; return wakes a retry immediately. Explicit saves are unchanged.

## Validation (2026-10-03)

The managed Linux environment uses Rust 1.96.0 and the pinned GPUI revision. Automated tests use GPUI headless windows, virtual time and a counting `MemoryRemote` adapter; they do not contact a production service. The full workspace passes 173 tests, including 80 GUI tests; three existing graph benchmarks remain ignored. Formatting, diff whitespace checks and GUI Clippy with warnings denied pass.

- Switching two local workspaces with identical node IDs resets selection and undo, retires the previous watcher/task, and saves only to the new target.
- Cancelled native folder selection, nonexistent paths, uninitialized folders, explicit initialization, same-target selection and invalid cloud links preserve the intended target/draft behavior.
- Startup precedence and user-config migration/history round trips are covered.
- Request count: initial load 1; activation +1; five active seconds +1; 120 inactive seconds +0; return +1. Hidden editor: 120 seconds +0, return +1. Rapid activation toggles coalesce into one fetch. Retirement: 120 seconds +0; manually supplied old completion ignored.
- Outage retries: no retry at five seconds; successive intervals 10/20/40/60/60 seconds. Toast serial stays unchanged throughout that outage. Return and recovery resume immediate/five-second fetching.
- Existing GUI input tests used macOS modifiers on Linux and failed in both the baseline and modified trees. The test helper now selects the platform's standard modifier, matching production bindings.

## Rendered GUI QA

Real windows were rendered on Linux X11 using Xvfb, Openbox and Mesa's software Vulkan driver. The QA build enables `gpui_platform/x11`; the release/default backend configuration is unchanged. Isolated user configuration and local fixtures prevent changing the user's repository history. Screenshots are saved under `/workspace/scratch/topo-gui-qa/screenshots/` in this execution workspace.

The redesigned chooser separates the current target, primary folder action, recent targets and initialization/error states. Its body scrolls while its header and Cancel footer remain visible. Recent rows keep their height with ten entries at 720 × 480, and the final entry remains selectable. Both wide and narrow rendered windows were inspected after adjusting path contrast and preventing row compression.

| Screenshot | Check |
| --- | --- |
| `12-open-repository-final.png` | Final chooser with actual cloud-linked current repository and recent targets |
| `13-open-repository-final-narrow.png` | Same design at 720 × 480 |
| `11-design-scrolled.png` | Long history scrolls while Cancel remains visible |
| `03-beta.png` | Local target switch updates graph and window title |
| `14-cloud-before-minimize.png`, `15-cloud-after-resume.png` | Live cloud-linked window minimized, task changed through topo CLI, restored view shows Done |
| `16-uninitialized-final.png` | Explicit initialization callout; clicking Initialize opens the created workspace |
| `17-error-final.png` | Missing target provides an actionable error and recent alternatives |
| `18-startup-restore-final.png` | No-argument restart restores the most recently opened workspace |

The actual cloud QA only changed the status of the authorized polling implementation task (`2p2274`) through topo CLI. The screenshot confirms the restored GUI applies that update; precise request counts and suppression while inactive are established by virtual-time tests.

Native macOS/Windows folder dialogs and rendering still need device checks. Linux's native folder portal was unavailable in the headless environment; folder selection cancellation is covered through GPUI's test picker. The existing screenshot feature exports `VisualTestAppContext` only on macOS, so Linux QA captures actual X11 windows instead. Broader frame-time/CPU/large-graph profiling remains in the shared performance tasks; these changes verify request counts, not a measured frame-time improvement.
