---
id: 8cofxs
kind: task
title: 環境トークンを信頼済みクラウド送信先に限定する
status: done
tags:
- security
- token-origin
- 44dbfab6-0d90-475d-b2a3-4df6534779e5
milestones:
- f0ehc5
---

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
統合した項目: D025, D030, D040, D041, F001, F018, F021, F025, F029, F033, F043, F047, F051, F058, F061, F065, F066, F070, F071
同じ原因の報告と未検証候補を追跡する。報告件数は独立した脆弱性数ではない。

---

## [D025] 確認: Workspace cloud URL may redirect the environment bearer token to another server

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D025
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "attacker": "Contributor or workspace supplier able to supply .topo/config.toml; no access to the victim's environment token",
    "candidateId": "cloud-env-token-untrusted-url",
    "evidence": [
      {
        "code": "let path = topo_dir.join(\"config.toml\");\n...\nlet Some(table) = read(topo_dir)?.remove(\"cloud\") else {\n    return Ok(None);\n};\ntable.try_into().map(Some).map_err(|e| Error::file(&topo_dir.join(\"config.toml\"), e))",
        "endLine": 32,
        "path": "crates/topo-cloud/src/config.rs",
        "startLine": 18
      },
      {
        "code": "pub fn open(dir: PathBuf) -> Result<Workspace, Error> {\n    let Some(cloud) = config::load(&dir)? else {\n        return Ok(Workspace::open(dir)?);\n    };\n    let client = Client::new(&cloud.url, credentials::token(&cloud.url)?);\n    Ok(Workspace::open_remote(dir, Arc::new(HttpRemote::new(client, cloud.workspace)))?)",
        "endLine": 53,
        "path": "crates/topo-cloud/src/lib.rs",
        "startLine": 48
      },
      {
        "code": "pub fn token(url: &str) -> Result<String, Error> {\n    if let Ok(token) = std::env::var(\"TOPO_TOKEN\") {\n        return Ok(token);\n    }\n    load(url)?.map(|credential| credential.token).ok_or_else(|| Error::NotSignedIn(url.to_owned()))",
        "endLine": 55,
        "path": "crates/topo-cloud/src/credentials.rs",
        "startLine": 51
      },
      {
        "code": "let url = format!(\"{}{path}\", self.url);\nlet mut request = Request::builder().method(method).uri(&url);\nif !self.token.is_empty() {\n    request = request.header(\"authorization\", format!(\"Bearer {}\", self.token));",
        "endLine": 58,
        "path": "crates/topo-cloud/src/client.rs",
        "startLine": 55
      }
    ],
    "locations": [
      {
        "endLine": 53,
        "path": "crates/topo-cloud/src/lib.rs",
        "role": "entrypoint",
        "startLine": 48
      },
      {
        "endLine": 55,
        "path": "crates/topo-cloud/src/credentials.rs",
        "role": "propagation",
        "startLine": 51
      },
      {
        "endLine": 72,
        "path": "crates/topo-cloud/src/client.rs",
        "role": "sink",
        "startLine": 55
      }
    ],
    "taxonomy": {
      "category": "sensitive-data-exposure",
      "cwe": [
        "CWE-200"
      ]
    },
    "title": "Workspace cloud URL may redirect the environment bearer token to another server",
    "unresolved": "Confirm automatic read/GUI startup callers, realistic shared workspace distribution and documentation, URL trust controls, proxy-placeholder counterevidence, and resulting token authority."
  },
  "candidateId": "cloud-env-token-untrusted-url",
  "id": "cloud-env-token-untrusted-url",
  "paths": [
    "crates/topo-cloud/src/lib.rs",
    "crates/topo-cloud/src/config.rs",
    "crates/topo-cloud/src/credentials.rs",
    "crates/topo-cloud/src/client.rs"
  ],
  "reason": "Confirm automatic read/GUI startup callers, realistic shared workspace distribution and documentation, URL trust controls, proxy-placeholder counterevidence, and resulting token authority."
}
```

---

## [D030] 確認: Workspace cloud URL may redirect the environment bearer token to another server

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D030
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "attacker": "Contributor or workspace supplier able to supply .topo/config.toml; no access to the victim's environment token",
    "candidateId": "cloud-env-token-untrusted-url",
    "evidence": [
      {
        "code": "let path = topo_dir.join(\"config.toml\");\n...\nlet Some(table) = read(topo_dir)?.remove(\"cloud\") else {\n    return Ok(None);\n};\ntable.try_into().map(Some).map_err(|e| Error::file(&topo_dir.join(\"config.toml\"), e))",
        "endLine": 32,
        "path": "crates/topo-cloud/src/config.rs",
        "startLine": 18
      },
      {
        "code": "pub fn open(dir: PathBuf) -> Result<Workspace, Error> {\n    let Some(cloud) = config::load(&dir)? else {\n        return Ok(Workspace::open(dir)?);\n    };\n    let client = Client::new(&cloud.url, credentials::token(&cloud.url)?);\n    Ok(Workspace::open_remote(dir, Arc::new(HttpRemote::new(client, cloud.workspace)))?)",
        "endLine": 53,
        "path": "crates/topo-cloud/src/lib.rs",
        "startLine": 48
      },
      {
        "code": "pub fn token(url: &str) -> Result<String, Error> {\n    if let Ok(token) = std::env::var(\"TOPO_TOKEN\") {\n        return Ok(token);\n    }\n    load(url)?.map(|credential| credential.token).ok_or_else(|| Error::NotSignedIn(url.to_owned()))",
        "endLine": 55,
        "path": "crates/topo-cloud/src/credentials.rs",
        "startLine": 51
      },
      {
        "code": "let url = format!(\"{}{path}\", self.url);\nlet mut request = Request::builder().method(method).uri(&url);\nif !self.token.is_empty() {\n    request = request.header(\"authorization\", format!(\"Bearer {}\", self.token));",
        "endLine": 58,
        "path": "crates/topo-cloud/src/client.rs",
        "startLine": 55
      }
    ],
    "investigatorOriginalEvidence": "Confirmed env-token leak source chain: README.md:236-253 explicitly commits cloud link and uses TOPO_TOKEN for agents; docs/cloud/README.md:64-86 and skills/topo/SKILL.md:105-112 establish shared workspace workflow. Any regular CLI command opens topo_cloud::open at main.rs:403; config::load reads untrusted .topo/config.toml; lib.rs:52 selects credentials::token(cloud.url); credentials.rs:51-55 returns TOPO_TOKEN independent of URL; client.rs:55-72 sends it as Authorization to that URL. Disk credentials are exact-URL keyed and block simple retargeting absent TOPO_TOKEN; placeholder proxy caveat limits proxy environments, not normal raw-token documented agent/CI use.",
    "locations": [
      {
        "endLine": 53,
        "path": "crates/topo-cloud/src/lib.rs",
        "role": "entrypoint",
        "startLine": 48
      },
      {
        "endLine": 55,
        "path": "crates/topo-cloud/src/credentials.rs",
        "role": "propagation",
        "startLine": 51
      },
      {
        "endLine": 72,
        "path": "crates/topo-cloud/src/client.rs",
        "role": "sink",
        "startLine": 55
      }
    ],
    "taxonomy": {
      "category": "sensitive-data-exposure",
      "cwe": [
        "CWE-200"
      ]
    },
    "title": "Workspace cloud URL may redirect the environment bearer token to another server",
    "unresolved": "Confirm automatic read/GUI startup callers, realistic shared workspace distribution and documentation, URL trust controls, proxy-placeholder counterevidence, and resulting token authority."
  },
  "candidateId": "cloud-env-token-untrusted-url",
  "id": "cloud-env-token-untrusted-url-fe0994df3ca5e73c",
  "paths": [
    "crates/topo-cloud/src/lib.rs",
    "crates/topo-cloud/src/config.rs",
    "crates/topo-cloud/src/credentials.rs",
    "crates/topo-cloud/src/client.rs"
  ],
  "reason": "Confirm automatic read/GUI startup callers, realistic shared workspace distribution and documentation, URL trust controls, proxy-placeholder counterevidence, and resulting token authority."
}
```

---

