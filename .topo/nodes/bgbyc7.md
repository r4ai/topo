---
id: bgbyc7
kind: task
title: 共有テキスト由来の端末制御文字を無害化する
status: done
tags:
- security
- terminal-output
- 44dbfab6-0d90-475d-b2a3-4df6534779e5
milestones:
- f0ehc5
---

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
統合した項目: D023, D029, D033, D035, D039, F006, F017, F024, F028, F032, F039, F046, F050, F056, F060, F069
同じ原因の報告と未検証候補を追跡する。報告件数は独立した脆弱性数ではない。

---

## [D023] 確認: Node titles and notes may include terminal control sequences

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D023
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "baselineOriginalEvidence": "CLI render::node_line/detail_text raw-concatenate titles/tags/notes and main::print writes bytes to terminal (render.rs:84-88,117-118; main.rs:277-282): cloud editor can inject CSI/OSC sequences.",
    "candidateId": "cli-raw-terminal-controls",
    "originalEvidence": "Investigator preliminary: plain CLI renders workspace titles/notes with raw terminal escapes.",
    "paths": [
      "crates/topo-cli/src/render.rs",
      "crates/topo-cli/src/main.rs"
    ],
    "reason": "Establish concrete terminal-sensitive operation and realistic impact; raw output alone is not validated security impact.",
    "title": "Node titles and notes may include terminal control sequences"
  },
  "candidateId": "cli-raw-terminal-controls",
  "id": "cli-raw-terminal-controls",
  "paths": [
    "crates/topo-cli/src/render.rs",
    "crates/topo-cli/src/main.rs"
  ],
  "reason": "Establish concrete terminal-sensitive operation and realistic impact; raw output alone is not validated security impact."
}
```

---

## [D029] 確認: Node titles and notes may include terminal control sequences

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D029
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "baselineFinalFinding": {
      "attacker": "A cloud workspace writer or repository author who controls node titles, tags, or Markdown notes viewed by another user's CLI.",
      "confidence": "high",
      "counterevidence": [
        "JSON and JSONL output serialize strings with escaping, preventing raw control sequences in those modes.",
        "Clipboard effects depend on terminal support and terminal security settings.",
        "No automatic command execution is established by this source evidence."
      ],
      "cwe": "CWE-150",
      "impact": "Attacker content can clear or overwrite displayed output and spoof preceding status or warnings. On terminals supporting OSC clipboard operations, the content can also replace the user's clipboard.",
      "locations": [
        {
          "file": "crates/topo-cli/src/render.rs",
          "line_end": 88,
          "line_start": 84
        },
        {
          "file": "crates/topo-cli/src/render.rs",
          "line_end": 118,
          "line_start": 100
        },
        {
          "file": "crates/topo-cli/src/main.rs",
          "line_end": 282,
          "line_start": 277
        },
        {
          "file": "crates/topo-cli/src/main.rs",
          "line_end": 469,
          "line_start": 463
        }
      ],
      "recommended_remediation": "Escape terminal control characters in all human-readable output derived from workspace or server content, including error messages and metadata. Preserve ordinary printable Unicode and deliberate formatting newlines while rendering ESC, C0, and applicable C1 controls visibly.",
      "severity": "low",
      "source_to_sink": "Node content accepts arbitrary strings through Add/Edit and cloud imports. render::node_line directly interpolates titles and tags; detail_text appends Markdown notes verbatim. CLI commands such as ls and show pass these strings to main::print, which writes them directly to stdout without escaping terminal control bytes.",
      "supporting_source_evidence": [
        "node_line interpolates node.title and each tag without filtering control characters.",
        "detail_text appends node.body.trim_end(), which leaves embedded ESC and other control bytes unchanged.",
        "print writes the assembled output with writeln!(stdout, \"{output}\").",
        "Core operation and wire types impose no control-character restrictions on these strings."
      ],
      "title": "Shared node content is emitted as executable terminal control sequences",
      "violated_security_invariant": "Untrusted task content must be displayed as text rather than interpreted as terminal controls."
    },
    "baselineOriginalEvidence": "CLI render::node_line/detail_text raw-concatenate titles/tags/notes and main::print writes bytes to terminal (render.rs:84-88,117-118; main.rs:277-282): cloud editor can inject CSI/OSC sequences.",
    "candidateId": "cli-raw-terminal-controls",
    "originalEvidence": "Investigator preliminary: plain CLI renders workspace titles/notes with raw terminal escapes.",
    "paths": [
      "crates/topo-cli/src/render.rs",
      "crates/topo-cli/src/main.rs"
    ],
    "reason": "Establish concrete terminal-sensitive operation and realistic impact; raw output alone is not validated security impact.",
    "title": "Node titles and notes may include terminal control sequences"
  },
  "candidateId": "cli-raw-terminal-controls",
  "id": "cli-raw-terminal-controls-c6e2942efee1ca6e",
  "paths": [
    "crates/topo-cli/src/render.rs",
    "crates/topo-cli/src/main.rs"
  ],
  "reason": "Establish concrete terminal-sensitive operation and realistic impact; raw output alone is not validated security impact."
}
```

---

## [D033] 確認: Node titles and notes may include terminal control sequences

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D033
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "candidateId": "cli-raw-terminal-controls",
    "originalEvidence": "Investigator preliminary: plain CLI renders workspace titles/notes with raw terminal escapes.",
    "paths": [
      "crates/topo-cli/src/render.rs",
      "crates/topo-cli/src/main.rs"
    ],
    "reason": "Establish concrete terminal-sensitive operation and realistic impact; raw output alone is not validated security impact.",
    "title": "Node titles and notes may include terminal control sequences"
  },
  "candidateId": "cli-raw-terminal-controls",
  "id": "cli-raw-terminal-controls-0beb2da22444aded",
  "paths": [
    "crates/topo-cli/src/render.rs",
    "crates/topo-cli/src/main.rs"
  ],
  "reason": "Establish concrete terminal-sensitive operation and realistic impact; raw output alone is not validated security impact."
}
```

---

## [D035] 確認: Cloud node text is emitted as raw terminal control sequences

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D035
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "investigatorEvidence": "Unbounded String title/tags/notes accepted by core ops (ops.rs:124-132,154-156) and WireNode; install validates only node ID + graph structure (store.rs:116-118). render::node_line raw title/tags (84-88); show raw body (117-118); main::print unchanged bytes (277-281). --json/JSONL encode ESC; TSV escapes tab/newline/CR/backslash but leaves ESC. Conservative impact display spoofing and conditional OSC 52 clipboard replacement. No unconditional RCE. TUI cell rendering is a distinct sink.",
    "originalEvidence": "Raw terminal escape sequences from cloud node title/body reach CLI render.rs 84-88/117-118 and main.rs 277-281; focused investigator can assess terminal injection.",
    "source": "baseline",
    "title": "Cloud node text is emitted as raw terminal control sequences"
  },
  "candidateId": "candidate-baseline-terminal-control-sequences",
  "id": "candidate-baseline-terminal-control-sequences",
  "paths": [
    "crates/topo-cli/src/render.rs",
    "crates/topo-cli/src/main.rs"
  ],
  "reason": "Awaiting investigator and parent analysis of attacker-controlled graph text and meaningful terminal capability gain."
}
```

---

## [D039] 確認: Cloud node text is emitted as raw terminal control sequences

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
保留番号: D039
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/coverage.json
未検証。重複、環境依存、攻撃前提を確認してから対応する。

