# Cloud API Specification

HTTP JSON API under `/v1`. The server stores and serializes state without evaluating DAG semantics. Clients download the workspace graph, compute queries via `topo-core`, and transmit mutations as atomic operations.

Interactive API documentation and schema files are served at `/docs` (via Scalar) and `/openapi.json`.

## Authentication

Every request requires an `Authorization: Bearer <token>` header, except:
- `GET /v1/auth/config` (OAuth configuration)
- `POST /v1/auth/github` (Device flow exchange)
- Documentation endpoints (`/openapi.json`, `/docs`)

Tokens use the format `topo_<hex-32-bytes>`. The server stores only the SHA-256 hash in the `tokens` table.

### Sign-in Flow

Sign-in uses GitHub's OAuth device flow:

```mermaid
sequenceDiagram
    participant C as topo CLI
    participant G as GitHub
    participant S as Server
    C->>S: GET /v1/auth/config
    S-->>C: { github_client_id }
    C->>G: POST /login/device/code (client_id)
    G-->>C: user_code, verification_uri
    Note over C: Displays code; user authorizes in browser
    C->>G: POST /login/oauth/access_token (polling)
    G-->>C: github_access_token
    C->>S: POST /v1/auth/github { access_token, name }
    S->>G: POST /applications/{client_id}/token (client_secret)
    G-->>S: Valid token confirmation with user identity
    S-->>C: { token }
```

The server verifies the GitHub token against GitHub's application token validation endpoint, ensuring tokens obtained for third-party apps cannot authenticate.

### Token Endpoints

```text
GET    /v1/auth/config         GitHub OAuth app configuration
POST   /v1/auth/github         Exchange authorized GitHub token for topo token
GET    /v1/user                Current authenticated user
GET    /v1/tokens              List tokens owned by caller (hashes/secrets omitted)
POST   /v1/tokens              Create token { name, workspace_id?, expires_in? }
DELETE /v1/tokens/{id}         Revoke token
```

Tokens scoped to a `workspace_id` can only access that specific workspace's graph data. They cannot manage memberships, delete workspaces, or issue new credentials.

### Agent Authentication

Headless agents running in CI or cloud containers do not authenticate interactively. Instead, an administrator generates a scoped token:

1. Generate token: `topo token create --name ci-runner --workspace <wid> --expires 90d`
2. Export environment variables in the runner:
   - `TOPO_TOKEN`: Bearer token string
   - `TOPO_CLOUD_URL`: Server URL
   - `TOPO_AGENT`: Optional writer label (e.g. `runner-42`)
3. Checked-out workspaces match `.topo/config.toml` with the environment variables automatically

### Authorization Roles

Access permissions are enforced via `workspace_members`:

- `owner`: Full control, including workspace renaming, deletion, and membership management
- `editor`: Read and mutate graph nodes and edges
- `viewer`: Read-only access to graph data and change logs

## Workspaces and Membership

```text
POST   /v1/workspaces                           Create workspace { name } (caller becomes owner)
GET    /v1/workspaces                           List caller's accessible workspaces
PATCH  /v1/workspaces/{wid}                     Rename workspace { name }
DELETE /v1/workspaces/{wid}                     Delete workspace
GET    /v1/workspaces/{wid}/members             List workspace members
PUT    /v1/workspaces/{wid}/members/{login}     Add or update member role { role }
DELETE /v1/workspaces/{wid}/members/{login}     Remove member
DELETE /v1/workspaces/{wid}/members/by-id/{user_id}   Remove member by GitHub user ID
```

Members are identified by GitHub login. The server resolves logins to immutable GitHub user IDs upon assignment. Workspaces require at least one active owner.

## Graph Endpoints

```text
GET    /v1/workspaces/{wid}/graph               Fetch complete workspace graph and version
PUT    /v1/workspaces/{wid}/graph               Replace entire graph (import)
POST   /v1/workspaces/{wid}/apply               Execute atomic batch mutation
GET    /v1/workspaces/{wid}/changes?after={v}   Audit change log
```

### Reading Graph State

`GET /v1/workspaces/{wid}/graph` returns the entire workspace payload:

```json
{
  "version": 14,
  "nodes": [
    {
      "id": "k2x9ab",
      "kind": "task",
      "title": "Core engine",
      "status": "doing",
      "due": "2026-10-31",
      "tags": ["core"],
      "priority": "high",
      "assignee": "agent-3",
      "prs": ["https://github.com/r4ai/topo/pull/12"],
      "created_at": "2026-10-01T09:00:00Z",
      "updated_at": "2026-10-03T12:30:00Z",
      "depends_on": ["m4p0zz"],
      "milestones": ["a1b2c3"],
      "body": "Task implementation notes"
    }
  ]
}
```

The response includes an `ETag` containing the version number. Polling clients include `If-None-Match: "<version>"` to receive lightweight `304 Not Modified` responses when no changes have occurred.

### Mutating Graph State (`apply`)

Edits are sent as operation batches to `POST /v1/workspaces/{wid}/apply` (`PUT /graph` replaces the whole graph and is meant for imports):

```http
POST /v1/workspaces/k3v9x0q2m1ab/apply
Idempotency-Key: 7c0e6f2a-5d0b-4c57-9a55-2f6f1f1f7a10
Topo-Agent: agent-3

{
  "ops": [
    { "op": "add", "ref": "m", "title": "v2.0 Release", "kind": "milestone" },
    { "op": "add", "ref": "core", "title": "Core engine", "in": ["$m"] },
    { "op": "status", "id": "$core", "status": "doing" }
  ]
}
```

- **All-or-Nothing Semantics**: Any validation error or cycle aborts the entire batch
- **Optimistic Concurrency**: The `if_status` parameter allows atomic task claiming without race conditions
- **Idempotency**: Clients supply `Idempotency-Key` headers on all write requests to prevent duplicate execution during retries

### Error Codes

Error responses return `{"error": {"code": string, "message": string}}`:

| Status | Code | Meaning |
| :--- | :--- | :--- |
| 400 | `bad_request` | Invalid JSON syntax, unknown operation, or missing required headers |
| 401 | `unauthenticated` | Missing, invalid, or expired authentication token |
| 403 | `forbidden` | Insufficient role or token permission |
| 404 | `not_found` | Resource does not exist or caller lacks membership |
| 409 | `conflict` | Optimistic concurrency check failed or last owner removal rejected |
| 412 | `precondition_failed` | `If-Match` version mismatch |
| 422 | `invalid_graph` | Graph invariant violation (e.g. cycle detected, invalid edge reference) |
| 429 | `rate_limited` | Too many sign-in attempts on `POST /v1/auth/github`; sent with `Retry-After: 60` |
| 500 | `internal` | Unexpected server error |
| 503 | `busy` | Concurrent transaction collision; client automatically retries |
