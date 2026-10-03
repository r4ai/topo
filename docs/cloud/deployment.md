# Deployment

The server is one Cloudflare Worker with one D1 database. It keeps no state
between requests, so there is nothing to operate besides the two.

## Configuration

[`crates/topo-server/wrangler.toml`](../../crates/topo-server/wrangler.toml)
declares the Worker, its D1 binding, and a `staging` environment beside
production. `worker-build` compiles the crate to WebAssembly and writes
`build/index.js`, which is the Worker's entry point.

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

4. Add a Cloudflare rate limiting rule on `POST /v1/auth/github`. It is the one
   route that calls GitHub without a token of this server, and the rule keeps
   it from being used to exhaust the app's GitHub quota.
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
