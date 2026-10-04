# topo 利用ガイド

English: [docs/usage.md](usage.md)

`topo` のコマンドリファレンス、各種インターフェース（CLI / TUI / GUI）の操作方法、およびクラウド・AI連携の詳細を解説する。

## 目次

- [コマンドリファレンス](#コマンドリファレンス)
- [CLIの詳細機能](#cliの詳細機能)
- [ノードの属性と仕様](#ノードの属性と仕様)
- [TUI（ターミナルUI）](#tuiターミナルui)
- [Native GUI（デスクトップアプリ）](#native-guiデスクトップアプリ)
- [クラウド同期](#クラウド同期)
- [AI・自動化連携](#ai自動化連携)

## コマンドリファレンス

| コマンド | 説明 | 主要オプション |
| :--- | :--- | :--- |
| `topo init` | ワークスペース（`.topo`）を初期化 | |
| `topo ready` | 前提タスクが完了した着手可能タスクを一覧表示 | `--under <id>`, `--assignee <name>`, `--unassigned`, `--pr <ref>`, `--sort priority`, `--format <fmt>` |
| `topo milestones` | マイルストーン一覧、進捗率、クリティカルパスを表示 | `--format <fmt>` |
| `topo add <title>` | タスクまたはマイルストーンを新規作成 | `--milestone`, `--dep <id>`, `--in <ms>`, `--due <date>`, `--tag <tag>`, `--priority <p>`, `--assignee <name>`, `--pr <ref>`, `--note <text>` |
| `topo link <from> <to>` | `<from>` が `<to>` に依存するエッジを追加 | |
| `topo unlink <from> <to>` | 依存関係を解除 | |
| `topo join <task> <ms>` | タスクをマイルストーンに追加 | |
| `topo leave <task> <ms>` | タスクをマイルストーンから除外 | |
| `topo status <id> <st>` | ステータスを更新（`todo`, `doing`, `done`, `dropped`） | `--if <st>`, `--assign <name>` |
| `topo edit <id>` | ノードの属性を変更 | `--title`, `--due`, `--no-due`, `--tag`, `--priority`, `--no-priority`, `--assignee`, `--no-assignee`, `--pr`, `--unpr`, `--note` |
| `topo rm <id>` | ノードおよび接続エッジを削除 | |
| `topo ls [-]` | ノード一覧を表示（標準入力からのID絞り込みに対応） | `--kind`, `--status`, `--under`, `--all`, `--tag`, `--in`, `--due-before`, `--due-after`, `--no-due`, `--ready`, `--blocked`, `--title`, `--priority`, `--assignee`, `--unassigned`, `--pr`, `--sort priority`, `--format` |
| `topo show <id>` | ノード詳細・隣接ノード・メモ・記録日時を表示 | `--json` |
| `topo deps <id>` | 前提ノードを一覧表示（マイルストーンメンバーを含む） | `--transitive`, `--format`, `--json` |
| `topo dependents <id>` | 指定ノードに依存する後続ノードを一覧表示 | `--transitive`, `--format`, `--json` |
| `topo members <ms>` | マイルストーンに属するタスクを一覧表示 | `--format`, `--json` |
| `topo critical-path <id>` | マイルストーン達成までの最長依存経路を表示 | `--format`, `--json` |
| `topo graph` | 依存グラフを出力 | `--format [tree\|mermaid\|dot]`, `--under <id>` |
| `topo apply` | JSONバッチによるアトミック更新 | `[file]` または標準入力 |
| `topo organize <what>` | 決定モデルによるグラフ最適化の提案・適用 | `deps`, `place`, `dupes`, `kinds`, `prioritize`, `--apply`, `--under <id>`（`prioritize` のみ） |
| `topo tui` | ターミナルUIを起動 | |
| `topo login` / `topo logout` | クラウドサーバーへサインイン／トークン削除 | `--url <url>`, `--name <token-name>` |
| `topo token <cmd>` | トークン管理（`create`, `ls`, `revoke`） | `--name`, `--workspace <id>`, `--expires <dur>` |
| `topo cloud <cmd>` | クラウド同期操作（`push`, `pull`, `link`, `ls`, `members`, `invite`, `remove`, `log`） | `--url <url>`, `--role <role>`, `--after <version>` |

## CLIの詳細機能

### 出力フォーマット

`--json` は全コマンドで使える。`--format` は一覧系コマンド（`ls`, `ready`, `milestones`, `deps`, `dependents`, `members`, `critical-path`）で使え、`--json` とは併用できない。

- `--json`: 完全なJSON構造を出力
- `--format text`: 人間向けの標準テキスト出力
- `--format ids`: ノードIDのみを行区切りで出力（他コマンドへのパイプに最適）
- `--format tsv`: タブ区切りテキスト（ID, 種類, ステータス, 期限, タイトル。ヘッダー行なし）
- `--format jsonl`: 1行1ノードのJSON Lines形式

### 標準入力（パイプライン連携）

`status`, `edit`, `rm`, `join`, `leave`, `link`, `unlink`, `ls` の引数に `-` を指定すると、標準入力から空白区切りのID列を受け取れる。

```bash
# 着手可能なタスクを一括で doing に変更
topo ready --format ids | topo status - doing

# 特定タグのタスクを絞り込み、マイルストーンへ一括追加
topo ls --tag gui --format ids | topo ls - --due-before 2026-10-31 --format ids | topo join - a1b2c3

# マイルストーンの前提タスクのうち、ブロック中のものを抽出
topo deps a1b2c3 --transitive | topo ls - --blocked --format tsv

# jq と組み合わせて特定条件のタスクIDを抽出
topo ready --format jsonl | jq -r 'select(.status == "todo") | .id'
```

すべてのIDと変更内容は保存前に一括検証される。不正なIDが含まれる場合やサイクルが発生する場合は何も変更されない。

## ノードの属性と仕様

### ステータス

- `todo`: 未着手
- `doing`: 進行中
- `done`: 完了（前提タスクとして満たされた状態）
- `dropped`: 中止・対象外（前提タスクとしては完了扱いとなり後続をブロックしない）

### 優先度（Priority）

優先度は任意の属性であり、依存関係の判定や着手可能判定には影響を与えない。

- 指定可能な値: `low`, `medium`, `high`, `urgent`
- 絞り込み: `topo ls --priority high --priority urgent` または `topo ls --no-priority`
- ソート: `--sort priority`（高優先度が先頭、未設定は末尾、同順位はID順）

### 担当者・期日・タグ・Pull Request

- 担当者: `--assignee <name>`（解除は `--no-assignee`、未設定検索は `--unassigned`）
- 期日: `--due YYYY-MM-DD`（解除は `--no-due`、範囲検索は `--due-before`, `--due-after`）
- タグ: `--tag <name>`（複数指定可能）
- Pull Request: `--pr <url|owner/repo#123>`（URLまたはGitHub形式の参照。リンクを1件外すには `--unpr <url|owner/repo#123>`）

### タイムスタンプ記録

各ノードは以下のタイムスタンプをUTC秒単位で自動記録する。

- `created_at`: ノード作成日時
- `updated_at`: ノード内容が実際に変更された日時
- `completed_at`: ステータスが `done` に遷移した日時（`todo` への差し戻しや `dropped` への変更でクリア）

## TUI（ターミナルUI）

`topo tui` でターミナル上のインタラクティブダッシュボードを起動する。

### キーバインド

| キー | アクション |
| :--- | :--- |
| `1` 〜 `4` | ビュー切り替え（1: Ready, 2: Milestones, 3: Open, 4: All） |
| `Tab` | 次のビューへ循環切り替え |
| `j` / `k`, `↓` / `↑` | リスト内のカーソル移動 |
| `Space` | ステータスを循環切り替え（`todo` → `doing` → `done` → `todo`） |
| `x` | ステータスを `done` に変更 |
| `d` | ステータスを `dropped` に変更 |
| `u` | ステータスを `todo` に変更 |
| `q`, `Esc` | TUIを終了 |

## Native GUI（デスクトップアプリ）

GPUIによるネイティブGUI（`topo-gui`）は、GPUアクセラレーションによる高速描画と滑らかなキャンバス操作を提供する。外部プロセスやGitによる変更もリアルタイムに検知して反映する。

### ワークスペース操作

- フォルダを開く: ツールバーのリポジトリ名をクリック、または `Cmd+O`（Linux/Windows: `Ctrl+O`）
- 自動探索: 指定フォルダ自身の `.topo` を開く（Gitリポジトリでなくても動作可能）
- 未初期化フォルダ: 画面上の「Initialize workspace」ボタンからワンクリックで初期化可能
- 履歴保持: 最近開いた最大10件のワークスペースを記憶

### キャンバス操作

- パン（移動）: 背景・カードのドラッグ、中ボタンドラッグ、またはトラックパッドの2本指スクロール
- ズーム: マウスホイール、`Cmd`/`Ctrl` + トラックパッドのスクロール、またはピンチ操作（macOSのみ）。カーソル位置を中心に拡大縮小
- マウスホイールでのパン: `Cmd`/`Ctrl` + ホイール（縦）、`Shift` + ホイール（横）
- 全体表示（Fit）: `f` キー
- 実寸表示: `Cmd+0`、拡大: `Cmd+=`、縮小: `Cmd+-`
- 実寸 / Fit 切り替え: トラックパッドの2本指ダブルタップ（macOSのみ）

### 表示モード

2つのトグルは画面下部のバーにあり、次回起動時も保持されます（ユーザー設定。全ワークスペース共通）。

- 優先度フィルタ: `Shift+P`（指定優先度未満のノードを薄く表示）
- 完了を隠す: `Shift+H`（done / dropped のノードを辺ごと非表示。非表示ノードを経由した辺の付け替えはしません）。隠れたノードは選択・すべて選択・検索の対象からも外れます
- タグでグループ化: `Shift+G`（タグごとの帯に並べ替え。タグ名順で、末尾に `untagged`）。タグは集合として扱うため、複数タグのノードは所属するすべての帯に表示されます。辺は同じ帯の中に描かれ、両端が同じ帯を共有しない辺は描かれません。ただし選択中のノードについては、どの帯にも描かれていない依存・マイルストーンの辺を、選択ノードのカード（最後にクリックしたもの）から相手の最も近いカードへ強調線として1本ずつ描き、たどれるようにします。折りたたまれた帯や「完了を隠す」で隠れた端にはカードがないため線を描きません。帯の見出しをクリックすると折りたたみ／展開（折りたたみは保持されません）
- これらのモードは互いに、また優先度フィルタとも組み合わせられます

### 外観

- モード: System（既定。OSの外観に追従）、Light、Dark の3種類
- 切り替え: `View > Appearance` またはツールバーのテーマボタン。選択はユーザーごとに保存され、すべてのウィンドウに反映されます
- 詳細: [gui/design-system.md](gui/design-system.md)

### ノード操作

- 選択: ノードをクリック（`Cmd` / `Ctrl` + クリックで複数選択）
- 新規作成: `n`（タスク作成）、`m`（マイルストーン作成）
- 前提・後続の追加: `Tab`（選択ノードの後続タスクを作成）、`Shift+Tab`（前提タスクを作成）
- 依存線の接続: ノード端のハンドルをドラッグ、または `Shift` + ドラッグ
  - 他のタスクへ接続: 依存関係を作成
  - マイルストーンへ接続: マイルストーンの構成メンバーに追加
  - 空白エリアへドロップ: 新規後続タスクを作成して自動接続
- 既存ノードとの接続: `l`（前提ノードを選択）、`Shift+l`（後続ノードを選択）
- ステータス変更: `Space`（循環変更）、`x`（Doneトグル）、`1`〜`4`（Todo, Doing, Done, Droppedを直接指定）
- 削除: `Backspace` または `Delete`
- 複製: `Cmd+C` / `Cmd+X` / `Cmd+V`（依存関係を維持したまま新規IDで複製）
- 検索: `/` または `Cmd+K` / `Cmd+F`（タイトル・タグ・IDで検索）
- ヘルプ: `?`（ショートカット一覧を表示）

### インスペクタ（右パネル）

- パネル幅の変更: 左端をドラッグ（次回起動時も幅を保持）
- フィールドのインライン編集: 各行をクリック、またはショートカットキー
  - `Enter` / `r`: タイトル
  - `p`: 優先度
  - `a`: 担当者
  - `d`: 期日
  - `t`: タグ（スペースまたはカンマで確定、Backspaceで削除）
  - `g`: Pull Request（URLまたは `owner/repo#123` を入力）
  - `e`: Markdownノート（詳細は下記）
- ノートエディタ: Markdown（見出し・強調・コード・リンク・リスト・引用）を色分けし、「Saved」/「Unsaved changes」で開いた時点からの変更有無を表示します
  - `Cmd+Enter` で保存して閉じます。`Esc`・他の場所のクリック・別ノードの選択・ウィンドウを閉じる操作は、変更がなければそのまま閉じ、未保存の変更があれば保存（`Enter`）/破棄（`D`）/編集を続ける（`Esc`）を確認します
  - `Home` / `End`（`Cmd+←` / `Cmd+→`、macOS では `Ctrl+A` / `Ctrl+E` も）は改行で区切られた行の先頭 / 末尾へ、`Cmd+↑` / `Cmd+↓`（`Ctrl+Home` / `Ctrl+End`）はノート全体の先頭 / 末尾へ、`Option+←` / `Option+→` は単語単位で移動します。`Shift` を併用すると選択を拡張します
  - `Tab` / `Shift+Tab` で選択行をインデント / アウトデント（選択がなければインデントを入力）、`Enter` でリスト・タスクリスト・引用を継続（空の項目で終了）、`Shift+Enter` は継続しない改行です
  - 長いノートでもカーソルが見える位置までパネルがスクロールします
- 入力補完: 既存の担当者・タグ、相対期日（`+3d`, `friday` など）の入力候補を自動提示

## クラウド同期

Cloudflare Workers と D1 をバックエンドに使い、複数デバイスや並列動作するAIエージェント間でグラフを共有できる。

### ワークスペースの移行と接続

```bash
# 1. サーバーへサインイン（GitHubデバイスフロー）
topo login --url https://topo.example.com

# 2. ローカルのワークスペースをクラウドへプッシュ
topo cloud push --url https://topo.example.com

# 3. 通常通りコマンドを実行（自動的にクラウド側と同期）
topo ready

# 4. ローカルのMarkdownファイルへ書き戻してリンクを解除する場合
topo cloud pull
```

クラウド接続情報は `.topo/config.toml` に保存される。クラウドのトークンはリポジトリ外のユーザー認証情報に保存されるため、`[cloud]` の設定はGitにコミットしても安全である。同じファイルの `[jev]` に Jev の `api_key` を書いた場合はコミットしないこと。

### トークン管理とエージェント連携

CIやAIエージェント向けに、ワークスペース単位でスコープを絞ったトークンを発行できる。

```bash
# 90日間有効なエージェント用トークンを発行
topo token create --name agent-1 --workspace <workspace-id> --expires 90d

# エージェント実行環境で環境変数を設定
export TOPO_CLOUD_URL="https://topo.example.com"
export TOPO_TOKEN="topo_xxxxxxxxxxxx"
export TOPO_AGENT="agent-1"

# 排他制御を伴うタスクの確保（未着手の場合のみ進行中に変更して自身をアサイン）
topo status <id> doing --if todo --assign "$TOPO_AGENT"
```

## AI・自動化連携

### トランザクション一括更新（`topo apply`）

複数のノード追加・依存関係構築・ステータス更新をアトミックに適用する。途中でエラーや循環依存が発生した場合はすべての変更がロールバックされる。

```bash
topo apply --json <<'EOF'
[
  {"op": "add", "ref": "m", "title": "v2.0 リリース", "kind": "milestone"},
  {"op": "add", "ref": "spec", "title": "設計書作成", "in": ["$m"]},
  {"op": "add", "ref": "impl", "title": "実装作業", "depends_on": ["$spec"], "in": ["$m"]},
  {"op": "status", "id": "$spec", "status": "doing"}
]
EOF
```

### 決定モデルによるグラフ整理（`topo organize`）

ローカルまたはリモートの決定モデルと連携し、グラフ構造の不備や改善点を検出する。

```bash
topo organize deps --json        # 不足している依存関係の推定
topo organize place --json       # どのマイルストーンにも属さないタスクの所属先を推薦
topo organize dupes --json       # 重複の疑いがあるタスクの検出
topo organize prioritize --json  # 着手可能タスクの優先度スコアリング
```

`deps`・`place`・`kinds` に `--apply` を付与すると、グラフの検証を通る提案だけを反映する。重複は報告のみでマージされず、`prioritize` は順位を表示するだけである。

### AIコーディングエージェント向けスキル

リポジトリ同梱の `skills/topo/SKILL.md` を利用することで、Claude CodeやCodexなどのAIエージェントが自律的にタスクを登録・着手・完了報告できるよう設計されている。
