# Canvas CPU Performance Measurements

Benchmarked on 2026-10-02 (macOS arm64, Rust 1.96.0, release build). The frame and grid benchmarks record CPU element tree generation and paint command submission using GPUI's headless testing platform. The selection benchmark times only the derived-data refresh and is not a frame timing.

## Benchmark Commands

Run the test suite benchmarks locally:

```bash
cargo test -p topo-gui --release canvas_frame_benchmark -- --ignored --nocapture
cargo test -p topo-gui --release grid_frame_benchmark -- --ignored --nocapture
cargo test -p topo-gui --release multi_selection_chain_benchmark -- --ignored --nocapture
```

## Derived Graph Data Caching

**Test Fixture**: 500 tasks, 25 parallel chains of 20 levels, 1360 × 860 window, 40 samples per operation.

| Operation | Recomputed Frame Time (ms) (Median / p95) | Cached Frame Time (ms) (Median / p95) | Speedup |
| :--- | :--- | :--- | :--- |
| **Pan** (8 px delta) | 4.212 / 4.558 | 2.563 / 2.710 | **1.64x** |
| **Zoom** (0.9 to 1.0) | 4.252 / 4.549 | 2.489 / 2.739 | **1.71x** |
| **Hover** (25 node cycle) | 4.258 / 4.595 | 2.585 / 2.877 | **1.65x** |

### Invalidation Strategy

- Layout coordinates, prerequisite counts, and critical path metrics are cached in memory
- Graph mutations invalidate structure-derived caches
- Selection changes only invalidate selection-derived highlight overlays
- Viewport culling skips offscreen node card elements and out-of-bounds Bezier edge curves

## Large Selection Chain Traversal

Comparing single shared-adjacency traversal against repeated per-node depth-first searches across long dependency chains. These are selection-refresh times with layout and adjacency construction excluded; the per-node column is a single run, the shared column 40 samples:

| Selected Chain Length | Per-Node Traversal (ms) | Shared Traversal (ms) (Median / p95) | Speedup |
| :--- | :--- | :--- | :--- |
| 500 nodes | 312.112 | 0.178 / 0.197 | **1,750x** |
| 1,000 nodes | 2,177.710 | 0.366 / 0.406 | **5,950x** |

A graph mutation only marks the cache dirty; the direct and reverse adjacency maps are rebuilt on the next refresh. The shared traversal then performs one forward and one backward traversal for the entire selection set.

## Dynamic Grid Density

Grid point evaluations at 1360 × 860 window bounds (0.5 zoom level, 80 samples):

| Configuration | Median CPU Time (ms) | p95 CPU Time (ms) | Quad Count |
| :--- | :--- | :--- | :--- |
| Disabled Grid | 0.005 | 0.012 | 0 |
| 14 px Minimum Spacing (previous) | 5.668 | 6.315 | ~6,076 |
| Dynamic 28 px Minimum Spacing | 0.793 | 0.824 | ~1,519 |

The grid starts at 28 canvas units. Below 100% zoom, spacing doubles until dots are at least 28 screen pixels apart. This cuts command submission overhead fourfold while preserving alignment with canvas coordinates.


## GUI responsiveness follow-up (2026-10-04)

Registered topo work: `a4j6vu`, `49i0c6`, `fnyol1`, `uewggr`, `3norqs`, `2nel5u`, `2vb20n`, `sfy3nl`, `fxc78l`; milestones `9vd6kv`, `7sdzo8`, `pk3uh2`.

Machine: Apple M5, macOS arm64, Rust 1.96.0. Baseline: `1af06254b5819a726cf6b2956b99099ca0a8039b`, with the same new benchmark harness copied into a separate temporary checkout. Both versions use release builds, one test thread, 1360 × 860 logical pixels, 40 frame samples, and 20 save samples. Fixtures include task titles, two tags, an assignee, a PR link, one milestone containing every twentieth task, and 25 parallel dependency lanes. The counts below exclude the extra milestone. Density is the number of incoming edges per task after the first level. The camera stays at 0.9–1.0 zoom with most large-graph nodes outside the viewport.

Acceptance criteria: CPU frame p95 below 16.7 ms for all fixtures; 500-node edit-to-visible-state p95 below 16.7 ms with 120 ms remote write latency; at least 30% less large-graph pan CPU time; zero periodic fetches while inactive/hidden; no unchanged-fetch view notification; bounded additional cache memory (less than 20% peak RSS growth). These are CPU budgets, not claims about measured display FPS.

```bash
cargo test --locked -p topo-gui --release benchmark -- --ignored --nocapture --test-threads=1
```

