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
- **Changes**: 5 vertical slices implemented. New files: api/webhooks.rs, Dockerfile, 2 migrations, 4 test files. Modified: main.rs, api/mod.rs, api/agents.rs, models/agent.rs, background/\*.rs, Cargo.toml, docker-compose.yml, tests/common/mod.rs.
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

- **Cosmetic**: 1 fix (test file count in log.md)
- **Structural**: 11 fixes (design.md: webhook_secret field, static/ dir, docker-compose desc, AgentResponse shape, webhook response bodies, rate limiting impl details, phantom env vars, missing runtime config vars; readiness.md: governor crate references)
- **Spec-violating**: None
- **Changes**: scaffolding/design.md, scaffolding/readiness.md, scaffolding/log.md
- **Next**: VERIFY

## v2 VERIFY — 2026-04-19T00:06Z

- **Gate**: PASS (attempt 1)
- **Evidence**:
  - 25/25 tests pass across 14 test files
  - All 15 ACs (AC-1 through AC-15) verified with real evidence
  - All v2 truths (T-13 through T-19) hold
  - No security issues (no secrets in source, parameterized SQL, constant-time HMAC, hashed session tokens)
  - Deployment config correct (Dockerfile + docker-compose.yml)
  - Tests are non-trivial (real DB, meaningful assertions, edge cases)
  - Non-blocking notes: rate_limit_test could assert Retry-After header; no authenticated rate limit test; shutdown test only covers stale recovery cancellation
- **Changes**: None (read-only verification)
- **Retries**: 0
- **Next**: DEPLOY

## v2 DEPLOY — 2026-04-19T00:07Z

- **Gate**: PASS (attempt 1)
- **Evidence**:
  - 25/25 tests pass (full router exercised via `oneshot()`)
  - README.md updated with v2 features, Docker Compose full-stack instructions, new API endpoints, rate limiting docs
  - DELIVERY.md updated to v2 with Docker deployment, v2 changelog, updated known limitations
  - Dockerfile with multi-stage build + healthcheck
  - docker-compose.yml with app + postgres services
  - .env.example present
- **Changes**: Updated README.md, DELIVERY.md, scaffolding/log.md
- **Retries**: 0
- **Next**: DONE — v2 deployed as self_host_individual (local binary + Docker Compose)

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

## v3 EXPAND — 2026-04-19T01:00Z

- **Gate**: PASS (attempt 1)
- **Evidence**: scope.md v3 appended with 6 ACs (AC-16..AC-21) spanning CI, JSON logging, Prometheus metrics, health/ready split, release hygiene + SBOM, operator runbooks. Derived from docs/input gap analysis + critical audit (scoped down from initial 7-AC OTEL-heavy draft).
- **Changes**: Updated `scaffolding/scope.md`
- **Retries**: 0
- **Next**: DESIGN

## v3 DESIGN — 2026-04-19T01:01Z

- **Gate**: PASS (attempt 1)
- **Evidence**: design.md v3 addendum (592 lines total) with metrics taxonomy, endpoint split, CI pipeline topology, release workflow with signed SBOM, observability layer module layout.
- **Changes**: Updated `scaffolding/design.md`
- **Retries**: 0
- **Next**: ANALYZE

## v3 ANALYZE — 2026-04-19T01:02Z

- **Gate**: PASS (attempt 1)
- **Evidence**: readiness.md v3 appended with Truths T-18..T-23, coverage rows for AC-16..AC-21 (each with planned test + runtime proof), scope reduction risks, build order. Verdict: READY.
- **Changes**: Updated `scaffolding/readiness.md`
- **Retries**: 0
- **Next**: BUILD

## v3 BUILD — 2026-04-19T01:03Z

