# Preferences

This file is consumed by the agent during BUILD. It contains implementation-level rules — what tools to use, how to structure code, what patterns to follow. Strategic and architectural decisions live in the TLA+ spec and design documents.

## Platform Stack

This repo is not a generic starter. Open Pincery has already chosen its implementation stack, and BUILD must follow that choice unless the TLA and architecture docs are updated first.

| Concern              | Required Choice                                     | Notes                                                              |
| -------------------- | --------------------------------------------------- | ------------------------------------------------------------------ |
| Runtime              | Rust                                                | Wake loop, control plane, background jobs, and security pipeline   |
| Database             | PostgreSQL                                          | Single source of truth for events, projections, audit, and quotas  |
| HTTP/API             | Rust + axum                                         | Webhook ingress and control-plane endpoints                        |
| Async                | tokio                                               | Wake orchestration, background work, LISTEN/NOTIFY handling        |
| SQL                  | sqlx + tokio-postgres                               | Compile-time checked queries plus Postgres notifications           |
| Sandbox              | zerobox                                             | Per-tool execution isolation and secret injection                  |
| Credential isolation | Vault/proxy model (OneCLI or equivalent)            | Must implement TLA §5c proxy-level credential injection guarantees |
| Auth                 | GitHub OAuth, Entra OIDC, generic OIDC, local_admin | Exact provider depends on deployment mode defined in the TLA       |

## Platform Infrastructure

Defaults are open-source, self-hostable, and aligned with the current spec.

- **Compute**: Rust service process plus PostgreSQL; background jobs may run in the same binary or a dedicated worker process using the same codebase.
- **Deployment modes**: `self_host_individual`, `self_host_team`, `saas_managed`, `enterprise_self_hosted`.
- **Reverse proxy**: Caddy is the default self-host recommendation; Nginx or Traefik are acceptable equivalents.
- **Secrets**: environment variables or SOPS/Vault are allowed for bootstrap and runtime infrastructure secrets only. Agent-use credentials must flow through the vault/proxy model, not raw env vars.
- **CI/CD**: GitHub Actions.
- **Monitoring**: Prometheus + Grafana + Loki, with OpenTelemetry where needed. Postgres audit tables remain the system of record for LLM and tool activity.
- **LLM observability**: token usage, cost per call, latency percentiles, prompt-hash lineage, and anomaly detection sourced from audit tables.
- **Email**: SMTP (Mailpit for dev) if notification surfaces are implemented.
- **Domains**: any registrar.

## Conventions

- **Always defer to idiomatic, orthodox solutions.** Use the standard way a language/framework community solves a problem. Don't invent novel patterns when established ones exist. Boring technology wins.
- Prefer simplicity. One file > elegant abstractions for small projects.
- Tests for business logic. Don't test boilerplate.
- README.md in every project — what, how to run, how to deploy.
- No frameworks for the sake of frameworks. Vanilla when it's simpler.
- Error messages should say what went wrong AND what to do about it.
- When making a design decision, ask: "what would a senior, principal engineer at a serious company do?" Do that.
- **Periodically audit scaffolding overhead.** Every process step encodes an assumption about what the model can't do on its own. As models improve, re-examine whether each step is still load-bearing. Strip what's no longer necessary; add new steps where the model's expanded capability enables more ambitious outcomes.
- **Formal spec is the source of truth.** When a TLA+ or equivalent spec exists, implementation follows the spec. Divergence is a bug. Every state, transition, and invariant in the spec must have a corresponding code path. Update the spec first, then the code.
- **Event-sourced systems are append-only.** Never mutate or delete events. State is derived from replaying the log. Schema changes to events use versioned event types, not ALTER on existing schemas. The event log is the system of record.
- **CAS before mutation.** All state transitions on shared resources use Compare-And-Swap (UPDATE ... WHERE version = $expected RETURNING ...). Optimistic concurrency, not locks. If CAS fails, retry or abort — never force.
- **One migration per schema change.** Each SQL schema addition from the TLA+ spec or best-practices.md becomes a single `sqlx migrate add <descriptive_name>` migration. Never bundle unrelated schema changes. Never hand-edit applied migrations.
- **Enum states match the spec exactly.** Rust enum variant names must match the TLA+ state names (`Resting`, `WakeAcquiring`, `Awake`, `ToolDispatching`, etc.). If a state is renamed in the spec, rename the enum variant and update all match arms.
- **Transitions are explicit match arms.** Each TLA+ transition becomes a function that takes the current state and returns the next state (or an error). No implicit state changes. No catch-all `_ =>` arms on state enums — exhaustive matching catches spec drift at compile time.

