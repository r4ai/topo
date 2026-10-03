---
id: fszdfu
kind: task
title: 不正なローカルURLでプレビューが終了する問題を防ぐ
status: todo
tags:
- security
- promo-preview
- 44dbfab6-0d90-475d-b2a3-4df6534779e5
milestones:
- f0ehc5
---

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
統合した項目: F009, F041, F057, F064
同じ原因の報告と未検証候補を追跡する。報告件数は独立した脆弱性数ではない。

---

## [F009] A malformed local HTTP request can stop the preview or rendering process

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F009
findingId: csf_7ae3642102663176889e9472
occurrenceId: occ_b20a1eabe0451692982d8fc7
重要度: low
共通原因: http-transport
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

The promo server decodes request paths without catching URI errors. On a shared host, another local user with loopback network access can interrupt the preview or render job without permission to signal its process.

## 根本原因
{
  "evidenceRefs": [
    "loopback-listener",
    "local-request",
    "process-owner",
    "dedup-0003-7-raw-request-target",
    "dedup-0003-7-renderer-exit-cleanup"
  ],
  "summary": "The `createServer()` callback passes client-controlled `req.url` through URL parsing and `decodeURIComponent()` before any error handling. Malformed percent escapes or invalid encoded UTF-8 throw `URIError`. The complete script has no catch around the request callback or uncaught-exception recovery, so the error terminates the process owning the local preview/render job."
}

## 修正方針
Catch URL parsing and percent-decoding failures within the HTTP callback, return a 400 response, and keep the server alive. Handle filesystem/read-stream failures at the same request boundary.

## 検証
{
  "counterEvidence": [
    "The listener is only 127.0.0.1; no remote or browser-origin attack is established.",
    "Developer must explicitly start this preview/render tool.",
    "A same-user process already able to signal the renderer gains no new authority; the finding is conditional on a lower-authority local client.",
    "The listener is bound to 127.0.0.1; direct remote network exposure is not established.",
    "The affected process is ancillary developer tooling.",
    "The attack does not disclose files or execute commands.",
    "No runtime denial-of-service test was performed."
  ],
  "evidenceRefs": [
    "local-request",
    "loopback-listener",
    "process-owner",
    "dedup-0003-7-raw-request-target",
    "dedup-0003-7-renderer-exit-cleanup"
  ],
  "limitations": [
    "No malformed request, process exit or runtime reproduction was performed.",
    "Shared-host exposure is a prerequisite, not an observed deployment.",
    "No application or denial-of-service request was executed.",
    "The separate-local-user scenario is conditional on a host allowing access to another user's loopback listener.",
    "No Internet, cross-origin browser or cross-machine reachability is claimed.",
    "An attacker already entitled to terminate the same process gains no additional capability."
  ],
  "method": "semantic reduction of already-validated Standard source findings; no additional validation",
  "status": "validated",
  "summary": "Both supplied reports establish synchronous unguarded URL/percent decoding in the local renderer request callback and shared ownership of its preview/browser/render session. Malformed percent escapes and invalid encoded UTF-8 can throw before a reply; the sole exit cleanup is not an exception recovery handler. A meaningful attacker is a separate local user or restricted client that can connect to loopback but lacks equivalent process-termination authority. Preview uses port 8765; normal rendering uses an ephemeral port. No request or runtime process termination was exercised."
}

## 回帰確認
[
  "Malformed percent escapes and invalid encoded UTF-8 must return 400 without terminating a subsequent preview request.",
  "Send malformed percent escapes to the request callback and assert HTTP 400 while a subsequent ordinary request still succeeds."
]

## 場所
[
  {
    "endLine": 37,
    "path": "promo/render.mjs",
    "role": "root_control",
    "startLine": 28
  },
  {
    "endLine": 53,
    "path": "promo/render.mjs",
    "role": "entrypoint",
    "startLine": 49
  },
  {
    "endLine": 72,
    "path": "promo/render.mjs",
    "role": "outcome",
    "startLine": 59
  }
]

---

## [F041] Malformed local requests can terminate promo preview and rendering

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F041
findingId: csf_8baf8ec6ea09cf75df5cd29b
occurrenceId: occ_a9728e2fe413bd478c12cabd
重要度: low
共通原因: promo-preview
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

The loopback preview server decodes request paths without catching synchronous URL/URI errors. A local caller can stop the process serving preview or an active render.

## 根本原因
{
  "evidenceRefs": [
    "local-request",
    "loopback-listener",
    "render-cleanup"
  ],
  "summary": "The createServer callback directly calls new URL and decodeURIComponent on unauthenticated req.url. URIError from malformed percent encoding escapes the callback; no catch or uncaught-exception handler contains it. Synchronous filesystem operations and an unhandled read stream also lack per-request containment. The same process owns the render job."
}

## 修正方針
Catch URL/URI and filesystem failures inside the request handler and return client/server errors. Handle read-stream errors without terminating the process.

## 検証
{
  "counterEvidence": [
    "Listener binds only 127.0.0.1.",
    "Preview mode uses port 8765; render mode uses an ephemeral port requiring discovery.",
    "Temporary browser/profile cleanup runs on render-process exit.",
    "No file disclosure, task mutation, or remote production deployment is established."
  ],
  "evidenceRefs": [
    "local-request",
    "loopback-listener",
    "render-cleanup"
  ],
  "limitations": [
    "Offline source inspection only; native Node and Chrome processes were not exercised."
  ],
  "method": "independent_static_source_trace",
  "status": "validated",
  "summary": "Read render.mjs completely and traced loopback requests into the synchronous decoder. Error handling is absent around that callback; existing exit cleanup ends Chrome rather than resuming an interrupted render. No malformed input was constructed or sent."
}

