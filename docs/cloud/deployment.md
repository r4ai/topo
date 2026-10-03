# Deployment

The server is one Cloudflare Worker with one D1 database. It keeps no state
between requests, so there is nothing to operate besides the two.

The production API is hosted at <https://topo.r4ai.dev>.
Its interactive API reference is at <https://topo.r4ai.dev/docs>.
Sign in from a build of the current CLI with:

```bash
topo login --url https://topo.r4ai.dev
```

## Configuration

[`crates/topo-server/wrangler.toml`](../../crates/topo-server/wrangler.toml)
declares the Worker, its D1 binding, and a `staging` environment beside
production. `worker-build` compiles the crate to WebAssembly and writes
`build/index.js`, which is the Worker's entry point.

Production serves every path on the `topo.r4ai.dev` custom domain. Staging
uses its own `workers.dev` hostname and does not inherit the production domain.

| Name | Kind | Purpose |
| :--- | :--- | :--- |
| `DB` | D1 binding | The database |
| `GITHUB_CLIENT_ID` | Variable in `wrangler.toml` | OAuth app id. Clients fetch it from `GET /v1/auth/config`. |
| `GITHUB_CLIENT_SECRET` | Secret | Authenticates the "check a token" and user lookup calls to GitHub |

## First deployment

These steps need a Cloudflare account and a GitHub account, once per
environment. The commands run in `crates/topo-server`; add `--env staging` for
staging.

1. Create a GitHub OAuth app and enable the device flow in its settings. It
   needs no callback URL that is ever used. Put its client id in
   `wrangler.toml` as `GITHUB_CLIENT_ID`.
2. Create the database and put the id it prints in `wrangler.toml` as
   `database_id`:

   ```bash
   npx wrangler d1 create topo
   ```

3. Store the OAuth app's client secret:

   ```bash
   npx wrangler secret put GITHUB_CLIENT_SECRET
   ```

4. Keep the `AUTH_RATE_LIMITER` binding in `wrangler.toml`. The Worker limits
   `POST /v1/auth/github` to 10 attempts per minute per IP at each Cloudflare
   location before calling GitHub, returning `429` with `Retry-After: 60`
   when exceeded. Production and staging use separate limiter namespaces.
5. Deploy with the workflow below, or by hand:

   ```bash
   npx wrangler d1 migrations apply topo --remote
   ```

   ```bash
   npx wrangler deploy
   ```

## Schema migrations

The schema of [database.md](database.md) lives as numbered SQL files in
`crates/topo-server/migrations`. `wrangler d1 migrations apply` records which
files a database has run and applies the rest. A migration is applied before
the Worker that needs it is deployed, so every migration must leave the
previous Worker working.

## Release

After GitHub release publication, the `Release` workflow calls `Deploy cloud API`
for production using the same tag commit. The production GitHub environment must
contain the Cloudflare secrets below. A deployment failure leaves the published
release intact; retry the failed deployment jobs after correcting the setup.

The `Deploy cloud API` workflow
([`deploy-cloud.yml`](../../.github/workflows/deploy-cloud.yml)) is started by
hand and takes the environment as its input. It runs the CI checks, applies
the migrations, and deploys the Worker. It reads two secrets, which can be set
per GitHub environment:

| Secret | Value |
| :--- | :--- |
| `CLOUDFLARE_API_TOKEN` | A Cloudflare API token limited to Workers and D1 on the account |
| `CLOUDFLARE_ACCOUNT_ID` | The account id |

## Local development

`wrangler dev` runs the Worker against a D1 file under `.wrangler/`.
[`dev/seed.sql`](../../crates/topo-server/dev/seed.sql) adds a user and the
token `topo_dev`, so the API can be used without GitHub:

```bash
npx wrangler d1 migrations apply topo --local
```

```bash
npx wrangler d1 execute topo --local --file dev/seed.sql
```

```bash
npx wrangler dev
```

[`dev/smoke.sh`](../../crates/topo-server/dev/smoke.sh) does all of that and
then drives the real `topo` CLI against the Worker: push, link, eight parallel
writers, a contested claim, and pull. The tests under `cargo test` run the same
router over SQLite and do not need wrangler; the smoke test covers what they
cannot, the WebAssembly build and D1 itself.

The smoke test uses a temporary D1 state directory and preserves existing
local development databases. It also runs
[`dev/metadata-smoke.py`](../../crates/topo-server/dev/metadata-smoke.py) to
check priority filters and sorting, atomic metadata edits and claims,
creation/update/completion times, no-op and failed writes, reopening, and
retention of metadata during pull. To repeat these checks against a deployed
API, create a disposable workspace, push it with the current CLI, then run:

```bash
python3 crates/topo-server/dev/metadata-smoke.py /absolute/path/to/topo /absolute/path/to/disposable-workspace
```

This creates four test nodes and pulls the workspace back to files. Delete
the disposable remote workspace afterwards with the API. For metadata
rollouts, apply `0002_node_metadata.sql` before updating the Worker; check
`/openapi.json` and run the CLI smoke test against the deployed domain before
considering the rollout complete.

## Limits

| Limit | Free plan | Consequence |
| :--- | :--- | :--- |
| CPU time per request | 10 ms | A write parses every node and builds a `Graph`. Large workspaces need the paid plan (30 s). Not measured yet. |
| D1 queries per request | 50 | A write uses one statement to authenticate, four to read, and three to write, per attempt. |
| D1 parameter size | 2 MB | The changed nodes of one write travel as one JSON parameter, which bounds a write, and an import in particular, to 2 MB. |
| D1 rows read per day | 5 million | An unchanged poll reads a handful of rows; a changed one reads every node of the workspace. |
| D1 rows written per day | 100,000 | One row per changed node, plus one per write. |
| Requests per day | 100,000 | One client polling every 5 seconds makes about 17,000. |

The API reference at `/docs` loads Scalar from a CDN, so the page needs
internet access; `/openapi.json` does not.

## If the constraints change

- **Changes must appear instantly**: add one SQLite-backed Durable Object per
  workspace that holds the graph in memory and pushes over WebSocket.
  `workers-rs` supports them, so the server stays in Rust.
- **WebAssembly becomes the obstacle**: the router does not depend on Workers.
  It already runs on the host over SQLite for the tests, so it can be served
  from a container with a SQLite file. That trades the free, unattended
  platform for a small monthly cost and owning the backups.
