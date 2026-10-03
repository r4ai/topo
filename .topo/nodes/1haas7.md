---
id: 1haas7
kind: task
title: 権限確認前のグラフ解析・全件取得を防ぐ
status: done
tags:
- security
- import-authorization
- 44dbfab6-0d90-475d-b2a3-4df6534779e5
milestones:
- f0ehc5
---

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
統合した項目: D013, D034, F008, F055, F072
同じ原因の報告と未検証候補を追跡する。報告件数は独立した脆弱性数ではない。

---

## [D013] 確認: Denied full-tenant database reads may consume resource budgets

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D013
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "originalEvidence": "{\"question\":\"Denied tenant-size database reads\",\"disposition\":\"needs_follow_up\",\"evidence\":[\"write.rs:42-50 fetches all nodes before writer permit; graph.rs:192-198 fetches up to 1000 change rows before role.\",\"get graph authorizes before full read at graph.rs:70-77.\"],\"limitations\":\"No data disclosure proven; resource impact depends on actual graph/audit data and deployment budgets.\"}",
    "title": "Denied full-tenant database reads may consume resource budgets",
    "validation": "Source ordering established; concrete security impact unvalidated. Not retained as a finding."
  },
  "candidateId": "candidate-denied-tenant-read-resource-cost",
  "id": "unresolved-server-question-4",
  "originalEvidence": "{\"question\":\"Denied tenant-size database reads\",\"disposition\":\"needs_follow_up\",\"evidence\":[\"write.rs:42-50 fetches all nodes before writer permit; graph.rs:192-198 fetches up to 1000 change rows before role.\",\"get graph authorizes before full read at graph.rs:70-77.\"],\"limitations\":\"No data disclosure proven; resource impact depends on actual graph/audit data and deployment budgets.\"}",
  "paths": [
    "crates/topo-server/src/write.rs",
    "crates/topo-server/src/routes/graph.rs"
  ],
  "reason": "The source was audited, but the following potential impact/control question remains unvalidated in the authorized offline static mode: Denied tenant-size database reads",
  "surfaceIds": [
    "server-question-4"
  ]
}
```

---

## [D034] 確認: Graph import computes unbounded graph analysis before workspace authorization

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D034
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "originalEvidence": "graph import routes/graph.rs 110-112 runs Graph::from_nodes before write_headers/authorization; graph.rs from_nodes 35-46 invokes topo_order; requirements 132-133 scans every graph node through members 127-128; topo_order visit 223-242 recursively descends without depth bound. Compact graph with ~30k independent tasks induces ~900m scans before caller scope/membership checked. Deep sorted dependency chain adds recursive stack exhaustion.",
    "source": "baseline",
    "title": "Graph import computes unbounded graph analysis before workspace authorization"
  },
  "candidateId": "candidate-baseline-import-resource-exhaustion",
  "id": "candidate-baseline-import-resource-exhaustion",
  "paths": [
    "crates/topo-server/src/routes/graph.rs",
    "crates/topo-core/src/graph.rs"
  ],
  "reason": "Awaiting parent validation of request limits, authentication/authorization ordering and graph algorithm complexity."
}
```

---

## [F008] Unauthorized graph imports can consume quadratic Worker CPU

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F008
findingId: csf_6e27cfbde53118af0e168e7d
occurrenceId: occ_2fefede7749bfd4e48d8e168
重要度: medium
共通原因: import-authorization
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

Any valid bearer holder can submit a large compact edgeless graph to a foreign or nonexistent workspace. The import handler performs quadratic graph validation before token workspace scope, membership, writer-role authorization, or required write headers are checked, wasting Worker CPU even when the request is ultimately rejected.

