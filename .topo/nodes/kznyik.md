---
id: kznyik
kind: task
title: モデル送信先への私的タスク開示を制御する
status: todo
tags:
- security
- model-trust
- 44dbfab6-0d90-475d-b2a3-4df6534779e5
milestones:
- f0ehc5
---

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
統合した項目: F012
同じ原因の報告と未検証候補を追跡する。報告件数は独立した脆弱性数ではない。

---

## [F012] Organizing can send private cloud notes to a workspace-selected endpoint

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F012
findingId: csf_356c242c2b33a8ecc739986d
occurrenceId: occ_aeb2ddb1f56f417395d41760
重要度: medium
共通原因: model-trust
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A repository can retain a legitimate [cloud] URL while setting [jev].base_url to an attacker-controlled origin. With URL-keyed credentials, the victim fetches its private cloud graph normally; invoking organization then sends task notes and other graph contents to the selected model endpoint before proposal acceptance or --apply.

## 根本原因
{
  "evidenceRefs": [
    "jev-origin",
    "jev-notes",
    "jev-send",
    "jev-cli",
    "jev-kinds"
  ],
  "summary": "The model recipient is loaded from producer-controlled workspace configuration and is treated as approval to export authenticated graph contents. Proposal acceptance controls mutation, but no independent recipient control gates the earlier model request."
}

## 修正方針
Approve model destinations in trusted user configuration or require an explicit canonical endpoint choice before exporting authenticated graph data. Store destination approval outside the repository and minimize the fields disclosed.

## 検証
{
  "counterEvidence": [
    "The victim must explicitly invoke model organization; ordinary startup does not call Jev.",
    "The default model endpoint is loopback.",
    "Model API keys come from the same workspace config; no independent ambient model-key theft is claimed.",
    "The privacy attack requires the victim-readable graph to contain data not already known to the producer.",
    "Typed model proposals do not execute code; graph mutation remains separately validated."
  ],
  "evidenceRefs": [
    "jev-origin",
    "jev-notes",
    "jev-send",
    "jev-cli",
    "jev-kinds"
  ],
  "limitations": [
    "No model request or private graph was used.",
    "Impact depends on the confidentiality of the linked graph, not simply on export of the attacker's own local tasks."
  ],
  "method": "static source trace",
  "summary": "Source establishes the stated chain and prerequisites. No application code was executed."
}

## 回帰確認
[
  "A changed repository model URL must not receive private cloud nodes before independent origin approval.",
  "Verify --apply=false and GUI proposal preview still enforce the same export approval, while accepted local endpoints remain usable."
]

## 場所
[
  {
    "endLine": 74,
    "path": "crates/topo-jev/src/lib.rs",
    "role": "user_input",
    "startLine": 60
  },
  {
    "endLine": 97,
    "path": "crates/topo-jev/src/organize.rs",
    "role": "propagation",
    "startLine": 82
  },
  {
    "endLine": 169,
    "path": "crates/topo-jev/src/lib.rs",
    "role": "sink",
    "startLine": 162
  },
  {
    "endLine": 547,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 535
  },
  {
    "endLine": 231,
    "path": "crates/topo-jev/src/organize.rs",
    "role": "propagation",
    "startLine": 214
  },
  {
    "endLine": 563,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 535
  },
  {
    "endLine": 756,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 739
  }
]


## 今回の区切り（2026-10-03）
未完了（todo）。Jev送信先はworkspaceで設定され、organize操作でgraphを送る設計。信頼方針・ユーザー同意の製品判断が必要。自己hostの通常利用を維持する方針を決めてから対応する。今回の資格情報送信先修正と設定ファイル境界保護は別途完了。
詳細: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/artifacts-6a4e5a1e5a63564edc8ca8693b4bf5cc93fdb5baaaf0c17a2de44b08336f148b/artifacts/report_fix_20261003.md
