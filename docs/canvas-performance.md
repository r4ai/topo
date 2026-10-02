# Canvas CPU frame measurements

Measured on 2026-10-02, macOS 26.6.2 / arm64, Rust 1.96.0, optimized release build. GPUI's headless test platform renders the element tree and records paint commands. These measurements include CPU render/paint work and test executor overhead; they exclude GPU execution, presentation, and the operating system's event delivery. They are reproducible comparisons, not claims about a physical display's frame rate.

```sh
cargo test -p topo-gui --release canvas_frame_benchmark -- --ignored --nocapture
cargo test -p topo-gui --release grid_frame_benchmark -- --ignored --nocapture
cargo test -p topo-gui --release multi_selection_chain_benchmark -- --ignored --nocapture
```

## Derived graph data

Fixture: 500 tasks, 25 parallel chains with 20 levels, 1360 × 860 window, 40 samples per operation following warmup. Pan alternates an 8 px camera movement; zoom alternates 0.9 and 1.0; hover cycles through 25 nodes. The comparison forces invalidation each frame versus retaining the cache. Both variants use viewport culling and the optimized grid, so this isolates recurring derived-data work rather than reproducing the entire old renderer.

| Operation | Recompute median / p95 (ms) | Cached median / p95 (ms) |
| --- | --- | --- |
| Pan | 4.212 / 4.558 | 2.563 / 2.710 |
| Zoom | 4.252 / 4.549 | 2.489 / 2.739 |
| Hover | 4.258 / 4.595 | 2.585 / 2.877 |

The cache stores layout cells, open requirement counts, milestone progress, the union of selected requirement/dependent chains, and selected milestones' critical paths. Graph mutation, reload, and history restore invalidate graph-derived data; selection changes only update selection-derived data. Fit, reveal, navigation, and link drop share the cached layout. Cards outside the viewport do not build elements; edges whose Bezier control hull misses the viewport do not paint. Crossing edges remain visible even when both endpoints are outside.

## Large selections

Selecting every task in a single dependency chain previously traversed the graph separately for each selected node. The cache now builds direct requirement and reverse dependent adjacency on graph changes, then traverses each direction once from all selected nodes. The directions keep separate visited sets so a shared requirement or dependent does not pull unrelated siblings into the highlighted chains.

The deterministic selection-only release benchmark excludes initial graph/layout/adjacency construction. It compares one execution of the previous union of per-node traversals with 40 selection refresh samples using the new adjacency. Every sample checks the resulting focus against the previous implementation; there are no timing assertions.

| Selected chain length | Previous traversal (ms) | Shared traversal median / p95 (ms) |
| --- | --- | --- |
| 500 | 312.112 | 0.178 / 0.197 |
| 1,000 | 2,177.710 | 0.366 / 0.406 |

These are selection calculation times, not whole frame timings. Selected milestones also cache each critical path's exact consecutive and terminal edges, so multiple milestones retain their own terminal highlights without marking unrelated edges between nodes from different paths.

## Dot grid

Isolated grid view at 1360 × 860 and zoom 0.5, 80 samples per variant:

| Minimum screen spacing | Median (ms) | p95 (ms) |
| --- | --- | --- |
| No grid | 0.005 | 0.012 |
| Previous 14 px | 5.668 | 6.315 |
| Updated 28 px | 0.793 | 0.824 |

At this zoom the old grid generated approximately 6,076 dot quads versus 1,519 with the updated density. The measured paint-command cost justifies thinning the grid at low zoom. The 28-world-unit grid stays unchanged at zoom 1 and above; below that, its spacing doubles as necessary to keep at least 28 screen pixels between dots. This keeps dots aligned with canvas coordinates while reducing paint work fourfold at affected zoom levels.

## Validation and limitations

Cache tests cover status and topology invalidation, a 1,000-node all-selected chain, diamond graphs without sibling leakage, milestone membership in both directions, and exact critical edges for multiple selected milestones. Geometry tests retain crossing edges and reject distant curves. Headless interaction tests cover line-wheel zoom, modified wheel panning, trackpad pixel panning, middle-button panning over cards without changing selection, and unrelated button releases. A resize test verifies previously culled cards appear after viewport growth; the test platform has no display-link callback, so the test models the scheduled display frames explicitly. The production renderer requests another frame when measured canvas bounds change.

The fixture intentionally has no milestone and no current selection, so its timing does not quantify worst-case critical-path or multi-selection costs. Full GPU frame timings on physical hardware remain a separate profiling exercise.
