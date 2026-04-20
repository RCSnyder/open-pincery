# Readiness: Open Pincery

## Verdict

READY

---

## v1 Truths

- **T-1**: CAS (`UPDATE ... WHERE status = $expected RETURNING *`) is the ONLY mechanism for wake acquisition. No agent transitions to `awake` without winning a CAS race. Concurrent attempts are naturally coalesced.
- **T-2**: The `events` table is append-only. No UPDATE, DELETE, or ALTER operations on event rows. Events are the system of record.
- **T-3**: At most one wake is active per agent at any time. This is enforced structurally by the CAS WHERE clause, not by application-level locking.
- **T-4**: Agent projections (identity, work_list) and wake summaries are versioned INSERT-only rows, never updated in place. Current state is the latest version.
- **T-5**: The prompt is bounded — each component (constitution, summaries, identity, work_list, messages) has a cap. Character-based trim drops oldest messages first. Context cannot grow unboundedly.
- **T-6**: Wake summaries are ≤500 characters each; up to 20 are included in prompt assembly.
- **T-7**: Every LLM call is recorded in `llm_calls` with model, token counts, latency, prompt hash, and response hash. Full prompts optionally stored in `llm_call_prompts`.
- **T-8**: Every tool execution is recorded in the event log (tool_call + tool_result events) and optionally in `tool_audit`.
- **T-9**: The bootstrap endpoint is gated by `OPEN_PINCERY_BOOTSTRAP_TOKEN` from the environment. Without it, no admin user or org can be created.
- **T-10**: DB-persisted agent lifecycle states are exactly `asleep`, `awake`, `maintenance`. Fine-grained TLA+ states (PromptAssembling, ToolDispatching, etc.) are in-memory runtime states, not persisted.
- **T-11**: No secrets or credential values appear in source code. All sensitive config flows through environment variables.
- **T-12**: Shell tool execution uses basic subprocess for v1 (Zerobox deferred to Phase 2). This is a conscious scope decision, not a placeholder.

## v2 Truths

- **T-13**: On SIGTERM/SIGINT the process stops accepting new connections, drains in-flight requests and active wake loops for up to 30 seconds, then exits with code 0. No abrupt termination of running wakes.
- **T-14**: `docker compose up` brings a fully functional stack (app + postgres) from zero. The app container waits for Postgres readiness before accepting traffic.
- **T-15**: Per-IP rate limiting is enforced at the middleware layer for every API route. Bootstrap gets a stricter limit (10 req/min) than authenticated endpoints (60 req/min). Exceeding the limit returns HTTP 429 with `Retry-After` header.
- **T-16**: Webhook payloads are verified with HMAC-SHA256 using a per-agent `webhook_secret`. Invalid signatures are rejected with 401 before any side effects occur.
- **T-17**: Webhook idempotency is enforced via a deduplication table keyed by `X-Idempotency-Key`. Duplicate deliveries return 200 without re-inserting events or re-triggering wakes.
- **T-18**: Disabled agents (`is_enabled = false`) reject wake acquisition at the CAS level. The `WHERE is_enabled = TRUE` clause in `acquire_wake` and `drain_reacquire` guarantees this structurally.
- **T-19**: Soft-delete sets `is_enabled = false` and `disabled_reason = 'deleted'`. No rows are removed from the database. The agent's event log, projections, and wake summaries remain intact.

## v1 Key Links

- **L-1** AC-1 → `models/agent.rs` (acquire_wake, transition_to_maintenance, release_to_asleep, drain_reacquire CAS functions) → `tests/lifecycle_test.rs` → Runtime proof: two concurrent wake attempts, exactly one wins
- **L-2** AC-2 → `models/event.rs` (append_event, recent_events) + migration `create_events.sql` → `tests/event_log_test.rs` → Runtime proof: send message → wake → query event log → verify complete sequence with ordering
- **L-3** AC-3 → `runtime/prompt.rs` (AssembledPrompt struct) + `models/projection.rs` + `models/prompt_template.rs` → `tests/prompt_test.rs` → Runtime proof: create agent with known projections/events → assemble prompt → verify all 6 components present and ordered
- **L-4** AC-4 → `runtime/wake_loop.rs` + `runtime/llm.rs` (LlmClient) + `runtime/tools.rs` (dispatch_tool) → `tests/wake_loop_test.rs` → Runtime proof: mock LLM returns tool_call then stop → verify events, iteration count, termination
- **L-5** AC-5 → `runtime/maintenance.rs` + `models/projection.rs` (new versioned rows) + wake_summaries → `tests/maintenance_test.rs` → Runtime proof: after wake, query projection versions and wake_summary table → confirm new rows with content
- **L-6** AC-6 → `api/agents.rs` + `api/messages.rs` + `api/events.rs` + `api/bootstrap.rs` → `tests/api_test.rs` → Runtime proof: exercise each endpoint with curl/reqwest → verify status codes and JSON response shapes
- **L-7** AC-7 → `background/listener.rs` + Postgres LISTEN/NOTIFY → `tests/trigger_test.rs` → Runtime proof: send message to resting agent → measure wake latency < 5 seconds
- **L-8** AC-8 → `background/stale.rs` → `tests/stale_test.rs` → Runtime proof: set agent to `awake` with wake_started_at 3 hours ago → run recovery job → verify status = `asleep` + `stale_wake_recovery` event
- **L-9** AC-9 → `runtime/drain.rs` + `models/event.rs` (has_pending_events) + `models/agent.rs` (drain_reacquire) → `tests/drain_test.rs` → Runtime proof: send message during active wake → confirm drain check triggers re-acquire after maintenance
- **L-10** AC-10 → `api/bootstrap.rs` + `db.rs` (migration runner) → `tests/bootstrap_test.rs` → Runtime proof: start with empty DB → call bootstrap endpoint → verify user, org, workspace rows created

## v2 Key Links

