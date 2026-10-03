# CI and Release Guide

Overview of CI checks, release automation, and binary verification workflows for `topo`.

## CI Pipeline

CI automatically triggers for pull requests, the `main` branch, merge queues, and manual dispatches:

- **Toolchain**: Rust 1.96.0, pinned in `rust-toolchain.toml`. The same version is hard-coded in `ci.yml`, `release.yml`, and `deploy-cloud.yml`; update them together
- **Workflow Lint**: `actionlint` on the workflows and `bash -n` on the packaging scripts
- **Formatting**: `cargo fmt --all -- --check` (macOS runner)
- **Linting**: `cargo clippy --all-targets -- -D warnings`
  - macOS: the whole workspace, plus `topo-gui` with the `screenshot` feature
  - Linux and Windows: every crate except `topo-gui`
  - Linux: additionally `topo-server` for `wasm32-unknown-unknown`
- **Cross-Platform Tests**:
  - Linux and Windows: test every crate except `topo-gui`
  - macOS: runs the full test suite, including GPUI headless tests
  - Linux and Windows compile-check `topo-gui` (including Windows icon resources)
- **Dependencies**: The checked-in `Cargo.lock` is strictly used (`--locked`); Dependabot runs weekly Cargo and GitHub Actions updates

## Publishing a Release

### Standard Flow

1. Update `workspace.package.version` in `Cargo.toml` and regenerate `Cargo.lock`
2. Merge the version bump PR into `main` after all CI checks pass
3. Tag the merge commit and push the tag:

   ```bash
   git switch main
   git pull --ff-only
   git tag -a v0.2.0 -m "topo v0.2.0"
   git push origin v0.2.0
   ```

### Automation Workflow

The release workflow validates the tag and automates artifact generation:

1. Verifies the tag matches `vMAJOR.MINOR.PATCH`, equals `workspace.package.version`, and points at a commit on `main`
2. Runs the full CI test suite on the tagged commit
3. Builds the release assets:
   - CLI archives (`.tar.gz`) for Linux x86_64 (glibc), Windows x86_64 (MSVC), and macOS Apple Silicon and Intel
   - macOS GUI installers (`.pkg`) for Apple Silicon and Intel. The layered icon is compiled once on macOS 26 and shared by both bundles
   - Windows GUI installer (`-setup.exe`), compiled with Inno Setup and then silently installed, upgraded, and uninstalled as a check
4. Executes smoke tests verifying workspace initialization and readiness evaluation
5. Creates a draft release, uploads the seven assets (four CLI archives, two macOS installers, one Windows installer) and `SHA256SUMS`, then publishes it. The notes are `docs/release-notes/<tag>.md` (when that file exists at the tagged commit) followed by GitHub's generated notes
6. Triggers production cloud deployment with the matching source commit

Pushing a `release/vMAJOR.MINOR.PATCH` branch at the version commit already on `main` starts the same workflow. It creates the annotated tag itself after all checks and builds pass.

### Retrying Failed Releases

If a release run fails before a release exists, run it again with the tag as both the ref and the input:

```bash
gh workflow run release.yml --ref v0.2.0 -f tag=v0.2.0
```

The publish job refuses to run while a release for the tag exists, including a draft. If the run failed after creating the draft, delete that incomplete draft before retrying.

Release assets and tags are immutable. Never move or overwrite an existing release; publish a patch version instead.

## Installation and Verification

Download the appropriate archive from [GitHub Releases](https://github.com/r4ai/topo/releases). `<target>` is one of `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, or `x86_64-apple-darwin`:

| Asset | Contents |
| :--- | :--- |
| `topo-<tag>-<target>.tar.gz` | CLI for every target. On Windows, extract with `tar -xzf` |
| `topo-gui-<tag>-<target>.pkg` | macOS GUI installer (both Apple targets) |
| `topo-gui-<tag>-x86_64-pc-windows-msvc-setup.exe` | Windows GUI installer |

v0.2.0 shipped the macOS GUI as `topo-gui-<tag>-<target>.tar.gz` and had no Windows GUI installer.

### Verify Checksums

Always verify downloaded archives against `SHA256SUMS`:

- **Linux**:
  ```bash
  sha256sum --ignore-missing --check SHA256SUMS
  ```
- **macOS**:
  ```bash
  shasum -a 256 topo-v0.2.0-aarch64-apple-darwin.tar.gz
  ```
- **Windows** (PowerShell):
  ```powershell
  Get-FileHash topo-v0.2.0-x86_64-pc-windows-msvc.tar.gz -Algorithm SHA256
  ```

### Build Provenance

GitHub artifact attestations are generated while the repository is public:

```bash
gh attestation verify topo-v0.2.0-aarch64-apple-darwin.tar.gz --repo r4ai/topo
```

### macOS GUI

The `.pkg` installs `topo.app` into `/Applications`, including on upgrades. It does not install the CLI. See [macOS installation](macos-install.md).

The bundle carries an ad hoc signature only: neither it nor the installer is signed with a Developer ID or notarized. If macOS blocks the installer, allow it in System Settings → Privacy & Security → Open Anyway.

```bash
# Launch with a specific workspace
open /Applications/topo.app --args /path/to/project/.topo
```

### Windows GUI

The setup executable installs per user into `%LOCALAPPDATA%\Programs\topo` without administrator rights, adds a Start menu shortcut (desktop shortcut optional), and can be removed from Settings → Apps. It is unsigned and may trigger SmartScreen.