## Security Baseline

- No secrets in source code. Ever. Use env vars or secrets manager.
- Parameterized queries for all database access.
- HTTPS everywhere.
- Input validation at system boundaries.
- Dependencies from known registries only.

### Agent Platform Security (additive for agent systems)

- **Credentials never enter agent address space.** Vault → sandbox env injection → network proxy substitution. The agent process sees placeholder names, never values.
- **All code execution in sandboxed processes.** Zerobox (or equivalent) for every shell/tool call. No direct host access.
- **Prompt injection defense at every ingestion point.** User input, tool output, webhook payloads, inter-agent messages, MCP tool results — all scanned before entering the LLM context.
- **Audit trail for every LLM call and tool execution.** Model, prompt hash, token count, cost, latency, correlation ID. Stored in Postgres, queryable by CISO.
- **Permission classification for all tools.** Every tool call categorized (read/write/execute/network/destructive) and gated by the agent's permission mode (yolo/supervised/locked).
- **MCP servers require authentication.** Discovered MCP endpoints are registered but not bound to agents without human approval. Credentials flow through the standard vault/proxy path.
- **Inter-agent messages are logged with cross-user attribution.** If agent A (owned by user X) messages agent B (owned by user Y), both sides are recorded with owner IDs.

## Quality Bar

Three tiers. Pick the right one in `scaffolding/scope.md`. When in doubt, go one tier up.

### Shed (personal tool, quick script, POC, simple package, client-side WASM tool)

May have users, but no tracking, no accounts, no server-side state.

| Artifact / Practice | Required?                                                                     |
| ------------------- | ----------------------------------------------------------------------------- |
| README.md           | Yes — what it is + how to run (5 lines minimum)                               |
| DELIVERY.md         | Yes — unified template, sections as brief as appropriate                      |
| .gitignore          | Yes                                                                           |
| Tests               | Yes — agent writes tests for verification loop. Proves it works autonomously. |
| CI/CD               | No                                                                            |
| Deploy              | Manual / local / publish to registry                                          |
| Monitoring          | No                                                                            |
| Security review     | Basic — no secrets in code                                                    |
| LICENSE             | No — user adds later if needed                                                |
| CONTRIBUTING.md     | No                                                                            |
| Scaffolding         | Persists — provenance record for iteration and audit                          |

### House (real project, tracked users, persistent data)

| Artifact / Practice | Required?                                                   |
| ------------------- | ----------------------------------------------------------- |
| README.md           | Yes — what, setup, run, deploy, test                        |
| DELIVERY.md         | Yes — unified template, full depth                          |
| .gitignore          | Yes                                                         |
| Tests               | Yes — key paths, business logic                             |
| CI/CD               | Yes — automated tests on push                               |
| Deploy              | Automated to single target                                  |
| Monitoring          | Error tracking at minimum                                   |
| Security review     | Input validation, dependency audit, no secrets              |
| LICENSE             | No — user adds later if needed                              |
| CONTRIBUTING.md     | If open-source or team project                              |
| CHANGELOG.md        | Recommended                                                 |
| Custom agents       | Yes — create `.github/agents/` as roles emerge during BUILD |
| Scaffolding         | Persists — provenance record for iteration and audit        |

### Skyscraper (complex system, multiple users, money)

| Artifact / Practice   | Required?                                                   |
| --------------------- | ----------------------------------------------------------- |
| README.md             | Yes — comprehensive, onboarding-grade                       |
| DELIVERY.md           | Yes — unified template, comprehensive depth                 |
| .gitignore            | Yes                                                         |
| Tests                 | Full — unit, integration, e2e                               |
| CI/CD                 | Yes — with staging environment                              |
| Deploy                | Staged (canary or blue-green)                               |
| Monitoring            | Metrics, alerts, dashboards                                 |
| Security review       | Threat model, dependency scanning, secrets rotation         |
| LICENSE               | No — user adds later if needed                              |
| CONTRIBUTING.md       | Yes                                                         |
| CHANGELOG.md          | Yes                                                         |
| RUNBOOK.md            | Yes — incident response, rollback procedures                |
| SBOM                  | Yes — CycloneDX or SPDX format, generated at build time     |
| Formal spec           | Yes — TLA+ (or equivalent). Spec updated before code.       |
| Audit schema          | Yes — LLM calls, tool calls, credential access, messages    |
| Event schema registry | Yes — versioned event types, append-only log integrity      |
| LLM observability     | Yes — cost/token/latency dashboards, anomaly alerts         |
| Custom agents         | Yes — create `.github/agents/` as roles emerge during BUILD |
| Scaffolding           | Persists — full provenance record, design.md is living doc  |