- **L-11** AC-11 → `main.rs` (tokio signal handler + `CancellationToken`) + axum `with_graceful_shutdown` + `background/listener.rs` + `background/stale.rs` (token-aware loops) → `tests/shutdown_test.rs` → Runtime proof: start server, trigger a wake via message, send SIGTERM, confirm wake completes and process exits with code 0 within 30s
- **L-12** AC-12 → `Dockerfile` (multi-stage build) + `docker-compose.yml` (app + postgres services, healthcheck) → Manual verification → Runtime proof: `docker compose up` from clean state, curl `GET /health` returns `{"status":"ok"}` within 60s
- **L-13** AC-13 → `api/mod.rs` (custom `governor` middleware with `RateLimiter<IpAddr, DashMapStateStore<IpAddr, DefaultClock>>`, two rate-limit tiers hardcoded in `AppState::new()`) → `tests/rate_limit_test.rs` → Runtime proof: send 61 requests in rapid succession to an authenticated endpoint, confirm 61st returns 429 with `Retry-After` header
- **L-14** AC-14 → `api/webhooks.rs` (HMAC-SHA256 verify, idempotency dedup) + `models/agent.rs` (`webhook_secret` column) + migration `add_webhook_secrets.sql` + migration `create_webhook_dedup.sql` + `models/event.rs` (append `webhook_received` event) → `tests/webhook_test.rs` → Runtime proof: send signed webhook → confirm `webhook_received` event in log + wake triggered; resend same webhook → confirm 200 without duplicate event; send bad signature → confirm 401
- **L-15** AC-15 → `api/agents.rs` (PATCH + DELETE handlers) + `models/agent.rs` (`update_agent`, `soft_delete_agent`) → `tests/agent_mgmt_test.rs` → Runtime proof: PATCH to disable agent → send message → confirm no wake occurs; PATCH to rename → confirm name changed; DELETE → confirm `is_enabled = false` and `disabled_reason = 'deleted'`

## Acceptance Criteria Coverage

| AC ID | Build Slice                      | Planned Test                                                                                                                                                                                                                                                                                   | Planned Runtime Proof                                                                                                                                                                               | Notes                                                                                                             |
| ----- | -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| AC-1  | Slice 3: CAS lifecycle functions | `lifecycle_test.rs` — two concurrent `acquire_wake` calls; assert exactly one returns `Some`, one returns `None`; test all four transitions (asleep→awake, awake→maintenance, maintenance→asleep, maintenance→awake)                                                                           | Run two tokio tasks racing `acquire_wake`; verify via DB query that only one wake_id is set                                                                                                         | Requires real Postgres; CAS relies on DB atomicity                                                                |
| AC-2  | Slice 2: Event log model         | `event_log_test.rs` — insert events, query back, verify sequence; attempt UPDATE/DELETE and confirm rejection (DB constraint or no API)                                                                                                                                                        | Send message via API → agent wakes → query `/api/agents/:id/events` → verify event_type ordering: message_received → wake_start → tool_call → tool_result → wake_end                                | Consider adding a DB trigger or CHECK to enforce append-only, or rely on no-UPDATE/DELETE in code                 |
| AC-3  | Slice 5: Prompt assembly         | `prompt_test.rs` — create agent with seed projections, 25 events, 3 wake summaries, active prompt template → assemble prompt → assert system_prompt contains all 6 components in order; assert messages ≤ 200; assert character trim works                                                     | Assemble prompt for a known agent and inspect output structure. Verify oldest messages dropped first when oversized                                                                                 | Depends on projections, wake_summaries, and prompt_templates tables existing                                      |
| AC-4  | Slice 6: Wake loop               | `wake_loop_test.rs` — mock LLM via wiremock → tool_calls response → dispatch shell (echo "hello") → result → LLM returns text → implicit sleep; separate test hitting iteration cap at 50                                                                                                      | End-to-end: send message to agent via API → background listener triggers wake → LLM mock responds → verify events sequence → agent returns to asleep                                                | Largest slice; may approach 400-line exception for wake_loop.rs                                                   |
| AC-5  | Slice 7: Maintenance cycle       | `maintenance_test.rs` — mock maintenance LLM call → provide previous identity/work_list/transcript → verify new projection rows inserted (version incremented) + wake_summary row ≤500 chars                                                                                                   | After a complete wake cycle, query `agent_projections` ORDER BY version DESC LIMIT 2 → confirm version incremented; query `wake_summaries` → confirm new entry                                      | Maintenance uses a separate LLM model config (`LLM_MAINTENANCE_MODEL`)                                            |
| AC-6  | Slice 10: HTTP API               | `api_test.rs` — test each of 6 endpoints: POST /api/agents (201), GET /api/agents (200 array), GET /api/agents/:id (200 with projections), POST /api/agents/:id/messages (202), GET /api/agents/:id/events (200 with events+total), GET /health (200)                                          | Curl each endpoint against running server; verify JSON shapes match design contracts                                                                                                                | Auth via session_token from bootstrap; 401 on missing/invalid token                                               |
| AC-7  | Slice 4: Wake triggers           | `trigger_test.rs` — insert message_received event → issue NOTIFY → assert listener receives and spawns wake task within 5s                                                                                                                                                                     | Send POST /api/agents/:id/messages → measure wall-clock time to wake_start event appearing in event log; assert < 5 seconds                                                                         | Requires real Postgres LISTEN/NOTIFY; cannot mock                                                                 |
| AC-8  | Slice 9: Stale wake recovery     | `stale_test.rs` — set agent status='awake', wake_started_at=NOW()-3h → run stale recovery job → assert status='asleep', wake_id=NULL, stale_wake_recovery event exists                                                                                                                         | Start stale recovery background task → manipulate time or directly set stale timestamp → verify recovery within one job cycle                                                                       | Tests both `awake` and `maintenance` stale states per AC-8                                                        |
| AC-9  | Slice 8: Drain check             | `drain_test.rs` — complete wake → enter maintenance → insert message_received during maintenance → drain check finds event → assert drain_reacquire CAS succeeds → new wake starts                                                                                                             | Send message mid-wake → let original wake complete + maintenance → verify a second wake starts without a new NOTIFY by checking for two wake_start events with same message visible                 | Most complex sequencing; needs careful test setup with timing control                                             |
| AC-10 | Slice 1: Bootstrap + migrations  | `bootstrap_test.rs` — start with empty DB → run migrations → POST /api/bootstrap with correct token → verify 201 + user/org/workspace rows; repeat call → verify 409 or idempotent behavior                                                                                                    | Start binary against empty Postgres → call bootstrap → query tables directly → confirm rows exist with expected roles                                                                               | First slice; all other slices depend on this working                                                              |
| AC-11 | v2 Slice 1: Graceful shutdown    | `shutdown_test.rs` — start server in a child process, send a message to trigger wake, send SIGTERM, assert process exits 0 within 30s; assert no partial wake events (wake_end must exist if wake_start exists)                                                                                | Start server, trigger wake, send SIGTERM via `nix::sys::signal` or `tokio::signal`, confirm clean exit with code 0 and all background tasks stopped                                                 | Requires `CancellationToken` threaded through listener + stale + wake_loop; must also test SIGINT path            |
| AC-12 | v2 Slice 2: Docker Compose       | Manual test — `Dockerfile` multi-stage build + `docker-compose.yml` with app + postgres services + healthcheck                                                                                                                                                                                 | `docker compose up` from clean state → curl `GET /health` → assert `{"status":"ok"}` within 60s; `docker compose down` clean                                                                        | No automated test — Docker-in-Docker is out of scope; verified manually during BUILD                              |
| AC-13 | v2 Slice 3: Rate limiting        | `rate_limit_test.rs` — send 61 requests to authenticated endpoint → assert first 60 return 200, 61st returns 429 with `Retry-After` header; separate test for bootstrap at 10 req/min                                                                                                          | Start server, burst 61 authenticated requests → verify 429 + header; burst 11 bootstrap requests → verify 429                                                                                       | Needs `tower-governor` or equivalent crate added to `Cargo.toml`                                                  |
| AC-14 | v2 Slice 4: Webhook ingress      | `webhook_test.rs` — compute HMAC-SHA256 of payload with agent webhook_secret, send POST with valid signature → assert 202 + `webhook_received` event; send with bad signature → assert 401; resend with same idempotency key → assert 200 without duplicate event                              | Send signed webhook to `/api/agents/:id/webhooks` → query event log → confirm `webhook_received` with correct content; resend → confirm no new event row; bad signature → confirm 401 with no event | Requires 2 new migrations (webhook_secret column + dedup table)                                                   |
| AC-15 | v2 Slice 5: Agent management     | `agent_mgmt_test.rs` — PATCH disable agent → assert `is_enabled = false`; send message → assert no wake (acquire_wake returns None); PATCH enable → send message → assert wake succeeds; PATCH rename → assert name changed; DELETE → assert `is_enabled = false, disabled_reason = 'deleted'` | PATCH to disable → POST message → query agent → confirm still asleep; PATCH enable → POST message → confirm wake; DELETE → confirm soft-delete fields                                               | CAS `WHERE is_enabled = TRUE` already in v1 acquire_wake — just needs the management endpoints to toggle the flag |

