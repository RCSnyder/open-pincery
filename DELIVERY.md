# DELIVERY.md — Open Pincery v4

## What Was Built

A multi-agent platform runtime implementing the Open Pincery architecture: event-sourced agents with CAS lifecycle management, LLM-powered wake/sleep cycles, maintenance projections, HTTP API, graceful shutdown, Docker Compose deployment, API rate limiting, webhook ingress, agent management, structured JSON logging, Prometheus metrics, health/readiness split, CI pipeline, signed release artifacts with SBOMs, and operator runbooks. v4 adds self-host hardening: non-root container user, runtime budget-cap enforcement with transactional cost accounting, authenticated webhook-secret rotation, a `pcy` CLI binary, a vanilla-JS control plane UI, and a published v4 API stability contract. Single-binary Rust server backed by PostgreSQL.

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

## v4 Changes (from v3)

- AC-22: Dockerfile runtime stage now creates a dedicated `pcy` system user (UID 10001) and drops to it via `USER pcy`; all runtime `COPY` directives use `--chown=pcy:pcy`. Verified by a static guard test (`tests/dockerfile_nonroot_test.rs`).
- AC-23: Runtime LLM budget cap enforced before CAS wake acquire. `background::listener` checks `agents.budget_used_usd` against `agents.budget_limit_usd` and appends a `budget_exceeded` event instead of waking when the cap is reached. Cost accounting is now real end-to-end: `LlmClient` carries a `Pricing` struct for primary and maintenance calls, `wake_loop`/`maintenance` compute `cost_usd = llm.estimate_cost(usage, is_maintenance)`, and `insert_llm_call` increments `agents.budget_used_usd` in the same transaction as the `llm_calls` insert. Configured via `LLM_PRICE_INPUT_PER_MTOK` / `LLM_PRICE_OUTPUT_PER_MTOK` / `LLM_MAINTENANCE_PRICE_INPUT_PER_MTOK` / `LLM_MAINTENANCE_PRICE_OUTPUT_PER_MTOK` (defaults 3.0 / 15.0 / 3.0 / 15.0 USD per million tokens).
- AC-24: Authenticated webhook-secret rotation endpoint `POST /api/agents/:id/rotate-webhook-secret`. Workspace-scoped via `scoped_agent`, returns the new secret exactly once, appends a `webhook_secret_rotated` event (no secret material in payload), and rotates the agent row atomically in the same transaction.
- AC-25: `pcy` CLI binary (`[[bin]] pcy` in `Cargo.toml`) with subcommands `bootstrap`, `login`, `agent` (`create`/`list`/`show`/`disable`/`rotate-secret`), `message`, `events`, `budget` (`show`/`set`/`reset`), and `status`. Thin shim at `src/bin/pcy.rs`; shared HTTP client at `src/api_client.rs`.
- AC-26: Vanilla-JS ES-module control plane UI served at `/` from `static/`. Split across `static/js/{app,api,state,ui}.js` plus `static/js/views/{login,agents,detail,settings}.js`; no bundler, no CDN, no single file exceeds 132 lines. Covers login, agent list, agent detail with long-poll event stream, and settings including secret rotation.
- AC-27: `docs/api.md` publishes the v4 HTTP surface as the stable contract, documents the three auth models (bootstrap token, session token, webhook HMAC), the common error shape, every endpoint with request/response examples, and the client coverage matrix against the `pcy` CLI and the static UI.

## Known Limitations

- **No sandboxing**: Shell tool runs with host privileges (future: Zerobox container isolation)
- **No inter-agent messaging**: Single-agent operation only
- **Single workspace enforcement**: Multi-tenancy schema exists; `scoped_agent` enforces workspace isolation on agent-level handlers, but cross-workspace administration is not yet exposed via API
- **Webhook secrets**: Still surfaced exactly once — on creation or after `POST /api/agents/:id/rotate-webhook-secret`. Operators must capture the response immediately.
- **Rate limiting is in-process**: Not shared across multiple server instances
- **Metrics recorder is process-global**: Only one Prometheus recorder per process; unit tests that install a recorder must run single-threaded.
- **Release workflow not yet exercised**: `cosign verify-blob` against a real tagged artifact will happen on first `v*` tag push.
- **RUSTSEC-2023-0071** (medium, CVSS 5.9): `rsa 0.9.10` pulled in transitively via unused `sqlx-mysql`. Not exploitable in this codebase (MySQL driver is never loaded). No upstream fix yet; acceptable per build gate (no high/critical).

- **Runtime**: Single Rust binary (~15MB release), or Docker image
- **Database**: PostgreSQL 16 (16 migration files)
- **External**: One OpenAI-compatible LLM API
- **Cost**: PostgreSQL hosting + LLM API usage. No other infrastructure costs.
