# Readiness: Open Pincery — v6 (Capability Foundations & Security Baseline)

> This file supersedes the prior v5 readiness record. v5 is shipped; its
> readiness artifact lives in git history (latest commit on the v5 branch
> before the v6 EXPAND commit `c46d4bc`). v6 covers AC-34 through AC-37
> only — the prior AC-1..AC-33 coverage is verified by the shipped v5
> suite and is not re-planned here.

## Verdict

READY

v6 is strictly additive runtime hardening: a typed `AgentStatus` enum behind
a single DB-boundary conversion, a capability classification + permission-mode
gate in front of every tool dispatch, a `ToolExecutor` trait with a hardened
`ProcessExecutor` default (tempdir cwd, `PATH`-only env, 30-second timeout,
no-sudo), and a zero-advisory `cargo deny` floor. No API shape changes, no
new external integrations, one schema migration that only extends an existing
CHECK constraint with two reserved values never written by shipped code.
Every AC has unambiguous pass/fail criteria, a named test file, and a concrete
runtime proof path. No clarifications are unresolved.

## Truths

Non-negotiable statements that must be true in the shipped v6 system:

- **T-v6-1** `src/models/agent.rs` exports
  `pub enum AgentStatus { Resting, WakeAcquiring, Awake, WakeEnding, Maintenance }`
  with `Debug + Clone + Copy + PartialEq + Eq + Hash`.
- **T-v6-2** `AgentStatus::as_db_str` and `AgentStatus::from_db_str` are the
  single conversion boundary between the enum and the lowercase DB strings
  (`asleep`, `wake_acquiring`, `awake`, `wake_ending`, `maintenance`).
- **T-v6-3** Every `status = '…'` / `status IN (…)` occurrence under `src/`
  either lives inside the `AgentStatus` constant-definition block in
  `src/models/agent.rs` or is interpolated from an `AgentStatus::DB_*`
  constant — no free-floating raw status literals remain.
- **T-v6-4** Migration `20260420000001_agent_status_states.sql` extends the
  `agents.status` CHECK constraint to include the reserved values
  `'wake_acquiring'` and `'wake_ending'`. No existing row is mutated.
- **T-v6-5** `src/runtime/capability.rs` defines
  `pub enum ToolCapability { ReadLocal, WriteLocal, ExecuteLocal, Network, Destructive }`
  and `pub enum PermissionMode { Yolo, Supervised, Locked }` with
  `PermissionMode::from_db_str` mapping unknown values to `Locked`.
- **T-v6-6** `capability::required_for` maps `shell → ExecuteLocal`,
  `plan → ReadLocal`, `sleep → ReadLocal`, and any unknown tool name to
  `Destructive`.
- **T-v6-7** `capability::mode_allows` implements the 15-cell table exactly:
  `Yolo` permits all; `Supervised` denies only `Destructive`; `Locked`
  permits only `ReadLocal`.
- **T-v6-8** When `mode_allows` returns `false`, `tools::dispatch_tool`
  appends exactly one `tool_capability_denied` event
  (`event_type = "tool_capability_denied"`, `source = "runtime"`, payload
  JSON `{required_capability, permission_mode}`) and returns
  `ToolResult::Error("tool disallowed by permission mode")` without
  invoking the executor.
- **T-v6-9** `src/runtime/sandbox.rs` defines
  `#[async_trait] pub trait ToolExecutor: Send + Sync` with
  `async fn run(&self, cmd: &ShellCommand, profile: &SandboxProfile) -> ExecResult`.
- **T-v6-10** `SandboxProfile` defaults to `env_allowlist = ["PATH"]`,
  `deny_net = true` (advisory in v6), `timeout = 30s`, and a fresh tempdir
  cwd per call.
- **T-v6-11** `ProcessExecutor::run` rejects any command that, after
  `trim_start`, begins with the word `sudo` without spawning a process,
  returning `ExecResult::Rejected("sudo is not permitted")`.
- **T-v6-12** `ProcessExecutor::run` uses `env_clear()` and re-adds only
  the allowlisted environment variables; spawned children cannot observe
  host env vars outside the allowlist.
- **T-v6-13** `ProcessExecutor::run` wraps child execution in
  `tokio::time::timeout(profile.timeout, …)` and kills the child on
  timeout, returning `ExecResult::Timeout`.
- **T-v6-14** Under `src/runtime/**` the regex `Command::new\(` matches
  exactly once, inside `sandbox.rs`. Every shell invocation routes through
  the `ToolExecutor` trait.