## Scope Reduction Risks

### v1 Risks (carried forward)

- **Shell tool becomes a no-op stub**: AC-4 requires the agent to "make at least one tool call." If the shell tool is implemented as a stub that returns a canned string without actually executing a subprocess, the wake loop appears to work but the system is not a real agent runtime. BUILD must implement `tokio::process::Command` with stdout/stderr capture, timeout, and exit code recording.

- **Prompt assembly skips character trim**: AC-3 specifies "character-based trim drops oldest messages first." If BUILD omits the trim logic and simply includes all messages, the criterion technically passes for small test cases but breaks at scale. BUILD must implement an explicit `max_prompt_chars` config with trim logic.

- **Drain check returns false unconditionally**: AC-9 is the most complex sequencing criterion. It's tempting to always return "no pending events" and skip re-acquisition. The test must insert a message during an active wake and verify a second wake starts.

- **Maintenance LLM call returns identity/work_list unchanged**: AC-5 requires the maintenance to return "updated" projections. If the mock always returns the input verbatim, it doesn't test that new versioned rows are written. The mock must return _different_ content and the test must verify the delta.

- **Session auth becomes a pass-through**: AC-6 includes `Authorization: Bearer <session_token>` on all endpoints. If auth middleware is skipped or always returns 200, the bootstrap token gating (AC-10) is meaningless. BUILD must implement real session lookup from `user_sessions` table.

- **LLM call auditing silently dropped**: T-7 requires every LLM call to be recorded. If `llm_calls` inserts are commented out or made optional-and-never-enabled, observability is gone. Tests must query `llm_calls` after wake + maintenance and verify rows exist.

### v2 Risks

- **Graceful shutdown waits 0 seconds**: AC-11 requires a 30-second drain window. If shutdown immediately cancels all tasks (e.g. token is cancelled without awaiting in-flight work), active wakes are terminated mid-flight. The test must start a wake, send SIGTERM, and confirm the wake completes before exit.

- **Rate limiting applied but with wrong limits or missing `Retry-After`**: AC-13 specifies exact thresholds (60/min auth, 10/min bootstrap) and requires `Retry-After` header. A rate limiter that uses different defaults or omits the header technically fails the criterion even though limiting occurs.

- **Webhook HMAC check uses timing-unsafe comparison**: T-16 requires HMAC-SHA256 verification. Using `==` instead of constant-time comparison introduces a timing side-channel. BUILD must use `hmac` crate's `verify_slice` or equivalent constant-time comparison.

- **Webhook dedup table grows unboundedly**: The idempotency dedup table has no TTL or cleanup. For v2, this is acceptable (mentioned as a bounded assumption below), but BUILD should not silently skip the dedup insert.

- **Agent soft-delete actually hard-deletes**: AC-15 requires `DELETE` to set `is_enabled = false` and `disabled_reason = 'deleted'`, not to remove the row. The test must query the agent after DELETE and confirm the row still exists with correct fields.

## Clarifications Needed

### v1 Clarifications (carried forward)

- **Prompt character budget**: AC-3 says "character-based trim" but no specific limit is defined in scope or design. **Bounded assumption**: use a configurable `MAX_PROMPT_CHARS` env var defaulting to 100,000 characters. This does not change the pass/fail of AC-3.