```json
{
  "candidate": {
    "originalEvidence": "Raw terminal escape sequences from cloud node title/body reach CLI render.rs 84-88/117-118 and main.rs 277-281; focused investigator can assess terminal injection.",
    "source": "baseline",
    "title": "Cloud node text is emitted as raw terminal control sequences"
  },
  "candidateId": "candidate-baseline-terminal-control-sequences",
  "id": "candidate-baseline-terminal-control-sequences-6be0bfd157f5dcbe",
  "paths": [
    "crates/topo-cli/src/render.rs",
    "crates/topo-cli/src/main.rs"
  ],
  "reason": "Awaiting investigator and parent analysis of attacker-controlled graph text and meaningful terminal capability gain."
}
```

---

## [F006] Displaying shared text can inject terminal controls through CLI output

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F006
findingId: csf_376efa7cdce76cc9a1e792e1
occurrenceId: occ_199537afc94887164371598a
重要度: low
共通原因: terminal-output
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

Shared task titles/tags/notes, local display identifiers, owner-chosen workspace names and recorded writer token/agent labels reach human-readable CLI stdout without terminal-control encoding. A victim terminal can interpret the bytes as display instructions and, when enabled, clipboard operations; JSON/JSONL and native GUI text counterevidence remain explicit, and code execution is not established.

## 根本原因
{
  "evidenceRefs": [
    "untrusted-text",
    "text-render",
    "body-render",
    "default-commands",
    "stdout-sink",
    "tsv-gap",
    "cloud-log-text",
    "terminal-model",
    "terminal-notes",
    "terminal-stdout",
    "dedup-0003-3-arbitrary-task-strings",
    "dedup-0003-3-default-human-output",
    "dedup-0003-3-plain-title-tags",
    "dedup-0003-3-plain-notes",
    "dedup-0003-3-cloud-name-validation",
    "dedup-0003-3-owner-controlled-workspace-name",
    "dedup-0003-3-cloud-list-terminal-name",
    "dedup-0003-3-unrestricted-token-name-input",
    "dedup-0003-3-audit-label-attribution",
    "dedup-0003-3-text-to-stdout",
    "dedup-0004-6-add-edges",
    "dedup-0004-6-show-text-entry",
    "dedup-0004-6-text-format-variants",
    "dedup-0004-6-stdout-no-escape"
  ],
  "summary": "Add/Edit retain arbitrary strings. Node/detail/tree/proposal renderers interpolate shared titles, tags and notes, and cloud listing/log renderers interpolate names and recorded token/agent labels. The CLI print function writes the human-readable string verbatim. TSV escapes only backslash, tab, newline and carriage return, leaving ESC untouched; trailing-whitespace trimming of notes likewise leaves embedded OSC/CSI. JSON and JSONL serializer paths escape controls. The missing terminal-safe encoding at the shared text-to-stdout boundary affects the reported views. The incoming source also includes local filename-compatible identifiers among the strings reaching human-readable output and its remediation tests; identifier validation at explicit add/import boundaries must not be generalized to every local file/library path. The additional cloud-name trace shows shared workspace-name validation only trims/checks length, owner-authorized rename persists the remaining control-bearing string, and cloud ls prints it raw. Unrestricted token creation accepts similarly valid-length labels; writer operations record the authenticated token name, and cloud log JSON-escapes operation contents while directly interpolating the surrounding label."
}

## 修正方針
Apply a common terminal-safe encoder at every human-readable output boundary for externally supplied task content, local identifiers, proposals, tree text, cloud names/token/agent labels and error text. Render ESC/OSC/CSI/C0 controls visibly while preserving intended layout and note newlines, stored content, and structured JSON/JSONL serialization.

## 検証
{
  "assertions": [
    "Node text is accepted without ESC filtering.",
    "Human-readable views preserve control bytes.",
    "JSON/JSONL encoding escapes those bytes instead."
  ],
  "counterEvidence": [
    "JSON and JSONL outputs serialize control characters.",
    "The native GUI displays text rather than interpreting terminal sequences.",
    "Effects beyond display manipulation depend on the terminal's enabled features.",
    "JSON/JSONL serialize controls safely; GPUI text rendering is not a terminal command sink.",
    "OSC52 effects require a permitting terminal; no ratatui escape handling flaw or general RCE is asserted.",
    "JSON/JSONL serialization escapes control characters.",
    "Impact depends on emulator features; OSC 52 can be disabled.",
    "JSON and JSONL serialize controls as escaped data.",
    "GPUI uses native text rendering, so this finding does not establish a GUI terminal sink.",
    "Clipboard alteration needs a supported/enabled terminal escape sequence; no automatic shell execution is proven.",
    "JSON/JSONL serialization escapes controls; validated IDs used in ID-only output are safe.",
    "GUI content is displayed as text and does not use this terminal sink.",
    "TUI uses widget rendering; this finding does not claim that widgets interpret all raw control sequences.",
    "If output is redirected to a nonterminal data consumer, the demonstrated terminal effect is absent."
  ],
  "evidenceRefs": [
    "untrusted-text",
    "text-render",
    "body-render",
    "default-commands",
    "stdout-sink",
    "tsv-gap",
    "cloud-log-text",
    "terminal-model",
    "terminal-notes",
    "terminal-stdout",
    "dedup-0003-3-arbitrary-task-strings",
    "dedup-0003-3-plain-title-tags",
    "dedup-0003-3-plain-notes",
    "dedup-0003-3-text-to-stdout",
    "dedup-0003-3-default-human-output",
    "dedup-0003-3-cloud-name-validation",
    "dedup-0003-3-owner-controlled-workspace-name",
    "dedup-0003-3-cloud-list-terminal-name",
    "dedup-0003-3-unrestricted-token-name-input",
    "dedup-0003-3-audit-label-attribution",
    "dedup-0004-6-add-edges",
    "dedup-0004-6-stdout-no-escape",
    "dedup-0004-6-show-text-entry",
    "dedup-0004-6-text-format-variants"
  ],
  "limitations": [
    "No arbitrary command or code execution is claimed.",
    "No active terminal-control input was created or displayed.",
    "No terminal sequence was emitted or runtime terminal behavior tested.",
    "Impact is limited to output interpretation; protected token/file access is not established by this issue alone.",
    "No live terminal payload was executed.",
    "No command execution or terminal-specific file access is claimed.",
    "Terminal capability and victim output mode vary; the verified mechanism is unneutralized bytes, not an exercised terminal payload.",
    "No terminal control sequences were executed.",
    "No shell execution, clipboard behavior or terminal-specific exploit is claimed."
  ],
  "method": "semantic reduction of already-validated Standard source findings; no additional validation",
  "status": "validated",
  "summary": "Shared node strings flow unchanged through default ls/ready/show/milestone and related text views into print; TSV does not neutralize ESC. Cloud logs directly interpolate recorded token names and optional agent labels, while operation JSON within the same line is serialized. An editor can choose a token name with an unscoped account token and then write to shared history. GUI text rendering is not a terminal interpreter; OSC52/clipboard effects depend on terminal settings. A prior source's framing also includes local filename-compatible identifiers and all text/proposal views; that source's attack-path likelihood is medium rather than the retained aggregate's high, depending on terminal features, and remains recorded in source assessments. The prior discovery-0002 source separately preserves owner-authorized workspace rename/name display and authenticated token-label storage before another member reads history. A user's private token listing alone is self-only; the shared-history label establishes the cross-member boundary. The assigned source limits its impact framing to screen/cursor/display spoofing, does not assert clipboard or shell effects, and does not establish TUI widget interpretation; prior conditional clipboard assessments remain preserved."
}

