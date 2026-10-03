---
id: 0mrpqe
kind: task
title: 深いグラフの再帰と過大な計算・メモリ使用を解消する
status: done
tags:
- security
- graph-scalability
- 44dbfab6-0d90-475d-b2a3-4df6534779e5
milestones:
- f0ehc5
---

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
統合した項目: D001, D015, D017, D020, D027, D031, F005, F037, F038
同じ原因の報告と未検証候補を追跡する。報告件数は独立した脆弱性数ではない。

---

## [D001] 確認: Unbounded recursive traversal is source-confirmed, but the minimum accepted graph depth and whether deployment behavior creates securit

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D001
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "candidateId": "candidate-recursive-traversal-depth",
    "counterEvidence": [
      "Per-request Worker CPU limits may abort graph validation before stack exhaustion.",
      "Effects limited to a single request or local process have not been measured."
    ],
    "originalCandidate": {
      "additionalWorkerEvidence": "Quadratic import validation: 60,000 valid id/title-only task nodes serialize to 1,740,011 bytes. topo_order -> requirements -> members scans all 60,000 graph nodes for each of 60,000 visits (3.6 billion iterations), even with zero edges. import calls Graph::from_nodes at routes/graph.rs:112 before write_headers or write::write membership/WRITERS checks, so any valid server token including unrelated/scoped/viewer can trigger graph validation. No application code was executed; docs document request CPU limits; exact stack-overflow thresholds unreported.",
      "candidateId": "candidate-persistent-graph-exhaustion",
      "counterevidence": [
        "Requires graph-write permission in the affected workspace.",
        "Worker and D1 runtime limits and request body size may constrain reachable depth."
      ],
      "originalBaselineFinding": {
        "attacker": "Any authenticated API user, including a viewer, a user unrelated to the requested workspace, or a token scoped to another workspace.",
        "confidence": "high",
        "counterevidence": [
          "The caller must possess a valid server token.",
          "Request-size and database parameter limits bound serialized bytes, but the demonstrated input is below 2 MB and remains computationally expensive.",
          "Cloudflare per-request CPU limits contain individual execution; docs/cloud/deployment.md:101-105 documents 10 ms and 30 s limits.",
          "No application code was executed and no runtime timing or global outage threshold was measured. The operation count follows directly from the implementation."
        ],
        "cwe": "CWE-407",
        "impact": "A compact request can consume excessive Worker CPU and cause the request to abort at the platform's CPU limit. Repeated authenticated requests drive avoidable account resource consumption without requiring authority over a workspace.",
        "locations": [
          {
            "end_line": 118,
            "file": "crates/topo-server/src/routes/graph.rs",
            "start_line": 103
          },
          {
            "end_line": 50,
            "file": "crates/topo-server/src/write.rs",
            "start_line": 42
          },
          {
            "end_line": 46,
            "file": "crates/topo-core/src/graph.rs",
            "start_line": 35
          },
          {
            "end_line": 134,
            "file": "crates/topo-core/src/graph.rs",
            "start_line": 126
          },
          {
            "end_line": 252,
            "file": "crates/topo-core/src/graph.rs",
            "start_line": 238
          }
        ],
        "recommended_remediation": "Authorize the workspace and writer role before graph construction. Enforce explicit node, edge, and operation budgets. Build a reverse milestone-membership index once and use it during traversal so validation is linear in nodes and edges, and add per-user resource limits appropriate to the deployment.",
        "severity": "medium",
        "source_to_sink": "The import handler constructs Graph::from_nodes before invoking write::write, where membership and writer authorization are enforced. Graph::from_nodes calls topo_order. Each node visit calls requirements, which scans every graph node through members even for a task with no memberships or dependencies. Consequently an edgeless graph causes N squared node scans. A data-only construction of 60,000 valid id/title-only nodes is 1,740,011 JSON bytes and induces approximately 3.6 billion scan iterations before the caller's workspace authorization is checked.",
        "supporting_source_evidence": [
          "import executes let graph = Graph::from_nodes(nodes)? before write_headers and write::write.",
          "Caller authentication occurs through the extractor, but writer authorization only occurs inside write::write after its read batch.",
          "Graph::requirements chains dependencies with self.members(&node.id).",
          "Graph::members iterates self.nodes.values() and filters each node's milestones.",
          "topo_order invokes requirements for every previously unvisited node.",
          "No node-count or computational-work limit appears in the handler or graph constructor."
        ],
        "title": "Quadratic graph import validation executes before workspace authorization",
        "violated_security_invariant": "An unauthorized caller must not trigger expensive workspace operations, and attacker-supplied graph validation must have a bounded computational cost."
      },
      "originalWorkerMessage": "ops::Add only validates existing references then Graph::insert (no topo_order), so one apply can add a long valid chain in reverse lexicographic ID order (a00000 -> a00001 -> ...). It commits without recursive validation. Subsequent server writes call Graph::from_nodes -> topo_order's unbounded recursive visit, and clients install snapshots the same way; even an owner importing an empty replacement first loads/validates the old graph. This can persist a workspace that faults during reads/writes. requirements() also scans all nodes for milestone membership on every node, giving O(N^2) for an edgeless graph. No node/depth budgets are present.",
      "provenance": {
        "source": "baseline"
      },
      "sourceEvidence": [
        {
          "code": "let before = graph(nodes)?;\nlet changed = change(&before)?;",
          "endLine": 60,
          "path": "crates/topo-server/src/write.rs",
          "startLine": 59
        },
        {
          "code": "fn graph(rows: Rows) -> Result<Graph, ApiError> {\n    // Only valid graphs are stored, so a failure here is a defect of the server.\n    Graph::from_nodes(nodes(rows)?).map_err(internal)\n}",
          "endLine": 108,
          "path": "crates/topo-server/src/write.rs",
          "startLine": 106
        },
        {
          "code": "pub fn members<'a>(&'a self, id: &'a NodeId) -> impl Iterator<Item = &'a Node> + 'a {\n    self.nodes.values().filter(move |n| n.milestones.contains(id))\n}\n\n/// What `node` directly requires: its dependencies and, for a milestone, its members.\npub fn requirements<'a>(&'a self, node: &'a Node) -> Vec<&'a NodeId> {\n    node.depends_on.iter().chain(self.members(&node.id).map(|m| &m.id)).collect()\n}",
          "endLine": 133,
          "path": "crates/topo-core/src/graph.rs",
          "startLine": 127
        }
      ],
      "unresolved": "Exact recursion and operation ordering, achievable input limits, and meaningful impact beyond authorized graph edits."
    },
    "retainedEvidence": [
      "crates/topo-core/src/graph.rs:220-252 recursively visits dependency chains.",
      "crates/topo-core/src/graph.rs critical_path recursion; no explicit graph depth bound."
    ],
    "status": "unvalidated"
  },
  "id": "candidate-recursive-traversal-depth",
  "reason": "Unbounded recursive traversal is source-confirmed, but the minimum accepted graph depth and whether deployment behavior creates security impact beyond a CPU-limited request are not established. Application execution is prohibited by the scan's offline mode."
}
```

---

## [D015] 確認: Recursive graph traversal has no explicit depth bound

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D015
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "originalEvidence": "Graph::topo_order and critical_path use recursive traversal (graph.rs:193-212,217-253), without explicit depth limits. Quadratic pre-authorization import is separately validated; concrete depth-only runtime failure threshold remains unmeasured.",
    "title": "Recursive graph traversal has no explicit depth bound",
    "validation": "Recursive source established; failure threshold/security impact unvalidated. Not retained as a finding."
  },
  "candidateId": "candidate-recursive-graph-depth-limit",
  "id": "unresolved-graph-depth-capacity",
  "originalEvidence": "Graph::topo_order and critical_path use recursive traversal (graph.rs:193-212,217-253), without explicit depth limits. Quadratic pre-authorization import is separately validated; concrete depth-only runtime failure threshold remains unmeasured.",
  "paths": [
    "crates/topo-core/src/graph.rs"
  ],
  "reason": "The source was audited, but the following potential impact/control question remains unvalidated in the authorized offline static mode: Recursive graph traversal depth and capacity",
  "surfaceIds": [
    "graph-depth-capacity"
  ]
}
```