- **Append-only enforcement mechanism**: AC-2 says events are "never updated or deleted." The design enforces this by simply never issuing UPDATE/DELETE in application code. **Bounded assumption**: application-level enforcement is sufficient for v1.

- **Bootstrap idempotency**: AC-10 says "on first run with an empty database" but doesn't specify behavior on repeat calls. **Bounded assumption**: the bootstrap endpoint returns 409 Conflict if bootstrap has already been completed.

### v2 Clarifications

- **Webhook dedup TTL**: The dedup table stores idempotency keys but scope does not specify a retention window. **Bounded assumption**: keys are stored indefinitely for v2; a TTL cleanup job is deferred to v3. This does not change AC-14's pass/fail — deduplication works regardless of retention policy.

- **Rate limit scope (per-IP vs per-token)**: AC-13 says "per-IP." This means unauthenticated and authenticated requests from the same IP share no state — they are tracked on separate middleware instances with different limits. **Bounded assumption**: `governor::RateLimiter<IpAddr, DashMapStateStore<IpAddr>, DefaultClock>` keyed by `ConnectInfo<SocketAddr>` peer IP, two `KeyedRateLimiter` instances with hardcoded configs in `AppState::new()`. Configurable rate limits deferred to v3.

- **Shutdown signal on Windows**: SIGTERM/SIGINT are Unix signals. On Windows, `tokio::signal::ctrl_c()` covers Ctrl+C. **Bounded assumption**: v2 uses `tokio::signal::ctrl_c()` for cross-platform support plus Unix-specific SIGTERM handling behind `#[cfg(unix)]`. This does not change AC-11's pass/fail — the test verifies the behavior, not the signal type.

- **Webhook secret provisioning**: AC-14 mentions "per-agent webhook secret" but scope defers UI management of webhook secrets to v3. **Bounded assumption**: `webhook_secret` is auto-generated (random 32-byte hex) when the agent is created and returned in the create-agent response. API consumers read it once and configure their webhook source. No rotation API in v2.

## Build Order

### v1 Slices (completed)

1. **AC-10 — Bootstrap + Migrations** (Slice 1)
2. **AC-2 — Event Log** (Slice 2)
3. **AC-1 — CAS Lifecycle** (Slice 3)
4. **AC-7 — Wake Triggers** (Slice 4)
5. **AC-3 — Prompt Assembly** (Slice 5)
6. **AC-4 — Wake Loop** (Slice 6)
7. **AC-5 — Maintenance Cycle** (Slice 7)
8. **AC-9 — Drain Check** (Slice 8)
9. **AC-8 — Stale Wake Recovery** (Slice 9)
10. **AC-6 — HTTP API** (Slice 10)

### v2 Slices (appended)

11. **AC-11 — Graceful Shutdown** (v2 Slice 1): Add `tokio_util::sync::CancellationToken` to `main.rs`. Wire `tokio::signal` for SIGTERM/SIGINT. Thread token through background listener and stale recovery task loops. Use `axum::serve(...).with_graceful_shutdown(...)` for HTTP. Wait up to 30s for in-flight wake loops via `JoinSet` or task tracking. Files: `main.rs`, `background/listener.rs`, `background/stale.rs`, `Cargo.toml` (add `tokio-util`). Foundation for all other v2 slices (clean shutdown needed before Docker).

12. **AC-12 — Docker Compose** (v2 Slice 2): Create `Dockerfile` (multi-stage: builder with cargo-chef + runtime with Debian slim). Update `docker-compose.yml` to add app service with healthcheck, depends_on postgres with condition `service_healthy`, env vars. Files: `Dockerfile`, `docker-compose.yml`. No Rust code changes. Manual verification only.

13. **AC-13 — Rate Limiting** (v2 Slice 3): Add `governor` to `Cargo.toml`. Create two `KeyedRateLimiter` instances in `AppState::new()` with hardcoded limits (10/min bootstrap, 60/min auth). Implement `unauth_rate_limit` and `auth_rate_limit` as custom axum middleware functions in `api/mod.rs` that return `Response` with `Retry-After` header on 429. Files: `api/mod.rs`, `Cargo.toml`. Test: `rate_limit_test.rs`.

14. **AC-14 — Webhook Ingress** (v2 Slice 4): Create `api/webhooks.rs` with HMAC-SHA256 verification, idempotency dedup lookup/insert, event append + NOTIFY. Add two migrations: `add_webhook_secrets.sql` (add `webhook_secret` column to agents), `create_webhook_dedup.sql` (idempotency key table). Register route in `api/mod.rs`. Add `hmac` + `sha2` crates (sha2 already present). Files: `api/webhooks.rs`, `api/mod.rs`, 2 migrations, `models/agent.rs` (expose webhook_secret in create). Test: `webhook_test.rs`.

15. **AC-15 — Agent Management** (v2 Slice 5): Add `update_agent` and `soft_delete_agent` functions to `models/agent.rs`. Add PATCH and DELETE handlers to `api/agents.rs`. Wire routes in agent router. Files: `models/agent.rs`, `api/agents.rs`. Test: `agent_mgmt_test.rs`. Depends on AC-11 implicitly (disabled wake rejection uses existing CAS WHERE clause).

## Complexity Exceptions

### v1 Exceptions (carried forward)

- **`wake_loop.rs` may exceed 300 lines (target ≤400)**: The LLM interaction loop, tool dispatch, iteration cap, event recording, and error handling are tightly coupled in a single control flow.

- **13+ migration files**: Each TLA+-specified table gets its own migration per preferences.md convention.

- **Slice 6 (Wake Loop) touches ~5 files**: Inherently coupled — the wake loop, LLM client, and tool dispatch form a single vertical slice.

### v2 Exceptions

- **v2 Slice 1 (Graceful Shutdown) touches 4 files**: `main.rs`, `background/listener.rs`, `background/stale.rs`, `Cargo.toml`. The `CancellationToken` must be threaded into every long-running task, making this inherently cross-cutting. Each file change is small (adding token parameter + select! on cancellation).

---

## v3 Truths