## 回帰確認
[
  "Text/TSV/detail/cloud-log views must not emit raw ESC controls from shared fields.",
  "JSON and JSONL must retain their existing valid escaped representation.",
  "Render title/tags/notes containing ESC/OSC/CSI and verify stdout contains visible escaped data rather than executable terminal sequences.",
  "Keep newline formatting and JSON semantics intact.",
  "Verify titles, tags, notes, local IDs, and proposal titles containing ESC/OSC/C0 controls cannot emit executable terminal sequences in any text view.",
  "Verify ordinary Unicode text and deliberate output layout remain readable.",
  "Print tasks containing ESC/OSC/CSI and other control bytes and assert text/TSV emit safe visible representations.",
  "Confirm ordinary Unicode titles and multiline notes remain readable and JSON/JSONL preserve the original data.",
  "ESC-bearing titles/tags/notes appear as inert text in default, TSV and plain-text views.",
  "Notes preserve intentional newlines while terminal-control bytes cannot reach stdout as instructions."
]

## 場所
[
  {
    "endLine": 88,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 84
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 117
  },
  {
    "endLine": 293,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 277
  },
  {
    "endLine": 478,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 463
  },
  {
    "endLine": 72,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 71
  },
  {
    "endLine": 181,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 167
  },
  {
    "endLine": 258,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 248
  },
  {
    "endLine": 224,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "sink",
    "startLine": 218
  },
  {
    "endLine": 251,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "sink",
    "startLine": 241
  },
  {
    "endLine": 119,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 117
  },
  {
    "endLine": 73,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 71
  },
  {
    "endLine": 281,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 277
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 124
  },
  {
    "endLine": 360,
    "path": "crates/topo-core/src/graph.rs",
    "role": "user_input",
    "startLine": 347
  },
  {
    "endLine": 102,
    "path": "crates/topo-core/src/model.rs",
    "role": "user_input",
    "startLine": 82
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 113
  },
  {
    "endLine": 286,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 278
  },
  {
    "endLine": 119,
    "path": "crates/topo-cli/src/render.rs",
    "role": "entrypoint",
    "startLine": 100
  },
  {
    "endLine": 281,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 277
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "propagation",
    "startLine": 124
  },
  {
    "endLine": 89,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 84
  },
  {
    "endLine": 120,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 117
  },
  {
    "endLine": 287,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 277
  },
  {
    "endLine": 35,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 27
  },
  {
    "endLine": 53,
    "path": "crates/topo-server/src/routes/workspaces.rs",
    "role": "propagation",
    "startLine": 44
  },
  {
    "endLine": 224,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "root_control",
    "startLine": 216
  },
  {
    "endLine": 49,
    "path": "crates/topo-server/src/routes/tokens.rs",
    "role": "propagation",
    "startLine": 37
  },
  {
    "endLine": 251,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "root_control",
    "startLine": 241
  },
  {
    "endLine": 74,
    "path": "crates/topo-server/src/write.rs",
    "role": "propagation",
    "startLine": 64
  },
  {
    "endLine": 134,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 124
  },
  {
    "endLine": 120,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 113
  },
  {
    "endLine": 290,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 280
  },
  {
    "endLine": 469,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 467
  },
  {
    "endLine": 72,
    "path": "crates/topo-cli/src/render.rs",
    "role": "entrypoint",
    "startLine": 42
  }
]

---

## [F017] Viewing shared tasks can execute terminal control sequences

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F017
findingId: csf_a113e589a57a69a069edb111
occurrenceId: occ_e9ba05300723617b59ad8312
重要度: low
共通原因: terminal-output
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A workspace editor or repository author can place escape sequences in task titles, tags, or notes. Default CLI text views print those fields unchanged, allowing terminal display manipulation and, on supporting terminals, clipboard replacement.

## 根本原因
{
  "evidenceRefs": [
    "terminal-model",
    "terminal-node",
    "terminal-notes",
    "terminal-stdout"
  ],
  "summary": "`Node` accepts arbitrary display strings. `node_line()` and `detail_text()` preserve embedded terminal controls when composing human-readable output, and `print()` writes that output without encoding the terminal grammar."
}

## 修正方針
Encode or strip terminal control characters from every untrusted field in human-readable output, retaining intended formatting outside those fields and JSON serialization for machine output.

## 検証
{
  "counterEvidence": [
    "JSON/JSONL serialization escapes control characters.",
    "Impact depends on emulator features; OSC 52 can be disabled."
  ],
  "evidenceRefs": [
    "terminal-model",
    "terminal-node",
    "terminal-notes",
    "terminal-stdout"
  ],
  "limitations": [
    "No live terminal payload was executed.",
    "No command execution or terminal-specific file access is claimed."
  ],
  "method": "static source trace",
  "summary": "The attacker can store the control bytes through a shared graph or supplied workspace, and normal text output forwards them to the viewer's terminal. Existing JSON serialization escapes those bytes but is an optional output mode."
}

## 回帰確認
[
  "Verify titles, tags, notes, local IDs, and proposal titles containing ESC/OSC/C0 controls cannot emit executable terminal sequences in any text view.",
  "Verify ordinary Unicode text and deliberate output layout remain readable."
]

## 場所
[
  {
    "endLine": 102,
    "path": "crates/topo-core/src/model.rs",
    "role": "user_input",
    "startLine": 82
  },
  {
    "endLine": 88,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 84
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 113
  },
  {
    "endLine": 286,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 278
  },
  {
    "endLine": 119,
    "path": "crates/topo-cli/src/render.rs",
    "role": "entrypoint",
    "startLine": 100
  },
  {
    "endLine": 281,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 277
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "propagation",
    "startLine": 124
  }
]

---

## [F024] Reading shared task text, workspace names or audit labels can alter terminal output

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F024
findingId: csf_f54b0250fdfd6bc027b1cf39
occurrenceId: occ_8a24d5c2a54bb49957d087c3
重要度: low
共通原因: terminal-output
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

Shared task fields,owner-chosen workspace names and writer token labels reach normal CLI output without terminal-control neutralization. Reading lists,task details or cloud history can manipulate the victim's display and conditionally the clipboard.

## 根本原因
{
  "evidenceRefs": [
    "arbitrary-task-strings",
    "plain-title-tags",
    "plain-notes",
    "text-to-stdout",
    "default-human-output",
    "cloud-name-validation",
    "owner-controlled-workspace-name",
    "cloud-list-terminal-name",
    "unrestricted-token-name-input",
    "audit-label-attribution",
    "audit-terminal-label",
    "tsv-leaves-controls"
  ],
  "summary": "Typed graph operations preserve arbitrary Unicode strings in title, tags, and notes. Human list rendering directly interpolates title/tags, while detail rendering appends the note body. `print()` writes the resulting string to stdout unchanged. JSON escaping protects JSON/JSONL output, but decoding JSON before the human renderer restores control bytes; TSV only escapes delimiters and line endings and still preserves ESC. The shared cloud name validator only trims/checks length. Cloud workspace listing and audit history also interpolate names or writer token labels into the same raw print path; audit operations are JSON-escaped but the surrounding label is not."
}