## [D040] 確認: Workspace-configured cloud destination receives unbound environment bearer token

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D040
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "originalEvidence": "workspace .topo/config.toml cloud.url flows via topo-cloud::open (lib.rs 48-53) into credentials::token (credentials.rs 51-55 returns TOPO_TOKEN regardless of URL) then Client::send (client.rs 55-58) Authorization to arbitrary URL. Shared/checked-out attacker workspace can leak CI/agent token on ordinary topo ls. File credentials are URL-keyed, env credentials are not.",
    "source": "baseline",
    "title": "Workspace-configured cloud destination receives unbound environment bearer token"
  },
  "candidateId": "candidate-baseline-env-token-destination",
  "id": "candidate-baseline-env-token-destination",
  "paths": [
    "crates/topo-cloud/src/lib.rs",
    "crates/topo-cloud/src/credentials.rs",
    "crates/topo-cloud/src/client.rs"
  ],
  "reason": "Awaiting independent parent validation of caller/config precedence, attacker-controlled destination, transport and mitigations."
}
```

---

## [D041] 確認: Workspace-controlled cloud URL may receive a process-wide TOPO_TOKEN

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D041
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "attacker": "Author of a workspace/repository opened by a user whose process has TOPO_TOKEN set",
    "counterevidence": [
      "Stored credentials are indexed by the exact URL, so the concern applies to the environment override.",
      "TOPO_TOKEN can be a sandbox placeholder; impact requires a real credential or a proxy that substitutes it for this destination."
    ],
    "cwe": [
      "CWE-201"
    ],
    "evidence": [
      {
        "code": "pub fn load(topo_dir: &Path) -> Result<Option<CloudConfig>, Error> {\n    let Some(table) = read(topo_dir)?.remove(\"cloud\") else {\n        return Ok(None);\n    };\n    table.try_into().map(Some).map_err(|e| Error::file(&topo_dir.join(\"config.toml\"), e))",
        "path": "crates/topo-cloud/src/config.rs",
        "startLine": 28
      },
      {
        "code": "pub fn token(url: &str) -> Result<String, Error> {\n    if let Ok(token) = std::env::var(\"TOPO_TOKEN\") {\n        return Ok(token);\n    }\n    load(url)?.map(|credential| credential.token).ok_or_else(|| Error::NotSignedIn(url.to_owned()))\n}",
        "path": "crates/topo-cloud/src/credentials.rs",
        "startLine": 51
      },
      {
        "code": "        let url = format!(\"{}{path}\", self.url);\n        let mut request = Request::builder().method(method).uri(&url);\n        if !self.token.is_empty() {\n            request = request.header(\"authorization\", format!(\"Bearer {}\", self.token));\n        }",
        "path": "crates/topo-cloud/src/client.rs",
        "startLine": 55
      }
    ],
    "invariant": "A process-wide bearer credential should not be sent to a destination selected by repository data without binding or confirmation",
    "locations": [
      {
        "endLine": 32,
        "path": "crates/topo-cloud/src/config.rs",
        "startLine": 28
      },
      {
        "endLine": 55,
        "path": "crates/topo-cloud/src/credentials.rs",
        "startLine": 49
      },
      {
        "endLine": 118,
        "path": "crates/topo-cli/src/cloud.rs",
        "startLine": 114
      },
      {
        "endLine": 59,
        "path": "crates/topo-cloud/src/client.rs",
        "startLine": 55
      }
    ],
    "source_to_sink": "config::load reads .topo/config.toml cloud.url; credentials::token returns TOPO_TOKEN regardless of URL; Client::send attaches it to requests for that URL",
    "title": "Workspace-controlled cloud URL may receive a process-wide TOPO_TOKEN",
    "unresolved": [
      "Confirm normal CLI/GUI opening of a cloud-linked repository reaches Client::send without destination consent; verify documented caller trust and destination binding."
    ]
  },
  "candidateId": "discovery-0011.env-token-destination",
  "id": "discovery-0011.env-token-destination",
  "reason": "Candidate awaiting independent parent validation of opening entrypoint, trust boundary and effective destination controls."
}
```

---

## [F001] Opening an untrusted workspace sends TOPO_TOKEN to its chosen server

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F001
findingId: csf_4e85a2b12391555bce5a6575
occurrenceId: occ_a7cdfc1ceabc5b59c15515bd
重要度: medium
共通原因: token-origin
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A workspace supplier or repository contributor can change `[cloud].url` so that normal CLI graph commands, GUI startup, or a documented checkout-based agent workflow send the process's real TOPO_TOKEN to an attacker-controlled origin. The bearer can be replayed at its legitimate server with its existing workspace scope or account authority.

## 根本原因
{
  "evidenceRefs": [
    "cloud-config",
    "cloud-open",
    "environment-token",
    "bearer-send",
    "agent-checkout",
    "ordinary-open",
    "workspace-cloud-config",
    "origin-config",
    "origin-load",
    "origin-send",
    "origin-cli",
    "origin-gui",
    "dedup-0003-0-cloud-config-input",
    "dedup-0003-0-ambient-token",
    "dedup-0003-0-bearer-network-sink",
    "dedup-0003-0-automatic-graph-fetch",
    "dedup-0003-0-http-remote-fetch",
    "dedup-0004-0-workspace-cloud-config",
    "dedup-0004-0-automatic-cloud-open",
    "dedup-0004-0-bearer-header-send",
    "dedup-0004-0-file-credential-binding"
  ],
  "summary": "Workspace-controlled config.toml selects cloud.url; topo_cloud::open constructs the remote client using credentials::token(cloud.url) and immediately fetches its graph. The environment branch returns TOPO_TOKEN for every destination before consulting the exact-URL credential store. Client::send appends the API path and attaches Authorization: Bearer to that selected URL without a separately trusted recipient binding. TLS authenticates the selected HTTPS endpoint, rather than establishing its authority to receive this credential."
}

## 修正方針
Bind environment credentials to an explicit trusted server origin outside repository/workspace data, and reject or require independent user approval for workspace-selected destinations that do not match it before constructing any authenticated request. Enforce this during CLI/GUI startup and linked operations. Preserve exact-URL file-backed credentials and destination-restricted secret-substitution proxy workflows.

## 検証
{
  "assertions": [
    "No origin allowlist or trusted origin binding exists in the environment-token branch.",
    "Failure to return a valid graph does not prevent the request header from reaching the configured origin."
  ],
  "counterEvidence": [
    "File-backed credentials require an exact URL key.",
    "Platform TLS verification protects transit but cannot establish that a repository-selected HTTPS origin is authorized to receive this token.",
    "A destination-restricting secret-substitution proxy can block this attack; a placeholder alone is not the real secret.",
    "Stored credentials are indexed by exact URL (credentials.rs:41-46), preventing this path when TOPO_TOKEN is absent.",
    "docs/cloud/api.md:83-91 describes optional proxy-only placeholder tokens; an outbound proxy that substitutes secrets only for authorized origins can block disclosure. The source does not require that deployment.",
    "Stored credentials are keyed by exact URL, so this chain depends on a real ambient token.",
    "TOPO_CLOUD_URL/--url select general management-command destinations but do not override ordinary linked workspace opening.",
    "A proxy may hold the real token and restrict substitution; that external protection is not present in the source.",
    "Stored credentials are selected by exact server URL when TOPO_TOKEN is absent (credentials.rs:41-55).",
    "Client uses a platform certificate verifier and a 15-second timeout (client.rs:31-39); an attacker can use a valid certificate for its own host.",
    "The token may be an inert proxy placeholder (credentials.rs:49-50); a proxy that enforces destinations can prevent secret disclosure.",
    "Absent TOPO_TOKEN, stored credentials are keyed by the exact configured URL and an unknown server fails as not signed in.",
    "Placeholder-based outbound proxies may enforce host binding outside this repository; the concrete credential disclosure path requires a real environment token or a proxy that releases its secret to the chosen destination.",
    "The attacker must induce workspace opening; this is not unauthenticated access to the victim's process."
  ],
  "evidenceRefs": [
    "cloud-config",
    "cloud-open",
    "environment-token",
    "bearer-send",
    "agent-checkout",
    "ordinary-open",
    "workspace-cloud-config",
    "origin-config",
    "origin-load",
    "origin-send",
    "origin-cli",
    "origin-gui",
    "dedup-0003-0-cloud-config-input",
    "dedup-0003-0-ambient-token",
    "dedup-0003-0-bearer-network-sink",
    "dedup-0003-0-automatic-graph-fetch",
    "dedup-0003-0-http-remote-fetch",
    "dedup-0004-0-workspace-cloud-config",
    "dedup-0004-0-automatic-cloud-open",
    "dedup-0004-0-bearer-header-send",
    "dedup-0004-0-cli-auto-open",
    "dedup-0004-0-gui-auto-open",
    "dedup-0004-0-file-credential-binding"
  ],
  "limitations": [
    "Requires a real token rather than a proxy-only placeholder and egress to the selected origin.",
    "No live deployment or runtime reproduction was inspected.",
    "Requires a real reusable TOPO_TOKEN and an attacker-controlled workspace cloud configuration.",
    "No live token, network request or exploit was used.",
    "No network request or credential was used.",
    "The attack does not obtain a token when no ambient token exists and no matching stored credential is available.",
    "Actual use of real TOPO_TOKEN and access to a lower-trust workspace are conditional; no credential values were read or transmitted.",
    "No application code was executed or live deployment tested.",
    "Externally enforced proxy destination restrictions are not visible in the source."
  ],
  "method": "semantic reduction of already-validated Standard source findings; no additional validation",
  "status": "validated",
  "summary": "Five supplied validated findings establish the same automatic first-request disclosure before any valid graph response or explicit login/link approval. Exact-URL stored credentials block this flow when TOPO_TOKEN is absent. General management-command --url/TOPO_CLOUD_URL overrides do not override ordinary linked workspace opening. A real reusable ambient token, reachable attacker origin, and absence of an enforcing outbound origin restriction are prerequisites. The assigned source also makes the public topo_cloud::open boundary explicit and does not depend on HTTP redirect behavior."
}

