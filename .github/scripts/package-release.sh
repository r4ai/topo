#!/usr/bin/env bash
set -euo pipefail
: "${RELEASE_TAG:?}" "${TARGET:?}" "${BUILD_GUI:?}"
[[ "$RELEASE_TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]
case "$TARGET" in
  x86_64-unknown-linux-gnu|aarch64-apple-darwin|x86_64-apple-darwin) suffix='' ;;
  x86_64-pc-windows-msvc) suffix='.exe' ;;
  *) echo "Unsupported target: $TARGET" >&2; exit 1 ;;
esac
binary_dir="$PWD/target/$TARGET/release"
version=$("$binary_dir/topo$suffix" --version)
[[ "$version" == "topo-cli ${RELEASE_TAG#v}" ]]
printf '%s\n' "$version"
smoke_dir=$(mktemp -d)
trap 'rm -rf "$smoke_dir"' EXIT
(
  cd "$smoke_dir"
  "$binary_dir/topo$suffix" init
  "$binary_dir/topo$suffix" add 'Release smoke test'
  "$binary_dir/topo$suffix" ready --json > ready.json
  python3 -c 'import json; data = json.load(open("ready.json")); assert "Release smoke test" in str(data), data'
)
mkdir -p dist
stage="$smoke_dir/topo-$RELEASE_TAG-$TARGET"
mkdir -p "$stage"
cp "$binary_dir/topo$suffix" README.md README.ja.md "$stage/"
cp -R skills "$stage/"
tar -czf "dist/topo-$RELEASE_TAG-$TARGET.tar.gz" -C "$smoke_dir" "$(basename "$stage")"
if [[ "$BUILD_GUI" == true && "$TARGET" == x86_64-pc-windows-msvc ]]; then
  compiler='/c/Program Files (x86)/Inno Setup 6/ISCC.exe'
  test -x "$compiler"
  MSYS_NO_PATHCONV=1 "$compiler" "/DAppVersion=${RELEASE_TAG#v}" "/DSourceDir=$(cygpath -w "$binary_dir")" \
    "/DOutputPath=$(cygpath -w "$PWD/dist")" .github/scripts/windows-installer.iss
elif [[ "$BUILD_GUI" == true ]]; then
  stage="$smoke_dir/topo-gui-$RELEASE_TAG-$TARGET"
  mkdir -p "$stage"
  app="$stage/topo.app/Contents"
  mkdir -p "$app/MacOS" "$app/Resources"
  cp "$binary_dir/topo-gui" "$app/MacOS/topo-gui"
  icon_info="$smoke_dir/icon-info.plist"
  xcrun actool assets/branding/topo.icon --compile "$app/Resources" \
    --platform macosx --minimum-deployment-target 13.0 --app-icon topo \
    --output-partial-info-plist "$icon_info" --output-format human-readable-text \
    --warnings --errors
  test -s "$app/Resources/Assets.car"
  test -s "$app/Resources/topo.icns"
  cat > "$app/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>topo</string>
  <key>CFBundleDisplayName</key><string>topo</string>
  <key>CFBundleIdentifier</key><string>dev.r4ai.topo</string>
  <key>CFBundleExecutable</key><string>topo-gui</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleIconFile</key><string>topo.icns</string>
  <key>CFBundleIconName</key><string>topo</string>
  <key>CFBundleShortVersionString</key><string>${RELEASE_TAG#v}</string>
  <key>CFBundleVersion</key><string>${RELEASE_TAG#v}</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
  plutil -lint "$app/Info.plist"
  # The linker signs only the Mach-O binary. Seal the completed bundle too.
  codesign --force --sign - "$stage/topo.app"
  codesign --verify --deep --strict --verbose=2 "$stage/topo.app"
  # Use a non-relocatable payload so upgrades always install in /Applications,
  # rather than replacing a previously extracted copy in Downloads.
  payload="$smoke_dir/installer-payload"
  mkdir -p "$payload"
  ditto "$stage/topo.app" "$payload/topo.app"
  pkgbuild --analyze --root "$payload" "$smoke_dir/components.plist"
  python3 - "$smoke_dir/components.plist" <<'PYCOMP'
import plistlib
import sys
path = sys.argv[1]
with open(path, "rb") as source:
    components = plistlib.load(source)
for component in components:
    component["BundleIsRelocatable"] = False
with open(path, "wb") as destination:
    plistlib.dump(components, destination)
PYCOMP
  installer="dist/topo-gui-$RELEASE_TAG-$TARGET.pkg"
  pkgbuild --root "$payload" --component-plist "$smoke_dir/components.plist" \
    --identifier dev.r4ai.topo.pkg --version "${RELEASE_TAG#v}" \
    --install-location /Applications --ownership recommended "$installer"
  expanded="$smoke_dir/expanded-installer"
  pkgutil --expand-full "$installer" "$expanded"
  codesign --verify --deep --strict --verbose=2 "$expanded/Payload/topo.app"
  python3 - "$expanded" "${RELEASE_TAG#v}" <<'PYVERIFY'
import pathlib
import plistlib
import sys
import xml.etree.ElementTree as ET
expanded, version = pathlib.Path(sys.argv[1]), sys.argv[2]
info = ET.parse(expanded / "PackageInfo").getroot()
assert info.get("install-location") == "/Applications"
assert info.get("relocatable") == "false"
assert info.get("version") == version
assert info.get("identifier") == "dev.r4ai.topo.pkg"
assert len(info.find("relocate")) == 0
assert not (expanded / "Scripts").exists()
with (expanded / "Payload/topo.app/Contents/Info.plist").open("rb") as source:
    app = plistlib.load(source)
assert app["CFBundleIdentifier"] == "dev.r4ai.topo"
assert app["CFBundleShortVersionString"] == version
assert app["CFBundleIconName"] == "topo"
assert (expanded / "Payload/topo.app/Contents/Resources/Assets.car").is_file()
PYVERIFY
fi