## 根本原因
{
  "evidenceRefs": [
    "import-input",
    "import-order",
    "graph-load",
    "membership-scan",
    "all-node-traversal",
    "late-writer-role",
    "cpu-import",
    "cpu-visits",
    "dedup-0004-5-write-rebuild"
  ],
  "summary": "`import()` constructs `Graph` from the request before invoking `write::write()` and its workspace authorization. `Graph::from_nodes()` traverses every vertex; each `requirements()` call uses `members()` to scan all graph nodes, even when the vertex is a task with no relations. Consequently a flat graph of V compact nodes incurs Θ(V²) checks before unauthorized callers are refused. Required write_headers are likewise parsed only after this construction; neither a missing write header nor later D1 parameter limits protect the earlier CPU work."
}

## 修正方針
Check token workspace scope, current membership, writer role and required write headers before constructing the graph. Apply explicit node/edge/work limits and precompute reverse membership adjacency so validation is O(V+E); enforce per-principal request limits. Use iterative traversal to bound stack usage in addition to total work.

## 検証
{
  "assertions": [
    "Caller authenticates before the handler, so a valid token is required.",
    "Workspace authorization happens only after the caller's graph has been fully built and checked.",
    "No application node/edge/depth cap or validation-work budget is implemented."
  ],
  "counterEvidence": [
    "Unauthenticated callers fail before graph construction.",
    "Unauthorized input is not persisted.",
    "Platform CPU limits and JSON byte limits constrain an individual request.",
    "No measured slowdown, economic loss or cross-invocation outage is asserted.",
    "A valid bearer token is required; this is not unauthenticated.",
    "Platform CPU and body limits bound an individual request but do not make N squared validation cheap.",
    "Documentation describes deployment CPU limits; no live account quota or broad service outage was verified.",
    "Caller extraction still requires a valid unexpired server token; no unauthenticated import path is claimed.",
    "Per-request CPU and byte limits constrain execution; the example is below two MB and practical node limits are absent from source.",
    "Unauthorized graph data is neither returned nor persisted.",
    "The platform may isolate or terminate each invocation; the finding does not assert that one request stops every tenant.",
    "Unauthenticated requests are rejected before handler execution.",
    "Dependency-level JSON byte limits and Cloudflare per-request CPU quotas constrain individual requests; this is not unlimited-body processing.",
    "No service-wide outage or benchmark was observed.",
    "External rate limiting may reduce repeated requests but is not established by repository configuration."
  ],
  "evidenceRefs": [
    "import-input",
    "import-order",
    "graph-load",
    "membership-scan",
    "all-node-traversal",
    "late-writer-role",
    "cpu-import",
    "cpu-visits",
    "dedup-0004-5-write-rebuild"
  ],
  "limitations": [
    "Actual deployment ingress/rate rules and effective CPU budgets were not inspected.",
    "The source documents Worker limits at docs/cloud/deployment.md:99-108; those external limits were not verified live.",
    "Recursive traversal depth is present, but no separate stack-exhaustion threshold was established.",
    "The operation count is source-derived; no timing or production request was measured.",
    "Recursive traversal stack-overflow thresholds remain unresolved and are retained separately.",
    "Actual deployed plan/body limits, external rate limits, measured runtimes and aggregate billing or availability impact were not verified.",
    "Committed deployment IDs are placeholders.",
    "Actual Worker CPU, billing and cross-request availability impact were not measured.",
    "The investigator's dependency-level byte-limit observation is retained as original evidence, not claimed as a repository-owned control.",
    "Source review establishes algorithmic amplification without a runtime reproduction."
  ],
  "method": "semantic reduction of already-validated Standard source findings; no additional validation",
  "operationCount": {
    "fullNodeScans": 3600000000,
    "nodes": 60000,
    "note": "For N edgeless tasks, topo_order calls requirements N times and each members iterator scans N nodes. Serialized-size examples are only data calculations, not executed graph validation."
  },
  "operationCountExamples": [
    {
      "fullNodeScans": 3600000000,
      "nodes": 60000,
      "note": "Retained supplied source-level illustration; no runtime acceptance or measured impact."
    },
    {
      "approximateSerializedBytes": 1350000,
      "fullNodeScans": 2500000000,
      "nodes": 50000,
      "note": "Latest supplied static six-character-ID illustration; no generated payload, benchmark or deployed acceptance."
    }
  ],
  "status": "validated",
  "summary": "Parent verified the unrestricted vector, compact Node defaults, handler order and full-map scan per visited vertex. No expensive graph relationship is needed: a collection of independent tasks still causes a full membership scan for each task. The later D1 parameter limit does not guard this pre-write work. The incoming validated trace also places write_headers after graph construction and supplies a static operation-count illustration: 60,000 edgeless nodes imply 3,600,000,000 full-node checks. This is a data/algorithm calculation, not executed validation, a measured slowdown, or an assertion that that size is accepted under a deployed byte limit. The prior discovery-0002 source independently supplies another static illustration: 50,000 unique minimal six-character-ID isolated nodes occupy roughly 1.35 MB of JSON and incur 2.5 billion filter visits. Neither illustration was executed or proves acceptance under an effective deployed body/CPU limit. The assigned report independently retains missing required Idempotency-Key rejection only after graph construction. It attributes a byte-limit observation to dependency-level investigator evidence rather than a repository-owned control, and rates amplification severity low with unknown practical impact; existing medium/high-likelihood ratings are not adjudicated."
}

