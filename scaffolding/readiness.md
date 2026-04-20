# Readiness: Open Pincery — v5 (Operator Onramp)

> This file supersedes the prior v4 readiness record. v4 is shipped; its
> readiness artifact lives in git history at commit `9013ff7` and earlier.
> v5 covers AC-28 through AC-33 only — the prior AC-1..AC-27 coverage is
> verified by the shipped v4 suite and is not re-planned here.

## Verdict

READY

v5 is additive docs, compose YAML, `.env.example`, scripts, and regression
tests — no new runtime modules, no new API endpoints, no schema migrations,
no dependencies. Every AC has unambiguous pass/fail criteria, a named test
file, and a concrete runtime proof path. No clarifications are unresolved.

## Truths

Non-negotiable statements that must be true in the shipped v5 system:

- **T-v5-1** `docker-compose.yml` contains no hardcoded secret literals —
  specifically no `changeme`, `change-me`, or default value for
  `OPEN_PINCERY_BOOTSTRAP_TOKEN`.
- **T-v5-2** Every environment variable the binary reads at runtime
  (every `std::env::var("…")` call site in `src/config.rs`,
  `src/main.rs` price/metrics blocks, `src/observability/logging.rs`,
  and `src/cli/**`) is either present in `.env.example` or explicitly
  listed as intentionally-internal in `tests/env_example_test.rs`.
- **T-v5-3** `docker-compose.yml` forwards every runtime-relevant variable
  to the `app` service via `${VAR}` / `${VAR:?…}` / `${VAR:-default}`
  interpolation — no hardcoded env values for anything the operator is
  expected to configure.
- **T-v5-4** Running `docker compose config` with a scrubbed environment
  (no `OPEN_PINCERY_BOOTSTRAP_TOKEN` or no `LLM_API_KEY`) exits non-zero
  with a message naming the missing variable.
- **T-v5-5** The default published port binding is `127.0.0.1:8080:8080`.
  Compose does not expose the app on `0.0.0.0` out of the box.
- **T-v5-6** `.env.example` defaults `OPEN_PINCERY_HOST=127.0.0.1`.
- **T-v5-7** `.env.example` ships OpenRouter as the default
  `LLM_API_BASE_URL` and includes a commented OpenAI-compatible
  alternative block.
- **T-v5-8** Each entry in `.env.example` has an inline comment describing
  purpose and default.
- **T-v5-9** `scripts/smoke.sh` completes the full onramp
  (`docker compose up -d --wait` → poll `/ready` until 200 within 60s →
  `pcy bootstrap` → `pcy agent create` → `pcy` send message → `pcy events`)
  and exits 0 only when a real `message_received` event for the created
  agent is observed in the event log.
- **T-v5-10** `scripts/smoke.ps1` performs the same steps with equivalent
  assertions on Windows and exits 0 only under the same observation.
- **T-v5-11** Failure paths in both smoke scripts emit actionable stderr
  that references a README Troubleshooting anchor by name.
- **T-v5-12** README Quick Start contains the three onramps in order
  (Web UI → `pcy` → curl/HTTP appendix), a "From Signed Release Binary"
  section referencing v3 AC-20 cosign verification, a Troubleshooting
  section covering the enumerated failure modes (bootstrap 401,
  rate-limit 429, silent wake, reset, `LOG_FORMAT=json`, `METRICS_ADDR`
  scrape, `pg_dump` backup), and a "Going public with HTTPS" subsection
  anchoring the Caddy overlay.
- **T-v5-13** Every milestone command executed by `scripts/smoke.sh`
  appears verbatim (or as a documented equivalent) in the README Quick
  Start body.
- **T-v5-14** The README API table includes
  `POST /api/agents/:id/rotate-webhook-secret` (v4 AC-24).
- **T-v5-15** `docker-compose.caddy.yml` defines a `caddy` service that
  fronts `app`; the overlay command
  `docker compose -f docker-compose.yml -f docker-compose.caddy.yml config`
  renders without error.
