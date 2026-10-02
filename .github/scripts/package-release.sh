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
"$binary_dir/topo$suffix" --version
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
if [[ "$BUILD_GUI" == true ]]; then
  stage="$smoke_dir/topo-gui-$RELEASE_TAG-$TARGET"
  mkdir -p "$stage"
  cp "$binary_dir/topo-gui" README.md README.ja.md "$stage/"
  app="$stage/topo.app/Contents"
  mkdir -p "$app/MacOS" "$app/Resources"
  cp "$binary_dir/topo-gui" "$app/MacOS/topo-gui"
  cp assets/branding/topo.icns "$app/Resources/topo.icns"
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
  <key>CFBundleShortVersionString</key><string>${RELEASE_TAG#v}</string>
  <key>CFBundleVersion</key><string>${RELEASE_TAG#v}</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
  plutil -lint "$app/Info.plist"
  tar -czf "dist/topo-gui-$RELEASE_TAG-$TARGET.tar.gz" -C "$smoke_dir" "$(basename "$stage")"
fi
