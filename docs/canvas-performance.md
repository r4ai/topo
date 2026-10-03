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
