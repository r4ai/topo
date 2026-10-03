---
name: topo
description: Manage tasks and milestones as one dependency graph with the `topo` CLI. Use when the user wants to plan work, add/break down/complete tasks, track milestones, ask "what should I do next", check progress toward a deadline, or visualize dependencies — in any directory that has (or should have) a `.topo` workspace.
---

# topo

Everything is a node in one DAG. A **task** is concrete work; a **milestone** is a *set* of tasks. Two relations connect nodes:

- `depends_on` orders work: A depends on B = B must be finished before A. Milestones can depend on each other (v2 after v1).
- Membership: a task lists the milestones it belongs to (`in`). A task can be in several milestones. Progress counts members only, and a milestone is reached when its members and its own dependencies are closed.

Members' prerequisites count toward a milestone's critical path and `ready --under` scope even if they are not members themselves.

Data lives in `.topo/nodes/<id>.md` (YAML frontmatter + Markdown notes), found by searching upward from the cwd like `.git`. **Never edit these files directly** — always use the CLI so invariants (no cycles, no dangling edges) are enforced.

## Rules

- Use `--json` and parse the output, or `--format ids` / `--format jsonl` when composing pipelines. Command errors go to stderr; validation failures leave the workspace unchanged.
- Ids are 6-char strings; any unique prefix works.
- Put a task in a milestone with `--in <milestone>` / `topo join <task> <milestone>` (remove with `topo leave`), not with tags. Only tasks can be members.
- Order work with `--dep <id>` / `topo link <later> <earlier>`; only for real prerequisites. Don't link a milestone to its own members.
- Status: `todo` → `doing` → `done` (or `dropped`). Closed nodes stop blocking.
- Optional metadata: `priority` (`low`, `medium`, `high`, `urgent`), `assignee` (a person or agent label), and `prs` (related pull requests, given as a URL or `owner/repo#123` and stored as the canonical URL). Priority never changes what is ready; it only ranks.
- `created_at`, `updated_at`, and `completed_at` (UTC) are recorded by topo and cannot be set. `completed_at` exists only while a node is `done`; nodes older than the fields have no times.

## Orient first

```bash
topo ready --json          # tasks that can start now
topo milestones --json     # `members`, `done`/`total`, `critical_path` (ids of the longest remaining chain), `reached`
topo ls --json             # open nodes; filter by kind, status, tags, due date, membership, readiness, title, priority, or assignee
topo show <id> --json      # node (with `milestones`, metadata, and times) + `needed_by` + milestone stats
```

If there is no workspace, ask before running `topo init` in the project root.

## Breaking work down: `topo apply`

Generate the plan yourself, then insert it atomically (all or nothing). `$ref` names refer to nodes created earlier in the same batch; plain strings are existing ids.

```bash
topo apply --json <<'EOF'
[
  {"op": "add", "ref": "m", "title": "v1 release", "kind": "milestone", "due": "2026-10-31"},
  {"op": "add", "ref": "design", "title": "Design the API", "in": ["$m"], "tags": ["api"]},
  {"op": "add", "ref": "impl", "title": "Implement the API", "depends_on": ["$design"], "in": ["$m"],
   "notes": "Markdown notes go here"},
  {"op": "join", "task": "abc123", "milestone": "$m"},
  {"op": "link", "from": "$m", "to": "prev01"},
  {"op": "status", "id": "$design", "status": "doing"}
]
EOF
```

Ops: `add` (`ref`, `title`, `kind`, `due`, `tags`, `priority`, `assignee`, `prs`, `notes`, `depends_on`, `in`), `link`/`unlink` (`from`, `to`), `join`/`leave` (`task`, `milestone`), `status` (`id`, `status`, `if_status`), `edit` (`id`, `title`, `kind`, `due`, `tags`, `priority`, `assignee`, `prs`, `notes`), `remove` (`id`). Output: `{"refs": {"m": "<id>", ...}}`.

In `edit`, an absent `due`, `priority`, or `assignee` is left unchanged and `null` clears it; `prs` replaces the whole list (`[]` clears it). Operations never carry timestamps.

Single edits: `topo add`, `topo link`, `topo unlink`, `topo join`, `topo leave`, `topo status <id> done`, `topo edit <id> --title ...`, `topo rm <id>`.

Metadata flags:

```bash
topo add "Fix login" --priority high --assignee alice --pr owner/repo#12
topo edit <id> --priority urgent            # --no-priority clears it
topo edit <id> --assignee bob               # --no-assignee clears it
topo edit <id> --pr owner/repo#13 --unpr owner/repo#12   # add / remove one link; --unpr fails if it is not linked
topo status <id> doing --assign alice       # status and assignee in one atomic write
```

## Pipelines

`ls`, `ready`, `milestones`, `deps`, `dependents`, `members`, and `critical-path` accept `--format text|ids|tsv|jsonl`. Listings default to text even when piped; traversal commands default to IDs. `--json` keeps the existing formatted array and cannot be combined with an explicit `--format`.