Frame CPU time, median / p95 in milliseconds:

| Tasks / density | Operation | Before | After |
| --- | --- | --- | --- |
| 100 / 1 | pan | 1.827 / 1.996 | 2.043 / 2.362 |
| 100 / 1 | zoom | 1.824 / 1.925 | 1.933 / 2.049 |
| 100 / 1 | hover | 1.804 / 1.947 | 1.912 / 1.979 |
| 100 / 1 | selection | 1.801 / 1.932 | 1.966 / 2.550 |
| 100 / 1 | search | 1.949 / 2.039 | 2.007 / 2.143 |
| 100 / 1 | input | 1.868 / 2.054 | 1.857 / 2.059 |
| 5000 / 1 | pan | 9.575 / 10.283 | 5.115 / 5.759 |
| 5000 / 1 | zoom | 9.837 / 10.539 | 5.417 / 5.794 |
| 5000 / 1 | hover | 9.689 / 13.510 | 5.173 / 5.412 |
| 5000 / 1 | selection | 7.179 / 7.434 | 5.315 / 5.726 |
| 5000 / 1 | search | 7.936 / 8.192 | 5.762 / 6.017 |
| 5000 / 1 | input | 7.275 / 7.880 | 5.174 / 5.450 |
| 1000 / 12 | pan | 8.662 / 9.207 | 4.983 / 5.261 |
| 1000 / 12 | zoom | 9.268 / 10.021 | 5.710 / 5.946 |
| 1000 / 12 | hover | 8.653 / 9.382 | 4.936 / 5.086 |
| 1000 / 12 | selection | 9.891 / 14.853 | 5.936 / 6.133 |
| 1000 / 12 | search | 8.150 / 8.578 | 5.505 / 5.635 |
| 1000 / 12 | input | 9.072 / 9.395 | 5.094 / 5.553 |

Small fixtures remain around 2 ms; some small-graph samples increased by approximately 0.2 ms as indexing and shared metrics add fixed overhead. Large-graph pan improved by 47%, while the dense graph also improved across camera, selection, search and input workloads.

With 120 ms remote write latency, edit-to-visible-state median / p95 changed from **140.398 / 146.198 ms** to **8.387 / 9.302 ms**. Edit-to-settled changed from **140.400 / 146.206 ms** to **149.486 / 154.571 ms**: persistence includes confirmation and reconciliation, while the visible edit no longer waits for those. Visible-state timing measures the mutation callback and its visible model state; compositor delivery latency is not measured.

The identical headless rendering workload consumed 6.786 s user CPU + 0.945 s system CPU before, versus 3.451 s + 0.832 s after, measured with `getrusage(RUSAGE_CHILDREN)` on the test executable without Cargo compilation. Peak RSS was 476,086,272 bytes before and 490,323,968 bytes after (+3.0%). This peak includes all three headless windows/fixtures, not a single graph's storage alone.

Native GUI idle measurement used the same 100-task local fixture, isolated `TOPO_CONFIG_DIR` directories, a 4 s warm-up and a 10 s sampling interval. Process CPU deltas were measured with `ps -o time=,rss=`. CPU was **0.80% before / 0.30% after** of one core. End RSS was **72,784 / 82,112 KiB**; cached data trades a little memory for less repeated work. These short samples depend on compositor activity and memory reclamation and do not establish long-duration battery impact.

Machine-readable measurements: [gui-2026-10-04.json](performance/gui-2026-10-04.json).

### Cache and drawing boundaries

- Added/removed IDs and dependency/membership changes rebuild layout, column/row lookup, and tiled edge indices. Layout groups each column once rather than scanning all nodes for every column.
- Status/kind changes refresh ready counts, open requirements, milestone progress and shared critical-path lengths. They retain layout and visibility indices. Critical-path predecessors are computed in one topological pass; full paths are materialized only for selected milestones.
- Title, notes, priority, assignee, PR and timestamp changes retain layout and graph metrics. Due/status changes refresh overdue IDs; a date-boundary clock refreshes relative dates without periodic full-view notifications.
- Cards are queried through column/row ranges. Edge control hulls are indexed in 560-unit tiles, followed by the existing precise conservative culling test. Very long edges use a separate fallback list instead of allocating an unbounded number of tile entries.
- Hit testing looks up the pointer's cell. Search highlighting reuses the combobox's existing query results. Toolbar and inspector share cached ready/progress/critical-path data.
- File monitoring waits for an event on a bounded channel, then coalesces changes for 300 ms. It has no repeating 300 ms idle timer. Unchanged reload/fetch responses do not notify the editor.

### Optimistic edits and persistence contract

