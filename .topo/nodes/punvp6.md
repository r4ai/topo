---
id: punvp6
kind: task
title: 可変GitHubログインによる誤ったメンバー失効を防ぐ
status: done
tags:
- security
- member-identity
- 44dbfab6-0d90-475d-b2a3-4df6534779e5
milestones:
- f0ehc5
---

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
統合した項目: F015
同じ原因の報告と未検証候補を追跡する。報告件数は独立した脆弱性数ではない。

---

## [F015] Removing a reused GitHub login can revoke the wrong workspace member

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F015
findingId: csf_1761616718448c93d3c00c02
occurrenceId: occ_a340bbd2e010656550418282
重要度: low
共通原因: member-identity
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

Member removal resolves its target by the first case-insensitive cached login, while memberships use stable GitHub IDs and the login cache is nonunique. After a rename and login reuse, multiple members can have the same cached login; removal can succeed for the wrong ID and leave the intended account authorized.

## 根本原因
{
  "evidenceRefs": [
    "fresh-github-signin",
    "cache-update",
    "nonunique-login",
    "member-cache-query",
    "remove-cache-match"
  ],
  "summary": "The supplied login is a display identifier that can change. Sign-in and invitations update only one row keyed by GitHub ID, leaving older cached logins unchanged, and the schema allows duplicates. `sql::members()` returns those display names with stable IDs. `remove()` uses `.find()` rather than resolving the current GitHub identity or rejecting ambiguity, so it can delete a different stable ID from the one the owner intended."
}

## 修正方針
Address membership removals by immutable GitHub user ID and expose that ID in member listings. If retaining login-based removal, resolve the current login to its stable ID and reject ambiguous cached matches.

## 検証
{
  "counterEvidence": [
    "Only an owner with an unrestricted token can remove members.",
    "A rename does not by itself transfer membership: authorization uses immutable user IDs.",
    "PUT resolves GitHub identity, and updating stale cached data can eliminate the collision.",
    "Tie ordering is unspecified; a given request can select the intended ID, the wrong ID, or hit last-owner protection depending on rows."
  ],
  "evidenceRefs": [
    "remove-cache-match",
    "cache-update",
    "member-cache-query",
    "nonunique-login",
    "fresh-github-signin"
  ],
  "limitations": [
    "External GitHub login-reuse behavior is a stated prerequisite; no live account or API operation was performed.",
    "This is not authentication impersonation or automatic cross-tenant access."
  ],
  "method": "static source trace",
  "status": "validated",
  "summary": "With two member IDs sharing cached login x after rename/reuse, DELETE members/x selects only the first returned row and issues delete_member for that ID. The remaining row retains membership and its bearer token still passes stable-ID authorization. No unique-login constraint or ambiguity check repairs the mismatch."
}

## 回帰確認
[
  "Two different member IDs with a stale/reused cached login cannot cause first-match removal of the unintended ID.",
  "A login rename does not prevent removal by stable identity."
]

## 場所
[
  {
    "endLine": 104,
    "path": "crates/topo-server/src/routes/members.rs",
    "role": "root_control",
    "startLine": 95
  },
  {
    "endLine": 139,
    "path": "crates/topo-server/src/sql.rs",
    "role": "propagation",
    "startLine": 132
  },
  {
    "endLine": 276,
    "path": "crates/topo-server/src/sql.rs",
    "role": "propagation",
    "startLine": 263
  },
  {
    "endLine": 5,
    "path": "crates/topo-server/migrations/0001_init.sql",
    "role": "user_input",
    "startLine": 1
  },
  {
    "endLine": 35,
    "path": "crates/topo-server/src/routes/auth.rs",
    "role": "entrypoint",
    "startLine": 30
  }
]


## 対応結果（2026-10-03）
結果: fixed。loginを現在のGitHub IDへ解決して除名。membersにstable user_idを追加し、by-id API/CLI --user-idでrename/削除済みaccountも扱う。login再割当・ID除名・owner保持・OpenAPI確認成功。
最終検証: cargo test --locked --workspace 116成功/既存3ignore、fmt、native/Worker clippy -D warnings、local Worker/D1 smoke成功。詳細: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/artifacts-6a4e5a1e5a63564edc8ca8693b4bf5cc93fdb5baaaf0c17a2de44b08336f148b/artifacts/report_fix_20261003.md