- **Gate**: PASS (attempt 1)
- **Evidence**:
  - Slice 1 (AC-17 JSON logging): `src/observability/{mod,logging}.rs` with `init_logging()` + 3 unit tests
  - Slice 2 (AC-19 health split): `src/api/health.rs` with `/health` (pure liveness) + `/ready` (DB + `background_alive` atomic); `AppState.background_alive: Arc<AtomicBool>` threaded into listener + stale tasks; `tests/health_test.rs` 4 tests
  - Slice 3 (AC-18 Prometheus metrics): `src/observability/{metrics,server}.rs` with canonical metric name constants (WAKE_STARTED/COMPLETED, LLM_CALL, LLM_PROMPT_TOKENS/COMPLETION_TOKENS, TOOL_CALL, WEBHOOK_RECEIVED, RATE_LIMIT_REJECTED) + metrics HTTP server; instrumentation in wake_loop, llm, tools, webhooks, api/mod; `tests/observability_test.rs` 1 test; `METRICS_ADDR` env opt-in
  - Slice 4 (AC-16 CI): `.github/workflows/ci.yml` with fmt/clippy/test (Postgres 16 service container)/deny jobs; `deny.toml` with license allowlist + `unknown-registry/git = "deny"`; fixed 9 clippy issues across `api/messages.rs`, `background/stale.rs`, `models/{event,llm_call}.rs`, `runtime/wake_loop.rs`, `tests/agent_mgmt_test.rs`; `cargo clippy --all-targets -- -D warnings` exits 0; `cargo fmt --all -- --check` exits 0
  - Slice 5 (AC-21 runbooks): 5 runbooks under `docs/runbooks/` (stale-wake-triage, db-restore, migration-rollback, rate-limit-tuning, webhook-debugging) each with Symptom/Diagnostic Commands/Remediation/Escalation
  - Full regression: **30 tests pass, 0 failed** (`TEST_DATABASE_URL=...5433/open_pincery_test cargo test -- --test-threads=1` → EXIT=0)
- **Changes**: New files: `src/observability/{mod,logging,metrics,server}.rs`, `src/api/health.rs`, `.github/workflows/ci.yml`, `deny.toml`, `tests/{health,observability}_test.rs`, `docs/runbooks/*.md`. Modified: `src/api/{mod,webhooks,messages}.rs`, `src/background/{listener,stale}.rs`, `src/models/{event,llm_call}.rs`, `src/runtime/{wake_loop,llm,tools}.rs`, `src/main.rs`, `src/lib.rs`, `tests/agent_mgmt_test.rs`, `Cargo.toml`.
- **Retries**: 0
- **Next**: Slice 6 (AC-20 release+SBOM), then REVIEW

## v3 BUILD Slice 6 — 2026-04-19T01:04Z

- **Gate**: PASS (attempt 1)
- **Evidence**:
  - `Cargo.toml` gained `[profile.release]` with `lto = "fat"`, `codegen-units = 1`, `strip = "symbols"`, `opt-level = 3`, `panic = "abort"` — placed in Cargo.toml rather than `.cargo/config.toml` because stable Rust reads profile settings from the manifest (flagged for RECONCILE to update design.md).
  - `.cargo/config.toml` created with `[net] retry = 3` and aarch64 cross-linker directive (`aarch64-linux-gnu-gcc`).
  - `.github/workflows/release.yml` created — triggers on `v*` tags, matrix-builds `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` with `--locked`, installs `gcc-aarch64-linux-gnu` for cross, stages binary + SHA-256, signs with cosign keyless (`id-token: write` permission, GitHub OIDC), emits `.sig` + `.pem` per artifact.
  - Separate `sbom` job runs `cargo cyclonedx --format json` pinned to `0.5.7 --locked`, signs the SBOM with cosign keyless.
  - `publish` job depends on `[build, sbom]`, downloads all artifacts, uses `softprops/action-gh-release@v2` with `fail_on_unmatched_files: true` and auto-detects prerelease from `-rc/-beta/-alpha` tag suffix.
  - `cargo metadata --no-deps` exits 0 (manifest still valid). `cargo fmt --all -- --check` exits 0.
- **Changes**: New: `.github/workflows/release.yml`, `.cargo/config.toml`. Modified: `Cargo.toml` ([profile.release] block added).
- **Retries**: 0
- **Next**: REVIEW

## REVIEW (first pass) — 2026-04-19T02:00Z

- **Gate**: FAIL (attempt 1)
- **Evidence**: REVIEW subagent returned 1 Critical + 5 Required findings against v3:
  - **Critical**: AC-18 missing `ACTIVE_WAKES` gauge + `WAKE_DURATION` histogram (readiness.md truths explicitly required histogram).
  - **Required #1**: `/ready` missing migration-applied check (2 of 3 required checks implemented).
  - **Required #2**: Single shared `background_alive` cannot distinguish per-task failure; whichever task starts first flips it for both.
  - **Required #3**: `alive` flag never cleared once set — shutdown/error does not downgrade readiness.
  - **Required #4**: No AC-17 test that actually parses JSON-formatted log output.
  - **Required #5**: `docs/runbooks/db-restore.md:45` referenced nonexistent `--migrate-only` CLI flag with `|| true` masking the error.
  - Plus `Consider` findings: `panic = "abort"` changed fault-isolation semantics without justification; `metrics-exporter-prometheus` `http-listener` feature unused.
