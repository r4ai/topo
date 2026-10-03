---
id: 30jxny
kind: task
title: ノード追加時の循環を拒否しDAGの不変条件を守る
status: done
tags:
- security
- graph-invariants
- 44dbfab6-0d90-475d-b2a3-4df6534779e5
milestones:
- f0ehc5
---

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
統合した項目: F004, F022, F027, F030, F035, F045, F049, F052, F068
同じ原因の報告と未検証候補を追跡する。報告件数は独立した脆弱性数ではない。

---

## [F004] A permitted Add operation can persist a cycle and block shared workspace repair

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F004
findingId: csf_8b969f8970542f181b95feaa
occurrenceId: occ_dc7604ce5d9f8771a659c9b8
重要度: low
共通原因: graph-invariants
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

An owner/editor can Add a task that both depends on and belongs to a milestone, causing an unchecked requirement cycle to be persisted. Cloud clients reject the snapshot and subsequent apply/import cannot load the prior graph to repair it. The same insertion control can save invalid local Markdown, which fails later open/reload; manual local file repair remains possible.

## 根本原因
{
  "evidenceRefs": [
    "add-fields",
    "insert-control",
    "implicit-member-edge",
    "http-apply",
    "graph-commit",
    "reload-rejects",
    "from-nodes-check",
    "client-rejects",
    "dedup-0003-1-http-apply",
    "dedup-0003-1-graph-write",
    "dedup-0003-1-graph-commit",
    "dedup-0003-1-later-graph-reject",
    "dedup-0003-1-client-snapshot-reject",
    "dedup-0003-1-local-apply-persistence",
    "dedup-0003-1-local-reopen-validation",
    "dedup-0003-1-join-cycle-check",
    "dedup-0004-2-add-edges",
    "dedup-0004-2-insert-no-cycle-check",
    "dedup-0004-2-write-rebuild",
    "dedup-0004-2-persist-unvalidated",
    "dedup-0004-2-repair-precondition",
    "dedup-0004-2-known-cycle-test"
  ],
  "summary": "Op::Add copies dependency and milestone membership fields into a new node, and Graph::insert checks only duplicate IDs and reference validity. Its assumption that no existing node can require a new task misses the reverse edge created by milestone membership. A task depending on the milestone it joins creates m -> task -> m; longer combined dependency/membership cycles are affected too. The mutation/persistence path trusts insert without final topo_order validation. Subsequent Graph::from_nodes and client snapshot installation enforce acyclicity, rejecting the already-persisted graph before an ordinary edit or replacing import can repair it. Local Workspace::apply likewise accepts the unchecked Add graph and saves Markdown without final acyclicity validation; read_nodes later rebuilds it through Graph::from_nodes and rejects the persisted cycle. Manual local editing can recover that instance, while normal server replacement/import is blocked by old-state reconstruction."
}

## 修正方針
Validate the full requirement relation, including reverse milestone-membership edges, when inserting a node; roll back rejected insertion. Enforce complete graph validity before both local and cloud persistence, and provide an owner-authorized replacement/repair import path for already-invalid stored cloud graphs.

## 検証
{
  "assertions": [
    "No cycle check runs after `Graph::insert()`.",
    "Import also uses the common writer, which loads and rejects the old graph before executing its replacement closure."
  ],
  "counterEvidence": [
    "Authentication and writer-role checks limit exploitation to owners/editors in that workspace.",
    "Separate `link()` and `join()` mutations have cycle controls.",
    "Owner workspace deletion and direct operator database repair remain possible.",
    "Graph::from_nodes rejects this exact pattern in graph.rs:415-419; link/join reject cycles at 286-287 and 313-314, but Add bypasses those guards.",
    "Requires Owner/Editor permission or a scoped token belonging to such a member (write.rs:50). Viewers/nonmembers are denied.",
    "An owner can delete/recreate the workspace; an operator can repair storage directly.",
    "Graph writes require current Owner/Editor membership and enforce scoped-token visibility (write.rs:50; auth.rs:55-71).",
    "Graph::from_nodes and sibling link/join check cycles (graph.rs:35-47,281-290,302-317), but those checks do not protect Add's combined fields.",
    "The attacker already has broad graph edit authority, so the report concerns durable inability to repair rather than ordinary node deletion.",
    "Local node files can be manually repaired;the lasting supported-API recovery failure concerns the server workspace.",
    "Nonmembers and viewers cannot write; Caller role/scope checks precede the mutation.",
    "Link and Join explicitly check reaches and bulk import validates its new Graph::from_nodes.",
    "An editor already has substantial node modification/deletion authority; the additional effect is denying repair through normal graph APIs.",
    "GET graph and the audit log can still return raw stored rows; ordinary clients reject the cyclic snapshot during install.",
    "Owners can delete/recreate the entire workspace; database operators can repair the persisted rows."
  ],
  "evidenceRefs": [
    "add-fields",
    "insert-control",
    "implicit-member-edge",
    "http-apply",
    "graph-commit",
    "reload-rejects",
    "from-nodes-check",
    "client-rejects",
    "dedup-0003-1-http-apply",
    "dedup-0003-1-graph-write",
    "dedup-0003-1-graph-commit",
    "dedup-0003-1-later-graph-reject",
    "dedup-0003-1-client-snapshot-reject",
    "dedup-0003-1-join-cycle-check",
    "dedup-0003-1-local-apply-persistence",
    "dedup-0003-1-local-reopen-validation",
    "dedup-0004-2-add-edges",
    "dedup-0004-2-insert-no-cycle-check",
    "dedup-0004-2-write-rebuild",
    "dedup-0004-2-persist-unvalidated",
    "dedup-0004-2-repair-precondition",
    "dedup-0004-2-known-cycle-test",
    "dedup-0004-2-import-uses-common-write",
    "dedup-0004-2-local-add-entry"
  ],
  "limitations": [
    "No database or runtime reproduction was executed.",
    "This does not expose other tenants or grant account authority.",
    "No application execution or crafted runtime request was performed.",
    "This is persistent same-workspace denial of ordinary writes/opening, not tenant escape or service-wide shutdown.",
    "No live deployment or runtime exploit was tested.",
    "Owner workspace deletion/recreation and direct operator database repair remain possible.",
    "No application code or live request was executed.",
    "The finding is scoped to an already writable workspace, not a cross-tenant or unauthenticated service compromise."
  ],
  "method": "semantic reduction of already-validated Standard source findings; no additional validation",
  "status": "validated",
  "summary": "An existing milestone m, or one added earlier in the same batch, plus task a with depends_on:[m] and in:[m] passes reference checks yet creates m -> a -> m. Apply commits the graph without final acyclicity validation. Future apply and import both validate old stored nodes first, including an otherwise valid replacement import; clients also reject installation. The sources note the exact rejected loading-test pattern and an API example containing this relation. A previously absorbed source explicitly preserves the local Workspace::apply/save and read_nodes reconstruction subcase; manual Markdown repair can restore local access and does not cross an independent remote authorization boundary. The assigned source retains the exact Graph::from_nodes test rejecting a task that belongs to and depends on its own milestone, raw GET graph/history availability despite ordinary client rejection, and an owner-authorized replacement test for already-invalid cloud storage."
}