- `ids`: one full ID per line, suitable for the next `topo` command.
- `tsv`: no header; columns are `id`, `kind`, `status`, `due`, `title`. Missing due dates are empty. Backslashes, tabs, newlines, and carriage returns in titles are escaped as `\\`, `\t`, `\n`, and `\r`.
- `jsonl`: one compact JSON object per line, using the corresponding `--json` item's schema (milestone listings retain progress and critical-path fields).

`topo ls -` reads whitespace-separated IDs from stdin and filters that subset. Unique prefixes are accepted and duplicates removed; empty input produces no nodes. The default open-only filter still applies; pass `--all` to include closed nodes. Filters combine with AND:

- Repeat `--tag <tag>` or `--in <milestone>` to require every tag or membership. `--under <id>` includes transitive prerequisites, while `--in` tests direct membership.
- `--due-before <date>` and `--due-after <date>` are inclusive and omit nodes without a due date. `--no-due` cannot be combined with either.
- `--ready` selects open nodes with closed prerequisites; `--blocked` selects open nodes whose prerequisites remain open. These flags are mutually exclusive.
- `--title <text>` matches a case-insensitive substring.
- Repeat `--priority <p>` to select nodes with any of the priorities; `--no-priority` selects those without one and cannot be combined with `--priority`. `--assignee <name>` selects that assignee's nodes and `--unassigned` those without one; the two are mutually exclusive.

`ls` and `ready` take `--sort priority`: highest first, nodes without a priority last, otherwise in the usual order. `topo ready --sort priority --format ids | head -1` is the most important task that can start now.

For `status`, `edit`, `rm`, `join`, `leave`, `link`, and `unlink`, `-` replaces the first positional ID only. Resolve and validate all input IDs and graph changes before saving; a validation failure persists no changes, and empty input is a no-op. With `--json`, stdin batches return an array even for one result (or `[]` for none); an explicit single ID retains its object result.

```bash
topo ready --format ids | topo status - doing
topo ls --tag gui --format ids | topo ls - --due-before 2026-10-31 --format ids | topo join - abc123
topo deps abc123 --transitive | topo ls - --blocked --format ids
topo ready --format jsonl | jq -r 'select(.status == "todo") | .id'
```

Graph traversal commands return nodes in the selected output format (`--json` returns an array):

| Command | Scope |
| --- | --- |
| `topo deps <id> [--transitive]` | Direct prerequisites, including milestone members; `--transitive` follows all requirements. |
| `topo dependents <id> [--transitive]` | Direct dependency edges; `--transitive` also follows task membership toward milestones. |
| `topo members <milestone>` | Direct member tasks; the ID must resolve to a milestone. |
| `topo critical-path <id>` | Longest remaining task chain in execution order, for any node. |

## Organizing with the local decision model

`topo organize` asks a Jev-compatible System One model (default `http://127.0.0.1:8000`, e.g. local `decider-4b`; configure `[jev] base_url / api_key / model / threshold` in `.topo/config.toml`). It only proposes unless `--apply` is given.

```bash
topo organize deps --json         # missing prerequisites between tasks
topo organize place --json        # which milestone tasks in no milestone belong to
topo organize dupes --json        # likely duplicates (never auto-merged)
topo organize kinds --json        # task vs milestone corrections
topo organize prioritize --json   # ready tasks ranked by score (0..1)
```

`organize prioritize` returns model recommendation scores and leaves stored `priority` values unchanged, including with `--apply`. Use `edit --priority` to save a choice; `ready --sort priority` ranks by those saved values.

Review proposals and apply them with `--apply` (strongest first; ones that would form a cycle are reported as skipped), or pick individual ones with `topo link`. If the server is not running, say so and continue without it.

## Sharing a workspace with other agents

When `.topo/config.toml` has a `[cloud]` table, the graph lives on a server and other agents may be changing it while you work. The commands are the same; `TOPO_TOKEN` in the environment authenticates you only when `TOPO_CLOUD_URL` explicitly matches the trusted server. Set both outside the repository so a workspace cannot redirect the token.

- Claim a task before working on it: `topo status <id> doing --if todo --assign "$TOPO_AGENT"`. The status and the assignee change in one atomic write, so it fails without touching the assignee if another agent claimed the task first; pick the next one from `topo ready`. A plain `topo status <id> doing` would succeed for both of you. `topo ls --assignee "$TOPO_AGENT"` lists what you hold, and `topo ls --ready --unassigned` what nobody does.
- Don't cache the graph across steps: re-run `topo ready` after each task, since other agents close and add nodes.
- `topo cloud log` shows who changed what. Set `TOPO_AGENT` to a label of your own so your writes are attributed to you.
- There are no node files in a linked workspace, so don't read or edit `.topo/nodes`; use `topo show <id>` and `topo edit <id> --note ...`.

## Showing the graph

`topo graph --format mermaid [--under <id>]` produces a Mermaid flowchart (prerequisite --> dependent, member -.-> milestone) you can show to the user; `--format tree` is a compact text view.

## Typical flow

1. `topo ready --json` and `topo milestones --json` to understand the state.
2. Decompose the user's goal into a milestone + tasks with `topo apply`.
3. `topo organize deps` / `place` to catch missing edges; confirm with the user.
4. As work progresses: `topo status <id> doing|done`, then `topo ready` for what's next.
