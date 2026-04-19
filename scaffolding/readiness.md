# Readiness: Open Pincery v1

## Verdict

READY

## Truths

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

## Key Links

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

## Scope Reduction Risks

- **Shell tool becomes a no-op stub**: AC-4 requires the agent to "make at least one tool call." If the shell tool is implemented as a stub that returns a canned string without actually executing a subprocess, the wake loop appears to work but the system is not a real agent runtime. BUILD must implement `tokio::process::Command` with stdout/stderr capture, timeout, and exit code recording.

- **Prompt assembly skips character trim**: AC-3 specifies "character-based trim drops oldest messages first." If BUILD omits the trim logic and simply includes all messages, the criterion technically passes for small test cases but breaks at scale. BUILD must implement an explicit `max_prompt_chars` config with trim logic.

- **Drain check returns false unconditionally**: AC-9 is the most complex sequencing criterion. It's tempting to always return "no pending events" and skip re-acquisition. The test must insert a message during an active wake and verify a second wake starts.

- **Maintenance LLM call returns identity/work_list unchanged**: AC-5 requires the maintenance to return "updated" projections. If the mock always returns the input verbatim, it doesn't test that new versioned rows are written. The mock must return *different* content and the test must verify the delta.

- **Session auth becomes a pass-through**: AC-6 includes `Authorization: Bearer <session_token>` on all endpoints. If auth middleware is skipped or always returns 200, the bootstrap token gating (AC-10) is meaningless. BUILD must implement real session lookup from `user_sessions` table.

- **LLM call auditing silently dropped**: T-7 requires every LLM call to be recorded. If `llm_calls` inserts are commented out or made optional-and-never-enabled, observability is gone. Tests must query `llm_calls` after wake + maintenance and verify rows exist.

## Clarifications Needed

- **Prompt character budget**: AC-3 says "character-based trim" but no specific limit is defined in scope or design. Scope defers "context character cap enforcement" to Phase 2 but the trim function itself is in-scope. **Bounded assumption**: use a configurable `MAX_PROMPT_CHARS` env var defaulting to 100,000 characters. This does not change the pass/fail of AC-3 — the test verifies trim *behavior* regardless of the specific limit.

- **Append-only enforcement mechanism**: AC-2 says events are "never updated or deleted." The design enforces this by simply never issuing UPDATE/DELETE in application code. There is no DB-level trigger or constraint preventing it. **Bounded assumption**: application-level enforcement is sufficient for v1. A DB trigger (`BEFORE UPDATE OR DELETE ON events ... RAISE EXCEPTION`) can be added later as defense-in-depth. This does not change AC-2's test — the test verifies the application never issues mutations.

- **Bootstrap idempotency**: AC-10 says "on first run with an empty database" but doesn't specify behavior on repeat calls. **Bounded assumption**: the bootstrap endpoint returns 409 Conflict if bootstrap has already been completed (check for existing local_admin user). This keeps the test deterministic without expanding scope.

## Build Order

1. **AC-10 — Bootstrap + Migrations** (Slice 1): Cargo project scaffold, all 13 migration files, `config.rs`, `db.rs`, `error.rs`, bootstrap endpoint, user/org/workspace models. This is the foundation — every other slice needs a running DB with schema.

2. **AC-2 — Event Log** (Slice 2): `models/event.rs` with `append_event` and `recent_events`, events migration already created in Slice 1. Test append-only semantics.

3. **AC-1 — CAS Lifecycle** (Slice 3): `models/agent.rs` with all four CAS functions. Concurrent wake test. This unlocks the wake executor.

4. **AC-7 — Wake Triggers** (Slice 4): `background/listener.rs` with LISTEN/NOTIFY. Wires message insertion to wake acquisition. Needs AC-1 + AC-2.

5. **AC-3 — Prompt Assembly** (Slice 5): `runtime/prompt.rs`, `models/projection.rs`, `models/prompt_template.rs`. Seed a default prompt template in migrations. Character-based trim. Needs projections + events.

6. **AC-4 — Wake Loop** (Slice 6): `runtime/wake_loop.rs`, `runtime/llm.rs`, `runtime/tools.rs`. LLM client with retry. Shell tool via `tokio::process::Command`. Plan + sleep tools. Iteration cap. This is the largest and most complex slice — complexity exception applies.

7. **AC-5 — Maintenance Cycle** (Slice 7): `runtime/maintenance.rs`. Separate LLM call, versioned projection writes, wake summary ≤500 chars. Needs wake loop to produce a transcript.

8. **AC-9 — Drain Check** (Slice 8): `runtime/drain.rs`. Post-maintenance event check + CAS re-acquire. Needs maintenance to be complete.

9. **AC-8 — Stale Wake Recovery** (Slice 9): `background/stale.rs`. Periodic background job. Independent of wake flow but needs agent CAS primitives.

10. **AC-6 — HTTP API** (Slice 10): `api/mod.rs`, `api/agents.rs`, `api/messages.rs`, `api/events.rs`. Session auth middleware. Assembles all internal components behind REST endpoints. Final integration point.

## Complexity Exceptions

- **`wake_loop.rs` may exceed 300 lines (target ≤400)**: Carried forward from design.md. The LLM interaction loop, tool dispatch, iteration cap, event recording, and error handling are tightly coupled in a single control flow. Artificial splitting would obscure the state machine.

- **13+ migration files**: Each TLA+-specified table gets its own migration per preferences.md convention ("One migration per schema change"). This is high file count but each file is small and self-contained.

- **Slice 6 (Wake Loop) touches ~5 files**: `wake_loop.rs`, `llm.rs`, `tools.rs`, `event.rs` (new event types), and potentially `agent.rs` (iteration count update). This is at the edge of the 5-file slice limit but is inherently coupled — the wake loop, LLM client, and tool dispatch form a single vertical slice. Can be sub-sliced: (6a) LLM client + mock, (6b) tool dispatch, (6c) wake loop integration.
