---
id: wy71xb
kind: task
title: 中断した調査の未確認範囲を記録する
status: done
tags:
- security
- coverage-review
- 44dbfab6-0d90-475d-b2a3-4df6534779e5
milestones:
- f0ehc5
---

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
統合した項目: D043
同じ原因の報告と未検証候補を追跡する。報告件数は独立した脆弱性数ではない。

---

## [D043] 確認: Scan canceled; saved findings and pending review were preserved.

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D043
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "id": "scan-stopped",
  "reason": "Scan canceled; saved findings and pending review were preserved."
}
```


対応結果: 調査範囲の記録を完了。ユーザーの希望どおり部分調査の段階で元スキャンを中断済み。73報告と43保留の全レコードを18グループに保存し、未確認範囲と残件を保持した。追加全体調査は今回行わない。脆弱性修正件数には含めない。