---

## [D017] 確認: Deep valid graph may exhaust viewer or Worker stack

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D017
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "additionalEvidence": "Graph::critical_path recursively walks dependencies (graph.rs:193-212), topo_order recurses at 223-251; CLI tree rendering recurses at render.rs:247-268. Shared snapshots hit store.rs:114-118; GUI layout hits layout.rs:13-41.",
    "counterEvidence": "No runtime trigger was created; per-request CPU/body limits may reject graph before recursion reaches a failure threshold.",
    "originalEvidence": "Graph::from_nodes calls topo_order (graph.rs:35-46), whose recursive visit follows one frame per dependency depth without a depth/node bound (223-251). Both local Workspace::open (store.rs:203-213) and remote Workspace::install (114-118) hit it. NodeId validation does not cap graph depth. Runtime thresholds and feasible remote chain construction remain unmeasured.",
    "title": "Deep valid graph may exhaust viewer or Worker stack"
  },
  "candidateId": "graph-recursion-impact",
  "id": "graph-recursion-impact",
  "paths": [
    "crates/topo-core/src/graph.rs",
    "crates/topo-core/src/store.rs",
    "crates/topo-cli/src/render.rs",
    "crates/topo-gui/src/layout.rs"
  ],
  "reason": "Unbounded recursive graph traversal is source-backed, but exact stack exhaustion thresholds and platform consequences are unvalidated in this offline scan. Retained separately from confirmed quadratic work.",
  "surfaceIds": [
    "deep-graph-recursion"
  ]
}
```

---

## [D020] 確認: Assess parser/body limits, exact chain reachability, stored impact beyond attacker own session and concrete stack/resource constraints.

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D020
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "candidateId": "graph-unbounded-recursive-depth",
    "investigatorOriginalEvidence": "Graph::insert graph.rs:265-272 checks references but never topo_order; Op::Add ops.rs:125-133 appends a valid long chain. Server apply routes/graph.rs:150-155 and write.rs:61-76 persist without depth validation. Next write calls write.rs:59 -> graph at 106-108 -> Graph::from_nodes -> recursive visit graph.rs:223-242. Fixed-width descending alphanumeric IDs ensure first visit takes full depth. Clients install snapshot store.rs:117-118 calls same validator; graph GET routes/graph.rs:77-80 returns stored rows. Import validates Graph::from_nodes before write::write authorizes role at routes/graph.rs:110-114. No explicit node/depth/ops bounds; plain axum Json at error.rs:90-93. D1 2 MB batch limit documented deployment.md:105; aggregate graph can grow over batches. Poisoned graph blocks ordinary repair because old graph is reconstructed before change; separate owner-authorized workspace deletion bypasses reconstruction.",
    "locations": [
      {
        "endLine": 242,
        "path": "crates/topo-core/src/graph.rs",
        "startLine": 223
      },
      {
        "endLine": 133,
        "path": "crates/topo-core/src/ops.rs",
        "startLine": 124
      }
    ],
    "originalEvidence": "apply can persist a long dependency chain without topo_order because Op::Add -> Graph::insert only checks references (ops.rs:124-133; graph.rs:264-272). Choose descending lexical ids so the lowest id depends on the next higher id; a batch of thousands of such nodes fits the JSON body limit. Every later write rebuilds Graph::from_nodes -> recursive topo_order::visit (graph.rs:223-242), while cloud clients do the same on install. No node/depth limit or iterative DFS exists. Stored chain can overflow clients/Worker stack or consume quadratic scans since each requirements() calls members() over every node.",
    "source": "baseline preliminary message",
    "unresolved": "Assess parser/body limits, exact chain reachability, stored impact beyond attacker own session and concrete stack/resource constraints."
  },
  "candidateId": "graph-unbounded-recursive-depth",
  "id": "graph-unbounded-recursive-depth",
  "paths": [
    "crates/topo-core/src/graph.rs",
    "crates/topo-core/src/ops.rs",
    "crates/topo-server/src/routes/graph.rs"
  ],
  "reason": "Assess parser/body limits, exact chain reachability, stored impact beyond attacker own session and concrete stack/resource constraints."
}
```