## 回帰確認
[
  "Unauthorized or unrelated scoped tokens must be rejected before graph validation.",
  "Validate processing-work bounds for large flat graphs and deep chains.",
  "Oversized node/edge collections must fail at explicit limits.",
  "Reject an unrelated/viewer/scoped-other-workspace import before Graph::from_nodes is reached.",
  "Verify edgeless and sparse imports perform O(N+E) bounded work, and oversize graphs fail before expensive traversal.",
  "Send an oversized-node import using a viewer or unrelated scoped token and assert authorization rejects it before graph construction.",
  "Verify configured node/edge budgets reject excessive input and iterative cycle validation scales with vertices plus edges.",
  "Retain cycle, reference and safe-ID rejection behavior.",
  "A valid token denied target-workspace access never invokes full graph construction.",
  "Disconnected graph validation scales linearly after indexing membership.",
  "Configured node/edge limits reject excess complexity before traversal."
]

## 場所
[
  {
    "endLine": 118,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "root_control",
    "startLine": 103
  },
  {
    "endLine": 134,
    "path": "crates/topo-core/src/graph.rs",
    "role": "sink",
    "startLine": 127
  },
  {
    "endLine": 251,
    "path": "crates/topo-core/src/graph.rs",
    "role": "sink",
    "startLine": 238
  },
  {
    "endLine": 55,
    "path": "crates/topo-core/src/wire.rs",
    "role": "user_input",
    "startLine": 50
  },
  {
    "endLine": 50,
    "path": "crates/topo-server/src/write.rs",
    "role": "expected_control",
    "startLine": 42
  },
  {
    "endLine": 116,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "entrypoint",
    "startLine": 103
  },
  {
    "endLine": 47,
    "path": "crates/topo-core/src/graph.rs",
    "role": "propagation",
    "startLine": 35
  },
  {
    "endLine": 134,
    "path": "crates/topo-core/src/graph.rs",
    "role": "root_control",
    "startLine": 126
  },
  {
    "endLine": 252,
    "path": "crates/topo-core/src/graph.rs",
    "role": "sink",
    "startLine": 238
  },
  {
    "endLine": 118,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "propagation",
    "startLine": 103
  },
  {
    "endLine": 46,
    "path": "crates/topo-core/src/graph.rs",
    "role": "propagation",
    "startLine": 35
  },
  {
    "endLine": 114,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "root_control",
    "startLine": 110
  },
  {
    "endLine": 134,
    "path": "crates/topo-core/src/graph.rs",
    "role": "propagation",
    "startLine": 126
  },
  {
    "endLine": 62,
    "path": "crates/topo-server/src/write.rs",
    "role": "propagation",
    "startLine": 50
  },
  {
    "endLine": 47,
    "path": "crates/topo-core/src/graph.rs",
    "role": "outcome",
    "startLine": 35
  }
]

