---
id: 1cvgx6
kind: task
title: ノード・設定ファイルの保存境界を守る
status: done
tags:
- security
- filesystem
- 44dbfab6-0d90-475d-b2a3-4df6534779e5
milestones:
- f0ehc5
---

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
統合した項目: D021, D022, D028, D032, D037, D042, F002, F011, F019, F020, F023, F026, F031, F034, F042, F044, F048, F053, F059, F062, F067
同じ原因の報告と未検証候補を追跡する。報告件数は独立した脆弱性数ではない。

---

## [D021] 確認: Predictable node temporary paths may follow attacker-supplied symlinks

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D021
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "baselineOriginalEvidence": "Workspace::save_files writes predictable `<id>.md.tmp` with fs::write before rename (store.rs:163-167), follows attacker-supplied repo symlink ignored during .md loading; node body can supply attacker SSH key/commands, enabling overwrite of victim-writable files after routine status/edit.",
    "candidateId": "workspace-temp-symlink-write",
    "originalEvidence": "Investigator preliminary: save_files follows predictable <id>.md.tmp symlinks (store.rs:163-167).",
    "paths": [
      "crates/topo-core/src/store.rs"
    ],
    "reason": "Verify source-sharing boundary, symlink preservation, actual write semantics, alternate sinks and realistic victim privileges.",
    "title": "Predictable node temporary paths may follow attacker-supplied symlinks"
  },
  "candidateId": "workspace-temp-symlink-write",
  "id": "workspace-temp-symlink-write",
  "paths": [
    "crates/topo-core/src/store.rs"
  ],
  "reason": "Verify source-sharing boundary, symlink preservation, actual write semantics, alternate sinks and realistic victim privileges."
}
```

---

## [D022] 確認: Public graph insertion may permit unsafe IDs before file save

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D022
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "candidateId": "library-node-id-path-write",
    "originalEvidence": "Investigator preliminary: Graph::insert/from_nodes do not validate IDs before Workspace::save path join.",
    "paths": [
      "crates/topo-core/src/graph.rs",
      "crates/topo-core/src/store.rs",
      "crates/topo-core/src/model.rs"
    ],
    "reason": "Verify reachable parser/public API and caller obligations; source-check product input validation and protected file containment.",
    "title": "Public graph insertion may permit unsafe IDs before file save"
  },
  "candidateId": "library-node-id-path-write",
  "id": "library-node-id-path-write",
  "paths": [
    "crates/topo-core/src/graph.rs",
    "crates/topo-core/src/store.rs",
    "crates/topo-core/src/model.rs"
  ],
  "reason": "Verify reachable parser/public API and caller obligations; source-check product input validation and protected file containment."
}
```

---

## [D028] 確認: Predictable node temporary paths may follow attacker-supplied symlinks

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D028
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "baselineFinalFinding": {
      "attacker": "An attacker who supplies a local workspace or repository containing a valid node and a symlink at that node's predictable .md.tmp path.",
      "confidence": "high",
      "counterevidence": [
        "Remote node IDs and operation-supplied IDs are validated against a restricted alphabet; this attack works with an ordinary valid ID and does not require traversal characters.",
        "Renaming the temporary path does not prevent the external overwrite, which has already happened.",
        "The attack requires filesystem symlinks to be preserved and requires a subsequent write to the affected node; reading the workspace alone does not trigger this overwrite."
      ],
      "cwe": "CWE-61",
      "impact": "A workspace can cause writes to arbitrary victim-writable files. The node's preserved Markdown body can contain an attacker SSH public-key line, allowing an a.md.tmp symlink to ~/.ssh/authorized_keys to install a key when the victim edits the node. Redirecting the temporary file to a shell startup file can similarly introduce executable commands in the body.",
      "locations": [
        {
          "file": "crates/topo-core/src/store.rs",
          "line_end": 167,
          "line_start": 162
        },
        {
          "file": "crates/topo-core/src/store.rs",
          "line_end": 210,
          "line_start": 198
        },
        {
          "file": "crates/topo-core/src/store.rs",
          "line_end": 222,
          "line_start": 216
        },
        {
          "file": "crates/topo-cli/src/main.rs",
          "line_end": 456,
          "line_start": 446
        }
      ],
      "recommended_remediation": "Create an unpredictable temporary file with exclusive creation and no symlink following, write through its already-open handle, and rename that file atomically. Also ensure writes remain anchored beneath the workspace nodes directory when directory components can be attacker-controlled.",
      "severity": "high",
      "source_to_sink": "read_nodes only loads files with the .md extension, so an attacker-supplied a.md.tmp symlink does not participate in graph validation. A routine status or title edit of node a invokes save_files, which writes the rendered node to a.md.tmp using fs::write. That operation follows the symlink and truncates its external target before fs::rename executes.",
      "supporting_source_evidence": [
        "The temporary path is deterministically derived with `path.with_extension(\"md.tmp\")`.",
        "`fs::write(&tmp, render(node))` follows an existing symbolic link.",
        "read_nodes ignores .tmp files because it checks for the exact md extension.",
        "render appends node.body verbatim after the frontmatter.",
        "Status edits preserve the existing attacker-controlled body while causing the node to be saved."
      ],
      "title": "Predictable temporary node files follow repository-supplied symlinks",
      "violated_security_invariant": "Saving a node must not overwrite files outside the selected workspace."
    },
    "baselineOriginalEvidence": "Workspace::save_files writes predictable `<id>.md.tmp` with fs::write before rename (store.rs:163-167), follows attacker-supplied repo symlink ignored during .md loading; node body can supply attacker SSH key/commands, enabling overwrite of victim-writable files after routine status/edit.",
    "candidateId": "workspace-temp-symlink-write",
    "originalEvidence": "Investigator preliminary: save_files follows predictable <id>.md.tmp symlinks (store.rs:163-167).",
    "paths": [
      "crates/topo-core/src/store.rs"
    ],
    "reason": "Verify source-sharing boundary, symlink preservation, actual write semantics, alternate sinks and realistic victim privileges.",
    "title": "Predictable node temporary paths may follow attacker-supplied symlinks"
  },
  "candidateId": "workspace-temp-symlink-write",
  "id": "workspace-temp-symlink-write-85b6ed09d06399cd",
  "paths": [
    "crates/topo-core/src/store.rs"
  ],
  "reason": "Verify source-sharing boundary, symlink preservation, actual write semantics, alternate sinks and realistic victim privileges."
}
```

---

## [D032] 確認: Predictable node temporary paths may follow attacker-supplied symlinks

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D032
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "candidateId": "workspace-temp-symlink-write",
    "originalEvidence": "Investigator preliminary: save_files follows predictable <id>.md.tmp symlinks (store.rs:163-167).",
    "paths": [
      "crates/topo-core/src/store.rs"
    ],
    "reason": "Verify source-sharing boundary, symlink preservation, actual write semantics, alternate sinks and realistic victim privileges.",
    "title": "Predictable node temporary paths may follow attacker-supplied symlinks"
  },
  "candidateId": "workspace-temp-symlink-write",
  "id": "workspace-temp-symlink-write-d638ff0934c4f1d7",
  "paths": [
    "crates/topo-core/src/store.rs"
  ],
  "reason": "Verify source-sharing boundary, symlink preservation, actual write semantics, alternate sinks and realistic victim privileges."
}
```

---

## [D037] 確認: A workspace-supplied temporary-file symlink redirects node writes outside the workspace

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D037
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "originalEvidence": "store.rs:164-167 writes predictable <id>.md.tmp with fs::write, which follows a preexisting symlink, then renames it. read_nodes considers only .md files (208), so a malicious shared workspace can include valid a.md plus ignored a.md.tmp symlink outside the workspace. Victim topo edit/status saves and truncates/overwrites symlink target under victim authority. Baseline independently found same path; valid IDs and atomic rename do not prevent it.",
    "source": "client_investigator and baseline",
    "title": "A workspace-supplied temporary-file symlink redirects node writes outside the workspace"
  },
  "candidateId": "candidate-client-predictable-temp-symlink",
  "id": "candidate-client-predictable-temp-symlink",
  "paths": [
    "crates/topo-core/src/store.rs"
  ],
  "reason": "Awaiting parent validation of imported workspace symlink capabilities, ignored temp entries and save sink."
}
```

---

## [D042] 確認: Independent baseline candidate awaiting full evidence packet and parent validation.

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D042
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "locations": [
      {
        "endLine": 167,
        "path": "crates/topo-core/src/store.rs",
        "startLine": 162
      }
    ],
    "originalMessage": "predictable node .md.tmp files are written via fs::write, so a committed symlink can overwrite a file outside .topo during an ordinary status/edit save (topo-core store.rs 162-167)",
    "producer": "baseline"
  },
  "candidateId": "discovery-0011.node-temp-symlink",
  "id": "discovery-0011.node-temp-symlink",
  "reason": "Independent baseline candidate awaiting full evidence packet and parent validation."
}
```