## 回帰確認
[
  "Opening a workspace with a changed cloud URL must not send a real environment token to that origin.",
  "File-backed credentials must remain bound to the exact trusted server.",
  "Open a workspace pointing at another origin with TOPO_TOKEN set and assert that no authorization header reaches that origin.",
  "Retain exact-origin authentication for explicitly trusted servers and proxy placeholder workflows.",
  "With a real test token and an unapproved workspace URL, opening CLI and GUI views must send no Authorization header to that origin.",
  "Verify approved URL-keyed credentials and proxy placeholders retain their intended behavior.",
  "Open a supplied workspace with a different cloud URL while TOPO_TOKEN is set and assert that no authenticated request is sent.",
  "Verify the intended server receives the token and URL-keyed stored credentials retain their existing behavior.",
  "Opening attacker-controlled cloud configuration with a token bound to another origin sends no network request or Authorization header.",
  "A matching trusted origin still receives the exact token or proxy placeholder."
]

## 場所
[
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 51
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "propagation",
    "startLine": 49
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 403
  },
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 28
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "propagation",
    "startLine": 48
  },
  {
    "endLine": 1219,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "propagation",
    "startLine": 108
  },
  {
    "endLine": 15,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 9
  },
  {
    "endLine": 54,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "entrypoint",
    "startLine": 48
  },
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 49
  },
  {
    "endLine": 58,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "endLine": 403,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 399
  },
  {
    "endLine": 1220,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  },
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "propagation",
    "startLine": 18
  },
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "propagation",
    "startLine": 51
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "propagation",
    "startLine": 55
  },
  {
    "endLine": 94,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 90
  },
  {
    "endLine": 403,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 401
  },
  {
    "endLine": 1224,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 114
  },
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 18
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "entrypoint",
    "startLine": 48
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 108
  },
  {
    "endLine": 1218,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1217
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "entrypoint",
    "startLine": 47
  },
  {
    "endLine": 56,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 49
  },
  {
    "endLine": 95,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 90
  },
  {
    "endLine": 404,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 400
  },
  {
    "endLine": 1225,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  },
  {
    "endLine": 47,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "expected_control",
    "startLine": 40
  }
]

---

## [F018] A supplied workspace can redirect an ambient cloud token

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F018
findingId: csf_4282575ac9789af915ece8c4
occurrenceId: occ_076160e43b78e894a53a06c5
重要度: medium
共通原因: token-origin
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A repository producer can set .topo/config.toml [cloud].url to an HTTPS endpoint they control. Opening that workspace through an ordinary CLI command or the GUI sends a real TOPO_TOKEN there, because environment-token selection is independent of the destination URL.

## 根本原因
{
  "evidenceRefs": [
    "origin-config",
    "origin-load",
    "origin-global-token",
    "origin-send",
    "origin-cli",
    "origin-gui"
  ],
  "summary": "Workspace configuration controls the cloud origin while credentials::token returns the ambient token for every origin. Startup combines them without a destination approval or origin binding."
}

## 修正方針
Bind environment credentials to a trusted origin supplied outside the workspace, and require explicit approval before a repository-selected cloud origin can receive credentials. Apply the gate during CLI and GUI startup and linked operations.

## 検証
{
  "counterEvidence": [
    "Stored credentials are keyed by exact URL, so this chain depends on a real ambient token.",
    "TOPO_CLOUD_URL/--url select general management-command destinations but do not override ordinary linked workspace opening.",
    "A proxy may hold the real token and restrict substitution; that external protection is not present in the source."
  ],
  "evidenceRefs": [
    "origin-config",
    "origin-load",
    "origin-global-token",
    "origin-send",
    "origin-cli",
    "origin-gui"
  ],
  "limitations": [
    "No network request or credential was used.",
    "The attack does not obtain a token when no ambient token exists and no matching stored credential is available."
  ],
  "method": "static source trace",
  "summary": "Source establishes the stated chain and prerequisites. No application code was executed."
}

## 回帰確認
[
  "With a real test token and an unapproved workspace URL, opening CLI and GUI views must send no Authorization header to that origin.",
  "Verify approved URL-keyed credentials and proxy placeholders retain their intended behavior."
]

## 場所
[
  {
    "endLine": 15,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 9
  },
  {
    "endLine": 54,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "entrypoint",
    "startLine": 48
  },
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 49
  },
  {
    "endLine": 58,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "endLine": 403,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 399
  },
  {
    "endLine": 1220,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  },
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "propagation",
    "startLine": 18
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "propagation",
    "startLine": 48
  },
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "propagation",
    "startLine": 51
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "propagation",
    "startLine": 55
  },
  {
    "endLine": 94,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 90
  },
  {
    "endLine": 403,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 401
  },
  {
    "endLine": 1224,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 114
  }
]

---

## [F021] Opening an untrusted workspace can send TOPO_TOKEN to its configured server

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F021
findingId: csf_3a7be37cab03e48112d7e157
occurrenceId: occ_1ba833ec9d7218f33494866a
重要度: medium
共通原因: token-origin
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A supplied `.topo/config.toml` can choose an attacker server. An ordinary CLI or GUI workspace open sends the user's real `TOPO_TOKEN` to that server before any trust confirmation or binding to the intended server.

## 根本原因
{
  "evidenceRefs": [
    "cloud-config-input",
    "automatic-cloud-open",
    "ambient-token",
    "bearer-network-sink",
    "automatic-graph-fetch",
    "http-remote-fetch"
  ],
  "summary": "`CloudConfig` decodes a URL from workspace-controlled configuration. `topo_cloud::open()` passes that URL to `credentials::token()`; the environment branch ignores the requested URL and returns `TOPO_TOKEN`. The remote adapter immediately fetches the graph, and `Client::send()` attaches that credential to a request built from the same untrusted URL. The missing control is a trusted binding between the environment credential and its permitted destination."
}

## 修正方針
Bind an environment credential to a trusted server origin supplied outside workspace files, and refuse authenticated requests when workspace cloud.url does not match that origin. Keep URL-keyed credentials and validate transport separately.

## 検証
{
  "counterEvidence": [
    "Stored credentials are selected by exact server URL when TOPO_TOKEN is absent (credentials.rs:41-55).",
    "Client uses a platform certificate verifier and a 15-second timeout (client.rs:31-39); an attacker can use a valid certificate for its own host.",
    "The token may be an inert proxy placeholder (credentials.rs:49-50); a proxy that enforces destinations can prevent secret disclosure."
  ],
  "evidenceRefs": [
    "cloud-config-input",
    "automatic-cloud-open",
    "ambient-token",
    "bearer-network-sink",
    "automatic-graph-fetch",
    "http-remote-fetch"
  ],
  "limitations": [
    "Actual use of real TOPO_TOKEN and access to a lower-trust workspace are conditional; no credential values were read or transmitted."
  ],
  "method": "Independent static source trace",
  "summary": "The parent verified configuration loading, environment precedence, immediate remote fetch, and bearer-header construction. An attacker needs only a cloud table naming its HTTPS server and an arbitrary workspace; the token is sent even if the server later returns an invalid graph."
}