---

## [D027] 確認: Assess parser/body limits, exact chain reachability, stored impact beyond attacker own session and concrete stack/resource constraints.

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D027
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "baselineFinalFinding": {
      "attacker": "A cloud workspace editor or owner who adds a deliberately deep dependency chain, or a repository author who supplies such a local graph.",
      "confidence": "high",
      "counterevidence": [
        "Cycles, dangling references, duplicate references, and malformed IDs are checked; the attack uses a valid acyclic graph.",
        "HTTP request and D1 parameter limits bound each batch, not the accumulated graph or dependency depth across accepted operations.",
        "A platform CPU budget may terminate some attempts earlier, and an owner can delete the entire workspace through the separate workspace route.",
        "The exact crash threshold depends on the target build and platform and was not dynamically measured."
      ],
      "cwe": "CWE-674",
      "impact": "A deep graph can overflow a client or Worker stack, or consume the Worker's CPU budget. Because Add can persist the graph without performing the vulnerable traversal on the resulting graph, the denial can survive across requests. Normal apply and import writes both load the existing graph before changing it, obstructing ordinary repair through graph-write endpoints.",
      "locations": [
        {
          "file": "crates/topo-core/src/graph.rs",
          "line_end": 47,
          "line_start": 35
        },
        {
          "file": "crates/topo-core/src/graph.rs",
          "line_end": 251,
          "line_start": 223
        },
        {
          "file": "crates/topo-core/src/graph.rs",
          "line_end": 272,
          "line_start": 264
        },
        {
          "file": "crates/topo-core/src/ops.rs",
          "line_end": 133,
          "line_start": 124
        },
        {
          "file": "crates/topo-server/src/write.rs",
          "line_end": 75,
          "line_start": 59
        },
        {
          "file": "crates/topo-core/src/store.rs",
          "line_end": 119,
          "line_start": 114
        }
      ],
      "recommended_remediation": "Replace recursive validation with an explicit-stack or iterative topological algorithm. Enforce graph node, edge, and supported-depth limits on the resulting graph before committing every mutation, and bound client rendering/query work. Ensure repair operations can replace an oversized existing graph without first traversing it recursively.",
      "severity": "medium",
      "source_to_sink": "The apply endpoint accepts batches of Add operations. Op::Add calls Graph::insert, which checks references but performs no depth check or topological traversal. An attacker can create nodes in descending lexical order, each depending on the previously created higher ID, so later traversal begins at the lowest ID and recursively visits the entire chain. Subsequent cloud writes reconstruct Graph::from_nodes, which invokes the recursive topo_order::visit without a depth bound. Cloud clients invoke the same reconstruction when installing snapshots.",
      "supporting_source_evidence": [
        "topo_order::visit recursively calls itself once per dependency and has no maximum depth.",
        "Graph::insert accepts valid references without inspecting overall graph depth.",
        "write::write persists changed.graph without calling topo_order on the resulting graph.",
        "Every later write calls graph(nodes), which reconstructs the stored graph through Graph::from_nodes.",
        "requirements() additionally calls members(), which scans all nodes, making validation of long chains incur quadratic scans.",
        "docs/cloud/deployment.md:101-107 documents finite Worker CPU and parameter budgets, but the implementation has no graph-size or depth invariant."
      ],
      "title": "Unbounded dependency depth can persist graphs that exhaust client and server stacks",
      "violated_security_invariant": "Accepted graph content must remain safely loadable and editable by other workspace members without exhausting finite call stacks."
    },
    "candidateId": "graph-unbounded-recursive-depth",
    "investigatorOriginalEvidence": "Graph::insert graph.rs:265-272 checks references but never topo_order; Op::Add ops.rs:125-133 appends a valid long chain. Server apply routes/graph.rs:150-155 and write.rs:61-76 persist without depth validation. Next write calls write.rs:59 -> graph at 106-108 -> Graph::from_nodes -> recursive visit graph.rs:223-242. Fixed-width descending alphanumeric IDs ensure first visit takes full depth. Clients install snapshot store.rs:117-118 calls same validator; graph GET routes/graph.rs:77-80 returns stored rows. Import validates Graph::from_nodes before write::write authorizes role at routes/graph.rs:110-114. No explicit node/depth/ops bounds; plain axum Json at error.rs:90-93. D1 2 MB batch limit documented deployment.md:105; aggregate graph can grow over batches. Poisoned graph blocks ordinary repair because old graph is reconstructed before change; separate owner-authorized workspace deletion bypasses reconstruction.",
    "locations": [
      {
        "endLine": 242,
        "path": "crates/topo-core/src/graph.rs",
        "startLine": 223
      },
      {
        "endLine": 133,
        "path": "crates/topo-core/src/ops.rs",
        "startLine": 124
      }
    ],
    "originalEvidence": "apply can persist a long dependency chain without topo_order because Op::Add -> Graph::insert only checks references (ops.rs:124-133; graph.rs:264-272). Choose descending lexical ids so the lowest id depends on the next higher id; a batch of thousands of such nodes fits the JSON body limit. Every later write rebuilds Graph::from_nodes -> recursive topo_order::visit (graph.rs:223-242), while cloud clients do the same on install. No node/depth limit or iterative DFS exists. Stored chain can overflow clients/Worker stack or consume quadratic scans since each requirements() calls members() over every node.",
    "source": "baseline preliminary message",
    "unresolved": "Assess parser/body limits, exact chain reachability, stored impact beyond attacker own session and concrete stack/resource constraints."
  },
  "candidateId": "graph-unbounded-recursive-depth",
  "id": "graph-unbounded-recursive-depth-92bbda729a448fa7",
  "paths": [
    "crates/topo-core/src/graph.rs",
    "crates/topo-core/src/ops.rs",
    "crates/topo-server/src/routes/graph.rs"
  ],
  "reason": "Assess parser/body limits, exact chain reachability, stored impact beyond attacker own session and concrete stack/resource constraints."
}
```

---

## [D031] 確認: Assess parser/body limits, exact chain reachability, stored impact beyond attacker own session and concrete stack/resource constraints.

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D031
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "candidateId": "graph-unbounded-recursive-depth",
    "locations": [
      {
        "endLine": 242,
        "path": "crates/topo-core/src/graph.rs",
        "startLine": 223
      },
      {
        "endLine": 133,
        "path": "crates/topo-core/src/ops.rs",
        "startLine": 124
      }
    ],
    "originalEvidence": "apply can persist a long dependency chain without topo_order because Op::Add -> Graph::insert only checks references (ops.rs:124-133; graph.rs:264-272). Choose descending lexical ids so the lowest id depends on the next higher id; a batch of thousands of such nodes fits the JSON body limit. Every later write rebuilds Graph::from_nodes -> recursive topo_order::visit (graph.rs:223-242), while cloud clients do the same on install. No node/depth limit or iterative DFS exists. Stored chain can overflow clients/Worker stack or consume quadratic scans since each requirements() calls members() over every node.",
    "source": "baseline preliminary message",
    "unresolved": "Assess parser/body limits, exact chain reachability, stored impact beyond attacker own session and concrete stack/resource constraints."
  },
  "candidateId": "graph-unbounded-recursive-depth",
  "id": "graph-unbounded-recursive-depth-4fa6c2cc2cc7e34b",
  "paths": [
    "crates/topo-core/src/graph.rs",
    "crates/topo-core/src/ops.rs",
    "crates/topo-server/src/routes/graph.rs"
  ],
  "reason": "Assess parser/body limits, exact chain reachability, stored impact beyond attacker own session and concrete stack/resource constraints."
}
```

