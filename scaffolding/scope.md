# Scope — Open Pincery v1

## Problem

There is no open-source platform that treats AI agents as continuous, durable entities with event-sourced memory, CAS-protected lifecycle, and wake/sleep cycles. Existing frameworks treat agents as ephemeral function calls that vanish between requests. Open Pincery implements the Continuous Agent Architecture: agents persist indefinitely with stable identity, append-only event logs, and self-configuration through conversation.

## Smallest Useful Version

A single Rust binary + PostgreSQL that implements the core agent runtime for `self_host_individual` mode. A user can: bootstrap a local admin, create an agent via HTTP API, send it a message, watch the agent wake via CAS, reason with an LLM, execute shell commands, update its own identity and work list through maintenance, and go back to sleep — with every action recorded in an append-only event log. The agent is a continuous entity that remembers across wakes.

This is the foundation that every other feature (webhooks, multi-tenancy, approval workflows, credential vaults, MCP, dashboard) builds on top of. Without this working end-to-end, nothing else matters.

## Acceptance Criteria

- **AC-1** (CAS Lifecycle): An agent transitions through Resting → WakeAcquiring → Awake → WakeEnding → Maintenance → Resting using `UPDATE ... WHERE status = $expected RETURNING *` in PostgreSQL. Concurrent wake attempts on the same agent are rejected; only one wake is active at any time. Verified by a test that attempts two simultaneous wake acquisitions and confirms exactly one succeeds.

- **AC-2** (Event Log): All agent activity (messages received, tool calls, tool results, wake start, wake end, plans, messages sent) is appended to an immutable event log in PostgreSQL. Events are never updated or deleted. Event ordering is preserved by timestamp. Verified by sending a message, letting the agent wake and act, then querying the event log and confirming the complete sequence.

- **AC-3** (Prompt Assembly): The system assembles a bounded prompt from: (1) constitution/system prompt from the active prompt template, (2) current UTC time, (3) up to 20 most recent wake summaries (each ≤500 chars), (4) current identity projection, (5) current work list projection, (6) most recent 200 events converted to chat messages. Character-based trim drops oldest messages first. Verified by creating an agent with known projections and events, assembling the prompt, and confirming all components are present and correctly ordered.

- **AC-4** (Wake Loop): The agent reasons via an OpenAI-compatible chat completions API. When the LLM returns tool_calls, the runtime dispatches them (shell, plan, sleep) and feeds results back. When the LLM returns text, the agent implicitly sleeps. The wake loop respects an iteration cap (default 50); exceeding it terminates the wake with `iteration_cap` reason. Verified by an integration test where an agent receives a message, makes at least one tool call, and completes a wake cycle end-to-end.

- **AC-5** (Maintenance Cycle): After each wake ends, a single LLM call receives the previous identity, work list, wake transcript, and termination reason, and returns updated identity (prose), updated work list (prose), and a wake summary (≤500 chars). These are written as new versioned rows in PostgreSQL, never updating in place. Verified by checking that after a wake, new projection rows and a wake summary exist with correct content.

- **AC-6** (HTTP API): The runtime exposes REST endpoints via axum: `POST /api/agents` (create), `GET /api/agents` (list), `GET /api/agents/:id` (detail with current projections), `POST /api/agents/:id/messages` (send message), `GET /api/agents/:id/events` (event log). All endpoints return JSON. Verified by exercising each endpoint and confirming correct status codes and response shapes.

- **AC-7** (Wake Triggers): When a message is inserted into the event log, the runtime issues `NOTIFY agent_<id>`. A background listener receives notifications and triggers wake acquisition. Verified by sending a message to a resting agent and confirming it wakes within 5 seconds without polling.

- **AC-8** (Stale Wake Recovery): A background job runs periodically and detects agents whose `wake_started_at` is older than 2 hours while still in `awake` or `maintenance` status. It force-releases them to `asleep` and records a `stale_wake_recovery` event. Verified by setting an agent to `awake` with a stale timestamp and confirming recovery occurs.

- **AC-9** (Drain Check): After maintenance completes, the system checks for `message_received` events newer than the wake's high-water mark. If found, it immediately acquires a new wake via CAS (maintenance → awake) without returning to rest. If none, it transitions to asleep. Verified by sending a message during an active wake and confirming the drain check triggers a follow-up wake.

- **AC-10** (Local Admin Bootstrap): On first run with an empty database, the system runs migrations, creates a bootstrap local_admin user, a default organization, and a default workspace. The bootstrap is gated by an install-time token passed via environment variable. Verified by starting with an empty database and confirming the bootstrap completes with correct rows.

## Stack

| Concern       | Choice                       | Source                         |
| ------------- | ---------------------------- | ------------------------------ |
| Runtime       | Rust                         | preferences.md                 |
| Database      | PostgreSQL                   | preferences.md                 |
| HTTP/API      | axum                         | preferences.md                 |
| Async         | tokio                        | preferences.md                 |
| SQL           | sqlx (compile-time checked)  | preferences.md                 |
| Serialization | serde + serde_json           | preferences.md                 |
| HTTP client   | reqwest                      | preferences.md (LLM API calls) |
| Logging       | tracing + tracing-subscriber | Standard Rust ecosystem        |

## Deployment Target

`self_host_individual` — a single Rust binary + PostgreSQL instance running on the user's machine or server. No cloud services, no containers required (though Docker Compose will be provided for convenience).

## Data Model

### Core tables (from TLA+ spec)

- `users` — authenticated humans (local_admin for bootstrap)
- `organizations` — tenant containers
- `workspaces` — agent grouping within orgs
- `organization_memberships` — user ↔ org roles
- `workspace_memberships` — user ↔ workspace roles
- `agents` — the continuous entities (status, wake_id, wake_started_at, iteration_count, owner_id, workspace_id, permission_mode, is_enabled, budget columns)
- `events` — append-only event log (agent_id, event_type, source, wake_id, tool_name, tool_input, tool_output, content, created_at)
- `agent_projections` — versioned identity + work list snapshots
- `wake_summaries` — compressed long-term memory per wake
- `prompt_templates` — versioned, immutable prompt templates with one-active-per-name constraint
- `llm_calls` — full LLM call provenance (model, tokens, cost, latency, prompt_hash, response_hash)
- `llm_call_prompts` — optional full prompt storage for reconstruction
- `tool_audit` — detailed tool execution records
- `user_sessions` — hashed session tokens
- `auth_audit` — authentication events

### Persistence

PostgreSQL is the single source of truth. All state is derived from the event log plus CAS-protected status columns. Append-only for events; versioned rows for projections.

## Estimated Cost

$0 — runs on localhost with a local PostgreSQL instance. No cloud services required. LLM API costs are usage-dependent and paid directly to the model provider (OpenRouter, OpenAI, etc.).

## Quality Tier

**Skyscraper** — TLA+ formal specification exists, event-sourced architecture, CAS concurrency control, multi-user system, security-critical (agents execute code), append-only audit trail. This demands: full test coverage (unit + integration + e2e), CI/CD, formal spec compliance, LLM observability, comprehensive audit schema, SBOM, and operational documentation.

## Clarifications Needed

1. **LLM provider default**: The spec says "OpenRouter/OpenAI compatible." Using a generic OpenAI-compatible client with base URL + API key configured via environment. Assuming this is correct.
2. **Zerobox availability**: Zerobox is listed as the sandbox but is a relatively new project. For v1, shell tool execution will use basic subprocess isolation (no filesystem/network restrictions). Zerobox integration is Phase 2 per security-architecture.md's own phasing.
3. **Constitution content**: The TLA+ spec describes what the constitution contains but no literal text. v1 will ship a default constitution template that covers the documented requirements.

## Deferred

- **Inter-agent messaging** (send_message, cross-log recording, NOTIFY target) — Phase 3
- **Credential vault** (OneCLI integration, proxy injection, Zerobox secrets) — Phase 3
- **Process sandboxing** (Zerobox per-tool isolation) — Phase 3
- **Budget enforcement** (per-agent USD limits, hard caps) — v2 tracks costs but does not enforce hard budget caps
- **Event collapse** (backpressure for burst events) — Phase 3
- **Prompt injection defense** (scanning, canary tokens, rail pipeline) — Phase 3
- **MCP server support** (discovery, registry, agent-built servers) — Phase 3
- **Multi-tenancy** (org/workspace RBAC enforcement, policy sets, RLS) — v2 creates the schema but does not enforce RLS
- **Greywall host sandbox** — Phase 3
- **Enterprise auth** (Entra OIDC, generic OIDC, SCIM) — Phase 3
- **SaaS features** (billing, subscriptions, abuse prevention) — Phase 3+
- **Compile/lint/test/typecheck verification tools** — Phase 3
- **Context character cap enforcement** — Phase 3 (iteration cap is v1)

---

## v2 — Operational Readiness

### Problem (v2)

v1 delivered the core agent runtime but is not operationally ready for real self-hosted use. There is no graceful shutdown, no rate limiting, no way to run the full stack with `docker compose up`, and webhook-driven integrations are impossible. The UI exists but was not part of v1's acceptance criteria.

### Changes from v1

- Minor architecture changes: new middleware layer (rate limiting), shutdown signal handling, webhook endpoint, Dockerfile
- No changes to existing core runtime (CAS lifecycle, wake loop, maintenance, drain check, event sourcing)
- Existing v1 ACs remain satisfied; v2 appends new ACs

### v2 Acceptance Criteria

- **AC-11** (Graceful Shutdown): On SIGTERM/SIGINT, the server stops accepting new requests, waits up to 30 seconds for in-flight requests and active wake loops to complete, then exits cleanly. Background listener and stale recovery tasks are cancelled gracefully. Verified by starting the server, sending a message that triggers a wake, then sending SIGTERM and confirming the wake completes and the process exits with code 0.

- **AC-12** (Docker Compose Full Stack): `docker compose up` starts both PostgreSQL and the Open Pincery binary from a multi-stage Dockerfile. The app waits for Postgres to be ready before starting. Verified by running `docker compose up` from a clean state and confirming the health endpoint returns `{"status":"ok"}` within 60 seconds.

- **AC-13** (API Rate Limiting): All API endpoints enforce per-IP rate limiting (default: 60 requests/minute for authenticated endpoints, 10 requests/minute for bootstrap). When the limit is exceeded, the server returns HTTP 429 with a `Retry-After` header. Verified by sending 61 requests in rapid succession and confirming the 61st returns 429.

- **AC-14** (Webhook Ingress): `POST /api/agents/:id/webhooks` accepts JSON payloads with HMAC-SHA256 signature verification via a per-agent webhook secret. Valid webhooks are recorded as `webhook_received` events and trigger wake acquisition via NOTIFY. Invalid signatures return 401. Duplicate webhooks (by idempotency key header) return 200 without re-processing. Verified by sending a signed webhook, confirming the event appears in the log, and sending the same webhook again confirming deduplication.

- **AC-15** (Agent Management API): `PATCH /api/agents/:id` supports enabling/disabling agents and updating the name. `DELETE /api/agents/:id` soft-deletes an agent (sets `is_enabled = false`, `disabled_reason = 'deleted'`). Disabled agents reject wake acquisition attempts. Verified by disabling an agent, sending it a message, and confirming no wake occurs.

### v2 Deployment Target

Same as v1: `self_host_individual` — but now with Docker Compose as a first-class deployment method alongside bare-metal.

### v2 Estimated Cost

$0 — same as v1. Docker is optional; bare-metal still works.

### v2 Deferred (from this iteration)

- Webhook UI management (creating/rotating secrets via UI) — v3
- Rate limit configuration per-workspace — v3
- Health check page in UI — nice-to-have, not critical

---

## v3 — Operational Observability & Release Hygiene

### Problem (v3)

v1 and v2 delivered a working runtime with deployment scaffolding, but the project still lacks the operational foundation required by its own declared **skyscraper** quality tier: no CI, no metrics, no structured machine-readable logs, no signed release artifacts, no SBOM, no split liveness/readiness checks, and no operator runbooks. Without these, we cannot safely add sandboxing (v4+) or multi-instance coordination — you can't operate what you can't observe.

v3 closes the skyscraper-tier operational gate and unblocks every later iteration. Scope is intentionally narrow: observability and release hygiene only, no new runtime features.

### Changes from v2

- Additive only: new observability module, new optional metrics listener, new logging format toggle, new CI workflow, new release workflow, new runbooks
- No changes to core runtime, API semantics, database schema, or existing ACs
- New optional dependencies gated by features/env vars; default binary size and behaviour unchanged

### v3 Acceptance Criteria

- **AC-16** (CI Pipeline): `.github/workflows/ci.yml` triggers on push and pull request. Runs, in order: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (against a PostgreSQL 16 service container with `TEST_DATABASE_URL` set), and `cargo deny check` (advisories + licenses + bans + sources). Any step failing fails the workflow. Verified by a green run on the branch that introduces the workflow, with the job log showing all four steps passing.

- **AC-17** (Structured JSON Logging): When the environment variable `LOG_FORMAT=json` is set, `tracing-subscriber` emits one JSON object per log line with fields `timestamp`, `level`, `target`, `message`, plus any span context. When `LOG_FORMAT` is unset or any other value, output remains human-readable (current behaviour). Verified by starting the server with `LOG_FORMAT=json`, triggering a wake via a message, and confirming every stdout line parses as valid JSON and contains the expected top-level fields.