## 修正方針
Render terminal control characters visibly in human and TSV output while preserving ordinary Unicode and intended line breaks. Apply the same terminal-safe rendering to cloud names, labels and remote error text; keep JSON modes serialized normally.

## 検証
{
  "counterEvidence": [
    "JSON and JSONL serialize controls as escaped data.",
    "GPUI uses native text rendering, so this finding does not establish a GUI terminal sink.",
    "Clipboard alteration needs a supported/enabled terminal escape sequence; no automatic shell execution is proven."
  ],
  "evidenceRefs": [
    "arbitrary-task-strings",
    "plain-title-tags",
    "plain-notes",
    "text-to-stdout",
    "default-human-output",
    "tsv-leaves-controls",
    "cloud-name-validation",
    "owner-controlled-workspace-name",
    "cloud-list-terminal-name",
    "unrestricted-token-name-input",
    "audit-terminal-label",
    "audit-label-attribution"
  ],
  "limitations": [
    "Terminal capability and victim output mode vary; the verified mechanism is unneutralized bytes, not an exercised terminal payload."
  ],
  "method": "Independent static source trace",
  "summary": "The parent followed arbitrary graph text through `node_line()` and `detail_text()` to the direct stdout write, and confirmed default list/show select these paths. No upstream text control filter is applied. Shared cloud membership gives a collaborator a concrete way to place text on another user's terminal. Sibling human sinks were checked: owner-controlled workspace names and accepted writer token labels likewise preserve valid-length ESC-bearing strings through storage and listing/history. A token's own private list alone is self-only;the reportable label boundary is another member reading shared audit history."
}

## 回帰確認
[
  "Print tasks containing ESC/OSC/CSI and other control bytes and assert text/TSV emit safe visible representations.",
  "Confirm ordinary Unicode titles and multiline notes remain readable and JSON/JSONL preserve the original data."
]

## 場所
[
  {
    "endLine": 89,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 84
  },
  {
    "endLine": 120,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 117
  },
  {
    "endLine": 73,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 71
  },
  {
    "endLine": 287,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 277
  },
  {
    "endLine": 35,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 27
  },
  {
    "endLine": 73,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 71
  },
  {
    "endLine": 53,
    "path": "crates/topo-server/src/routes/workspaces.rs",
    "role": "propagation",
    "startLine": 44
  },
  {
    "endLine": 224,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "root_control",
    "startLine": 216
  },
  {
    "endLine": 49,
    "path": "crates/topo-server/src/routes/tokens.rs",
    "role": "propagation",
    "startLine": 37
  },
  {
    "endLine": 251,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "root_control",
    "startLine": 241
  },
  {
    "endLine": 74,
    "path": "crates/topo-server/src/write.rs",
    "role": "propagation",
    "startLine": 64
  }
]

---

## [F028] Displaying shared task text can inject terminal control sequences

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F028
findingId: csf_7065de4514aadcf47e9f48b0
occurrenceId: occ_ac8b5fe7cc4797c1fd903dd7
重要度: low
共通原因: terminal-output
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A workspace writer can place terminal control sequences in task titles, tags or notes. Human-readable CLI output emits those strings unchanged, allowing display spoofing and, where the terminal permits it, clipboard replacement.

## 根本原因
{
  "evidenceRefs": [
    "raw-title-tags",
    "raw-notes",
    "text-to-terminal"
  ],
  "summary": "`Op::Add` and `Graph::edit()` store arbitrary text. `node_line()` and `detail_text()` combine title/tag/note content without escaping terminal controls, and `print()` writes that string directly to stdout. TSV only escapes tabs/newlines/carriage returns/backslashes and still leaves ESC untouched, whereas JSON serialization safely encodes controls."
}

## 修正方針
Escape or visibly substitute terminal control characters at every human-readable output boundary, including task content, proposals, cloud labels and error text. Preserve stored content and machine-readable JSON.

## 検証
{
  "counterEvidence": [
    "JSON/JSONL serialize controls safely; GPUI text rendering is not a terminal command sink.",
    "OSC52 effects require a permitting terminal; no ratatui escape handling flaw or general RCE is asserted."
  ],
  "limitations": [
    "No terminal sequence was emitted or runtime terminal behavior tested.",
    "Impact is limited to output interpretation; protected token/file access is not established by this issue alone."
  ],
  "method": "static_source_review",
  "status": "validated",
  "summary": "Shared graph data becomes Node strings unchanged. CLI ls/show and related text renderers feed raw node_line/detail_text results to print; no terminal-control escaping intervenes. ESC display sequences can therefore affect another user's terminal rather than remain visible task data."
}

## 回帰確認
[
  "Render title/tags/notes containing ESC/OSC/CSI and verify stdout contains visible escaped data rather than executable terminal sequences.",
  "Keep newline formatting and JSON semantics intact."
]

## 場所
[
  {
    "endLine": 88,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 84
  },
  {
    "endLine": 119,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 117
  },
  {
    "endLine": 73,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 71
  },
  {
    "endLine": 281,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 277
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 124
  },
  {
    "endLine": 360,
    "path": "crates/topo-core/src/graph.rs",
    "role": "user_input",
    "startLine": 347
  }
]

---

## [F032] Viewing shared tasks can execute terminal control sequences

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F032
findingId: csf_fd35b1d81fa2cee6703434f1
occurrenceId: occ_c444fe179a2e97dde1f5c937
重要度: low
共通原因: terminal-output
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

A collaborator can place control sequences in task titles, tags or notes. Ordinary CLI text views write them directly to the terminal, allowing display spoofing and, where enabled, clipboard manipulation. Shared token names in human-readable cloud logs use the same unchecked output path.

## 根本原因
{
  "evidenceRefs": [
    "untrusted-text",
    "default-commands",
    "text-render",
    "body-render",
    "tsv-gap",
    "cloud-log-text",
    "stdout-sink"
  ],
  "summary": "Shared task fields are plain Rust strings with no terminal-safe constraint. Human-readable node/detail/proposal/tree rendering interpolates those strings directly, and the CLI's `print()` writes the result verbatim. TSV escapes record delimiters but not ESC controls. Cloud listing/log formatting likewise inserts names directly, so the terminal interprets content as control instructions instead of text."
}

## 修正方針
Apply one terminal-safe encoder to all untrusted human-readable output fields, including task text, shared names and error text. Render control characters visibly while preserving intended notes newlines, and leave structured JSON serialization unchanged.

## 検証
{
  "assertions": [
    "Node text is accepted without ESC filtering.",
    "Human-readable views preserve control bytes.",
    "JSON/JSONL encoding escapes those bytes instead."
  ],
  "counterEvidence": [
    "JSON and JSONL outputs serialize control characters.",
    "The native GUI displays text rather than interpreting terminal sequences.",
    "Effects beyond display manipulation depend on the terminal's enabled features."
  ],
  "evidenceRefs": [
    "untrusted-text",
    "text-render",
    "body-render",
    "default-commands",
    "stdout-sink",
    "tsv-gap",
    "cloud-log-text"
  ],
  "limitations": [
    "No arbitrary command or code execution is claimed.",
    "No active terminal-control input was created or displayed."
  ],
  "method": "independent static source trace",
  "summary": "Parent traced shared node text through default ls/ready/show to raw stdout and verified that no terminal-control encoder is applied. Separate cloud-log formatting also prints the recorded token name directly, which an editor can choose when using an unscoped account token and then writing to the shared workspace."
}