- **T-v5-16** `Caddyfile.example` parses as valid Caddy configuration
  (via `caddy validate` when available, else structural parse).
- **T-v5-17** No v1–v4 AC regresses: core runtime (CAS lifecycle, wake
  loop, maintenance, drain, event log), API surface, schema, and CLI
  behavior are unchanged.

## Key Links

Each v5 AC maps to a design artifact, a planned test file, and a runtime
proof path.

- **L-28** AC-28 → `docker-compose.yml` env block (design.md v5 §
  Modified Files) → `tests/compose_env_test.rs` → VERIFY runs
  `docker compose config` against a fixture `.env` and inspects the
  rendered `app.environment` map for passthrough values and absence of
  `changeme`.
- **L-29** AC-29 → `.env.example` (design.md v5 § Modified Files) →
  `tests/env_example_test.rs` → VERIFY parses `.env.example`, scans
  `src/**/*.rs` for `std::env::var("…")` string literals, diffs against
  the allowlist-of-allowlists.
- **L-30** AC-30 → `scripts/smoke.sh` + `scripts/smoke.ps1` (design.md
  v5 § New Files) → `tests/smoke_script_test.rs` (gated by `DOCKER_SMOKE=1`)
  for the bash half; PowerShell exercised manually on the Windows dev host
  during BUILD → VERIFY re-runs `bash scripts/smoke.sh` against a live
  compose stack and observes a `message_received` event.
- **L-31** AC-31 → `README.md` Quick Start rewrite (design.md v5 §
  Modified Files) → `tests/readme_quickstart_test.rs` → VERIFY walks the
  Quick Start against a live v5 stack step-by-step.
- **L-32** AC-32 → `docker-compose.yml` published ports +
  `.env.example` `OPEN_PINCERY_HOST` default → assertions in
  `tests/compose_env_test.rs` on the ports block and empty-env fail-fast
  path → VERIFY inspects `docker compose config` output for
  `127.0.0.1:8080:8080`.
- **L-33** AC-33 → `docker-compose.caddy.yml` + `Caddyfile.example` +
  README "Going public with HTTPS" subsection (design.md v5 § New Files)
  → `tests/caddy_overlay_test.rs` → VERIFY runs the overlay `config`
  command and confirms the caddy service is present with correct port
  mapping.

## Acceptance Criteria Coverage

