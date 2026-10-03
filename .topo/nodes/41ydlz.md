---
id: 41ydlz
kind: task
title: GitHubログインの交換先を明示して信頼境界を守る
status: done
tags:
- security
- login-trust
- 44dbfab6-0d90-475d-b2a3-4df6534779e5
milestones:
- f0ehc5
---

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
統合した項目: D024, D038, F010, F036, F063
同じ原因の報告と未検証候補を追跡する。報告件数は独立した脆弱性数ではない。

---

## [D024] 確認: Implicit workspace login destination receives GitHub device-flow token

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D024
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "candidateId": "login-github-token-workspace-server",
    "locations": [
      {
        "endLine": 128,
        "path": "crates/topo-cli/src/cloud.rs",
        "startLine": 99
      },
      {
        "endLine": 55,
        "path": "crates/topo-cloud/src/device.rs",
        "startLine": 42
      },
      {
        "endLine": 130,
        "path": "crates/topo-cloud/src/client.rs",
        "startLine": 128
      }
    ],
    "originalEvidence": "`topo login` with no --url uses workspace .topo/config.toml (cloud.rs:99-105,121-128). Attacker endpoint /v1/auth/config can return the public client ID of victim's legitimate topo deployment; device flow stays on genuine github.com (device.rs:10,42-55), and prompt shows only GitHub URI/code (cloud.rs:125-127), then resulting real GitHub token is POSTed to attacker-selected server (cloud.rs:128 -> client.rs:129-130). Attacker can exchange that token with legitimate topo `/v1/auth/github` for unrestricted nonexpiring token (server/routes/auth.rs:30-36), because legitimate public /v1/auth/config exposes correct app ID (lines 9-11). Requires victim initiating login and approving genuine GitHub app; no prior TOPO_TOKEN needed, proxy placeholder mitigation doesn't protect freshly issued GH access token in JSON.",
    "source": "focused investigator preliminary message",
    "title": "Implicit workspace login destination receives GitHub device-flow token"
  },
  "candidateId": "login-github-token-workspace-server",
  "id": "login-github-token-workspace-server",
  "paths": [
    "crates/topo-cli/src/cloud.rs",
    "crates/topo-cloud/src/device.rs",
    "crates/topo-cloud/src/client.rs",
    "crates/topo-server/src/routes/auth.rs"
  ],
  "reason": "Await parent validation of login destination trust, device-token audience and meaningful capability gain; preserve separately from environment token selection."
}
```

---

## [D038] 確認: Implicit workspace login destination receives the GitHub device-flow token

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D038
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "originalEvidence": "topo login defaults URL to workspace-controlled [cloud].url (cloud.rs:100-105,121-124); attacker server supplies github_client_id, device flow runs against fixed HTTPS GitHub and returns GitHub access token, posted to same attacker URL (125-128). Device prompt only shows GitHub verification URI/code, not selected cloud destination. Malicious workspace can trigger NotSignedIn hint to run topo login and impersonate genuine OAuth app using public client ID; requires victim OAuth approval. Stored exact-URL credentials are not automatically leaked.",
    "source": "client_investigator",
    "title": "Implicit workspace login destination receives the GitHub device-flow token"
  },
  "candidateId": "candidate-client-login-github-token-destination",
  "id": "candidate-client-login-github-token-destination",
  "paths": [
    "crates/topo-cli/src/cloud.rs",
    "crates/topo-cloud/src/device.rs",
    "crates/topo-cloud/src/client.rs"
  ],
  "reason": "Awaiting parent validation of implicit login URL choice, OAuth approval UI, app-client-ID binding and transfer of resulting GitHub token."
}
```

---

## [F010] Workspace login can disclose a legitimate GitHub OAuth token

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F010
findingId: csf_96b9f26eb9189c2b7d28f0a3
occurrenceId: occ_b50595707b77d258157939fb
重要度: medium
共通原因: login-trust
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A repository can select an attacker-controlled cloud URL for `topo login`. That server can advertise the legitimate service's public GitHub client ID. After the user approves the legitimate GitHub app, the CLI sends its access token to the selected server, which can relay it to the legitimate service for an unrestricted topo token.

## 根本原因
{
  "evidenceRefs": [
    "login-flow",
    "login-issuance",
    "login-send",
    "login-replay",
    "login-fixed-github",
    "login-real-app-check",
    "login-public-client-id"
  ],
  "summary": "Login trusts a repository-selected exchange server to choose the OAuth application and receive the approved token. GitHub app identity is checked by the real server, but it does not bind the token's local exchange recipient to that server."
}

## 修正方針
Approve and bind the canonical exchange origin and expected GitHub application in trusted user configuration before starting the device flow. Show the exchange origin before approval and never authorize its token receipt using only a repository-selected auth-config response.