---

## [F005] Deep acyclic graphs can exhaust client stack and graph-validation work

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F005
findingId: csf_06b27d0699ef27549e14252c
occurrenceId: occ_67c62675966928f070e2f939
重要度: low
共通原因: graph-invariants
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

Graph validation recursively traverses caller-controlled dependencies without a depth budget and scans all nodes at each step. A supplied or shared deep graph can exhaust a native client's stack; authenticated cloud imports incur this work before workspace authorization.

## 根本原因
{
  "evidenceRefs": [
    "input-graph-validation",
    "recursive-topological-visit",
    "repeated-membership-scan",
    "import-before-membership"
  ],
  "summary": "`Graph::from_nodes()` invokes `topo_order()` on arbitrary caller-supplied nodes. Its `visit()` recursively descends requirements and each requirements call scans all nodes for membership, with no depth/node/edge budget. `critical_path()` separately recurses and clones growing path vectors. Local file loads and remote snapshot installation call this engine, and the cloud import handler does so before workspace-role authorization."
}

## 修正方針
Use iterative graph traversal and precomputed membership adjacency, bound node/edge/depth and operation budgets at input/persistence boundaries, compute critical paths without accumulating quadratic path copies, and authorize cloud imports before expensive validation.

## 検証
{
  "counterEvidence": [
    "Cycles are detected and memo/marks prevent repeated complete traversal, but valid chain depth remains unbounded.",
    "Framework body limits and Worker CPU/request quotas constrain cloud requests; the API investigator did not establish service-wide effects.",
    "Explicit input node IDs are validated, preventing path traversal but imposing no graph-depth limit."
  ],
  "limitations": [
    "Exact stack overflow size and effective framework limits are unverified; no application execution was performed.",
    "No claim of production deployment, cross-request failure or service-wide DoS."
  ],
  "method": "static_source_review",
  "status": "validated",
  "summary": "A valid dependency chain whose first visited node requires the remaining nodes causes linear recursive depth and quadratic full-node scans. Reference and cycle checks do not reject such a chain. Snapshot install and local read_nodes reach this engine, and server import accepts caller input before target membership validation."
}