| AC ID | Build Slice                                                        | Planned Test                                                                                                                                                                                                                                                                                                                                                            | Planned Runtime Proof                                                                                                                                                | Notes                                                                                                                            |
| ----- | ------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| AC-28 | (1) compose + `.env.example` rewrite                               | `tests/compose_env_test.rs` — renders `docker compose config` against a fixture `.env`; asserts each required var appears in the rendered `app.environment` map; asserts no `changeme` literal anywhere in the rendered yaml; asserts fail-fast on missing required secret                                                                                              | `docker compose config` output inspected during VERIFY; fixture `.env` values appear in the rendered yaml; empty env exits non-zero                                  | Coupled to AC-29 and AC-32 — same compose file, same `.env.example`                                                              |
| AC-29 | (1) compose + `.env.example` rewrite; (2) test                     | `tests/env_example_test.rs` — regex-scans `src/**/*.rs` for `std::env::var("KEY")` literals; parses `.env.example` keys; diffs against the internal allowlist; asserts every runtime-read key is covered                                                                                                                                                                | VERIFY compares parsed `.env.example` key set against the live `env::var` call-site set at HEAD                                                                      | Test must scan actual string literals, not just section headers in `.env.example`                                                |
| AC-30 | (3) `scripts/smoke.sh` + integration test; (4) `scripts/smoke.ps1` | `tests/smoke_script_test.rs` gated by `DOCKER_SMOKE=1` invoking `scripts/smoke.sh` end-to-end against the running test-DB stack; `scripts/smoke.ps1` exercised manually on the Windows dev host during BUILD; CI-side PowerShell validation is structural syntax parse only per design.md                                                                               | VERIFY executes `bash scripts/smoke.sh` against a running compose stack, observes exit 0 and a `message_received` event in `pcy events` output for the created agent | Per scope.md v5 clarifications — PowerShell half is structural in CI; real execution happens on the author's Windows workstation |
| AC-31 | (5) README rewrite + test                                          | `tests/readme_quickstart_test.rs` — `include_str!("../README.md")` followed by substring assertions for each named anchor (Quick Start, Web UI, pcy, curl/HTTP appendix, From Signed Release Binary, Troubleshooting subsections by anchor, Going public with HTTPS) and for each smoke-script milestone command                                                        | VERIFY reads the rendered README and walks through Quick Start manually against a live v5 stack                                                                      | Anchors must match the smoke-script step names and Troubleshooting slug IDs, not just section headings                           |
| AC-32 | (1) compose + `.env.example` rewrite                               | `tests/compose_env_test.rs` — published-ports assertion on `127.0.0.1:8080:8080`; second assertion runs `docker compose config` with empty env and asserts non-zero exit with a message naming the missing required var                                                                                                                                                 | VERIFY inspects `docker compose config` output for the loopback binding; attempts `docker compose up` with empty env and observes refusal                            | Same test file as AC-28; distinct assertions                                                                                     |
| AC-33 | (6) Caddy overlay + example + test + README subsection             | `tests/caddy_overlay_test.rs` — runs `docker compose -f docker-compose.yml -f docker-compose.caddy.yml config` and asserts `caddy` service present with 80/443 published; runs `caddy validate --config Caddyfile.example` when the `caddy` binary is available, falls back to structural parse otherwise; asserts README contains the "Going public with HTTPS" anchor | VERIFY manually activates the overlay on a staging domain, or at minimum inspects the rendered overlay compose config                                                | Per design.md: `caddy` binary is not required in CI; structural parse fallback is the documented path                            |

## Scope Reduction Risks

Places where BUILD might be tempted to ship a shell and close an AC
dishonestly. Each is paired with an explicit guard that must be present
in the test or the slice output.

- **R-1 (AC-30 — smoke script exits 0 without really exercising the
  onramp)**: Guard — the bash test must parse `pcy events` output and
  grep for `message_received` scoped to the created agent's ID. "No
  command returned non-zero" is not sufficient proof.
- **R-2 (AC-29 — env test only checks key presence in `.env.example`)**:
  Guard — the test must walk `src/**/*.rs`, extract `env::var("…")`
  string literals via regex, and diff against the parsed example file.
  A test that tautologically compares `.env.example` to itself is
  rejected.
- **R-3 (AC-31 — README test only matches section headings)**: Guard —
  the test must assert on step content: the actual command strings
  (`docker compose up -d --wait`, `pcy bootstrap`, curl examples) and
  each Troubleshooting anchor slug (e.g. `#bootstrap-401`,
  `#rate-limit-429`, `#silent-wake`, `#reset`, `#log-format-json`,
  `#metrics-scrape`, `#backup-one-liner`). Heading-only assertions fail.
- **R-4 (AC-33 — Caddy test only checks files exist)**: Guard — the
  test must render the overlay via `docker compose config` and inspect
  the service map; and must at minimum structurally parse
  `Caddyfile.example` (verify site block, `reverse_proxy` directive,
  TLS email/domain placeholders) when `caddy` is unavailable.
- **R-5 (AC-28 — compose test only greps for `${VAR}` tokens)**:
  Guard — the test must run `docker compose config` against a fixture
  `.env` and inspect the **rendered** environment map for the expected
  values. A syntactic grep on the source yaml is rejected.