- **AC-18** (Prometheus Metrics Endpoint): When the environment variable `METRICS_ADDR` is set (for example `127.0.0.1:9090`), a dedicated HTTP listener binds to that address and serves `GET /metrics` in Prometheus text format. The main API port never serves `/metrics`. Metrics exposed: counters for wake starts, wake completions by termination reason, LLM calls, LLM prompt and completion tokens, tool executions, webhook receipts, and rate-limit rejections; a gauge for active wake count; a histogram for wake duration in seconds. When `METRICS_ADDR` is unset, no metrics listener is started. Verified by starting with `METRICS_ADDR=127.0.0.1:9090`, triggering at least one wake, scraping `/metrics`, and confirming non-zero counter values and well-formed Prometheus output.

- **AC-19** (Health / Readiness Split): `GET /health` returns `200 {"status":"ok"}` whenever the HTTP server is running (liveness only — no dependency checks). `GET /ready` returns `200 {"status":"ready"}` only when: the database pool can execute `SELECT 1`, all expected migrations are applied, and background tasks (listener, stale recovery) are running; otherwise returns `503` with a JSON body naming the failing check. Verified by stopping the PostgreSQL container mid-run and confirming `/health` continues to return 200 while `/ready` returns 503 with `database` listed as the failing check.

- **AC-20** (Signed Release Artifacts with SBOM): `.github/workflows/release.yml` triggers on tags matching `v*`. It builds release binaries for `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` using a release profile defined in `.cargo/config.toml` (LTO enabled, symbols stripped). It generates a CycloneDX SBOM via `cargo cyclonedx`. It signs each binary and the SBOM with cosign keyless signing backed by GitHub Actions OIDC. It publishes binaries, SHA-256 checksums, cosign signatures and certificates, and the SBOM to the GitHub Release for that tag. Verified by pushing tag `v0.3.0-rc1`, downloading the published artifacts, and running `cosign verify-blob` successfully against at least one artifact.

- **AC-21** (Operator Runbooks): `docs/runbooks/` contains at least five runbooks: `stale-wake-triage.md`, `db-restore.md`, `migration-rollback.md`, `rate-limit-tuning.md`, and `webhook-debugging.md`. Each runbook contains the four sections: **Symptom**, **Diagnostic Commands**, **Remediation**, **Escalation**. Diagnostic commands must be concrete shell commands an operator can copy-paste, not prose. Verified by the REVIEW agent confirming all five files exist, each has all four sections, and each diagnostic command is a real executable invocation.

### v3 Deployment Target

Same as v1/v2: `self_host_individual`. Observability features are opt-in; a self-hoster who sets no observability env vars gets exactly the same behaviour as v2.

### v3 Estimated Cost

$0 — all tooling is free and OSS. Metrics scraping and log aggregation, if the operator runs them, use their existing infrastructure.

### v3 Quality Tier

Still skyscraper. v3 closes the CI / SBOM / observability / runbook gaps that were always required by that tier but were carried forward from v1 and v2 as implicit debt.

### v3 Clarifications Needed

None. All six ACs have unambiguous pass/fail criteria and do not depend on external business decisions.

### v3 Deferred (from this iteration)

- OpenTelemetry distributed tracing (OTLP export) — v4+. Rationale: the self-host target does not justify the binary size and dependency cost until a real operator requests it.
- Log aggregation stack (Loki / Grafana deployment manifests) — v4+. Runbooks will describe the pattern; we will not ship the stack.
- Prometheus alerting rules and Grafana dashboards as code — v4+. Example queries will live in the metrics runbook.
- Multi-instance coordination and leader election for background jobs — v4+.
- Performance baselining and load-test harness — separate iteration.
- Binary signing with non-keyless cosign (hardware-token backed) — future enterprise release line.

---

## v4 — Usable Self-Host (CLI + Minimal UI + Safety Hardening)

### Problem (v4)

v3 closed the operational gate, but the runtime is still not actually usable by an individual operator: there is no CLI to drive the API, the UI is a placeholder static page, and three v3-known limitations (root container, unenforced budgets, no webhook rotation) keep blocking the path to a safely-hostable system.

The vision is two deployment modes worth shipping: `self_host_individual` (now) and `saas_managed` (eventually). v4 makes `self_host_individual` real, locks in an HTTP API contract that downstream consumers (CLI, UI, future WASM rewrite, future SaaS control plane) can depend on, and finishes the safety items v3 deferred. v4 explicitly does **not** attempt: real signup/login (v5), multi-tenant RBAC enforcement (v5), tool sandboxing (v6), or a tagged release (deferred until the product is genuinely usable).

### Changes from v3

- Additive runtime + container changes: Dockerfile non-root user, budget-check in wake acquisition, one new endpoint (`POST /api/agents/:id/webhook/rotate`)
- New deliverable: a `pcy` CLI binary added to the existing Cargo workspace
- New deliverable: real (not placeholder) UI under `static/` — vanilla JS, no build step
- New deliverable: `docs/api.md` API contract document
- No new database tables or columns. The `agents.budget_limit_usd` / `budget_used_usd` columns from v1 are now enforced rather than added.
- No changes to existing AC semantics (AC-1..AC-21 remain satisfied)

### v4 Acceptance Criteria

- **AC-22** (Container runs as non-root): The runtime stage of `Dockerfile` creates a non-root system user `pcy` (UID 10001) and ends with `USER pcy`. The application binary, `/app/migrations`, and `/app/static` are owned by `pcy:pcy`. Inside the running container `id -u` returns `10001`, and a write attempt outside the user-owned tree (e.g. `touch /etc/foo`) fails with permission denied. The existing `/health` HEALTHCHECK continues to succeed (port 8080 is bind-able as non-root). Verified by `docker compose up -d` followed by `docker compose exec app id -u` returning `10001`, `docker compose exec app touch /etc/x` failing with non-zero exit, and `curl http://localhost:8080/health` returning 200.

- **AC-23** (Hard budget enforcement at wake acquisition): Before each wake acquisition, the runtime reads `agents.budget_used_usd` and `agents.budget_limit_usd`. If `budget_limit_usd > 0` and `budget_used_usd >= budget_limit_usd`, wake acquisition is rejected: no LLM call is issued, no `wake_started` event is logged, but exactly one `budget_exceeded` event is appended (`event_type='budget_exceeded'`, `source='runtime'`, JSON payload `{"limit_usd": …, "used_usd": …}`), and the agent's `status` returns to `asleep`. After every successful LLM call, `agents.budget_used_usd` is incremented by the call's `cost_usd` in the same transaction that inserts into `llm_calls`. `budget_limit_usd = 0` means unlimited (escape hatch). Verified by an integration test that sets a test agent's `budget_limit_usd = 0.000001` and `budget_used_usd = 0.000002`, sends a message, and asserts: zero new `llm_calls` rows for that agent, exactly one new `budget_exceeded` event, and `agents.status = 'asleep'` after settle.

- **AC-24** (Webhook secret rotation): `POST /api/agents/:id/webhook/rotate` (session-token authenticated, workspace-scoped exactly like the existing PATCH/DELETE endpoints) returns `200 {"webhook_secret":"<new-base64-32>"}` exactly once. The new secret atomically replaces `agents.webhook_secret`. After rotation, an HMAC computed with the old secret returns `401 Unauthorized` from `POST /api/agents/:id/webhooks`; an HMAC computed with the new secret returns `202 Accepted`. A `webhook_secret_rotated` event is appended (`event_type='webhook_secret_rotated'`, `source='api'`, no secret material in payload). Verified by an integration test that creates an agent, captures the original secret, rotates, then sends two webhooks (one signed with each secret) and asserts the 401/202 split + the audit event.

- **AC-25** (`pcy` CLI binary): A second binary `pcy` is added to the Cargo workspace (built by `cargo build --release` alongside `open-pincery`). Subcommands: `pcy bootstrap` (calls `POST /api/bootstrap`, writes returned token to `~/.config/open-pincery/config.toml`), `pcy login --token <token>` (writes a token to config without bootstrapping), `pcy agent {create,list,show,disable,rotate-secret}`, `pcy message <agent> <text>`, `pcy events <agent> [--tail --since <id>]`, `pcy budget {set,show,reset} <agent> [<usd>]`, `pcy status` (calls `/ready`, exits 0 only if all checks pass). Reads `OPEN_PINCERY_URL` (default `http://localhost:8080`) and the cached token from env or config file; `--url` and `--token` flags override. Verified by an end-to-end shell test (`tests/e2e/cli.sh` or equivalent Rust integration test) that runs `pcy bootstrap` against a live server, creates an agent, sends a message, tails events, rotates the secret, and exits 0 — without any direct `curl` invocation.

- **AC-26** (Minimal control-plane UI): The existing placeholder `static/index.html` is replaced with a real single-page UI in vanilla JavaScript (no framework, no build step). Five views, all served as static files by the existing axum static handler: (1) login (paste session token, persisted to `localStorage`), (2) agent list (calls `GET /api/agents`), (3) agent detail with live event stream via long-poll on `GET /api/agents/:id/events?since=<last_id>` (4 second poll interval, exponential backoff on error), (4) send-message form, (5) settings panel per-agent (rotate webhook secret button, set/show budget). No multi-user features; the UI assumes a single-tenant operator. CSS is a small reset + utility classes only — no design system. Verified by an integration test that boots a live server with a freshly bootstrapped agent, drives the UI through `headless_chrome` or a `curl`+grep equivalent, and asserts: index loads with 200, `/api/agents` is hit on list view, posting the message form results in a `wake_started` event appearing in the stream within 5 seconds, rotate button replaces the secret in `agents.webhook_secret`.

- **AC-27** (HTTP API stability contract): `docs/api.md` documents every public HTTP endpoint that `pcy` and the UI consume, with for each: method + path, required headers (auth, content-type), request body shape (typed fields, required vs optional), response body shape per status code, and any side effects (events appended, status transitions). The document declares the v4 API as **stable through v5** — endpoints may be added but not removed or renamed; field types may not change incompatibly. Verified by REVIEW confirming every endpoint reachable from `src/api/` that the CLI or UI calls is documented, and every documented endpoint exists in `src/api/`.

### v4 Deployment Target

Same as v1/v2/v3: `self_host_individual`. The deliverable for v4 is the source repo + Docker Compose stack + `pcy` binary + browser UI. No tagged release.

### v4 Estimated Cost

$0 — all changes are inside the existing binary, image, and CI surface. No new dependencies of significance (clap for the CLI is the one notable add).

### v4 Quality Tier

Still skyscraper. v4 closes the usability + safety gaps that block individual operator adoption.

### v4 Clarifications Needed

None. Vanilla JS without a type-check step is locked. CLI name `pcy` is locked. AC-27 stability scope (v4 → v5) is locked.

### v4 Deferred (from this iteration)

- **Real signup/login flow** (password hashing, session creation from credentials rather than bootstrap token) — v5
- **Multi-tenant RBAC enforcement on the API** — v5
- **Per-workspace rate limits** — v5
- **Account suspension model** — v5
- **TLA+ enum-name alignment** (runtime currently uses raw status strings; spec requires `Resting`/`WakeAcquiring`/etc.) — v5 RECONCILE work, tracked but out of v4 scope
- **Tool sandboxing (Zerobox or fallback)** — v6
- **Credential vault / proxy injection** — v6
- **GitHub OAuth signup, billing, abuse prevention, ToS/Privacy** — v7 (SaaS productization)
- **First tagged release with cosign-verified artifacts** — deferred until v7 produces a usable end-state product
- **WASM UI rewrite** — possible v7+ if SPA features are actually needed; v4 vanilla JS stays the canonical UI until proven inadequate
- **CLI auth subcommands beyond `bootstrap`/`login`** (e.g. `pcy logout`, `pcy whoami`) — v5 when real auth lands
- **UI styling beyond a minimal reset** — out of scope; intentionally utilitarian

---

## v5 — Operator Onramp

### Problem (v5)

v4 shipped the pieces a self-hoster needs (non-root container, budget cap, webhook rotation, `pcy` CLI, UI, API contract) but the **day-zero path** from "clone the repo" to "first working agent" is still broken. Concrete blockers verified in the v4 artifact:

- `docker-compose.yml` hardcodes `OPEN_PINCERY_BOOTSTRAP_TOKEN: changeme` so the operator's `.env` is ignored and bootstrap returns 401.
- Compose only forwards `LLM_API_BASE_URL` and `LLM_API_KEY` — `LLM_MODEL`, `LLM_MAINTENANCE_MODEL`, all four `LLM_PRICE_*_PER_MTOK` (v4), `LOG_FORMAT`/`METRICS_ADDR` (v3), and `RUST_LOG` are silently dropped.
- `.env.example` is v3-shaped and missing every v4 variable.
- Quick Start in README is v3-era with contradictory Option A/B steps, no `pcy`, no web UI, no Troubleshooting, no reset, no observability pointers, no signed-binary install path (v3 AC-20), and no backup pointer (v3 AC-21 runbook exists but is invisible from the landing page).
- Compose publishes on `0.0.0.0:8080` by default — unacceptable for a platform whose explicit purpose is remote shell execution.
- No Caddy/TLS example exists despite `preferences.md` naming Caddy as the self-host default (gap also called out in `docs/input/self-host-readiness.md` §2).

This iteration is **operator-experience only** — no new runtime features, no interface changes, no new dependencies. Every shipped v1–v4 capability must become reachable via a single linear onramp with test-enforced docs/config consistency.

### Changes from v4

- `docker-compose.yml` env block rewritten to use `${VAR}` interpolation with fail-fast `:?` guards for required secrets and safe `${VAR:-default}` for optional settings
- `.env.example` refreshed with all config-read variables, grouped + commented, OpenRouter default + OpenAI alternative block
- `README.md` Quick Start rewritten: UI-primary → `pcy`-secondary → curl-appendix, plus Troubleshooting, Reset, Backup one-liner, and signed-binary install path referencing v3 AC-20 release artifacts
- New `scripts/smoke.sh` and `scripts/smoke.ps1` proving the onramp end-to-end
- New `docker-compose.caddy.yml` + sample `Caddyfile` as the documented localhost→HTTPS overlay
- New regression tests enforcing compose/env/readme consistency