## 回帰確認
[
  "An add that both depends on and belongs to the same milestone must fail atomically.",
  "Check a multi-hop dependency plus milestone membership cycle.",
  "A rejected batch must leave the cloud graph loadable and unchanged.",
  "Reject adding a task that both depends on and joins the same milestone, including indirect cycles and batch references.",
  "Verify rejected Add leaves the graph/storage unchanged and subsequent writes and opens succeed.",
  "An Add with the same milestone in depends_on and in must fail without changing nodes or version.",
  "Reject longer requirement cycles introduced by new task membership while accepting valid membership additions.",
  "Verify a failed cyclic Add leaves CLI/GUI opens and later graph imports usable.",
  "Add with depends_on=[m] and in=[m] is rejected atomically and does not change the graph version or rows.",
  "Valid graph replacement can repair a stored invalid graph under the appropriate owner authorization."
]

## 場所
[
  {
    "endLine": 272,
    "path": "crates/topo-core/src/graph.rs",
    "role": "root_control",
    "startLine": 265
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "propagation",
    "startLine": 124
  },
  {
    "endLine": 156,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "entrypoint",
    "startLine": 143
  },
  {
    "endLine": 76,
    "path": "crates/topo-server/src/write.rs",
    "role": "sink",
    "startLine": 59
  },
  {
    "endLine": 108,
    "path": "crates/topo-server/src/write.rs",
    "role": "outcome",
    "startLine": 106
  },
  {
    "endLine": 118,
    "path": "crates/topo-core/src/store.rs",
    "role": "outcome",
    "startLine": 116
  },
  {
    "endLine": 423,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 413
  },
  {
    "endLine": 120,
    "path": "crates/topo-core/src/store.rs",
    "role": "outcome",
    "startLine": 114
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 124
  },
  {
    "endLine": 79,
    "path": "crates/topo-server/src/write.rs",
    "role": "sink",
    "startLine": 59
  },
  {
    "endLine": 109,
    "path": "crates/topo-server/src/write.rs",
    "role": "outcome",
    "startLine": 106
  },
  {
    "endLine": 121,
    "path": "crates/topo-core/src/store.rs",
    "role": "outcome",
    "startLine": 114
  },
  {
    "endLine": 188,
    "path": "crates/topo-core/src/store.rs",
    "role": "sink",
    "startLine": 181
  },
  {
    "endLine": 214,
    "path": "crates/topo-core/src/store.rs",
    "role": "outcome",
    "startLine": 203
  },
  {
    "endLine": 134,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 124
  },
  {
    "endLine": 272,
    "path": "crates/topo-core/src/graph.rs",
    "role": "root_control",
    "startLine": 264
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
    "endLine": 76,
    "path": "crates/topo-server/src/write.rs",
    "role": "sink",
    "startLine": 72
  },
  {
    "endLine": 112,
    "path": "crates/topo-server/src/write.rs",
    "role": "outcome",
    "startLine": 106
  },
  {
    "endLine": 47,
    "path": "crates/topo-core/src/graph.rs",
    "role": "outcome",
    "startLine": 35
  },
  {
    "endLine": 419,
    "path": "crates/topo-core/src/graph.rs",
    "role": "expected_control",
    "startLine": 415
  },
  {
    "endLine": 118,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "outcome",
    "startLine": 110
  }
]

---

## [F022] Adding a cyclic task can block normal access and repairs to a shared workspace

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F022
findingId: csf_73c542af1b0795589e2f69b4
occurrenceId: occ_7c9fe1f2b05803e07dc0574f
重要度: low
共通原因: graph-invariants
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

An editor can add a task that both depends on and belongs to one milestone. The server stores that invalid requirement cycle, after which clients cannot install the graph and normal graph writes or imports cannot repair it.

## 根本原因
{
  "evidenceRefs": [
    "add-relations",
    "insert-no-cycle-check",
    "membership-requirement",
    "http-apply",
    "graph-write",
    "graph-commit",
    "later-graph-reject",
    "client-snapshot-reject",
    "local-apply-persistence",
    "local-reopen-validation",
    "join-cycle-check"
  ],
  "summary": "`Op::Add` decodes both relations and inserts them in one step. `Graph::insert()` checks reference existence and uniqueness but skips requirement-cycle detection on the incorrect assumption that new nodes have no incoming requirements. Membership makes the existing milestone require the new task, so depending on that milestone closes a cycle. The API writer commits the resulting graph without revalidation; later writes and imports first rebuild that invalid prior graph and fail before reaching their repair logic. The same shared Add control can save a cyclic graph locally through Workspace::apply();later read_nodes reconstruction rejects it. Local file editing can repair that case,while the server case additionally blocks the supported full-import recovery."
}

