---
id: u09xph
kind: task
title: 実環境のレート制限・ログ・権限設定を確認する
status: todo
tags:
- security
- deployment-controls
- 44dbfab6-0d90-475d-b2a3-4df6534779e5
milestones:
- f0ehc5
---

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
統合した項目: D011, D014
同じ原因の報告と未検証候補を追跡する。報告件数は独立した脆弱性数ではない。

---

## [D011] 確認: Source exposes unauthenticated sign-in and delegates rate limiting to an external Cloudflare rule. Actual rule existence, thresholds, a

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D011
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "candidateId": "candidate-public-auth-github-quota",
    "missingImpactPrerequisite": "A deployed endpoint without sufficient platform rate protection and source/dependency-supported shared-quota impact; no external service or request exercised.",
    "originalEvidence": "Documentation requires a Cloudflare rate-limit rule for unauthenticated GitHub sign-in. The router/config/workflow do not implement or provision that rule. Its existence, thresholds, and applicability are unresolved deployment prerequisites. Documented Cloudflare plan quotas are contextual platform claims, not verified or independently enforced application limits.",
    "source": "architecture",
    "sourceAnchors": [
      "docs/cloud/deployment.md:41-43",
      "docs/cloud/deployment.md:99-108",
      "crates/topo-server/src/lib.rs:67-87",
      ".github/workflows/deploy-cloud.yml:47-58",
      "crates/topo-server/src/routes/auth.rs:26-36",
      "crates/topo-server/src/entry.rs:112-118"
    ]
  },
  "candidateId": "candidate-public-auth-github-quota",
  "id": "candidate-public-auth-github-quota",
  "paths": [
    "crates/topo-server/src/routes/auth.rs",
    "crates/topo-server/src/entry.rs",
    "docs/cloud/deployment.md",
    ".github/workflows/deploy-cloud.yml"
  ],
  "reason": "Source exposes unauthenticated sign-in and delegates rate limiting to an external Cloudflare rule. Actual rule existence, thresholds, and shared GitHub-quota exhaustion impact are unavailable in this offline source scope; no quota-exhaustion vulnerability is validated.",
  "surfaceIds": [
    "surface-public-signin-rate-control"
  ]
}
```

---

## [D014] 確認: The source was audited, but the following potential impact/control question remains unvalidated in the authorized offline static mode:

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D014
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "id": "unresolved-server-question-6",
  "originalEvidence": "{\"question\":\"Deployment controls/rate limiting\",\"disposition\":\"needs_follow_up\",\"evidence\":[\"deploy-cloud.yml manual env, pinned actions, CI; wrangler placeholder IDs.\",\"docs/cloud/deployment.md:41-43 requires externally configured rate limit.\",\"dev smoke and known token seed local only; prod migrations do not seed.\",\"gitignore excludes .dev.vars and generated/local state.\"],\"limitations\":\"Live rate limiter, GitHub environment approval and secrets not observed.\"}",
  "paths": [
    "crates/topo-server/wrangler.toml",
    ".github/workflows/deploy-cloud.yml",
    "docs/cloud/deployment.md"
  ],
  "reason": "The source was audited, but the following potential impact/control question remains unvalidated in the authorized offline static mode: Deployment controls/rate limiting",
  "surfaceIds": [
    "server-question-6"
  ]
}
```


## 今回の区切り（2026-10-03）
未完了（todo）。実際のCloudflare設定・資格情報・公開範囲の確認が必要。ローカルD1/Workerの動作確認は完了したが、本番設定・hosted CI・配備は未検証。
詳細: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/artifacts-6a4e5a1e5a63564edc8ca8693b4bf5cc93fdb5baaaf0c17a2de44b08336f148b/artifacts/report_fix_20261003.md
