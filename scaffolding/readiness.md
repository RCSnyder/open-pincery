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
- **L-13** AC-13 → `api/mod.rs` (tower-governor middleware, two rate-limit tiers) + `config.rs` (`RATE_LIMIT_PER_MINUTE`, `RATE_LIMIT_BOOTSTRAP_PER_MINUTE`) → `tests/rate_limit_test.rs` → Runtime proof: send 61 requests in rapid succession to an authenticated endpoint, confirm 61st returns 429 with `Retry-After` header
- **L-14** AC-14 → `api/webhooks.rs` (HMAC-SHA256 verify, idempotency dedup) + `models/agent.rs` (`webhook_secret` column) + migration `add_webhook_secrets.sql` + migration `create_webhook_dedup.sql` + `models/event.rs` (append `webhook_received` event) → `tests/webhook_test.rs` → Runtime proof: send signed webhook → confirm `webhook_received` event in log + wake triggered; resend same webhook → confirm 200 without duplicate event; send bad signature → confirm 401
- **L-15** AC-15 → `api/agents.rs` (PATCH + DELETE handlers) + `models/agent.rs` (`update_agent`, `soft_delete_agent`) → `tests/agent_mgmt_test.rs` → Runtime proof: PATCH to disable agent → send message → confirm no wake occurs; PATCH to rename → confirm name changed; DELETE → confirm `is_enabled = false` and `disabled_reason = 'deleted'`

## Acceptance Criteria Coverage