- **T-v6-15** `AppState` (in `src/api/mod.rs`) holds
  `pub executor: Arc<dyn ToolExecutor>`. Production `src/main.rs`
  constructs `Arc::new(ProcessExecutor)`; integration tests may inject
  their own impl.
- **T-v6-16** `wake_loop::run_wake_loop` reads `current.permission_mode`
  on each loop iteration and passes `PermissionMode::from_db_str(&…)` plus
  `state.executor.clone()` into `tools::dispatch_tool`.
- **T-v6-17** `deny.toml` `[advisories]` sets `version = 2`,
  `yanked = "deny"`, and an `ignore` list containing only documented,
  allowlisted exceptions pinned by `tests/deny_config_test.rs`. Version 2
  implicitly denies known vulnerabilities (the legacy `vulnerability = "deny"`
  key was removed in cargo-deny's v2 advisories schema), so these three
  settings together establish the zero-new-advisory floor. As of v6 HEAD
  the allowlist is a single dated entry: `RUSTSEC-2023-0071` (`rsa` via
  `sqlx-macros-core -> sqlx-mysql`; no Postgres-runtime exposure; no
  upstream fix). Any additional entry requires co-editing deny.toml and
  the test's `ALLOWED_ADVISORIES` constant in the same change.
- **T-v6-18** `cargo deny check advisories` exits 0 on v6 HEAD.
- **T-v6-19** No v1–v5 AC regresses: CAS lifecycle, wake loop, maintenance,
  drain, event log, API surface, HMAC verification, rate limiting, budget
  enforcement, webhook rotation, CLI, UI, observability, runbooks, release
  workflow, operator onramp — all unchanged.

## Key Links

- **AC-34** → scope.md v6 AC-34 → design.md v6 AgentStatus interface → `src/models/agent.rs` (`AgentStatus`, `as_db_str`, `from_db_str`, `DB_*` consts) + `migrations/20260420000001_agent_status_states.sql` → `tests/agent_status_test.rs` + `tests/no_raw_status_literals.rs` → runtime proof: unit + static tests pass; `psql` confirms the widened CHECK constraint.
- **AC-35** → scope.md v6 AC-35 → design.md v6 capability interface → `src/runtime/capability.rs` + `src/runtime/tools.rs::dispatch_tool` + `src/runtime/wake_loop.rs` → `tests/capability_gate_test.rs` → runtime proof: integration test observes one `tool_capability_denied` event and `CountingExecutor::spawns() == 0` for a `Locked` agent.
- **AC-36** → scope.md v6 AC-36 → design.md v6 sandbox interface → `src/runtime/sandbox.rs` + `src/api/mod.rs` (`AppState.executor`) + `src/main.rs` → `tests/sandbox_test.rs` + `tests/no_raw_command_new.rs` → runtime proof: (a) env strip, (b) timeout, (c) sudo reject, (d) exactly one `Command::new(` under `src/runtime/`.
- **AC-37** → scope.md v6 AC-37 → design.md v6 deny.toml schema → `deny.toml` `[advisories]` + `.github/workflows/ci.yml` (already wired by v3 AC-16) → `tests/deny_config_test.rs` → runtime proof: `cargo deny check advisories` exits 0.

## Acceptance Criteria Coverage

| AC    | Planned test                                                     | Planned runtime proof                                                                                                                                                                                                                       |
| ----- | ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC-34 | `tests/agent_status_test.rs` + `tests/no_raw_status_literals.rs` | Round-trip all 5 variants; static grep finds zero unguarded raw status literals in `src/`; migration runs cleanly                                                                                                                           |
| AC-35 | `tests/capability_gate_test.rs` (unit + integration)             | 15-cell `mode_allows` table passes; Locked agent + `shell` tool call emits exactly one `tool_capability_denied` event; `CountingExecutor::spawns() == 0`                                                                                    |
| AC-36 | `tests/sandbox_test.rs` + `tests/no_raw_command_new.rs`          | (a) `HOME`/`MY_SECRET` absent from child env; (b) `sleep 60` with 1s timeout → `Timeout`; (c) `sudo`-prefixed command → `Rejected`, no spawn; (d) one `Command::new(` match under `src/runtime/`                                            |
| AC-37 | `tests/deny_config_test.rs` + CI `cargo deny check`              | Parses `deny.toml`; asserts `version = 2`, `yanked = "deny"`, and that the `ignore` list contains ONLY documented exceptions (RUSTSEC-2023-0071 with a dated reason) matching the test's allowlist. Any undocumented entry fails the build. |

## Scope Reduction Risks

- **AC-34 — Enum added but raw string literals left behind**: Tempting to land the enum type and skip the call-site refactor. `tests/no_raw_status_literals.rs` enforces the invariant.
- **AC-34 — Migration that also renames existing rows**: Out of scope. v6 only widens the CHECK constraint; row renames happen in v10 with the CAS pipeline refactor.
- **AC-35 — Gate wired as a no-op**: Tempting to compute `mode_allows` and ignore the result. Integration test asserts the executor is never called for a denied dispatch.
- **AC-35 — Unknown tools fall through to `Yolo`**: Scope locks unknown → `Destructive`. Future tools added without a `required_for` arm are denied by default.
- **AC-35 — Denial collapses into `tool_result` with error body**: Tempting to reuse the existing `tool_result` path. Scope locks a distinct `event_type = "tool_capability_denied"` with a structured payload.
- **AC-36 — `dispatch_tool` still calls `tokio::process::Command` directly**: `tests/no_raw_command_new.rs` enforces exactly one match in `sandbox.rs`.
- **AC-36 — Tempdir cwd skipped "because tests need a stable cwd"**: Tests inject their own executor; production default must be a fresh tempdir. `SandboxProfile.cwd: Option<PathBuf>` exists as the escape hatch.
- **AC-36 — `env_clear()` weakened to preserve a convenient passthrough**: Scope locks `PATH`-only default. Operators who need more configure `SandboxProfile.env_allowlist` per-profile.
- **AC-36 — Timeout implemented as a soft signal instead of a kill**: Scope locks `child.start_kill()` followed by `ExecResult::Timeout`.
- **AC-36 — `sudo` check relaxed to a warning**: Scope locks rejection before spawn.
- **AC-37 — Zero-ignore policy weakened "because of one legacy advisory"**: A single documented, dated exception (RUSTSEC-2023-0071, `rsa` via `sqlx-macros-core -> sqlx-mysql`) is permitted because no upstream fix exists since 2023-11 and the path is not reachable at Postgres runtime. `tests/deny_config_test.rs` pins the exception set via an `ALLOWED_ADVISORIES` allowlist; adding any new entry requires updating both the test and deny.toml in the same change — a STOP-and-raise event.
- **AC-37 — `yanked = "warn"` left in place**: Scope locks `yanked = "deny"`.

## Clarifications Needed

None. The two reserved DB status values (`wake_acquiring`, `wake_ending`) are intentionally unused by shipped transitions; a future TLA+-faithful CAS split is tracked for v10.

## Build Order

Each slice is sized to ship as 1–2 commits. Order is chosen for independence — slice N never blocks on slice M for M > N.

1. **Slice 1 — AC-37 `deny.toml`.** Shortest, most isolated. Update `[advisories]` to `vulnerability = "deny"`, `yanked = "deny"`, `ignore = []`. Add `toml = "0.8"` under `[dev-dependencies]` if not already present. Write `tests/deny_config_test.rs`. Run `cargo deny check advisories` locally — address any transitive advisory here before proceeding.
2. **Slice 2 — AC-34 `AgentStatus` enum.** Add migration `20260420000001_agent_status_states.sql`. Write the enum + helpers + `DB_*` consts in `src/models/agent.rs`. Replace every in-file raw literal with a `const` reference. Write `tests/agent_status_test.rs` + `tests/no_raw_status_literals.rs`. No runtime behavior change — only type-system hardening.
3. **Slice 3 — AC-35 capability gate.** Create `src/runtime/capability.rs`. Extend `tools::dispatch_tool` signature to take `mode: PermissionMode` + `pool` + `agent_id` + `wake_id`. Wire the gate in front of the existing tool-match. Update the one call site in `src/runtime/wake_loop.rs`. Write `tests/capability_gate_test.rs`. Executor is still the legacy in-place `tokio::process::Command` — that is refactored in Slice 4.
4. **Slice 4 — AC-36 `ToolExecutor` trait + `ProcessExecutor`.** Create `src/runtime/sandbox.rs`. Replace the direct `Command::new` inside `src/runtime/tools.rs` with a call through `Arc<dyn ToolExecutor>`. Add `executor: Arc<dyn ToolExecutor>` to `AppState`. Construct `Arc::new(ProcessExecutor)` in `src/main.rs`. Write `tests/sandbox_test.rs` + `tests/no_raw_command_new.rs`.

After Slice 4: `cargo test --all-targets -- --test-threads=1` + `cargo clippy --all-targets -- -D warnings` + `cargo fmt --all -- --check` + `cargo deny check` all pass. Then REVIEW.

## Complexity Exceptions

None. Every new file stays under 200 lines (see design.md v6 addendum for the explicit budget).