## 回帰確認
[
  "Text/TSV/detail/cloud-log views must not emit raw ESC controls from shared fields.",
  "JSON and JSONL must retain their existing valid escaped representation."
]

## 場所
[
  {
    "endLine": 88,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 84
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 117
  },
  {
    "endLine": 293,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 277
  },
  {
    "endLine": 478,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 463
  },
  {
    "endLine": 72,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 71
  },
  {
    "endLine": 181,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 167
  },
  {
    "endLine": 258,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 248
  },
  {
    "endLine": 224,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "sink",
    "startLine": 218
  },
  {
    "endLine": 251,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "sink",
    "startLine": 241
  }
]

---

## [F039] Collaborative node text can inject terminal control sequences

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F039
findingId: csf_24e81d6fa06ef611783ddcdc
occurrenceId: occ_7dbf5f923c9aee549a035f24
重要度: low
共通原因: terminal-output
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

Titles, tags, and notes from node data reach default CLI text and TSV output with terminal control bytes intact. A shared-workspace editor can influence another user's terminal display and, where enabled, terminal clipboard operations.

## 根本原因
{
  "evidenceRefs": [
    "terminal-node-input",
    "terminal-title-tags",
    "terminal-notes",
    "terminal-tsv",
    "terminal-output",
    "terminal-default"
  ],
  "summary": "Raw node content is valid stored data, but human-readable output lacks a control-character escape boundary. node_line interpolates title/tags, detail_text appends body, and print writes the complete string directly to stdout. TSV escapes field separators but leaves ESC intact."
}

## 修正方針
Escape unsafe control characters at human-readable CLI output boundaries, retaining intentional formatting added by the application. Preserve original stored text and keep structured serialization available.

## 検証
{
  "counterEvidence": [
    "IDs are validated and need no similar attacker-controlled control bytes.",
    "JSON and JSONL serialize control characters safely.",
    "Clipboard replacement requires a terminal with the relevant control protocol enabled.",
    "No effect on GPUI/Ratatui is asserted; no terminal-assisted code execution is claimed."
  ],
  "evidenceRefs": [
    "terminal-node-input",
    "terminal-title-tags",
    "terminal-notes",
    "terminal-tsv",
    "terminal-output",
    "terminal-default"
  ],
  "limitations": [
    "No terminal/control sequence was exercised; terminal-specific policies were not inspected."
  ],
  "method": "independent_static_source_trace",
  "status": "validated",
  "summary": "Traced Add/Edit-controlled text to default ls/show rendering and direct stdout writes. Escaped JSON control characters are decoded as literal string characters before these renderers. A terminal interpreting the output can treat them as control commands rather than visible node content."
}

## 回帰確認
[
  "Ensure ESC and other untrusted terminal controls render as visible escaped text in list/detail/TSV outputs.",
  "Preserve readable Japanese text and intended application formatting."
]

## 場所
[
  {
    "endLine": 156,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 124
  },
  {
    "endLine": 88,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 84
  },
  {
    "endLine": 120,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 109
  },
  {
    "endLine": 73,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 71
  },
  {
    "endLine": 283,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 277
  },
  {
    "endLine": 469,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 463
  }
]

---

## [F046] Displaying shared text can inject terminal controls through CLI output

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F046
findingId: csf_8231e10cc01b8bb5fd531444
occurrenceId: occ_6cd4566739e71a068a4c347f
重要度: low
共通原因: terminal-output
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

Task titles, tags and notes controlled by a collaborator or workspace supplier reach human-readable CLI stdout without terminal-control encoding. Shared token names and agent labels in cloud history use the same output boundary. A victim terminal can interpret embedded controls as display instructions and, when enabled, clipboard operations; code execution is not established.

## 根本原因
{
  "evidenceRefs": [
    "untrusted-text",
    "text-render",
    "body-render",
    "default-commands",
    "stdout-sink",
    "tsv-gap",
    "cloud-log-text"
  ],
  "summary": "Add/Edit retain arbitrary strings. Node/detail/tree/proposal renderers interpolate shared titles, tags and notes, and cloud listing/log renderers interpolate names and recorded token/agent labels. The CLI print function writes the human-readable string verbatim. TSV escapes only backslash, tab, newline and carriage return, leaving ESC untouched; trailing-whitespace trimming of notes likewise leaves embedded OSC/CSI. JSON and JSONL serializer paths escape controls. The missing terminal-safe encoding at the shared text-to-stdout boundary affects the reported views."
}

## 修正方針
Apply a common terminal-safe encoder at every human-readable output boundary for externally supplied task content, proposals, tree text, cloud names/token/agent labels and error text. Render control characters visibly while preserving intended note newlines and formatting, and preserve stored content and structured JSON/JSONL serialization.

## 検証
{
  "assertions": [
    "Node text is accepted without ESC filtering.",
    "Human-readable views preserve control bytes.",
    "JSON/JSONL encoding escapes those bytes instead."
  ],
  "counterEvidence": [
    "JSON and JSONL outputs serialize control characters.",
    "The native GUI displays text rather than interpreting terminal sequences.",
    "Effects beyond display manipulation depend on the terminal's enabled features.",
    "JSON/JSONL serialize controls safely; GPUI text rendering is not a terminal command sink.",
    "OSC52 effects require a permitting terminal; no ratatui escape handling flaw or general RCE is asserted."
  ],
  "evidenceRefs": [
    "untrusted-text",
    "text-render",
    "body-render",
    "default-commands",
    "stdout-sink",
    "tsv-gap",
    "cloud-log-text"
  ],
  "limitations": [
    "No arbitrary command or code execution is claimed.",
    "No active terminal-control input was created or displayed.",
    "No terminal sequence was emitted or runtime terminal behavior tested.",
    "Impact is limited to output interpretation; protected token/file access is not established by this issue alone."
  ],
  "method": "semantic reduction of already-validated static source findings; no additional validation",
  "status": "validated",
  "summary": "Shared node strings flow unchanged through default ls/ready/show/milestone and related text views into print; TSV does not neutralize ESC. Cloud logs directly interpolate recorded token names and optional agent labels, while operation JSON within the same line is serialized. An editor can choose a token name with an unscoped account token and then write to shared history. GUI text rendering is not a terminal interpreter; OSC52/clipboard effects depend on terminal settings."
}

## 回帰確認
[
  "Text/TSV/detail/cloud-log views must not emit raw ESC controls from shared fields.",
  "JSON and JSONL must retain their existing valid escaped representation.",
  "Render title/tags/notes containing ESC/OSC/CSI and verify stdout contains visible escaped data rather than executable terminal sequences.",
  "Keep newline formatting and JSON semantics intact."
]

## 場所
[
  {
    "endLine": 88,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 84
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 117
  },
  {
    "endLine": 293,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 277
  },
  {
    "endLine": 478,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 463
  },
  {
    "endLine": 72,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 71
  },
  {
    "endLine": 181,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 167
  },
  {
    "endLine": 258,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 248
  },
  {
    "endLine": 224,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "sink",
    "startLine": 218
  },
  {
    "endLine": 251,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "sink",
    "startLine": 241
  },
  {
    "endLine": 119,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 117
  },
  {
    "endLine": 73,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 71
  },
  {
    "endLine": 281,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 277
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 124
  },
  {
    "endLine": 360,
    "path": "crates/topo-core/src/graph.rs",
    "role": "user_input",
    "startLine": 347
  }
]

---