## 検証
{
  "counterEvidence": [
    "The user must initiate login and approve GitHub's browser device flow.",
    "No GitHub scopes are requested; the stated impact is the topo account obtained via legitimate token exchange.",
    "A trusted explicit --url/TOPO_CLOUD_URL overrides repository URL selection for login.",
    "Tokens from unrelated OAuth apps are rejected by the real server; the attack uses the legitimate app's public client ID."
  ],
  "evidenceRefs": [
    "login-flow",
    "login-issuance",
    "login-send",
    "login-replay",
    "login-fixed-github",
    "login-real-app-check",
    "login-public-client-id"
  ],
  "limitations": [
    "No OAuth flow, network request, or captured credential was executed.",
    "Impact is restricted to the chosen legitimate topo service and victim identity."
  ],
  "method": "static source trace",
  "summary": "Source establishes the stated chain and prerequisites. No application code was executed."
}

## 回帰確認
[
  "A workspace-selected unapproved server must not receive any GitHub access token even if it advertises the legitimate client ID.",
  "Verify explicit trusted origins work and unrelated OAuth app tokens remain rejected."
]

## 場所
[
  {
    "endLine": 105,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "user_input",
    "startLine": 99
  },
  {
    "endLine": 130,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 121
  },
  {
    "endLine": 130,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 124
  },
  {
    "endLine": 36,
    "path": "crates/topo-server/src/routes/auth.rs",
    "role": "propagation",
    "startLine": 26
  },
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/device.rs",
    "role": "propagation",
    "startLine": 42
  },
  {
    "endLine": 118,
    "path": "crates/topo-server/src/entry.rs",
    "role": "expected_control",
    "startLine": 112
  },
  {
    "endLine": 12,
    "path": "crates/topo-server/src/routes/auth.rs",
    "role": "user_input",
    "startLine": 8
  },
  {
    "endLine": 11,
    "path": "crates/topo-server/src/routes/auth.rs",
    "role": "propagation",
    "startLine": 10
  },
  {
    "endLine": 36,
    "path": "crates/topo-server/src/routes/auth.rs",
    "role": "propagation",
    "startLine": 30
  }
]

---

## [F036] Login can deliver a newly approved GitHub token to a workspace-selected server

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F036
findingId: csf_900d1f2e69152f666cc039ff
occurrenceId: occ_d03554c56d9154e0191fbda9
重要度: medium
共通原因: login-trust
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

Running `topo login` without a trusted explicit destination in a supplied workspace lets its server choose a legitimate topo OAuth app ID and receive the approved GitHub token. The server can exchange that token at the legitimate topo service for unrestricted victim-account access.

## 根本原因
{
  "evidenceRefs": [
    "login-origin-default",
    "oauth-client-registration",
    "github-device-issuance",
    "oauth-exchange-sink",
    "public-oauth-id",
    "oauth-replay-outcome",
    "oauth-app-check"
  ],
  "summary": "`url` inherits a project's cloud destination when no explicit destination is supplied. `login` asks that server for its OAuth client ID, then sends the newly issued GitHub access token to the same server without verifying a trusted app-to-recipient registration. An attacker can supply the legitimate topo app's public client ID, so the user approves the expected application while the resulting credential goes to the attacker's exchange endpoint. The legitimate service subsequently accepts it and issues an unrestricted topo token."
}

## 修正方針
Require an independently trusted server registration before starting login, binding the OAuth client ID and token-exchange origin to that registration. Clearly identify the verified server before device approval and reject any workspace-selected unregistered recipient.

## 検証
{
  "counterEvidence": [
    "GitHub endpoints are fixed, TLS checks remain enabled, and the user must approve the device flow.",
    "A different OAuth app's token is rejected; the attack uses the legitimate public client ID.",
    "Explicit trusted --url or TOPO_CLOUD_URL bypasses the workspace destination.",
    "No OAuth scopes are requested; impact is topo identity takeover through token exchange, not private GitHub-resource compromise."
  ],
  "evidenceRefs": [
    "login-origin-default",
    "oauth-client-registration",
    "github-device-issuance",
    "oauth-exchange-sink",
    "public-oauth-id",
    "oauth-replay-outcome",
    "oauth-app-check"
  ],
  "limitations": [
    "Requires a configured legitimate topo service and OAuth app, plus approval of an implicit login in the supplied workspace.",
    "The checked Wrangler configuration still contains placeholder app IDs; no current live victim deployment was established."
  ],
  "method": "independent_static_source_trace",
  "status": "validated",
  "summary": "The malicious server returns the legitimate application's public ID. GitHub's fixed device endpoint gives the CLI a token for that app after user approval. Client::sign_in posts it to the malicious workspace URL. The legitimate service's app-specific token check accepts the captured token because it was issued for the legitimate app, and insert_token uses scope None and expiry None."
}