No changes to core runtime (CAS lifecycle, wake loop, maintenance, drain, event log), API surface, schema, or any existing v1–v4 AC behavior.

### v5 Acceptance Criteria

- **AC-28** (Compose Honors `.env`): `docker-compose.yml` passes every runtime-relevant variable to the `app` container via `${VAR}` interpolation. `OPEN_PINCERY_BOOTSTRAP_TOKEN` and `LLM_API_KEY` are required and compose fails fast with a clear error when unset. `LLM_API_BASE_URL`, `LLM_MODEL`, `LLM_MAINTENANCE_MODEL`, `LLM_PRICE_INPUT_PER_MTOK`, `LLM_PRICE_OUTPUT_PER_MTOK`, `LLM_MAINTENANCE_PRICE_INPUT_PER_MTOK`, `LLM_MAINTENANCE_PRICE_OUTPUT_PER_MTOK`, `LOG_FORMAT`, `METRICS_ADDR`, `RUST_LOG`, `OPEN_PINCERY_HOST`, `OPEN_PINCERY_PORT` all flow through with sensible defaults. Verified by `tests/compose_env_test.rs`: runs `docker compose config` against a fixture `.env`, asserts the rendered app env contains the fixture values, contains no literal `changeme`, and that compose fails fast when a required secret is missing.

- **AC-29** (`.env.example` Is Current): `.env.example` contains every variable read by `src/config.rs`, `src/runtime/llm.rs` pricing, and `src/observability/`. Each entry has an inline comment describing what it does and its default. Includes a commented OpenAI-compatible alternative block alongside the OpenRouter default. Verified by `tests/env_example_test.rs`: parses keys from `.env.example`, scans the source tree for `std::env::var("…")` call sites against an allowlist-of-allowlists, and asserts every read variable is either present in `.env.example` or listed as intentionally-internal in the test.

- **AC-30** (End-to-End Smoke Script): `scripts/smoke.sh` (bash) and `scripts/smoke.ps1` (PowerShell) execute the full onramp against a running compose stack: `docker compose up -d --wait`, poll `/ready` until 200 (max 60s), bootstrap via `pcy`, create an agent, send a message, tail events, assert a `message_received` event exists for the created agent. Exit 0 on success, non-zero with actionable stderr on failure (each failure mode references a Troubleshooting entry by anchor). Verified by a test that runs the bash smoke script against the existing test database stack and asserts exit 0 plus the expected event.

- **AC-31** (README Quick Start Matches Reality): README Quick Start presents three onramps in order of expected use — (1) Web UI at `http://localhost:8080`, (2) `pcy` CLI, (3) curl/HTTP appendix — plus a "From Signed Release Binary" section referencing v3 AC-20 cosign verification, plus Troubleshooting covering: bootstrap 401, 429 rate limit, silent wake (check `LLM_API_KEY` and `docker compose logs -f app`), "already bootstrapped" reset (`docker compose down -v`), `LOG_FORMAT=json` enablement, `METRICS_ADDR` Prometheus scrape example, and a `pg_dump` backup one-liner linking to `docs/runbooks/db-restore.md`. The API table includes `POST /api/agents/:id/rotate-webhook-secret` (v4 AC-24). Verified by `tests/readme_quickstart_test.rs`: greps README for each of the named anchor strings and for every step executed by `scripts/smoke.sh`.

- **AC-32** (Secure-By-Default Compose): Out-of-box `docker-compose.yml` publishes the app port bound to `127.0.0.1:8080:8080` (loopback only), not `0.0.0.0:8080`. `.env.example` defaults `OPEN_PINCERY_HOST=0.0.0.0` so the app remains reachable across the compose network; host exposure is still restricted by the loopback-only published port mapping. No literal default exists for `OPEN_PINCERY_BOOTSTRAP_TOKEN` anywhere — compose refuses to start when unset. Verified by `tests/compose_env_test.rs` assertions on the published ports block and by a `docker compose config` run with empty env that exits non-zero.

- **AC-33** (Caddy TLS Overlay): `docker-compose.caddy.yml` and `Caddyfile.example` ship as the documented path from localhost HTTP to public HTTPS. Overlay is activated via `docker compose -f docker-compose.yml -f docker-compose.caddy.yml up -d`, puts Caddy in front of the app service, and obtains a TLS cert for a configurable domain. README Quick Start links to a "Going public with HTTPS" subsection documenting the overlay. Verified by a test that runs `caddy validate --config Caddyfile.example` (or equivalent syntax check) and asserts the README links to the subsection anchor.

### v5 Deployment Target

Same as v1–v4: `self_host_individual`. Everything new is opt-in docs, overlay, or config — an operator already running v4 sees no behavior change unless they re-read the rewritten Quick Start.

### v5 Estimated Cost

$0. All changes are docs, compose YAML, `.env.example`, scripts, and tests. No runtime dependencies added.

### v5 Quality Tier

Still skyscraper. v5 closes the operator-UX gate that skyscraper tier implies but v1–v4 carried as debt.

### v5 Clarifications Needed

None. All six ACs have unambiguous pass/fail criteria. OpenRouter remains the default LLM base URL; OpenAI ships as a commented alternative.

### v5 Deferred (from this iteration)

- **Bootstrap token rotation/expiry rules** (`self-host-readiness.md` §1) — v6
- **`self_host_team` split-topology docs and compose overlay** (readiness §2) — v6
- **Upgrade runbook and backup-encryption guidance** (readiness §5) — v6
- **Local admin lockout recovery and MFA policy** (readiness §3) — v6
- **UI first-run wizard / `pcy status` unbootstrapped auto-detect** — v6
- **Machine-readable `AGENTS.md` for operator agents** — v6
- **Published OCI images on `ghcr.io`** — v6 (pairs with team-topology work)
- **TLA+ enum-name alignment** (runtime raw status strings → spec variant names) — v6 RECONCILE work, carried forward from v4
- **Real signup/login flow, multi-tenant RBAC enforcement, per-workspace rate limits, account suspension** — previously tagged v5 in v4 Deferred; reassigned to v6 because v5 is operator-onramp only
- **CLI auth subcommands beyond `bootstrap`/`login`** — reassigned to v6 with the auth flow above

---

## v6 — Capability Foundations & Security Baseline

### Problem (v6)

v5 shipped a usable operator onramp, but `docs/input/north-star-2026-04.md` landed as the canonical direction for the project and reveals that the current runtime does not yet honor three of the north star's load-bearing security invariants:

1. **Agent-authored code runs on the substrate host.** `src/runtime/tools.rs::dispatch_tool` shells out directly via `tokio::process::Command::new("sh")` with the full host environment and no filesystem, network, or syscall confinement. North star Bet #11a requires every tool execution to run inside a capability-scoped, disposable sandbox (Zerobox on Linux, Bubblewrap+Seccomp; Seatbelt on macOS) and `docs/input/security-architecture.md` §Layer 1 makes this mandatory. Current state is a direct violation of the Professional Bar criterion "rollback-capable or confirmation-gated" and of the invariant "agent-authored code runs in a Zerobox sandbox, not on the substrate host."

2. **Tools have no declared capability class.** There is no `ReadLocal`/`WriteLocal`/`ExecuteLocal`/`Network`/`Destructive` classification, and the `agents.permission_mode` column (present in schema since v1) is never consulted. `yolo`, `supervised`, and `locked` modes are indistinguishable at runtime. This blocks north-star Bet #3 ("capability-scoped credentials beat ambient authority") from being enforceable at all.

3. **Runtime status is raw strings, not a typed enum.** `src/models/agent.rs` still writes literal `'awake'`/`'asleep'`/`'maintenance'` in nine SQL sites (grep-verified). The TLA+ spec names `Resting`, `WakeAcquiring`, `Awake`, `WakeEnding`, `Maintenance` have never been reflected in the Rust type system, despite `preferences.md` "Enum states match the spec exactly." Carried forward as debt since v4. Every future spec transition rename silently succeeds in code until it doesn't.

Secondary gap surfaced during v4/v5: the project has a medium-severity ignored advisory (`RUSTSEC-2023-0071`) carried on an explicit ignore list. The transitive path was already eliminated in v4 (sqlx features narrowed to Postgres only) but `deny.toml` still allows any vulnerability to pass — the gate is "no high/critical," not "no advisories." For a skyscraper-tier platform whose explicit purpose is executing code on behalf of agents, vulnerability floor is now zero.

v6 is **security-foundation only** — no new runtime features, no API changes, no schema changes. Each AC is a small, independently-shippable slice designed to land as 1–2 commits each. The larger north-star work (mission primitive, credential vault + proxy, real Zerobox binding, signals primitive, reasoner routing by governance class, skill tree, pgvector/CozoDB memory, MCP outward, SaaS) is sequenced in Deferred below.

**Re-sequencing notice.** v5's Deferred list tagged several operator-UX items (team-topology docs, upgrade runbook, first-run wizard, OCI publishing, real signup/login, multi-tenant RBAC, CLI auth subcommands) as "v6." The arrival of the north star makes the sovereign-substrate security trajectory the higher-leverage investment: tool confinement and capability enforcement unblock every downstream mission-primitive and vault work, whereas operator-UX polish compounds only after the substrate is safe to run. Those items are re-sequenced to v7–v9 below.

### Changes from v5

- New module `src/runtime/sandbox.rs` defining a `ToolExecutor` trait and a default `ProcessExecutor` implementation; `dispatch_tool` routes every shell invocation through the trait (no direct `Command::new` anywhere in `src/runtime/`).
- Default `ProcessExecutor` profile tightens shell execution: temp-directory cwd per invocation, minimal env (`PATH` only — no `HOME`, no `SSH_AUTH_SOCK`, no `*_TOKEN`/`*_KEY`/`*_SECRET`), 30-second wall-clock timeout, stdin closed. Not a kernel sandbox; a real defense-in-depth baseline that Zerobox (v8) will supplant at the same trait seam.
- New module `src/runtime/capability.rs` defining `ToolCapability` and `PermissionMode` enums + a static `required_capability(tool_name) -> ToolCapability` table. Pre-dispatch gate in `dispatch_tool` rejects forbidden calls and appends a `tool_capability_denied` event.
- New Rust enum `AgentStatus` (`Resting`/`WakeAcquiring`/`Awake`/`WakeEnding`/`Maintenance`) in `src/models/agent.rs`. DB storage continues to use lowercase strings; a single boundary conversion (`AgentStatus::from_db_str` / `AgentStatus::as_db_str`) is the only place raw strings appear. All nine existing string-literal SQL sites keep the lowercase constants but source them from the enum's `as_db_str()` via `const` bindings — a future spec rename breaks compilation.
- `deny.toml` vulnerability policy switched from "deny high/critical" to "deny any" with the ignore list emptied; CI `cargo deny check` now fails on any advisory regardless of severity.
- No schema changes. No API shape changes. No new dependencies beyond internal refactor.

### v6 Acceptance Criteria

- **AC-34** (Typed `AgentStatus` aligned with TLA+): `src/models/agent.rs` exports a `pub enum AgentStatus { Resting, WakeAcquiring, Awake, WakeEnding, Maintenance }` with `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]`. Helpers `AgentStatus::as_db_str(self) -> &'static str` and `AgentStatus::from_db_str(&str) -> Result<AgentStatus, InvalidStatus>` are the single conversion boundary. Variant-to-string mapping: `Resting → "asleep"`, `WakeAcquiring → "wake_acquiring"`, `Awake → "awake"`, `WakeEnding → "wake_ending"`, `Maintenance → "maintenance"` (two new DB values added to the CHECK constraint via migration `20260420000001_agent_status_states.sql`). Every SQL site that currently writes a literal `'asleep'`/`'awake'`/`'maintenance'` sources the string from a `const` bound to `AgentStatus::*.as_db_str()`. Verified by: (1) new unit test `tests/agent_status_test.rs` round-tripping every variant through `from_db_str`/`as_db_str`; (2) a `tests/no_raw_status_literals.rs` build-time test that greps `src/` for the regex `status\s*=\s*'(asleep|awake|maintenance|wake_acquiring|wake_ending)'` and asserts every match occurs inside a `const` declaration in `src/models/agent.rs`; (3) `cargo test --all-targets -- --test-threads=1` continues to pass.

- **AC-35** (Tool capability classification + permission-mode gate): `src/runtime/capability.rs` defines `pub enum ToolCapability { ReadLocal, WriteLocal, ExecuteLocal, Network, Destructive }` and `pub enum PermissionMode { Yolo, Supervised, Locked }`. A static `required_capability(&str) -> ToolCapability` maps `shell → ExecuteLocal`, `plan → ReadLocal`, `sleep → ReadLocal`. A static `mode_allows(PermissionMode, ToolCapability) -> bool` implements the gate: `Yolo` permits all; `Supervised` denies `Destructive`; `Locked` permits only `ReadLocal`. `dispatch_tool` calls the gate before executing; on deny it appends an event `{ event_type: "tool_capability_denied", source: "runtime", tool_name, required_capability, permission_mode }` and returns `ToolResult::Error("tool disallowed by permission mode")` to the LLM without invoking the executor. Verified by: (1) table-driven unit test covering all 15 `(mode × capability)` combinations against the gate; (2) integration test `tests/capability_gate_test.rs` that creates a `Locked` agent, sends a message the LLM answers with a `shell` tool call (wiremock), asserts the event log contains one `tool_capability_denied` and zero `tool_result` events for that wake, and asserts no process was spawned (via a counting `ToolExecutor`).