---

## [F002] Saving or deleting supplied nodes can modify files outside the workspace

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F002
findingId: csf_9b7cfa8e0b92163b15878c1c
occurrenceId: occ_139fcc5767bafd853e0c7a29
重要度: medium
共通原因: filesystem
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

An untrusted workspace can plant a predictable <id>.md.tmp symlink: ordinary CLI/GUI/TUI edits or local materialization/export follow it, truncating a victim-writable outside file with serialized node metadata and notes before rename. A symlinked .topo/nodes directory independently redirects reads, saves, and deleted-node removal into an external valid node store.

## 根本原因
{
  "evidenceRefs": [
    "node-read",
    "local-apply",
    "temp-write",
    "node-path",
    "notes-render",
    "symlink-loader",
    "symlink-write",
    "symlink-cli-save",
    "symlink-directory-identity",
    "symlink-directory-delete",
    "dedup-0003-2-temp-write-follows-link",
    "dedup-0003-2-attacker-body",
    "dedup-0004-1-node-mutation-entry",
    "dedup-0004-1-workspace-save-dispatch"
  ],
  "summary": "read_nodes loads only .md entries and ignores the planted .md.tmp symlink. save_files derives that temporary path from a normal validated node ID and calls fs::write without exclusive creation or no-follow protection; the write follows the existing link and truncates its outside target. The subsequent rename protects neither the already-completed external write nor the temporary path. Metadata and free-form body from render(node) determine the written bytes; lexical NodeId validation does not constrain a symlink target. The incoming source also establishes that nodes_dir retains the supplied directory entry and node_path/remove_file follow a linked nodes directory. That variant redirects save and removal even without a staging-file symlink, provided the external nodes parse successfully. A link only at the final .md entry is replaced by rename and is not the proven direct outside-file overwrite."
}

## 修正方針
Create a fresh unpredictable temporary file exclusively in a trusted nodes directory, write through its already-open handle and close it, then atomically rename it over the intended node. Never truncate a pre-existing predictable temporary path. Reject symlinked storage paths where workspace confinement is required and anchor directory operations when concurrent directory replacement is in scope.

## 検証
{
  "counterEvidence": [
    "Requires local workspace delivery or write access to its node directory.",
    "Cloud graph-only operation does not use this path until a local save/export occurs.",
    "The outside file must be writable by the victim; no privilege escalation beyond that user's authority is claimed.",
    "Node IDs from remote input are restricted to lowercase alphanumerics by model.rs:29-31, which blocks lexical traversal but not this valid-name symlink.",
    "Attacker must supply a symlink-containing workspace or write within its nodes directory; there is no unauthenticated remote file-write claim.",
    "Node IDs are constrained at application mutation/import boundaries, so this attack does not need path traversal.",
    "A symlink only at the final .md pathname is replaced by the rename; the proven direct external overwrite uses the staging path.",
    "Ordinary OS file permissions constrain the destination; this is not OS privilege escalation.",
    "Directory redirection requires an external directory whose nodes parse successfully; the victim must mutate or delete a loaded node.",
    "Reading alone does not trigger the write; the victim must modify and save the specific task.",
    "The target requires an existing parent and victim write permission.",
    "This does not assume the attacker already controls a user process or global configuration; supplying a lower-trust workspace filesystem is sufficient.",
    "Node ID checks prevent string traversal, and rename provides ordinary replacement atomicity, but both occur independently of the already-followed temporary link.",
    "Read-only commands do not call save_files.",
    "The target must be writable by the victim; this does not exceed their OS permissions.",
    "Git on platforms that check out symlinks as plain files will not follow the attacker-supplied entry.",
    "Cloud-backed saves send operations to a remote and do not take this local file path."
  ],
  "evidenceRefs": [
    "node-read",
    "local-apply",
    "temp-write",
    "node-path",
    "notes-render",
    "symlink-loader",
    "symlink-write",
    "symlink-cli-save",
    "symlink-directory-identity",
    "symlink-directory-delete",
    "dedup-0003-2-temp-write-follows-link",
    "dedup-0003-2-attacker-body",
    "dedup-0004-1-node-mutation-entry",
    "dedup-0004-1-workspace-save-dispatch"
  ],
  "limitations": [
    "No planted symlink or destructive write was created.",
    "Attacker cannot choose arbitrary file bytes independently of serialized task metadata and body.",
    "No filesystem mutation or exploit reproduction was performed.",
    "This establishes content replacement/truncation with serialized node content; arbitrary machine-code execution is not claimed.",
    "No application execution or file-write payload was attempted.",
    "Victim processing must preserve supplied symlinks and change a matching node.",
    "No filesystem exploit was executed.",
    "Unix symlink support supplies a concrete platform; Windows symlink/reparse prerequisites and later code execution depend on host configuration.",
    "Validated from source; no malicious checkout or filesystem writes were executed.",
    "Reliable code execution is not established; the demonstrated effect is outside-file truncation/overwrite with serialized node content."
  ],
  "method": "semantic reduction of already-validated Standard source findings; no additional validation",
  "status": "validated",
  "summary": "All five sources establish the predictable staging-file write and its ignored .md.tmp entry. A previously absorbed source additionally establishes linked-directory redirection through nodes_dir, node_path, and deleted-node remove_file; its external store must contain valid loadable nodes. Identifier restrictions prevent lexical traversal but neither link variant. Cloud graph-only operation reaches these sinks only during local save/export/pull materialization. All conclusions are supplied static findings; no reducer write or reproduction occurred. The assigned source explicitly notes that the external file has already been truncated/overwritten even if the subsequent rename fails, and that hosts checking out symlinks as plain files do not take the planted-link path."
}

## 回帰確認
[
  "A pre-existing `<id>.md.tmp` symlink must not modify its outside target during task save.",
  "Saving must reject a symlinked node directory when workspace confinement is required.",
  "Save a changed node with a preexisting .md.tmp symlink and verify its external target remains unchanged.",
  "Verify failed temporary-file creation leaves existing node data intact.",
  "A supplied <id>.md.tmp symlink must not change its external target after a normal node status/edit operation.",
  "Verify valid local saves remain atomic per node and reject linked nodes directories that escape the intended store.",
  "A linked nodes directory targeting another valid workspace must not write or delete that workspace's node files.",
  "Place a symlink at the legacy node temporary path and assert saving neither follows it nor changes its target.",
  "Verify ordinary saves still replace node files atomically and remove their temporary files.",
  "Check directory-link handling for supported platforms.",
  "A pre-existing {id}.md.tmp symlink causes save to fail safely or use a fresh exclusive temporary file, leaving the outside target unchanged.",
  "Normal local node edits still replace only their node file."
]

## 場所
[
  {
    "endLine": 167,
    "path": "crates/topo-core/src/store.rs",
    "role": "root_control",
    "startLine": 162
  },
  {
    "endLine": 213,
    "path": "crates/topo-core/src/store.rs",
    "role": "entrypoint",
    "startLine": 203
  },
  {
    "endLine": 222,
    "path": "crates/topo-core/src/store.rs",
    "role": "sink",
    "startLine": 217
  },
  {
    "endLine": 457,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 453
  },
  {
    "endLine": 196,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 189
  },
  {
    "endLine": 211,
    "path": "crates/topo-core/src/store.rs",
    "role": "user_input",
    "startLine": 203
  },
  {
    "endLine": 188,
    "path": "crates/topo-core/src/store.rs",
    "role": "entrypoint",
    "startLine": 181
  },
  {
    "endLine": 200,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 198
  },
  {
    "endLine": 328,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 314
  },
  {
    "endLine": 106,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 103
  },
  {
    "endLine": 174,
    "path": "crates/topo-core/src/store.rs",
    "role": "sink",
    "startLine": 169
  },
  {
    "endLine": 171,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 162
  },
  {
    "endLine": 213,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 198
  },
  {
    "endLine": 325,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 313
  },
  {
    "endLine": 460,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 446
  },
  {
    "endLine": 64,
    "path": "crates/topo-cli/src/tui.rs",
    "role": "entrypoint",
    "startLine": 61
  },
  {
    "endLine": 343,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 341
  },
  {
    "endLine": 381,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 377
  },
  {
    "endLine": 195,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 189
  },
  {
    "endLine": 210,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 198
  },
  {
    "endLine": 222,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 217
  },
  {
    "endLine": 457,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 446
  },
  {
    "endLine": 213,
    "path": "crates/topo-core/src/store.rs",
    "role": "entrypoint",
    "startLine": 198
  },
  {
    "endLine": 222,
    "path": "crates/topo-core/src/store.rs",
    "role": "user_input",
    "startLine": 216
  },
  {
    "endLine": 449,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 446
  },
  {
    "endLine": 147,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 143
  },
  {
    "endLine": 168,
    "path": "crates/topo-core/src/store.rs",
    "role": "root_control",
    "startLine": 162
  },
  {
    "endLine": 213,
    "path": "crates/topo-core/src/store.rs",
    "role": "user_input",
    "startLine": 203
  }
]

