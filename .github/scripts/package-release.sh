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
  tar -czf "dist/topo-gui-$RELEASE_TAG-$TARGET.tar.gz" -C "$smoke_dir" "$(basename "$stage")"
fi