| AC ID | Build Slice | Planned Test | Planned Runtime Proof | Notes |
|-------|-------------|--------------|----------------------|-------|
| AC-1 | Slice 3: CAS lifecycle functions | `lifecycle_test.rs` — two concurrent `acquire_wake` calls; assert exactly one returns `Some`, one returns `None`; test all four transitions (asleep→awake, awake→maintenance, maintenance→asleep, maintenance→awake) | Run two tokio tasks racing `acquire_wake`; verify via DB query that only one wake_id is set | Requires real Postgres; CAS relies on DB atomicity |
| AC-2 | Slice 2: Event log model | `event_log_test.rs` — insert events, query back, verify sequence; attempt UPDATE/DELETE and confirm rejection (DB constraint or no API) | Send message via API → agent wakes → query `/api/agents/:id/events` → verify event_type ordering: message_received → wake_start → tool_call → tool_result → wake_end | Consider adding a DB trigger or CHECK to enforce append-only, or rely on no-UPDATE/DELETE in code |
| AC-3 | Slice 5: Prompt assembly | `prompt_test.rs` — create agent with seed projections, 25 events, 3 wake summaries, active prompt template → assemble prompt → assert system_prompt contains all 6 components in order; assert messages ≤ 200; assert character trim works | Assemble prompt for a known agent and inspect output structure. Verify oldest messages dropped first when oversized | Depends on projections, wake_summaries, and prompt_templates tables existing |
| AC-4 | Slice 6: Wake loop | `wake_loop_test.rs` — mock LLM via wiremock → tool_calls response → dispatch shell (echo "hello") → result → LLM returns text → implicit sleep; separate test hitting iteration cap at 50 | End-to-end: send message to agent via API → background listener triggers wake → LLM mock responds → verify events sequence → agent returns to asleep | Largest slice; may approach 400-line exception for wake_loop.rs |
| AC-5 | Slice 7: Maintenance cycle | `maintenance_test.rs` — mock maintenance LLM call → provide previous identity/work_list/transcript → verify new projection rows inserted (version incremented) + wake_summary row ≤500 chars | After a complete wake cycle, query `agent_projections` ORDER BY version DESC LIMIT 2 → confirm version incremented; query `wake_summaries` → confirm new entry | Maintenance uses a separate LLM model config (`LLM_MAINTENANCE_MODEL`) |
| AC-6 | Slice 10: HTTP API | `api_test.rs` — test each of 6 endpoints: POST /api/agents (201), GET /api/agents (200 array), GET /api/agents/:id (200 with projections), POST /api/agents/:id/messages (202), GET /api/agents/:id/events (200 with events+total), GET /health (200) | Curl each endpoint against running server; verify JSON shapes match design contracts | Auth via session_token from bootstrap; 401 on missing/invalid token |
| AC-7 | Slice 4: Wake triggers | `trigger_test.rs` — insert message_received event → issue NOTIFY → assert listener receives and spawns wake task within 5s | Send POST /api/agents/:id/messages → measure wall-clock time to wake_start event appearing in event log; assert < 5 seconds | Requires real Postgres LISTEN/NOTIFY; cannot mock |
| AC-8 | Slice 9: Stale wake recovery | `stale_test.rs` — set agent status='awake', wake_started_at=NOW()-3h → run stale recovery job → assert status='asleep', wake_id=NULL, stale_wake_recovery event exists | Start stale recovery background task → manipulate time or directly set stale timestamp → verify recovery within one job cycle | Tests both `awake` and `maintenance` stale states per AC-8 |
| AC-9 | Slice 8: Drain check | `drain_test.rs` — complete wake → enter maintenance → insert message_received during maintenance → drain check finds event → assert drain_reacquire CAS succeeds → new wake starts | Send message mid-wake → let original wake complete + maintenance → verify a second wake starts without a new NOTIFY by checking for two wake_start events with same message visible | Most complex sequencing; needs careful test setup with timing control |
| AC-10 | Slice 1: Bootstrap + migrations | `bootstrap_test.rs` — start with empty DB → run migrations → POST /api/bootstrap with correct token → verify 201 + user/org/workspace rows; repeat call → verify 409 or idempotent behavior | Start binary against empty Postgres → call bootstrap → query tables directly → confirm rows exist with expected roles | First slice; all other slices depend on this working |
| AC-11 | v2 Slice 1: Graceful shutdown | `shutdown_test.rs` — start server in a child process, send a message to trigger wake, send SIGTERM, assert process exits 0 within 30s; assert no partial wake events (wake_end must exist if wake_start exists) | Start server, trigger wake, send SIGTERM via `nix::sys::signal` or `tokio::signal`, confirm clean exit with code 0 and all background tasks stopped | Requires `CancellationToken` threaded through listener + stale + wake_loop; must also test SIGINT path |
| AC-12 | v2 Slice 2: Docker Compose | Manual test — `Dockerfile` multi-stage build + `docker-compose.yml` with app + postgres services + healthcheck | `docker compose up` from clean state → curl `GET /health` → assert `{"status":"ok"}` within 60s; `docker compose down` clean | No automated test — Docker-in-Docker is out of scope; verified manually during BUILD |
| AC-13 | v2 Slice 3: Rate limiting | `rate_limit_test.rs` — send 61 requests to authenticated endpoint → assert first 60 return 200, 61st returns 429 with `Retry-After` header; separate test for bootstrap at 10 req/min | Start server, burst 61 authenticated requests → verify 429 + header; burst 11 bootstrap requests → verify 429 | Needs `tower-governor` or equivalent crate added to `Cargo.toml` |
| AC-14 | v2 Slice 4: Webhook ingress | `webhook_test.rs` — compute HMAC-SHA256 of payload with agent webhook_secret, send POST with valid signature → assert 202 + `webhook_received` event; send with bad signature → assert 401; resend with same idempotency key → assert 200 without duplicate event | Send signed webhook to `/api/agents/:id/webhooks` → query event log → confirm `webhook_received` with correct content; resend → confirm no new event row; bad signature → confirm 401 with no event | Requires 2 new migrations (webhook_secret column + dedup table) |
| AC-15 | v2 Slice 5: Agent management | `agent_mgmt_test.rs` — PATCH disable agent → assert `is_enabled = false`; send message → assert no wake (acquire_wake returns None); PATCH enable → send message → assert wake succeeds; PATCH rename → assert name changed; DELETE → assert `is_enabled = false, disabled_reason = 'deleted'` | PATCH to disable → POST message → query agent → confirm still asleep; PATCH enable → POST message → confirm wake; DELETE → confirm soft-delete fields | CAS `WHERE is_enabled = TRUE` already in v1 acquire_wake — just needs the management endpoints to toggle the flag |

