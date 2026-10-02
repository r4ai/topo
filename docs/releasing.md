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
Apple Silicon and Intel. Both macOS targets also include separate GUI archives.
Archives contain the READMEs; CLI archives also contain the bundled agent skill.
Each built CLI runs a smoke test that initializes a workspace, creates a task,
and confirms it appears in JSON ready output.

Only after all checks and builds succeed does the publish job create a draft,
upload all six archives plus `SHA256SUMS`, and publish the release with generated
notes. Builds use read-only tokens, checkouts do not persist credentials, release
builds do not reuse CI caches, and only the final publish job can write releases.
The publish job refuses to overwrite an existing release. If publication failed
after creating a draft, inspect and delete only that incomplete draft before retrying.
Tags and published assets must never be moved or replaced; fixes get a new version.

To retry a failed run before a release exists:

```sh
gh workflow run release.yml --ref v0.1.0 -f tag=v0.1.0
```

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

The macOS GUI archives contain both a command-line executable and a `topo.app`
bundle with the topo logo in Finder and the Dock. Launch from your initialized
workspace, or pass the `.topo` directory: `./topo-gui /path/to/project/.topo`.
To launch the bundle with a workspace, use
`open topo.app --args /path/to/project/.topo`. The bundle is unsigned and not
notarized. Apple signing/notarization requires
separate developer credentials and is not configured by this workflow.

Build provenance attestations are enabled automatically for public repositories:
`gh attestation verify ARCHIVE.tar.gz --repo r4ai/topo`. GitHub requires Enterprise
Cloud for private repository attestations; this private repository uses SHA256
checksums and GitHub's workflow/artifact records. No long-lived release secret
is needed; publication uses the job-scoped `GITHUB_TOKEN`.