- **R-6 (AC-30 PowerShell half — script is copy-pasted from bash and
  never executed)**: Guard — `scripts/smoke.ps1` must be exercised on
  the Windows host during BUILD (the author is on Windows). The BUILD
  log entry for that slice must record actual `pwsh scripts/smoke.ps1`
  output, not just "syntax parses."
- **R-7 (AC-28 fail-fast test passes because the operator has env vars
  exported)**: Guard — the test must invoke `docker compose config`
  with an explicitly scrubbed environment (empty env or
  `--env-file /dev/null` equivalent) to prove the `:?` guard triggers.
- **R-8 (AC-32 — ports assertion accepts either `0.0.0.0` or
  `127.0.0.1`)**: Guard — the test must assert on the literal string
  `127.0.0.1:8080:8080` in the rendered yaml. Accepting either binding
  defeats the point of the AC.
- **R-9 (AC-31 — API table skips `rotate-webhook-secret`)**: Guard —
  `readme_quickstart_test.rs` must substring-check for
  `/api/agents/:id/rotate-webhook-secret` specifically, not just any
  `rotate` string.

## Clarifications Needed

None. Scope.md v5 and design.md v5 explicitly resolve all open questions:

- OpenRouter is the default `LLM_API_BASE_URL`; OpenAI ships as a
  commented alternative in `.env.example`.
- Default port binding is `127.0.0.1:8080:8080`; `0.0.0.0` override is
  documented in README Troubleshooting.
- PowerShell smoke script is shipped with structural validation in CI;
  real end-to-end execution happens on the Windows dev host.
- Caddy binary is not required in CI; the test falls back to structural
  parse.

## Build Order

Dependencies flow left-to-right; each step produces a committable
checkpoint, closes the cited AC(s), and must pass its test before the
next step starts.

1. **Rewrite `docker-compose.yml` + `.env.example` together**
   (closes AC-28 partial, AC-29 partial, AC-32 partial). They are
   coupled — every compose `${VAR:-default}` must match the
   `.env.example` default byte-for-byte.
2. **Write `tests/compose_env_test.rs` + `tests/env_example_test.rs`**
   (closes AC-28, AC-29, AC-32). Machine-checked contract locked in
   before any docs or scripts depend on it.
3. **Write `scripts/smoke.sh` + `tests/smoke_script_test.rs`**
   (closes AC-30 bash half). Exercise the script against the existing
   compose/test-DB stack. This step produces the canonical list of
   milestone commands that README must subsequently match.
4. **Port to `scripts/smoke.ps1`** (closes AC-30 Windows half).
   Exercise it on the author's Windows box; capture output in the BUILD
   log. CI test is structural (syntax parse) per design.md.
5. **Rewrite README Quick Start + `tests/readme_quickstart_test.rs`**
   (closes AC-31). Anchors and step content must mirror step 3
   exactly. Includes updating the API table with
   `POST /api/agents/:id/rotate-webhook-secret`.
6. **Write `docker-compose.caddy.yml` + `Caddyfile.example` +
   `tests/caddy_overlay_test.rs` + README "Going public with HTTPS"
   subsection** (closes AC-33). Last because the README structure
   from step 5 must be in place before the subsection is added.

Each step ends with `cargo test` green (plus `pwsh` validation for step
4), a `git add -A && git commit` checkpoint, and a `scaffolding/log.md`
entry referencing the closed `AC-*` IDs.

## Complexity Exceptions

None. Per design.md v5 addendum:

- Every new file stays under 300 lines.
- Compose baseline has zero added complexity for operators who skip
  TLS — the Caddy overlay is a separate file activated with an extra
  `-f` flag.
- The v4 `src/cli/**` 600-line budget is unchanged — v5 touches no
  Rust source outside the `tests/` directory.

---

_Readiness records for v1–v4 live in git history (see commit `9013ff7` and
earlier tags). This file is authoritative for v5 only._

<!-- Historical v1–v4 readiness content intentionally removed from this file.
     Retrieve with: `git show 9013ff7:scaffolding/readiness.md` -->