---

## [F055] A graph import can consume disproportionate API CPU before permission checks

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F055
findingId: csf_5f7110d0fc843d75bf20e476
occurrenceId: occ_5dc33101c7d3013eba3d8319
重要度: low
共通原因: import-authorization
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

Any authenticated caller can send a compact graph of many independent tasks to the import route, even for a workspace they cannot write. Before checking write headers or workspace permissions, the server scans all nodes for every node during validation, causing quadratic CPU work.

## 根本原因
{
  "evidenceRefs": [
    "resource-import",
    "resource-full-graph",
    "resource-topo-visits",
    "resource-quadratic",
    "resource-authorization",
    "resource-auth"
  ],
  "summary": "The import handler builds the submitted `Graph` before validating required write headers or calling `write::write()` for target permissions. `Graph::from_nodes()` runs topological validation. For each newly visited node, `requirements()` calls `members()`, which scans every graph node even for tasks with empty membership. A graph of independent tasks therefore causes quadratic repeated scanning without a node/edge complexity limit."
}

## 修正方針
Check required write headers and target workspace write permission before expensive graph construction. Bound node and edge counts, and precompute milestone membership adjacency once so graph validation scales with nodes plus edges.

## 検証
{
  "counterEvidence": [
    "`Caller` rejects absent or invalid credentials first (auth.rs:25-33).",
    "JSON body, Worker CPU and account quotas constrain individual requests; docs/cloud/deployment.md:99-108 describes request CPU and database limits.",
    "No runtime reproduction, actual CPU timing, deployment rate limit or billing configuration was checked.",
    "The later D1 parameter size limit is a persistence constraint and does not bound algorithmic amplification before validation."
  ],
  "evidenceRefs": [
    "resource-import",
    "resource-full-graph",
    "resource-quadratic",
    "resource-authorization",
    "resource-auth",
    "resource-topo-visits"
  ],
  "limitations": [
    "No claim of unauthenticated access or guaranteed outage of other Worker isolates.",
    "A distinct deep-recursion consequence remains deferred rather than merged into this algorithmic finding."
  ],
  "method": "static complexity and source-flow analysis",
  "summary": "For N unique independent tasks, topological traversal visits each node once and its requirements lookup examines all N nodes. No duplicate IDs, dependency edges or invalid references are needed. This computation occurs before header validation and before target membership/role enforcement, so an authenticated nonmember can trigger it even when persistence is later rejected."
}

## 回帰確認
[
  "A nonmember import must reject before invoking full graph validation.",
  "Independent-task graph validation should use linear membership indexing and reject inputs beyond explicit complexity limits."
]

## 場所
[
  {
    "endLine": 118,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "entrypoint",
    "startLine": 103
  },
  {
    "endLine": 47,
    "path": "crates/topo-core/src/graph.rs",
    "role": "propagation",
    "startLine": 35
  },
  {
    "endLine": 134,
    "path": "crates/topo-core/src/graph.rs",
    "role": "root_control",
    "startLine": 126
  },
  {
    "endLine": 59,
    "path": "crates/topo-server/src/write.rs",
    "role": "expected_control",
    "startLine": 48
  },
  {
    "endLine": 33,
    "path": "crates/topo-server/src/auth.rs",
    "role": "expected_control",
    "startLine": 25
  },
  {
    "endLine": 251,
    "path": "crates/topo-core/src/graph.rs",
    "role": "propagation",
    "startLine": 238
  }
]

---

## [F072] Graph imports can exhaust shared CPU before workspace authorization

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F072
findingId: csf_f2f92573af4013bc7bf219ca
occurrenceId: occ_0690c46e9252994efc4951a1
重要度: medium
共通原因: import-authorization
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