## 回帰確認
[
  "Open a supplied workspace with a different cloud URL while TOPO_TOKEN is set and assert that no authenticated request is sent.",
  "Verify the intended server receives the token and URL-keyed stored credentials retain their existing behavior."
]

## 場所
[
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 51
  },
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 18
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "entrypoint",
    "startLine": 48
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 403
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 108
  },
  {
    "endLine": 1218,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1217
  }
]

---

## [F025] Opening an untrusted workspace sends TOPO_TOKEN to its chosen server

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F025
findingId: csf_3fab18f6ee8ffe087ef06ba0
occurrenceId: occ_6eeb558e47745d526d4c634d
重要度: medium
共通原因: token-origin
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

Opening or listing a workspace with an attacker-controlled `.topo/config.toml` sends a real `TOPO_TOKEN` to the configured cloud URL. The captured bearer token can then be replayed against its legitimate server with the victim's granted permissions.

## 根本原因
{
  "evidenceRefs": [
    "workspace-cloud-config",
    "automatic-remote-open",
    "unbound-env-token",
    "bearer-to-selected-url"
  ],
  "summary": "`config::load()` accepts the workspace's cloud URL, and `topo_cloud::open()` uses it to select both client and credential. `credentials::token()` returns `TOPO_TOKEN` before checking the URL-keyed credential store, so a lower-trust workspace can choose the recipient of a higher-trust secret. `Client::send()` attaches that secret as a bearer header to the selected URL."
}

## 修正方針
Bind environment credentials to an explicit trusted server origin outside workspace data and reject workspace-selected destinations that do not match it before attaching any credential.

## 検証
{
  "counterEvidence": [
    "Stored credentials are indexed by exact URL (credentials.rs:41-46), preventing this path when TOPO_TOKEN is absent.",
    "docs/cloud/api.md:83-91 describes optional proxy-only placeholder tokens; an outbound proxy that substitutes secrets only for authorized origins can block disclosure. The source does not require that deployment."
  ],
  "evidenceRefs": [
    "automatic-remote-open",
    "unbound-env-token",
    "bearer-to-selected-url"
  ],
  "limitations": [
    "Requires a real reusable TOPO_TOKEN and an attacker-controlled workspace cloud configuration.",
    "No live token, network request or exploit was used."
  ],
  "method": "static_source_review",
  "status": "validated",
  "summary": "Source tracing confirms CLI graph commands and GUI startup call cloud open, remote opening fetches the graph, and the HTTP client sends the environment token to the workspace-selected destination without a separate trust decision."
}

## 回帰確認
[
  "Open a workspace pointing at another origin with TOPO_TOKEN set and assert that no authorization header reaches that origin.",
  "Retain exact-origin authentication for explicitly trusted servers and proxy placeholder workflows."
]

## 場所
[
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 51
  },
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 28
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "propagation",
    "startLine": 48
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 403
  },
  {
    "endLine": 1219,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "propagation",
    "startLine": 108
  }
]

---

## [F029] Opening a checked-out workspace can send TOPO_TOKEN to its configured server

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F029
findingId: csf_015c42fec83665cc996545df
occurrenceId: occ_1e15d00e31cdded067da2748
重要度: medium
共通原因: token-origin
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

An attacker who changes a repository's committed `[cloud].url` can receive the real `TOPO_TOKEN` inherited by an agent or local process when it runs an ordinary topo command. The stolen token retains its original server permissions and workspace scope.

## 根本原因
{
  "evidenceRefs": [
    "cloud-config",
    "cloud-open",
    "environment-token",
    "bearer-send",
    "agent-checkout",
    "ordinary-open"
  ],
  "summary": "`config::load()` obtains the HTTP origin from repository-controlled `config.toml`. `topo_cloud::open()` passes that origin into `credentials::token()`, but the environment branch returns `TOPO_TOKEN` without any destination comparison. `Client::send()` then adds it as a bearer header for that origin. File-backed credentials are keyed by URL and do not share this specific failure."
}

## 修正方針
Bind environment credentials to a separately trusted server origin and reject workspace-selected origins that do not match it before constructing any authenticated request. Keep this binding outside repository-controlled configuration.

## 検証
{
  "assertions": [
    "No origin allowlist or trusted origin binding exists in the environment-token branch.",
    "Failure to return a valid graph does not prevent the request header from reaching the configured origin."
  ],
  "counterEvidence": [
    "File-backed credentials require an exact URL key.",
    "Platform TLS verification protects transit but cannot establish that a repository-selected HTTPS origin is authorized to receive this token.",
    "A destination-restricting secret-substitution proxy can block this attack; a placeholder alone is not the real secret."
  ],
  "evidenceRefs": [
    "cloud-config",
    "cloud-open",
    "environment-token",
    "bearer-send",
    "agent-checkout",
    "ordinary-open"
  ],
  "limitations": [
    "Requires a real token rather than a proxy-only placeholder and egress to the selected origin.",
    "No live deployment or runtime reproduction was inspected."
  ],
  "method": "independent static source trace",
  "summary": "Parent review verified the full checkout-config to bearer-header path and the documented secret-store agent workflow. An attacker needs only to replace `[cloud].url` with a reachable server they control; opening the workspace initiates a graph fetch with the inherited credential."
}

## 回帰確認
[
  "Opening a workspace with a changed cloud URL must not send a real environment token to that origin.",
  "File-backed credentials must remain bound to the exact trusted server."
]

## 場所
[
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 51
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "propagation",
    "startLine": 49
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 403
  }
]

---

## [F033] Opening an untrusted workspace sends the ambient cloud token to its server

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F033
findingId: csf_95f6221dd761e3535568fe27
occurrenceId: occ_0cbe375ce41605d9438ff41b
重要度: medium
共通原因: token-origin
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

An ordinary CLI command or GUI startup in a supplied workspace sends a real `TOPO_TOKEN` to the URL in `.topo/config.toml`. A workspace author can receive and replay that credential against its legitimate server.

## 根本原因
{
  "evidenceRefs": [
    "cloud-link-input",
    "cli-auto-open",
    "cloud-open",
    "env-token-unbound",
    "credential-request-sink",
    "immediate-remote-read",
    "stored-origin-control"
  ],
  "summary": "`config::load` accepts a project cloud URL. `topo_cloud::open` treats that URL as the client destination and requests a token for it. `credentials::token` returns `TOPO_TOKEN` without checking the URL, so project data gains authority over the recipient of a process credential. `Workspace::open_remote` immediately fetches, and `Client::send` places the credential in the Authorization header."
}

## 修正方針
Bind environment credentials to an independently trusted server origin and reject mismatched project URLs before any authenticated request. Establish and persist explicit origin trust outside the workspace configuration, and normalize server URLs consistently.

## 検証
{
  "counterEvidence": [
    "Stored credentials are keyed by exact URL and do not expose other-origin stored tokens through this path.",
    "TLS authenticates the chosen origin but does not establish it is the intended credential recipient.",
    "The documented outbound-proxy mode sends only a placeholder unless the proxy substitutes a secret for an allowed domain (docs/cloud/api.md:84-95); that mode does not expose a real token to an arbitrary host."
  ],
  "evidenceRefs": [
    "cloud-link-input",
    "cli-auto-open",
    "cloud-open",
    "env-token-unbound",
    "credential-request-sink",
    "immediate-remote-read",
    "stored-origin-control"
  ],
  "limitations": [
    "Requires a real reusable TOPO_TOKEN and an attacker-supplied workspace opened by the victim.",
    "Actual deployment and runtime reproduction were not observed."
  ],
  "method": "independent_static_source_trace",
  "status": "validated",
  "summary": "A workspace author needs only to supply its cloud URL and convince a token-bearing user or agent to run a normal command there. The first graph request sends the real environment token before the response is checked; an API-compatible response is unnecessary to steal it. CLI and GUI callers open the cloud workspace automatically."
}

## 回帰確認
[
  "With a real environment token and an untrusted cloud URL, opening/listing the workspace must send no bearer request.",
  "A token bound to server A must not be sent to server B through project configuration.",
  "Exercise both CLI and GUI startup plus linked cloud commands."
]