<!-- HISTORICAL_TAIL_REMOVED

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
- **T-29**: The browser UI is vanilla JavaScript served statically: no bundler, no framework, no TypeScript, no external CDN fetches. It is split by concern across ES modules: `static/index.html` (shell, `<script type="module" src="/js/app.js">`), `static/js/{app,api,state,ui}.js`, `static/js/views/{login,agents,detail,settings}.js`, `static/css/style.css`. Five hash-routed views exist: `#/login`, `#/agents`, `#/agents/:id`, `#/agents/:id/settings`, plus the send-message form on the detail view. The live event stream is a real long-poll (in `state.js`) against `GET /api/agents/:id/events?since=<last_id>` (4s interval, exponential backoff on error capped at 32s).
- **T-30**: `docs/api.md` is the HTTP API contract. It documents every endpoint the `pcy` CLI or browser UI calls. It declares the v4 surface **stable through v5**: endpoints may be added, but none will be removed or renamed, and documented request/response field types will not change incompatibly without a major version bump. Every endpoint reachable from `src/api/` that CLI/UI consume is documented; every documented endpoint exists in `src/api/`.
- **T-31**: v4 introduces **no database schema changes**. `agents.budget_limit_usd` and `agents.budget_used_usd` already exist since v1. No new migrations land in v4.

## v4 Key Links

- **L-16** AC-22 → `Dockerfile` (runtime stage: non-root user `pcy` UID 10001, `USER pcy`, `--chown=pcy:pcy` on COPY) → `tests/docker_nonroot_test.sh` (shell, gated by `DOCKER_AVAILABLE=1`) → Runtime proof: `docker compose up -d` → `docker compose exec app id -u` returns `10001`; `docker compose exec app touch /etc/x` returns non-zero; `curl http://localhost:8080/health` returns 200.
- **L-17** AC-23 → `src/background/listener.rs` (pre-acquire budget check; appends `budget_exceeded` event, returns before `agent::acquire_wake`) + `src/runtime/llm.rs` (`Pricing` struct + `LlmClient::with_pricing(...)` builder wired from `LLM_PRICE_{,MAINTENANCE_}{INPUT,OUTPUT}_PER_MTOK` env vars in `src/main.rs`) + `src/models/llm_call.rs` (LLM-call insert and `agents.budget_used_usd` increment live in one transaction) → **refusal path** `tests/budget_test.rs` (seed `budget_limit_usd = 0.000001`, `budget_used_usd = 0.000002`, POST a message, wait for listener, assert `SELECT count(*) FROM llm_calls WHERE agent_id=…` unchanged, `SELECT count(*) FROM events WHERE agent_id=… AND event_type='budget_exceeded'` == 1, `agents.status = 'asleep'`); **cost-accumulation path** `tests/wake_loop_test.rs::test_wake_loop_sleep_termination` (seed `Pricing::new(3.0, 15.0)`, mock LLM returning `Usage { prompt_tokens: 100, completion_tokens: 10 }`, complete one wake cycle, assert `SELECT cost_usd FROM llm_calls` == `Decimal(0.00045)` and `SELECT budget_used_usd FROM agents` == `Decimal(0.00045)`).
- **L-18** AC-24 → `src/api/agents.rs` (handler `rotate_webhook_secret_handler` + `POST /agents/{id}/webhook/rotate` route, inlined next to PATCH/DELETE under `auth_middleware`; no separate `webhook_rotate.rs` module) + `src/models/agent.rs::rotate_webhook_secret_tx` + `src/models/event.rs::append_event_tx` (append `webhook_secret_rotated` in the same transaction) → `tests/webhook_rotate_test.rs` → Runtime proof: create agent, capture original secret from create response, POST rotate → assert 200 + new secret in body; send signed webhook using old secret → assert 401; send signed webhook using new secret → assert 202; query events → assert one `webhook_secret_rotated` row with empty/NULL payload.
- **L-19** AC-25 → `src/bin/pcy.rs` (thin shim) + `src/cli/mod.rs` (clap `Parser`/`Subcommand`, dispatch) + `src/cli/config.rs` (read/write `~/.config/open-pincery/config.toml` via `dirs::config_dir()`) + `src/cli/commands/{bootstrap,login,agent,message,events,budget,status}.rs` + `src/api_client.rs` (shared HTTP client) + `Cargo.toml` (second `[[bin]]`, `clap`, `toml`, `dirs` deps) → `tests/cli_e2e_test.rs` (`assert_cmd` against a live test server) → Runtime proof: `pcy bootstrap` against running server → config file written with token; `pcy agent create` → agent exists; `pcy message <id> hello` → message event appears; `pcy events <id> --tail` → stream shows wake events; `pcy agent rotate-secret <id>` → new secret printed; `pcy status` → exits 0.
- **L-20** AC-26 → `static/index.html` (SPA shell, `<script type="module" src="/js/app.js">`) + `static/js/{app,api,state,ui}.js` + `static/js/views/{login,agents,detail,settings}.js` (hash router + 4 views + long-poll event stream in `state.js`) + `static/css/style.css` (minimal reset + utility) — served by existing axum static handler. Layers are split by concern as ES modules rather than a single `app.js`; largest file is `views/detail.js` at 132 lines → `tests/ui_smoke_test.rs` → Runtime proof: GET `/` returns 200 with `index.html` body containing `#app`; GET `/js/app.js` returns 200; `api.js` / `views/*.js` source grep-asserts `#/login`, `#/agents`, `/api/agents`, `since=`; headless probe (optional, gated) drives login → list → detail and asserts a `wake_started` event surfaces in the stream within 5s of posting the message form; rotate button issues `POST /api/agents/:id/webhook/rotate` and `agents.webhook_secret` changes.
- **L-21** AC-27 → `docs/api.md` (one section per public endpoint: method + path, required headers, request body typed fields, response body per status code, side effects) → REVIEW subagent pass (no automated test; cross-reference against `src/api/`) → Runtime proof: REVIEW lists every route registered in `src/api/mod.rs` that CLI/UI call and confirms a matching section exists in `docs/api.md`; stability banner (`Stability: v4 → v5 compatible`) present at the top.

