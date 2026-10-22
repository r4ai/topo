# Graph query correctness and cycle-check performance

Measured on 2026-10-22 using macOS arm64, Rust 1.96.0 and release builds.
The baseline is commit `8fd73ec611a230d962d3e20ef09edea52bcb0e3a`, with
the same `sparse_link_benchmark` test copied into an isolated source snapshot.
Both builds use the locked dependency versions. The isolated baseline retains
only the core workspace member, so its lockfile is pruned offline.

Each workload starts with 5,000 independent tasks and adds 1,000 dependencies
from different tasks to one shared prerequisite. The prerequisite is either a
leaf task or an additional milestone with one member task. Graph construction,
cloning and final topological validation are outside the timed section.
The table shows elapsed-time medians from five fresh test processes per build.

| Prerequisite | Before | After | Speedup |
| --- | ---: | ---: | ---: |
| Leaf task | 281.475 ms | 0.973 ms | 289x |
| Milestone | 285.641 ms | 23.978 ms | 11.9x |

Raw samples are in [graph-queries-2026-10-22.json](graph-queries-2026-10-22.json).
These are local graph mutation workloads, not GUI frame times or hosted API
latencies. Dense graphs still require visiting reachable edges; reaching a
milestone still requires indexing workspace memberships once per query.
Host load affects elapsed time, so the speedups describe these fixtures.

```sh
cargo test --locked --release -p topo-core sparse_link_benchmark -- --ignored --nocapture
```

Previously, every non-reflexive `reaches` call constructed a complete
requirement map and collected cloned descendant IDs into a sorted set.
The new iterative traversal stops when it finds its target, keeps borrowed IDs
in a visited set, and indexes memberships only when it reaches a milestone.
There is no persistent cache to invalidate after graph mutations.

Missing IDs previously panicked in `descendants` and `critical_path`.
They now return empty results, and `reaches` returns false for distinct IDs
when either node is absent. Reflexive reachability remains true as documented.
Tests cover absent nodes, every node pair in mixed and dense task/milestone
graphs, rejection of membership cycles without mutation, and a 20,000-node
chain without recursive traversal.