## 回帰確認
[
  "An untrusted workspace server returning a known legitimate OAuth client ID must never receive a newly approved token.",
  "Verify implicit login requires trusted registration and explicit URL login validates the same app-to-recipient binding.",
  "Ensure the recipient is identified before asking the user to approve device authorization."
]

## 場所
[
  {
    "endLine": 105,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "user_input",
    "startLine": 99
  },
  {
    "endLine": 131,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "root_control",
    "startLine": 121
  },
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/device.rs",
    "role": "propagation",
    "startLine": 30
  },
  {
    "endLine": 131,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 124
  },
  {
    "endLine": 11,
    "path": "crates/topo-server/src/routes/auth.rs",
    "role": "propagation",
    "startLine": 8
  },
  {
    "endLine": 36,
    "path": "crates/topo-server/src/routes/auth.rs",
    "role": "outcome",
    "startLine": 26
  },
  {
    "endLine": 118,
    "path": "crates/topo-server/src/entry.rs",
    "role": "expected_control",
    "startLine": 112
  }
]

---

## [F063] Signing in from a shared workspace can disclose the GitHub token to its selected server

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F063
findingId: csf_64275d1d9520ac4794b6a4ea
occurrenceId: occ_b6b61de6327662e1fde15012
重要度: medium
共通原因: login-trust
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

`topo login` accepts the linked workspace's server and its advertised GitHub OAuth client ID. A malicious server can advertise the legitimate app's public ID; after the victim approves that app on GitHub, the CLI sends the fresh GitHub token to the malicious server. It can redeem the token at the legitimate topo API for unrestricted account access.

## 根本原因
{
  "evidenceRefs": [
    "implicit-login-destination",
    "server-app-id-and-exchange",
    "github-token-acquired",
    "github-token-posted",
    "unrestricted-token-redemption"
  ],
  "summary": "Implicit server resolution trusts shared `cloud.url`. Login then obtains `github_client_id` from that same untrusted server, requests a GitHub device token for it, and posts the token back without binding the app identity to a trusted server origin. The legitimate app ID is publicly exposed by `/v1/auth/config`, so a malicious recipient can use that ID and later redeem the captured token at the real server."
}

## 修正方針
Select and validate the login server from trusted user configuration before OAuth consent, and bind the allowed GitHub client ID to that server origin. Shared workspace content must not silently choose the recipient of a newly issued OAuth token; require explicit destination trust before sending it.

## 検証
{
  "counterEvidence": [
    "An explicit trusted --url or TOPO_CLOUD_URL overrides shared configuration at cloud.rs:100-104.",
    "GitHub approval is required and the CLI uses the fixed https://github.com origin; this attack does not bypass GitHub authentication.",
    "Device flow requests no extra GitHub scopes. Concrete impact is topo account access, not GitHub repository takeover."
  ],
  "evidenceRefs": [
    "server-app-id-and-exchange",
    "github-token-acquired",
    "github-token-posted",
    "unrestricted-token-redemption"
  ],
  "limitations": [
    "Requires victim approval and a deployed legitimate server with device flow enabled.",
    "No live GitHub token or real exchange was used."
  ],
  "method": "Static OAuth source/consumer trace",
  "status": "validated",
  "summary": "The client executes the real GitHub flow for the server-supplied ID and delivers its token to the workspace-selected URL. Server `entry.rs:112-118` verifies only that the token belongs to its OAuth app; `routes/auth.rs:30-36` then creates an unscoped token, so using that app's public ID defeats the intended server distinction."
}

## 回帰確認
[
  "A shared link to another server must not receive a device token for a trusted server's client ID.",
  "Reject auth-config client IDs that do not match the trusted origin binding before starting device flow."
]

## 場所
[
  {
    "endLine": 104,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "user_input",
    "startLine": 100
  },
  {
    "endLine": 128,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "root_control",
    "startLine": 121
  },
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/device.rs",
    "role": "propagation",
    "startLine": 42
  },
  {
    "endLine": 130,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 124
  },
  {
    "endLine": 36,
    "path": "crates/topo-server/src/routes/auth.rs",
    "role": "outcome",
    "startLine": 30
  },
  {
    "endLine": 118,
    "path": "crates/topo-server/src/entry.rs",
    "role": "supporting",
    "startLine": 112
  }
]


## 対応結果（2026-10-03）
結果: fixed。loginは明示--url/外部TOPO_CLOUD_URLを必須にし、workspace linkだけの暗黙送信先をGitHub通信前に拒否。自己hostの明示URL利用は維持。
最終検証: cargo test --locked --workspace 116成功/既存3ignore、fmt、native/Worker clippy -D warnings、local Worker/D1 smoke成功。詳細: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/artifacts-6a4e5a1e5a63564edc8ca8693b4bf5cc93fdb5baaaf0c17a2de44b08336f148b/artifacts/report_fix_20261003.md