## 場所
[
  {
    "endLine": 34,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 9
  },
  {
    "endLine": 403,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 393
  },
  {
    "endLine": 58,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "propagation",
    "startLine": 48
  },
  {
    "endLine": 56,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 49
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "endLine": 95,
    "path": "crates/topo-core/src/store.rs",
    "role": "outcome",
    "startLine": 90
  },
  {
    "endLine": 46,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "expected_control",
    "startLine": 40
  },
  {
    "endLine": 1219,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1217
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "propagation",
    "startLine": 109
  },
  {
    "endLine": 185,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "propagation",
    "startLine": 180
  },
  {
    "endLine": 226,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "propagation",
    "startLine": 224
  }
]

---

## [F043] Opening an untrusted workspace sends TOPO_TOKEN to its chosen server

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F043
findingId: csf_64e39aafcb180197d2961a02
occurrenceId: occ_77c83a5870163143b22b2609
重要度: medium
共通原因: token-origin
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A workspace supplier or repository contributor can change `[cloud].url` so that normal CLI graph commands, GUI startup, or a documented checkout-based agent workflow send the process's real TOPO_TOKEN to an attacker-controlled origin. The bearer can be replayed at its legitimate server with its existing workspace scope or account authority.

## 根本原因
{
  "evidenceRefs": [
    "cloud-config",
    "cloud-open",
    "environment-token",
    "bearer-send",
    "agent-checkout",
    "ordinary-open",
    "workspace-cloud-config"
  ],
  "summary": "Workspace-controlled config.toml selects cloud.url; topo_cloud::open constructs the remote client using credentials::token(cloud.url) and immediately fetches its graph. The environment branch returns TOPO_TOKEN for every destination before consulting the exact-URL credential store. Client::send appends the API path and attaches Authorization: Bearer to that selected URL without a separately trusted recipient binding. TLS authenticates the selected HTTPS endpoint, rather than establishing its authority to receive this credential."
}

## 修正方針
Bind environment credentials to an explicit trusted server origin outside repository/workspace data, and reject workspace-selected destinations that do not match it before constructing any authenticated request. Preserve exact-URL file-backed credentials and destination-restricted secret-substitution proxy workflows.

## 検証
{
  "assertions": [
    "No origin allowlist or trusted origin binding exists in the environment-token branch.",
    "Failure to return a valid graph does not prevent the request header from reaching the configured origin."
  ],
  "counterEvidence": [
    "File-backed credentials require an exact URL key.",
    "Platform TLS verification protects transit but cannot establish that a repository-selected HTTPS origin is authorized to receive this token.",
    "A destination-restricting secret-substitution proxy can block this attack; a placeholder alone is not the real secret.",
    "Stored credentials are indexed by exact URL (credentials.rs:41-46), preventing this path when TOPO_TOKEN is absent.",
    "docs/cloud/api.md:83-91 describes optional proxy-only placeholder tokens; an outbound proxy that substitutes secrets only for authorized origins can block disclosure. The source does not require that deployment."
  ],
  "evidenceRefs": [
    "cloud-config",
    "cloud-open",
    "environment-token",
    "bearer-send",
    "agent-checkout",
    "ordinary-open",
    "workspace-cloud-config"
  ],
  "limitations": [
    "Requires a real token rather than a proxy-only placeholder and egress to the selected origin.",
    "No live deployment or runtime reproduction was inspected.",
    "Requires a real reusable TOPO_TOKEN and an attacker-controlled workspace cloud configuration.",
    "No live token, network request or exploit was used."
  ],
  "method": "semantic reduction of already-validated static source findings; no additional validation",
  "status": "validated",
  "summary": "The two validated findings establish the same automatic first-request disclosure. No explicit login/link approval or valid graph response is needed: rejecting the response cannot undo the already-sent header. Exact-URL stored credentials prevent this particular flow when TOPO_TOKEN is absent; a real reusable token, reachable attacker origin, and no enforcing outbound origin restriction are required."
}

## 回帰確認
[
  "Opening a workspace with a changed cloud URL must not send a real environment token to that origin.",
  "File-backed credentials must remain bound to the exact trusted server.",
  "Open a workspace pointing at another origin with TOPO_TOKEN set and assert that no authorization header reaches that origin.",
  "Retain exact-origin authentication for explicitly trusted servers and proxy placeholder workflows."
]

## 場所
[
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 51
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "propagation",
    "startLine": 49
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 403
  },
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 28
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "propagation",
    "startLine": 48
  },
  {
    "endLine": 1219,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "propagation",
    "startLine": 108
  }
]

---

## [F047] Opening an untrusted workspace can send TOPO_TOKEN to its author

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F047
findingId: csf_f42451787a2eeab5f0a4562f
occurrenceId: occ_e9ed13891f730e296ca98fa3
重要度: medium
共通原因: token-origin
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

Ordinary CLI commands and GUI startup automatically read the workspace's `[cloud].url`. `credentials::token()` returns the process's `TOPO_TOKEN` for any URL, so a malicious workspace can direct that bearer credential to an attacker-controlled server.

## 根本原因
{
  "evidenceRefs": [
    "workspace-cloud-config",
    "automatic-cloud-open",
    "unbound-environment-token",
    "remote-open-fetch",
    "bearer-header-send",
    "file-credential-binding"
  ],
  "summary": "`config::load()` decodes a URL from the opened workspace. `open()` uses it to request credentials, but `credentials::token()` returns TOPO_TOKEN before consulting URL-keyed credentials. `Workspace::open_remote()` immediately fetches and `Client::send()` puts that secret into the Authorization header for the workspace-selected destination. The missing invariant is binding a process credential to a trusted server independently of repository configuration."
}

## 修正方針
Bind TOPO_TOKEN to a trusted server origin supplied independently of workspace files, and reject destination mismatches before any request. Preserve verbatim proxy placeholders while requiring host binding; gate first use of workspace-provided destinations on explicit trust.

## 検証
{
  "counterEvidence": [
    "Absent TOPO_TOKEN, stored credentials are keyed by the exact configured URL and an unknown server fails as not signed in.",
    "Placeholder-based outbound proxies may enforce host binding outside this repository; the concrete credential disclosure path requires a real environment token or a proxy that releases its secret to the chosen destination.",
    "The attacker must induce workspace opening; this is not unauthenticated access to the victim's process."
  ],
  "evidenceRefs": [
    "workspace-cloud-config",
    "automatic-cloud-open",
    "unbound-environment-token",
    "remote-open-fetch",
    "bearer-header-send",
    "cli-auto-open",
    "gui-auto-open",
    "file-credential-binding"
  ],
  "limitations": [
    "No application code was executed or live deployment tested.",
    "Externally enforced proxy destination restrictions are not visible in the source."
  ],
  "method": "static source trace",
  "status": "validated",
  "summary": "A workspace author can supply a reachable HTTPS endpoint in `.topo/config.toml`. An ordinary `topo ls` or GUI open reaches the first graph request with the victim's environment token before the remote response is interpreted. Exact-URL file lookup and TLS certificate validation do not bind the environment credential to its intended server."
}

## 回帰確認
[
  "Opening attacker-controlled cloud configuration with a token bound to another origin sends no network request or Authorization header.",
  "A matching trusted origin still receives the exact token or proxy placeholder."
]

## 場所
[
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 18
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "entrypoint",
    "startLine": 47
  },
  {
    "endLine": 56,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 49
  },
  {
    "endLine": 95,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 90
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "endLine": 404,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 400
  },
  {
    "endLine": 1225,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  },
  {
    "endLine": 47,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "expected_control",
    "startLine": 40
  }
]

---

## [F051] Opening a shared workspace can send TOPO_TOKEN to its contributor

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F051
findingId: csf_9e9cfe6333cae3548901c67c
occurrenceId: occ_6a7fdb76974a1e237270e082
重要度: medium
共通原因: token-origin
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A contributor can change the committed `.topo/config.toml` cloud URL to an endpoint they control. When a user or agent with a real `TOPO_TOKEN` runs an ordinary CLI command or opens the GUI, the client sends that token to the supplied endpoint before returning the graph.

## 根本原因
{
  "evidenceRefs": [
    "shared-config",
    "automatic-open",
    "ambient-token",
    "initial-fetch",
    "credential-send"
  ],
  "summary": "`config::load()` reads the repository-selected server URL. `topo_cloud::open()` constructs a client from that URL and `credentials::token()`, whose environment branch returns `TOPO_TOKEN` without binding it to an independently trusted destination. `open_remote()` immediately fetches the graph, and `Client::send()` sends the bearer header to the selected URL."
}

## 修正方針
Bind ambient credentials to a separately trusted server origin, such as a platform-provided `TOPO_SERVER_URL`, and reject workspace URLs that do not match before any request. Preserve verbatim placeholder support for proxies that enforce the same audience boundary.

