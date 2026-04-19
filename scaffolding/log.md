# Open Pincery — Experiment Log

## EXPAND — 2026-04-18T00:00Z

- **Gate**: PASS (attempt 1)
- **Evidence**: scope.md created with 10 acceptance criteria (AC-1 through AC-10), Skyscraper tier, self_host_individual deploy target, Rust+Postgres stack per preferences.md. All 12 gate conditions verified.
- **Changes**: Created `scaffolding/scope.md`
- **Retries**: 0
- **Next**: DESIGN

## DESIGN — 2026-04-18T00:01Z

- **Gate**: PASS (attempt 1)
- **Evidence**: design.md created with architecture diagram, directory structure (30+ files), interfaces for Agent/Event/Prompt/LLM/Tool/API, external integrations with error handling and test strategies, observability section, complexity exceptions. Key scenario traced end-to-end.
- **Changes**: Created `scaffolding/design.md`
- **Retries**: 0
- **Next**: ANALYZE

## ANALYZE — 2026-04-18T00:02Z

- **Gate**: PASS (attempt 1)
- **Evidence**: readiness.md created with Verdict=READY. All 10 ACs mapped to design components, tests, and runtime proofs. 12 truths, 6 scope-reduction risks, 3 bounded clarifications, 10-slice build order, 3 complexity exceptions.
- **Changes**: Created `scaffolding/readiness.md`
- **Retries**: 0
- **Next**: BUILD

## BUILD — 2026-04-18T00:03Z

- **Gate**: PASS (attempt 1)
- **Evidence**: 
  - Code compiles with 0 errors, 0 warnings
  - 15 integration tests pass across 10 test files (serial execution)
  - All 10 ACs have corresponding tests:
    - AC-1: lifecycle_test (CAS lifecycle happy path + invalid transitions)
    - AC-2: event_log_test (append and query)
    - AC-3: prompt_test (prompt assembly)
    - AC-4: wake_loop_test (sleep termination + iteration cap, using wiremock)
    - AC-5: maintenance_test (projection creation from mocked LLM)
    - AC-6: api_test (CRUD agents + auth enforcement)
    - AC-7: trigger_test (LISTEN/NOTIFY)
    - AC-8: stale_test (stale agent detection and recovery)
    - AC-9: drain_test (reacquire on pending events + release when empty)
    - AC-10: bootstrap_test (bootstrap flow + wrong token rejection)
  - Cargo.lock present
  - No hardcoded secrets in source
  - cargo-audit: unable to install (timeout), deferred
  - Schema fixes applied: llm_calls (purpose→call_type), projection work_list (JSONB→TEXT)
- **Changes**: All source modules, migrations, tests created. 5 commits.
- **Retries**: 0
- **Next**: REVIEW

## REVIEW — 2026-04-18T00:04Z

- **Gate**: PASS (attempt 1)
- **Evidence**: Review identified 2 critical and 6 required findings. All addressed in commit 82f7935. No `Critical` or `Required` findings remain.
- **Changes**: Code fixes applied per review findings (commit `fix(build): address REVIEW findings`)
- **Retries**: 0
- **Next**: RECONCILE

## RECONCILE — 2026-04-18T00:05Z

- **Structural drift fixed**:
  - design.md directory structure: tests were under `tests/integration/` in doc but `tests/` in code; added missing `src/lib.rs` and `src/auth.rs`
  - design.md interfaces: Agent struct missing `disabled_reason`/`disabled_at` fields; Event.source was `Option<String>` but code uses `String`; ChatRequest had `temperature` field not in code; ChatResponse.usage was non-optional but code uses `Option<Usage>`; LlmClient missing `maintenance_model` field; AgentStatus enum described but not implemented (code uses raw strings); `append_event` used `NewEvent` struct in doc but code uses individual params; `has_pending_events` query filter differed; `ToolCall` type renamed to `ToolCallRequest` in code
  - readiness.md Key Links: all 10 paths referenced `tests/integration/` but actual paths are `tests/`
  - log.md: missing REVIEW phase entry despite git commit 82f7935 recording review fixes
- **Documents updated**: `scaffolding/design.md`, `scaffolding/readiness.md`, `scaffolding/log.md`

## VERIFY — 2026-04-18T00:06Z

- **Gate**: PASS (attempt 1)
- **Evidence**: 17/17 tests pass. All 10 ACs verified with real evidence. All 12 truths confirmed by code inspection. No secrets in source. Deployment config exists (docker-compose.yml, .env.example). Application compiles cleanly (0 errors, 0 warnings).
- **Changes**: None (read-only verification)
- **Retries**: 0
- **Next**: DEPLOY

## DEPLOY — 2026-04-18T00:07Z

- **Gate**: PASS (attempt 1)
- **Evidence**: Application starts successfully, health endpoint returns 200 `{"status":"ok"}`, bootstrap creates admin + returns session token, double-bootstrap returns 409, auth rejects invalid tokens. docker-compose.yml + .env.example present. README.md updated with setup/run instructions. DELIVERY.md created.
- **Changes**: Updated README.md, created DELIVERY.md
- **Retries**: 0
- **Next**: DONE — deployed as self_host_individual (local binary + PostgreSQL)

---