---

## [F011] Cloud link updates can rewrite an external TOML file

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F011
findingId: csf_404b10b5c79c390873a74b60
occurrenceId: occ_2d951c732ddbef44bab98b77
重要度: low
共通原因: filesystem
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A supplied .topo/config.toml symlink is followed by cloud link persistence. A normal successful push, link, or pull can rewrite a user-writable external TOML file, or create an absent target, under the victim's filesystem authority.

## 根本原因
{
  "evidenceRefs": [
    "config-write",
    "config-read",
    "config-trigger"
  ],
  "summary": "Configuration persistence directly writes the selected config.toml pathname after reading it through links. Neither operation establishes that the target remains inside the selected workspace."
}

## 修正方針
Persist configuration with exclusive temporary creation and a controlled rename in a verified directory. Reject or explicitly authorize external symlink destinations and linked parent directories.

## 検証
{
  "counterEvidence": [
    "Non-TOML existing targets fail parsing before the write.",
    "Other parsed tables are preserved, so arbitrary bytes or command execution are not established.",
    "Push/link require no existing cloud link and network operations must succeed.",
    "Normal OS permissions constrain external targets."
  ],
  "evidenceRefs": [
    "config-write",
    "config-read",
    "config-trigger"
  ],
  "limitations": [
    "No write or application command was executed."
  ],
  "method": "static source trace",
  "summary": "Source establishes the stated chain and prerequisites. No application code was executed."
}

## 回帰確認
[
  "A config.toml link to compatible external TOML or a missing external target must not modify/create that target after cloud link persistence.",
  "Verify unrelated tables and standard workspace updates remain valid."
]

## 場所
[
  {
    "endLine": 48,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "root_control",
    "startLine": 35
  },
  {
    "endLine": 24,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 18
  },
  {
    "endLine": 211,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 200
  },
  {
    "endLine": 23,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "propagation",
    "startLine": 18
  },
  {
    "endLine": 179,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 170
  },
  {
    "endLine": 196,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 189
  }
]

---

## [F019] Saving or deleting supplied nodes can modify files outside the workspace

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F019
findingId: csf_8d472214c061dd5dbd41d8a0
occurrenceId: occ_c79142adc85b4c6818119f48
重要度: medium
共通原因: filesystem
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A repository producer can include a symlink named .topo/nodes/<existing-id>.md.tmp pointing to another file writable by the victim. The loader ignores this path. When the victim changes that node, save follows and truncates the symlink target before renaming the staging path. A symlinked .topo/nodes directory also redirects save/removal operations into an external valid node store.

## 根本原因
{
  "evidenceRefs": [
    "symlink-loader",
    "symlink-route",
    "symlink-write",
    "symlink-node-path",
    "symlink-cli-save",
    "symlink-directory-identity",
    "symlink-directory-delete"
  ],
  "summary": "The local store uses a predictable staging filename and fs::write follows existing symlinks. The subsequent rename provides no protection for the earlier target truncation. The store also retains and follows a producer-selected nodes directory, letting a directory link redirect reads, writes, and deletion to another valid node store."
}

## 修正方針
Create a fresh unpredictable staging file with exclusive creation inside a verified workspace directory, reject symlinked workspace directories, and atomically replace the final entry without following a supplied staging link. Use directory handles and confinement checks appropriate to each supported platform.

## 検証
{
  "counterEvidence": [
    "Node IDs are constrained at application mutation/import boundaries, so this attack does not need path traversal.",
    "A symlink only at the final .md pathname is replaced by the rename; the proven direct external overwrite uses the staging path.",
    "Ordinary OS file permissions constrain the destination; this is not OS privilege escalation.",
    "Directory redirection requires an external directory whose nodes parse successfully; the victim must mutate or delete a loaded node."
  ],
  "evidenceRefs": [
    "symlink-loader",
    "symlink-route",
    "symlink-write",
    "symlink-node-path",
    "symlink-cli-save",
    "symlink-directory-identity",
    "symlink-directory-delete"
  ],
  "limitations": [
    "No application execution or file-write payload was attempted.",
    "Victim processing must preserve supplied symlinks and change a matching node."
  ],
  "method": "static source trace",
  "summary": "Source establishes the stated chain and prerequisites. No application code was executed. Parent inspected the independently returned directory-link variant in the same store and verified nodes_dir plus removal sink; it is retained under the same filesystem-confinement root cause."
}

## 回帰確認
[
  "A supplied <id>.md.tmp symlink must not change its external target after a normal node status/edit operation.",
  "Verify valid local saves remain atomic per node and reject linked nodes directories that escape the intended store.",
  "A linked nodes directory targeting another valid workspace must not write or delete that workspace's node files."
]

## 場所
[
  {
    "endLine": 211,
    "path": "crates/topo-core/src/store.rs",
    "role": "user_input",
    "startLine": 203
  },
  {
    "endLine": 188,
    "path": "crates/topo-core/src/store.rs",
    "role": "entrypoint",
    "startLine": 181
  },
  {
    "endLine": 167,
    "path": "crates/topo-core/src/store.rs",
    "role": "root_control",
    "startLine": 162
  },
  {
    "endLine": 200,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 198
  },
  {
    "endLine": 328,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 314
  },
  {
    "endLine": 106,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 103
  },
  {
    "endLine": 174,
    "path": "crates/topo-core/src/store.rs",
    "role": "sink",
    "startLine": 169
  },
  {
    "endLine": 171,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 162
  },
  {
    "endLine": 213,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 198
  },
  {
    "endLine": 325,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 313
  },
  {
    "endLine": 460,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 446
  },
  {
    "endLine": 64,
    "path": "crates/topo-cli/src/tui.rs",
    "role": "entrypoint",
    "startLine": 61
  },
  {
    "endLine": 343,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 341
  },
  {
    "endLine": 381,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 377
  },
  {
    "endLine": 195,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 189
  },
  {
    "endLine": 210,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 198
  },
  {
    "endLine": 222,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 217
  },
  {
    "endLine": 457,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 446
  }
]

---

## [F020] Saving a supplied workspace can overwrite a file outside it

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F020
findingId: csf_61817904b45c8e8f4b0d2dc9
occurrenceId: occ_9af066bb5a1023a50ff7db74
重要度: medium
共通原因: filesystem
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A repository producer can include a symlink named .topo/nodes/<existing-id>.md.tmp pointing to another file writable by the victim. The loader ignores this path. When the victim changes that node, save follows and truncates the symlink target before renaming the staging path.

## 根本原因
{
  "evidenceRefs": [
    "symlink-loader",
    "symlink-route",
    "symlink-write",
    "symlink-node-path",
    "symlink-cli-save"
  ],
  "summary": "The local store uses a predictable staging filename and fs::write follows existing symlinks. The subsequent rename provides no protection for the earlier target truncation."
}

## 修正方針
Create a fresh unpredictable staging file with exclusive creation inside a verified workspace directory, reject symlinked workspace directories, and atomically replace the final entry without following a supplied staging link. Use directory handles and confinement checks appropriate to each supported platform.

## 検証
{
  "counterEvidence": [
    "Node IDs are constrained at application mutation/import boundaries, so this attack does not need path traversal.",
    "A symlink only at the final .md pathname is replaced by the rename; the proven direct external overwrite uses the staging path.",
    "Ordinary OS file permissions constrain the destination; this is not OS privilege escalation."
  ],
  "evidenceRefs": [
    "symlink-loader",
    "symlink-route",
    "symlink-write",
    "symlink-node-path",
    "symlink-cli-save"
  ],
  "limitations": [
    "No application execution or file-write payload was attempted.",
    "Victim processing must preserve supplied symlinks and change a matching node."
  ],
  "method": "static source trace",
  "summary": "Source establishes the stated chain and prerequisites. No application code was executed."
}

## 回帰確認
[
  "A supplied <id>.md.tmp symlink must not change its external target after a normal node status/edit operation.",
  "Verify valid local saves remain atomic per node and reject linked nodes directories that escape the intended store."
]