## 修正方針
Validate the complete requirement relation before accepting an insertion that adds milestone memberships,roll back invalid insertions,and revalidate the final graph before local or server persistence.

## 検証
{
  "counterEvidence": [
    "Graph writes require current Owner/Editor membership and enforce scoped-token visibility (write.rs:50; auth.rs:55-71).",
    "Graph::from_nodes and sibling link/join check cycles (graph.rs:35-47,281-290,302-317), but those checks do not protect Add's combined fields.",
    "The attacker already has broad graph edit authority, so the report concerns durable inability to repair rather than ordinary node deletion.",
    "Local node files can be manually repaired;the lasting supported-API recovery failure concerns the server workspace."
  ],
  "evidenceRefs": [
    "add-relations",
    "insert-no-cycle-check",
    "membership-requirement",
    "http-apply",
    "graph-write",
    "graph-commit",
    "later-graph-reject",
    "client-snapshot-reject",
    "join-cycle-check",
    "local-apply-persistence",
    "local-reopen-validation"
  ],
  "limitations": [
    "No live deployment or runtime exploit was tested.",
    "Owner workspace deletion/recreation and direct operator database repair remain possible."
  ],
  "method": "Independent static source trace",
  "summary": "With existing milestone `m`, a new task `t` with `depends_on: [m]` and `in: [m]` passes insertion reference checks. Requirement construction yields `t -> m -> t`, while subsequent `Graph::from_nodes()` invokes `topo_order()` and rejects it. The same shared writer is used for full graph import, so importing a valid replacement still fails while reconstructing the poisoned prior state. Local Workspace::apply and read_nodes confirm the same persistence/reload failure locally;manual Markdown repair is possible and is not claimed to cross an independent remote privilege boundary."
}

## 回帰確認
[
  "An Add with the same milestone in depends_on and in must fail without changing nodes or version.",
  "Reject longer requirement cycles introduced by new task membership while accepting valid membership additions.",
  "Verify a failed cyclic Add leaves CLI/GUI opens and later graph imports usable."
]

## 場所
[
  {
    "endLine": 272,
    "path": "crates/topo-core/src/graph.rs",
    "role": "root_control",
    "startLine": 265
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 124
  },
  {
    "endLine": 156,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "entrypoint",
    "startLine": 143
  },
  {
    "endLine": 79,
    "path": "crates/topo-server/src/write.rs",
    "role": "sink",
    "startLine": 59
  },
  {
    "endLine": 109,
    "path": "crates/topo-server/src/write.rs",
    "role": "outcome",
    "startLine": 106
  },
  {
    "endLine": 121,
    "path": "crates/topo-core/src/store.rs",
    "role": "outcome",
    "startLine": 114
  },
  {
    "endLine": 423,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 413
  },
  {
    "endLine": 188,
    "path": "crates/topo-core/src/store.rs",
    "role": "sink",
    "startLine": 181
  },
  {
    "endLine": 214,
    "path": "crates/topo-core/src/store.rs",
    "role": "outcome",
    "startLine": 203
  }
]

---

## [F027] A permitted Add operation can permanently block a shared workspace

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F027
findingId: csf_d1601cef1c00685ea98dadb7
occurrenceId: occ_d440740293df2501a7c246cc
重要度: low
共通原因: graph-invariants
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A writer can add a task that both depends on and belongs to the same milestone. The Add path accepts this cyclic graph and stores it; later edits/imports and client opens reject the stored graph, so members cannot repair it through ordinary workspace operations.

## 根本原因
{
  "evidenceRefs": [
    "add-both-edge-kinds",
    "membership-reverses-edge",
    "insert-omits-cycle-check",
    "changed-graph-stored",
    "old-graph-blocks-repair"
  ],
  "summary": "`Op::Add` copies both dependencies and milestone membership into a node. `Graph::insert()` checks referential validity but assumes a new node cannot close a cycle, overlooking the reverse edge introduced by joining a milestone. `write()` trusts the mutation and persists it without validating the resulting graph. The next write validates the old stored graph first and fails before any repair; clients likewise validate snapshots on opening."
}

## 修正方針
Validate acyclicity for Graph::insert after accounting for both dependency and reverse membership edges, roll back rejected insertion, and enforce final graph validity before every persistence boundary. Provide an owner repair/import path for already invalid stored graphs.

## 検証
{
  "counterEvidence": [
    "Graph::from_nodes rejects this exact pattern in graph.rs:415-419; link/join reject cycles at 286-287 and 313-314, but Add bypasses those guards.",
    "Requires Owner/Editor permission or a scoped token belonging to such a member (write.rs:50). Viewers/nonmembers are denied.",
    "An owner can delete/recreate the workspace; an operator can repair storage directly."
  ],
  "limitations": [
    "No application execution or crafted runtime request was performed.",
    "This is persistent same-workspace denial of ordinary writes/opening, not tenant escape or service-wide shutdown."
  ],
  "method": "static_source_review",
  "status": "validated",
  "summary": "An existing or newly added milestone m and a task a with depends_on:[m] and in:[m] satisfy the reference checks but create m->a->m. The apply route calls ops::apply and commits its resulting nodes. Future apply/import calls graph() before the change closure, and remote install rejects the same cycle."
}

## 回帰確認
[
  "Reject adding a task that both depends on and joins the same milestone, including indirect cycles and batch references.",
  "Verify rejected Add leaves the graph/storage unchanged and subsequent writes and opens succeed."
]