## Toolchain Rules

These prevent common agent tarpits. Follow them exactly.

### Python

- **Use `uv`**, not pip, not poetry, not conda. Always.
- Project init: `uv init` → produces `pyproject.toml` (the single source of truth for deps, metadata, tool config).
- Add deps: `uv add <package>`. Dev deps: `uv add --dev <package>`.
- Run anything: `uv run python ...`, `uv run pytest`, etc.
- Build: use the default build backend from `uv init`. `pyproject.toml` only — no `setup.py`, no `setup.cfg`.
- **Never activate a venv.** `uv run` handles it. Venv activation does not persist between terminal calls in agent mode — this is a tarpit.
- **Never use `pip install`.**

### Rust

- Project init: `cargo init` (existing dir) or `cargo new <name>`
- Build: `cargo build`. Test: `cargo test`. Run: `cargo run`.
- For WASM: use `trunk` (install: `cargo install trunk`). Build: `trunk build`. Serve: `trunk serve`.
- Expect first compile to be slow (minutes). This is normal, not a hang.
- **Database (sqlx):** Use `sqlx` with compile-time query checking (`sqlx::query!` / `sqlx::query_as!`). Queries are verified against a live database at compile time — SQL errors become compile errors. Run `cargo sqlx prepare` to generate offline query metadata for CI.
- **Migrations:** Use `sqlx-cli` (`cargo install sqlx-cli`). Create: `sqlx migrate add <name>`. Run: `sqlx migrate run`. Migrations are `.sql` files in `migrations/`, applied in order. Never hand-edit applied migrations; add a new one.
- **CAS pattern:** All state transitions on shared rows use `UPDATE ... SET version = version + 1 WHERE id = $1 AND version = $2 RETURNING *`. Check rows_affected = 1. If 0, another writer won — retry or abort.

### Go

- Project init: `go mod init <module-path>`
- Always use modules. Never rely on GOPATH.

### Node / TypeScript

- **Use `npm`** unless the project already has a different lockfile.
- Always `npm install` before running anything.
- For global CLI tools: `npx <tool>` instead of global install.

### Browser Testing (Playwright)

- Install: `uv add playwright` then `uv run playwright install --with-deps`
- The `--with-deps` flag installs system browser dependencies. Without it, tests will fail on a fresh machine.

### General Anti-Tarpits

- **Always pass `-y` or `--yes`** to any command that might prompt for confirmation (apt, brew, npm init, etc.).
- **Never run interactive commands.** If a tool requires interactive input, find the non-interactive flag or config file alternative.
- **Always check tool availability first.** Before using ANY tool for the first time in a session, run `which <tool>` (Unix) or `where <tool>` (Windows). If missing, install it or STOP. Do not assume anything is on PATH.
- **Timeouts on long commands.** If a build or test takes longer than expected, check if it's actually running (not hung). Rust compiles and `playwright install` are legitimately slow.
- **No global installs** unless it's a CLI tool you'll reuse (trunk, uv, docker). Everything else goes in the project.

### Agent Platform Testing

These tests map directly to TLA+ invariants. Each test category corresponds to a section of the spec. When adding a new spec section, add the corresponding test category here.

- **Deterministic replay tests.** Seed the event log with known events, replay the wake loop, assert the agent produces the expected tool calls and state transitions. Mock the LLM with canned responses.
- **Permission boundary tests.** For each permission mode (yolo/supervised/locked) × tool category, verify that the permission gate accepts or rejects correctly. These are table-driven unit tests.
- **Credential isolation tests.** Verify that sandbox processes receive credentials via env vars, that proxy substitution works for allowed hosts, and that disallowed hosts are blocked. Assert credentials never appear in event logs, LLM prompts, or tool outputs.
- **CAS contention tests.** Simulate concurrent wake acquisition: two workers try to CAS the same agent from Resting → Awake. Exactly one succeeds; the other gets WakeAcquireFailed. Use `tokio::spawn` with a shared test database.
- **Event log integrity tests.** Assert events are append-only. After a wake cycle, verify event count increased monotonically and no prior events were modified.
- **Prompt injection boundary tests.** Feed known injection payloads through each ingestion point (webhook, tool result, inter-agent message). Verify the defense layer flags or neutralizes them before they reach the LLM context.