- **T-18**: `.github/workflows/ci.yml` runs fmt, clippy (`-D warnings`), tests (against a real Postgres 16 service container), and `cargo deny check` on every push and pull request. CI is the authoritative gate — local "it compiles on my machine" is no longer sufficient.
- **T-19**: Logging format is controlled by `LOG_FORMAT`. Default is human-readable; `LOG_FORMAT=json` produces one JSON object per line with `timestamp`, `level`, `target`, `message`, and span context fields. Other values fall back to the default.
- **T-20**: The Prometheus metrics listener is opt-in. When `METRICS_ADDR` is unset, no listener is started and no extra port is bound. When set, a separate axum app binds to that address and serves only `GET /metrics`. The main API port is never used for `/metrics`.
- **T-21**: `/health` is liveness only (returns 200 whenever the process is up). `/ready` is readiness (returns 200 only when DB reachable, migrations applied, background tasks running; 503 otherwise with `failing` field naming the failed check). Load balancers and container orchestrators should use `/ready` for traffic routing; `/health` for restart decisions.
- **T-22**: Release artifacts for tags matching `v*` are reproducibly built with LTO + strip, signed with cosign keyless (GitHub OIDC), and accompanied by a CycloneDX SBOM and SHA-256 checksums. Consumers can verify provenance with `cosign verify-blob` without possessing any long-lived keys.
- **T-23**: Operator knowledge for stale wakes, DB restore, migration rollback, rate-limit tuning, and webhook debugging lives in `docs/runbooks/` as markdown files with Symptom / Diagnostic Commands / Remediation / Escalation sections. Not tribal, not Slack threads, not in the head of whoever wrote it.

## v3 Key Links

- **AC-16 → CI workflow → `.github/workflows/ci.yml` → green run proof** (GitHub Actions log)
- **AC-17 → `src/observability/logging.rs::init_logging` → `tests/observability_test.rs::json_log_format_emits_valid_json` → stdout capture with `LOG_FORMAT=json` parsed as JSON**
- **AC-18 → `src/observability/metrics.rs` + `src/observability/server.rs` → `tests/observability_test.rs::metrics_endpoint_exposes_wake_counters` → curl `/metrics` after triggering a wake, grep for counter > 0**
- **AC-19 → `src/api/health.rs::{health, ready}` → `tests/health_test.rs::ready_returns_503_when_db_unreachable` → stop DB pool, call `/ready`, expect 503 with failing=database**
- **AC-20 → `.github/workflows/release.yml` + `.cargo/config.toml` → manual tag `v0.3.0-rc1` → `cosign verify-blob` on downloaded artifact**
- **AC-21 → `docs/runbooks/*.md` (5 files) → REVIEW agent confirms structure + concrete commands**

## v3 Acceptance Criteria Coverage

| AC    | Truth(s) | Planned test                                                                               | Planned runtime proof                                                                                                           |
| ----- | -------- | ------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| AC-16 | T-18     | CI workflow itself running green                                                           | GitHub Actions log showing all 4 steps pass                                                                                     |
| AC-17 | T-19     | `observability_test.rs::json_log_format_emits_valid_json`                                  | Start server `LOG_FORMAT=json`, capture stderr, each line `serde_json::from_str::<Value>` succeeds                              |
| AC-18 | T-20     | `observability_test.rs::metrics_endpoint_exposes_wake_counters`                            | `METRICS_ADDR=127.0.0.1:0`, trigger wake, GET `/metrics`, parse Prometheus text, confirm `open_pincery_wake_started_total >= 1` |
| AC-19 | T-21     | `health_test.rs::health_stays_up_when_db_down`, `health_test.rs::ready_fails_when_db_down` | Close pool, call `/health` → 200, call `/ready` → 503 with `{"failing":"database"}`                                             |
| AC-20 | T-22     | Release workflow YAML lint + test release on RC tag                                        | Download `open-pincery-v0.3.0-rc1-linux-x86_64`, run `cosign verify-blob --certificate-identity-regexp ...`, exit 0             |
| AC-21 | T-23     | REVIEW agent validates all 5 files exist with 4 required sections and concrete commands    | `grep -l "^## Symptom" docs/runbooks/*.md \| wc -l` equals 5                                                                    |

## v3 Scope Reduction Risks

- **AC-18 (metrics)**: Tempting to skip the histogram (wake duration) if `metrics-exporter-prometheus` bucket config is fiddly. The histogram is in scope. Default buckets are acceptable — we do not need to tune them.
- **AC-19 (readiness)**: Tempting to check only the DB pool and skip the background-tasks check. Per-task liveness flags (`listener_alive` + `stale_alive`) must both be present and AND'd. A server with a dead listener — even with a healthy stale-recovery task — should fail readiness with `failing: "background_task:listener"`.
- **AC-20 (release)**: Tempting to ship unsigned binaries to "get the first release out." This AC is signed-or-not-shipped. If cosign keyless breaks on first attempt, we debug it rather than skip signing.
- **AC-21 (runbooks)**: Tempting to write thin runbooks with vague prose. Diagnostic commands must be concrete copy-paste shell invocations. Review will catch and reject vague runbooks.
- **AC-17 (logging)**: Tempting to leave existing ad-hoc `println!` calls (if any) untouched. Any stdout writes from runtime code must go through `tracing` so they respect the JSON toggle. Audit during BUILD.

## v3 Clarifications Needed

None.

## v3 Build Order

1. **AC-17 — JSON logging** (v3 Slice 1): Minimal, self-contained. `src/observability/logging.rs` + wire in `main.rs`. Unblocks observable BUILD for later slices.
2. **AC-19 — Health/ready split** (v3 Slice 2): Small, self-contained. Before metrics because readiness needs the per-task liveness flags (`listener_alive` + `stale_alive`, each guarded by an `AliveGuard` RAII that resets on any exit path) which we wire once and then reuse.
3. **AC-18 — Metrics** (v3 Slice 3): Depends on the observability module scaffolding from Slice 1 and the `AppState` plumbing from Slice 2. Add counters incrementally across existing runtime files.
4. **AC-16 — CI workflow** (v3 Slice 4): Written once the new code exists so CI actually runs the new tests. Includes `deny.toml` with an initial conservative allow-list.
5. **AC-21 — Runbooks** (v3 Slice 5): Pure docs, parallelizable. Uses the metrics and health endpoints from earlier slices in its diagnostic commands.
6. **AC-20 — Release workflow + SBOM** (v3 Slice 6): Last. Needs `.cargo/config.toml` release profile, `deny.toml` already in place, and tested on a real tag. This is the most operator-facing change and should follow everything else being green.

