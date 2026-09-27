---
name: topological-todo
description: Manage tasks and milestones as one dependency graph with the `topo` CLI. Use when the user wants to plan work, add/break down/complete tasks, track milestones, ask "what should I do next", check progress toward a deadline, or visualize dependencies — in any directory that has (or should have) a `.topo` workspace.
---

# topological-todo (`topo`)

Everything is a node in one DAG. A **task** is concrete work; a **milestone** is a node that depends on the work needed to reach it. The only edge is `depends_on` (A depends on B = B must be finished before A). Hierarchy, projects and progress are all derived from edges.

Data lives in `.topo/nodes/<id>.md` (YAML frontmatter + Markdown notes), found by searching upward from the cwd like `.git`. **Never edit these files directly** — always use the CLI so invariants (no cycles, no dangling edges) are enforced.

## Rules

- Always pass `--json` and parse the output. Errors go to stderr with exit code 1 and change nothing.
- Ids are 6-char strings; any unique prefix works.
- Put a task under a milestone with `--for <milestone>` (the milestone depends on the task), not with tags.
- Order work with `--dep <id>` / `topo link <later> <earlier>`; only for real prerequisites.
- Status: `todo` → `doing` → `done` (or `dropped`). Closed nodes stop blocking.

## Orient first

```bash
topo ready --json          # tasks that can start now
topo milestones --json     # progress, `critical_path` (ids of the longest remaining chain), `reached`
topo ls --json             # open nodes (--all, --kind, --status, --under <id>)
topo show <id> --json      # node + `needed_by` + milestone stats
```

If there is no workspace, ask before running `topo init` in the project root.

## Breaking work down: `topo apply`

Generate the plan yourself, then insert it atomically (all or nothing). `$ref` names refer to nodes created earlier in the same batch; plain strings are existing ids.

```bash
topo apply --json <<'EOF'
[
  {"op": "add", "ref": "m", "title": "v1 release", "kind": "milestone", "due": "2026-10-31"},
  {"op": "add", "ref": "design", "title": "Design the API", "for": ["$m"], "tags": ["api"]},
  {"op": "add", "ref": "impl", "title": "Implement the API", "depends_on": ["$design"], "for": ["$m"],
   "notes": "Markdown notes go here"},
  {"op": "link", "from": "$m", "to": "abc123"},
  {"op": "status", "id": "$design", "status": "doing"}
]
EOF
```

Ops: `add` (`ref`, `title`, `kind`, `due`, `tags`, `notes`, `depends_on`, `for`), `link`/`unlink` (`from`, `to`), `status` (`id`, `status`), `edit` (`id`, `title`, `kind`, `due`, `tags`, `notes`), `remove` (`id`). Output: `{"refs": {"m": "<id>", ...}}`.

Single edits: `topo add`, `topo link`, `topo unlink`, `topo status <id> done`, `topo edit <id> --title ...`, `topo rm <id>`.

## Organizing with the local decision model

`topo organize` asks a Jev-compatible System One model (default `http://127.0.0.1:8000`, e.g. local `decider-4b`; configure `[jev] base_url / api_key / model / threshold` in `.topo/config.toml`). It only proposes unless `--apply` is given.

```bash
topo organize deps --json         # missing prerequisites between tasks
topo organize place --json        # which milestone unplaced tasks belong to
topo organize dupes --json        # likely duplicates (never auto-merged)
topo organize kinds --json        # task vs milestone corrections
topo organize prioritize --json   # ready tasks ranked by score (0..1)
```

Review proposals and apply them with `--apply` (strongest first; ones that would form a cycle are reported as skipped), or pick individual ones with `topo link`. If the server is not running, say so and continue without it.

## Showing the graph

`topo graph --format mermaid [--under <id>]` produces a Mermaid flowchart (prerequisite --> dependent) you can show to the user; `--format tree` is a compact text view.

## Typical flow

1. `topo ready --json` and `topo milestones --json` to understand the state.
2. Decompose the user's goal into a milestone + tasks with `topo apply`.
3. `topo organize deps` / `place` to catch missing edges; confirm with the user.
4. As work progresses: `topo status <id> doing|done`, then `topo ready` for what's next.