## 場所
[
  {
    "endLine": 211,
    "path": "crates/topo-core/src/store.rs",
    "role": "user_input",
    "startLine": 203
  },
  {
    "endLine": 188,
    "path": "crates/topo-core/src/store.rs",
    "role": "entrypoint",
    "startLine": 181
  },
  {
    "endLine": 167,
    "path": "crates/topo-core/src/store.rs",
    "role": "root_control",
    "startLine": 162
  },
  {
    "endLine": 200,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 198
  },
  {
    "endLine": 328,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 314
  }
]

---

## [F023] Saving an untrusted local workspace can overwrite files outside its nodes directory

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F023
findingId: csf_d653b54f184f299596789880
occurrenceId: occ_6fe5a66e8b17ecaf39058f64
重要度: medium
共通原因: filesystem
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A supplied workspace can contain a valid task and a symlink at its predictable `.md.tmp` path. When the victim changes that task, saving follows the symlink and truncates an unrelated victim-writable file with task contents.

## 根本原因
{
  "evidenceRefs": [
    "loader-ignores-temp",
    "node-path",
    "temp-write-follows-link",
    "attacker-body",
    "normal-local-save"
  ],
  "summary": "A supplied checkout can provide `a.md` together with a symlink `a.md.tmp` pointing outside the workspace. `read_nodes()` ignores the latter. The victim's normal task mutation enters `save_files()`, which derives that same fixed temporary filename and uses `fs::write()` rather than exclusive regular-file creation. Opening the path follows the symlink and overwrites its target with serialized task data and the supplied notes before `fs::rename()` can replace the node path."
}

## 修正方針
Create a unique temporary regular file with exclusive creation inside a verified nodes directory, write through its handle, and atomically rename that file. Protect directory traversal from symlinks or reparse points where untrusted workspace directories are supported.

## 検証
{
  "counterEvidence": [
    "Reading alone does not trigger the write; the victim must modify and save the specific task.",
    "The target requires an existing parent and victim write permission.",
    "This does not assume the attacker already controls a user process or global configuration; supplying a lower-trust workspace filesystem is sufficient.",
    "Node ID checks prevent string traversal, and rename provides ordinary replacement atomicity, but both occur independently of the already-followed temporary link."
  ],
  "evidenceRefs": [
    "loader-ignores-temp",
    "node-path",
    "temp-write-follows-link",
    "attacker-body",
    "normal-local-save"
  ],
  "limitations": [
    "No filesystem exploit was executed.",
    "Unix symlink support supplies a concrete platform; Windows symlink/reparse prerequisites and later code execution depend on host configuration."
  ],
  "method": "Independent static source trace",
  "summary": "The parent verified ignored temporary extensions, deterministic node and temporary paths, the order of write then rename, and verbatim notes rendering. Safe node IDs constrain path text but do not constrain filesystem link targets."
}

## 回帰確認
[
  "Place a symlink at the legacy node temporary path and assert saving neither follows it nor changes its target.",
  "Verify ordinary saves still replace node files atomically and remove their temporary files.",
  "Check directory-link handling for supported platforms."
]

## 場所
[
  {
    "endLine": 167,
    "path": "crates/topo-core/src/store.rs",
    "role": "root_control",
    "startLine": 162
  },
  {
    "endLine": 213,
    "path": "crates/topo-core/src/store.rs",
    "role": "entrypoint",
    "startLine": 198
  },
  {
    "endLine": 222,
    "path": "crates/topo-core/src/store.rs",
    "role": "user_input",
    "startLine": 216
  },
  {
    "endLine": 449,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 446
  }
]

---

## [F026] Editing an untrusted workspace can overwrite files outside the workspace

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F026
findingId: csf_bb47746d28e9b7536774ee1f
occurrenceId: occ_7daf40f069501cd33a015e8b
重要度: medium
共通原因: filesystem
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A workspace can contain a symlink at a node's predictable `.md.tmp` path. Editing that node follows the symlink and truncates a user-writable file outside `.topo` before the save renames the symlink.

## 根本原因
{
  "evidenceRefs": [
    "temp-file-ignored-on-load",
    "node-path-within-workspace",
    "predictable-following-temp-write"
  ],
  "summary": "`read_nodes()` accepts the valid `.md` node and ignores its `.md.tmp` sibling. On an edit, `save_files()` derives a predictable temporary path from the node ID and calls `fs::write()` without exclusive creation or a no-follow safeguard. An attacker-supplied symlink redirects the truncate/write outside the workspace before the later rename."
}

## 修正方針
Create a fresh unpredictable temporary file exclusively in the nodes directory, never follow an existing temporary path, write and close it, then atomically rename it over the intended node. Anchor operations to the directory if concurrent directory replacement is in scope.

## 検証
{
  "counterEvidence": [
    "Node IDs from remote input are restricted to lowercase alphanumerics by model.rs:29-31, which blocks lexical traversal but not this valid-name symlink.",
    "Attacker must supply a symlink-containing workspace or write within its nodes directory; there is no unauthenticated remote file-write claim."
  ],
  "limitations": [
    "No filesystem mutation or exploit reproduction was performed.",
    "This establishes content replacement/truncation with serialized node content; arbitrary machine-code execution is not claimed."
  ],
  "method": "static_source_review",
  "status": "validated",
  "summary": "Source tracing confirms ordinary CLI edits save changed nodes through this path. The planted .md.tmp file is not parsed during opening, and ID validation does not govern symlink targets. The sensitive operation is the initial write, so renaming afterwards does not prevent external file damage."
}

## 回帰確認
[
  "Save a changed node with a preexisting .md.tmp symlink and verify its external target remains unchanged.",
  "Verify failed temporary-file creation leaves existing node data intact."
]

## 場所
[
  {
    "endLine": 167,
    "path": "crates/topo-core/src/store.rs",
    "role": "root_control",
    "startLine": 162
  },
  {
    "endLine": 213,
    "path": "crates/topo-core/src/store.rs",
    "role": "entrypoint",
    "startLine": 203
  },
  {
    "endLine": 457,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 453
  },
  {
    "endLine": 196,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 189
  }
]

---

## [F031] Editing a supplied local workspace can overwrite a file outside it

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F031
findingId: csf_6bda17cd511ba321ceba3567
occurrenceId: occ_78f3b31f8010e5159c2dc095
重要度: low
共通原因: filesystem
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A workspace supplier can plant `<id>.md.tmp` as a symlink to a file writable by the victim. An ordinary edit of that task writes task metadata and notes through the link, truncating the outside file before renaming the temporary entry.

## 根本原因
{
  "evidenceRefs": [
    "node-read",
    "local-apply",
    "node-path",
    "temp-write",
    "notes-render"
  ],
  "summary": "The local loader reads only `.md` files, allowing an attacker-supplied `.md.tmp` symlink to survive opening. When a task changes, `save_files()` derives a fixed temporary path and calls `fs::write()` without exclusive/no-follow creation. The outside target is truncated before `rename()` updates the node entry, so atomic replacement of the node itself does not protect the temporary write."
}

## 修正方針
Create a new unpredictable temporary file exclusively in a trusted node directory, reject symlinked storage paths, write through its already-open handle, then atomically rename it into place. Never truncate a pre-existing predictable temporary path.

## 検証
{
  "counterEvidence": [
    "Requires local workspace delivery or write access to its node directory.",
    "Cloud graph-only operation does not use this path until a local save/export occurs.",
    "The outside file must be writable by the victim; no privilege escalation beyond that user's authority is claimed."
  ],
  "evidenceRefs": [
    "node-read",
    "local-apply",
    "temp-write",
    "node-path",
    "notes-render"
  ],
  "limitations": [
    "No planted symlink or destructive write was created.",
    "Attacker cannot choose arbitrary file bytes independently of serialized task metadata and body."
  ],
  "method": "independent static source trace",
  "summary": "Parent review confirmed that a valid `a.md` and an `a.md.tmp` symlink can coexist, that loading ignores the latter, and that editing task `a` writes through that exact path. Node ID validation and filename agreement do not constrain an existing symlink's target."
}

## 回帰確認
[
  "A pre-existing `<id>.md.tmp` symlink must not modify its outside target during task save.",
  "Saving must reject a symlinked node directory when workspace confinement is required."
]

## 場所
[
  {
    "endLine": 167,
    "path": "crates/topo-core/src/store.rs",
    "role": "root_control",
    "startLine": 162
  },
  {
    "endLine": 213,
    "path": "crates/topo-core/src/store.rs",
    "role": "entrypoint",
    "startLine": 203
  },
  {
    "endLine": 222,
    "path": "crates/topo-core/src/store.rs",
    "role": "sink",
    "startLine": 217
  }
]

---