- **AC-36** (Shell executor behind a `ToolExecutor` trait with a hardened default profile): `src/runtime/sandbox.rs` defines `#[async_trait] pub trait ToolExecutor: Send + Sync { async fn run(&self, cmd: &ShellCommand, profile: &SandboxProfile) -> ExecResult; }` and ships a `ProcessExecutor` impl. `SandboxProfile` declares `{ cwd: PathBuf (tempdir per call), env_allowlist: Vec<String> (default: ["PATH"]), deny_net: bool (default true — advisory for v6, enforced in v8), timeout: Duration (default 30s) }`. `ProcessExecutor::run` creates a fresh tempdir, builds `tokio::process::Command` with `.env_clear()`, re-adds only the allowlisted vars from the host env, sets cwd, closes stdin, wraps in `tokio::time::timeout`, kills the child on timeout and returns `ExecResult::Timeout`. Rejects any command containing the substring `sudo ` or starting with `sudo`. `dispatch_tool` in `src/runtime/tools.rs` holds a `Arc<dyn ToolExecutor>` (injected at `AppState` construction) and routes every shell call through it — zero direct `Command::new` remains under `src/runtime/`. Verified by: (1) unit test asserting `ProcessExecutor` strips `HOME`/`SSH_AUTH_SOCK`/`MY_SECRET` from the child env (test sets them before `run`, asserts the child can't see them via `printenv`); (2) unit test asserting a `sleep 60` command against a 1-second timeout returns `ExecResult::Timeout` and the child process is no longer alive; (3) unit test asserting `sudo ls` is rejected before spawning; (4) a `tests/no_raw_command_new.rs` build-time test greps `src/runtime/` for `Command::new\(` and asserts the only match is the single call site inside `ProcessExecutor::run`.

- **AC-37** (Zero-advisory vulnerability gate): `deny.toml` `[advisories]` section sets `vulnerability = "deny"`, `unmaintained = "warn"`, `yanked = "deny"`, and `ignore = []` (empty). No `RUSTSEC-*` ignores remain. CI `cargo deny check advisories` must pass with zero findings on a clean checkout. Verified by: (1) `cargo deny check advisories` exits 0 locally on the v6 commit; (2) `.github/workflows/ci.yml` job runs `cargo deny check` (already wired in v3 AC-16) and green-lights the v6 PR; (3) `tests/deny_config_test.rs` parses `deny.toml` via the `toml` crate and asserts `advisories.vulnerability == "deny"` and `advisories.ignore == []`.

### v6 Deployment Target

Same as v1–v5: `self_host_individual`. No deployment surface changes. An operator running v5 will see no behavior change for unsandboxed command paths that were already inside the documented threat model — but every `locked` agent now actually behaves differently from a `yolo` agent, and every shell invocation now runs in a tempdir with a stripped environment.

### v6 Estimated Cost

$0. All changes are internal refactors plus one migration adding two values to an existing CHECK constraint. No new runtime dependencies; no new infrastructure.

### v6 Quality Tier

Still skyscraper. v6 closes invariant-level gaps that skyscraper tier implies but v1–v5 carried as debt.

### v6 Clarifications Needed

None. Every AC has a concrete pass/fail test. The two new DB status values (`wake_acquiring`, `wake_ending`) are not yet written by any transition — they are reserved so that a future TLA+-faithful refactor of the CAS pipeline does not require a second migration. The `deny_net` flag on `SandboxProfile` is advisory in v6 and enforced by the Zerobox executor in v8.

### v6 Deferred (explicit roadmap)

North-star-driven roadmap in rough dependency order. Each version intentionally sized to 3–6 ACs so BUILD commits stay small.

- **v7 — Credential vault + reasoner-secret refusal.** AES-256-GCM encrypted `credentials` table keyed by `(workspace_id, name)`; operator-only CRUD via `pcy credential {add,list,revoke}`; `list_credentials` returns names only. System prompt hardened to refuse pasted secrets and redirect to the vault. Tool-dispatch path receives a `PLACEHOLDER:<credential_name>` envelope instead of raw values. No proxy injection yet — just the vault storage and the placeholder handshake. (north-star Bet #3 first half; security-architecture Layer 2)
- **v8 — Zerobox executor implementation.** Second `ToolExecutor` impl behind `--features zerobox`; `SANDBOX_BACKEND=zerobox` env selector; Bubblewrap+Seccomp on Linux, Seatbelt on macOS, stub error on Windows. `deny_net` enforced. Per-tool profile loaded from `src/runtime/tool_profiles.rs`. (security-architecture Layer 1)
- **v9 — Proxy credential injection.** HTTP egress from Zerobox sandboxes routed through a proxy that substitutes `PLACEHOLDER:<name>` with real vault values for pre-approved hosts. Closes the "agent never touches real secret" invariant. (north-star Bet #3 second half)
- **v10 — Mission primitive.** New `missions` table (`id`, `agent_id`, `accountable_user_id`, `charter`, `capability_scope JSONB`, `budget_usd`, `budget_wall_clock_seconds`, `acceptance_contract JSONB`, `status`, `started_at`, `completed_at`). `POST /api/agents/:id/missions`; wake loop consumes the active mission's capability scope as an additional gate on top of `permission_mode`. (north-star Bet #4)
- **v11 — Signals primitive.** Generic agent↔human signal records with direction, free-form tag, payload, response expectation. `pcy signals` inbox command; UI signals panel. Escalation is convention built on signals, not a substrate enum. (north-star Bet #5a)
- **v12 — Reasoner routing by governance class.** `ReasonerRole` + `GovernanceClass` enums; catalog-level `(role, class) → (provider, model)` map; mission acceptance contract declares minimum governance class; runtime refuses to pick a reasoner below that class. (north-star Bet #10)
- **v13 — pgvector L1/L4 memory.** `pgvector` extension added; projections and wake summaries embedded; memory controller API abstracts layer traversal. (north-star Bet #2 v7-ish target, re-sequenced after mission primitive lands)
- **v14 — Pincer skill tree (L3 memory).** Auto-crystallized execution paths tied to capability dependencies; per-pincer-scope skill library. (north-star Bet #6a)
- **v15 — MCP outward.** Expose `src/runtime/tools.rs` over MCP server protocol; agent-grantable MCP tool discovery. (north-star Bet #9, Boundary #1)
- **v16 — Greywall host sandbox + first tagged release.** `greywall -- ./open-pincery serve` as the documented launch path; `v1.0.0` cosign-verified release. (security-architecture Layer 4 + v3 AC-20 exercise)
- **v17+ — SaaS productization.** Real signup/login, multi-tenant RBAC enforcement, per-workspace rate limits, account suspension, billing, abuse prevention, ToS/Privacy. (north-star Non-Goal #4 until the substrate is proven; deferred from v5)
- **Operator-UX polish (re-sequenced from v5 Deferred).** Bootstrap token rotation/expiry, `self_host_team` topology, upgrade runbook, backup encryption, local-admin lockout recovery + MFA, UI first-run wizard, machine-readable `AGENTS.md`, published OCI images — folded into whichever above slice naturally carries them. Not its own version; each item lands when its dependency lands.
- **TLA+ transition refactor (use new status values).** The two reserved status values `wake_acquiring` / `wake_ending` added by AC-34 get populated when the CAS pipeline is split to match the TLA+ transition graph exactly. Tracked, unscheduled — likely pairs with v10 mission primitive since missions induce a natural refactor of the wake pipeline.
- **OpenClaw integration.** Complement per north-star Boundaries; shape TBD once OpenClaw publishes a stable surface.
- **CozoDB embedded graph substrate.** Target north-star Bet #2 "v10-ish" — added when a Tier 1 mission's acceptance contract requires a query Postgres recursive CTEs cannot answer cleanly.

### v6 Dependencies on Prior Versions

None broken. All v1–v5 ACs remain satisfied. AC-34 adds two unused enum values to the status CHECK constraint; no existing row is affected. AC-35 and AC-36 are strictly more restrictive than v5 behavior, but the default permission mode remains `yolo`, so unsandboxed v5 behavior is the default shape — operators opt into `supervised` or `locked` per agent.

---

## v7 — Credential Vault & Reasoner-Secret Refusal

### Problem (v7)

v6 closed the substrate's execution baseline (typed `AgentStatus`, capability gate, hardened `ProcessExecutor`, zero-advisory `deny.toml`), but the north star's most load-bearing security invariant is still unenforced:

> _"Credentials themselves flow through a two-layer mechanism … the operator provisions secrets out-of-band through a dedicated credential vault (encrypted at rest, AES-256-GCM); the operator never pastes a secret into a chat surface … the reasoner is system-prompted to refuse any attempt to receive a secret through conversation and to redirect the operator to the vault."_
>
> — `docs/input/north-star-2026-04.md` §Bet #3; `docs/input/security-architecture.md` §Layer 2

Concretely, the shipped v6 runtime has **no vault at all**:

1. There is no `credentials` table, no encryption-at-rest path, and no place for an operator to store an external API key, OAuth token, or database password tied to a workspace.
2. Agents have no `list_credentials` tool, so even the "names-only discoverability" half of Bet #3 is absent. The only way for an agent to use an external service today is for the operator to hand-paste the secret into a message or charter — exactly the anti-pattern the north star forbids.
3. The system prompt (`migrations/20260418000009_create_prompt_templates.sql` default) contains no instruction to refuse pasted secrets. A well-meaning operator will leak a key on their first non-trivial task.
4. Tool dispatch has no concept of a `PLACEHOLDER:<name>` envelope, so when v8 ships the real Zerobox executor and v9 ships the proxy, there is no existing API seam for the proxy to hook into — the substitution will look like an architectural afterthought instead of a pre-reserved extension point.

v7 is **vault storage + handshake surface only** — no proxy, no network interception, no secret ever leaving the substrate process. The real cryptographic isolation (secret only exists inside the Zerobox proxy) is v9's responsibility. v7's job is:

- Ship an encrypted vault an operator can trust with real credentials today.
- Ship the `list_credentials` tool so agents can discover what they have.
- Ship the `PLACEHOLDER:<name>` envelope so v8/v9 can drop in without an API change.
- Ship the reasoner-refusal behavior so operators are mechanically redirected to the vault before they leak a secret.

Each AC is a small, independently-shippable slice. No schema changes outside the new `credentials` table and one additive prompt-template version.

### Changes from v6

- New migration `20260420000002_create_credentials.sql` — `credentials(id uuid pk, workspace_id uuid fk, name text, ciphertext bytea, nonce bytea, aad bytea, created_by uuid fk users, created_at timestamptz, revoked_at timestamptz null, unique(workspace_id, name) where revoked_at is null)`.
- New module `src/runtime/vault.rs` — AES-256-GCM sealed-box API over a workspace-scoped master key loaded from `OPEN_PINCERY_VAULT_KEY` (base64-encoded 32-byte random). Uses the `aes-gcm` crate (orthodox RustCrypto AEAD). Per-credential 96-bit nonce from `OsRng`. AAD binds ciphertext to `workspace_id || name`.
- New operator-only API endpoints in `src/api/credentials.rs`: `POST /api/workspaces/:id/credentials` (create), `GET /api/workspaces/:id/credentials` (list, names only), `DELETE /api/workspaces/:id/credentials/:name` (revoke). All require `workspace_admin` or `local_admin`; bearer session token from v2 auth.
- New CLI surface in `src/cli/commands/credential.rs`: `pcy credential add <name> [--value -]`, `pcy credential list`, `pcy credential revoke <name>`. `add` reads the secret from stdin or a TTY prompt — **never** from argv (prevents shell history / ps leakage).
- New tool `list_credentials` in `src/runtime/tools.rs`. Registered in `src/runtime/capability.rs` as `ToolCapability::ReadLocal`. Returns `Vec<String>` of non-revoked credential names scoped to the agent's workspace. Never returns values.
- New envelope type `CredentialRef(String)` in `src/runtime/vault.rs`. When `dispatch_tool` receives a shell invocation whose `env` map includes an entry like `{"AWS_ACCESS_KEY_ID": "PLACEHOLDER:aws_prod"}`, the runtime resolves `aws_prod` against the workspace vault, confirms it exists and is unrevoked, and passes the **unchanged placeholder string** to `ProcessExecutor` (v7 does not yet perform substitution — v9 will replace this step with real proxy-side injection). A missing/revoked credential aborts dispatch with a `credential_unresolved` event and returns `ToolResult::Error` to the LLM.
- System prompt hardening: a new prompt-template row (`name = "default_agent"`, version bumped) adds a non-negotiable "Credential Handling" section instructing the reasoner to refuse any inbound message that appears to contain pasted secret material, emit a `message` tool call redirecting the operator to `pcy credential add`, and never echo credential values back in tool output or chat. v1 AC-3 prompt-assembly order is preserved.
- No changes to CAS lifecycle, wake loop, maintenance pipeline, capability gate semantics (beyond adding `list_credentials` to the required-capability table), or v5 operator onramp.

### v7 Acceptance Criteria

- **AC-38** (Encrypted vault storage): Migration `20260420000002_create_credentials.sql` creates the `credentials` table as described above with `CHECK (length(ciphertext) >= 16)` and `CHECK (length(nonce) = 12)`. `src/runtime/vault.rs` exports `Vault::seal(workspace_id, name, plaintext) -> SealedCredential` and `Vault::open(workspace_id, name, sealed) -> Result<Vec<u8>, VaultError>`. Master key is loaded once at startup from `OPEN_PINCERY_VAULT_KEY` (base64, 32 bytes decoded); missing/malformed key fails the process with an actionable error before any HTTP listener binds. Nonce is freshly sampled from `OsRng` for every seal; AAD is `format!("{workspace_id}:{name}").as_bytes()`. Verified by `tests/vault_roundtrip_test.rs`: seal + open round-trips a 32-byte secret across 100 iterations with distinct nonces; tampering with `ciphertext`, `nonce`, `aad`, or the `(workspace_id, name)` pair on `open` returns `VaultError::Authentication`; a sealed secret is unreadable with a different master key (assert `VaultError::Authentication`, not panic).

- **AC-39** (Operator-only vault API): `src/api/credentials.rs` exposes `POST /api/workspaces/:id/credentials` (body `{ "name": "...", "value": "..." }` — name matches regex `^[a-z0-9_]{1,64}$`; value length 1–8192 bytes), `GET /api/workspaces/:id/credentials` (returns `[{ "name": "...", "created_at": "...", "created_by": "..." }]` — **no `value` field, no ciphertext, no nonce**), and `DELETE /api/workspaces/:id/credentials/:name` (soft-revokes by setting `revoked_at`). All three require a session whose user holds `workspace_admin` on the target workspace or is a `local_admin`; any other role returns 403 and appends an `auth_forbidden` event. Writes append a `credential_added` or `credential_revoked` event to the workspace's audit stream including `created_by`/`revoked_by` and the credential name (never the value). Verified by `tests/vault_api_test.rs`: admin can create/list/revoke; non-admin workspace member gets 403 on all three; `GET` response JSON is scanned and asserted to contain zero occurrences of the secret value bytes and zero bytea-shaped fields; duplicate-name create on a non-revoked credential returns 409; `DELETE` then `POST` with the same name succeeds (re-add after revoke).

- **AC-40** (Operator CLI ergonomics): `pcy credential add <name>` reads the secret value from stdin (pipe-safe) or from an interactive TTY prompt that **disables echo** (via `rpassword` or equivalent orthodox crate); never from argv. `pcy credential list` prints a two-column `NAME  CREATED_AT` table of non-revoked credentials for the current workspace. `pcy credential revoke <name>` prompts for confirmation unless `--yes` is passed and prints the revocation timestamp on success. All three commands authenticate using the v4 `pcy login`-stored session token and operate on the workspace selected by `pcy` config — no credential value ever appears in shell history, argv, environment variables, or CLI log output. Verified by `tests/cli_credential_test.rs`: (a) asserts `pcy credential add` rejects being given the value on argv with a clear error message; (b) exercises a stdin-piped add + list + revoke round-trip against a live test server and grep-asserts the raw secret bytes never appear in captured stdout/stderr of any command; (c) asserts the `rpassword` code path is reachable (integration-test via a PTY fixture or a mockable trait) and fails closed if a TTY is required but unavailable.

- **AC-41** (`list_credentials` tool returns names only): A new tool `list_credentials` is registered in `src/runtime/tools.rs`. Its capability is `ToolCapability::ReadLocal` (per v6 AC-35 table) so `Locked`, `Supervised`, and `Yolo` agents all have access. Given no arguments, it queries credentials for the agent's workspace, filters `revoked_at IS NULL`, and returns a JSON array of `{ "name": "...", "created_at": "..." }` — **no `value`, no ciphertext, no nonce**. The tool records a `tool_call` event and the response as a `tool_result` event like every other tool. Verified by `tests/list_credentials_tool_test.rs`: a `Locked` agent whose workspace has 3 credentials (one revoked) receives a message that prompts the LLM (wiremock) to call `list_credentials`; the resulting `tool_result` event payload is asserted to contain exactly the two non-revoked names and zero occurrences of any known credential value bytes; a cross-workspace agent is asserted to see an empty list (workspace isolation).

- **AC-42** (Reasoner refuses pasted secrets): A new prompt template row (`name = "default_agent"`, `version = N+1`, `is_active = true`) includes a "Credential Handling" section instructing the reasoner: (a) treat any inbound message containing a value longer than 24 characters and matching a sensitive-material heuristic (contiguous `[A-Za-z0-9_\-+/=]{24,}` with at least one digit and one non-alpha) as a potential pasted secret; (b) refuse to echo, store, summarize, or act on it; (c) emit a user-facing `message` redirecting to `pcy credential add <name>` and referencing `/api/workspaces/:id/credentials`; (d) never include credential values in `identity` or `work_list` maintenance output. The previous template row stays in the table (immutable per v1) but `is_active = false`. Verified by `tests/reasoner_secret_refusal_test.rs`: an agent is sent a message containing a fake high-entropy token; the LLM response (via wiremock fixture built from the assembled prompt) is asserted to contain the phrase `pcy credential add` and to not echo the token back; the v1 AC-3 prompt-assembly ordering test continues to pass against the new active template; the maintenance cycle fixture asserts the updated `identity` and `work_list` projections contain zero occurrences of the token.

- **AC-43** (`PLACEHOLDER:<name>` dispatch handshake): When `dispatch_tool` executes a `shell` tool call whose `env` map contains any entry whose value starts with the literal prefix `PLACEHOLDER:`, the runtime (i) extracts the credential name suffix, (ii) looks up the `(workspace_id, name)` in `credentials` with `revoked_at IS NULL`, (iii) on hit, leaves the env value **unchanged** as `PLACEHOLDER:<name>` and proceeds to `ProcessExecutor::run` (real substitution is v9's responsibility), (iv) on miss or revoked, aborts dispatch before spawning, appends a `credential_unresolved` event `{ tool_name, credential_name, reason: "missing"|"revoked" }`, and returns `ToolResult::Error("credential not found: <name>")` to the LLM. Non-placeholder env values flow through unchanged. Verified by `tests/placeholder_envelope_test.rs`: (a) a `Yolo` agent whose workspace has `stripe_test` but not `stripe_prod` invokes shell with `env = { "STRIPE_KEY": "PLACEHOLDER:stripe_prod" }` and receives `ToolResult::Error`; the event log contains exactly one `credential_unresolved` event with `reason = "missing"` and zero processes spawned (via the counting `ToolExecutor` from v6); (b) the same agent with `PLACEHOLDER:stripe_test` proceeds to spawn and the child's environment contains the literal string `PLACEHOLDER:stripe_test` (proving v7 does not yet substitute — the seam is reserved for v9); (c) a revoked `stripe_test` yields `reason = "revoked"`.

### v7 Stack Additions

| Concern              | Addition                   | Notes                                                                                                                     |
| -------------------- | -------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| AEAD                 | `aes-gcm` (RustCrypto)     | Orthodox pure-Rust AEAD; audited; active maintenance                                                                      |
| RNG                  | `rand` + `getrandom/OsRng` | Already transitively present; made explicit for nonce generation                                                          |
| TTY echo suppression | `rpassword`                | Standard Rust crate for password prompts; MIT-licensed; no transitive security advisories as of the v6 `cargo deny` floor |

No new runtime services. No network dependencies. Everything runs in the existing single binary against the existing PostgreSQL instance.

### v7 Deployment Target

Same as v1–v6: `self_host_individual`. One new required environment variable — `OPEN_PINCERY_VAULT_KEY` (base64 32 random bytes). Compose and `.env.example` gain a generator comment (`openssl rand -base64 32`). The v5 `.env.example` consistency test (AC-29) automatically covers the new variable via its allowlist-of-reads check.

### v7 Estimated Cost

$0. One new migration, one new crate dependency (`aes-gcm`, already transitively available on most Rust toolchains), one new CLI command group, one new API router. No new infra, no new services, no new ports.

### v7 Quality Tier

Still skyscraper. v7 begins honoring the north star's load-bearing security invariant; skipping it would make Bets #3, #9, #11a unverifiable.

### v7 Clarifications Needed

None with pass/fail impact. Two resolved-here choices worth calling out:

- **Master key rotation** is intentionally out of scope for v7. Re-keying every sealed credential requires either online re-encryption or a key-version column; both are non-trivial and would dominate v7. A tracked Deferred item captures this.
- **Per-credential ACLs (which agent in a workspace can see which name)** are deferred to the mission primitive (v10), whose `capability_scope` is the correct place to scope credential access below the workspace boundary.

### v7 Deferred

- **Real proxy-side secret substitution.** The Zerobox egress proxy replaces `PLACEHOLDER:<name>` with real values for pre-approved hosts. Requires v8 Zerobox executor and v9 proxy — tracked in v6's roadmap.
- **Master-key rotation (`pcy vault rotate`).** Re-key every sealed row without downtime; adds `key_version` column. Land when the operator community asks; the crypto is orthodox so this is bounded work.
- **Per-mission credential ACLs.** Currently a credential is visible to every agent in the workspace. v10 mission primitive's `capability_scope` narrows this to per-mission.
- **Bitwarden / external-vault adapters.** Pull credential material on demand from an external password manager; never store locally. v11+.
- **Credential usage audit view.** A `credential_audit` table or a materialized view over `events` that shows "which agent used which credential in which wake." Useful but not required for v7's invariant — v2 `tool_audit` already captures tool calls.
- **Prompt-injection detector upgrade.** The regex-based heuristic in AC-42 is deliberately simple. A real classifier (north-star Bet #11b) is a research task beyond v7's scope.
- **Vault-backed LLM API key.** Move `LLM_API_KEY` itself out of `docker-compose.yml` environment and into the vault. Natural pairing with v9's proxy work; noted but not sized yet.

### v7 Dependencies on Prior Versions

None broken. v1 AC-3 prompt assembly still passes against the new active template (v1 invariant preserved by one-active-per-name constraint). v5 AC-28 / AC-29 compose and `.env.example` tests automatically extend to `OPEN_PINCERY_VAULT_KEY`. v6 AC-35 capability gate is extended with one new entry (`list_credentials → ReadLocal`) — the table-driven test from AC-35 gains a row, otherwise untouched. v6 AC-36 `ProcessExecutor` is unchanged; v7 only adds a pre-spawn credential-resolution step inside `dispatch_tool` before `ProcessExecutor::run` is called. No existing DB row is touched by the new migration.

---

## v8 — Unified API Surface (Schema-Driven CLI, MCP, and Distribution)

### Problem (v8)

v7 shipped the encrypted vault and `PLACEHOLDER` handshake, but the product's external surface — the thing any operator, agent, or downstream SDK touches first — remains artisanal and inconsistent:

1. **The HTTP API has no machine-readable contract.** `docs/api.md` is hand-maintained prose; no OpenAPI/Swagger, no `/openapi.json`, no `.well-known/ai-plugin.json`. A remote agent that discovers a Pincery server has no way to enumerate operations, types, or auth requirements without scraping the repo. This blocks every pincer-birthing workflow the north star turns on.
2. **The `pcy` CLI is ergonomically inconsistent** with `aws`, `kubectl`, `gh`, `terraform`, and Cloudflare's next-gen `cf`. Concretely: `bootstrap` is a separate stateful verb that 409s once the system is initialized, forcing operators to learn two commands for one intent; agents must be addressed by UUID because lookups don't accept names; output defaults to raw JSON with no tabular human view and no `--output json|yaml|jsonpath` flag; there is no `whoami`; there is no context/profile concept, so a single operator cannot have both `local` and `prod` servers configured simultaneously; flag names are inconsistent (`--yes` vs. `--skip-confirm`); no shell completions; `demo` is a production-visible subcommand.
3. **There is no MCP server exposing the API to agents.** The north star (Bet #9, Boundary #1) and Cloudflare's Code Mode MCP both point at the same insight: agents drive CLIs and APIs more than humans do, and MCP is how a Pincery agent operates _another_ Pincery. Today, wiring Open Pincery into an MCP-capable client (Claude Desktop, Copilot Chat, Cursor, etc.) requires the agent's developer to hand-write a bespoke adapter.
4. **Distribution for remote clients is half-finished.** The v7 release workflow already publishes cosign-signed `pcy` binaries for five targets (linux-x86_64, linux-aarch64, macos-x86_64, macos-aarch64, windows-x86_64) but there is no `install.sh` that detects platform, verifies the signature, and installs the right binary. A Mac operator today has to read the release page, pick the right asset, run `shasum -c`, `cosign verify-blob`, and `chmod +x` by hand.
5. **There is no shell completion.** `aws`, `kubectl`, `gh`, and `cf` all ship completions. `pcy` does not.

Cloudflare's April 2026 post on `cf` — the single-schema source of truth that derives their REST API, SDKs, Terraform provider, and CLI — is the right model for v8 adapted to this repo's Rust-first stack. The north-star alignment is direct: a Pincery whose entire API surface is machine-introspectable by any agent, via any of {MCP, CLI, HTTP+OpenAPI, signed binary}, is the primitive that makes "pincers make more pincers" mechanically honest instead of aspirational.

v8 is **surface-only** — no schema changes, no runtime semantics changes, no new workspace features. Every AC sharpens or unifies an existing touchpoint. Breaking CLI changes are explicitly deferred: v7-era `pcy` command names continue to work for one release with deprecation warnings on stderr.

### Changes from v7

- New crate dependency: `utoipa` + `utoipa-axum` — annotate existing axum handlers with `#[utoipa::path]` + `ToSchema` derives on request/response DTOs; `utoipa::OpenApi` aggregator emits the full spec at compile time. Zero handler logic changes; purely derive-macro annotation work.
- New endpoint `GET /openapi.json` (unauthenticated; same rate bucket as `/health`) serves the OpenAPI 3.1 document. Also served as static `/openapi.yaml`.
- New module `src/mcp/` — stdio JSON-RPC Model Context Protocol server. `pcy mcp serve` reads context from the caller's CLI config + env, opens an authenticated session against the target Pincery server, and exposes every OpenAPI operation as an MCP tool by reading the same `utoipa` registry the server uses. Hand-writes the dispatch; no external MCP library (the protocol is small and stable; one external dep less).
- Restructured CLI in `src/cli/` to a kubectl-style `noun verb` tree via clap: top-level nouns `agent`, `credential`, `budget`, `event`, `context`, `auth`, `api`. Every list/get accepts name or UUID. Every command accepts `--output {table|json|yaml|jsonpath=<expr>|name}` defaulting to table-on-TTY / json-when-piped.
- New `pcy context` noun — named contexts (`[contexts.local]`, `[contexts.prod]`) with `current-context`. Auto-migrates the v4-era flat `url`/`token`/`workspace_id` config file into a `default` context on first run.
- New `pcy login` semantics — idempotent and the sole auth verb. On a fresh server it calls `POST /api/bootstrap`; on subsequent runs it calls `POST /api/login`. The separate `pcy bootstrap` subcommand is **removed** from the CLI surface entirely — matches the kubectl/gh/terraform/oc ergonomic of a single `login` verb. Server-side `/api/bootstrap` is kept unchanged; callers that need the raw endpoint use `curl`.
- New `pcy whoami` — calls `/api/me` and prints context + user_id + workspace_id + server URL; exit 0 iff authenticated.
- New `pcy completion {bash|zsh|fish|powershell}` — emits a completion script via `clap_complete`.
- New `install.sh` at repo root — detects OS + arch, downloads the matching asset from the latest GitHub release (or `--version <tag>`), enforces SHA-256, verifies cosign signature if cosign is installed (required with `--require-cosign`), installs to `$PCY_PREFIX/bin` (default `$HOME/.local/bin`), hints at `PATH` if needed. Already drafted during v7 exploration; v8 finalizes and tests it.
- `pcy demo` is removed from the production CLI and re-homed at `scripts/demo.sh`.
- Flag-name guardrails: destructive verbs accept `--force` (not `--yes`, which remains accepted for one release with a deprecation warning); any flag called `--format` is renamed to `--output`.
- Deprecation infrastructure: one shared `warn_deprecated(command)` helper that emits a single `warning: 'pcy X' is deprecated, use 'pcy Y' instead.` line to stderr, governed by `OPEN_PINCERY_NO_DEPRECATION_WARNINGS=1` for CI environments.

No changes to: core runtime (CAS lifecycle, wake loop, maintenance, drain, event log), database schema, capability gate semantics, credential vault, any existing v1–v7 AC behavior. No new runtime env vars. Handler signatures unchanged — only `#[utoipa::path]` annotations are added.

### v8 Acceptance Criteria

- **AC-44** (OpenAPI 3.1 spec served at `/openapi.json`): `src/api/mod.rs` aggregates every `#[utoipa::path]`-annotated handler into a single `utoipa::OpenApi`-derived registry. `GET /openapi.json` returns the full document with `Content-Type: application/json`; `GET /openapi.yaml` returns the YAML serialization; both are unauthenticated and share the `/health` rate-limit bucket. The document: declares `openapi: "3.1.0"`; names `Open Pincery API` and the crate version in `info`; includes a `bearerAuth` security scheme; covers every path currently registered in `api::router()` including the v7 credentials endpoints and v4 `/api/me`; and declares schemas for every DTO that crosses a handler boundary. Verified by: (1) a new `tests/openapi_spec_test.rs` that fetches `/openapi.json`, parses it with `openapiv3` (or equivalent), asserts it validates against OpenAPI 3.1, and diff-asserts the enumerated paths against a compile-time list of handler routes extracted from `api::router()`; (2) a grep test that asserts zero routes in `src/api/` lack a `#[utoipa::path]` annotation (regex over `.route("/api/..."` excluding `/openapi.*`, `/health`, `/ready`, and `/metrics`).

- **AC-45** (`pcy login` is idempotent and is the sole auth verb): `pcy login --bootstrap-token <T>` succeeds against both a fresh server and an already-bootstrapped server. On a fresh server it calls `POST /api/bootstrap` and persists the returned session token. On an already-bootstrapped server it transparently falls back to `POST /api/login` using the same token and persists the returned session token. Both paths emit one JSON line to stdout containing `session_token`, `workspace_id` (when present), and `already_bootstrapped: bool` so CI jobs can distinguish first-run from re-run. The standalone `pcy bootstrap` subcommand does **not** exist — the CLI surface matches the ergonomic of `gh auth login`, `oc login`, `terraform login`. Verified by `tests/cli_login_idempotent_test.rs`: (a) against a freshly-reset compose stack, `pcy login --bootstrap-token $T` succeeds and a subsequent `pcy whoami` returns 200; (b) a second `pcy login --bootstrap-token $T` on the same stack also succeeds with `already_bootstrapped: true` and does not 409; (c) `pcy --help` stdout contains `login` and does **not** contain `bootstrap`.

- **AC-46** (Noun-verb CLI tree with name-or-UUID resolution): Top-level nouns registered via clap: `agent`, `credential`, `budget`, `event`, `context`, `auth`, `api`, `completion`, `mcp`, `whoami`. Verbs under each noun use the canonical vocabulary: `list`, `get`, `create`, `update`, `delete` where applicable; `send` under `event`; `rotate` under `credential` (future) and `agent` (webhook). Every `get`/`update`/`delete`/`send`/subcommand that names an agent, credential, budget, or event accepts **either a name or a UUID**; ambiguous names (multiple matches) exit 2 with a two-column `ID  NAME` disambiguation table on stderr. Legacy v4 commands (`pcy agent create`, `pcy message <agent>`, `pcy events <agent>`) continue to work as hidden aliases for one release, each emitting exactly one deprecation warning redirecting to the new form (`pcy agent send`, `pcy event list`). Verified by `tests/cli_noun_verb_test.rs`: a parameterized test enumerates `(legacy_command, new_command)` pairs from a fixture table, runs both against the test server, asserts stdout is byte-identical, and asserts the legacy path wrote exactly one `warning: 'pcy X' is deprecated` line to stderr. A separate subtest creates two agents with the same name and asserts `pcy agent get <name>` exits 2 with both UUIDs in the disambiguation output.

- **AC-47** (`--output` flag is universal and TTY-aware): Every command that prints structured data accepts `--output {table|json|yaml|jsonpath=<expr>|name}`. Default is `table` when stdout is a TTY, `json` when stdout is piped or redirected. `table` output obeys `NO_COLOR` and suppresses ANSI when not-a-TTY. `jsonpath=<expr>` uses the same `jq`-style selector syntax as `kubectl` (e.g. `-o jsonpath='{.items[*].name}'`). `name` output emits one resource name per line for use in shell pipelines. A lint test asserts no legacy `--format` flag exists anywhere in `src/cli/`. Destructive verbs (`delete`, `revoke`) accept `--force`; `--yes` remains accepted for one release and triggers a deprecation warning. Verified by `tests/cli_output_flag_test.rs`: (a) running `pcy agent list -o json | jq '.[0].id'` returns a parseable UUID for every agent; (b) running `pcy agent list -o name` on a stack with three agents emits exactly three lines; (c) running `pcy agent list -o jsonpath='{.items[*].name}'` returns space-separated names; (d) running `pcy agent list` to a pipe emits JSON; (e) running the same under a PTY fixture emits a formatted table without ANSI when `NO_COLOR=1`; (f) `pcy credential revoke foo --yes` succeeds and warns about `--yes` deprecation; `pcy credential revoke foo --force` succeeds silently.

- **AC-48** (Named contexts with auto-migration): `~/.config/open-pincery/config.toml` supports the schema `current-context = "<name>"` and `[contexts.<name>]` tables each containing `url`, `token`, `workspace_id`, `user_id`. `pcy context list` prints a three-column `CURRENT  NAME  URL` table; `pcy context use <name>` switches `current-context`; `pcy context set <name> --url <u> [--token <t>]` upserts a context; `pcy context delete <name>` removes one; `pcy context current` prints only the current context name. Overrides (precedence, highest first): `--context <name>` flag, `OPEN_PINCERY_CONTEXT` env var, `current-context`. On first run after v8 upgrade, if the legacy flat `url`/`token`/`workspace_id` keys exist at the top level, the CLI migrates them into a new `default` context and sets `current-context = "default"`, writing the result atomically. `pcy whoami` (new command) prints the current context + user_id + workspace_id + server URL as table or JSON per `--output`, and exits 0 iff the token resolves via `/api/me`. Verified by `tests/cli_context_test.rs`: (a) seed a legacy-shaped config, run any `pcy` command, assert the file is migrated to the new schema and a `default` context exists; (b) add a second context `prod` with a different URL, switch between them with `pcy context use`, and confirm `pcy whoami` reports the correct URL for each; (c) `--context prod` overrides `current-context`; `OPEN_PINCERY_CONTEXT=staging` overrides `current-context` but is itself overridden by `--context`; (d) `pcy whoami` against a server the token doesn't authenticate to exits 1 and prints nothing to stdout.

- **AC-49** (MCP server exposes every API operation): `pcy mcp serve` runs a stdio JSON-RPC Model Context Protocol server (version `2025-06-18` of the MCP spec, or the latest stable when v8 lands). It authenticates to the target Pincery server using the current context's session token and exposes **every** operation in the server's `/openapi.json` as an MCP tool, named `<noun>.<verb>` (e.g. `agent.create`, `credential.list`, `agent.send_message`). Tool descriptions and input schemas are derived from the OpenAPI `summary`/`description` and `requestBody` / `parameters` schemas. Each tool invocation proxies to the corresponding HTTP operation against the current context's server; HTTP errors become structured MCP error responses. A `pincery.whoami` virtual tool exposes the current context metadata. Verified by `tests/mcp_smoke_test.rs`: (1) spawn `pcy mcp serve` in a subprocess with a pre-authenticated context, send an MCP `initialize` request, assert a valid `serverInfo` + `tools/list` capability response; (2) call `tools/list` and diff-assert the result against the route list from `api::router()` — every non-internal route must appear exactly once; (3) call `tools/call` with name `agent.list` and assert the response mirrors what `pcy agent list -o json` would have returned; (4) call a tool that hits `/api/agents/:id/messages` and assert a `message_received` event lands on the server (closes the loop).

- **AC-50** (`install.sh` one-line installer with signature verification): `install.sh` at the repo root detects OS and architecture via `uname`, resolves the release tag (default: latest via `api.github.com/repos/<owner>/<repo>/releases/latest`; overridable via `--version <tag>` or `PCY_VERSION` env), downloads the matching `pcy-<tag>-<os>-<arch>[.exe]` asset plus its `.sha256` file, enforces the checksum via `sha256sum` or `shasum -a 256`, additionally verifies the cosign signature if `cosign` is on `PATH` (required with `--require-cosign`, skippable with `--skip-cosign`), installs to `$PCY_PREFIX/bin` (default `$HOME/.local/bin`, overridable via `--prefix`), and prints a `PATH` hint when the install dir is not on `PATH`. The script supports the documented pipe invocation `curl -fsSL <raw_url> | bash` with `bash -s -- --version <tag>` flags. Supported targets: linux-{x86_64,aarch64}, macos-{x86_64,aarch64}, windows-x86_64 (via git-bash). Verified by `tests/installer_test.rs` (gated behind a `cargo test --features installer-e2e` flag so the normal `cargo test` doesn't hit GitHub): (a) the script passes `bash -n install.sh` syntax check and `shellcheck install.sh` with zero findings at the `-S warning` level; (b) against a locally-served fixture that mimics a GitHub release (sha256 + cosign artifacts), the script installs `pcy` to a tempdir prefix and the installed binary runs `--version`; (c) sha256 mismatch fails with a clear error and exits non-zero; (d) `--require-cosign` on a host without cosign exits non-zero with an actionable message.

- **AC-51** (Shell completions for 4 shells): `pcy completion {bash|zsh|fish|powershell}` emits a completion script for the named shell via `clap_complete`. The generated completion covers every noun, verb, subcommand, flag, and `--output` value. README documents the one-line install for each shell (e.g. `pcy completion bash > /etc/bash_completion.d/pcy`). Verified by `tests/cli_completion_test.rs`: for each of the four shells, assert `pcy completion <shell>` exits 0 and stdout is non-empty; assert bash completion contains the literal `_pcy()` function name; assert zsh completion contains `#compdef pcy`; assert fish completion contains `complete -c pcy`; assert PowerShell completion contains `Register-ArgumentCompleter`.

- **AC-52** (Schema-layer consistency guardrails): A new `tests/api_naming_test.rs` asserts project-wide conventions at compile time by walking the generated OpenAPI document: (a) every path segment that names a collection is plural (`/agents`, `/credentials`, `/events`); (b) every path uses `{id}` not `{agentId}`/`{credentialName}` as the parameter name for primary-key segments (except where disambiguation requires a compound key — which the test explicitly allowlists); (c) every operation summary is non-empty; (d) every DTO that appears on the wire has a `description` in its schema; (e) no operation uses the HTTP verb `PUT` except where idempotent upsert is explicitly required (v8 has zero PUTs); (f) no field named `format` appears in any DTO (reserved for OpenAPI's own `format` slot). A second lint test (`tests/cli_naming_test.rs`) walks the clap tree and asserts: every subcommand has a non-empty about string; no two sibling subcommands use both a verb-form and a noun-form for the same concept (e.g. `delete` and `remove`); every `-o`/`--output` flag accepts the same value set; no flag named `--format`/`--yes` exists in non-deprecated paths. Failures print actionable diffs. Verified by CI running both tests on every push (wired into existing `cargo test` under `.github/workflows/ci.yml` from v3 AC-16).

### v8 Stack Additions

| Concern                   | Addition                   | Notes                                                                                                                           |
| ------------------------- | -------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| OpenAPI generation        | `utoipa` + `utoipa-axum`   | Widely-used Rust OpenAPI derive crate; zero runtime cost; orthodox choice                                                       |
| Shell completion          | `clap_complete`            | Same maintainer as clap; already a transitive dep; just made explicit                                                           |
| OpenAPI validation (test) | `openapiv3`                | Parser used to validate `/openapi.json`; dev-dep only                                                                           |
| jsonpath evaluation       | `jsonpath-rust` (or equiv) | Already transitive via sqlx; if not, a lightweight dev crate                                                                    |
| MCP protocol              | None (hand-written)        | Protocol is small (~3 JSON-RPC methods: `initialize`, `tools/list`, `tools/call`); no external dep reduces supply-chain surface |

No new runtime dependencies beyond `utoipa`, `utoipa-axum`, `clap_complete`. No new infrastructure. No new ports. The `/openapi.json` endpoint adds bytes to the binary at compile time but no runtime cost beyond the first request (spec is built once at startup).

### v8 Deployment Target

Same as v1–v7: `self_host_individual`. One existing operator who upgrades sees: (a) a new `/openapi.json` endpoint (harmless, unauthenticated, behaves like `/health`); (b) the CLI auto-migrates their config to contexts on first run; (c) a handful of commands now emit deprecation warnings pointing at the new form. No env var changes. No compose changes except a comment pointing at `install.sh` for bare-metal CLI installs.

### v8 Estimated Cost

$0. All work is client-side + derive-macro annotations + one new HTTP endpoint + one new subprocess (`pcy mcp serve`). Release pipeline already publishes all five platform binaries (finalized during v7 exploration). No new infra.

### v8 Quality Tier

**House** — the CLI + API + MCP surface is long-lived and downstream consumers (SDKs generated later, agents driving MCP tools, operator scripts) depend on backward compatibility. Skyscraper is not warranted because the changes are surface-only: no novel concurrency, no new crypto, no new persistence, no schema change. Tests-per-AC with traceability, design review against existing scope/design, reconcile + verify as for house tier.

### v8 Clarifications Needed

None with pass/fail impact. Three resolved-here choices worth calling out:

- **Hand-written MCP vs. external crate.** Hand-written keeps the dep count down and the protocol is small. If a stable `mcp-rs` crate becomes the obvious choice by the time v8 lands, we can swap the implementation behind `src/mcp/` without an AC change — same seam.
- **Deprecation window.** v8 ships deprecation warnings, not hard removals. v1.1.0 (the tag that ships v8) keeps aliases; v1.2.0 (post-v8) is the earliest point any legacy command is removed.
- **Config file migration is automatic.** Operators do not run a `pcy migrate-config` command. The CLI auto-detects legacy-shaped configs and rewrites them atomically (temp file + rename) on first invocation after upgrade. Original config is backed up to `config.toml.pre-v8`.

### v8 Deferred

- **Generated language SDKs** (Python, TypeScript) from `/openapi.json`. Now trivially possible once AC-44 lands — but the CI + publish pipeline for multi-language SDK releases is its own project. Target v10+.
- **Terraform provider.** Same story — `/openapi.json` makes it feasible; v8 just doesn't ship it. The Cloudflare post is the reference implementation; revisit when a real operator asks.
- **Dashboard / docs site regen.** A `docs/api/` site auto-built from `/openapi.json` via Redoc or similar. Purely additive; schedule with the first operator-UX pass after substrate stability.
- **Auto-generated CLI from the OpenAPI spec.** v8 keeps the clap tree hand-written so we can tune ergonomics per command (Cloudflare's own post flags this tension). Once AC-52 consistency rules stabilize, we can revisit generation from schema for net-new nouns.
- **`pcy event tail` as real server-sent events.** v8 keeps the v4-era poll loop. SSE/WebSocket upgrades pair naturally with v11 signals primitive.
- **Homebrew tap / winget / apt repo.** `install.sh` + signed binaries are enough for v8. Package-manager distribution is a compounding maintenance cost; defer until there's a community asking for it.
- **MCP server hosted as a separate daemon.** v8 `pcy mcp serve` is a stdio subprocess the operator's MCP-capable client spawns. A long-running TCP-listening MCP daemon (for remote agents) is a v11+ story that pairs with the signals primitive and real auth-per-agent credentials.
- **OpenAPI-driven fuzz / contract tests for every handler.** Nice-to-have supply-chain hardening; target v12+ once the surface is fully stable.
- **Per-context default `--output` preference.** Some operators will want `default = table`, others `default = json`. Config schema already allows it; v8 ships with TTY autodetect and adds per-context override later if requested.

### v8 Dependencies on Prior Versions

None broken.

- v4 AC-27 (HTTP API stability contract): preserved. Every documented endpoint gets a `#[utoipa::path]` annotation; no method or path changes.
- v5 AC-29 (`.env.example` is current): no new runtime env vars in v8 — test continues to pass untouched.
- v5 AC-30 (smoke script): `scripts/smoke.sh` updated to invoke `pcy login` instead of `pcy bootstrap` and to assert `/openapi.json` responds 200. Legacy smoke assertions all continue to hold.
- v6 AC-35 capability gate: unchanged. MCP tool dispatch goes through the same authenticated HTTP API as the CLI; capability decisions happen server-side as today.
- v7 AC-39 / AC-40 (vault API + CLI): the credential endpoints gain `#[utoipa::path]` annotations; CLI commands are restructured under `pcy credential <verb>` (verbs are unchanged semantically); the legacy top-level `pcy credential` command tree remains accessible.

## v9 — Solo-Founder Trust Gate (Security Truth, Credential Requests, Auth Model, UI Rebuild)

### Problem

A skeptical solo-founder CEO walked through v8.0 live. They flagged twelve blockers, grouped P0 / P1 / P2, that stop them from betting their company on the product. The common thread: **the security story is shipped as marketing, not as code.** README advertises `zerobox` and `greywall`; `src/runtime/sandbox.rs` runs `sh` directly on the host. The secrets story protects agent → tool but leaves human → vault wide open (an agent can echo a plaintext key into `assistant_message` and poison the event log). The auth/session model has no TTL, no revocation, no users, no RBAC. The UI is routing bones, not a product. No event-log search, export, cost reports, retention, or multi-tenant story.

v9 is **the trust gate.** No further versions ship until every P0 is real code with a failing-adversary test, every P1 has a visible UI surface, and every P2 has a design doc or an explicit "deferred to v10." This is not a feature version — it is the version that makes everything already built defensible.

### Smallest Useful Version

A v9.0 release where: (a) the binary ships with **real** Linux sandboxing wired to `ProcessExecutor` or the sandbox claim is removed everywhere, (b) the **Credential Request** out-of-band deposit flow is live end-to-end (new table, tool, API, dedicated deposit UI), (c) session tokens have TTL + refresh + revocation + a users/roles table with at least `admin | operator | viewer`, and (d) a rebuilt UI surfaces the event-timeline, the budget burndown, and the credential-request inbox. P1 observability items (search, export, cost reports, retention) ship as v9.1. P2 items (multi-tenant, tool catalog, Ollama bullet, version handshake, terminology pass) ship as v9.2 or are explicitly deferred to v10.

### Acceptance Criteria

- **AC-53** (Sandbox Truth — kill the marketing lie): Exactly one of the following is true and asserted by test. **Option A**: every shell tool call on Linux runs inside a Bubblewrap + seccomp-bpf-based sandbox (the Zerobox primitive) that the `ProcessExecutor` invokes via a new `SandboxedExecutor`; an adversarial test `tests/sandbox_escape_test.rs` runs three payloads (`cat /etc/shadow`, `curl attacker.example.com`, `ls /host`) inside the sandbox and asserts all three fail. **Option B**: every occurrence of the words `zerobox`, `Zerobox`, `greywall`, `Greywall`, `sandbox`, and `isolation` in `README.md`, `preferences.md`, `DELIVERY.md`, the landing-page HTML, and the CLI `--help` output is replaced with the literal phrase `process-level hardening (not container isolation)`; a `tests/no_sandbox_lie_test.rs` grep-lint enforces this. The repository MUST NOT merge both paths simultaneously. Verified by the adversarial test *or* the lint test, selected by `SANDBOX_MODE=real|disabled` in scope.

- **AC-54** (Security Threat Model): `docs/SECURITY.md` exists and is linked from `README.md`. It contains four sections with minimum content: **(1) Adversary capabilities** (what a malicious prompt / compromised LLM / compromised tool output can do), **(2) In-scope attacks** (at least: prompt-injection exfil, tool-sandbox escape, credential leak via event log, session hijack, webhook replay), **(3) Out-of-scope** (at least: compromised host, compromised Postgres, insider with DB credentials), **(4) Disclosure**: a working email address + PGP key fingerprint OR a GitHub Security Advisories link. A CI test `tests/security_doc_test.rs` asserts all four sections are present by regex.

- **AC-55** (Credential Request Tool — agent asks for a secret out-of-band): A new migration creates `credential_requests (id uuid pk, agent_id uuid fk, name text, reason text, doc_url text nullable, status text, deposit_token text unique, deposit_token_expires_at timestamptz, created_at timestamptz, fulfilled_at timestamptz nullable, fulfilled_by uuid nullable)`. A new tool `request_credential(name, reason, doc_url?)` is registered in `src/runtime/tools.rs`; calling it inserts a row and emits a `credential_requested` event containing `{request_id, name, reason, doc_url}` — **never the deposit_token**, which stays server-only. A new API `POST /api/agents/:id/credential-requests` (for programmatic inspection) and `GET /api/credential-requests?status=pending` return request metadata. Verified by `tests/credential_request_tool_test.rs`: agent tool call inserts row + emits event; event payload contains `request_id` but no `deposit_token`; listing the pending requests via API returns the row.

- **AC-56** (Credential Deposit Page — human deposits the secret, not the agent): Server renders an HTML page at `GET /deposit/:deposit_token` that shows `{name, reason, doc_url, agent_name}`, a single password-type input with `autocomplete="off"` + `name="value"`, and a submit button. `POST /deposit/:deposit_token` AEAD-encrypts the value with the existing vault key, inserts into `credentials` table, updates `credential_requests.status=fulfilled` and `fulfilled_at`, emits a `credential_deposited` event (name only, no plaintext, no ciphertext), and invalidates the deposit_token. The deposit_token is **single-use** and expires 24h after creation (configurable via `OPEN_PINCERY_DEPOSIT_TTL_HOURS`). Stale or already-used tokens render HTTP 410 Gone with a clear "this request has expired or already been fulfilled" message. Verified by `tests/credential_deposit_test.rs`: valid token → deposit works → event emitted → second POST returns 410; expired token returns 410.

- **AC-57** (Credential Request CLI + UI Inbox): `pcy credential request list --output json` returns all pending requests for the workspace. `pcy credential request approve <request_id>` prints the deposit URL (operator then opens it in a browser). `pcy credential request reject <request_id> --reason "..."` closes the request with status=rejected and emits a `credential_request_rejected` event the agent can see on its next wake. The UI `/credentials/requests` view renders the pending-request list with a "Open deposit page" button per row. Verified by `tests/cli_credential_request_test.rs` exercising all three verbs against a live test DB.

- **AC-58** (Session TTL + Refresh + Revocation): Sessions have a server-enforced TTL of 24h by default (configurable via `OPEN_PINCERY_SESSION_TTL_HOURS`). A new column `sessions.expires_at timestamptz not null` is added via migration and backfilled to `created_at + 24h`. Every authenticated request checks `expires_at > now()`; expired tokens return HTTP 401 with `{"error":"session_expired"}`. `POST /api/sessions/refresh` takes a valid session token and returns a new one with extended expiry. `POST /api/sessions/revoke` deletes the current session. `pcy session list --output json` returns `[{id, user_id, created_at, expires_at, last_used_at}]`; `pcy session revoke <id>` removes it. Verified by `tests/session_ttl_test.rs`: expired token returns 401; refresh extends TTL; revoke invalidates immediately.

- **AC-59** (Users + Roles): A new migration creates `roles` enum `admin | operator | viewer` and adds `users.role text not null default 'admin'` (existing admin user backfilled to `admin`). New CLI: `pcy user add <email> --role operator`, `pcy user list`, `pcy user set-role <email> <role>`, `pcy user delete <email>`. Role enforcement: `admin` = all endpoints; `operator` = everything except user-management + role-change; `viewer` = `GET` endpoints only (no `POST`/`PATCH`/`DELETE`, no tool execution). Verified by `tests/rbac_test.rs`: each role exercised against each endpoint with correct allow/deny.

- **AC-60** (Auth README Rewrite + Three-Box Diagram): `README.md` has a new "Authentication" section above Quickstart. It contains: a mermaid or ASCII three-box diagram (`Admin Seed → Bootstrap → Session Token`), a table defining each token's lifetime + source + purpose, and a worked example showing the three commands in order (`pcy login --bootstrap-token`, `pcy whoami`, `pcy session list`). The env var `OPEN_PINCERY_BOOTSTRAP_TOKEN` is renamed to `OPEN_PINCERY_ADMIN_SEED` with a deprecation alias honoured for one version; `scripts/smoke.sh` and docker-compose.yml are updated. Verified by `tests/readme_auth_section_test.rs` asserting the diagram block + the token table + the env var presence.

- **AC-61** (UI Rebuild on a Design System): The static frontend migrates off hand-rolled hash-routing onto a single declared design system. `static/js/` is replaced with a vendored build of either HTMX + Pico.css (zero-build) or a Preact + Tailwind artifact (prebuilt, committed). The new UI ships six views minimum: **Login**, **Agents list**, **Agent detail** (identity, work list, budget, rotate secret), **Event timeline** (color-coded by event_type, tool_call rows expandable), **Budget burndown** (sparkline per agent), **Credential request inbox**. Dark mode toggle present. Verified by `tests/ui_smoke_test_v9.rs`: each route returns 200, the rendered HTML contains the view's primary h1 text, and `Content-Security-Policy` header is present.

- **AC-62** (Event Log Search + Export): `GET /api/agents/:id/events` gains query params `q=<substring>` (ILIKE over `content`), `type=<event_type>`, `since=<event_id>`, `until=<event_id>`, `limit=<n ≤ 1000>`. `GET /api/agents/:id/events.jsonl` streams newline-delimited JSON with the same filters. `GET /api/agents/:id/events.csv` streams CSV with the same filters (columns: `id,created_at,event_type,source,content,tool_name,wake_id`). `pcy events <agent> --search "foo" --type tool_call --format jsonl > out.jsonl` plumbs the CLI. Verified by `tests/event_search_export_test.rs`: seed 200 events, filter by q returns correct subset, jsonl/csv round-trip preserves all rows.

- **AC-63** (Cost Reports): New API `GET /api/agents/:id/cost?group_by=day|model|tool&since=<ts>&until=<ts>` returns `[{bucket, usd_total, call_count}]`. `pcy cost <agent> --group-by model --since 2026-01-01` renders a sortable table. A new `/costs` UI view renders the per-agent breakdown as a stacked bar chart per day. Verified by `tests/cost_report_test.rs`: seed `llm_calls` rows with known usd values, assert grouping math.

- **AC-64** (Event Retention + Export-Then-Prune): A new background job (configurable via `OPEN_PINCERY_RETENTION_DAYS`, default `unlimited`) can prune events older than N days — but only after they are written to an append-only gzipped JSONL archive directory (`OPEN_PINCERY_ARCHIVE_DIR`). A CLI `pcy events archive --older-than 90d --dry-run` reports what would be archived; without `--dry-run` it performs the archive + prune transaction. The archive is **never** deleted by Pincery. Verified by `tests/event_retention_test.rs`: seed old events, run archive, assert rows deleted + archive file contains identical JSONL.

- **AC-65** (Multi-Tenant Declaration): `DELIVERY.md` and `README.md` gain a bold "**Multi-Tenant Support**" section that either (a) declares "Pincery is single-tenant per deployment; running a SaaS requires one deployment per customer" with a link to a future v12 multi-tenant roadmap, OR (b) ships `workspace_memberships` enforcement on every API endpoint with `tests/multi_tenant_test.rs` asserting cross-workspace isolation for events, agents, credentials, and sessions. v9 picks (a) unless (b) is explicitly selected during DESIGN. Verified by `tests/multi_tenant_declaration_test.rs`: the section exists with the exact heading and at least one of the two contracts is fulfilled.

- **AC-66** (Tool Catalog Expansion): In addition to `shell` and `list_credentials`, ship at least: `http_get(url, headers?)` (with per-agent allowlist from a new `agent_http_allowlist` table), `file_read(path)` (scoped to a per-agent writable tempdir, no host FS access), `db_query(connection_name, sql)` (uses a stored credential, read-only enforced by server-side regex). Each new tool has its own integration test asserting the scoping rule. Verified by `tests/tool_catalog_test.rs`: four tools present in the runtime's tool list, each has a passing scoping test.

- **AC-67** (Workspace-Level Rate Limiting): A new config `OPEN_PINCERY_TOOL_CALLS_PER_MINUTE` (default 600) caps total tool calls across all agents in a workspace over a rolling 60-second window. Exceeding emits a `rate_limit_exceeded` event and delays (not fails) the tool call with a 1s backoff. Verified by `tests/workspace_rate_limit_test.rs`: seed 601 tool calls within a minute, assert the 601st is delayed and the event is emitted.

- **AC-68** (Offline / Local LLM Story): `README.md` gains a bullet in the "Stack" section: "**Local LLM**: works out-of-the-box with Ollama by setting `OPEN_PINCERY_LLM_BASE_URL=http://host.docker.internal:11434/v1`. No cloud LLM required for self-hosted operation." A new test `tests/ollama_config_test.rs` (unit, no external deps) asserts the README contains the bullet and the config loader accepts the Ollama URL shape.

- **AC-69** (Version Handshake): `pcy status` output is extended to include `server_version`, `cli_version`, and `compatible: bool`. A new API `GET /api/version` returns `{version, commit_sha, build_time}`. The CLI compares major.minor; mismatched minor versions print a stderr warning but proceed; mismatched major versions refuse with exit code 3. Verified by `tests/version_handshake_test.rs`: build a stubbed server with v0.8.x against a v0.9.x CLI and assert the warning; stub a v1.x server against a v0.x CLI and assert exit 3.

- **AC-70** (Terminology Lock): The README's opening paragraph declares the canonical vocabulary: "A **pincer** is a single continuous AI agent. **Pincery** is the server + CLI that hosts them. `pcy` is the CLI binary." A new lint `tests/terminology_test.rs` asserts that in `README.md`, `DELIVERY.md`, and `docs/api.md`, the words `bot`, `assistant`, and `worker` are never used as synonyms for `pincer` (verified by regex with an explicit allowlist for unrelated contexts like "chatbot" in citations).

### Stack

No new core runtime dependencies beyond what's in v8. The following are added and pinned:

| Concern                          | Choice                           | Why                                                             |
| -------------------------------- | -------------------------------- | --------------------------------------------------------------- |
| Linux sandboxing (AC-53 Option A) | `bubblewrap` (system binary) + seccomp-bpf profile | Mature, battle-tested (used by Flatpak), zero new Rust deps |
| UI framework (AC-61)             | HTMX 1.9 + Pico.css              | Zero-build, ships as ~15KB; no npm pipeline to maintain         |
| Retention archive format (AC-64) | gzipped JSONL, one file per day  | Trivially greppable, resumable, matches event-log schema        |
| Session cookie signing (AC-58)   | Existing vault key (reused)      | No new key-management surface                                   |

### Deployment Target

Unchanged: single Rust binary + PostgreSQL + static assets. The only new operational surface is the **deposit page** served from the same HTTP listener at `/deposit/:token` — no separate service.

### Data Model

New tables:

- `credential_requests (id, agent_id, name, reason, doc_url, status, deposit_token, deposit_token_expires_at, created_at, fulfilled_at, fulfilled_by)` — AC-55
- `agent_http_allowlist (id, agent_id, host_pattern, created_at)` — AC-66

New columns on existing tables:

- `sessions.expires_at timestamptz not null` — AC-58
- `sessions.last_used_at timestamptz` — AC-58
- `users.role text not null default 'admin'` — AC-59

New event types on `events.event_type`:

- `credential_requested` (AC-55), `credential_deposited` (AC-56), `credential_request_rejected` (AC-57)
- `rate_limit_exceeded` (AC-67)
- `sandbox_blocked` (AC-53 Option A)

No destructive schema changes. Every migration is forward-only and additive.

### Estimated Cost

$0 incremental — no new infrastructure required beyond what v8 already uses. The deposit page + retention archive live on the same Postgres + host filesystem.

### Quality Tier

**House** — production-facing trust gate. REVIEW and RECONCILE are mandatory on every AC. Every P0 AC (AC-53 through AC-61) requires an adversarial test, not just a happy-path test. v9 is the version that decides whether Pincery is a prototype or a product.

### Clarifications Needed

1. **AC-53 Option A vs Option B.** Wiring real Bubblewrap sandboxing is 1-2 weeks of engineering with adversarial testing; removing the marketing claim is a day. The solo founder's explicit ask is "don't release until all this is shipped" — interpreting that as Option A. If budget is tight, Option B + a "v10 will ship real sandboxing" commitment is defensible. **Default: Option A. Confirm before DESIGN begins.**
2. **AC-61 UI stack.** HTMX + Pico is zero-build and ships today; Preact + Tailwind is more capable but adds a committed build artifact. **Default: HTMX + Pico** for the zero-build property. The UI rebuild ships six views; additional polish (animations, charts beyond sparklines) is v10.
3. **AC-65 multi-tenant.** True multi-tenant enforcement touches every endpoint and is easily 2 weeks alone. **Default: ship the explicit single-tenant declaration (option a)** and sequence real multi-tenant as v12. The solo founder's use case is one deployment, one workspace — single-tenant is correct for now.
4. **AC-59 roles granularity.** Three roles (admin/operator/viewer) is the SMB baseline. Adding custom roles / ABAC / scoped tokens is v11. Confirm three is enough.

### Deferred to v10+

- **SaaS control plane** (self-service signup, per-tenant billing, invite flows, password reset). v9 ships the RBAC primitive; the SaaS skin is v12.
- **Prompt template editor UI.** Schema exists; v9 ships the event-timeline and credential-inbox views first. Template editor = v10.
- **Real SSE/WebSocket event streaming** (AC-62 uses bounded polling). Pair with v11 signals primitive.
- **Zerobox on macOS (Seatbelt) / Windows.** v9 AC-53 Option A is Linux-only. Cross-platform sandboxing = v12.
- **Custom roles / ABAC.** v9 AC-59 ships fixed admin/operator/viewer. Custom roles = v11.
- **MCP stdio server** (was deferred from v8.1). Pair with v11 signals.
- **pgvector / CozoDB memory** (was v8+ north-star). v9 is security + trust gate only.

### v9 Build Order

The order is sequenced so each slice gates the next and every slice is independently committable + verifiable.

**Phase A — Security Truth (P0, ~1-2 weeks)**

1. **Slice A1 — AC-54 Threat Model + SECURITY.md**: writes the truth about what v8 actually protects. Ships first because every subsequent slice is scoped by this document. (1 day)
2. **Slice A2 — AC-53 Sandbox Truth**: default Option A (real Bubblewrap + seccomp). Highest risk slice — may need to fall back to Option B if the adversarial test can't be stabilized in time. (3-5 days)
3. **Slice A3 — AC-58 Session TTL + Refresh + Revocation**: no new features blocked by it — can ship parallel to A2. (1-2 days)
4. **Slice A4 — AC-59 Users + Roles**: depends on A3's session machinery. (2-3 days)
5. **Slice A5 — AC-60 Auth README Rewrite**: documentation commit. (½ day)

**Phase B — Credential Requests (P0, ~1 week)**

6. **Slice B1 — AC-55 Credential Request Tool + schema**: backend only; agent can emit requests, API can list them. (2 days)
7. **Slice B2 — AC-56 Deposit Page**: HTML form + POST handler + single-use token. (1-2 days)
8. **Slice B3 — AC-57 CLI + UI Inbox**: operator surfaces — depends on B1 + B2. (2 days)

**Phase C — UI Rebuild (P1, ~1 week)**

9. **Slice C1 — AC-61 UI Rebuild on HTMX + Pico**: six views, dark mode, CSP header. (3-5 days)

**Phase D — Observability (P1, ~1 week)**

10. **Slice D1 — AC-62 Event Search + Export**: query params + jsonl/csv streaming. (2 days)
11. **Slice D2 — AC-63 Cost Reports**: grouping API + UI chart. (1-2 days)
12. **Slice D3 — AC-64 Retention + Archive**: background job + CLI. (2 days)

**Phase E — Multi-tenant + Tool Catalog + Polish (P2, ~1 week)**

13. **Slice E1 — AC-65 Multi-Tenant Declaration**: doc commit selecting option (a). (½ day)
14. **Slice E2 — AC-66 Tool Catalog Expansion**: http_get + file_read + db_query with scoping tests. (3 days)
15. **Slice E3 — AC-67 Workspace Rate Limiting**: config + enforcement + event emission. (1 day)
16. **Slice E4 — AC-68 Ollama Bullet**: README + test. (½ day)
17. **Slice E5 — AC-69 Version Handshake**: `/api/version` + CLI mismatch warning. (1 day)
18. **Slice E6 — AC-70 Terminology Lock**: README rewrite + lint test. (½ day)

**Total estimate: 4-6 weeks of focused engineering at one-slice-per-day cadence.** No version ships before Phase A + B are complete (these are the trust gate). Phase C ships as v9.0. Phases D + E ship as v9.1 and v9.2 incrementally, each gated by its own REVIEW + VERIFY.

### v9 Dependencies on Prior Versions

None broken.

- v1 AC-10 bootstrap primitive: **renamed** (`OPEN_PINCERY_BOOTSTRAP_TOKEN` → `OPEN_PINCERY_ADMIN_SEED`). Deprecation alias honoured for v9.0 with warning; removed in v10.
- v7 AC-40 credential vault: **extended** — `credential_requests` table added; existing `credentials` table and API unchanged.
- v8 AC-45 idempotent login: unchanged.
- v8 AC-47 `--output`: every new list endpoint (requests, sessions, users, costs) honours the global `--output` flag.
- v8 AC-52b CLI naming lint: new `credential request`, `session`, `user`, `cost` subcommands added to the lint allowlist.
- v6 AC-36 `ProcessExecutor`: **wrapped** by `SandboxedExecutor` under AC-53 Option A; `tests/sandbox_test.rs` from v6 continues to pass.

### Why v9 Is Worth a Whole Version

A product that says "sandboxed" in marketing but runs `sh` is not a security product. A product where the agent can leak a secret into the event log is not a secrets product. A product where sessions never expire is not an auth product. v9 is the version that lets the solo founder look their CTO / their board / their first enterprise customer in the eye and say "yes, we sandbox; yes, secrets are isolated; yes, sessions rotate; yes, we have RBAC." Every AC above is a specific claim with a specific test. If any AC ships with placeholder behaviour, the version fails.