## 場所
[
  {
    "endLine": 272,
    "path": "crates/topo-core/src/graph.rs",
    "role": "root_control",
    "startLine": 265
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "propagation",
    "startLine": 124
  },
  {
    "endLine": 156,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "entrypoint",
    "startLine": 143
  },
  {
    "endLine": 76,
    "path": "crates/topo-server/src/write.rs",
    "role": "sink",
    "startLine": 59
  },
  {
    "endLine": 108,
    "path": "crates/topo-server/src/write.rs",
    "role": "outcome",
    "startLine": 106
  },
  {
    "endLine": 120,
    "path": "crates/topo-core/src/store.rs",
    "role": "outcome",
    "startLine": 114
  }
]

---

## [F030] Adding a task can leave a shared cloud workspace impossible to reopen or repair

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F030
findingId: csf_5d77eb866e81924c00a73dbb
occurrenceId: occ_b268721d25a87a7aacd854f1
重要度: low
共通原因: graph-invariants
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A workspace editor can add a task that both depends on and belongs to one existing milestone. Insertion persists the resulting cycle, after which normal clients and every graph-write route reject the stored graph. An owner must delete the workspace or an operator must repair the database.

## 根本原因
{
  "evidenceRefs": [
    "add-fields",
    "insert-control",
    "implicit-member-edge",
    "http-apply",
    "graph-commit",
    "reload-rejects",
    "client-rejects",
    "from-nodes-check"
  ],
  "summary": "`Op::Add` copies both relations into a node and calls `Graph::insert()`. Insertion checks only references, relying on the incorrect assumption that an existing node cannot require a newly added task. Milestone membership immediately makes the existing milestone require the task, so a dependency back to that milestone closes a cycle. The writer persists this graph without the `topo_order()` validation used on later reloads."
}

## 修正方針
Validate the complete requirement relation when inserting a node with milestone membership, and roll back insertion on a cycle. Validate the resulting graph before persistence and allow an owner-authorized replacement import to repair invalid stored state.

## 検証
{
  "assertions": [
    "No cycle check runs after `Graph::insert()`.",
    "Import also uses the common writer, which loads and rejects the old graph before executing its replacement closure."
  ],
  "counterEvidence": [
    "Authentication and writer-role checks limit exploitation to owners/editors in that workspace.",
    "Separate `link()` and `join()` mutations have cycle controls.",
    "Owner workspace deletion and direct operator database repair remain possible."
  ],
  "evidenceRefs": [
    "add-fields",
    "insert-control",
    "implicit-member-edge",
    "http-apply",
    "graph-commit",
    "reload-rejects",
    "from-nodes-check",
    "client-rejects"
  ],
  "limitations": [
    "No database or runtime reproduction was executed.",
    "This does not expose other tenants or grant account authority."
  ],
  "method": "independent static source trace",
  "summary": "Parent review traced an editor's add operation with both `depends_on:[m]` and `in:[m]`. Both references pass validation, but the combined relation is `m -> task -> m`. The stored graph is then rejected by the next write's mandatory pre-load, including a valid replacement import."
}

## 回帰確認
[
  "An add that both depends on and belongs to the same milestone must fail atomically.",
  "Check a multi-hop dependency plus milestone membership cycle.",
  "A rejected batch must leave the cloud graph loadable and unchanged."
]

## 場所
[
  {
    "endLine": 272,
    "path": "crates/topo-core/src/graph.rs",
    "role": "root_control",
    "startLine": 265
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "propagation",
    "startLine": 124
  },
  {
    "endLine": 156,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "entrypoint",
    "startLine": 143
  },
  {
    "endLine": 76,
    "path": "crates/topo-server/src/write.rs",
    "role": "sink",
    "startLine": 59
  },
  {
    "endLine": 108,
    "path": "crates/topo-server/src/write.rs",
    "role": "outcome",
    "startLine": 106
  },
  {
    "endLine": 118,
    "path": "crates/topo-core/src/store.rs",
    "role": "outcome",
    "startLine": 116
  },
  {
    "endLine": 423,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 413
  }
]

---

## [F035] A shared-workspace writer can insert a cycle that blocks clients and corrective writes

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F035
findingId: csf_040ab623526e65b698365dff
occurrenceId: occ_8b2645c7820dfefc72e8cfbb
重要度: low
共通原因: graph-invariants
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

An editor or scoped writer can add a task that both depends on milestone `m` and belongs to `m`. The server persists that cyclic graph, after which clients reject it and subsequent graph writes, including a valid replacement import, fail before they can repair it.

## 根本原因
{
  "evidenceRefs": [
    "cycle-add-input",
    "milestone-back-edge",
    "insert-missing-cycle-check",
    "cycle-server-entry",
    "cycle-persist",
    "cycle-repair-blocked",
    "cycle-load-consumer",
    "cycle-expected-control"
  ],
  "summary": "An Add operation copies both dependency and milestone-membership edges before calling `Graph::insert`. `requirements` treats each milestone's member task as its requirement, so these fields create opposite edges. `insert` checks reference existence but assumes a new node is required by nothing and skips cycle detection. `write::write` then persists that graph without validating the final state. Every later graph write reconstructs the old graph with `Graph::from_nodes` before its corrective operation, trapping the stored workspace in an invalid state."
}

## 修正方針
Validate the complete requirement graph after inserting any node with milestone membership and before committing changed graph state. Reject the entire operation batch on a cycle, and provide an owner-authorized repair/import path that can replace invalid stored graphs.