Any authenticated API user can submit a compact graph to `PUT /v1/workspaces/{wid}/graph`, even for a workspace they cannot access. The server computes an unbounded recursive, quadratic graph traversal before checking membership or required write headers, consuming Worker CPU and causing request failures.

## 根本原因
{
  "evidenceRefs": [
    "import-before-authorization",
    "construction-topo-traversal",
    "quadratic-members-scan",
    "recursive-unbounded-visit",
    "late-workspace-role"
  ],
  "summary": "`ImportRequest.nodes` is an unbounded vector. The import handler validates IDs and constructs the graph before authorization. Construction invokes `topo_order()`, whose per-node `requirements()` scans all nodes through `members()` and recursively traverses dependencies, creating quadratic work and unbounded call depth without a semantic resource budget."
}

## 修正方針
Authorize workspace access before graph construction; bound node count, edge count and traversal depth/work for all imported or reconstructed graphs, and use indexed milestone membership with iterative linear-time traversal.

## 検証
{
  "counterEvidence": [
    "The endpoint is authenticated (auth.rs:25-33) and Axum JSON extraction may impose a byte limit; compact node counts within ordinary bounded JSON still yield quadratic work.",
    "Worker CPU/stack/request isolation constrains a single request; global outage is not established.",
    "Graph apply and stored graph reconstruction are authorized before traversal (write.rs:50-60), narrowing the nonmember path to import; they share the same costly algorithm for authorized graphs."
  ],
  "evidenceRefs": [
    "import-before-authorization",
    "construction-topo-traversal",
    "quadratic-members-scan",
    "recursive-unbounded-visit",
    "late-workspace-role"
  ],
  "limitations": [
    "Exact deployed body, CPU, rate and account-admission limits are unknown.",
    "No runtime crash, latency or shared-service outage was reproduced."
  ],
  "method": "independent static source trace and complexity analysis",
  "status": "validated",
  "summary": "Verified Caller runs before Json, so a valid bearer token is required. Workspace membership occurs in write::write after Graph::from_nodes. Node fields default in model.rs:80-93 and WireNode flattens Node (wire.rs:13-18), allowing compact independent tasks; 30,000 such tasks cause approximately 900 million member checks. Deep acyclic chains also reach recursive visit. No application code or trigger input was executed."
}

## 回帰確認
[
  "Reject unauthorized imports before graph analysis.",
  "Verify maximum supported node/edge/depth bounds and that independent tasks scale linearly.",
  "Verify long acyclic dependency chains return a controlled limit error instead of exhausting the call stack."
]

## 場所
[
  {
    "endLine": 118,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "entrypoint",
    "startLine": 103
  },
  {
    "endLine": 46,
    "path": "crates/topo-core/src/graph.rs",
    "role": "propagation",
    "startLine": 35
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/graph.rs",
    "role": "root_control",
    "startLine": 127
  },
  {
    "endLine": 251,
    "path": "crates/topo-core/src/graph.rs",
    "role": "sink",
    "startLine": 223
  },
  {
    "endLine": 50,
    "path": "crates/topo-server/src/write.rs",
    "role": "expected_control",
    "startLine": 42
  },
  {
    "endLine": 60,
    "path": "crates/topo-server/src/write.rs",
    "role": "affected_operation",
    "startLine": 59
  },
  {
    "endLine": 212,
    "path": "crates/topo-core/src/graph.rs",
    "role": "supporting_operation",
    "startLine": 193
  }
]


## 対応結果（2026-10-03）
結果: fixed。importのGraph構築を認可済みwriteの中へ移動。非member404/viewer403が不正グラフ解析より先になることと、正当importをAPIテストで確認。
最終検証: cargo test --locked --workspace 116成功/既存3ignore、fmt、native/Worker clippy -D warnings、local Worker/D1 smoke成功。詳細: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/artifacts-6a4e5a1e5a63564edc8ca8693b4bf5cc93fdb5baaaf0c17a2de44b08336f148b/artifacts/report_fix_20261003.md
