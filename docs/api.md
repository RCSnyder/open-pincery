# Open Pincery HTTP API Contract (v4)

This document defines the supported HTTP surface for v4 and the compatibility promise into v5.

## Stability Statement (v4 -> v5)

Open Pincery treats the endpoints in this document as the stable v4 public contract.

For v5, compatibility means:

- Existing paths and methods remain available.
- Existing required request fields remain required with the same meaning.
- Existing response fields are not removed or renamed.
- Additive changes are allowed (new optional fields, new endpoints, new query params).

Breaking changes require a new versioned contract section and migration notes.

## Base URL

Examples assume `http://localhost:8080`.

## Authentication Models

There are three auth patterns:

- Bootstrap auth: `Authorization: Bearer <bootstrap_token>` for `POST /api/bootstrap` and `POST /api/login`.
- Session auth: `Authorization: Bearer <session_token>` for all authenticated `/api/agents/*` routes.
- Webhook signature auth: unauthenticated route with `X-Webhook-Signature` HMAC header for webhook ingress.

Rate limiting is enforced at middleware level:

- Unauthenticated routes: 10 requests/minute per client IP.
- Authenticated routes: 60 requests/minute per client IP.

## Common Error Shape

Most application errors return JSON:

```json
{ "error": "message" }
```

HTTP status code semantics:

- `400` bad request
- `401` unauthorized
- `403` forbidden (valid auth, no workspace membership)
- `404` not found
- `409` conflict
- `429` too many requests (plain text body from rate limiter)
- `500` internal error
- `503` readiness failure

## Endpoints

### Health and Readiness

#### `GET /health`

- Auth: none
- Description: liveness probe; returns 200 if the process is serving HTTP.
- Response 200:

```json
{ "status": "ok" }
```

#### `GET /ready`

- Auth: none
- Description: readiness probe; requires DB connectivity, expected migrations, and alive background tasks.
- Response 200:

```json
{ "status": "ready" }
```

- Response 503 example:

```json
{ "status": "not_ready", "failing": "database" }
```

Other `failing` values include `migrations`, `background_tasks`, `background_task:listener`, and `background_task:stale_recovery`.

### Bootstrap

#### `POST /api/bootstrap`

- Auth: bootstrap token (`Authorization: Bearer <bootstrap_token>`)
- Description: one-time initialization of default admin/org/workspace and session token issuance.
- Request body: none required.
- Response 201:

```json
{
  "user_id": "uuid",
  "organization_id": "uuid",
  "workspace_id": "uuid",
  "session_token": "token"
}
```

- Errors:
- `401` missing/invalid bootstrap token
- `409` already bootstrapped — use `POST /api/login` to get a new session token

### Login

#### `POST /api/login`

- Auth: bootstrap token (`Authorization: Bearer <bootstrap_token>`)
- Description: issue a new session token for the admin user. Use when the system is already bootstrapped and you need a fresh session (e.g., lost token, expired session).
- Request body: none.
- Response 200:

```json
{
  "user_id": "uuid",
  "session_token": "token"
}
```

- Errors:
- `401` missing/invalid bootstrap token
- `400` system not yet bootstrapped

### Agents

#### `POST /api/agents`

- Auth: session token
- Request body:

```json
{ "name": "agent-name" }
```

- Response 201:

```json
{
  "id": "uuid",
  "name": "agent-name",
  "status": "asleep",
  "is_enabled": true,
  "disabled_reason": null,
  "webhook_secret": "secret",
  "identity": null,
  "work_list": null,
  "budget_limit_usd": "10.0",
  "budget_used_usd": "0",
  "created_at": "timestamp"
}
```

#### `GET /api/agents`

- Auth: session token
- Response headers: `Accept: application/json`
- Response 200: array of agents for caller workspace.
- Response body:

```json
[
  {
    "id": "uuid",
    "name": "agent-name",
    "status": "asleep",
    "is_enabled": true,
    "disabled_reason": null,
    "identity": null,
    "work_list": null,
    "budget_limit_usd": "10.0",
    "budget_used_usd": "0",
    "created_at": "timestamp"
  }
]
```

- Note: `webhook_secret` is omitted in list responses.
- Errors:
- `401` missing/invalid session token
- `403` authenticated caller has no active workspace membership

#### `GET /api/agents/{id}`

- Auth: session token
- Response headers: `Accept: application/json`
- Response 200: single agent with latest projection fields when available.
- Response body:

