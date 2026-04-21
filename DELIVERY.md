# DELIVERY.md — Open Pincery v7

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

## v5 Changes (from v4) — Operator Onramp

- AC-28: `docker-compose.yml` env block rewritten — every runtime-read env var forwarded via `${VAR:-default}` interpolation; required secrets (`OPEN_PINCERY_BOOTSTRAP_TOKEN`, `LLM_API_BASE_URL`, `LLM_API_KEY`) use `:?` fail-fast guards. No hardcoded tokens or credentials remain.
- AC-29: `.env.example` refreshed to cover every `env::var` call in the source. Grouped by function (server, LLM, auth, budget, stale recovery, observability), commented with purpose and defaults. OpenRouter default + commented OpenAI alternative.
- AC-30: End-to-end smoke scripts (`scripts/smoke.sh` + `scripts/smoke.ps1`) exercise `docker compose up --wait` → health poll → `pcy bootstrap` → agent create → message → event query → assert `message_received`. Both use `curl.exe` explicitly to avoid PowerShell alias issues.
- AC-31: `README.md` Quick Start rewritten — Web UI path, `pcy` CLI path, curl/HTTP appendix, signed binary install, troubleshooting (7 anchors), reset, going public with HTTPS, observability. API table includes canonical `POST /api/agents/:id/webhook/rotate` with compat note for legacy `rotate-webhook-secret` spelling.
- AC-32: Secure-by-default compose — host ports bound to `127.0.0.1`, `.env.example` defaults `OPEN_PINCERY_HOST=0.0.0.0` so the app is reachable inside the Docker network (loopback restricted by port mapping on host side).
- AC-33: Caddy TLS overlay (`docker-compose.caddy.yml` + `Caddyfile.example`) for HTTPS exposure. `docker compose -f docker-compose.yml -f docker-compose.caddy.yml up -d` adds Caddy fronting the app on ports 80/443.

## v6 Changes (from v5) — Security Baseline

- AC-34: `AgentStatus` enum (`Resting`, `WakeAcquiring`, `Awake`, `WakeEnding`, `Maintenance`) in `src/models/agent.rs` with compile-time-aligned TLA+ state names. All SQL CAS status literals route through `AgentStatus::DB_*` consts + `as_db_str` / `from_db_str`; a static guard test (`tests/no_raw_status_literals.rs`) prevents relapse. Migration `20260420000001_agent_status_states.sql` additively widens the `agents.status` CHECK constraint.
- AC-35: Tool capability gate. `src/runtime/capability.rs` defines `ToolCapability` (5 variants) and `PermissionMode` (`Yolo`, `Supervised`, `Locked`, fail-closed on unknown). `dispatch_tool` consults `mode_allows` **before** any executor side effect; denials emit a `tool_capability_denied` event (source=`runtime`, payload `{required_capability, permission_mode}`) and never spawn a child. 9 tests including a DB-backed integration test proving a Locked agent's shell call is denied, audited, and the probe file is never created.
- AC-36: Hardened `ProcessExecutor` behind a `ToolExecutor` trait (`src/runtime/sandbox.rs`). Every shell invocation now runs under: (1) pre-spawn rejection of any command containing a `sudo` token (tokenised on shell word-boundaries — catches `echo ok && sudo …`, `(sudo -i)`, etc.); (2) fresh per-call tempdir as cwd; (3) `Command::new("sh").env_clear()` with a `PATH`-only allowlist re-added; (4) `kill_on_drop(true)`; (5) 30s wall-clock timeout via `tokio::time::timeout`. Exactly one `Command::new(` exists under `src/runtime/` — enforced by `tests/no_raw_command_new.rs`. 6 sandbox tests (env scrub, timeout-does-not-hang, three sudo-reject variants incl. chained, Ok path).
- AC-37: Zero-advisory-or-allowlisted-exception floor. `deny.toml` `[advisories]` uses cargo-deny v2 (implicit vulnerability deny) with `yanked = "deny"`. The `ignore` list contains exactly one dated, documented entry — `RUSTSEC-2023-0071` (transitive `rsa` via unused `sqlx-mysql`, no upstream fix, not reachable in our runtime). `tests/deny_config_test.rs` pins `ALLOWED_ADVISORIES = ["RUSTSEC-2023-0071"]` and requires a non-empty `reason` on every entry, so adding a new exception requires a deliberate co-edit of both `deny.toml` and the test.