## v4 Acceptance Criteria Coverage

| AC    | Component                                                                                                                                                                                                                                                                   | Test                                                                                                  | Runtime Proof                                                                                                                                                                                                                                                                                                                                        |
| ----- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC-22 | `Dockerfile` (runtime stage, non-root `pcy` UID 10001)                                                                                                                                                                                                                      | `tests/docker_nonroot_test.sh` (gated by `DOCKER_AVAILABLE=1`)                                        | `docker compose exec app id -u` → `10001`; `docker compose exec app touch /etc/x` → non-zero; `curl /health` → 200                                                                                                                                                                                                                                   |
| AC-23 | `src/background/listener.rs` (pre-CAS budget check) + `src/runtime/llm.rs` (`Pricing` struct + `LlmClient::with_pricing`, wired from `LLM_PRICE_*` env in `src/main.rs`) + `src/models/llm_call.rs` (in-tx `budget_used_usd` increment)                                     | `tests/budget_test.rs` (refusal) + `tests/wake_loop_test.rs::test_wake_loop_sleep_termination` (cost) | **Refusal**: seed `budget_limit_usd=0.000001`, `budget_used_usd=0.000002`; POST message; assert 0 new `llm_calls`, exactly 1 new `budget_exceeded` event, `agents.status='asleep'`. **Cost**: `Pricing::new(3.0, 15.0)` + `Usage { prompt_tokens: 100, completion_tokens: 10 }` → `llm_calls.cost_usd = 0.00045`, `agents.budget_used_usd = 0.00045` |
| AC-24 | `src/api/agents.rs::rotate_webhook_secret_handler` (inlined next to PATCH/DELETE; no separate `webhook_rotate.rs`) + `src/models/agent.rs::rotate_webhook_secret_tx` + `src/models/event.rs::append_event_tx`, all under `auth_middleware` + `scoped_agent` workspace guard | `tests/webhook_rotate_test.rs`                                                                        | POST rotate → 200 + new secret returned once; POST webhook signed with old secret → 401; POST webhook signed with new secret → 202; `webhook_secret_rotated` event present                                                                                                                                                                           |
| AC-25 | `src/bin/pcy.rs` (thin) + `src/cli/**` + `src/api_client.rs` + `[[bin]] pcy` in `Cargo.toml`                                                                                                                                                                                | `tests/cli_e2e_test.rs` (`assert_cmd`)                                                                | End-to-end shell flow: `pcy bootstrap` → `pcy agent create` → `pcy message` → `pcy events --tail` → `pcy agent rotate-secret` → `pcy status` exits 0; no direct `curl` in test                                                                                                                                                                       |
| AC-26 | `static/index.html` + `static/js/{app,api,state,ui}.js` + `static/js/views/{login,agents,detail,settings}.js` + `static/css/style.css` (split by concern as ES modules; largest file `views/detail.js` at 132 lines)                                                        | `tests/ui_smoke_test.rs`                                                                              | GET `/` returns SPA shell; router in `app.js` dispatches 5 hash routes; `state.js` long-polls `GET /api/agents/:id/events?since=…`; posted message surfaces `wake_started` within 5s; rotate-secret button mutates `agents.webhook_secret`                                                                                                           |
| AC-27 | `docs/api.md`                                                                                                                                                                                                                                                               | REVIEW subagent cross-reference                                                                       | Every endpoint called from `src/cli/**` or `static/js/**` is documented; every documented endpoint is registered in `src/api/mod.rs`; stability statement (`v4 → v5 compatible`) present                                                                                                                                                             |

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
5. **Slice 5 — AC-26 UI**: Replace `static/index.html` with an ES-module shell; add `static/js/{app,api,state,ui}.js` + `static/js/views/{login,agents,detail,settings}.js` (vanilla JS hash router, 5 views, long-poll with exponential backoff in `state.js`) and `static/css/style.css` (minimal reset + utility, ~150 lines). Add `tests/ui_smoke_test.rs`. Relies on Slices 2–4's endpoints being stable. Files are split by concern; no single file should exceed ~200 lines.
6. **Slice 6 — AC-27 `docs/api.md`**: Written last, once all endpoints touched by Slices 2–4 are stable. One section per endpoint reachable from CLI (Slice 4) or UI (Slice 5). Stability banner (`v4 → v5 compatible`) at the top. REVIEW subagent cross-references `docs/api.md` ↔ `src/api/` ↔ CLI/UI callers. Pure docs, no code.

## v4 Complexity Exceptions

Carried forward from `scaffolding/design.md` v4 section:

- **`static/js/**`— split by concern, no single-file ceiling.** BUILD moved away from the original single-file`static/app.js` plan to four ES-module layers (`app.js`, `api.js`, `state.js`, `ui.js`) plus one file per view (`views/{login,agents,detail,settings}.js`). Served as-is by the existing axum static handler via `<script type="module" src="/js/app.js">`— still no bundler, no framework, no build step, no CDN fetches. Largest file:`views/detail.js` at 132 lines. The previous ~400-line single-file ceiling is retired; if any single module grows past ~200 lines, split it further.
- **`src/cli/**`— 600-line total budget** across`src/cli/mod.rs`, `src/cli/config.rs`, and `src/cli/commands/\*.rs`. Justification: a second binary in the same crate is cohesive with the runtime and shares `src/api_client.rs`; extracting to a separate workspace member is premature at v4 size. If v5 pushes the CLI past 600 lines, extract to a workspace member per preferences.md convention.

HISTORICAL_TAIL_REMOVED -->
