---
id: prmpp3
kind: task
title: デスクトップエントリへの改行注入を拒否する
status: todo
tags:
- security
- desktop-launcher
- 44dbfab6-0d90-475d-b2a3-4df6534779e5
milestones:
- f0ehc5
---

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
統合した項目: D003, D004, F016
同じ原因の報告と未検証候補を追跡する。報告件数は独立した脆弱性数ではない。

---

## [D003] 確認: Source audit completed for the launcher, but security validation remains unresolved: the repository contains no downstream desktop pars

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D003
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "candidateId": "investigator-desktop-newline",
    "originalEvidence": "assets/branding/install-linux-desktop.sh:24-35 escapes %, backslash, quotes, backticks and $, but not LF/CR before writing a line-oriented .desktop Exec value. realpath and directory/executable checks (8-14) do not reject newline-bearing paths. This could produce additional desktop-file lines; downstream duplicate-key/quote behavior is unavailable, and operator-supplied paths already carry substantial authority, so no privilege gain is assumed.",
    "originalFinalCandidate": {
      "conclusion": "The launcher escapes percent signs, backslashes, quotes, backticks, and dollar signs, but preserves internal LF and CR characters when embedding canonicalized paths in the line-oriented desktop file. The path existence and executable checks do not reject those characters.",
      "downstreamControl": "The optional update-desktop-database invocation has no implementation in this repository. No desktop launcher parser or consumer source was available to establish malformed-line, duplicate-key, or Exec interpretation.",
      "evidence": [
        "assets/branding/install-linux-desktop.sh:8-14",
        "assets/branding/install-linux-desktop.sh:24-37",
        "assets/branding/install-linux-desktop.sh:39-40",
        "assets/branding/README.md:20-27"
      ],
      "id": "linux-launcher-newline-key-injection",
      "missingImpactPrerequisite": "Security impact requires a concrete downstream consumer accepting useful injected entries and a lower-trust source for the installed paths. The packet treats these paths as explicit operator inputs. No privilege gain or command execution finding is asserted.",
      "status": "unresolved"
    },
    "source": "architecture-followup",
    "validation": {
      "counterEvidence": [
        "Installer arguments are explicit operator inputs; the operator already selects the executable to launch (install-linux-desktop.sh:8-14; assets/branding/README.md:20-27).",
        "No cloud graph, model response, or HTTP input flows to these installed path arguments.",
        "The downstream desktop consumer is absent from selected source; update-desktop-database is only an optional external invocation (install-linux-desktop.sh:39-40)."
      ],
      "method": "static_source_review",
      "status": "unresolved",
      "summary": "Canonicalization, existence checks, and quote escaping preserve internal LF/CR before .desktop serialization. A malicious desktop-entry interpretation and a meaningful attacker-to-operator authority boundary require evidence outside the authorized offline source scope."
    }
  },
  "id": "investigator-desktop-newline",
  "paths": [
    "assets/branding/install-linux-desktop.sh"
  ],
  "reason": "Source audit completed for the launcher, but security validation remains unresolved: the repository contains no downstream desktop parser and establishes no lower-trust source for the explicit operator-selected executable/workspace arguments. Preserve the LF/CR evidence without asserting command execution or privilege gain."
}
```

---

## [D004] 確認: Newlines in workspace paths inject desktop-entry fields

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D004
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "originalEvidence": "assets/branding/install-linux-desktop.sh:24-29 quote() escapes %, backslash, quotes, backticks, $, but leaves LF/CR. It embeds attacker-influenced workspace/binary paths into .desktop Exec line at 32-36. A workspace pathname containing LF can inject a second Exec= line and comment out the remainder; generated desktop launcher can run that inserted command when launched. Requires user registering and launching a specially named workspace. Classify conservatively.",
    "source": "baseline",
    "title": "Newlines in workspace paths inject desktop-entry fields"
  },
  "candidateId": "desktop-newline-001",
  "id": "desktop-newline-001",
  "paths": [
    "assets/branding/install-linux-desktop.sh"
  ],
  "reason": "Baseline confirms newline/CR can inject desktop-entry lines, but downstream duplicate-key behavior and a meaningful launcher trust boundary were not validated. Candidate remains unresolved with original evidence; no command-execution finding is asserted."
}
```

---

## [F016] Workspace path newlines can inject executable desktop launcher directives

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F016
findingId: csf_15492d025a5aca54b42e8bf4
occurrenceId: occ_9771e788ca370d65f11fbeea
重要度: low
共通原因: desktop-launcher
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

The Linux desktop installer writes caller-selected workspace paths into a line-oriented Desktop Entry using an escaping helper that preserves literal newlines. A supplied initialized workspace whose path contains an injected Exec directive can replace the launcher command on desktops that accept the last duplicate key.

## 根本原因
{
  "evidenceRefs": [
    "desktop-workspace-path",
    "desktop-path-encoding",
    "desktop-file-write"
  ],
  "summary": "quote() encodes the Exec argument layer while leaving LF/CR active in the surrounding line-oriented Desktop Entry. realpath and directory validation do not remove those delimiters. The resulting user launcher contains attacker-supplied key directives."
}

## 修正方針
Reject LF and CR in workspace and executable paths before writing any desktop file, or use a serializer that correctly encodes the Desktop Entry string layer and the Exec argument layer. Validate the resulting launcher before installation.

## 検証
{
  "counterEvidence": [
    "Binary and workspace existence checks are present; they do not constrain the desktop directives created by internal newlines.",
    "Ordinary paths and shell metacharacters handled by the helper do not take this path.",
    "The victim explicitly runs the installer and later invokes the registered launcher.",
    "The path must be attacker-influenced independently of trusted installer and GUI code."
  ],
  "evidenceRefs": [
    "desktop-workspace-path",
    "desktop-path-encoding",
    "desktop-file-write"
  ],
  "limitations": [
    "No desktop launcher or application was executed.",
    "Duplicate-key acceptance is a desktop implementation prerequisite; parsers that reject duplicate keys may fail closed instead.",
    "The investigator's local GLib documentation observation is retained as original evidence, rather than asserted as a verified deployment dependency."
  ],
  "method": "static source trace",
  "status": "validated",
  "summary": "An initialized directory named with internal LF followed by Exec=sh -c id and a final LF/X-Ignore=tail passes the workspace checks. Its quoted path is written as multiple physical key-file lines. A desktop parser honoring the later duplicate Exec uses the injected command instead of the checked topo-gui binary."
}

## 回帰確認
[
  "Newline-bearing workspace/executable paths are rejected or remain one inert Exec argument.",
  "Normal paths containing spaces, quotes, backslashes and percent characters retain their intended arguments."
]

## 場所
[
  {
    "endLine": 14,
    "path": "assets/branding/install-linux-desktop.sh",
    "role": "entrypoint",
    "startLine": 8
  },
  {
    "endLine": 29,
    "path": "assets/branding/install-linux-desktop.sh",
    "role": "root_control",
    "startLine": 24
  },
  {
    "endLine": 36,
    "path": "assets/branding/install-linux-desktop.sh",
    "role": "sink",
    "startLine": 31
  },
  {
    "endLine": 33,
    "path": "assets/branding/README.md",
    "role": "supporting",
    "startLine": 23
  }
]


## 今回の区切り（2026-10-03）
未完了（todo）。Linux desktop launcher/インストール経路を実環境で確認する必要がある。今回はmacOSのコード・テストとWorkerを検証。未修正として保持。
詳細: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/artifacts-6a4e5a1e5a63564edc8ca8693b4bf5cc93fdb5baaaf0c17a2de44b08336f148b/artifacts/report_fix_20261003.md
