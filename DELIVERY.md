# DELIVERY.md — Open Pincery v3

## What Was Built

A multi-agent platform runtime implementing the Open Pincery architecture: event-sourced agents with CAS lifecycle management, LLM-powered wake/sleep cycles, maintenance projections, HTTP API, graceful shutdown, Docker Compose deployment, API rate limiting, webhook ingress, agent management, structured JSON logging, Prometheus metrics, health/readiness split, CI pipeline, signed release artifacts with SBOMs, and operator runbooks. Single-binary Rust server backed by PostgreSQL.

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

## v3 Changes (from v2)

- AC-16: CI workflow (`.github/workflows/ci.yml`) running fmt + clippy + tests (against Postgres 16 service container) + `cargo deny check` on every push/PR. `deny.toml` enforces license allow-list and denies unknown registries/git sources.
- AC-17: Structured JSON logging. Set `LOG_FORMAT=json` to emit one JSON object per line (`timestamp`, `level`, `target`, `fields.message` + span context) for log pipelines; unset for human-readable output.
- AC-18: Prometheus `/metrics` endpoint (opt-in via `METRICS_ADDR`, served on its own port). Eight counters (`open_pincery_wake_started_total`, `open_pincery_wake_completed_total{reason}`, `open_pincery_llm_call_total`, `open_pincery_llm_prompt_tokens_total`, `open_pincery_llm_completion_tokens_total`, `open_pincery_tool_call_total`, `open_pincery_webhook_received_total`, `open_pincery_rate_limit_rejected_total`) + `open_pincery_active_wakes` gauge + `open_pincery_wake_duration_seconds` histogram.
- AC-19: Split `/health` (liveness — always 200 while the process serves HTTP) from `/ready` (readiness — 200 only when DB reachable AND all migrations applied AND both background tasks alive). 503 responses name the failing subsystem in a `failing` field.
- AC-20: Tag-triggered signed release workflow (`.github/workflows/release.yml`). Builds `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` binaries with LTO + strip + codegen-units=1 (`[profile.release]` in `Cargo.toml`), generates CycloneDX SBOM, signs binary + SBOM with cosign keyless (GitHub OIDC), publishes via `softprops/action-gh-release`. Prerelease auto-detected from `-rc/-beta/-alpha` tag suffixes.
- AC-21: Five operator runbooks under `docs/runbooks/` — stale wake triage, DB restore, migration rollback, rate-limit tuning, webhook debugging. Each includes Symptom / Diagnostic Commands / Remediation / Escalation sections with concrete copy-paste commands.

- **No sandboxing**: Shell tool runs with host privileges (future: Zerobox container isolation)
- **No inter-agent messaging**: Single-agent operation only
- **Single workspace**: Multi-tenancy schema exists but not enforced in API authorization
- **No UI beyond status page**: API-first interface
- **Webhook secrets**: Only visible on agent creation — if lost, requires database access to retrieve
- **Rate limiting is in-process**: Not shared across multiple server instances
- **Metrics recorder is process-global**: Only one Prometheus recorder per process; unit tests that install a recorder must run single-threaded.
- **Release workflow not yet exercised**: `cosign verify-blob` against a real tagged artifact will happen on first `v*` tag push.
- **Dockerfile runs as root**: No `USER` directive yet; acceptable for self-host single-operator deployments.

- **Runtime**: Single Rust binary (~15MB release), or Docker image
- **Database**: PostgreSQL 16 (16 migration files)
- **External**: One OpenAI-compatible LLM API
- **Cost**: PostgreSQL hosting + LLM API usage. No other infrastructure costs.
