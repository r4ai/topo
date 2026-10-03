---
id: tlqtnp
kind: task
title: 生成Wasmとソースの対応・生成コードの検証を確認する
status: todo
tags:
- security
- build-provenance
- 44dbfab6-0d90-475d-b2a3-4df6534779e5
milestones:
- f0ehc5
---

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
統合した項目: D002, D005, D012, D018
同じ原因の報告と未検証候補を追跡する。報告件数は独立した脆弱性数ではない。

---

## [D002] 確認: Current generated executable composition was statically inspected but complete compiled function-body analysis and equivalence to curre

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D002
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "evidence": "2,039,755-byte Wasm;SHA256 2e6ace562d676194b4271be16ac4433bfebfe2ed38424d758225e00eadd60323;2195 defined functions;141 imports all ./index_bg.js;fetch export index1922;auth/routes/D1/GitHub symbols corroborate ownership.",
  "id": "compiled-worker-function-bodies",
  "paths": [
    "crates/topo-server/build/index_bg.wasm"
  ],
  "reason": "Current generated executable composition was statically inspected but complete compiled function-body analysis and equivalence to current Rust are unresolved. This is a coverage limitation,not an unvalidated vulnerability candidate."
}
```

---

## [D005] 確認: Generated Worker loader and WASM import/export/section metadata were inspected as data, but the 2,039,755-byte compiled code has no sou

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D005
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "id": "compiled-worker-implementation",
  "paths": [
    "crates/topo-server/build/index_bg.wasm"
  ],
  "reason": "Generated Worker loader and WASM import/export/section metadata were inspected as data, but the 2,039,755-byte compiled code has no source map or build-to-current-source attestation in this checkout. Rust owners and all JS binding/entry logic were audited; exhaustive instruction-level review and equivalence of this prebuilt WASM to current Rust remain unverified. No application execution, rebuild, tool installation or network access was authorized.",
  "surfaceIds": [
    "generated-worker-loader"
  ]
}
```

---

## [D012] 確認: The generated implementation-owning WASM artifact was structurally inspected as data, but full bytecode semantics and source correspond

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D012
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "id": "generated-wasm-semantic-correspondence",
  "originalEvidence": {
    "bytes": 2039755,
    "definedFunctions": 2195,
    "exports": [
      "fetch",
      "init"
    ],
    "importModules": [
      "./index_bg.js"
    ],
    "imports": 141,
    "sha256": "2e6ace562d676194b4271be16ac4433bfebfe2ed38424d758225e00eadd60323"
  },
  "paths": [
    "crates/topo-server/build/index_bg.wasm"
  ],
  "reason": "The generated implementation-owning WASM artifact was structurally inspected as data, but full bytecode semantics and source correspondence cannot be claimed from the offline source audit.",
  "surfaceIds": [
    "generated-worker-binary"
  ]
}
```

---

## [D018] 確認: Ignored build/index.js was found in the expanded current-state inventory. Same architecture worker is now auditing implementation-ownin

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D018
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "id": "remaining-generated-worker",
  "paths": [
    "crates/topo-server/build/index.js"
  ],
  "reason": "Ignored build/index.js was found in the expanded current-state inventory. Same architecture worker is now auditing implementation-owning generated JS as data; no finding is assumed."
}
```


## 今回の区切り（2026-10-03）
未完了（todo）。ビルド用スクリプト/外部依存の信頼性・実環境の前提を別途確認。未確認を安全扱いにしない。
詳細: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/artifacts-6a4e5a1e5a63564edc8ca8693b4bf5cc93fdb5baaaf0c17a2de44b08336f148b/artifacts/report_fix_20261003.md