### v6 Operator Impact

- **New permission mode field**: agents carry `permission_mode` (default `yolo` for v5 compatibility). Set to `locked` to fully disable tool execution; `supervised` is reserved for future approval flows (currently behaves like `locked` for destructive tools).
- **No behaviour change for existing agents**: v6 is additive. v5 agents continue to wake, call tools, and complete cycles. The security baseline kicks in only when `permission_mode` is tightened.
- **Audit trail**: every gate denial is persisted as a `tool_capability_denied` event alongside `tool_result` / `tool_error`, so denials are queryable via the existing event API.

## v7 Changes (from v6) — Credential Vault & Reasoner-Secret Refusal

- AC-38: AES-256-GCM credential vault (`src/runtime/vault.rs`). Master key loaded from `OPEN_PINCERY_VAULT_KEY` (base64-encoded 32 bytes); startup fails fast if missing, wrong length, or invalid base64. `Vault::seal(workspace_id, name, plaintext) → SealedCredential {nonce, ciphertext}` binds the credential to its `{workspace_id}:{name}` AAD; `Vault::open(workspace_id, name, sealed)` collapses all failure modes (wrong key, wrong workspace, wrong name, tampered ciphertext, wrong nonce length) to a single `VaultError::Authentication` variant — no oracle. Random 96-bit nonce per seal via `OsRng`. `credential::Credential` is deliberately NOT `Serialize`; only `CredentialSummary` (id, name, created_at, revoked_at) ever leaves the process.
- AC-39: REST credential API under `/api/workspaces/:id/credentials`. POST accepts `{name, value}` (name `^[A-Z][A-Z0-9_]{0,63}$`, value ≤ 32 KiB), seals it, and returns the summary only (never the value). GET lists active (non-revoked) summaries. DELETE `/api/workspaces/:id/credentials/:name` marks revoked. All three routes are workspace-admin gated via `scoped_workspace`. Unique partial index (`name`) WHERE `revoked_at IS NULL` enforces one-active-per-name at the DB layer.
- AC-40: `pcy credential add|list|revoke` CLI. `add` uses `rpassword` so the secret is never echoed, never in argv, never in shell history; auto-resolves `workspace_id` via `GET /api/me` (cached in `CliConfig.workspace_id` after first use). `list` prints name + created_at + age in a table. `revoke` takes `--name`, confirms, and calls DELETE.
- AC-41: `list_credentials` reasoner tool registered with `ToolCapability::ReadLocal`. Returns `{credentials:[{name, created_at}, ...]}` — names only, never values. The gate in AC-35 still applies; a Locked agent cannot list. `workspace_id: Uuid` is now a required param on `dispatch_tool` so every tool call is workspace-scoped.
- AC-42: Hardened wake system prompt v2. Migration `20260420000003_prompt_template_credentials.sql` deactivates v1 and inserts v2 with five mandatory substrings (REFUSE contract, "never reveal", "never echo", the `PLACEHOLDER:<name>` syntax, and the `list_credentials` tool reference). `tests/prompt_v2_credential_test.rs` pins all five substrings so a silent prompt rewrite fails the build.
- AC-43: PLACEHOLDER dispatch handshake. `ShellArgs.env: HashMap<String,String>` lets the reasoner pass `PLACEHOLDER:<name>` values; `dispatch_tool` resolves them pre-spawn via `credential::find_active` + `vault.open` into a private `HashMap`, which becomes `ShellCommand.env` and is injected AFTER the `PATH`-only allowlist. On any failure (missing, revoked, invalid nonce, authentication, non-UTF-8, lookup error) the call fails closed — no executor spawn — and a `credential_unresolved` event is written with payload `{name, reason}` only. Plaintext never appears in any event, log line, or tool output (leak-canary test scans every event row for the agent after a successful resolve).