## 回帰確認
[
  "Ensure malformed request paths receive an error response while the preview process continues."
]

## 場所
[
  {
    "endLine": 37,
    "path": "promo/render.mjs",
    "role": "root_control",
    "startLine": 28
  },
  {
    "endLine": 54,
    "path": "promo/render.mjs",
    "role": "entrypoint",
    "startLine": 49
  },
  {
    "endLine": 71,
    "path": "promo/render.mjs",
    "role": "propagation",
    "startLine": 57
  }
]

---

## [F057] Malformed request paths terminate the local promo preview

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F057
findingId: csf_e8efd3c442a41d39624271c7
occurrenceId: occ_cc793cf6e396f5db97c9a5c0
重要度: low
共通原因: promo-preview
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

An unauthenticated local caller can send an invalid percent-encoded URL path to the promo HTTP server. The request callback lets decodeURIComponent throw, which terminates the preview or rendering process under normal Node behavior.

## 根本原因
{
  "evidenceRefs": [
    "promo-request-decode",
    "promo-exit-cleanup"
  ],
  "summary": "The HTTP request callback calls decodeURIComponent on req.url's pathname without a request-local exception guard. Malformed percent escapes raise URIError outside the callback's response handling; the program supplies only an exit cleanup handler."
}

## 修正方針
Catch URL and percent-decoding failures inside the request callback and return HTTP 400. Keep invalid requests inside the HTTP response boundary so the process remains available.

## 検証
{
  "counterEvidence": [
    "Only loopback access is source-backed; external-website reachability depends on browser localhost restrictions and was not established.",
    "The tool is explicitly operator-invoked development code, and restarting it restores service.",
    "A process running under the same UID may already be able to terminate it; the additional boundary is a separate local user that can connect but cannot signal the operator's process."
  ],
  "evidenceRefs": [
    "promo-request-decode",
    "promo-loopback-listener",
    "promo-exit-cleanup"
  ],
  "limitations": [
    "No triggering HTTP request or application execution was performed.",
    "Process termination assumes default Node uncaught-exception behavior; no deployment-specific exception handler was inspected."
  ],
  "method": "static source trace",
  "summary": "A path containing an incomplete or invalid percent escape reaches decodeURIComponent before file checks or a response. The listener accepts requests without authentication on 127.0.0.1:8765 for --serve; the module imports only built-in modules and does not catch the callback exception. Under default Node exception behavior the operator's process exits."
}

## 回帰確認
[
  "Send an invalid percent-encoded path and verify HTTP 400 while a subsequent normal request succeeds.",
  "Check the same handling for both --serve and rendering-mode listeners."
]

## 場所
[
  {
    "endLine": 37,
    "path": "promo/render.mjs",
    "role": "root_control",
    "startLine": 27
  },
  {
    "endLine": 54,
    "path": "promo/render.mjs",
    "role": "entrypoint",
    "startLine": 49
  },
  {
    "endLine": 71,
    "path": "promo/render.mjs",
    "role": "propagation",
    "startLine": 64
  }
]

---

## [F064] A malformed local preview request stops the renderer

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F064
findingId: csf_40774a748c98ead9811b89e5
occurrenceId: occ_23f2600a930ae9f48c83a140
重要度: low
共通原因: promo-preview
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

While the promotional preview or video renderer is running, a local caller can send a malformed percent-encoded URL. The request handler calls `decodeURIComponent` without catching its exception, terminating the Node process and interrupting the preview/render.

## 根本原因
{
  "evidenceRefs": [
    "loopback-preview-listener",
    "preview-url-decode"
  ],
  "summary": "The HTTP callback directly decodes the request URL with `decodeURIComponent`. Invalid percent-encoding throws before response handling, and no error handler catches that callback exception. The same process serves the preview and coordinates rendering, so its termination interrupts both."
}

## 修正方針
Catch request URL parsing and decoding failures inside the HTTP callback, return a 400 response, and keep the server process running.

## 検証
{
  "counterEvidence": [
    "The service binds only 127.0.0.1.",
    "This is auxiliary promo tooling, not the topo API or a deployed product service.",
    "The attacker must reach an active listener; render mode uses a temporary port."
  ],
  "evidenceRefs": [
    "preview-url-decode",
    "loopback-preview-listener"
  ],
  "limitations": [
    "No live reproduction was run.",
    "No internet/browser-origin attack path or arbitrary code execution is claimed."
  ],
  "method": "Complete static JavaScript request-handler review",
  "status": "validated",
  "summary": "The request path reaches an exception-throwing decoder synchronously, outside try/catch; render.mjs contains no recovery handler for that exception. A separate local OS user can reach loopback even without authority to signal the renderer's process."
}

## 回帰確認
[
  "Invalid percent-encoding returns 400 and a subsequent valid request still succeeds."
]

## 場所
[
  {
    "endLine": 37,
    "path": "promo/render.mjs",
    "role": "root_control",
    "startLine": 28
  },
  {
    "endLine": 53,
    "path": "promo/render.mjs",
    "role": "entrypoint",
    "startLine": 49
  },
  {
    "endLine": 71,
    "path": "promo/render.mjs",
    "role": "outcome",
    "startLine": 64
  }
]


## 今回の区切り（2026-10-03）
未完了（todo）。promo開発用preview serverの公開範囲/パス処理を別途確認する。今回の通常API/CLI修正には含めず、未修正として保持。
詳細: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/artifacts-6a4e5a1e5a63564edc8ca8693b4bf5cc93fdb5baaaf0c17a2de44b08336f148b/artifacts/report_fix_20261003.md