## 検証
{
  "counterEvidence": [
    "File-stored credentials are looked up by exact URL (credentials.rs:41-55).",
    "A domain-bound secret substitution proxy sends only a placeholder to disallowed origins (docs/cloud/api.md:84-96).",
    "TLS verification protects transport to the selected endpoint, but does not establish that endpoint as the token's intended audience."
  ],
  "evidenceRefs": [
    "cli-open",
    "gui-open",
    "shared-config",
    "ambient-token",
    "initial-fetch",
    "credential-send"
  ],
  "limitations": [
    "Requires a real environment token and a victim opening attacker-controlled configuration; proxy-only placeholders are excluded.",
    "Validated offline; no token was transmitted."
  ],
  "method": "static source trace",
  "summary": "The normal CLI and GUI entry points reach the automatic fetch. An attacker needs only to supply a syntactically valid HTTPS URL and workspace string in configuration; no sign-in or write approval occurs before transmission."
}

## 回帰確認
[
  "With a trusted token origin and a different URL in workspace configuration, ordinary read and GUI startup must fail before sending an Authorization header.",
  "Confirm matching trusted origins and domain-bound proxy placeholders continue to work."
]

## 場所
[
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 18
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "propagation",
    "startLine": 48
  },
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 49
  },
  {
    "endLine": 95,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 90
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "endLine": 405,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 401
  },
  {
    "endLine": 1224,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  }
]

---

## [F058] Opening an untrusted workspace sends TOPO_TOKEN to its chosen server

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F058
findingId: csf_01c239e3e051740e374ab123
occurrenceId: occ_1a08be9f87cdae9375ef9e30
重要度: medium
共通原因: token-origin
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A workspace supplier or repository contributor can change `[cloud].url` so that normal CLI graph commands, GUI startup, or a documented checkout-based agent workflow send the process's real TOPO_TOKEN to an attacker-controlled origin. The bearer can be replayed at its legitimate server with its existing workspace scope or account authority.

## 根本原因
{
  "evidenceRefs": [
    "cloud-config",
    "cloud-open",
    "environment-token",
    "bearer-send",
    "agent-checkout",
    "ordinary-open",
    "workspace-cloud-config",
    "origin-config",
    "origin-load",
    "origin-send",
    "origin-cli",
    "origin-gui"
  ],
  "summary": "Workspace-controlled config.toml selects cloud.url; topo_cloud::open constructs the remote client using credentials::token(cloud.url) and immediately fetches its graph. The environment branch returns TOPO_TOKEN for every destination before consulting the exact-URL credential store. Client::send appends the API path and attaches Authorization: Bearer to that selected URL without a separately trusted recipient binding. TLS authenticates the selected HTTPS endpoint, rather than establishing its authority to receive this credential."
}

## 修正方針
Bind environment credentials to an explicit trusted server origin outside repository/workspace data, and reject or require independent user approval for workspace-selected destinations that do not match it before constructing any authenticated request. Enforce this during CLI/GUI startup and linked operations. Preserve exact-URL file-backed credentials and destination-restricted secret-substitution proxy workflows.

## 検証
{
  "assertions": [
    "No origin allowlist or trusted origin binding exists in the environment-token branch.",
    "Failure to return a valid graph does not prevent the request header from reaching the configured origin."
  ],
  "counterEvidence": [
    "File-backed credentials require an exact URL key.",
    "Platform TLS verification protects transit but cannot establish that a repository-selected HTTPS origin is authorized to receive this token.",
    "A destination-restricting secret-substitution proxy can block this attack; a placeholder alone is not the real secret.",
    "Stored credentials are indexed by exact URL (credentials.rs:41-46), preventing this path when TOPO_TOKEN is absent.",
    "docs/cloud/api.md:83-91 describes optional proxy-only placeholder tokens; an outbound proxy that substitutes secrets only for authorized origins can block disclosure. The source does not require that deployment.",
    "Stored credentials are keyed by exact URL, so this chain depends on a real ambient token.",
    "TOPO_CLOUD_URL/--url select general management-command destinations but do not override ordinary linked workspace opening.",
    "A proxy may hold the real token and restrict substitution; that external protection is not present in the source."
  ],
  "evidenceRefs": [
    "cloud-config",
    "cloud-open",
    "environment-token",
    "bearer-send",
    "agent-checkout",
    "ordinary-open",
    "workspace-cloud-config",
    "origin-config",
    "origin-load",
    "origin-send",
    "origin-cli",
    "origin-gui"
  ],
  "limitations": [
    "Requires a real token rather than a proxy-only placeholder and egress to the selected origin.",
    "No live deployment or runtime reproduction was inspected.",
    "Requires a real reusable TOPO_TOKEN and an attacker-controlled workspace cloud configuration.",
    "No live token, network request or exploit was used.",
    "No network request or credential was used.",
    "The attack does not obtain a token when no ambient token exists and no matching stored credential is available."
  ],
  "method": "semantic reduction of already-validated static source findings; no additional validation",
  "status": "validated",
  "summary": "Three supplied validated findings establish the same automatic first-request disclosure before any valid graph response or explicit login/link approval. Exact-URL stored credentials block this flow when TOPO_TOKEN is absent. General management-command --url/TOPO_CLOUD_URL overrides do not override ordinary linked workspace opening. A real reusable ambient token, reachable attacker origin, and absence of an enforcing outbound origin restriction are prerequisites."
}

## 回帰確認
[
  "Opening a workspace with a changed cloud URL must not send a real environment token to that origin.",
  "File-backed credentials must remain bound to the exact trusted server.",
  "Open a workspace pointing at another origin with TOPO_TOKEN set and assert that no authorization header reaches that origin.",
  "Retain exact-origin authentication for explicitly trusted servers and proxy placeholder workflows.",
  "With a real test token and an unapproved workspace URL, opening CLI and GUI views must send no Authorization header to that origin.",
  "Verify approved URL-keyed credentials and proxy placeholders retain their intended behavior."
]

## 場所
[
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 51
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "propagation",
    "startLine": 49
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 403
  },
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 28
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "propagation",
    "startLine": 48
  },
  {
    "endLine": 1219,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "propagation",
    "startLine": 108
  },
  {
    "endLine": 15,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 9
  },
  {
    "endLine": 54,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "entrypoint",
    "startLine": 48
  },
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 49
  },
  {
    "endLine": 58,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "endLine": 403,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 399
  },
  {
    "endLine": 1220,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  },
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "propagation",
    "startLine": 18
  },
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "propagation",
    "startLine": 51
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "propagation",
    "startLine": 55
  },
  {
    "endLine": 94,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 90
  },
  {
    "endLine": 403,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 401
  },
  {
    "endLine": 1224,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 114
  }
]

---

## [F061] Opening a shared workspace can send the agent token to its chosen server

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F061
findingId: csf_5db26297af9239aede082dc1
occurrenceId: occ_ad018a33787dd5a004b72bf3
重要度: medium
共通原因: token-origin
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

With a real secret in `TOPO_TOKEN`, opening an attacker-supplied `.topo/config.toml` sends that secret to the file's `cloud.url`. Ordinary CLI queries and GUI startup fetch the graph automatically, letting a repository contributor steal the token and use its workspace privileges.

## 根本原因
{
  "evidenceRefs": [
    "workspace-cloud-url",
    "open-cloud-with-token",
    "global-token-override",
    "bearer-to-selected-url"
  ],
  "summary": "`config::load` reads the repository-supplied `cloud.url`, and `open` passes it to `credentials::token`. The environment override returns `TOPO_TOKEN` without checking that URL. `Client::send` then adds the bearer header to the URL and sends the initial graph request, crossing from shared workspace content into the victim's credential authority."
}

## 修正方針
Bind the environment token to a trusted server origin selected outside workspace content, and reject workspace cloud URLs that do not match that origin before adding Authorization. Preserve verbatim placeholder support after destination validation.

## 検証
{
  "counterEvidence": [
    "File credentials are keyed by exact URL at credentials.rs:41-46 and are not disclosed to a new URL.",
    "docs/cloud/api.md:84-93 describes proxies substituting secrets only for an allowed HTTPS domain; those deployments may expose only a placeholder to the attacker.",
    "The server enforces token workspace scope and current membership at topo-server/src/auth.rs:55-63."
  ],
  "evidenceRefs": [
    "open-cloud-with-token",
    "global-token-override",
    "bearer-to-selected-url"
  ],
  "limitations": [
    "Requires an actual readable secret in TOPO_TOKEN, rather than a domain-bound proxy placeholder.",
    "Static review only; no real credential or outbound request used."
  ],
  "method": "Independent static source trace",
  "status": "validated",
  "summary": "The parent verified that ordinary CLI dispatch calls `topo_cloud::open`, `Workspace::open_remote` fetches before snapshot validation, and the client transmits the environment token to the unvalidated workspace-selected destination."
}