### v7 Operator Impact

- **New required env var**: `OPEN_PINCERY_VAULT_KEY` — 32 random bytes, base64-encoded. Generate once with `openssl rand -base64 32`; store alongside `OPEN_PINCERY_BOOTSTRAP_TOKEN`; losing it means losing access to every stored credential. Rotation requires re-sealing — deferred to v8.
- **New CLI verbs**: `pcy credential add|list|revoke`. The `add` path prompts for the value via rpassword and never touches argv/history.
- **New tool available to agents**: `list_credentials` (names only). The reasoner is prompted to use `PLACEHOLDER:<name>` in `env` on any shell call instead of ever pasting a secret value.
- **Zero runtime substitution outside dispatch**: There is no network-level redaction or proxy. If an agent names a credential and also echoes the raw value in its own text, the harness cannot prevent that — the v2 prompt makes this refusal contract explicit. Cryptographic isolation of secrets from the reasoner (Zerobox-style) is the v8/v9 step.
- **Additive migrations**: Three new migration files; no v6 row is mutated.

## Known Limitations

- **Host-level sandbox only**: v6 ships env-clear + tempdir + 30s timeout + sudo-token rejection via `ProcessExecutor`. This is defense-in-depth, not isolation — a process running as the `pcy` user can still read any file that user can read. True container-level isolation (Zerobox) is on the roadmap.
- **Sudo reject is token-based, not path-based**: commands containing a `sudo` token are rejected pre-spawn; commands invoking `/usr/bin/sudo` by absolute path are not caught by the tokeniser and rely on `env_clear` + tempdir + no-tty for defense. Documented in `src/runtime/sandbox.rs`.
- **Credential substitution is reasoner-cooperative**: v7 protects against _accidental_ leakage through event/log/argv paths and against unauthorised dispatch spawns, but a malicious or confused reasoner that types a PLACEHOLDER value into plaintext content will still produce plaintext. Cryptographic isolation (v8/v9) is the structural fix.
- **Vault master-key rotation not yet implemented**: `OPEN_PINCERY_VAULT_KEY` is single-valued; re-sealing on rotation is a v8 item.
- **No inter-agent messaging**: Single-agent operation only
- **Single workspace enforcement**: Multi-tenancy schema exists; `scoped_agent` enforces workspace isolation on agent-level handlers, but cross-workspace administration is not yet exposed via API
- **Webhook secrets**: Still surfaced exactly once — on creation or after `POST /api/agents/:id/rotate-webhook-secret`. Operators must capture the response immediately.
- **Rate limiting is in-process**: Not shared across multiple server instances
- **Metrics recorder is process-global**: Only one Prometheus recorder per process; unit tests that install a recorder must run single-threaded.
- **Release workflow not yet exercised**: `cosign verify-blob` against a real tagged artifact will happen on first `v*` tag push.
- **RUSTSEC-2023-0071** (medium, CVSS 5.9): `rsa 0.9.10` pulled in transitively via unused `sqlx-mysql`. Not reachable at runtime (MySQL driver is never loaded); documented allowlist entry in `deny.toml` keyed by `tests/deny_config_test.rs`. Revisit on upstream `rsa` fix, sqlx 0.9 stable, or migration off `sqlx::FromRow` derive.

## Footprint

- **Runtime**: Single Rust binary (~15MB release), or Docker image
- **Database**: PostgreSQL 16 (19 migration files)
- **External**: One OpenAI-compatible LLM API
- **Stack additions in v7**: `aes-gcm`, `rpassword`, `walkdir` (dev-only)
- **Cost**: PostgreSQL hosting + LLM API usage. No other infrastructure costs.