All graph mutations (create, edit, status, dependency/membership edits, delete, clipboard batches, accepted organizer proposals, undo and redo) are validated locally and displayed immediately. Text-field drafts retain their existing submit/cancel semantics; typing alone is not a graph write. Repository loading/initialization and organizer requests retain their own loading states.

The editor separates the confirmed `Workspace` from ordered pending field/edge patches. One background lane owns file reload/parse/save, remote writes, and snapshot validation/install. Only one write is in flight. Later unsent patches coalesce into batches of at most 128 operations; the in-flight request and its key/body stay immutable. Each patch replays operations in order against the latest confirmed graph, preserving unrelated external edits. Whole-graph rollback is never used to undo a failed remote save.

HTTP retries reuse the exact same idempotency key and body, including when the response was lost after the server committed. Fixed IDs prevent duplicate creations. An acknowledged write retains its minimum server version; stale confirmation snapshots cannot make the pending edit disappear. If the write succeeded but confirmation failed, Retry fetches confirmation without submitting the write again. Server timestamps appear after confirmation, rather than inventing timestamps for an unsaved edit.

Saving/confirmation state appears in a separate 30-pixel row to keep the toolbar usable at narrow widths. Failures pause the queue, preserve the visible draft and later edits, and expose Retry / Discard drafts. Invalid or conflicting pending operations are retained for that explicit resolution. Discard returns to the latest confirmed graph and reloads authoritative state; it does not delete already committed server changes.

Undo/redo during a save enqueue ordered inverse edits. External content changes clear older whole-graph undo history to avoid undoing another writer's work. Repository switching, native window close, and the app's close/quit actions wait until pending changes are saved or explicitly resolved. Each replacement editor owns new tasks and watchers; retired views ignore delayed results. Abrupt process/OS termination is outside this in-memory draft contract; pending changes are not a durable offline journal.

### Verification

- `cargo test --locked --workspace`: **185 passed**, 5 benchmark tests ignored by default; all five benchmarks were separately executed and passed.
- Workspace Clippy, including all targets and `topo-gui/screenshot`, with `-D warnings`; `cargo fmt`; `git diff --check` passed.
- Regression coverage includes immediate visible state before I/O, create/edit/delete/undo in a coalesced batch, local external edits, concurrent remote writers, old polls, stale save confirmations, permission errors, offline failure/discard, response loss/idempotent retry, acknowledged-save fetch failure, server timestamps, and the failure row at 720 × 480.
- Spatial-index results were checked against complete card/Bezier-hull scans across negative/outside viewports and zoom 0.25/0.9/2.5. Crossing edges, arrows, membership and selection are retained.
- Existing inspector, notes, palette, standard text shortcuts, clipboard, drag/zoom, priority/metadata and repository lifecycle tests passed.
- Existing counting-remote tests verify zero requests over 120 s inactive/hidden, exactly one immediate resume request, unchanged-fetch notify count zero, one outage toast, 10/20/40/60 s retry backoff and retired-response rejection.
- Actual Metal offscreen screenshots checked 1360 × 860 and 800 × 640 logical pixels (240-pixel inspector with Japanese text being edited). The saving/failure row uses headless rendered-bound checks; it was not separately captured by Metal.

Windows/Linux hardware, native GPU motion frame timing, live WAN latency and forced-termination recovery were not measured. Remote latency/failure/competition tests use controlled remotes and HTTP mocks rather than mutating production task data for UI experiments.

## Request and server load reduction (2026-10-04)

A remote save now makes one `POST /apply` followed by one conditional confirmation `GET /graph`, instead of a pre-save GET, POST and confirmation GET: **3 → 2 HTTP requests (−33%)**. The server applies operations against its current graph, so a pre-save remote read was redundant. Local saves still reload disk before replaying edits. A confirmed-but-unfetched save retries only its confirmation; an ambiguous response resends the same immutable key/body. Later unsent operations still coalesce into bounded batches.

The poll scheduler skips new requests while persistence is running or ready to save. A poll already in flight can finish; its snapshot remains subject to version ordering. After successive unchanged responses, active-window intervals grow from 5 to 10, 20 and 30 seconds. At steady state this is **12 → 2 polls/minute (−83%)**. Changed responses and activation return to the 5-second interval; activation/visibility restoration fetches immediately when persistence is free. Inactive/hidden windows still make no periodic requests. A quiet active editor can take **up to 30 seconds** to detect another writer's changes, plus request latency. Network failures retain the existing 10–60 second retry backoff.