## [F050] Shared task content can alter another user's terminal display

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F050
findingId: csf_6686ea84bb8d065781ced700
occurrenceId: occ_65e7b2263ed4559aec1c2623
重要度: low
共通原因: terminal-output
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

Task titles, tags and notes can contain decoded terminal controls. Default CLI listing/detail rendering emits them directly to stdout, so an editor or untrusted workspace author can clear or reposition the victim's terminal and forge visible task status output.

## 根本原因
{
  "evidenceRefs": [
    "add-edges",
    "show-text-entry",
    "raw-text-fields",
    "raw-note-output",
    "text-format-variants",
    "stdout-no-escape"
  ],
  "summary": "`Add` assigns caller-provided title/tags/notes as strings. Human-readable render functions interpolate those fields and append note contents with only trailing whitespace trimming. `print()` then writes the result directly to stdout. JSON escaping is applied only in the separate machine-readable path, so decoded ESC controls remain active terminal instructions in default text, TSV and other plain-text views."
}

## 修正方針
Centralize terminal-safe encoding of untrusted fields across text, TSV, graph, proposal and error rendering. Escape ESC and other active control bytes while preserving intended note line breaks; keep machine-readable JSON output unchanged.

## 検証
{
  "counterEvidence": [
    "JSON/JSONL serialization escapes controls; validated IDs used in ID-only output are safe.",
    "GUI content is displayed as text and does not use this terminal sink.",
    "TUI uses widget rendering; this finding does not claim that widgets interpret all raw control sequences.",
    "If output is redirected to a nonterminal data consumer, the demonstrated terminal effect is absent."
  ],
  "evidenceRefs": [
    "add-edges",
    "raw-text-fields",
    "raw-note-output",
    "stdout-no-escape",
    "show-text-entry",
    "text-format-variants"
  ],
  "limitations": [
    "No terminal control sequences were executed.",
    "No shell execution, clipboard behavior or terminal-specific exploit is claimed."
  ],
  "method": "static source trace",
  "status": "validated",
  "summary": "A cloud editor or local task author can store ESC-bearing title or note text. Ordinary ls/show retrieves that graph data, and no source layer neutralizes ESC before writeln to stdout. Standard screen-clear/cursor controls can hide existing output and reposition attacker text, changing displayed task information across the shared-data boundary."
}

## 回帰確認
[
  "ESC-bearing titles/tags/notes appear as inert text in default, TSV and plain-text views.",
  "Notes preserve intentional newlines while terminal-control bytes cannot reach stdout as instructions."
]

## 場所
[
  {
    "endLine": 134,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 124
  },
  {
    "endLine": 88,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 84
  },
  {
    "endLine": 120,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 113
  },
  {
    "endLine": 290,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 280
  },
  {
    "endLine": 469,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 467
  },
  {
    "endLine": 72,
    "path": "crates/topo-cli/src/render.rs",
    "role": "entrypoint",
    "startLine": 42
  }
]

---

## [F056] Reading shared tasks can let their author control the terminal display

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F056
findingId: csf_b645859302b23d4a3cd1ffb1
occurrenceId: occ_2216d4633f2ba9ce01ba6e62
重要度: low
共通原因: terminal-output
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

CLI text output prints shared node titles, tags and notes without escaping terminal controls. A contributor can include control sequences in normal JSON or Markdown content so `topo ls`, `show`, tree/proposal output, or cloud log display interprets them as terminal instructions rather than data.

## 根本原因
{
  "evidenceRefs": [
    "terminal-input",
    "terminal-edit-input",
    "terminal-line",
    "terminal-notes",
    "terminal-tree",
    "terminal-proposal",
    "terminal-tsv",
    "terminal-cloud-logs",
    "terminal-cloud-workspace",
    "terminal-stdout"
  ],
  "summary": "Add and Edit copy contributor-controlled text into nodes. Default node, detail, tree and proposal renderers interpolate it without neutralizing control characters; TSV escaping also leaves ESC intact. The CLI writes the result directly to stdout. Shared workspace names and contributor token names in cloud list/log output use the same raw-text sink."
}

## 修正方針
Visibly encode or remove terminal control characters from every human-readable node, workspace, token and attribution field before formatting. Preserve only deliberate structural line breaks; keep JSON/JSONL modes and validated ID output unchanged.

## 検証
{
  "counterEvidence": [
    "JSON and JSONL modes serialize control characters; operations in cloud logs are JSON-serialized even though token names are not.",
    "Remote ID-only output uses validated identifiers.",
    "Terminal settings can disable clipboard controls; no readback, code execution or separate TUI rendering exploit was verified.",
    "Ordinary GUI content renders as GPUI strings, with clipboard changes only from explicit text-input actions."
  ],
  "evidenceRefs": [
    "terminal-input",
    "terminal-line",
    "terminal-notes",
    "terminal-command",
    "terminal-stdout",
    "terminal-edit-input",
    "terminal-tsv",
    "terminal-proposal",
    "terminal-tree",
    "terminal-cloud-logs",
    "terminal-cloud-workspace"
  ],
  "limitations": [
    "No terminal-control payload was executed.",
    "Clipboard impact is conditional; display manipulation is the primary finding."
  ],
  "method": "static source trace",
  "summary": "String fields accept JSON-decoded or Markdown control characters. Human-readable `ls`/`show` use raw renderers, and `print()` writes their resulting bytes unchanged. A terminal that interprets controls can change cursor/display state and conceal or spoof surrounding CLI output; enabled clipboard controls can additionally replace clipboard content."
}

## 回帰確認
[
  "Control-bearing titles, tags, notes and token/workspace names must be rendered as visible data without active terminal instruction bytes.",
  "Check list, detail, tree, proposal, TSV and cloud log formatting, while preserving intended multiline layout."
]

## 場所
[
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 124
  },
  {
    "endLine": 88,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 84
  },
  {
    "endLine": 120,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 100
  },
  {
    "endLine": 478,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 463
  },
  {
    "endLine": 286,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 277
  },
  {
    "endLine": 156,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 154
  },
  {
    "endLine": 73,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 61
  },
  {
    "endLine": 181,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 167
  },
  {
    "endLine": 258,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 247
  },
  {
    "endLine": 250,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "propagation",
    "startLine": 241
  },
  {
    "endLine": 223,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "propagation",
    "startLine": 216
  }
]

---

## [F060] Displaying shared text can inject terminal controls through CLI output

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F060
findingId: csf_68d302cb6139d7283493e4e7
occurrenceId: occ_12fa7e9f039213c0ea1b481a
重要度: low
共通原因: terminal-output
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

Task titles, tags and notes controlled by a collaborator or workspace supplier reach human-readable CLI stdout without terminal-control encoding. Shared token names and agent labels in cloud history use the same output boundary. A victim terminal can interpret embedded controls as display instructions and, when enabled, clipboard operations; code execution is not established.

## 根本原因
{
  "evidenceRefs": [
    "untrusted-text",
    "text-render",
    "body-render",
    "default-commands",
    "stdout-sink",
    "tsv-gap",
    "cloud-log-text",
    "terminal-model",
    "terminal-notes",
    "terminal-stdout"
  ],
  "summary": "Add/Edit retain arbitrary strings. Node/detail/tree/proposal renderers interpolate shared titles, tags and notes, and cloud listing/log renderers interpolate names and recorded token/agent labels. The CLI print function writes the human-readable string verbatim. TSV escapes only backslash, tab, newline and carriage return, leaving ESC untouched; trailing-whitespace trimming of notes likewise leaves embedded OSC/CSI. JSON and JSONL serializer paths escape controls. The missing terminal-safe encoding at the shared text-to-stdout boundary affects the reported views. The incoming source also includes local filename-compatible identifiers among the strings reaching human-readable output and its remediation tests; identifier validation at explicit add/import boundaries must not be generalized to every local file/library path."
}