## v3 Complexity Exceptions

None. All slices are small, self-contained, and within the build-discipline slice-size limits (≤5 files, ≤100 lines before verification).

## Verdict (v3)

READY. Proceed to BUILD.

---

## Verdict (v4)

READY. Proceed to BUILD.

## v4 Truths

- **T-24**: The runtime container's final image runs as a non-root user `pcy` (UID 10001, GID 10001). The `Dockerfile` runtime stage ends with `USER pcy`; the application binary, `/app/migrations`, and `/app/static` are owned `pcy:pcy`. Writes outside the user-owned tree (e.g. `/etc`) fail with permission denied inside the running container.
- **T-25**: Wake acquisition is gated by budget. When `agents.budget_limit_usd > 0` AND `agents.budget_used_usd >= agents.budget_limit_usd`, the runtime — in `src/background/listener.rs` **before** calling `agent::acquire_wake` — appends exactly one `budget_exceeded` event (`source='runtime'`, payload `{"limit_usd":…,"used_usd":…}`) and returns without issuing any LLM call or writing a `wake_started` event. The agent's `status` remains `asleep`. `budget_limit_usd = 0` means unlimited (explicit escape hatch; `NULL` is not used — column is `NOT NULL DEFAULT 10.0`).
- **T-26**: Every successful LLM call increments `agents.budget_used_usd` by the call's `cost_usd` in the **same transaction** as the `INSERT INTO llm_calls` row. There is no code path that writes an `llm_calls` row without also bumping `budget_used_usd`, and no code path that bumps `budget_used_usd` without an `llm_calls` row.
- **T-27**: `POST /api/agents/:id/webhook/rotate` is mounted under the existing `auth_middleware` and workspace-scoped exactly like the other agent-management endpoints. It returns `200 {"webhook_secret":"<new-base64-no-pad-32>"}` exactly once. The new secret atomically replaces `agents.webhook_secret`. After rotation, an HMAC computed with the old secret against `POST /api/agents/:id/webhooks` returns `401`; the new secret returns `202`. A `webhook_secret_rotated` event (`source='api'`, no secret material in payload) is appended.
- **T-28**: A second binary `pcy` is declared in the existing `Cargo.toml` via `[[bin]]` pointing at `src/bin/pcy.rs`. `src/bin/pcy.rs` contains only a thin `fn main() -> ExitCode { open_pincery::cli::run() }` shim. All command logic lives in `src/cli/**`. The CLI and integration tests share a single HTTP client module at `src/api_client.rs`. No direct `reqwest` calls are made from CLI command files — they go through `Client`.
- **T-29**: The browser UI is vanilla JavaScript served statically: no bundler, no build step, no framework, no TypeScript, no external CDN fetches. It consists of `static/index.html`, `static/app.js`, `static/style.css`. Five hash-routed views exist: `#/login`, `#/agents`, `#/agents/:id`, `#/agents/:id/settings`, plus the send-message form on the detail view. The live event stream is a real long-poll against `GET /api/agents/:id/events?since=<last_id>` (4s interval, exponential backoff on error capped at 32s).
- **T-30**: `docs/api.md` is the HTTP API contract. It documents every endpoint the `pcy` CLI or browser UI calls. It declares the v4 surface **stable through v5**: endpoints may be added, but none will be removed or renamed, and documented request/response field types will not change incompatibly without a major version bump. Every endpoint reachable from `src/api/` that CLI/UI consume is documented; every documented endpoint exists in `src/api/`.
- **T-31**: v4 introduces **no database schema changes**. `agents.budget_limit_usd` and `agents.budget_used_usd` already exist since v1. No new migrations land in v4.

## v4 Key Links

- **L-16** AC-22 → `Dockerfile` (runtime stage: non-root user `pcy` UID 10001, `USER pcy`, `--chown=pcy:pcy` on COPY) → `tests/docker_nonroot_test.sh` (shell, gated by `DOCKER_AVAILABLE=1`) → Runtime proof: `docker compose up -d` → `docker compose exec app id -u` returns `10001`; `docker compose exec app touch /etc/x` returns non-zero; `curl http://localhost:8080/health` returns 200.
- **L-17** AC-23 → `src/background/listener.rs` (pre-acquire budget check; appends `budget_exceeded` event, returns before `agent::acquire_wake`) + `src/runtime/llm.rs` (LLM-call record transaction increments `agents.budget_used_usd` alongside `llm_calls` insert) → `tests/budget_test.rs` → Runtime proof: seed agent with `budget_limit_usd = 0.000001`, `budget_used_usd = 0.000002`, POST a message, wait for listener to process, assert `SELECT count(*) FROM llm_calls WHERE agent_id=…` unchanged, `SELECT count(*) FROM events WHERE agent_id=… AND event_type='budget_exceeded'` == 1, `agents.status = 'asleep'`.
- **L-18** AC-24 → `src/api/webhook_rotate.rs` (handler `rotate_webhook_secret`) + `src/api/agents.rs::router()` (new `POST /agents/{id}/webhook/rotate` route under `auth_middleware`) + `src/models/event.rs` (append `webhook_secret_rotated`) → `tests/webhook_rotate_test.rs` → Runtime proof: create agent, capture original secret from create response, POST rotate → assert 200 + new secret in body; send signed webhook using old secret → assert 401; send signed webhook using new secret → assert 202; query events → assert one `webhook_secret_rotated` row with empty/NULL payload.
- **L-19** AC-25 → `src/bin/pcy.rs` (thin shim) + `src/cli/mod.rs` (clap `Parser`/`Subcommand`, dispatch) + `src/cli/config.rs` (read/write `~/.config/open-pincery/config.toml` via `dirs::config_dir()`) + `src/cli/commands/{bootstrap,login,agent,message,events,budget,status}.rs` + `src/api_client.rs` (shared HTTP client) + `Cargo.toml` (second `[[bin]]`, `clap`, `toml`, `dirs` deps) → `tests/cli_e2e_test.rs` (`assert_cmd` against a live test server) → Runtime proof: `pcy bootstrap` against running server → config file written with token; `pcy agent create` → agent exists; `pcy message <id> hello` → message event appears; `pcy events <id> --tail` → stream shows wake events; `pcy agent rotate-secret <id>` → new secret printed; `pcy status` → exits 0.
- **L-20** AC-26 → `static/index.html` (SPA shell) + `static/app.js` (hash router, 5 views, long-poll event stream) + `static/style.css` (minimal reset + utility) — served by existing axum static handler → `tests/ui_smoke_test.rs` → Runtime proof: GET `/` returns 200 with `index.html` body containing `#app`; GET `/app.js` returns 200; `app.js` source grep-asserts `#/login`, `#/agents`, `fetch(API + '/api/agents')`, `since=`; headless probe (optional, gated) drives login → list → detail and asserts a `wake_started` event surfaces in the stream within 5s of posting the message form; rotate button issues `POST /api/agents/:id/webhook/rotate` and `agents.webhook_secret` changes.
- **L-21** AC-27 → `docs/api.md` (one section per public endpoint: method + path, required headers, request body typed fields, response body per status code, side effects) → REVIEW subagent pass (no automated test; cross-reference against `src/api/`) → Runtime proof: REVIEW lists every route registered in `src/api/mod.rs` that CLI/UI call and confirms a matching section exists in `docs/api.md`; stability banner (`Stability: v4 → v5 compatible`) present at the top.