- **Retries**: 1
- **Next**: Fix all findings, re-run REVIEW.

## REVIEW FIX — 2026-04-19T02:30Z

- **Gate**: N/A (work phase feeding the next REVIEW attempt)
- **Evidence**:
  - **Critical fix (AC-18)**: `ACTIVE_WAKES` gauge + `WAKE_DURATION` histogram constants added to `src/observability/metrics.rs`. RAII `WakeMetricsGuard` in `src/runtime/wake_loop.rs` increments the gauge on construction and on `Drop` decrements the gauge + records the histogram with `Instant::now().elapsed()`. Every wake termination path (iteration_cap, llm_error, empty_response, sleep, completed) goes through Drop. `tests/observability_test.rs` extended to assert both metric names appear in the `/metrics` scrape.
  - **Required #1 fix (migration check)**: `src/db.rs` exposes `pub static MIGRATOR` + `pub fn expected_migration_count()`. `ready()` now runs 3 checks: (1) `SELECT 1`, (2) `COUNT(*) FROM _sqlx_migrations WHERE success = TRUE >= expected_migration_count()` → 503 with `failing: "migrations"` + `expected`/`applied` fields, (3) both alive flags AND'd.
  - **Required #2 fix (per-task flags)**: `AppState.background_alive` replaced with `listener_alive: Arc<AtomicBool>` + `stale_alive: Arc<AtomicBool>`. `/ready` reports `failing: "background_task:listener"` / `"background_task:stale_recovery"` / `"background_tasks"` depending on which combination is down. `src/main.rs` threads each flag to its own task.
  - **Required #3 fix (reset on exit)**: Both `src/background/listener.rs` and `src/background/stale.rs` now construct an `AliveGuard(Arc<AtomicBool>)` at the top of the task body whose `Drop` impl stores `false`. Every return path — initial `PgListener::connect_with` error, `listen()` error, shutdown-cancelled, any panic in the loop — clears the flag.
  - **Required #4 fix (AC-17 JSON assertion)**: `src/observability/logging.rs` exposes `json_subscriber_for_writer<W: MakeWriter>` for test injection. New unit test `json_output_is_parseable_with_required_fields` installs the JSON subscriber against a shared `Arc<Mutex<Vec<u8>>>` writer, emits `tracing::info!(target: "ac17_test", ...)`, parses every line as `serde_json::Value`, and asserts `timestamp`/`level`/`target`/`fields` are present and `fields.message` matches.
  - **Required #5 fix (runbook)**: `docs/runbooks/db-restore.md` Path A step 4 replaced with startup-driven migration (`docker compose start app` + `docker compose logs --tail=50 app | grep -E "Migrations complete|migrate"`).
  - **Consider fixes**: `panic = "abort"` removed from `[profile.release]` (restores unwind semantics so one task panic doesn't crash the multi-agent service); `metrics-exporter-prometheus` `http-listener` feature removed (unused — code uses hand-rolled axum `/metrics` server).
  - Health test suite expanded: 4 → 6 tests (added `ready_503_when_only_listener_down`, `ready_503_when_only_stale_down`).
  - `cargo check --all-targets` clean; `cargo build --tests` clean; full regression **33 passed / 0 failed** (`TEST_DATABASE_URL=...5433/open_pincery_test cargo test --all-targets -- --test-threads=1` → EXIT=0).
- **Changes**: Modified: `Cargo.toml`, `src/api/{mod,health}.rs`, `src/background/{listener,stale}.rs`, `src/db.rs`, `src/main.rs`, `src/observability/{logging,metrics}.rs`, `src/runtime/wake_loop.rs`, `tests/{health,observability}_test.rs`, `docs/runbooks/db-restore.md`, `Cargo.lock`.
- **Retries**: 0
- **Next**: REVIEW (second pass) — expecting PASS.

## RECONCILE — 2026-04-19T02:45Z

- **Gate**: PASS (auto-fix)
- **Evidence**: design.md and readiness.md realigned with shipped v3 code: `.cargo/config.toml` purpose corrected (net retry + cross-linker); `[profile.release]` acknowledged to live in Cargo.toml (stable-rust requirement); `metrics-exporter-prometheus` dependency snippet updated (no `http-listener` feature); `/ready` pseudo-code now shows 3 checks with per-task failing labels; AppState plumbing row split into `listener_alive`/`stale_alive` with `AliveGuard` reset-on-drop. Directory structure, interfaces, scope ACs, and log entries all match the code as of `ca92607`.
- **Changes**: `scaffolding/design.md`, `scaffolding/readiness.md`.
- **Retries**: 0
- **Next**: REVIEW (second pass).

## REVIEW (second pass) — 2026-04-19T03:00Z

- **Gate**: PASS (attempt 2)
- **Evidence**: REVIEW subagent verdict **PASS**. All 6 findings from first pass confirmed resolved with specific file+line citations (wake_loop.rs:14-34 WakeMetricsGuard, health.rs:22-82 3-check ready, api/mod.rs:29-41 per-task flags, listener.rs:24-32 + stale.rs:19-27 AliveGuard, logging.rs json_subscriber_for_writer + json_output_is_parseable_with_required_fields test, db-restore.md migration step). No new Critical/Required findings. Two FYI items noted non-blocking: JSON envelope nests `message` under `fields.message` (idiomatic tracing-subscriber shape); Prometheus recorder is process-global so a second install-test would panic — fine with `--test-threads=1`.
- **Retries**: 1 (first pass FAIL, second pass PASS)
- **Next**: VERIFY.

## VERIFY — 2026-04-19T03:30Z

- **Gate**: PASS (attempt 2)
- **Evidence**: VERIFY subagent returned structured report. First pass **FAIL** on a single fmt regression (`src/observability/logging.rs` single-line `assert!` exceeded rustfmt max_width). Fixed with `cargo fmt --all` → commit `d853a20`. Verified post-fix: `cargo fmt --all -- --check` EXIT=0, `cargo clippy --all-targets -- -D warnings` EXIT=0, full regression **33 passed / 0 failed** EXIT=0. All 21 ACs (AC-1..AC-21) individually verified with evidence: 15 via targeted tests + source inspection, 6 via live `cargo run` probes (AC-6 POST /api/bootstrap 201, AC-7 message-triggered wake observed in metrics within 3s, AC-10 bootstrap against empty DB, AC-17 11 JSON lines parsed as valid JSON with required fields, AC-18 `/metrics` scrape showed `wake_started_total`, `wake_completed_total{reason=...}`, `active_wakes`, `wake_duration_seconds` with quantiles, AC-19 `/health` 200 + `/ready` 200). Security audit clean (secrets env-gated, HMAC constant-time, no SQL injection). Deployment config verified (Dockerfile, docker-compose.yml, ci.yml, release.yml, deny.toml, .cargo/config.toml, Cargo.toml profile.release). Two FYI items non-blocking.
- **Retries**: 1 (first pass FAIL on fmt, second pass PASS)
- **Next**: DEPLOY.

## DEPLOY — 2026-04-19T04:00Z

- **Gate**: PASS (attempt 1)
- **Evidence**: v3 targets `self_host_individual` — the deliverable is the source repo + Docker Compose stack + signed-release workflow, not a hosted URL. DEPLOY artifacts:
  - `README.md` updated: status line now reflects v3; added `/ready` example with all 5 `failing` modes; added "Observability (optional)" section covering `LOG_FORMAT=json`, `METRICS_ADDR`, and runbooks; API table includes `/ready` and `/metrics`.
  - `DELIVERY.md` updated to v3: title, what-was-built paragraph, new v3 Changes section (AC-16..AC-21 each with one-paragraph summary), Known Limitations section updated (removed stale "cargo-audit deferred" — now wired via cargo-deny in CI; added metrics-recorder global / Dockerfile-runs-as-root / release-workflow-not-exercised).
  - Release pipeline (`.github/workflows/release.yml`) ready; first tagged release (`v0.3.0-rc1` or similar) will exercise cosign keyless signing + SBOM publication.
  - Final regression: 33 tests pass, 0 fail, EXIT=0. Clippy clean. Fmt clean.
- **Changes**: `README.md`, `DELIVERY.md`, `scaffolding/log.md`.
- **Retries**: 0
- **Next**: v3 complete. Await iteration signal (ITERATE on new inputs).