## 検証
{
  "counterEvidence": [
    "Graph::from_nodes and individual Link/Join routes reject cycles; the defect is the Add combination path.",
    "GUI remote save uses ops::diff with separately checked link/join operations in some flows, but explicit API apply and Workspace::apply send original Add directly.",
    "Attacker must be an owner/editor or a scoped token whose user can write this workspace.",
    "GET graph still returns stored nodes; owner DELETE remains available, and operator database repair is possible."
  ],
  "evidenceRefs": [
    "cycle-add-input",
    "milestone-back-edge",
    "insert-missing-cycle-check",
    "cycle-server-entry",
    "cycle-persist",
    "cycle-repair-blocked",
    "cycle-load-consumer",
    "cycle-expected-control"
  ],
  "limitations": [
    "Static validation only; no malicious request was executed.",
    "Impact is scoped to the attacker's writable shared workspace."
  ],
  "method": "independent_static_source_trace",
  "status": "validated",
  "summary": "With an existing milestone m, Add of task a with depends_on:[m] and in:[m] passes reference validation but creates a↔m in the requirement graph. The API stores it. Subsequent POST corrective Remove/Leave and PUT valid import both load and reject the old cyclic graph before changing it; standard cloud clients likewise fail during Workspace::install."
}

## 回帰確認
[
  "Reject Add of task a with depends_on:[m] and in:[m] without changing nodes or version.",
  "Reject indirect cycles involving new task memberships and existing milestone dependencies.",
  "Confirm API Add, local Workspace::apply, and all related graph mutation paths enforce the same invariant.",
  "Verify authorized repair can replace a preexisting invalid graph."
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
    "endLine": 134,
    "path": "crates/topo-core/src/graph.rs",
    "role": "propagation",
    "startLine": 131
  },
  {
    "endLine": 272,
    "path": "crates/topo-core/src/graph.rs",
    "role": "root_control",
    "startLine": 264
  },
  {
    "endLine": 156,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "entrypoint",
    "startLine": 143
  },
  {
    "endLine": 76,
    "path": "crates/topo-server/src/write.rs",
    "role": "sink",
    "startLine": 49
  },
  {
    "endLine": 61,
    "path": "crates/topo-server/src/write.rs",
    "role": "outcome",
    "startLine": 59
  },
  {
    "endLine": 109,
    "path": "crates/topo-server/src/write.rs",
    "role": "outcome",
    "startLine": 106
  },
  {
    "endLine": 418,
    "path": "crates/topo-core/src/graph.rs",
    "role": "expected_control",
    "startLine": 415
  },
  {
    "endLine": 120,
    "path": "crates/topo-core/src/store.rs",
    "role": "outcome",
    "startLine": 114
  },
  {
    "endLine": 121,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "outcome",
    "startLine": 115
  }
]

---

## [F045] A permitted Add operation can persist a cycle and block shared workspace repair

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F045
findingId: csf_38d4e3a028200805fd290937
occurrenceId: occ_526a1a68103bd40daab3b00a
重要度: low
共通原因: graph-invariants
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A writer can add a task that both depends on and belongs to the same milestone, creating a requirement cycle that Graph::insert accepts and the cloud writer persists. Clients then reject the snapshot, and subsequent apply or replacement-import operations fail while loading the old graph before any repair can run.

## 根本原因
{
  "evidenceRefs": [
    "add-fields",
    "insert-control",
    "implicit-member-edge",
    "http-apply",
    "graph-commit",
    "reload-rejects",
    "from-nodes-check",
    "client-rejects"
  ],
  "summary": "Op::Add copies dependency and milestone membership fields into a new node, and Graph::insert checks only duplicate IDs and reference validity. Its assumption that no existing node can require a new task misses the reverse edge created by milestone membership. A task depending on the milestone it joins creates m -> task -> m; longer combined dependency/membership cycles are affected too. The mutation/persistence path trusts insert without final topo_order validation. Subsequent Graph::from_nodes and client snapshot installation enforce acyclicity, rejecting the already-persisted graph before an ordinary edit or replacing import can repair it."
}

## 修正方針
Validate the full requirement relation, including reverse milestone-membership edges, when inserting a node; roll back rejected insertion. Enforce complete graph validity before every persistence boundary and provide an owner-authorized replacement/repair import path for already-invalid stored graphs.

## 検証
{
  "assertions": [
    "No cycle check runs after `Graph::insert()`.",
    "Import also uses the common writer, which loads and rejects the old graph before executing its replacement closure."
  ],
  "counterEvidence": [
    "Authentication and writer-role checks limit exploitation to owners/editors in that workspace.",
    "Separate `link()` and `join()` mutations have cycle controls.",
    "Owner workspace deletion and direct operator database repair remain possible.",
    "Graph::from_nodes rejects this exact pattern in graph.rs:415-419; link/join reject cycles at 286-287 and 313-314, but Add bypasses those guards.",
    "Requires Owner/Editor permission or a scoped token belonging to such a member (write.rs:50). Viewers/nonmembers are denied.",
    "An owner can delete/recreate the workspace; an operator can repair storage directly."
  ],
  "evidenceRefs": [
    "add-fields",
    "insert-control",
    "implicit-member-edge",
    "http-apply",
    "graph-commit",
    "reload-rejects",
    "from-nodes-check",
    "client-rejects"
  ],
  "limitations": [
    "No database or runtime reproduction was executed.",
    "This does not expose other tenants or grant account authority.",
    "No application execution or crafted runtime request was performed.",
    "This is persistent same-workspace denial of ordinary writes/opening, not tenant escape or service-wide shutdown."
  ],
  "method": "semantic reduction of already-validated static source findings; no additional validation",
  "status": "validated",
  "summary": "An existing milestone m, or one added earlier in the same batch, plus task a with depends_on:[m] and in:[m] passes reference checks yet creates m -> a -> m. Apply commits the graph without final acyclicity validation. Future apply and import both validate old stored nodes first, including an otherwise valid replacement import; clients also reject installation. The sources note the exact rejected loading-test pattern and an API example containing this relation."
}

## 回帰確認
[
  "An add that both depends on and belongs to the same milestone must fail atomically.",
  "Check a multi-hop dependency plus milestone membership cycle.",
  "A rejected batch must leave the cloud graph loadable and unchanged.",
  "Reject adding a task that both depends on and joins the same milestone, including indirect cycles and batch references.",
  "Verify rejected Add leaves the graph/storage unchanged and subsequent writes and opens succeed."
]

