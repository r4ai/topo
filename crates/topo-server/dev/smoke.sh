#!/usr/bin/env bash
# Runs the real `topo` CLI against the Worker under `wrangler dev` with a local D1.
# It checks what the tests on the host cannot: the WebAssembly build and D1 itself.
set -euo pipefail

server=$(cd "$(dirname "$0")/.." && pwd)
root=$(cd "$server/../.." && pwd)
url=http://localhost:8787
work=$(mktemp -d)
wrangler() { (cd "$server" && npx --yes wrangler@4 "$@"); }

cargo build --quiet --manifest-path "$root/Cargo.toml" -p topo-cli
topo() { "$root/target/debug/topo" "$@"; }

# The secret only has to exist; the smoke test never calls GitHub.
[ -f "$server/.dev.vars" ] || echo 'GITHUB_CLIENT_SECRET=dev' > "$server/.dev.vars"
rm -rf "$server/.wrangler/state"
wrangler d1 migrations apply topo --local > /dev/null
wrangler d1 execute topo --local --file dev/seed.sql > /dev/null

# Job control gives the dev server its own process group, so the whole of it can be stopped.
set -m
wrangler dev --port 8787 > "$work/wrangler.log" 2>&1 &
dev=$!
set +m
trap 'kill -- -$dev 2> /dev/null; rm -rf "$work"' EXIT
until curl --silent --fail --output /dev/null "$url/v1/auth/config"; do
  kill -0 $dev 2> /dev/null || { cat "$work/wrangler.log"; exit 1; }
  sleep 1
done

export TOPO_TOKEN=topo_dev XDG_CONFIG_HOME="$work/config"
unset TOPO_DIR TOPO_CLOUD_URL TOPO_AGENT
mkdir "$work/one" "$work/two"

cd "$work/one"
topo init > /dev/null
milestone=$(topo add 'v1' --milestone --due 2026-10-31)
design=$(topo add "it's; DROP TABLE nodes" --in "$milestone" --note 'notes')
topo add 'build' --in "$milestone" --dep "$design" --tag x > /dev/null
workspace=$(topo cloud push --url "$url" --name smoke --json | python3 -c 'import json, sys; print(json.load(sys.stdin)["workspace"])')

cd "$work/two"
topo cloud link "$workspace" --url "$url" > /dev/null
[ "$(topo ls --all --format ids | wc -l)" -eq 3 ]

# Eight writers at once: the version key serializes them and none is lost.
writers=()
for i in 1 2 3 4 5 6 7 8; do
  TOPO_AGENT="agent-$i" topo add "parallel $i" > /dev/null &
  writers+=($!)
done
wait "${writers[@]}"
[ "$(topo ls --all --format ids | wc -l)" -eq 11 ]

# Of two claims of the same task, one wins.
topo status "$design" doing --if todo
if topo status "$design" doing --if todo 2> /dev/null; then echo 'the second claim succeeded'; exit 1; fi

cd "$work/one"
[ "$(topo cloud log --json | python3 -c 'import json, sys; print(len(json.load(sys.stdin)))')" -eq 10 ]
topo cloud pull > /dev/null
[ "$(ls .topo/nodes | wc -l)" -eq 11 ]
grep -q 'status: doing' ".topo/nodes/$design.md"

curl --silent --fail "$url/openapi.json" | python3 -c 'import json, sys; assert len(json.load(sys.stdin)["paths"]) == 12'
curl --silent --fail "$url/docs" | grep -q scalar
echo "smoke test passed against $url"