## 修正方針
Apply a common terminal-safe encoder at every human-readable output boundary for externally supplied task content, local identifiers, proposals, tree text, cloud names/token/agent labels and error text. Render ESC/OSC/CSI/C0 controls visibly while preserving intended layout and note newlines, stored content, and structured JSON/JSONL serialization.

## 検証
{
  "assertions": [
    "Node text is accepted without ESC filtering.",
    "Human-readable views preserve control bytes.",
    "JSON/JSONL encoding escapes those bytes instead."
  ],
  "counterEvidence": [
    "JSON and JSONL outputs serialize control characters.",
    "The native GUI displays text rather than interpreting terminal sequences.",
    "Effects beyond display manipulation depend on the terminal's enabled features.",
    "JSON/JSONL serialize controls safely; GPUI text rendering is not a terminal command sink.",
    "OSC52 effects require a permitting terminal; no ratatui escape handling flaw or general RCE is asserted.",
    "JSON/JSONL serialization escapes control characters.",
    "Impact depends on emulator features; OSC 52 can be disabled."
  ],
  "evidenceRefs": [
    "untrusted-text",
    "text-render",
    "body-render",
    "default-commands",
    "stdout-sink",
    "tsv-gap",
    "cloud-log-text",
    "terminal-model",
    "terminal-notes",
    "terminal-stdout"
  ],
  "limitations": [
    "No arbitrary command or code execution is claimed.",
    "No active terminal-control input was created or displayed.",
    "No terminal sequence was emitted or runtime terminal behavior tested.",
    "Impact is limited to output interpretation; protected token/file access is not established by this issue alone.",
    "No live terminal payload was executed.",
    "No command execution or terminal-specific file access is claimed."
  ],
  "method": "semantic reduction of already-validated static source findings; no additional validation",
  "status": "validated",
  "summary": "Shared node strings flow unchanged through default ls/ready/show/milestone and related text views into print; TSV does not neutralize ESC. Cloud logs directly interpolate recorded token names and optional agent labels, while operation JSON within the same line is serialized. An editor can choose a token name with an unscoped account token and then write to shared history. GUI text rendering is not a terminal interpreter; OSC52/clipboard effects depend on terminal settings. The incoming framing also includes local filename-compatible identifiers and all text/proposal views; its attack-path likelihood is medium rather than the retained aggregate's high, depending on terminal features, and remains recorded in source assessments."
}

## 回帰確認
[
  "Text/TSV/detail/cloud-log views must not emit raw ESC controls from shared fields.",
  "JSON and JSONL must retain their existing valid escaped representation.",
  "Render title/tags/notes containing ESC/OSC/CSI and verify stdout contains visible escaped data rather than executable terminal sequences.",
  "Keep newline formatting and JSON semantics intact.",
  "Verify titles, tags, notes, local IDs, and proposal titles containing ESC/OSC/C0 controls cannot emit executable terminal sequences in any text view.",
  "Verify ordinary Unicode text and deliberate output layout remain readable."
]

## 場所
[
  {
    "endLine": 88,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 84
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 117
  },
  {
    "endLine": 293,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 277
  },
  {
    "endLine": 478,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 463
  },
  {
    "endLine": 72,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 71
  },
  {
    "endLine": 181,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 167
  },
  {
    "endLine": 258,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 248
  },
  {
    "endLine": 224,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "sink",
    "startLine": 218
  },
  {
    "endLine": 251,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "sink",
    "startLine": 241
  },
  {
    "endLine": 119,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 117
  },
  {
    "endLine": 73,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 71
  },
  {
    "endLine": 281,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 277
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 124
  },
  {
    "endLine": 360,
    "path": "crates/topo-core/src/graph.rs",
    "role": "user_input",
    "startLine": 347
  },
  {
    "endLine": 102,
    "path": "crates/topo-core/src/model.rs",
    "role": "user_input",
    "startLine": 82
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 113
  },
  {
    "endLine": 286,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 278
  },
  {
    "endLine": 119,
    "path": "crates/topo-cli/src/render.rs",
    "role": "entrypoint",
    "startLine": 100
  },
  {
    "endLine": 281,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 277
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "propagation",
    "startLine": 124
  }
]

---

## [F069] Displaying shared text can inject terminal controls through CLI output

スキャン: 44dbfab6-0d90-475d-b2a3-4df6534779e5
指摘番号: F069
findingId: csf_46e5c1ebe891de44cc463838
occurrenceId: occ_53de67e8797e848b0a67062a
重要度: low
共通原因: terminal-output
元データ: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/a11e0dc222073580267e9db95182657c54532148_20261003T004741Z_cj30b2n2/findings.json

Shared task titles/tags/notes, local display identifiers, owner-chosen workspace names and recorded writer token/agent labels reach human-readable CLI stdout without terminal-control encoding. A victim terminal can interpret the bytes as display instructions and, when enabled, clipboard operations; JSON/JSONL and native GUI text counterevidence remain explicit, and code execution is not established.

## 根本原因
{
  "evidenceRefs": [
    "untrusted-text",
    "text-render",
    "body-render",
    "default-commands",
    "stdout-sink",
    "tsv-gap",
    "cloud-log-text",
    "terminal-model",
    "terminal-notes",
    "terminal-stdout",
    "dedup-0003-3-arbitrary-task-strings",
    "dedup-0003-3-default-human-output",
    "dedup-0003-3-plain-title-tags",
    "dedup-0003-3-plain-notes",
    "dedup-0003-3-cloud-name-validation",
    "dedup-0003-3-owner-controlled-workspace-name",
    "dedup-0003-3-cloud-list-terminal-name",
    "dedup-0003-3-unrestricted-token-name-input",
    "dedup-0003-3-audit-label-attribution",
    "dedup-0003-3-text-to-stdout"
  ],
  "summary": "Add/Edit retain arbitrary strings. Node/detail/tree/proposal renderers interpolate shared titles, tags and notes, and cloud listing/log renderers interpolate names and recorded token/agent labels. The CLI print function writes the human-readable string verbatim. TSV escapes only backslash, tab, newline and carriage return, leaving ESC untouched; trailing-whitespace trimming of notes likewise leaves embedded OSC/CSI. JSON and JSONL serializer paths escape controls. The missing terminal-safe encoding at the shared text-to-stdout boundary affects the reported views. The incoming source also includes local filename-compatible identifiers among the strings reaching human-readable output and its remediation tests; identifier validation at explicit add/import boundaries must not be generalized to every local file/library path. The additional cloud-name trace shows shared workspace-name validation only trims/checks length, owner-authorized rename persists the remaining control-bearing string, and cloud ls prints it raw. Unrestricted token creation accepts similarly valid-length labels; writer operations record the authenticated token name, and cloud log JSON-escapes operation contents while directly interpolating the surrounding label."
}

## 修正方針
Apply a common terminal-safe encoder at every human-readable output boundary for externally supplied task content, local identifiers, proposals, tree text, cloud names/token/agent labels and error text. Render ESC/OSC/CSI/C0 controls visibly while preserving intended layout and note newlines, stored content, and structured JSON/JSONL serialization.

