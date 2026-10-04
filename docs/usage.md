# topo Usage Guide

日本語: [docs/usage.ja.md](usage.ja.md)

Detailed usage guide for `topo`, covering the complete command reference, CLI/TUI/GUI interfaces, cloud synchronization, and AI agent integrations.

## Table of Contents

- [Command Reference](#command-reference)
- [CLI Advanced Features](#cli-advanced-features)
- [Node Attributes and Schema](#node-attributes-and-schema)
- [TUI (Terminal UI)](#tui-terminal-ui)
- [Native GUI (Desktop App)](#native-gui-desktop-app)
- [Cloud Synchronization](#cloud-synchronization)
- [AI and Automation](#ai-and-automation)

## Command Reference

| Command | Description | Common Flags |
| :--- | :--- | :--- |
| `topo init` | Initialize a `.topo` workspace | |
| `topo ready` | List unblocked tasks ready to start | `--under <id>`, `--assignee <name>`, `--unassigned`, `--pr <ref>`, `--sort priority`, `--format <fmt>` |
| `topo milestones` | List milestones with progress bar and critical path | `--format <fmt>` |
| `topo add <title>` | Create a new task or milestone | `--milestone`, `--dep <id>`, `--in <ms>`, `--due <date>`, `--tag <tag>`, `--priority <p>`, `--assignee <name>`, `--pr <ref>`, `--note <text>` |
| `topo link <from> <to>` | Add a dependency edge (`<from>` depends on `<to>`) | |
| `topo unlink <from> <to>` | Remove a dependency | |
| `topo join <task> <ms>` | Add a task to milestone membership | |
| `topo leave <task> <ms>` | Remove a task from a milestone | |
| `topo status <id> <st>` | Update status (`todo`, `doing`, `done`, `dropped`) | `--if <st>`, `--assign <name>` |
| `topo edit <id>` | Modify node attributes | `--title`, `--due`, `--no-due`, `--tag`, `--priority`, `--no-priority`, `--assignee`, `--no-assignee`, `--pr`, `--unpr`, `--note` |
| `topo rm <id>` | Delete a node and its adjacent edges | |
| `topo ls [-]` | List nodes or filter IDs from stdin | `--kind`, `--status`, `--under`, `--all`, `--tag`, `--in`, `--due-before`, `--due-after`, `--no-due`, `--ready`, `--blocked`, `--title`, `--priority`, `--assignee`, `--unassigned`, `--pr`, `--sort priority`, `--format` |
| `topo show <id>` | Show node details, neighbors, notes, and timestamps | `--json` |
| `topo deps <id>` | List prerequisites (including milestone members) | `--transitive`, `--format`, `--json` |
| `topo dependents <id>` | List downstream dependents | `--transitive`, `--format`, `--json` |
| `topo members <ms>` | List tasks belonging to a milestone | `--format`, `--json` |
| `topo critical-path <id>` | Display the longest remaining chain to completion | `--format`, `--json` |
| `topo graph` | Output dependency graph | `--format [tree\|mermaid\|dot]`, `--under <id>` |
| `topo apply` | Atomically apply JSON batch operations | `[file]` or stdin |
| `topo organize <what>` | Propose or apply graph optimizations via decision model | `deps`, `place`, `dupes`, `kinds`, `prioritize`, `--apply`, `--under <id>` (`prioritize` only) |
| `topo tui` | Launch terminal dashboard | |
| `topo login` / `topo logout` | Sign in to a cloud server / revoke token | `--url <url>`, `--name <token-name>` |
| `topo token <cmd>` | Manage tokens (`create`, `ls`, `revoke`) | `--name`, `--workspace <id>`, `--expires <dur>` |
| `topo cloud <cmd>` | Cloud workspace operations (`push`, `pull`, `link`, `ls`, `members`, `invite`, `remove`, `log`) | `--url <url>`, `--role <role>`, `--after <version>` |

## CLI Advanced Features

### Output Formats

`--json` works on every command. `--format` is available on the listing commands (`ls`, `ready`, `milestones`, `deps`, `dependents`, `members`, `critical-path`) and cannot be combined with `--json`:

- `--json`: Complete JSON tree or array
- `--format text`: Standard human-readable output
- `--format ids`: Newline-separated list of node IDs (ideal for pipes)
- `--format tsv`: Tab-separated values (ID, Kind, Status, Due Date, Title; no header row)
- `--format jsonl`: Line-delimited JSON objects

### Stdin and Pipeline Composition

Pass `-` as the argument to commands such as `status`, `edit`, `rm`, `join`, `leave`, `link`, `unlink`, or `ls` to process whitespace-separated IDs from stdin.

```bash
# Mark all ready tasks as doing
topo ready --format ids | topo status - doing

# Filter tagged tasks and assign to a milestone
topo ls --tag gui --format ids | topo ls - --due-before 2026-10-31 --format ids | topo join - a1b2c3

# Extract blocked prerequisite tasks for a milestone
topo deps a1b2c3 --transitive | topo ls - --blocked --format tsv

# Query specific task IDs with jq
topo ready --format jsonl | jq -r 'select(.status == "todo") | .id'
```

All operations and cycle invariants are validated atomically before writing to disk. If an invalid ID or cycle is detected, no changes are committed.

## Node Attributes and Schema

### Status Lifecycle

- `todo`: Work not yet started
- `doing`: Work currently in progress
- `done`: Work completed (satisfies prerequisites for downstream tasks)
- `dropped`: Work abandoned (treated as closed; does not block downstream tasks)

### Priority

Priority is an optional attribute and does not alter graph dependency or ready evaluation.

- Allowed values: `low`, `medium`, `high`, `urgent`
- Filter: `topo ls --priority high --priority urgent` or `topo ls --no-priority`
- Sort: `--sort priority` (highest priority first, unset last, ties broken by ID)

### Assignee, Due Date, Tags, and Pull Requests

- Assignee: `--assignee <name>` (clear with `--no-assignee`, filter with `--unassigned`)
- Due Date: `--due YYYY-MM-DD` (clear with `--no-due`, query with `--due-before`, `--due-after`)
- Tags: `--tag <name>` (repeatable)
- Pull Request: `--pr <url|owner/repo#123>` (canonical URL or GitHub shorthand; remove one link with `--unpr <url|owner/repo#123>`)

### Recorded Timestamps

Node timestamps are recorded in UTC seconds:

- `created_at`: Set when a node is initially created
- `updated_at`: Updated only when node content or attributes actually change
- `completed_at`: Recorded when status changes to `done` (cleared on reopen or drop)

## TUI (Terminal UI)

Launch the interactive dashboard with `topo tui`.

### Keybindings

| Key | Action |
| :--- | :--- |
| `1` – `4` | Switch view (1: Ready, 2: Milestones, 3: Open, 4: All) |
| `Tab` | Cycle to next view |
| `j` / `k`, `↓` / `↑` | Move selection |
| `Space` | Cycle status (`todo` → `doing` → `done` → `todo`) |
| `x` | Set status directly to `done` |
| `d` | Set status directly to `dropped` |
| `u` | Set status directly to `todo` |
| `q`, `Esc` | Quit TUI |

## Native GUI (Desktop App)

Built on GPUI (the GPU-accelerated UI framework powering the Zed editor), `topo-gui` provides high-performance canvas editing and instant synchronization with disk changes.

### Workspace Management

- Open Folder: Click the repository name in the toolbar or press `Cmd+O` (`Ctrl+O` on Linux/Windows)
- Automatic Discovery: Opens the selected folder's own `.topo` workspace without requiring a Git root
- Uninitialized Folders: Offers an inline "Initialize workspace" button
- History: Remembers up to 10 recent workspace paths across app restarts

### Canvas Navigation

- Pan: Drag canvas background/cards, middle-click drag, or use trackpad two-finger scroll
- Zoom: Mouse wheel, `Cmd`/`Ctrl` + trackpad scroll, or trackpad pinch (macOS only); zooms centered on the pointer
- Pan with a mouse wheel: `Cmd`/`Ctrl` + wheel (vertical), `Shift` + wheel (horizontal)
- Fit to View: Press `f`
- Actual Size: `Cmd+0`, Zoom In: `Cmd+=`, Zoom Out: `Cmd+-`
- Toggle Fit / Actual Size: Trackpad two-finger double-tap (macOS only)

### View Modes

Both toggles are in the bottom bar and are remembered across app restarts (user config, shared by all workspaces).

- Priority Filter: `Shift+P` dims nodes below a priority
- Hide Completed: `Shift+H` hides every done and dropped node together with its edges (edges are not re-routed through hidden nodes). A hidden node is also dropped from the selection, from Select All and from search
- Group by Tag: `Shift+G` lays the canvas out as one band per tag, sorted by name, with an `untagged` band last. Tags are sets, so a node with several tags appears in every one of its bands. Edges are drawn inside a band; an edge whose ends share no band is not drawn, except for the selected nodes: each of their dependency and milestone edges that no band shows gets one highlighted line from the selected node's card (the one last clicked) to the nearest card of the other end, so you can trace it. Ends in a folded band or hidden by Hide Completed have no card and get no line. Click a band header to fold or unfold it (folding is not remembered)
- The modes combine with each other and with the priority filter

### Appearance

- Modes: System (default, follows the OS), Light and Dark
- Monotone: an option, off by default, that draws every signal hue (blue, amber, green) as a neutral and leaves only the alarm colours; it combines with any mode
- Switch: `View > Appearance` or the theme button in the toolbar (a menu with the modes and Monotone); the choice is saved per user and applies to every window
- Details: [gui/design-system.md](gui/design-system.md)

### Node Operations

- Select: Click node (`Cmd` / `Ctrl` + click for multi-selection)
- Create: `n` for task, `m` for milestone
- Add Dependencies: `Tab` (creates follow-up task), `Shift+Tab` (creates prerequisite task)
- Connect Edges: Drag from edge handle or `Shift` + drag
  - Drop on another task: Creates dependency edge
  - Drop on a milestone: Joins milestone membership
  - Drop on empty canvas: Creates and connects a new follow-up task
- Connect Existing Nodes: `l` (pick prerequisite), `Shift+l` (pick dependent)
- Change Status: `Space` (cycle), `x` (toggle done), `1`–`4` (set Todo, Doing, Done, Dropped)
- Delete: `Backspace` or `Delete`
- Copy / Paste: `Cmd+C` / `Cmd+X` / `Cmd+V` (creates duplicates with fresh IDs preserving internal edges)
- Search: `/` or `Cmd+K` / `Cmd+F` (search by title, tag, or ID)
- Cheat Sheet: `?` (toggle shortcut overlay)

### Inspector (Right Panel)

- Resize Panel: Drag the left border (width persists in user config)
- In-Place Editing: Click any field or use shortcuts
  - `Enter` / `r`: Title
  - `p`: Priority
  - `a`: Assignee
  - `d`: Due date
  - `t`: Tags (press Space or comma to commit chip, Backspace to delete)
  - `g`: Pull Request (enter URL or `owner/repo#123`)
  - `e`: Notes editor (see below)
- Notes editor: Markdown is highlighted (headings, emphasis, code, links, lists, quotes); a "Saved" / "Unsaved changes" badge shows whether the text differs from what it opened with
  - `Cmd+Enter` saves and closes. `Esc`, a click elsewhere, selecting another node, or closing the window closes it silently when nothing changed; with unsaved changes it asks to Save (`Enter`), Discard (`D`) or Keep editing (`Esc`)
  - `Home` / `End` (also `Cmd+←` / `Cmd+→`, and `Ctrl+A` / `Ctrl+E` on macOS) go to the start / end of the line between line breaks; `Cmd+↑` / `Cmd+↓` (`Ctrl+Home` / `Ctrl+End`) go to the start / end of the notes; `Option+←` / `Option+→` move by word. Add `Shift` to extend the selection
  - `Tab` / `Shift+Tab` indent / outdent the selected lines (or type an indentation); `Enter` continues a list, task list or quote (an empty item ends it); `Shift+Enter` breaks the line plainly
  - Long notes scroll the panel to keep the cursor in view
- Autocompletion: Suggests workspace assignees, existing tags, and relative dates (`+3d`, `friday`)

## Cloud Synchronization

Share one graph across multiple devices and parallel AI agents using Cloudflare Workers and D1.

### Migration and Linking

```bash
# 1. Sign in via GitHub device flow
topo login --url https://topo.example.com

# 2. Push local workspace to cloud
topo cloud push --url https://topo.example.com

# 3. Use standard CLI/GUI commands (transparent cloud sync)
topo ready

# 4. Pull back to local Markdown files and unlink
topo cloud pull
```

Connection parameters are stored in `.topo/config.toml`. Cloud tokens are kept in user credentials outside the repository, so the `[cloud]` settings are safe to commit. If you add a Jev `api_key` under `[jev]` in the same file, do not commit it.

### Agent Coordination and Tokens

Issue scoped tokens for automation and CI environments:

```bash
# Create a 90-day token restricted to a workspace
topo token create --name agent-1 --workspace <workspace-id> --expires 90d

# Set environment variables in agent worker
export TOPO_CLOUD_URL="https://topo.example.com"
export TOPO_TOKEN="topo_xxxxxxxxxxxx"
export TOPO_AGENT="agent-1"

# Claim a task with optimistic concurrency
topo status <id> doing --if todo --assign "$TOPO_AGENT"
```

## AI and Automation

### Atomic Batch Mutations (`topo apply`)

Mutate nodes and edges atomically. If any operation fails or introduces a cycle, the entire transaction aborts.

```bash
topo apply --json <<'EOF'
[
  {"op": "add", "ref": "m", "title": "v2.0 Release", "kind": "milestone"},
  {"op": "add", "ref": "spec", "title": "API Specification", "in": ["$m"]},
  {"op": "add", "ref": "impl", "title": "Implementation", "depends_on": ["$spec"], "in": ["$m"]},
  {"op": "status", "id": "$spec", "status": "doing"}
]
EOF
```

### Graph Optimization Models (`topo organize`)

Evaluate and optimize graph structure using local or remote decision models:

```bash
topo organize deps --json        # Infer missing prerequisite edges
topo organize place --json       # Suggest milestone placement for tasks in no milestone
topo organize dupes --json       # Detect duplicate tasks
topo organize prioritize --json  # Rank ready tasks by model score
```

Pass `--apply` to `deps`, `place`, or `kinds` to apply the proposals that pass graph validation. Duplicates are only reported, never merged, and `prioritize` only prints a ranking.

### AI Agent Skill

`topo` includes a preconfigured agent skill at `skills/topo/SKILL.md`. AI coding tools like Claude Code and Codex can directly invoke `topo` to decompose goals, track dependencies, and manage workflow state.
