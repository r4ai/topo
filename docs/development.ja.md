# topo 開発者ガイド

English: [docs/development.md](development.md)

`topo` のリポジトリ構成、開発環境のセットアップ、テスト、およびベンチマークをまとめる。

## 目次

- [クレート構成](#クレート構成)
- [開発環境のセットアップ](#開発環境のセットアップ)
- [ビルドと実行](#ビルドと実行)
- [テストと静的解析](#テストと静的解析)
- [ベンチマーク](#ベンチマーク)
- [ヘッドレススクリーンショット（Visual QA）](#ヘッドレススクリーンショットvisual-qa)
- [リリースプロセス](#リリースプロセス)

## クレート構成

本リポジトリは Cargo ワークスペースとして構成されている。

```text
.
├── crates/
│   ├── topo-core/    # コアDAGエンジン、検証、トポロジカルソート、Markdown永続化
│   ├── topo-cli/     # CLIバイナリ、出力レンダラー、Ratatui TUI
│   ├── topo-gui/     # GPUIベースのGPUアクセラレーションデスクトップGUI
│   ├── topo-jev/     # 決定モデル（System One）クライアントおよび最適化ロジック
│   ├── topo-cloud/   # クラウドAPIクライアント（認証、トークン、同期）
│   └── topo-server/  # Cloudflare Workers + D1 クラウドAPI実装
├── docs/             # 技術ドキュメント
├── assets/           # アイコン・ロゴ・アセット
└── skills/           # AIエージェント向け指示セット（SKILL.md）
```

### 各クレートの責務

| クレート | 責務と特徴 |
| :--- | :--- |
| `topo-core` | タスク・マイルストーンの有向非巡回グラフモデル。サイクルの検証、トポロジカルソート、クリティカルパス計算、`.topo/nodes/<id>.md` のパースと保存を担当。Wasmターゲットにも対応 |
| `topo-cli` | ユーザーが実行する `topo` コマンドラインツール。引数解析、パイプライン処理、TSV/JSON/Mermaid等の出力、およびRatatuiによるTUIダッシュボードを提供 |
| `topo-gui` | GPUIを採用したネイティブデスクトップアプリ。GPUアクセラレーションによるキャンバス描画、ファイル変更の自動検知、直感的なドラッグ＆ドロップ操作を実現 |
| `topo-jev` | Jev互換の決定モデルAPIと通信し、不足している依存関係、重複、マイルストーンへの配置、ノード種別、優先順位の提案を行う |
| `topo-cloud` | GitHubデバイスフローによる認証、トークン管理、およびリモートサーバーとの同期の基本機能を提供 |
| `topo-server` | Cloudflare Workers上で動作するステートレスAPI。D1データベースを使い、複数クライアント間のアトミックな更新を直列化し、サーバー側でもグラフの不変条件を検証 |

## 開発環境のセットアップ

### 前提条件

- Rust 1.96.0（`rust-toolchain.toml` で固定。rustup が自動でインストールする）
- macOS, Linux, または Windows
- Linux で `topo-gui` をビルドする場合のみ:
  ```bash
  sudo apt-get install -y libasound2-dev libfontconfig-dev libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev libssl-dev libclang-dev
  ```

```bash
# リポジトリのクローン
git clone https://github.com/r4ai/topo.git
cd topo

# Rustツールチェーンの確認（rust-toolchain.toml に基づき自動選択）
rustc --version
cargo --version
```

## ビルドと実行

```bash
# CLIのビルド
cargo build -p topo-cli

# デバッグビルドのCLIを実行
cargo run -p topo-cli -- --help

# ネイティブGUIを起動（Releaseビルド推奨）
cargo run -p topo-gui --release

# TUIを起動
cargo run -p topo-cli -- tui
```

## テストと静的解析

CIの主なチェックは手元で実行できる。

```bash
# コードフォーマットの検証
cargo fmt --all -- --check

# Clippy（警告をエラーとして処理）
cargo clippy --workspace --all-targets -- -D warnings

# 全テストの実行
cargo test --workspace

# GUI関連のテストのみを実行
cargo test -p topo-gui
```

CIがGUIのテストと上記のワークスペース全体のコマンドを実行するのは macOS のみである。Linux と Windows では `--exclude topo-gui` を付け、GUIは `cargo check -p topo-gui` でコンパイル確認だけを行う。CIはこのほかに次も実行する。

```bash
# スクリーンショット版GUIのLint（macOS）
cargo clippy -p topo-gui --all-targets --features screenshot -- -D warnings

# サーバーのWorkerビルドのLint
cargo clippy -p topo-server --target wasm32-unknown-unknown -- -D warnings
```

## ベンチマーク

GUIの描画性能および大規模グラフの計算速度を計測するためのベンチマークが用意されている。

```bash
# キャンバス描画のCPUフレーム時間計測
cargo test -p topo-gui --release canvas_frame_benchmark -- --ignored --nocapture

# グリッド描画性能計測
cargo test -p topo-gui --release grid_frame_benchmark -- --ignored --nocapture

# 大規模依存チェーン走査性能計測
cargo test -p topo-gui --release multi_selection_chain_benchmark -- --ignored --nocapture
```

詳細な測定結果およびキャッシュ戦略については [docs/canvas-performance.md](canvas-performance.md) を参照。

## ヘッドレススクリーンショット（Visual QA）

`screenshot` フィーチャを有効にしてビルドすることで、ディスプレイや画面収録権限のない環境でもオフスクリーン描画してPNG画像を出力できる。GPUIのMetalバックエンドを使うため、macOSでのみ動作する。

```bash
cargo run -p topo-gui --features screenshot -- \
  --screenshot qa.png --width 1360 --height 860 --select <node-id>
```

主要オプション：
- `--select <id>`: 特定ノードを選択した状態で描画（カンマ区切りで複数指定可能）
- `--inspector-width <px>`: 右パネルの幅を指定
- `--edit <field>`: 属性（`title`, `priority`, `assignee`, `due`, `tags`, `pr`）を編集中の状態で表示。ノードを1つ指定した `--select` が必要
- `--type <text>`: `--edit` で開いたフィールドにテキストを入力
- `--edit-notes`: ノートを編集中の状態で表示。`--select` が必要で、`--edit` とは併用不可
- `--search <query>`: 検索バーを開いた状態で表示
- `--help-overlay`: キーボードショートカット一覧を表示

## リリースプロセス

リリースタグの付与、バイナリビルド、チェックサム検証、GitHub Releases への発行手順は [docs/releasing.md](releasing.md) に集約されている。