## 場所
[
  {
    "endLine": 272,
    "path": "crates/topo-core/src/graph.rs",
    "role": "root_control",
    "startLine": 265
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "propagation",
    "startLine": 124
  },
  {
    "endLine": 156,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "entrypoint",
    "startLine": 143
  },
  {
    "endLine": 76,
    "path": "crates/topo-server/src/write.rs",
    "role": "sink",
    "startLine": 59
  },
  {
    "endLine": 108,
    "path": "crates/topo-server/src/write.rs",
    "role": "outcome",
    "startLine": 106
  },
  {
    "endLine": 118,
    "path": "crates/topo-core/src/store.rs",
    "role": "outcome",
    "startLine": 116
  },
  {
    "endLine": 423,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 413
  },
  {
    "endLine": 120,
    "path": "crates/topo-core/src/store.rs",
    "role": "outcome",
    "startLine": 114
  }
]

---

## [F049] A workspace editor can save a cycle that blocks later graph repairs

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F049
findingId: csf_7eb1bd23a58c542d180e15d3
occurrenceId: occ_542e2d7ff4b1a414ac640945
重要度: low
共通原因: graph-invariants
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

An `Add` operation can simultaneously make a task depend on a milestone and belong to that milestone. `Graph::insert()` accepts the resulting requirement cycle, and the cloud write path persists it. Every later graph mutation or replacement rebuilds the invalid stored graph first and fails, including repairs attempted by an owner.

## 根本原因
{
  "evidenceRefs": [
    "apply-route",
    "add-edges",
    "insert-no-cycle-check",
    "membership-reverse-edge",
    "write-rebuild",
    "persist-unvalidated",
    "repair-precondition",
    "graph-load-validator",
    "known-cycle-test"
  ],
  "summary": "The caller supplies both `depends_on` and `in` on `Add`. `ops::apply_one()` installs those lists before `Graph::insert()` checks only reference existence/kinds. Milestone requirements are computed from their member tasks, so inserting a task that belongs to and depends on the same milestone immediately closes a two-node cycle. The server assumes every mutation preserves acyclicity and commits the returned graph without revalidation. Its next write reconstructs that stored graph with `Graph::from_nodes()` before evaluating any repair."
}

## 修正方針
Check the full requirement relation after inserting initial dependency and membership edges, roll back the insert on failure, and revalidate every changed graph before persistence. Provide an owner-only repair path that can replace invalid stored graphs without first rebuilding them.

## 検証
{
  "counterEvidence": [
    "Nonmembers and viewers cannot write; Caller role/scope checks precede the mutation.",
    "Link and Join explicitly check reaches and bulk import validates its new Graph::from_nodes.",
    "An editor already has substantial node modification/deletion authority; the additional effect is denying repair through normal graph APIs.",
    "GET graph and the audit log can still return raw stored rows; ordinary clients reject the cyclic snapshot during install.",
    "Owners can delete/recreate the entire workspace; database operators can repair the persisted rows."
  ],
  "evidenceRefs": [
    "apply-route",
    "add-edges",
    "insert-no-cycle-check",
    "membership-reverse-edge",
    "write-rebuild",
    "persist-unvalidated",
    "repair-precondition",
    "graph-load-validator",
    "known-cycle-test",
    "import-uses-common-write",
    "local-add-entry"
  ],
  "limitations": [
    "No application code or live request was executed.",
    "The finding is scoped to an already writable workspace, not a cross-tenant or unauthenticated service compromise."
  ],
  "method": "static source trace",
  "status": "validated",
  "summary": "For milestone m, Add(t, depends_on=[m], in=[m]) passes check_references: m exists and is a milestone, references are distinct within each list, and t is not a self dependency. Requirements then include t->m and m->t. Nothing rechecks the changed graph before the D1 batch. A later remove/unlink request, or even valid PUT graph replacement, fails while loading the old cycle."
}

## 回帰確認
[
  "Add with depends_on=[m] and in=[m] is rejected atomically and does not change the graph version or rows.",
  "Valid graph replacement can repair a stored invalid graph under the appropriate owner authorization."
]

## 場所
[
  {
    "endLine": 156,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "entrypoint",
    "startLine": 143
  },
  {
    "endLine": 134,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 124
  },
  {
    "endLine": 272,
    "path": "crates/topo-core/src/graph.rs",
    "role": "root_control",
    "startLine": 264
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
    "endLine": 76,
    "path": "crates/topo-server/src/write.rs",
    "role": "sink",
    "startLine": 72
  },
  {
    "endLine": 112,
    "path": "crates/topo-server/src/write.rs",
    "role": "outcome",
    "startLine": 106
  },
  {
    "endLine": 47,
    "path": "crates/topo-core/src/graph.rs",
    "role": "outcome",
    "startLine": 35
  },
  {
    "endLine": 419,
    "path": "crates/topo-core/src/graph.rs",
    "role": "expected_control",
    "startLine": 415
  },
  {
    "endLine": 118,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "outcome",
    "startLine": 110
  },
  {
    "endLine": 423,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 413
  }
]

---

## [F052] An editor can add a task that makes a shared workspace unusable

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F052
findingId: csf_c1ebcca698c885d99e1ac572
occurrenceId: occ_a1c35e39ca3c8c55b3147699
重要度: low
共通原因: graph-invariants
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A workspace editor can add a task that both depends on a milestone and belongs to that milestone. The API accepts and persists the resulting cycle. Other members cannot open the graph, and subsequent graph writes and imports fail before they can repair it.