## Open Pincery Implementation Rules

These are project-specific. They apply only to this repo (betterclaw / Open Pincery).

### Source-of-Truth Files

- `OpenPinceryAgent.tla`: Governs the state machine, transitions, and invariants. Spec first. Code must implement every state and transition. Divergence is a bug.
- `technical-stack.md`: Governs core platform crates and Postgres mapping. New infrastructure crates or any new data store require updating this file first. Support crates must also be listed in the allowlist below.
- `security-architecture.md`: Governs security layers and the threat model. All security implementation follows the 6-layer model. New attack surfaces must be added here before mitigations are coded.
- `best-practices.md`: Governs patterns from academic research. SQL schemas defined there are canonical starting points when the TLA or current phase plan adopts them; otherwise treat them as staged roadmap patterns, not immediate build requirements.

### Crate Allowlist

Only these crates (and their transitive deps) are permitted without explicit approval. This prevents dependency sprawl during agentic BUILD loops.

| Crate                            | Purpose                                       |
| -------------------------------- | --------------------------------------------- |
| `axum`                           | HTTP API + webhook ingress                    |
| `tokio`                          | Async runtime                                 |
| `sqlx`                           | Compile-time SQL                              |
| `tokio-postgres`                 | LISTEN/NOTIFY                                 |
| `serde` / `serde_json`           | Serialization                                 |
| `reqwest`                        | LLM API calls                                 |
| `zerobox`                        | Per-tool sandbox execution                    |
| `uuid`                           | ID generation                                 |
| `chrono`                         | Timestamps                                    |
| `sha2`                           | Hashing (prompt/event/webhook dedup)          |
| `tracing` / `tracing-subscriber` | Structured logging                            |
| `tower` / `tower-http`           | Middleware (rate limiting, CORS, compression) |
| `clap`                           | CLI argument parsing                          |
| `dotenvy`                        | Env var loading for dev                       |
| `thiserror` / `anyhow`           | Error handling                                |

Adding a crate not on this list requires updating this table first with justification.

### Module Structure

```text
src/
  main.rs              — Entry point, axum router, LISTEN/NOTIFY loop
  state.rs             — AgentState enum (must match TLA+ states exactly)
  transitions.rs       — One function per TLA+ transition
  events.rs            — Event types (versioned, serde-tagged enum)
  db/
    mod.rs             — Connection pool setup
    agents.rs          — Agent CRUD, CAS operations
    events.rs          — Event log insert/query
    projections.rs     — Identity + work list read/write
    audit.rs           — LLM calls, tool audit, credential audit tables
  llm/
    mod.rs             — LLM client trait
    openrouter.rs      — OpenRouter/OpenAI-compatible implementation
  tools/
    mod.rs             — Tool dispatch, permission checking
    shell.rs           — Sandboxed shell execution (Zerobox)
    mcp.rs             — MCP tool discovery and dispatch
    verify.rs          — Built-in compile/lint/test/typecheck/schema validation tools
  security/
    mod.rs             — Prompt injection scanning, credential proxy
    permissions.rs     — Permission mode + category classification
    credentials.rs     — Vault integration, proxy injection
  webhooks/
    mod.rs             — Webhook ingress, dedup, normalization
migrations/            — sqlx migrations (one per schema change)
tests/
  replay.rs            — Deterministic wake loop replay tests
  permissions.rs       — Permission boundary table-driven tests
  cas.rs               — Concurrent CAS contention tests
  credentials.rs       — Credential isolation tests
  events.rs            — Event log integrity tests
  injection.rs         — Prompt injection boundary tests
```

The agent must create files in this structure. If a new concern emerges during BUILD that doesn't fit, add a module and update this table — don't scatter code.

### BUILD Stage Checklist

When the agentic loop enters BUILD, the agent must:

1. Read `OpenPinceryAgent.tla` to identify which states/transitions are being implemented this cycle.
2. Read `best-practices.md` and `security-architecture.md` for any schemas or patterns referenced by the target section.
3. Write the migration(s) first. Run `sqlx migrate run`. Verify.
4. Write the Rust implementation. Enum variants and function names must match TLA+ names.
5. Write the corresponding test(s) from the Agent Platform Testing section.
6. Run `cargo test`. Fix until green.
7. Run `cargo clippy`. Fix all warnings.
8. If the repo is using branch-based iteration, commit to the working branch with a message referencing the TLA+ section. Protected-branch pushes, merges, releases, and deployments happen only after REVIEW → RECONCILE → VERIFY and any required approval gates pass.