## 回帰確認
[
  "Opening a workspace with a different cloud origin must not transmit TOPO_TOKEN.",
  "The configured trusted origin receives the original token or placeholder unchanged."
]

## 場所
[
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 51
  },
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 18
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "propagation",
    "startLine": 48
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 403
  },
  {
    "endLine": 1219,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1217
  }
]

---

## [F065] Opening a shared workspace can send the agent token to its chosen server

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F065
findingId: csf_0606651a72875fb675f08b21
occurrenceId: occ_ed9c1bb2e20154f5a57ef43b
重要度: medium
共通原因: token-origin
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

With a real secret in `TOPO_TOKEN`, opening an attacker-supplied `.topo/config.toml` sends that secret to the file's `cloud.url`. Ordinary CLI queries and GUI startup fetch the graph automatically, letting a repository contributor steal the token and use its workspace privileges.

## 根本原因
{
  "evidenceRefs": [
    "workspace-cloud-url",
    "open-cloud-with-token",
    "global-token-override",
    "bearer-to-selected-url"
  ],
  "summary": "`config::load` reads the repository-supplied `cloud.url`, and `open` passes it to `credentials::token`. The environment override returns `TOPO_TOKEN` without checking that URL. `Client::send` then adds the bearer header to the URL and sends the initial graph request, crossing from shared workspace content into the victim's credential authority."
}

## 修正方針
Bind the environment token to a trusted server origin selected outside workspace content, and reject workspace cloud URLs that do not match that origin before adding Authorization. Preserve verbatim placeholder support after destination validation.

## 検証
{
  "counterEvidence": [
    "File credentials are keyed by exact URL at credentials.rs:41-46 and are not disclosed to a new URL.",
    "docs/cloud/api.md:84-93 describes proxies substituting secrets only for an allowed HTTPS domain; those deployments may expose only a placeholder to the attacker.",
    "The server enforces token workspace scope and current membership at topo-server/src/auth.rs:55-63."
  ],
  "evidenceRefs": [
    "open-cloud-with-token",
    "global-token-override",
    "bearer-to-selected-url"
  ],
  "limitations": [
    "Requires an actual readable secret in TOPO_TOKEN, rather than a domain-bound proxy placeholder.",
    "Static review only; no real credential or outbound request used."
  ],
  "method": "Independent static source trace",
  "summary": "The parent verified that ordinary CLI dispatch calls `topo_cloud::open`, `Workspace::open_remote` fetches before snapshot validation, and the client transmits the environment token to the unvalidated workspace-selected destination."
}

## 回帰確認
[
  "Opening a workspace with a different cloud origin must not transmit TOPO_TOKEN.",
  "The configured trusted origin receives the original token or placeholder unchanged."
]

## 場所
[
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 51
  },
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 18
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "propagation",
    "startLine": 48
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 403
  },
  {
    "endLine": 1219,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1217
  }
]

---

## [F066] Opening an untrusted workspace sends TOPO_TOKEN to its chosen server

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F066
findingId: csf_380580739ca3781697b99b29
occurrenceId: occ_78a221aa10aa2aae1ff1bfe3
重要度: medium
共通原因: token-origin
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A workspace supplier or repository contributor can change `[cloud].url` so that normal CLI graph commands, GUI startup, or a documented checkout-based agent workflow send the process's real TOPO_TOKEN to an attacker-controlled origin. The bearer can be replayed at its legitimate server with its existing workspace scope or account authority.

## 根本原因
{
  "evidenceRefs": [
    "cloud-config",
    "cloud-open",
    "environment-token",
    "bearer-send",
    "agent-checkout",
    "ordinary-open",
    "workspace-cloud-config",
    "origin-config",
    "origin-load",
    "origin-send",
    "origin-cli",
    "origin-gui",
    "dedup-0003-0-cloud-config-input",
    "dedup-0003-0-ambient-token",
    "dedup-0003-0-bearer-network-sink",
    "dedup-0003-0-automatic-graph-fetch",
    "dedup-0003-0-http-remote-fetch"
  ],
  "summary": "Workspace-controlled config.toml selects cloud.url; topo_cloud::open constructs the remote client using credentials::token(cloud.url) and immediately fetches its graph. The environment branch returns TOPO_TOKEN for every destination before consulting the exact-URL credential store. Client::send appends the API path and attaches Authorization: Bearer to that selected URL without a separately trusted recipient binding. TLS authenticates the selected HTTPS endpoint, rather than establishing its authority to receive this credential."
}

## 修正方針
Bind environment credentials to an explicit trusted server origin outside repository/workspace data, and reject or require independent user approval for workspace-selected destinations that do not match it before constructing any authenticated request. Enforce this during CLI/GUI startup and linked operations. Preserve exact-URL file-backed credentials and destination-restricted secret-substitution proxy workflows.

## 検証
{
  "assertions": [
    "No origin allowlist or trusted origin binding exists in the environment-token branch.",
    "Failure to return a valid graph does not prevent the request header from reaching the configured origin."
  ],
  "counterEvidence": [
    "File-backed credentials require an exact URL key.",
    "Platform TLS verification protects transit but cannot establish that a repository-selected HTTPS origin is authorized to receive this token.",
    "A destination-restricting secret-substitution proxy can block this attack; a placeholder alone is not the real secret.",
    "Stored credentials are indexed by exact URL (credentials.rs:41-46), preventing this path when TOPO_TOKEN is absent.",
    "docs/cloud/api.md:83-91 describes optional proxy-only placeholder tokens; an outbound proxy that substitutes secrets only for authorized origins can block disclosure. The source does not require that deployment.",
    "Stored credentials are keyed by exact URL, so this chain depends on a real ambient token.",
    "TOPO_CLOUD_URL/--url select general management-command destinations but do not override ordinary linked workspace opening.",
    "A proxy may hold the real token and restrict substitution; that external protection is not present in the source.",
    "Stored credentials are selected by exact server URL when TOPO_TOKEN is absent (credentials.rs:41-55).",
    "Client uses a platform certificate verifier and a 15-second timeout (client.rs:31-39); an attacker can use a valid certificate for its own host.",
    "The token may be an inert proxy placeholder (credentials.rs:49-50); a proxy that enforces destinations can prevent secret disclosure."
  ],
  "evidenceRefs": [
    "cloud-config",
    "cloud-open",
    "environment-token",
    "bearer-send",
    "agent-checkout",
    "ordinary-open",
    "workspace-cloud-config",
    "origin-config",
    "origin-load",
    "origin-send",
    "origin-cli",
    "origin-gui",
    "dedup-0003-0-cloud-config-input",
    "dedup-0003-0-ambient-token",
    "dedup-0003-0-bearer-network-sink",
    "dedup-0003-0-automatic-graph-fetch",
    "dedup-0003-0-http-remote-fetch"
  ],
  "limitations": [
    "Requires a real token rather than a proxy-only placeholder and egress to the selected origin.",
    "No live deployment or runtime reproduction was inspected.",
    "Requires a real reusable TOPO_TOKEN and an attacker-controlled workspace cloud configuration.",
    "No live token, network request or exploit was used.",
    "No network request or credential was used.",
    "The attack does not obtain a token when no ambient token exists and no matching stored credential is available.",
    "Actual use of real TOPO_TOKEN and access to a lower-trust workspace are conditional; no credential values were read or transmitted."
  ],
  "method": "semantic reduction of already-validated Standard source findings; no additional validation",
  "status": "validated",
  "summary": "Four supplied validated findings establish the same automatic first-request disclosure before any valid graph response or explicit login/link approval. Exact-URL stored credentials block this flow when TOPO_TOKEN is absent. General management-command --url/TOPO_CLOUD_URL overrides do not override ordinary linked workspace opening. A real reusable ambient token, reachable attacker origin, and absence of an enforcing outbound origin restriction are prerequisites."
}