```json
{
  "id": "uuid",
  "name": "agent-name",
  "status": "asleep",
  "is_enabled": true,
  "disabled_reason": null,
  "identity": "projection text",
  "work_list": "projection text",
  "budget_limit_usd": "10.0",
  "budget_used_usd": "0.0025",
  "created_at": "timestamp"
}
```

- Errors:
- `401` missing/invalid session token
- `403` agent exists outside caller workspace
- `404` agent not found

#### `PATCH /api/agents/{id}`

- Auth: session token
- Request headers:
- `Content-Type: application/json`
- `Accept: application/json`
- Request body (all fields optional):

```json
{
  "name": "new-name",
  "is_enabled": false,
  "budget_limit_usd": "12.50"
}
```

- Response 200: updated agent.
- Response body:

```json
{
  "id": "uuid",
  "name": "new-name",
  "status": "asleep",
  "is_enabled": false,
  "disabled_reason": "disabled_by_user",
  "identity": null,
  "work_list": null,
  "budget_limit_usd": "12.50",
  "budget_used_usd": "0",
  "created_at": "timestamp"
}
```

- Errors:
- `401` missing/invalid session token
- `403` agent exists outside caller workspace
- `404` agent not found

#### `DELETE /api/agents/{id}`

- Auth: session token
- Request headers:
- `Accept: application/json`
- Description: soft delete (disables agent).
- Response 200: updated agent record.
- Response body:

```json
{
  "id": "uuid",
  "name": "agent-name",
  "status": "asleep",
  "is_enabled": false,
  "disabled_reason": "deleted",
  "identity": null,
  "work_list": null,
  "budget_limit_usd": "10.0",
  "budget_used_usd": "0",
  "created_at": "timestamp"
}
```

- Errors:
- `401` missing/invalid session token
- `403` agent exists outside caller workspace
- `404` agent not found

#### `POST /api/agents/{id}/webhook/rotate`

- Auth: session token
- Description: rotate stored webhook secret and append a `webhook_secret_rotated` audit event.
- Response 200:

```json
{ "webhook_secret": "new-secret" }
```

### Messages and Events

#### `POST /api/agents/{id}/messages`

- Auth: session token
- Request body:

```json
{ "content": "hello" }
```

- Response 202:

```json
{ "event_id": "uuid" }
```

- Side effect: appends `message_received` event and emits Postgres `NOTIFY` to wake listener.

#### `GET /api/agents/{id}/events`

- Auth: session token
- Query params:
- `limit` (optional integer, default `100`, max `1000`)
- `since` (optional event UUID; returns events created strictly after this event)
- Response 200:

```json
{
  "events": [
    {
      "id": "uuid",
      "agent_id": "uuid",
      "event_type": "message_received",
      "source": "human",
      "wake_id": null,
      "tool_name": null,
      "tool_input": null,
      "tool_output": null,
      "content": "hello",
      "termination_reason": null,
      "created_at": "timestamp"
    }
  ],
  "total": 1
}
```

### Webhook Ingress

#### `POST /api/agents/{id}/webhooks`

- Auth: HMAC signature header (no bearer session required)
- Required header: `X-Webhook-Signature` (`sha256=<hex>` or raw hex)
- Optional header: `X-Idempotency-Key`
- Request body:

```json
{
  "content": "payload text",
  "source": "optional-source"
}
```

- Response 202:

```json
{ "status": "accepted" }
```

- Duplicate idempotency key response 200:

```json
{ "status": "duplicate" }
```

- Errors:
- `401` missing/invalid signature
- `403` agent disabled
- `404` unknown agent

## Client Coverage Matrix

The following call sites are covered by the endpoints above:

| Caller                              | Endpoints used                                                                                                                                                         |
| ----------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/api_client.rs` (CLI transport) | `/api/bootstrap`, `/api/login`, `/api/agents`, `/api/agents/{id}`, `/api/agents/{id}/webhook/rotate`, `/api/agents/{id}/messages`, `/api/agents/{id}/events`, `/ready` |
| `static/js/api.js` (UI transport)   | `/api/bootstrap`, `/health`, `/ready`, `/api/agents`, `/api/agents/{id}`, `/api/agents/{id}/webhook/rotate`, `/api/agents/{id}/messages`, `/api/agents/{id}/events`    |

## Route Registration Source of Truth

All documented application routes are registered from `src/api/mod.rs` via:

- health/readiness direct routes
- merged routers (`agents`, `messages`, `events`, `bootstrap`, `webhooks`)
- `/api` nesting for authenticated and unauthenticated groups