## 根本原因
{
  "evidenceRefs": [
    "cycle-input",
    "cycle-insert",
    "cycle-requirements",
    "cycle-apply-route",
    "cycle-persist",
    "cycle-reopen",
    "cycle-client",
    "cycle-full-check"
  ],
  "summary": "`Op::Add` sets both `depends_on` and milestone membership before calling `Graph::insert()`. Insert assumes new nodes cannot be required by existing nodes and checks references only. Membership invalidates that assumption: the milestone now requires its new member while the member can require the milestone. The graph write path stores this graph without another acyclicity check; later `Graph::from_nodes()` rejects the persisted cycle."
}

## 修正方針
Validate membership-induced cycles in `Graph::insert()` and reject the mutation atomically. Add a final acyclicity check before committing graph changes, and provide an owner-authorized recovery path for already-invalid stored graphs.

## 検証
{
  "assertions": [
    "`Graph::insert()` does not call `topo_order()` or check membership-induced cycles.",
    "Both ordinary apply and replacement import load the persisted graph through `write::write()` before changing it.",
    "Full construction's existing own-milestone cycle test at graph.rs:415-418 rejects the same relationship."
  ],
  "counterEvidence": [
    "Caller authentication and Owner/Editor permission are enforced before apply (auth.rs:25-33,71; write.rs:48-50).",
    "`Graph::link()` and `Graph::join()` explicitly reject equivalent cycles (graph.rs:281-288,301-315).",
    "An owner can delete/recreate the workspace; a trusted operator can repair database rows. There is no source-supported remote Editor repair after poisoning."
  ],
  "evidenceRefs": [
    "cycle-input",
    "cycle-insert",
    "cycle-requirements",
    "cycle-apply-route",
    "cycle-write-guard",
    "cycle-persist",
    "cycle-reopen",
    "cycle-client",
    "cycle-full-check"
  ],
  "limitations": [
    "The API can still return raw node JSON and membership management works; graph opening and graph mutation are affected.",
    "No exploit was executed."
  ],
  "method": "static source trace",
  "summary": "For existing milestone `m`, a task `a` with `depends_on: [m]` and `in: [m]` passes ID and reference checks. `requirements(a)` includes `m`, and `requirements(m)` includes `a`, proving a cycle. Successful API apply persists it; every later graph write builds the old graph before applying repair operations, and client installation also rejects it."
}

## 回帰確認
[
  "An Add task depending on and belonging to the same milestone must fail without changing nodes, version or change log.",
  "Test an indirect dependency path to any milestone named by a new task's membership.",
  "Assert valid Add batches still work through local operations and cloud apply."
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
    "endLine": 272,
    "path": "crates/topo-core/src/graph.rs",
    "role": "root_control",
    "startLine": 264
  },
  {
    "endLine": 134,
    "path": "crates/topo-core/src/graph.rs",
    "role": "propagation",
    "startLine": 126
  },
  {
    "endLine": 156,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "entrypoint",
    "startLine": 149
  },
  {
    "endLine": 60,
    "path": "crates/topo-server/src/write.rs",
    "role": "expected_control",
    "startLine": 48
  },
  {
    "endLine": 76,
    "path": "crates/topo-server/src/write.rs",
    "role": "sink",
    "startLine": 61
  },
  {
    "endLine": 108,
    "path": "crates/topo-server/src/write.rs",
    "role": "outcome",
    "startLine": 106
  },
  {
    "endLine": 118,
    "path": "crates/topo-core/src/store.rs",
    "role": "outcome",
    "startLine": 114
  },
  {
    "endLine": 47,
    "path": "crates/topo-core/src/graph.rs",
    "role": "expected_control",
    "startLine": 35
  }
]

---

## [F068] A permitted Add operation can persist a cycle and block shared workspace repair

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F068
findingId: csf_327dac1926d666821de43e02
occurrenceId: occ_4c787f6736ccc7013f602d21
重要度: low
共通原因: graph-invariants
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

An owner/editor can Add a task that both depends on and belongs to a milestone, causing an unchecked requirement cycle to be persisted. Cloud clients reject the snapshot and subsequent apply/import cannot load the prior graph to repair it. The same insertion control can save invalid local Markdown, which fails later open/reload; manual local file repair remains possible.

## 根本原因
{
  "evidenceRefs": [
    "add-fields",
    "insert-control",
    "implicit-member-edge",
    "http-apply",
    "graph-commit",
    "reload-rejects",
    "from-nodes-check",
    "client-rejects",
    "dedup-0003-1-http-apply",
    "dedup-0003-1-graph-write",
    "dedup-0003-1-graph-commit",
    "dedup-0003-1-later-graph-reject",
    "dedup-0003-1-client-snapshot-reject",
    "dedup-0003-1-local-apply-persistence",
    "dedup-0003-1-local-reopen-validation",
    "dedup-0003-1-join-cycle-check"
  ],
  "summary": "Op::Add copies dependency and milestone membership fields into a new node, and Graph::insert checks only duplicate IDs and reference validity. Its assumption that no existing node can require a new task misses the reverse edge created by milestone membership. A task depending on the milestone it joins creates m -> task -> m; longer combined dependency/membership cycles are affected too. The mutation/persistence path trusts insert without final topo_order validation. Subsequent Graph::from_nodes and client snapshot installation enforce acyclicity, rejecting the already-persisted graph before an ordinary edit or replacing import can repair it. Local Workspace::apply likewise accepts the unchecked Add graph and saves Markdown without final acyclicity validation; read_nodes later rebuilds it through Graph::from_nodes and rejects the persisted cycle. Manual local editing can recover that instance, while normal server replacement/import is blocked by old-state reconstruction."
}

## 修正方針
Validate the full requirement relation, including reverse milestone-membership edges, when inserting a node; roll back rejected insertion. Enforce complete graph validity before both local and cloud persistence, and provide an owner-authorized replacement/repair import path for already-invalid stored cloud graphs.