## 回帰確認
[
  "Opening a workspace with a changed cloud URL must not send a real environment token to that origin.",
  "File-backed credentials must remain bound to the exact trusted server.",
  "Open a workspace pointing at another origin with TOPO_TOKEN set and assert that no authorization header reaches that origin.",
  "Retain exact-origin authentication for explicitly trusted servers and proxy placeholder workflows.",
  "With a real test token and an unapproved workspace URL, opening CLI and GUI views must send no Authorization header to that origin.",
  "Verify approved URL-keyed credentials and proxy placeholders retain their intended behavior.",
  "Open a supplied workspace with a different cloud URL while TOPO_TOKEN is set and assert that no authenticated request is sent.",
  "Verify the intended server receives the token and URL-keyed stored credentials retain their existing behavior."
]

## 場所
[
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 51
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "propagation",
    "startLine": 49
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 403
  },
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 28
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "propagation",
    "startLine": 48
  },
  {
    "endLine": 1219,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "propagation",
    "startLine": 108
  },
  {
    "endLine": 15,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 9
  },
  {
    "endLine": 54,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "entrypoint",
    "startLine": 48
  },
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 49
  },
  {
    "endLine": 58,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "endLine": 403,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 399
  },
  {
    "endLine": 1220,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  },
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "propagation",
    "startLine": 18
  },
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "propagation",
    "startLine": 51
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "propagation",
    "startLine": 55
  },
  {
    "endLine": 94,
    "path": "crates/topo-core/src/store.rs",
    "role": "propagation",
    "startLine": 90
  },
  {
    "endLine": 403,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 401
  },
  {
    "endLine": 1224,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 114
  },
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 18
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "entrypoint",
    "startLine": 48
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 108
  },
  {
    "endLine": 1218,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1217
  }
]

---

## [F070] Opening a shared workspace sends TOPO_TOKEN to its configured server

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F070
findingId: csf_817c913e415eaa2c3c6cb038
occurrenceId: occ_a4872e5c5c7ff37f7d2f03fb
重要度: medium
共通原因: token-origin
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A contributor can change the committed `[cloud].url` in `.topo/config.toml`. When a victim runs a normal CLI command or opens the GUI with a real `TOPO_TOKEN`, the client sends that token to the contributor's server before authenticating or trusting that destination. The stolen token can be replayed against its legitimate server with its existing workspace or account permissions.

## 根本原因
{
  "evidenceRefs": [
    "cloud-link-fields",
    "cloud-cli-open",
    "cloud-open",
    "cloud-env-token",
    "cloud-bearer-send"
  ],
  "summary": "`config::load` decodes the workspace-owned server URL. `topo_cloud::open` passes that URL to both `Client::new` and `credentials::token`. The credential resolver uses `TOPO_TOKEN` before URL-indexed stored credentials and accepts it for any destination. `Client::send` then attaches the token to the URL selected by the workspace. No trusted recipient binding separates committed configuration from the user's environment secret."
}

## 修正方針
Bind environment credentials to an independently trusted server origin and reject a workspace URL that does not match before sending any request. Preserve URL-indexed disk credentials; require explicit trust before a newly supplied link can receive an environment token.

## 検証
{
  "evidenceRefs": [
    "cloud-cli-open",
    "cloud-open",
    "cloud-env-token",
    "cloud-bearer-send"
  ],
  "limitations": [
    "Offline review; no runtime reproduction or deployed server was contacted.",
    "Requires a real token in TOPO_TOKEN. A proxy that substitutes secrets only for approved origins may prevent disclosure."
  ],
  "method": "static source trace",
  "summary": "The parent independently verified the full path from committed cloud URL to `Workspace::open_remote` -> `HttpRemote::fetch` -> `Client::graph` -> `Client::send`. README.md:236-253 and docs/cloud/README.md:64-86 explicitly support committed links and environment tokens for agents/CI. credentials.rs:41-46 provides counterevidence for disk-only use: it selects a credential by exact URL. Placeholder-preserving comments at credentials.rs:49-50/client.rs:252-254 do not enforce an origin restriction in this application."
}

## 回帰確認
[
  "With an environment token bound to server A, opening a workspace linked to server B must fail before B receives Authorization.",
  "Ordinary CLI/GUI opening must enforce the same recipient binding as explicit cloud commands.",
  "URL-indexed credentials and correctly bound proxy placeholders should continue to work."
]

## 場所
[
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 51
  },
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 18
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "entrypoint",
    "startLine": 48
  },
  {
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 403
  },
  {
    "endLine": 1224,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  }
]

---

## [F071] Opening a shared workspace can send TOPO_TOKEN to its author's server

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F071
findingId: csf_abc78a49c789fe6d0b41edf1
occurrenceId: occ_c46ef51c51f92cf05f2e80f9
重要度: medium
共通原因: token-origin
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A workspace author can set `.topo/config.toml` `cloud.url` to an attacker-controlled HTTPS endpoint. Ordinary CLI graph commands and GUI startup automatically open that link and transmit the victim's ambient `TOPO_TOKEN` as a bearer credential, allowing the author to use it against the legitimate cloud service.

## 根本原因
{
  "evidenceRefs": [
    "workspace-link-input",
    "auto-cloud-open",
    "ambient-token-unbound",
    "bearer-to-selected-url",
    "file-credential-binding"
  ],
  "summary": "The workspace-controlled `cloud.url` enters `CloudConfig`, then `open()` passes it to `credentials::token()`. That helper returns ambient `TOPO_TOKEN` before consulting its URL-keyed credential store. `Client::send()` consequently attaches that secret to the workspace author's destination without a trusted recipient check."
}

## 修正方針
Bind ambient credentials to a separately trusted server origin (for example a required TOPO_TOKEN_URL) and reject any workspace destination mismatch before adding Authorization; retain per-origin credential lookup and require an explicit trusted action to change the recipient.

## 検証
{
  "counterEvidence": [
    "Stored credentials are keyed by exact URL (credentials.rs:41-46).",
    "The credentials.rs:49-50 comment allows broker-replaced placeholders; an actually destination-enforcing broker can mitigate that deployment, but the implementation also accepts real tokens.",
    "TOPO_CLOUD_URL/--url apply to explicit cloud commands; ordinary automatic graph opening uses workspace CloudConfig (lib.rs:48-53)."
  ],
  "evidenceRefs": [
    "workspace-link-input",
    "auto-cloud-open",
    "ambient-token-unbound",
    "bearer-to-selected-url"
  ],
  "limitations": [
    "No application execution or live network proof was performed.",
    "The victim environment must contain a real usable token; external broker enforcement and deployed token scope are unknown."
  ],
  "method": "independent static source trace",
  "status": "validated",
  "summary": "Verified CLI main.rs:403, GUI main.rs:1215-1219 and cloud helpers all reach the same automatic open/credential path; Workspace::open_remote fetches immediately (store.rs:91-94). The environment-token branch contains no URL binding, and Client sends the header before processing any response."
}

## 回帰確認
[
  "Open an attacker-linked workspace with a token bound to another origin and assert that no authenticated request reaches the attacker endpoint.",
  "Verify legitimate bound environment credentials and URL-keyed stored credentials continue to work."
]

## 場所
[
  {
    "endLine": 55,
    "path": "crates/topo-cloud/src/credentials.rs",
    "role": "root_control",
    "startLine": 51
  },
  {
    "endLine": 32,
    "path": "crates/topo-cloud/src/config.rs",
    "role": "user_input",
    "startLine": 18
  },
  {
    "endLine": 53,
    "path": "crates/topo-cloud/src/lib.rs",
    "role": "propagation",
    "startLine": 48
  },
  {
    "endLine": 72,
    "path": "crates/topo-cloud/src/client.rs",
    "role": "sink",
    "startLine": 55
  },
  {
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 403
  },
  {
    "endLine": 1219,
    "path": "crates/topo-gui/src/main.rs",
    "role": "entrypoint",
    "startLine": 1215
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "entrypoint",
    "startLine": 108
  }
]


## 対応結果（2026-10-03）
結果: fixed。環境トークンを外部TOPO_CLOUD_URLへ束縛し、別送信先・未設定をCLIの複数入口で拒否。通常の保存済み資格情報、proxy/system certificates、placeholderは維持。
最終検証: cargo test --locked --workspace 116成功/既存3ignore、fmt、native/Worker clippy -D warnings、local Worker/D1 smoke成功。詳細: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/artifacts-6a4e5a1e5a63564edc8ca8693b4bf5cc93fdb5baaaf0c17a2de44b08336f148b/artifacts/report_fix_20261003.md