## [F034] Editing a supplied workspace can overwrite a file outside its node directory

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F034
findingId: csf_a1f6f068d13375342169e0e7
occurrenceId: occ_1318b760ef1aed3a5a40b6c1
重要度: medium
共通原因: filesystem
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A workspace containing a valid node and a preexisting `<id>.md.tmp` symlink can redirect a normal status/edit save to an external victim-writable file. The full attacker-authored node contents overwrite that file before the temporary path is renamed.

## 根本原因
{
  "evidenceRefs": [
    "node-file-load",
    "node-parse-body",
    "status-save-entry",
    "local-apply-save",
    "temporary-symlink-write",
    "node-destination"
  ],
  "summary": "`read_nodes` accepts the `.md` node but skips the sibling `.md.tmp` symlink. After a status or edit operation, `save_files` chooses that predictable temporary name and calls `fs::write`, which follows an existing link and truncates its external target. The later rename changes the workspace path only after the external overwrite has happened. Node ID validation and filename matching do not address symlink resolution."
}

## 修正方針
Create each temporary node file with an unpredictable name and exclusive creation inside a trusted node directory, rejecting symlinks and unsafe directory resolution before writing. Write through the safely opened descriptor and only then atomically rename it into place.

## 検証
{
  "counterEvidence": [
    "Node IDs in product input paths are checked and loaded node filenames must match; lexical ../ traversal is not required.",
    "Replacing the final .md via rename does not protect the preceding write to the symlink's target.",
    "Remote-only workspaces do not call save_files until explicitly pulled to local files."
  ],
  "evidenceRefs": [
    "node-file-load",
    "node-parse-body",
    "status-save-entry",
    "local-apply-save",
    "temporary-symlink-write",
    "node-destination"
  ],
  "limitations": [
    "Requires a symlink-supporting checkout/filesystem and a predictable external target writable by the victim.",
    "No runtime filesystem reproduction was executed."
  ],
  "method": "independent_static_source_trace",
  "status": "validated",
  "summary": "A supplied valid node plus its temporary-file symlink survives workspace loading. Changing only the node status still rewrites its rendered YAML and Markdown body through the symlink. The target only needs to resolve to a file the victim can write; the attacker need not control that external file or win a timing race."
}

## 回帰確認
[
  "Saving a valid node with a preexisting .md.tmp symlink must leave the external target unchanged.",
  "Exercise status/edit and cloud pull paths with a symlink-bearing node directory.",
  "Verify temporary-file creation fails safely for symlinked node directories as well as file entries."
]

## 場所
[
  {
    "endLine": 213,
    "path": "crates/topo-core/src/store.rs",
    "role": "user_input",
    "startLine": 203
  },
  {
    "endLine": 241,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 234
  },
  {
    "endLine": 451,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 446
  },
  {
    "endLine": 189,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 181
  },
  {
    "endLine": 167,
    "path": "crates/topo-core/src/store.rs",
    "role": "root_control",
    "startLine": 162
  },
  {
    "endLine": 200,
    "path": "crates/topo-core/src/store.rs",
    "role": "expected_control",
    "startLine": 198
  },
  {
    "endLine": 195,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 189
  }
]

---

## [F042] Public graph mutation and save APIs permit writes outside the workspace

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F042
findingId: csf_512d30edbcf50d55b115f285
occurrenceId: occ_859381bf8bcd303140d2edbf
重要度: low
共通原因: node-id
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

The documented public-library composition accepts unvalidated string node IDs and later uses them as paths. A host consuming untrusted nodes through Graph::from_nodes or insert and Workspace::save can overwrite unrelated Markdown files outside its selected workspace.

## 根本原因
{
  "evidenceRefs": [
    "public-node-id",
    "id-grammar",
    "graph-construction",
    "graph-insertion",
    "documented-save-api",
    "local-save-route",
    "id-file-path",
    "external-write-delete"
  ],
  "summary": "NodeId exposes a String and derives transparent Deserialize, while Graph::from_nodes and Graph::insert omit NodeId::validate. Workspace documentation explicitly permits mutating its public graph and then saving. Local save constructs nodes_dir.join(format!(\"{id}.md\")) without a persistence validation boundary, so absolute or parent-containing identifiers select external paths. The same unchecked path helper governs later deletion after an unsafe ID has been successfully saved."
}

## 修正方針
Enforce node-ID grammar in construction/deserialization and graph insertion/loading. Validate all current and saved IDs before local persistence performs any writes or deletions, keeping validation all-or-nothing.

## 検証
{
  "counterEvidence": [
    "Bundled Op::Add, remote snapshot installation, and server import validate external IDs; local Markdown parsing binds IDs to immediate file stems.",
    "Explicit NodeId::validate in a library host defeats this path.",
    "Destinations normally end in .md; no suffix bypass, credentials.toml/.env overwrite, arbitrary-byte write, or direct code execution is claimed.",
    "Rendered bytes contain generated YAML frontmatter; Node deserialization skips body.",
    "External parent directory must exist and permit temporary-file creation and rename.",
    "Deletion requires an unsafe ID already in the private saved map after an earlier successful local save."
  ],
  "evidenceRefs": [
    "public-node-id",
    "id-grammar",
    "graph-construction",
    "graph-insertion",
    "documented-save-api",
    "local-save-route",
    "id-file-path",
    "external-write-delete"
  ],
  "limitations": [
    "No runtime or integrating deployment was inspected.",
    "The finding applies to the public-library composition, not a new bundled application input route."
  ],
  "method": "independent_static_source_trace",
  "status": "validated",
  "summary": "Read public NodeId/Node types, graph constructors, Workspace contract and complete file persistence. A node without references satisfies all checked graph invariants despite an unsafe ID. The documented public-library composition reaches write/rename outside nodes_dir. After a successful save, omitting that ID from a later graph also reaches remove_file with the escaped path. An independent investigator confirmed this distinct library boundary."
}

## 回帰確認
[
  "Reject absolute and parent-containing IDs through public graph construction/insertion and local save before filesystem mutation.",
  "Verify normal safe-ID saves and removals preserve behavior."
]

## 場所
[
  {
    "endLine": 12,
    "path": "crates/topo-core/src/model.rs",
    "role": "user_input",
    "startLine": 8
  },
  {
    "endLine": 32,
    "path": "crates/topo-core/src/model.rs",
    "role": "propagation",
    "startLine": 27
  },
  {
    "endLine": 47,
    "path": "crates/topo-core/src/graph.rs",
    "role": "propagation",
    "startLine": 35
  },
  {
    "endLine": 272,
    "path": "crates/topo-core/src/graph.rs",
    "role": "propagation",
    "startLine": 264
  },
  {
    "endLine": 55,
    "path": "crates/topo-core/src/store.rs",
    "role": "entrypoint",
    "startLine": 45
  },
  {
    "endLine": 147,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 142
  },
  {
    "endLine": 200,
    "path": "crates/topo-core/src/store.rs",
    "role": "root_control",
    "startLine": 198
  },
  {
    "endLine": 174,
    "path": "crates/topo-core/src/store.rs",
    "role": "sink",
    "startLine": 162
  }
]

---

## [F044] Editing an untrusted workspace can overwrite files outside the workspace

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F044
findingId: csf_68f432e0a4c50fea9118cd31
occurrenceId: occ_6ffae992e965ef2da6504a03
重要度: medium
共通原因: filesystem
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

An untrusted workspace can contain a valid node plus a symlink at its predictable <id>.md.tmp sibling. Ordinary CLI/GUI edits or local materialization/export follow that link, truncating and replacing a selected victim-writable outside file with serialized node metadata and notes before renaming the temporary entry.

## 根本原因
{
  "evidenceRefs": [
    "node-read",
    "local-apply",
    "temp-write",
    "node-path",
    "notes-render"
  ],
  "summary": "read_nodes loads only .md entries and ignores the planted .md.tmp symlink. save_files derives that temporary path from a normal validated node ID and calls fs::write without exclusive creation or no-follow protection; the write follows the existing link and truncates its outside target. The subsequent rename protects neither the already-completed external write nor the temporary path. Metadata and free-form body from render(node) determine the written bytes; lexical NodeId validation does not constrain a symlink target."
}

## 修正方針
Create a fresh unpredictable temporary file exclusively in a trusted nodes directory, write through its already-open handle and close it, then atomically rename it over the intended node. Never truncate a pre-existing predictable temporary path. Reject symlinked storage paths where workspace confinement is required and anchor directory operations when concurrent directory replacement is in scope.

