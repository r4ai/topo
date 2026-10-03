# Install topo on macOS

Download the `.pkg` installer from [GitHub Releases](https://github.com/r4ai/topo/releases):

- Apple Silicon (M1 or later): `topo-gui-v0.2.1-aarch64-apple-darwin.pkg`
- Intel Mac: `topo-gui-v0.2.1-x86_64-apple-darwin.pkg`

Open the installer and follow its steps. It installs `topo.app` in `/Applications`.
Open topo from Applications, then choose your repository or folder. Installing or
upgrading does not change `.topo` workspaces or user configuration. The CLI remains
a separate download. To uninstall the GUI, remove `/Applications/topo.app`.

The installer is not signed with an Apple Developer ID or notarized. If macOS
blocks it, first try opening it, then go to System Settings → Privacy & Security
and click Open Anyway for this installer. Confirm the installation yourself.
No installation script disables Gatekeeper or removes quarantine attributes.

## Why v0.2.0 showed “damaged”

The release copied a linker-signed executable into an app bundle without signing
the completed bundle. `codesign --verify --deep --strict` rejected that bundle
because its resources were not sealed. Starting with v0.2.1, packaging ad hoc signs
the completed bundle, verifies it, builds the installer, expands its payload, and
verifies the installed app's signature again. Ad hoc signing protects integrity;
it does not establish a trusted developer identity. Downloaded archive copies can
still trigger Gatekeeper rejection, so the `.pkg` is the recommended GUI installation.

Verify downloads against the release's `SHA256SUMS` before installation:

```sh
shasum -a 256 topo-gui-v0.2.1-aarch64-apple-darwin.pkg
```

Compare the hash to the corresponding entry in `SHA256SUMS`. Do not apply a blanket
security exception if the downloaded file does not match.

Apple's instructions: [Open apps safely](https://support.apple.com/102445).