## v4 Acceptance Criteria Coverage

| AC    | Component                                                                                                      | Test                                                           | Runtime Proof                                                                                                                                                                                                                  |
| ----- | -------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| AC-22 | `Dockerfile` (runtime stage, non-root `pcy` UID 10001)                                                         | `tests/docker_nonroot_test.sh` (gated by `DOCKER_AVAILABLE=1`) | `docker compose exec app id -u` → `10001`; `docker compose exec app touch /etc/x` → non-zero; `curl /health` → 200                                                                                                             |
| AC-23 | `src/background/listener.rs` (pre-CAS budget check) + `src/runtime/llm.rs` (in-tx `budget_used_usd` increment) | `tests/budget_test.rs`                                         | Seed `budget_limit_usd=0.000001`, `budget_used_usd=0.000002`; POST message; assert 0 new `llm_calls`, exactly 1 new `budget_exceeded` event, `agents.status='asleep'`                                                          |
| AC-24 | `src/api/webhook_rotate.rs` + route registered in `src/api/agents.rs::router()` under `auth_middleware`        | `tests/webhook_rotate_test.rs`                                 | POST rotate → 200 + new secret returned once; POST webhook signed with old secret → 401; POST webhook signed with new secret → 202; `webhook_secret_rotated` event present                                                     |
| AC-25 | `src/bin/pcy.rs` (thin) + `src/cli/**` + `src/api_client.rs` + `[[bin]] pcy` in `Cargo.toml`                   | `tests/cli_e2e_test.rs` (`assert_cmd`)                         | End-to-end shell flow: `pcy bootstrap` → `pcy agent create` → `pcy message` → `pcy events --tail` → `pcy agent rotate-secret` → `pcy status` exits 0; no direct `curl` in test                                                 |
| AC-26 | `static/index.html` + `static/app.js` + `static/style.css`                                                     | `tests/ui_smoke_test.rs`                                       | GET `/` returns SPA shell; `app.js` exports 5 hash routes and calls `GET /api/agents/:id/events?since=…`; posted message surfaces `wake_started` via long-poll within 5s; rotate-secret button mutates `agents.webhook_secret` |
| AC-27 | `docs/api.md`                                                                                                  | REVIEW subagent cross-reference                                | Every endpoint called from `src/cli/**` or `static/app.js` is documented; every documented endpoint is registered in `src/api/mod.rs`; stability statement (`v4 → v5 compatible`) present                                      |

## v4 Scope Reduction Risks