## 検証
{
  "counterEvidence": [
    "Requires local workspace delivery or write access to its node directory.",
    "Cloud graph-only operation does not use this path until a local save/export occurs.",
    "The outside file must be writable by the victim; no privilege escalation beyond that user's authority is claimed.",
    "Node IDs from remote input are restricted to lowercase alphanumerics by model.rs:29-31, which blocks lexical traversal but not this valid-name symlink.",
    "Attacker must supply a symlink-containing workspace or write within its nodes directory; there is no unauthenticated remote file-write claim."
  ],
  "evidenceRefs": [
    "node-read",
    "local-apply",
    "temp-write",
    "node-path",
    "notes-render"
  ],
  "limitations": [
    "No planted symlink or destructive write was created.",
    "Attacker cannot choose arbitrary file bytes independently of serialized task metadata and body.",
    "No filesystem mutation or exploit reproduction was performed.",
    "This establishes content replacement/truncation with serialized node content; arbitrary machine-code execution is not claimed."
  ],
  "method": "semantic reduction of already-validated static source findings; no additional validation",
  "status": "validated",
  "summary": "Both reviews describe the same unchecked temporary-file write. A valid a.md and a.md.tmp symlink coexist without the latter being parsed; editing a writes through the exact temporary path before rename. Normal node IDs and filename agreement prevent lexical traversal but do not protect this target. Cloud graph-only operations reach it only upon local save/export or cloud-pull materialization."
}

## 回帰確認
[
  "A pre-existing `<id>.md.tmp` symlink must not modify its outside target during task save.",
  "Saving must reject a symlinked node directory when workspace confinement is required.",
  "Save a changed node with a preexisting .md.tmp symlink and verify its external target remains unchanged.",
  "Verify failed temporary-file creation leaves existing node data intact."
]

## 場所
[
  {
    "endLine": 167,
    "path": "crates/topo-core/src/store.rs",
    "role": "root_control",
    "startLine": 162
  },
  {
    "endLine": 213,
    "path": "crates/topo-core/src/store.rs",
    "role": "entrypoint",
    "startLine": 203
  },
  {
    "endLine": 222,
    "path": "crates/topo-core/src/store.rs",
    "role": "sink",
    "startLine": 217
  },
  {
    "endLine": 457,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 453
  },
  {
    "endLine": 196,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 189
  }
]

---

## [F048] Editing an untrusted workspace can overwrite a file outside it

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F048
findingId: csf_21fa90ff98f748eb287c7e52
occurrenceId: occ_5a3f470d6b22a0dda24275a1
重要度: medium
共通原因: filesystem
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

The local node save path writes to predictable `{id}.md.tmp` filenames using `fs::write()`. A workspace author can pre-position that temporary filename as a symlink, so an ordinary node edit truncates and overwrites an external file writable by the victim.

## 根本原因
{
  "evidenceRefs": [
    "uninspected-temp-entry",
    "node-mutation-entry",
    "workspace-save-dispatch",
    "node-path",
    "predictable-temp-write"
  ],
  "summary": "A workspace-provided `{id}.md.tmp` entry survives loading because `read_nodes()` reads only `.md` files. After a node changes, `save_files()` derives that exact temporary path and calls `fs::write()` without exclusive creation or rejecting symlinks. The write follows the symlink and truncates its outside target before the later rename. Joining a filename under `.topo/nodes` does not contain the actual opened resource."
}

## 修正方針
Create a fresh temporary file exclusively in a verified nodes directory, write through its already-open handle, and atomically rename it. Reject symlinked workspace directories or resolve them against an explicit trusted root; never reuse a predictable temporary entry with a symlink-following open.

## 検証
{
  "counterEvidence": [
    "Read-only commands do not call save_files.",
    "The target must be writable by the victim; this does not exceed their OS permissions.",
    "Git on platforms that check out symlinks as plain files will not follow the attacker-supplied entry.",
    "Cloud-backed saves send operations to a remote and do not take this local file path."
  ],
  "evidenceRefs": [
    "node-mutation-entry",
    "workspace-save-dispatch",
    "predictable-temp-write",
    "uninspected-temp-entry",
    "node-path"
  ],
  "limitations": [
    "Validated from source; no malicious checkout or filesystem writes were executed.",
    "Reliable code execution is not established; the demonstrated effect is outside-file truncation/overwrite with serialized node content."
  ],
  "method": "static source trace",
  "status": "validated",
  "summary": "A valid node and a matching .md.tmp symlink can coexist in the supplied workspace. Editing that node reaches fs::write on the symlink before fs::rename, so the target has already been modified even if rename fails. Valid node IDs, valid graph structure and the destination directory do not prevent this alternate filesystem resolution."
}

## 回帰確認
[
  "A pre-existing {id}.md.tmp symlink causes save to fail safely or use a fresh exclusive temporary file, leaving the outside target unchanged.",
  "Normal local node edits still replace only their node file."
]

## 場所
[
  {
    "endLine": 457,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 453
  },
  {
    "endLine": 147,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 143
  },
  {
    "endLine": 168,
    "path": "crates/topo-core/src/store.rs",
    "role": "root_control",
    "startLine": 162
  },
  {
    "endLine": 213,
    "path": "crates/topo-core/src/store.rs",
    "role": "user_input",
    "startLine": 203
  },
  {
    "endLine": 200,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 198
  }
]

---

## [F053] Saving an imported workspace can overwrite a file outside it

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F053
findingId: csf_bcd9663da01aea7d5d493fc3
occurrenceId: occ_10da54ce3e60e3174c603072
重要度: low
共通原因: import-authorization
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

An attacker-supplied local workspace can include a symlink at a node's predictable `.md.tmp` path. A normal status or edit operation follows that symlink and overwrites a victim-writable file outside the workspace with the rendered node before renaming the temporary path.

## 根本原因
{
  "evidenceRefs": [
    "save-local-open",
    "save-status",
    "save-local-apply",
    "save-filename",
    "save-symlink-write",
    "save-body"
  ],
  "summary": "`read_nodes()` ignores `.md.tmp` entries while opening a supplied workspace. Saving a changed node derives that predictable path and uses `fs::write()`, which follows a preexisting symlink and truncates its target. The rendered node includes the supplied Markdown body. Renaming the temporary entry happens only after the external target has already been overwritten."
}

## 修正方針
Create a random temporary file exclusively inside the node directory with `create_new` semantics, write using its open handle, and rename it into place. Refuse preexisting temporary links and use directory-relative operations if directory links are also within the supported untrusted-workspace model.

## 検証
{
  "counterEvidence": [
    "Cloud graph input alone cannot create the symlink.",
    "The target must be writable by the victim process and known/addressable to the attacker.",
    "The output contains node YAML frontmatter, so arbitrary target overwrite is established; unrestricted exact-byte write or code execution is not asserted.",
    "A final `.md` symlink is replaced by rename, and removal unlinks the entry, so those siblings do not have this temporary-file sink."
  ],
  "evidenceRefs": [
    "save-local-open",
    "save-status",
    "save-local-apply",
    "save-symlink-write",
    "save-filename",
    "save-body"
  ],
  "limitations": [
    "Requires attacker-controlled local filesystem layout, such as an imported or Git-shared workspace.",
    "No target file was accessed or modified."
  ],
  "method": "static source trace",
  "summary": "A valid `a.md` plus a symlink `a.md.tmp` is accepted on local load. `topo status a done` reaches the shared save path, whose write follows the temporary symlink. Node ID validation does not prevent a link at the legitimate derived name, and the final rename does not undo target corruption."
}

## 回帰確認
[
  "Saving with a preexisting `<id>.md.tmp` symlink must leave its external target unchanged.",
  "Verify normal local save and cloud pull still replace only intended node files."
]

## 場所
[
  {
    "endLine": 213,
    "path": "crates/topo-core/src/store.rs",
    "role": "user_input",
    "startLine": 203
  },
  {
    "endLine": 451,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 446
  },
  {
    "endLine": 188,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 181
  },
  {
    "endLine": 167,
    "path": "crates/topo-core/src/store.rs",
    "role": "root_control",
    "startLine": 162
  },
  {
    "endLine": 200,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 198
  },
  {
    "endLine": 222,
    "path": "crates/topo-core/src/store.rs",
    "role": "sink",
    "startLine": 217
  }
]

---

## [F059] Saving or deleting supplied nodes can modify files outside the workspace

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F059
findingId: csf_d034150a24a431d0bf20757b
occurrenceId: occ_03aabf4661ce8bc58185903d
重要度: medium
共通原因: filesystem
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

An untrusted workspace can plant a predictable <id>.md.tmp symlink: ordinary CLI/GUI/TUI edits or local materialization/export follow it, truncating a victim-writable outside file with serialized node metadata and notes before rename. A symlinked .topo/nodes directory independently redirects reads, saves, and deleted-node removal into an external valid node store.