## v2 EXPAND — 2026-04-19T00:00Z

- **Gate**: PASS (attempt 1)
- **Evidence**: scope.md v2 section added with AC-11 through AC-15 (graceful shutdown, Docker compose, rate limiting, webhook ingress, agent management). All criteria have stable IDs and measurable thresholds.
- **Changes**: Updated `scaffolding/scope.md` with v2 section
- **Retries**: 0
- **Next**: DESIGN

## v2 DESIGN — 2026-04-19T00:01Z

- **Gate**: PASS (attempt 1)
- **Evidence**: design.md v2 addendum added covering CancellationToken shutdown, docker-compose config, governor rate limiting, HMAC webhook verification, PATCH/DELETE agent endpoints.
- **Changes**: Updated `scaffolding/design.md` with v2 addendum
- **Retries**: 0
- **Next**: ANALYZE

## v2 ANALYZE — 2026-04-19T00:02Z

- **Gate**: PASS (attempt 1)
- **Evidence**: readiness.md updated with v2 truths, key links, coverage table for AC-11–AC-15. READY verdict.
- **Changes**: Updated `scaffolding/readiness.md`
- **Retries**: 0
- **Next**: BUILD

## v2 BUILD — 2026-04-19T00:03Z

- **Gate**: PASS (attempt 1)
- **Evidence**:
  - Code compiles with 0 errors
  - 24 integration tests pass across 14 test files
  - v2 ACs have corresponding tests:
    - AC-11: shutdown_test (CancellationToken cancels stale recovery)
    - AC-12: Dockerfile + docker-compose.yml created
    - AC-13: rate_limit_test (11th unauth request gets 429)
    - AC-14: webhook_test (valid sig 202, bad sig 401, idempotency dedup)
    - AC-15: agent_mgmt_test (PATCH rename/disable, DELETE soft-delete)
  - Cargo.lock present
  - No hardcoded secrets in source
- **Changes**: 5 vertical slices implemented. New files: api/webhooks.rs, Dockerfile, 2 migrations, 4 test files. Modified: main.rs, api/mod.rs, api/agents.rs, models/agent.rs, background/*.rs, Cargo.toml, docker-compose.yml, tests/common/mod.rs.
- **Retries**: 0
- **Next**: REVIEW

## v2 REVIEW — 2026-04-19T00:04Z

- **Gate**: PASS (attempt 1, after fix cycle)
- **Evidence**: Review found 2 Critical + 4 Required findings. All fixed:
  - Critical #1: ConnectInfo<SocketAddr> injection for per-IP rate limiting
  - Critical #2: Retry-After:60 header on 429 responses
  - Required #1: docker-compose.yml env vars matched to config.rs
  - Required #2: disabled_reason assertion in test_delete_agent
  - Required #4: webhook_secret hidden from non-create responses (skip_serializing_if)
  - Required #5: X-Forwarded-For trust removed, peer addr only
  - Bonus: Dockerfile apt-get layers combined, HEALTHCHECK ordering fixed
  - All 24 tests pass after fixes
- **Changes**: 6 files changed in fix commit (51791db)
- **Retries**: 0
- **Next**: RECONCILE

## v2 RECONCILE — 2026-04-19T00:05Z

- **Structural drift fixed**:
  - design.md Agent struct: added missing `webhook_secret: String` field to match code
  - design.md directory structure: added `static/` directory (index.html, css/, js/) and migration `20260418000014_event_source_not_null.sql`; updated docker-compose.yml comment to "App + Postgres"
  - design.md v1 API contracts: updated POST/GET /api/agents response shapes to include `is_enabled`, `disabled_reason`, `webhook_secret` (on create only), `identity`, `work_list` per current `AgentResponse`
  - design.md v2 webhook response bodies: corrected from `{ event_id }` to `{ status: "accepted" }` / `{ status: "duplicate" }` matching code
  - design.md v2 rate limiting: corrected from "tower-governor middleware" to "custom axum middleware using `governor` crate directly with `RateLimiter<IpAddr, DashMapStateStore<IpAddr>, DefaultClock>`"
  - design.md v2 config: removed nonexistent `RATE_LIMIT_PER_MINUTE` and `RATE_LIMIT_BOOTSTRAP_PER_MINUTE` env vars (limits are hardcoded in `AppState::new()`)
  - design.md v1 config: added 5 missing runtime config env vars (`MAX_PROMPT_CHARS`, `ITERATION_CAP`, `STALE_WAKE_HOURS`, `WAKE_SUMMARY_LIMIT`, `EVENT_WINDOW_LIMIT`)
  - readiness.md L-13: corrected from "tower-governor middleware" + config env var references to "custom `governor` middleware" with hardcoded limits
  - readiness.md Build Order Slice 13: corrected from `tower-governor`/`GovernorLayer` to `governor`/`KeyedRateLimiter`
  - readiness.md rate limit clarification: corrected from `GovernorLayer` to `KeyedRateLimiter`
- **Cosmetic drift fixed**:
  - log.md v2 BUILD: corrected "15 test files" → "14 test files" (common/mod.rs is a helper, not a test file)
- **Documents updated**: `scaffolding/design.md`, `scaffolding/readiness.md`, `scaffolding/log.md`
- **Confidence**: REPAIRED