The API reads membership, version and conditionally selected nodes in one transactional batch, keeping the returned nodes/version and authorization consistent. Conditional node queries gate the indexed workspace lookup with NULL when the version is unchanged; they do not scan or serialize that workspace's nodes. The write read batch similarly gates node access on both writer permission and an unrecorded idempotency key. This eliminates the separate permission round trip while still checking permission before graph validation. Commit authorization, version uniqueness and key uniqueness remain transactionally enforced. Empty node upserts/deletes are omitted; even a no-op write keeps its audit/idempotency record.

Database work below excludes the separate bearer-token lookup common to every request:

| Handler | Before batches / statements | After batches / statements |
| --- | --- | --- |
| Full or changed graph GET | 2 / 5 | 1 / 3 |
| Unchanged graph GET | 1 / 2 | 1 / 3 |
| Edit modifying nodes without deletion | 3 / 9 | 2 / 7 |
| Idempotency replay | 2 / 6, all nodes read | 1 / 5, no nodes read |

An unchanged GET adds one cheap conditional statement to allow changed GETs to use one batch. The lower polling frequency offsets this overhead. Authorization is checked on every request; no permission or graph cache is shared across callers.

Release SQLite benchmark: Apple M5/macOS arm64, Rust 1.96.0, 5,000 nodes, 100 calls per run, five runs, median milliseconds per call. This measures SQL execution and row conversion, excluding HTTP, D1 round-trip latency and application graph validation.

| Workload | Before ms | After ms |
| --- | --- | --- |
| Full graph database read | 4.588 | 4.662 |
| Unchanged graph database read | 0.0210 | 0.0309 |
| Node query during idempotency replay | 4.614 | 0.0216 |

The full local database read remains dominated by converting 5,000 rows; the benefit is fewer database round trips, not a demonstrated CPU speedup for that read. Replay node retrieval is reduced by about 99.5%. At the steady polling rate, the measured unchanged-read work is approximately 0.252 → 0.062 ms per minute per active GUI (−75%). These are local SQLite measurements, not production Worker CPU or D1 billing measurements.

Run `cargo test --release -p topo-server graph_database_load_benchmark -- --ignored --nocapture`. Raw results and request/query counts: [server-load-2026-10-04.json](performance/server-load-2026-10-04.json).

Validation: 188 workspace tests pass; targeted tests count save requests, poll intervals, DB batches/statements and returned node rows, including viewer rejection and permission revocation before full/304 reads. Existing tests cover concurrent commits, membership/token revocation, key retries, conditional writes, stale responses and unsaved drafts. Strict Clippy, formatting and a `wasm32-unknown-unknown` server check pass. Changes remain local; production performance and deployment are not verified by this benchmark.

## Design system migration (2026-10-04)

The theme lookup is resolved once per `graph_view()` call and passed down to the card, edge and grid builders, so a frame reads the thread-local theme once instead of once per element. The migration to the monotone design system leaves frame CPU time unchanged within noise.

Medians in milliseconds on the same machine (Apple M5, macOS arm64, release build), 500 nodes, before → after:

| Operation | Recompute | Before | After |
| :--- | :--- | :--- | :--- |
| Pan | true | 2.47 | 2.43 |
| Zoom | true | 2.41 | 2.29 |
| Hover | true | 2.33 | 2.31 |
| Pan | false | 1.43 | 1.43 |
| Zoom | false | 1.42 | 1.43 |
| Hover | false | 1.43 | 1.43 |

| Grid minimum spacing | Before | After |
| :--- | :--- | :--- |
| 14 px | 1.63 | 1.63 |
| 28 px | 0.23 | 0.23 |


## Large graph rendering follow-up (2026-10-06)

Baseline: `4bbca9c`, using the same benchmark harness in an isolated source snapshot. Builds use Rust 1.96.0, release optimization, one test thread and 1360 × 860 logical pixels. Frame workloads use 40 samples; layout workloads use 10. Task counts in the frame fixtures exclude one additional milestone. The 5000-task independent fixture has no task dependencies and places every twentieth task in that milestone. The all-selected fixture selects every task and the extra milestone.

### Rendering changes

- The overview retains sorted row identities and renders only rows intersecting the viewport plus a small overscan. Priority, due-date, status and graph edits refresh its order and row measurements. All entries remain scrollable; row selection and status actions retain their existing behavior.
- Multiple selection uses a virtual list beneath the selection summary and status controls. Selected IDs and their common status are refreshed when selection or graph metrics change.
- Toolbar counts share immutable ID lists. Search highlighting shares a lookup set built with the search results; measuring a completion list's widest item happens when choices change. Camera and hover updates avoid cloning and sorting these full lists.
- The first canvas frame uses the window as a conservative viewport until canvas bounds become available, preventing an initial construction of every offscreen card.
- Memberships are indexed once during layout, avoiding a full graph scan for each milestone and tag band.
- Dashed edges are flattened with a 0.2-pixel geometric tolerance, then clipped before generating stroke geometry. Hidden segments still advance the dash phase, preserving the pattern when the viewport moves. The renderer retains crossing segments and a stroke/antialiasing margin; arrowheads keep their existing geometry.