## 根本原因
{
  "evidenceRefs": [
    "node-read",
    "local-apply",
    "temp-write",
    "node-path",
    "notes-render",
    "symlink-loader",
    "symlink-write",
    "symlink-cli-save",
    "symlink-directory-identity",
    "symlink-directory-delete"
  ],
  "summary": "read_nodes loads only .md entries and ignores the planted .md.tmp symlink. save_files derives that temporary path from a normal validated node ID and calls fs::write without exclusive creation or no-follow protection; the write follows the existing link and truncates its outside target. The subsequent rename protects neither the already-completed external write nor the temporary path. Metadata and free-form body from render(node) determine the written bytes; lexical NodeId validation does not constrain a symlink target. The incoming source also establishes that nodes_dir retains the supplied directory entry and node_path/remove_file follow a linked nodes directory. That variant redirects save and removal even without a staging-file symlink, provided the external nodes parse successfully. A link only at the final .md entry is replaced by rename and is not the proven direct outside-file overwrite."
}

## 修正方針
Create a fresh unpredictable temporary file exclusively in a trusted nodes directory, write through its already-open handle and close it, then atomically rename it over the intended node. Never truncate a pre-existing predictable temporary path. Reject symlinked storage paths where workspace confinement is required and anchor directory operations when concurrent directory replacement is in scope.

## 検証
{
  "counterEvidence": [
    "Requires local workspace delivery or write access to its node directory.",
    "Cloud graph-only operation does not use this path until a local save/export occurs.",
    "The outside file must be writable by the victim; no privilege escalation beyond that user's authority is claimed.",
    "Node IDs from remote input are restricted to lowercase alphanumerics by model.rs:29-31, which blocks lexical traversal but not this valid-name symlink.",
    "Attacker must supply a symlink-containing workspace or write within its nodes directory; there is no unauthenticated remote file-write claim.",
    "Node IDs are constrained at application mutation/import boundaries, so this attack does not need path traversal.",
    "A symlink only at the final .md pathname is replaced by the rename; the proven direct external overwrite uses the staging path.",
    "Ordinary OS file permissions constrain the destination; this is not OS privilege escalation.",
    "Directory redirection requires an external directory whose nodes parse successfully; the victim must mutate or delete a loaded node."
  ],
  "evidenceRefs": [
    "node-read",
    "local-apply",
    "temp-write",
    "node-path",
    "notes-render",
    "symlink-loader",
    "symlink-write",
    "symlink-cli-save",
    "symlink-directory-identity",
    "symlink-directory-delete"
  ],
  "limitations": [
    "No planted symlink or destructive write was created.",
    "Attacker cannot choose arbitrary file bytes independently of serialized task metadata and body.",
    "No filesystem mutation or exploit reproduction was performed.",
    "This establishes content replacement/truncation with serialized node content; arbitrary machine-code execution is not claimed.",
    "No application execution or file-write payload was attempted.",
    "Victim processing must preserve supplied symlinks and change a matching node."
  ],
  "method": "semantic reduction of already-validated static source findings; no additional validation",
  "status": "validated",
  "summary": "All three sources establish the predictable staging-file write and its ignored .md.tmp entry. The incoming source additionally establishes linked-directory redirection through nodes_dir, node_path, and deleted-node remove_file; its external store must contain valid loadable nodes. Identifier restrictions prevent lexical traversal but neither link variant. Cloud graph-only operation reaches these sinks only during local save/export/pull materialization. All conclusions are supplied static findings; no reducer write or reproduction occurred."
}

## 回帰確認
[
  "A pre-existing `<id>.md.tmp` symlink must not modify its outside target during task save.",
  "Saving must reject a symlinked node directory when workspace confinement is required.",
  "Save a changed node with a preexisting .md.tmp symlink and verify its external target remains unchanged.",
  "Verify failed temporary-file creation leaves existing node data intact.",
  "A supplied <id>.md.tmp symlink must not change its external target after a normal node status/edit operation.",
  "Verify valid local saves remain atomic per node and reject linked nodes directories that escape the intended store.",
  "A linked nodes directory targeting another valid workspace must not write or delete that workspace's node files."
]

## 場所
[
  {
    "endLine": 167,
    "path": "crates/topo-core/src/store.rs",
    "role": "root_control",
    "startLine": 162
  },
  {
    "endLine": 213,
    "path": "crates/topo-core/src/store.rs",
    "role": "entrypoint",
    "startLine": 203
  },
  {
    "endLine": 222,
    "path": "crates/topo-core/src/store.rs",
    "role": "sink",
    "startLine": 217
  },
  {
    "endLine": 457,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 453
  },
  {
    "endLine": 196,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 189
  },
  {
    "endLine": 211,
    "path": "crates/topo-core/src/store.rs",
    "role": "user_input",
    "startLine": 203
  },
  {
    "endLine": 188,
    "path": "crates/topo-core/src/store.rs",
    "role": "entrypoint",
    "startLine": 181
  },
  {
    "endLine": 200,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 198
  },
  {
    "endLine": 328,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 314
  },
  {
    "endLine": 106,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 103
  },
  {
    "endLine": 174,
    "path": "crates/topo-core/src/store.rs",
    "role": "sink",
    "startLine": 169
  },
  {
    "endLine": 171,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 162
  },
  {
    "endLine": 213,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 198
  },
  {
    "endLine": 325,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 313
  },
  {
    "endLine": 460,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 446
  },
  {
    "endLine": 64,
    "path": "crates/topo-cli/src/tui.rs",
    "role": "entrypoint",
    "startLine": 61
  },
  {
    "endLine": 343,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 341
  },
  {
    "endLine": 381,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 377
  },
  {
    "endLine": 195,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 189
  },
  {
    "endLine": 210,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 198
  },
  {
    "endLine": 222,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 217
  },
  {
    "endLine": 457,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 446
  }
]

---

## [F062] Editing a shared workspace can overwrite files outside its directory

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F062
findingId: csf_a63ae353ef91f9b198f85bee
occurrenceId: occ_b8cc94b8dfb3ad2ac22c4196
重要度: medium
共通原因: filesystem
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A shared local workspace can contain a valid node and a symlink at its predictable `<id>.md.tmp` path. Updating that node makes `Workspace::save_files` follow the symlink and overwrite its target with rendered Markdown before renaming the link.

## 根本原因
{
  "evidenceRefs": [
    "load-markdown-only",
    "status-applies-operation",
    "local-apply-save",
    "predictable-temp-write"
  ],
  "summary": "The loader accepts the normal Markdown node while skipping its `.md.tmp` sibling. A routine mutation calls the local save path, which derives that exact temporary filename and uses `fs::write` without exclusive creation or a symlink restriction. The write follows the attacker-supplied link into a victim-writable file outside `.topo/nodes`; subsequent rename preserves the link rather than restoring the target."
}

## 修正方針
Create a new temporary file exclusively in the verified node directory, write through its returned handle, and atomically rename it. Reject existing symlink/directory entries and avoid reusable attacker-selected temporary paths.

## 検証
{
  "counterEvidence": [
    "Node IDs from remote snapshots and imports are restricted to alphanumeric IDs, but the attack uses a valid ID.",
    "The victim must perform a write; read-only graph commands alone do not trigger this sink.",
    "The attacker cannot overwrite files outside the victim's existing OS write permissions."
  ],
  "evidenceRefs": [
    "load-markdown-only",
    "status-applies-operation",
    "local-apply-save",
    "predictable-temp-write"
  ],
  "limitations": [
    "Symlink-preserving workspace distribution is required, generally Unix or suitably configured Windows.",
    "No code-execution claim or runtime reproduction is made; external file truncation/overwrite is directly supported."
  ],
  "method": "Static attacker-to-filesystem trace",
  "status": "validated",
  "summary": "A planted temporary-file symlink is reachable without controlling the victim account: the attacker distributes workspace content, the victim performs an ordinary node edit, and the standard write follows the existing link. The filename/ID check covers the .md node, not the .tmp sink."
}

## 回帰確認
[
  "Preplant `<id>.md.tmp` as a symlink to a file outside the workspace; editing the node must leave the target unchanged.",
  "Normal edits and cloud pull must still atomically replace Markdown nodes."
]

## 場所
[
  {
    "endLine": 167,
    "path": "crates/topo-core/src/store.rs",
    "role": "root_control",
    "startLine": 162
  },
  {
    "endLine": 211,
    "path": "crates/topo-core/src/store.rs",
    "role": "user_input",
    "startLine": 203
  },
  {
    "endLine": 188,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 181
  },
  {
    "endLine": 450,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 446
  },
  {
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 343
  },
  {
    "endLine": 195,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 191
  },
  {
    "path": "crates/topo-cli/src/tui.rs",
    "role": "entrypoint",
    "startLine": 63
  },
  {
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 324
  },
  {
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 423
  },
  {
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 563
  },
  {
    "endLine": 395,
    "path": "crates/topo-gui/src/main.rs",
    "role": "propagation",
    "startLine": 379
  },
  {
    "endLine": 439,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 438
  }
]

