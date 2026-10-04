# topo Developer Guide

日本語: [docs/development.ja.md](development.ja.md)

Architecture overview, development setup, testing, and benchmarking for `topo`.

## Table of Contents

- [Crate Architecture](#crate-architecture)
- [Development Setup](#development-setup)
- [Building and Running](#building-and-running)
- [Testing and Linting](#testing-and-linting)
- [Benchmarks](#benchmarks)
- [Headless Screenshots (Visual QA)](#headless-screenshots-visual-qa)
- [Release Process](#release-process)

## Crate Architecture

The repository is organized as a Cargo workspace:

```text
.
├── crates/
│   ├── topo-core/    # Core DAG engine, invariant validation, topological sort, Markdown persistence
│   ├── topo-cli/     # CLI binary, output renderers, Ratatui TUI
│   ├── topo-gui/     # GPUI native GPU-accelerated desktop application
│   ├── topo-jev/     # Decision model (System One) client and organization heuristics
│   ├── topo-cloud/   # Cloud API client (authentication, token store, remote sync)
│   └── topo-server/  # Cloudflare Workers + D1 backend API
├── docs/             # Technical documentation
├── assets/           # Application icons, logos, and branding
└── skills/           # AI coding agent instructions (SKILL.md)
```

### Responsibilities

| Crate | Responsibilities |
| :--- | :--- |
| `topo-core` | Models tasks and milestones as a unified DAG. Enforces cycle prevention, evaluates readiness, calculates critical paths, and parses/serializes `.topo/nodes/<id>.md`. Compiles to WebAssembly |
| `topo-cli` | The main `topo` command-line executable. Handles CLI arguments, pipeline integration, text/TSV/JSON/Mermaid rendering, and the Ratatui terminal dashboard |
| `topo-gui` | Native desktop editor built on GPUI. Provides GPU-accelerated canvas rendering, live filesystem synchronization, and drag-and-drop dependency management |
| `topo-jev` | Interacts with Jev-compatible decision model endpoints to propose missing dependencies, duplicates, milestone placement, node kinds, and a priority ranking |
| `topo-cloud` | Implements GitHub device-flow authentication, token persistence, and remote synchronization primitives |
| `topo-server` | Stateless Cloudflare Workers API backed by D1 SQLite. Serializes multi-client writes and enforces server-side graph invariants |

## Development Setup

### Prerequisites

- Rust 1.96.0 (pinned in `rust-toolchain.toml`; rustup installs it automatically)
- macOS, Linux, or Windows
- Linux only, for building `topo-gui`:
  ```bash
  sudo apt-get install -y libasound2-dev libfontconfig-dev libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev libssl-dev libclang-dev
  ```

```bash
# Clone repository
git clone https://github.com/r4ai/topo.git
cd topo

# Verify toolchain
rustc --version
cargo --version
```

## Building and Running

```bash
# Build CLI
cargo build -p topo-cli

# Run debug CLI
cargo run -p topo-cli -- --help

# Run native GUI (release recommended for smooth canvas navigation)
cargo run -p topo-gui --release

# Run TUI
cargo run -p topo-cli -- tui
```

## Testing and Linting

The main CI checks can be run locally:

```bash
# Check code formatting
cargo fmt --all -- --check

# Run Clippy with warnings denied
cargo clippy --workspace --all-targets -- -D warnings

# Run all workspace tests
cargo test --workspace

# Run GUI tests only
cargo test -p topo-gui
```

CI runs GUI tests and the full-workspace commands above on macOS only; on Linux and Windows it adds `--exclude topo-gui` and compile-checks the GUI with `cargo check -p topo-gui`. CI also runs:

```bash
# Lint the screenshot build of the GUI (macOS)
cargo clippy -p topo-gui --all-targets --features screenshot -- -D warnings

# Lint the Worker build of the server
cargo clippy -p topo-server --target wasm32-unknown-unknown -- -D warnings
```

## Benchmarks

Benchmarks measure canvas rendering frame times and large-graph traversal performance:

```bash
# Measure canvas CPU frame times
cargo test -p topo-gui --release canvas_frame_benchmark -- --ignored --nocapture

# Measure background grid rendering performance
cargo test -p topo-gui --release grid_frame_benchmark -- --ignored --nocapture

# Measure large selection traversal performance
cargo test -p topo-gui --release multi_selection_chain_benchmark -- --ignored --nocapture
```

See [docs/canvas-performance.md](canvas-performance.md) for detailed methodology and cached layout analysis.

## Headless Screenshots (Visual QA)

Build with the `screenshot` feature to render offscreen PNG frames without requiring a display or screen-recording permissions. It uses GPUI's Metal backend, so it runs on macOS only:

```bash
cargo run -p topo-gui --features screenshot -- \
  --screenshot qa.png --width 1360 --height 860 --select <node-id>
```

Key options:
- `--select <id>`: Select specific node(s) on initial load (comma-separated)
- `--inspector-width <px>`: Set inspector panel width
- `--edit <field>`: Open an attribute field (`title`, `priority`, `assignee`, `due`, `tags`, `pr`) for inline editing; requires `--select` with one node
- `--type <text>`: Type text into the field opened by `--edit`
- `--edit-notes`: Open notes editor; requires `--select` and cannot be combined with `--edit`
- `--search <query>`: Open search bar with query
- `--help-overlay`: Open keyboard shortcut cheatsheet
- `--theme <dark|light>`: Render with this theme (default: dark)

## Release Process

Tag verification, cross-compilation matrix, checksum generation, and release publishing steps are documented in [docs/releasing.md](releasing.md).
