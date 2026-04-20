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
- No Caddy/TLS example exists despite `preferences.md` naming Caddy as the self-host default (gap also called out in `docs/input/self_host_readiness.md` §2).

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

- **AC-32** (Secure-By-Default Compose): Out-of-box `docker-compose.yml` publishes the app port bound to `127.0.0.1:8080:8080` (loopback only), not `0.0.0.0:8080`. `.env.example` defaults `OPEN_PINCERY_HOST=127.0.0.1`. No literal default exists for `OPEN_PINCERY_BOOTSTRAP_TOKEN` anywhere — compose refuses to start when unset. Verified by `tests/compose_env_test.rs` assertions on the published ports block and by a `docker compose config` run with empty env that exits non-zero.

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

- **Bootstrap token rotation/expiry rules** (`self_host_readiness.md` §1) — v6
- **`self_host_team` split-topology docs and compose overlay** (readiness §2) — v6
- **Upgrade runbook and backup-encryption guidance** (readiness §5) — v6
- **Local admin lockout recovery and MFA policy** (readiness §3) — v6
- **UI first-run wizard / `pcy status` unbootstrapped auto-detect** — v6
- **Machine-readable `AGENTS.md` for operator agents** — v6
- **Published OCI images on `ghcr.io`** — v6 (pairs with team-topology work)
- **TLA+ enum-name alignment** (runtime raw status strings → spec variant names) — v6 RECONCILE work, carried forward from v4
- **Real signup/login flow, multi-tenant RBAC enforcement, per-workspace rate limits, account suspension** — previously tagged v5 in v4 Deferred; reassigned to v6 because v5 is operator-onramp only
- **CLI auth subcommands beyond `bootstrap`/`login`** — reassigned to v6 with the auth flow above