## 回帰確認
[
  "Verify a deep acyclic chain is rejected by an explicit budget or processed iteratively without stack growth.",
  "Verify nonmember import authorization happens before graph validation and work is bounded."
]

## 場所
[
  {
    "endLine": 251,
    "path": "crates/topo-core/src/graph.rs",
    "role": "root_control",
    "startLine": 223
  },
  {
    "endLine": 134,
    "path": "crates/topo-core/src/graph.rs",
    "role": "root_control",
    "startLine": 127
  },
  {
    "endLine": 212,
    "path": "crates/topo-core/src/graph.rs",
    "role": "sink",
    "startLine": 193
  },
  {
    "endLine": 118,
    "path": "crates/topo-core/src/store.rs",
    "role": "entrypoint",
    "startLine": 114
  },
  {
    "endLine": 114,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "entrypoint",
    "startLine": 110
  }
]

---

## [F037] A deep valid graph can exhaust stack space when a workspace is opened or rewritten

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F037
findingId: csf_bda3f5b5b4f1bc191972d817
occurrenceId: occ_12043e51e52861c639b7779e
重要度: low
共通原因: graph-depth
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

Unbounded recursive topological validation processes attacker-supplied dependency depth. A sufficiently deep valid graph can abort a client during startup and fail server graph writes before corrective operations run.