### Measurement

The benchmark reports both elapsed time (`PERF`) and, on macOS, rendering-thread CPU time (`PERF_CPU`, via `CLOCK_THREAD_CPUTIME_ID`). Elapsed samples include scheduler stalls; the CPU samples exclude time while the rendering thread is descheduled. File initialization, graph loading and layout are outside the steady-state frame samples. Earlier trials experienced host memory pressure. The reported table is a sequential rerun after the implementation and visual fixes; these measurements do not establish display FPS or input-to-display latency. The saved measurements include elapsed results as well as CPU results.

```bash
cargo test --locked -p topo-gui --release perf_tests:: -- --ignored --nocapture --test-threads=1
# Run one frame fixture in a fresh process:
TOPO_PERF_CASE=5000/0 cargo test --locked -p topo-gui --release responsiveness_benchmark -- --ignored --nocapture --test-threads=1
```

Rendering-thread CPU time, median / p95 in milliseconds:

| Workload | Before | After | Median reduction |
| --- | --- | --- | --- |
| nodes=5000 density=0 pan | 179.234 / 274.872 | 6.916 / 7.857 | 96.1% |
| 5000 selected nodes pan | 171.820 / 463.472 | 2.379 / 2.527 | 98.6% |
| nodes=20000 density=1 pan | 15.492 / 16.773 | 2.051 / 2.269 | 86.8% |
| 10000 nodes 5000 milestones grouped=false layout | 415.408 / 439.612 | 10.796 / 11.260 | 97.4% |

All elapsed and CPU measurements, including the small and dense fixtures: [gui-large-graphs-2026-10-06.json](performance/gui-large-graphs-2026-10-06.json). Results apply to the specified workloads on this host; the artifact retains every measured operation.

Validation: 291 workspace tests pass; strict workspace Clippy with all targets and the screenshot feature, formatting, and diff checks pass. Tests cover scrolling to the last entry, selecting/completing tasks from virtual rows, priority reordering, inspector insets and row widths, and bounded dashed geometry with a stable phase. Native Metal rendering was checked for overview, multiple selection and search across all four palettes (12 captures), plus a 5000-task overview and all-selected view (2 captures). Screenshot rendering required access to macOS font/XPC services outside the shell sandbox.

## Reflow motion (2026-10-09)

Cards, group headers and edges now travel to their new cells on springs when a layout change (a structural edit, hiding completed tasks, grouping) moves them, new cards grow in and removed cards fade out as ghosts. The per-card springs add work to every frame while a change is in flight, and the layout change itself still runs the layout once. Motion rules and caps: [gui/motion.md](gui/motion.md#performance).

Same harness and host as above (Apple M5, macOS arm64, release build, 1360 × 860, one test thread), rendering-thread CPU time, median / p95 in milliseconds. Cases are written nodes / density.

```bash
cargo test --locked -p topo-gui --release benchmark -- --ignored --nocapture --test-threads=1
```

Steady-state frames with this work in place:

| Workload | 5000 / 1 | 1000 / 12 |
| --- | --- | --- |
| Pan | 2.408 / 2.828 | 5.981 / 9.762 |
| Zoom | 2.929 / 3.740 | 7.368 / 9.782 |
| Selection | 4.143 / 4.930 | 10.051 / 11.608 |

The new `reflow_benchmark` measures frames while the cards are travelling and the frame that starts the change:

| Frame | 5000 / 1 | 1000 / 12 |
| --- | --- | --- |
| Animated frames after the change | 2.631 / 3.910 | 6.967 / 10.615 |
| Reflow start (the frame that recomputes the layout) | 23.254 / 26.693 | 24.746 / 27.315 |

The reflow-start rows are six samples each.

**The frame that recomputes the layout exceeds the 16.7 ms budget** (23.3 to 24.7 ms median, 26.7 to 27.3 ms p95). It was already over budget before this work: with the motion code stubbed out the same frame measured about 19.5 to 24.9 ms, so the motion adds roughly 1 to 5 ms to it. The animated frames that follow are within budget (median 2.6 and 7.0 ms, p95 3.9 and 10.6 ms), as are the steady-state frames. Only the single frame in which the layout is recomputed is slow; the benchmark does not establish display FPS or input-to-display latency.
