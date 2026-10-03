---
id: wgpiyq
kind: task
title: クラウド認証情報の平文HTTP送信を防ぐ
status: done
tags:
- security
- http-transport
- 44dbfab6-0d90-475d-b2a3-4df6534779e5
milestones:
- f0ehc5
---

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
統合した項目: F013
同じ原因の報告と未検証候補を追跡する。報告件数は独立した脆弱性数ではない。

---

## [F013] Cloud credentials can be sent over non-loopback HTTP

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F013
findingId: csf_b137c4c0fa2553278aa4712c
occurrenceId: occ_02c613b03ceb25c070a9c6dd
重要度: low
共通原因: http-transport
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

The cloud client accepts HTTP destinations without restricting them to loopback. If an operator configures a non-loopback HTTP server, the client sends reusable bearer credentials and GitHub sign-in token bodies in cleartext, exposing them to observers on that network path.

## 根本原因
{
  "evidenceRefs": [
    "http-client",
    "http-send",
    "http-integration",
    "login-send"
  ],
  "summary": "The authenticated client stores arbitrary URL strings and sends secrets without a required-HTTPS or explicit loopback-exception control. Configuring TLS verification protects HTTPS but does not upgrade HTTP."
}

## 修正方針
Require HTTPS before attaching credentials or sign-in bodies to any non-loopback destination. Permit HTTP only through a deliberate loopback development exception, with parsed canonical host handling.

## 検証
{
  "counterEvidence": [
    "HTTPS uses the platform certificate verifier.",
    "Production documentation uses HTTPS.",
    "Loopback HTTP is useful for local development and is not the reported exposure.",
    "Source tests exercise ordinary HTTP through the same Client; no network eavesdropping was executed."
  ],
  "evidenceRefs": [
    "http-client",
    "http-send",
    "http-integration",
    "login-send"
  ],
  "limitations": [
    "No non-loopback HTTP deployment is established by this checkout.",
    "Live transport and dependency behavior were not executed."
  ],
  "method": "static source trace",
  "summary": "Source establishes the stated chain and prerequisites. No application code was executed."
}

## 回帰確認
[
  "Non-loopback HTTP must fail before any bearer header or GitHub token body is sent.",
  "Verify HTTPS certificate verification remains enabled and explicitly allowed loopback development works."
]

## 場所
[
  {
    "endLine": 39,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "root_control",
    "startLine": 31
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "endLine": 41,
    "path": "crates/topo-cli/tests/cloud.rs",
    "role": "entrypoint",
    "startLine": 28
  },
  {
    "endLine": 130,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 124
  },
  {
    "endLine": 15,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "propagation",
    "startLine": 11
  },
  {
    "endLine": 39,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "propagation",
    "startLine": 30
  },
  {
    "endLine": 130,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "propagation",
    "startLine": 128
  }
]


## 対応結果（2026-10-03）
結果: fixed。HTTPSを共通送信境界で必須とし、IPv4/IPv6/localhost loopback開発だけHTTPを許可。redirectを追わずGitHub/Bearer転送を防止。偽loopback等の拒否・通常local API・D1 smoke成功。
最終検証: cargo test --locked --workspace 116成功/既存3ignore、fmt、native/Worker clippy -D warnings、local Worker/D1 smoke成功。詳細: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/artifacts-6a4e5a1e5a63564edc8ca8693b4bf5cc93fdb5baaaf0c17a2de44b08336f148b/artifacts/report_fix_20261003.md
