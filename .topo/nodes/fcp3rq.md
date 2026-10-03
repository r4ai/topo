---
id: fcp3rq
kind: task
title: 同時起動時のID・再試行キー衝突による書き込み欠落を修正する
status: done
tags:
- security
- write-identity
- validated
milestones:
- f0ehc5
---

追加指摘 R001（元スキャン外、2026-10-03のD1並行テストで再現）。8 CLIを同時起動したところ、異なるエージェントが同じノードIDを表示し、8件中7件しか変更ログとノードに残らなかった。時刻とスレッドIDを元にしたグローバル擬似乱数をノードIDとIdempotency-Keyで共有している。キーの衝突により異なる操作が同一リクエストとして成功扱いになる。ノードIDとキーをOS乱数から生成し、乱数を取得できないHTTP書き込みは明示的に失敗させる。シードを同じ値に戻す回帰テストと実D1並行テストで確認する。

対応結果: fixed。OS乱数からIDと128-bit request keyを生成し、同じ擬似乱数seedでもキーが同じにならないテストが成功。local D1では修正後の11ノード/10変更/8 distinct agentを確認。全体116テスト、native/Worker clippy成功。詳細: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/artifacts-6a4e5a1e5a63564edc8ca8693b4bf5cc93fdb5baaaf0c17a2de44b08336f148b/artifacts/report_fix_20261003.md