## 根本原因
{
  "evidenceRefs": [
    "deep-add",
    "deep-store",
    "deep-load",
    "recursive-topo",
    "deep-client",
    "deep-server"
  ],
  "summary": "`Graph::from_nodes` runs `topo_order`, whose `visit` recursively enters each requirement without a depth bound. Acyclicity only prevents repeated cycles, so a valid chain can use a call frame per dependency. Add validates references without bounding or traversing final depth, allowing accepted changes to increase it before persistence. Startup snapshot installation and later server writes invoke this recursive validator; related critical-path and tree traversals also recurse over graph depth."
}

## 修正方針
Use iterative graph validation, critical-path traversal, and tree rendering so graph depth cannot consume the process stack. Enforce graph resource limits before committing data and authorize imports before expensive graph validation.

## 検証
{
  "counterEvidence": [
    "Cycles, dangling/self/duplicate references are rejected, but an arbitrarily deep acyclic chain passes those structural checks.",
    "Persistent cloud poisoning requires owner/editor permissions and a graph accepted under actual request/CPU/database limits.",
    "Worker generated glue (crates/topo-server/build/index.js:1, H/$/O) marks WebAssembly.RuntimeError and reinitializes on a later call; no permanent global Worker-process outage is claimed.",
    "Owner DELETE does not need graph reconstruction and can remove affected workspace."
  ],
  "evidenceRefs": [
    "deep-add",
    "deep-store",
    "deep-load",
    "recursive-topo",
    "deep-client",
    "deep-server"
  ],
  "limitations": [
    "No stack/CPU threshold or runtime crash was measured.",
    "Cloud persistent reachability depends on graph depth being accepted before a later load exceeds the relevant runtime's limits."
  ],
  "method": "independent_static_source_trace",
  "status": "validated",
  "summary": "A graph with the lexicographically earliest node requiring the next node throughout a long acyclic chain causes topo_order's first DFS to descend through the entire chain. Product import/local-file/public parser and cloud snapshot paths reach it. Direct Add may commit increased depth without final traversal, while the next corrective write validates existing nodes first."
}

## 回帰確認
[
  "Process long valid dependency chains without recursion-related failure in native and Worker configurations.",
  "Ensure graph additions cannot commit a graph beyond supported resource limits.",
  "Authorize a workspace import before running costly structural validation."
]