---

## [F067] Saving or deleting supplied nodes can modify files outside the workspace

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F067
findingId: csf_44e6f7274a5339c1795f5346
occurrenceId: occ_3ba76b2bb88a0bb1c2a6bce9
重要度: medium
共通原因: filesystem
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

An untrusted workspace can plant a predictable <id>.md.tmp symlink: ordinary CLI/GUI/TUI edits or local materialization/export follow it, truncating a victim-writable outside file with serialized node metadata and notes before rename. A symlinked .topo/nodes directory independently redirects reads, saves, and deleted-node removal into an external valid node store.

## 根本原因
{
  "evidenceRefs": [
    "node-read",
    "local-apply",
    "temp-write",
    "node-path",
    "notes-render",
    "symlink-loader",
    "symlink-write",
    "symlink-cli-save",
    "symlink-directory-identity",
    "symlink-directory-delete",
    "dedup-0003-2-temp-write-follows-link",
    "dedup-0003-2-attacker-body"
  ],
  "summary": "read_nodes loads only .md entries and ignores the planted .md.tmp symlink. save_files derives that temporary path from a normal validated node ID and calls fs::write without exclusive creation or no-follow protection; the write follows the existing link and truncates its outside target. The subsequent rename protects neither the already-completed external write nor the temporary path. Metadata and free-form body from render(node) determine the written bytes; lexical NodeId validation does not constrain a symlink target. The incoming source also establishes that nodes_dir retains the supplied directory entry and node_path/remove_file follow a linked nodes directory. That variant redirects save and removal even without a staging-file symlink, provided the external nodes parse successfully. A link only at the final .md entry is replaced by rename and is not the proven direct outside-file overwrite."
}

## 修正方針
Create a fresh unpredictable temporary file exclusively in a trusted nodes directory, write through its already-open handle and close it, then atomically rename it over the intended node. Never truncate a pre-existing predictable temporary path. Reject symlinked storage paths where workspace confinement is required and anchor directory operations when concurrent directory replacement is in scope.

## 検証
{
  "counterEvidence": [
    "Requires local workspace delivery or write access to its node directory.",
    "Cloud graph-only operation does not use this path until a local save/export occurs.",
    "The outside file must be writable by the victim; no privilege escalation beyond that user's authority is claimed.",
    "Node IDs from remote input are restricted to lowercase alphanumerics by model.rs:29-31, which blocks lexical traversal but not this valid-name symlink.",
    "Attacker must supply a symlink-containing workspace or write within its nodes directory; there is no unauthenticated remote file-write claim.",
    "Node IDs are constrained at application mutation/import boundaries, so this attack does not need path traversal.",
    "A symlink only at the final .md pathname is replaced by the rename; the proven direct external overwrite uses the staging path.",
    "Ordinary OS file permissions constrain the destination; this is not OS privilege escalation.",
    "Directory redirection requires an external directory whose nodes parse successfully; the victim must mutate or delete a loaded node.",
    "Reading alone does not trigger the write; the victim must modify and save the specific task.",
    "The target requires an existing parent and victim write permission.",
    "This does not assume the attacker already controls a user process or global configuration; supplying a lower-trust workspace filesystem is sufficient.",
    "Node ID checks prevent string traversal, and rename provides ordinary replacement atomicity, but both occur independently of the already-followed temporary link."
  ],
  "evidenceRefs": [
    "node-read",
    "local-apply",
    "temp-write",
    "node-path",
    "notes-render",
    "symlink-loader",
    "symlink-write",
    "symlink-cli-save",
    "symlink-directory-identity",
    "symlink-directory-delete",
    "dedup-0003-2-temp-write-follows-link",
    "dedup-0003-2-attacker-body"
  ],
  "limitations": [
    "No planted symlink or destructive write was created.",
    "Attacker cannot choose arbitrary file bytes independently of serialized task metadata and body.",
    "No filesystem mutation or exploit reproduction was performed.",
    "This establishes content replacement/truncation with serialized node content; arbitrary machine-code execution is not claimed.",
    "No application execution or file-write payload was attempted.",
    "Victim processing must preserve supplied symlinks and change a matching node.",
    "No filesystem exploit was executed.",
    "Unix symlink support supplies a concrete platform; Windows symlink/reparse prerequisites and later code execution depend on host configuration."
  ],
  "method": "semantic reduction of already-validated Standard source findings; no additional validation",
  "status": "validated",
  "summary": "All four sources establish the predictable staging-file write and its ignored .md.tmp entry. A previously absorbed source additionally establishes linked-directory redirection through nodes_dir, node_path, and deleted-node remove_file; its external store must contain valid loadable nodes. Identifier restrictions prevent lexical traversal but neither link variant. Cloud graph-only operation reaches these sinks only during local save/export/pull materialization. All conclusions are supplied static findings; no reducer write or reproduction occurred."
}

## 回帰確認
[
  "A pre-existing `<id>.md.tmp` symlink must not modify its outside target during task save.",
  "Saving must reject a symlinked node directory when workspace confinement is required.",
  "Save a changed node with a preexisting .md.tmp symlink and verify its external target remains unchanged.",
  "Verify failed temporary-file creation leaves existing node data intact.",
  "A supplied <id>.md.tmp symlink must not change its external target after a normal node status/edit operation.",
  "Verify valid local saves remain atomic per node and reject linked nodes directories that escape the intended store.",
  "A linked nodes directory targeting another valid workspace must not write or delete that workspace's node files.",
  "Place a symlink at the legacy node temporary path and assert saving neither follows it nor changes its target.",
  "Verify ordinary saves still replace node files atomically and remove their temporary files.",
  "Check directory-link handling for supported platforms."
]

## 場所
[
  {
    "endLine": 167,
    "path": "crates/topo-core/src/store.rs",
    "role": "root_control",
    "startLine": 162
  },
  {
    "endLine": 213,
    "path": "crates/topo-core/src/store.rs",
    "role": "entrypoint",
    "startLine": 203
  },
  {
    "endLine": 222,
    "path": "crates/topo-core/src/store.rs",
    "role": "sink",
    "startLine": 217
  },
  {
    "endLine": 457,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 453
  },
  {
    "endLine": 196,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 189
  },
  {
    "endLine": 211,
    "path": "crates/topo-core/src/store.rs",
    "role": "user_input",
    "startLine": 203
  },
  {
    "endLine": 188,
    "path": "crates/topo-core/src/store.rs",
    "role": "entrypoint",
    "startLine": 181
  },
  {
    "endLine": 200,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 198
  },
  {
    "endLine": 328,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 314
  },
  {
    "endLine": 106,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 103
  },
  {
    "endLine": 174,
    "path": "crates/topo-core/src/store.rs",
    "role": "sink",
    "startLine": 169
  },
  {
    "endLine": 171,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 162
  },
  {
    "endLine": 213,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 198
  },
  {
    "endLine": 325,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 313
  },
  {
    "endLine": 460,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 446
  },
  {
    "endLine": 64,
    "path": "crates/topo-cli/src/tui.rs",
    "role": "entrypoint",
    "startLine": 61
  },
  {
    "endLine": 343,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 341
  },
  {
    "endLine": 381,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 377
  },
  {
    "endLine": 195,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 189
  },
  {
    "endLine": 210,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 198
  },
  {
    "endLine": 222,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 217
  },
  {
    "endLine": 457,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 446
  },
  {
    "endLine": 213,
    "path": "crates/topo-core/src/store.rs",
    "role": "entrypoint",
    "startLine": 198
  },
  {
    "endLine": 222,
    "path": "crates/topo-core/src/store.rs",
    "role": "user_input",
    "startLine": 216
  },
  {
    "endLine": 449,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 446
  }
]


## 対応結果（2026-10-03）
結果: fixed。ディレクトリのhandleを基準に読み書きし、排他的な一時ファイルで置換。root/nodesリンク、外部Markdown/configリンク、不正NodeIdを拒否。private一時ファイルは作成時0600、名前はOS乱数。symlink/hardlink/途中差し替え/通常Markdown編集を実行確認。
最終検証: cargo test --locked --workspace 116成功/既存3ignore、fmt、native/Worker clippy -D warnings、local Worker/D1 smoke成功。詳細: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/artifacts-6a4e5a1e5a63564edc8ca8693b4bf5cc93fdb5baaaf0c17a2de44b08336f148b/artifacts/report_fix_20261003.md
