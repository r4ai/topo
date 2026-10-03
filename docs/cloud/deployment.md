# Deployment Guide

The `topo-server` backend runs as a Cloudflare Worker backed by a Cloudflare D1 database. The architecture is stateless, requiring zero persistent process management.

The production API is hosted at `https://topo.r4ai.dev`, with interactive documentation at `https://topo.r4ai.dev/docs`.

Sign in using the CLI:

```bash
topo login --url https://topo.r4ai.dev
```

## Configuration

Worker configuration is defined in [`crates/topo-server/wrangler.toml`](../../crates/topo-server/wrangler.toml):

| Name | Kind | Purpose |
| :--- | :--- | :--- |
| `DB` | D1 binding | Persistent SQLite database connection |
| `GITHUB_CLIENT_ID` | Environment variable | OAuth application ID fetched via `GET /v1/auth/config` |
| `GITHUB_CLIENT_SECRET` | Secret | Authenticates OAuth token validations with GitHub |
| `AUTH_RATE_LIMITER` | Rate limiter binding | Restricts `POST /v1/auth/github` to 10 requests per minute per IP; beyond that it answers `429 rate_limited` with `Retry-After: 60` |

`wrangler.toml` also declares a `staging` environment (`topo-staging`) with its own D1 database, rate limiter namespace, and `workers.dev` hostname.

## Initial Setup

Initial provisioning requires Cloudflare and GitHub credentials. Run the `wrangler` commands in this guide from `crates/topo-server`; add `--env staging` (and the `topo-staging` database name) for staging:

1. **GitHub OAuth Application**: Create an OAuth application on GitHub with Device Flow enabled. Set `GITHUB_CLIENT_ID` in `wrangler.toml`
2. **D1 Database Creation**:
   ```bash
   npx wrangler d1 create topo
   ```
   Copy the emitted database ID into `wrangler.toml` under `database_id`.
3. **Store Secret**:
   ```bash
   npx wrangler secret put GITHUB_CLIENT_SECRET
   ```
4. **Apply Migrations and Deploy**:
   ```bash
   npx wrangler d1 migrations apply topo --remote
   npx wrangler deploy
   ```

## Schema Migrations

Database schema migrations are stored as numbered SQL files in `crates/topo-server/migrations`. Execute pending migrations using:

```bash
npx wrangler d1 migrations apply topo --remote
```

All migrations must remain backward-compatible with currently active Worker builds to allow zero-downtime rollouts.

## Deployment Workflow

[`deploy-cloud.yml`](../../.github/workflows/deploy-cloud.yml) runs the CI checks, applies pending migrations, and deploys the Worker. The release workflow calls it for `production` after publishing a release; it can also be dispatched by hand for `staging` or `production`.

It reads two secrets, set per GitHub environment:

| Secret | Value |
| :--- | :--- |
| `CLOUDFLARE_API_TOKEN` | Cloudflare API token limited to Workers and D1 on the account |
| `CLOUDFLARE_ACCOUNT_ID` | Cloudflare account ID |

## Local Development and Verification

Run the local development server against an isolated local D1 database, from `crates/topo-server`:

```bash
# The Worker requires the secret to exist; any value works without GitHub sign-in
echo 'GITHUB_CLIENT_SECRET=dev' > .dev.vars

# Apply migrations locally
npx wrangler d1 migrations apply topo --local

# Seed development account and test credentials (topo_dev)
npx wrangler d1 execute topo --local --file dev/seed.sql

# Start development worker
npx wrangler dev
```

Run the end-to-end smoke verification script from the repository root:

```bash
bash crates/topo-server/dev/smoke.sh
```

## Platform Limits and Constraints

| Metric | Free Tier Quota | Behavioral Impact |
| :--- | :--- | :--- |
| CPU execution time | 10 ms per request | In-memory validation parses entire workspace; large graphs may require paid tier (up to 30 s) |
| Queries per request | 50 queries | A write uses 1 authentication statement, then 1 membership check, 5 read, and 3 write statements per attempt (up to 3 attempts) |
| Parameter payload | 2 MB | Limits individual atomic write and bulk import payload size to 2 MB |
| Daily read quota | 5,000,000 rows | An unchanged poll reads a handful of rows (token, membership, version); a changed one reads every node of the workspace |
