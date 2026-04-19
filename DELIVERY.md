# DELIVERY.md — Open Pincery v2

## What Was Built

A multi-agent platform runtime implementing the Open Pincery architecture: event-sourced agents with CAS lifecycle management, LLM-powered wake/sleep cycles, maintenance projections, HTTP API, graceful shutdown, Docker Compose deployment, API rate limiting, webhook ingress, and agent management. Single-binary Rust server backed by PostgreSQL.

## How to Use It

### Docker Compose (recommended)

1. `cp .env.example .env` — configure (set `LLM_API_KEY`, `OPEN_PINCERY_BOOTSTRAP_TOKEN`)
2. `docker compose up -d` — starts both the app and PostgreSQL
3. `POST /api/bootstrap` with bearer token → get session token
4. `POST /api/agents` → create agents (response includes `webhook_secret`)
5. `POST /api/agents/:id/messages` → send messages (triggers wake cycle)

### From Source

1. `docker compose up -d db` — start PostgreSQL only
2. `cp .env.example .env` — configure
3. `cargo build --release && source .env && ./target/release/open-pincery`

## What It Does

- **Agent lifecycle**: Agents transition `asleep → awake → maintenance → asleep` via atomic CAS operations
- **Wake loop**: On message, agent wakes, calls LLM iteratively with tools (shell, plan, sleep), records all events
- **Maintenance**: After each wake, LLM updates agent identity, work list, and summary
- **Drain check**: If new messages arrive during wake, agent re-wakes instead of sleeping
- **Stale recovery**: Background job detects agents stuck awake and force-releases them
- **Event log**: Append-only, ordered, complete history of every agent action
- **Projections**: Versioned, immutable snapshots of agent state after each wake
- **Graceful shutdown**: SIGTERM/Ctrl-C stops accepting connections, waits up to 30s for in-flight wakes and background tasks to complete
- **Rate limiting**: Per-IP rate limits — 10 req/min unauthenticated, 60 req/min authenticated. Returns 429 with `Retry-After` header
- **Webhook ingress**: External systems post events via HMAC-SHA256-signed webhooks with idempotency deduplication
- **Agent management**: PATCH to rename or enable/disable agents; DELETE to soft-delete (sets `is_enabled=false, disabled_reason='deleted'`; disabled agents cannot wake)
- **Docker deployment**: Multi-stage Dockerfile with health check, docker-compose.yml with app + postgres

## v2 Changes (from v1)

- AC-11: Graceful shutdown via CancellationToken + `with_graceful_shutdown`
- AC-12: Docker Compose one-command start (`docker compose up -d`)
- AC-13: Per-IP rate limiting using `governor` crate
- AC-14: Webhook ingress with HMAC-SHA256 verification and idempotency dedup
- AC-15: PATCH/DELETE agent management endpoints

## Known Limitations

- **No sandboxing**: Shell tool runs with host privileges (Phase 3: Zerobox container isolation)
- **No inter-agent messaging**: Single-agent operation only (Phase 3)
- **Single workspace**: Multi-tenancy schema exists but not enforced in API authorization
- **No UI beyond health page**: API-first interface with minimal status page
- **Webhook secrets**: Only visible on agent creation — if lost, requires database access to retrieve
- **Rate limiting is in-process**: Not shared across multiple server instances
- **cargo-audit**: Dependency vulnerability scan deferred

## Infrastructure

- **Runtime**: Single Rust binary (~15MB release), or Docker image
- **Database**: PostgreSQL 16 (16 migration files)
- **External**: One OpenAI-compatible LLM API
- **Cost**: PostgreSQL hosting + LLM API usage. No other infrastructure costs.