- **AC-22 — USER directive dropped or set to root**: Tempting to use `USER root:root` or omit `USER` entirely "temporarily" if bind/permission issues arise during BUILD. The acceptance criterion requires `id -u` to return `10001` inside the container. Any "just for now" root fallback fails AC-22 and MUST be surfaced as a scope clarification, not committed.
- **AC-22 — Ownership not applied to `/app/static`/`/app/migrations`**: `COPY --from=builder --chown=pcy:pcy` must be used for the binary, migrations, and static directories. Leaving ownership as `root:root` works at runtime (reads) but violates the invariant that a non-root process owns its read-only assets and will trip future writes (e.g. cache files in static).
- **AC-23 — Budget check placed after `acquire_wake`**: Tempting to put the check inside the wake loop "for simplicity." That would still write a `wake_started` event and possibly a partial LLM call before refusing. The check MUST sit in `src/background/listener.rs` _before_ `agent::acquire_wake` so the CAS never fires when budget is exhausted.
- **AC-23 — `budget_used_usd` increment outside the `llm_calls` transaction**: If the increment is a second statement against a separate connection or after `tx.commit()`, a crash between them produces an `llm_calls` row without a budget update. Same `tx`, same transaction, both statements. No shortcuts.
- **AC-23 — Escape hatch silently changed**: Tempting to redefine "unlimited" as `NULL` or `-1`. Scope locks it to `budget_limit_usd = 0`. The column is `NOT NULL DEFAULT 10.0`; do not alter the schema.
- **AC-24 — New secret persisted _before_ event append, or event append before update, without transactional wrapper**: The audit event and the secret rotation should be consistent. If the update succeeds and the event append fails, the system silently loses the audit trail. Wrap in a transaction or treat event-append failure as fatal (500) and rely on idempotent retry.
- **AC-24 — Secret material leaked into event payload**: Tempting to include `{"new_secret":"…"}` in the `webhook_secret_rotated` event "for debugging." The scope explicitly forbids secret material in the payload. Payload is empty/NULL.
- **AC-24 — Endpoint mounted outside `auth_middleware`**: The rotate route MUST sit inside the authenticated, workspace-scoped stack alongside PATCH/DELETE. A public or cross-workspace-accessible rotate endpoint is a critical auth-bypass.
- **AC-25 — CLI subcommands reduced to a bootstrap-only demo**: Tempting to ship `pcy bootstrap` + `pcy status` and defer `agent rotate-secret` / `budget {set,show,reset}` / `message` / `events --tail` "for v5." Scope requires the full subcommand set listed in the AC-25 description. Every subcommand ships in v4 or the AC is not closed.
- **AC-25 — Fat `src/bin/pcy.rs`**: Tempting to write command logic directly in `src/bin/pcy.rs` "since it's small." The invariant is that `src/bin/pcy.rs` is a 3-line shim; logic lives in `src/cli/**`. This preserves testability (`open_pincery::cli::run` callable from integration tests) and keeps the binary layer thin.
- **AC-25 — Direct `reqwest` calls from CLI commands**: Tempting to reach for `reqwest::Client` inline in each command file. All HTTP work MUST go through `src/api_client.rs`. Divergent call patterns undermine the shared-client invariant (T-28) and will drift from `docs/api.md`.
- **AC-26 — UI reduces to a static link list / curl instructions**: Tempting to ship an `index.html` that just says "use curl or `pcy`." Scope requires five real hash-routed views and a live long-poll event stream. Static link list is a placeholder, not AC-26.
- **AC-26 — Event stream degrades to a one-shot GET with a reload button**: The live stream MUST be an actual poll loop with `since=<last_id>` and exponential backoff. A manual-reload UX fails the "live" requirement in the AC and the 5-second wake-event surfacing proof.
- **AC-26 — Framework / build step sneaks in**: Tempting to "just add" React / Preact / Vite because it's faster to author. v4 locks vanilla JS, no build step, no CDN fetches. Any `npm install` or `<script src="https://…">` fails the AC.
- **AC-27 — `docs/api.md` becomes a stub pointing at `src/api/mod.rs`**: Tempting to write "see source for details" rather than typed field-by-field shapes. The REVIEW pass will reject a stub. Each endpoint needs method + path + headers + request body + response bodies per status + side effects.
- **AC-27 — Stability banner omitted or hedged**: Tempting to write "best-effort compatibility" or leave the stability statement out. The scope locks `v4 → v5 compatible` with the exact guarantees (no remove, no rename, no incompatible type change). The banner must say so explicitly.

## v4 Clarifications Needed

None. Vanilla JS (no framework, no build step, no TypeScript) is locked. CLI name `pcy` is locked. AC-27 stability scope (v4 → v5) is locked. TLA+ enum-name alignment is explicitly deferred to v5.

## v4 Build Order

1. **Slice 1 — AC-22 Dockerfile only**: Update `Dockerfile` runtime stage: add `pcy` user (UID/GID 10001), `--chown=pcy:pcy` on all COPY lines for binary + migrations + static, `USER pcy` before `ENTRYPOINT`. No Rust code changes. Verify with `tests/docker_nonroot_test.sh` (gated). Smallest, most isolated slice.
2. **Slice 2 — AC-23 budget enforcement**: Insert pre-CAS budget check in `src/background/listener.rs`; wrap `llm_calls` insert + `agents.budget_used_usd` increment in a single transaction in `src/runtime/llm.rs` (or wherever `llm_call::record_call` is invoked). Add `tests/budget_test.rs`. Pure runtime change, no new endpoints.
3. **Slice 3 — AC-24 webhook rotation**: Create `src/api/webhook_rotate.rs` with the rotate handler. Register `POST /agents/{id}/webhook/rotate` in `src/api/agents.rs::router()` under `auth_middleware`. Append `webhook_secret_rotated` event. Add `tests/webhook_rotate_test.rs`. One new endpoint, no new migrations.
4. **Slice 4 — AC-25 `pcy` CLI**: Add `[[bin]] pcy` + `clap`/`toml`/`dirs` deps in `Cargo.toml`. Create `src/bin/pcy.rs` (3-line shim), `src/api_client.rs` (shared HTTP client), `src/cli/mod.rs`, `src/cli/config.rs`, and `src/cli/commands/{bootstrap,login,agent,message,events,budget,status}.rs`. Add `tests/cli_e2e_test.rs` using `assert_cmd` against a live test server. Largest slice; watch the 600-line `src/cli/**` budget declared in the v4 Complexity Exceptions.
5. **Slice 5 — AC-26 UI**: Replace `static/index.html`; add `static/app.js` (vanilla JS hash router, 5 views, long-poll with exponential backoff) and `static/style.css` (minimal reset + utility, ~80 lines). Add `tests/ui_smoke_test.rs`. Relies on Slices 2–4's endpoints being stable. Keep `static/app.js` within the ~400-line soft ceiling (complexity exception).
6. **Slice 6 — AC-27 `docs/api.md`**: Written last, once all endpoints touched by Slices 2–4 are stable. One section per endpoint reachable from CLI (Slice 4) or UI (Slice 5). Stability banner (`v4 → v5 compatible`) at the top. REVIEW subagent cross-references `docs/api.md` ↔ `src/api/` ↔ CLI/UI callers. Pure docs, no code.

## v4 Complexity Exceptions

Carried forward from `scaffolding/design.md` v4 section:

- **`static/app.js` — soft ceiling ~400 lines** (exceeds the normal 300-line guideline). Justification: a single-file vanilla SPA without a build step is the intentional deployment-artifact-free approach; splitting into `<script type="module">` imports would add CORS/relative-path tax without material benefit at this size. If the file blows past ~400 lines, re-evaluate — do not silently extend the ceiling.
- **`src/cli/**`— 600-line total budget** across`src/cli/mod.rs`, `src/cli/config.rs`, and `src/cli/commands/\*.rs`. Justification: a second binary in the same crate is cohesive with the runtime and shares `src/api_client.rs`; extracting to a separate workspace member is premature at v4 size. If v5 pushes the CLI past 600 lines, extract to a workspace member per preferences.md convention.