## 検証
{
  "assertions": [
    "No cycle check runs after `Graph::insert()`.",
    "Import also uses the common writer, which loads and rejects the old graph before executing its replacement closure."
  ],
  "counterEvidence": [
    "Authentication and writer-role checks limit exploitation to owners/editors in that workspace.",
    "Separate `link()` and `join()` mutations have cycle controls.",
    "Owner workspace deletion and direct operator database repair remain possible.",
    "Graph::from_nodes rejects this exact pattern in graph.rs:415-419; link/join reject cycles at 286-287 and 313-314, but Add bypasses those guards.",
    "Requires Owner/Editor permission or a scoped token belonging to such a member (write.rs:50). Viewers/nonmembers are denied.",
    "An owner can delete/recreate the workspace; an operator can repair storage directly.",
    "Graph writes require current Owner/Editor membership and enforce scoped-token visibility (write.rs:50; auth.rs:55-71).",
    "Graph::from_nodes and sibling link/join check cycles (graph.rs:35-47,281-290,302-317), but those checks do not protect Add's combined fields.",
    "The attacker already has broad graph edit authority, so the report concerns durable inability to repair rather than ordinary node deletion.",
    "Local node files can be manually repaired;the lasting supported-API recovery failure concerns the server workspace."
  ],
  "evidenceRefs": [
    "add-fields",
    "insert-control",
    "implicit-member-edge",
    "http-apply",
    "graph-commit",
    "reload-rejects",
    "from-nodes-check",
    "client-rejects",
    "dedup-0003-1-http-apply",
    "dedup-0003-1-graph-write",
    "dedup-0003-1-graph-commit",
    "dedup-0003-1-later-graph-reject",
    "dedup-0003-1-client-snapshot-reject",
    "dedup-0003-1-join-cycle-check",
    "dedup-0003-1-local-apply-persistence",
    "dedup-0003-1-local-reopen-validation"
  ],
  "limitations": [
    "No database or runtime reproduction was executed.",
    "This does not expose other tenants or grant account authority.",
    "No application execution or crafted runtime request was performed.",
    "This is persistent same-workspace denial of ordinary writes/opening, not tenant escape or service-wide shutdown.",
    "No live deployment or runtime exploit was tested.",
    "Owner workspace deletion/recreation and direct operator database repair remain possible."
  ],
  "method": "semantic reduction of already-validated Standard source findings; no additional validation",
  "status": "validated",
  "summary": "An existing milestone m, or one added earlier in the same batch, plus task a with depends_on:[m] and in:[m] passes reference checks yet creates m -> a -> m. Apply commits the graph without final acyclicity validation. Future apply and import both validate old stored nodes first, including an otherwise valid replacement import; clients also reject installation. The sources note the exact rejected loading-test pattern and an API example containing this relation. The third source explicitly preserves the local Workspace::apply/save and read_nodes reconstruction subcase; manual Markdown repair can restore local access and does not cross an independent remote authorization boundary."
}

## 回帰確認
[
  "An add that both depends on and belongs to the same milestone must fail atomically.",
  "Check a multi-hop dependency plus milestone membership cycle.",
  "A rejected batch must leave the cloud graph loadable and unchanged.",
  "Reject adding a task that both depends on and joins the same milestone, including indirect cycles and batch references.",
  "Verify rejected Add leaves the graph/storage unchanged and subsequent writes and opens succeed.",
  "An Add with the same milestone in depends_on and in must fail without changing nodes or version.",
  "Reject longer requirement cycles introduced by new task membership while accepting valid membership additions.",
  "Verify a failed cyclic Add leaves CLI/GUI opens and later graph imports usable."
]

## 場所
[
  {
    "endLine": 272,
    "path": "crates/topo-core/src/graph.rs",
    "role": "root_control",
    "startLine": 265
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "propagation",
    "startLine": 124
  },
  {
    "endLine": 156,
    "path": "crates/topo-server/src/routes/graph.rs",
    "role": "entrypoint",
    "startLine": 143
  },
  {
    "endLine": 76,
    "path": "crates/topo-server/src/write.rs",
    "role": "sink",
    "startLine": 59
  },
  {
    "endLine": 108,
    "path": "crates/topo-server/src/write.rs",
    "role": "outcome",
    "startLine": 106
  },
  {
    "endLine": 118,
    "path": "crates/topo-core/src/store.rs",
    "role": "outcome",
    "startLine": 116
  },
  {
    "endLine": 423,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 413
  },
  {
    "endLine": 120,
    "path": "crates/topo-core/src/store.rs",
    "role": "outcome",
    "startLine": 114
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 124
  },
  {
    "endLine": 79,
    "path": "crates/topo-server/src/write.rs",
    "role": "sink",
    "startLine": 59
  },
  {
    "endLine": 109,
    "path": "crates/topo-server/src/write.rs",
    "role": "outcome",
    "startLine": 106
  },
  {
    "endLine": 121,
    "path": "crates/topo-core/src/store.rs",
    "role": "outcome",
    "startLine": 114
  },
  {
    "endLine": 188,
    "path": "crates/topo-core/src/store.rs",
    "role": "sink",
    "startLine": 181
  },
  {
    "endLine": 214,
    "path": "crates/topo-core/src/store.rs",
    "role": "outcome",
    "startLine": 203
  }
]


## 対応結果（2026-10-03）
結果: fixed。Graph::insertで所属の暗黙辺も含め、直接・推移的閉路を追加前に拒否。失敗時の無変更、正常追加、既存操作テスト成功。
最終検証: cargo test --locked --workspace 116成功/既存3ignore、fmt、native/Worker clippy -D warnings、local Worker/D1 smoke成功。詳細: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/artifacts-6a4e5a1e5a63564edc8ca8693b4bf5cc93fdb5baaaf0c17a2de44b08336f148b/artifacts/report_fix_20261003.md
