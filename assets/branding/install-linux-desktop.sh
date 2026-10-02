#!/usr/bin/env bash
# Register a source-built GUI and its workspace with Linux desktop environments.
set -euo pipefail
if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "Usage: $0 WORKSPACE [TOPO_GUI_BINARY]" >&2
  exit 1
fi
workspace=$(realpath "$1")
if [[ "$(basename "$workspace")" != .topo ]]; then
  workspace="$workspace/.topo"
fi
[[ -d "$workspace/nodes" ]] || { echo "Not an initialized topo workspace: $workspace" >&2; exit 1; }
binary=$(realpath "${2:-$(command -v topo-gui)}")
[[ -x "$binary" ]] || { echo "GUI binary is not executable: $binary" >&2; exit 1; }
asset_dir=$(cd "$(dirname "$0")" && pwd)
data_dir="${XDG_DATA_HOME:-$HOME/.local/share}"
mkdir -p "$data_dir/applications" "$data_dir/icons/hicolor/512x512/apps"
cp "$asset_dir/topo-app-icon.png" "$data_dir/icons/hicolor/512x512/apps/dev.r4ai.topo.png"
# Desktop Exec has its own escaping rules, separate from shell quoting.
python3 - "$binary" "$workspace" "$data_dir/applications/dev.r4ai.topo.desktop" <<'PY'
import pathlib
import sys

def quote(value):
    value = value.replace('%', '%%')
    for char in ('\\', '"', '`', '$'):
        value = value.replace(char, '\\' + char)
    # The desktop file string layer is decoded before the Exec quoting layer.
    return '"' + value.replace('\\', '\\\\') + '"'

binary, workspace, destination = sys.argv[1:]
pathlib.Path(destination).write_text(
    '[Desktop Entry]\nType=Application\nName=topo\n'
    f'Exec={quote(binary)} {quote(workspace)}\n'
    'Icon=dev.r4ai.topo\nStartupWMClass=dev.r4ai.topo\n'
    'Terminal=false\nCategories=Office;ProjectManagement;\n'
)
PY
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$data_dir/applications"
fi
echo "Installed topo launcher for $workspace"