## 検証
{
  "assertions": [
    "Node text is accepted without ESC filtering.",
    "Human-readable views preserve control bytes.",
    "JSON/JSONL encoding escapes those bytes instead."
  ],
  "counterEvidence": [
    "JSON and JSONL outputs serialize control characters.",
    "The native GUI displays text rather than interpreting terminal sequences.",
    "Effects beyond display manipulation depend on the terminal's enabled features.",
    "JSON/JSONL serialize controls safely; GPUI text rendering is not a terminal command sink.",
    "OSC52 effects require a permitting terminal; no ratatui escape handling flaw or general RCE is asserted.",
    "JSON/JSONL serialization escapes control characters.",
    "Impact depends on emulator features; OSC 52 can be disabled.",
    "JSON and JSONL serialize controls as escaped data.",
    "GPUI uses native text rendering, so this finding does not establish a GUI terminal sink.",
    "Clipboard alteration needs a supported/enabled terminal escape sequence; no automatic shell execution is proven."
  ],
  "evidenceRefs": [
    "untrusted-text",
    "text-render",
    "body-render",
    "default-commands",
    "stdout-sink",
    "tsv-gap",
    "cloud-log-text",
    "terminal-model",
    "terminal-notes",
    "terminal-stdout",
    "dedup-0003-3-arbitrary-task-strings",
    "dedup-0003-3-plain-title-tags",
    "dedup-0003-3-plain-notes",
    "dedup-0003-3-text-to-stdout",
    "dedup-0003-3-default-human-output",
    "dedup-0003-3-cloud-name-validation",
    "dedup-0003-3-owner-controlled-workspace-name",
    "dedup-0003-3-cloud-list-terminal-name",
    "dedup-0003-3-unrestricted-token-name-input",
    "dedup-0003-3-audit-label-attribution"
  ],
  "limitations": [
    "No arbitrary command or code execution is claimed.",
    "No active terminal-control input was created or displayed.",
    "No terminal sequence was emitted or runtime terminal behavior tested.",
    "Impact is limited to output interpretation; protected token/file access is not established by this issue alone.",
    "No live terminal payload was executed.",
    "No command execution or terminal-specific file access is claimed.",
    "Terminal capability and victim output mode vary; the verified mechanism is unneutralized bytes, not an exercised terminal payload."
  ],
  "method": "semantic reduction of already-validated Standard source findings; no additional validation",
  "status": "validated",
  "summary": "Shared node strings flow unchanged through default ls/ready/show/milestone and related text views into print; TSV does not neutralize ESC. Cloud logs directly interpolate recorded token names and optional agent labels, while operation JSON within the same line is serialized. An editor can choose a token name with an unscoped account token and then write to shared history. GUI text rendering is not a terminal interpreter; OSC52/clipboard effects depend on terminal settings. A prior source's framing also includes local filename-compatible identifiers and all text/proposal views; that source's attack-path likelihood is medium rather than the retained aggregate's high, depending on terminal features, and remains recorded in source assessments. The latest source separately preserves owner-authorized workspace rename/name display and authenticated token-label storage before another member reads history. A user's private token listing alone is self-only; the shared-history label establishes the cross-member boundary."
}

## 回帰確認
[
  "Text/TSV/detail/cloud-log views must not emit raw ESC controls from shared fields.",
  "JSON and JSONL must retain their existing valid escaped representation.",
  "Render title/tags/notes containing ESC/OSC/CSI and verify stdout contains visible escaped data rather than executable terminal sequences.",
  "Keep newline formatting and JSON semantics intact.",
  "Verify titles, tags, notes, local IDs, and proposal titles containing ESC/OSC/C0 controls cannot emit executable terminal sequences in any text view.",
  "Verify ordinary Unicode text and deliberate output layout remain readable.",
  "Print tasks containing ESC/OSC/CSI and other control bytes and assert text/TSV emit safe visible representations.",
  "Confirm ordinary Unicode titles and multiline notes remain readable and JSON/JSONL preserve the original data."
]

## 場所
[
  {
    "endLine": 88,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 84
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 117
  },
  {
    "endLine": 293,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 277
  },
  {
    "endLine": 478,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 463
  },
  {
    "endLine": 72,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 71
  },
  {
    "endLine": 181,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 167
  },
  {
    "endLine": 258,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 248
  },
  {
    "endLine": 224,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "sink",
    "startLine": 218
  },
  {
    "endLine": 251,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "sink",
    "startLine": 241
  },
  {
    "endLine": 119,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 117
  },
  {
    "endLine": 73,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 71
  },
  {
    "endLine": 281,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 277
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 124
  },
  {
    "endLine": 360,
    "path": "crates/topo-core/src/graph.rs",
    "role": "user_input",
    "startLine": 347
  },
  {
    "endLine": 102,
    "path": "crates/topo-core/src/model.rs",
    "role": "user_input",
    "startLine": 82
  },
  {
    "endLine": 118,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 113
  },
  {
    "endLine": 286,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 278
  },
  {
    "endLine": 119,
    "path": "crates/topo-cli/src/render.rs",
    "role": "entrypoint",
    "startLine": 100
  },
  {
    "endLine": 281,
    "path": "crates/topo-cli/src/main.rs",
    "role": "entrypoint",
    "startLine": 277
  },
  {
    "endLine": 133,
    "path": "crates/topo-core/src/ops.rs",
    "role": "propagation",
    "startLine": 124
  },
  {
    "endLine": 89,
    "path": "crates/topo-cli/src/render.rs",
    "role": "root_control",
    "startLine": 84
  },
  {
    "endLine": 120,
    "path": "crates/topo-cli/src/render.rs",
    "role": "propagation",
    "startLine": 117
  },
  {
    "endLine": 287,
    "path": "crates/topo-cli/src/main.rs",
    "role": "sink",
    "startLine": 277
  },
  {
    "endLine": 35,
    "path": "crates/topo-core/src/ops.rs",
    "role": "user_input",
    "startLine": 27
  },
  {
    "endLine": 53,
    "path": "crates/topo-server/src/routes/workspaces.rs",
    "role": "propagation",
    "startLine": 44
  },
  {
    "endLine": 224,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "root_control",
    "startLine": 216
  },
  {
    "endLine": 49,
    "path": "crates/topo-server/src/routes/tokens.rs",
    "role": "propagation",
    "startLine": 37
  },
  {
    "endLine": 251,
    "path": "crates/topo-cli/src/cloud.rs",
    "role": "root_control",
    "startLine": 241
  },
  {
    "endLine": 74,
    "path": "crates/topo-server/src/write.rs",
    "role": "propagation",
    "startLine": 64
  }
]


## 対応結果（2026-10-03）
結果: fixed。stdout/通常エラーの共通出力でESC/OSC/C1/CRを可視化。JSON/JSONLは有効なUnicodeエスケープで値を復元可能。日本語・改行・tab・パイプ互換性を検証。
最終検証: cargo test --locked --workspace 116成功/既存3ignore、fmt、native/Worker clippy -D warnings、local Worker/D1 smoke成功。詳細: /Users/r4ai/.codex/state/plugins/codex-security/scans/topological-todo/artifacts-6a4e5a1e5a63564edc8ca8693b4bf5cc93fdb5baaaf0c17a2de44b08336f148b/artifacts/report_fix_20261003.md
