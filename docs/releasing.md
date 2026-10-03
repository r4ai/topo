# CI and releases

CI runs for pull requests, main, merge queues, and manual requests. Rust is pinned
in `rust-toolchain.toml`; update the two workflow toolchain inputs and cache key
when changing it. CI checks formatting, Clippy with warnings denied, and tests.
Linux and Windows test the core, Jev client and CLI/TUI; macOS also tests GPUI.
All three operating systems compile-check the native GUI, including Windows icon resources.
Workflows and packaging scripts are linted. Cargo always uses the checked-in lockfile.
Dependabot opens weekly Cargo and SHA-pinned Actions updates for review.

## Publish a version

1. Change `workspace.package.version` in `Cargo.toml`, regenerate `Cargo.lock`,
   and merge the change into main after CI passes.
2. Tag that commit and push the tag:

   ```sh
   git switch main
   git pull --ff-only
   git tag -a v0.1.0 -m 'topo v0.1.0'
   git push origin v0.1.0
   ```

The Release workflow verifies the stable `vMAJOR.MINOR.PATCH` tag against the
workspace version and main ancestry. It runs CI on that exact tag commit and
builds CLI archives for Linux x86_64 (glibc), Windows x86_64 (MSVC), and macOS
Apple Silicon and Intel. Both macOS targets include GUI `.pkg` installers; Windows includes a GUI `-setup.exe` installer.
The Windows installer is compiled with Inno Setup supplied by the hosted runner,
then silently installed, upgraded and uninstalled to verify the installed binary
and Start menu shortcut while preserving user settings.
Archives contain the READMEs; CLI archives also contain the bundled agent skill.
Each built CLI runs a smoke test that initializes a workspace, creates a task,
and confirms it appears in JSON ready output.

Only after all checks and builds succeed does the publish job create a draft,
upload all four CLI archives, two macOS installers, one Windows installer, and `SHA256SUMS`, and publish the release with generated
notes. Builds use read-only tokens, checkouts do not persist credentials, release
builds do not reuse CI caches, and only the final publish job can write releases.
The publish job refuses to overwrite an existing release. If publication failed
after creating a draft, inspect and delete only that incomplete draft before retrying.
Tags and published assets must never be moved or replaced; fixes get a new version.

To retry a failed run before a release exists:

```sh
gh workflow run release.yml --ref v0.1.0 -f tag=v0.1.0
```

Alternatively, create a `release/vMAJOR.MINOR.PATCH` branch at the version commit
already on main. This starts the same checks and builds, then creates an annotated
tag and publishes the release only after they pass. Existing tags are never moved.
This path also works when using GitHub's branch API rather than Git push credentials.
Release publication calls the production cloud deployment workflow with the same
source commit; configure its Cloudflare secrets in the production environment.

Manual dispatch must select the same tag as its input, ensuring the reusable CI
workflow tests the actual release source rather than the current main branch.

## Install and verify

Download the archive matching your operating system and CPU from
[GitHub Releases](https://github.com/r4ai/topo/releases). Extract it and place
`topo` (or `topo.exe`) on your PATH. On Windows, use `tar -xzf ARCHIVE.tar.gz`.
Linux binaries are built on Ubuntu 24.04 and require compatible glibc libraries.

Download the matching checksums and verify before running the binary. On Linux,
`sha256sum --ignore-missing --check SHA256SUMS` verifies downloaded archives.
On macOS, use `shasum -a 256 ARCHIVE.tar.gz` and compare to `SHA256SUMS`.
On Windows, use `Get-FileHash ARCHIVE.tar.gz -Algorithm SHA256`.

The macOS GUI installers contain a `topo.app` bundle with the topo logo in
Finder and the Dock. Launch it from Applications, or pass a workspace with
`open /Applications/topo.app --args /path/to/project/.topo`.
The completed bundle is ad hoc signed and strictly verified before packaging,
and the app extracted from each installer is verified again. Developer ID signing
and notarization require separate Apple credentials and are not configured.

For macOS GUI installation, prefer `topo-gui-vVERSION-TARGET.pkg` from the release.
Choose `aarch64-apple-darwin` for Apple Silicon or `x86_64-apple-darwin` for Intel.
The installer places `topo.app` in `/Applications`, including on upgrades. It does
not install the CLI or modify workspaces, user settings, or macOS security policy.
The installer is not Developer ID signed: if macOS blocks it, explicitly allow
that installer in System Settings → Privacy & Security → Open Anyway. Once installed,
open `/Applications/topo.app`. See [macOS installation](macos-install.md).

A valid ad hoc signature verifies bundle integrity; it is not Apple approval.
GUI `.pkg` installers replace the legacy GUI `.tar.gz` downloads from v0.2.0.
A browser-downloaded archive copy of an ad hoc signed app can still be rejected
by Gatekeeper. The installer is the supported installation path; it does not
remove quarantine attributes or disable macOS security policy.

For Windows GUI installation, run `topo-gui-vVERSION-x86_64-pc-windows-msvc-setup.exe`
and open topo from the Start menu. It installs per user into
`%LOCALAPPDATA%\Programs\topo`, supports upgrades, and appears in Settings → Apps
for uninstall. An optional desktop shortcut is available. Workspaces and settings
are preserved. This installer is unsigned and may trigger SmartScreen.

Build provenance attestations are enabled automatically for public repositories:
`gh attestation verify ARCHIVE.tar.gz --repo r4ai/topo`. GitHub requires Enterprise
Cloud for private repository attestations; private repositories can use SHA256
checksums and GitHub's workflow/artifact records. No long-lived release secret
is needed; publication uses the job-scoped `GITHUB_TOKEN`.