## 場所
[
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 124
  },
  {
    "endLine": 76,
    "path": "crates/topo-server/src/write.rs",
    "role": "propagation",
    "startLine": 59
  },
  {
    "endLine": 47,
    "path": "crates/topo-core/src/graph.rs",
    "role": "propagation",
    "startLine": 35
  },
  {
    "endLine": 253,
    "path": "crates/topo-core/src/graph.rs",
    "role": "root_control",
    "startLine": 223
  },
  {
    "endLine": 120,
    "path": "crates/topo-core/src/store.rs",
    "role": "sink",
    "startLine": 114
  },
  {
    "endLine": 109,
    "path": "crates/topo-server/src/write.rs",
    "role": "sink",
    "startLine": 106
  },
  {
    "endLine": 118,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "entrypoint",
    "startLine": 110
  },
  {
    "endLine": 258,
    "path": "crates/topo-cli/src/render.rs",
    "role": "sink",
    "startLine": 248
  },
  {
    "endLine": 212,
    "path": "crates/topo-core/src/graph.rs",
    "role": "sink",
    "startLine": 193
  }
]

---

## [F038] Milestone rendering retains quadratic amounts of graph data

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F038
findingId: csf_94bc9c169ebfebfe87332539
occurrenceId: occ_cc5e317e3909fe6f04e35d5b
重要度: low
共通原因: graph-resources
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

Critical-path memoization stores a complete owned path for every visited task. A supplied open-task chain therefore amplifies linear graph input into quadratic memory during normal GUI overview and CLI milestone rendering.

## 根本原因
{
  "evidenceRefs": [
    "chain-input",
    "memo-full-paths",
    "overview-consumer",
    "cli-path-consumer"
  ],
  "summary": "Graph::critical_path recursively builds a Vec<NodeId> and clones its whole contents into a HashMap for each visited node. For N open tasks in a chain, these cached prefixes retain N(N+1)/2 independently owned NodeId values. Memoization avoids repeated traversal but does not limit the aggregate retained data. The GUI computes milestone paths while constructing its ordinary overview, and CLI milestone formats use the same implementation."
}

## 修正方針
Memoize path length and a single predecessor per node, then reconstruct only the requested final path. Bound graph resources before persistence and avoid recomputing full paths for overview counts.

## 検証
{
  "counterEvidence": [
    "Closed tasks are excluded, so the chain must remain open.",
    "Inputs must pass ID/reference/cycle validation.",
    "Cloud persistence requires an owner/editor role and actual platform acceptance; the client availability issue is confined to affected workspace consumption.",
    "Both text and JSON milestone formats compute paths; JSON escaping does not change the memory defect."
  ],
  "evidenceRefs": [
    "chain-input",
    "memo-full-paths",
    "overview-consumer",
    "cli-path-consumer"
  ],
  "limitations": [
    "Exact client-memory, stack, cloud-input, and failure thresholds are unmeasured."
  ],
  "method": "independent_static_source_trace",
  "status": "validated",
  "summary": "Traced accepted dependencies to critical_path and its consumers. A valid open-task chain whose final task belongs to a milestone yields path lengths 1 through N in the memo map. Ascending task IDs can make topological validation shallow through already completed marks; the allocation defect is distinct from recursive validation. No application or triggering input was executed."
}

## 回帰確認
[
  "Verify aggregate critical-path storage remains linear for long valid chains.",
  "Check GUI overview counts and returned critical paths preserve semantics."
]

## 場所
[
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 124
  },
  {
    "endLine": 212,
    "path": "crates/topo-core/src/graph.rs",
    "role": "root_control",
    "startLine": 193
  },
  {
    "endLine": 231,
    "path": "crates/topo-gui/src/inspector.rs",
    "role": "sink",
    "startLine": 223
  },
  {
    "endLine": 143,
    "path": "crates/topo-cli/src/render.rs",
    "role": "sink",
    "startLine": 123
  }
]


## 対応結果（2026-10-03）
結果: fixed。隣接情報を一度構築して反復DFS/DPへ変更。critical_pathは長さと先行ノードだけを保持。20,000-node chainの順序/経路/祖先/子孫が成功。treeは全ノードを残し32階層以降の字下げを制限、1,000-node出力上限とJSONL互換性を確認。
最終検証: cargo test --locked --workspace 116成功/既存3ignore、fmt、native/Worker clippy -D warnings、local Worker/D1 smoke成功。詳細: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/artifacts-6a4e5a1e5a63564edc8ca8693b4bf5cc93fdb5baaaf0c17a2de44b08336f148b/artifacts/report_fix_20261003.md