## Scope Reduction Risks

### v1 Risks (carried forward)

- **Shell tool becomes a no-op stub**: AC-4 requires the agent to "make at least one tool call." If the shell tool is implemented as a stub that returns a canned string without actually executing a subprocess, the wake loop appears to work but the system is not a real agent runtime. BUILD must implement `tokio::process::Command` with stdout/stderr capture, timeout, and exit code recording.

- **Prompt assembly skips character trim**: AC-3 specifies "character-based trim drops oldest messages first." If BUILD omits the trim logic and simply includes all messages, the criterion technically passes for small test cases but breaks at scale. BUILD must implement an explicit `max_prompt_chars` config with trim logic.

- **Drain check returns false unconditionally**: AC-9 is the most complex sequencing criterion. It's tempting to always return "no pending events" and skip re-acquisition. The test must insert a message during an active wake and verify a second wake starts.

- **Maintenance LLM call returns identity/work_list unchanged**: AC-5 requires the maintenance to return "updated" projections. If the mock always returns the input verbatim, it doesn't test that new versioned rows are written. The mock must return *different* content and the test must verify the delta.

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

- **Rate limit scope (per-IP vs per-token)**: AC-13 says "per-IP." This means unauthenticated and authenticated requests from the same IP share no state — they are tracked on separate middleware instances with different limits. **Bounded assumption**: tower-governor keyed by `ConnectInfo<SocketAddr>` peer IP, two `GovernorLayer` instances with distinct configs.

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

13. **AC-13 — Rate Limiting** (v2 Slice 3): Add `tower-governor` to `Cargo.toml`. Create two `GovernorLayer` configs in `api/mod.rs` — one for bootstrap (10/min), one for authenticated (60/min). Apply as tower middleware layers on the respective route groups. Files: `api/mod.rs`, `config.rs`, `Cargo.toml`. Test: `rate_limit_test.rs`.

14. **AC-14 — Webhook Ingress** (v2 Slice 4): Create `api/webhooks.rs` with HMAC-SHA256 verification, idempotency dedup lookup/insert, event append + NOTIFY. Add two migrations: `add_webhook_secrets.sql` (add `webhook_secret` column to agents), `create_webhook_dedup.sql` (idempotency key table). Register route in `api/mod.rs`. Add `hmac` + `sha2` crates (sha2 already present). Files: `api/webhooks.rs`, `api/mod.rs`, 2 migrations, `models/agent.rs` (expose webhook_secret in create). Test: `webhook_test.rs`.

15. **AC-15 — Agent Management** (v2 Slice 5): Add `update_agent` and `soft_delete_agent` functions to `models/agent.rs`. Add PATCH and DELETE handlers to `api/agents.rs`. Wire routes in agent router. Files: `models/agent.rs`, `api/agents.rs`. Test: `agent_mgmt_test.rs`. Depends on AC-11 implicitly (disabled wake rejection uses existing CAS WHERE clause).

## Complexity Exceptions

### v1 Exceptions (carried forward)

- **`wake_loop.rs` may exceed 300 lines (target ≤400)**: The LLM interaction loop, tool dispatch, iteration cap, event recording, and error handling are tightly coupled in a single control flow.

- **13+ migration files**: Each TLA+-specified table gets its own migration per preferences.md convention.

- **Slice 6 (Wake Loop) touches ~5 files**: Inherently coupled — the wake loop, LLM client, and tool dispatch form a single vertical slice.

### v2 Exceptions

- **v2 Slice 1 (Graceful Shutdown) touches 4 files**: `main.rs`, `background/listener.rs`, `background/stale.rs`, `Cargo.toml`. The `CancellationToken` must be threaded into every long-running task, making this inherently cross-cutting. Each file change is small (adding token parameter + select! on cancellation).
