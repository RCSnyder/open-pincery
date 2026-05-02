# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Security

- _(AC-78)_ **Event-log hash chain.** Every row in `events` now carries `prev_hash` and `entry_hash` columns, populated atomically by a per-agent `BEFORE INSERT` trigger (migration `20260501000001_add_event_hash_chain.sql`). The trigger uses `pg_advisory_xact_lock(44224, hashtext(agent_id::text))` plus `SELECT FOR UPDATE` against the prior tail to serialize concurrent inserts at genesis and a strict-monotonic `created_at` bump to disambiguate microsecond ties. The canonical pre-image is length-prefixed (4-byte BE length + UTF-8 bytes per field, then 8-byte BE micros for `created_at`) so reordering or partial fields cannot collide. A `DO`-block backfill walks every existing agent's history in `(created_at, id)` order before `prev_hash`/`entry_hash` are set `NOT NULL`.
- _(AC-78)_ **Verifier and CLI / HTTP surface.** New `src/background/audit_chain.rs` exposes `verify_audit_chain(pool, agent_id) -> ChainStatus { Verified | Broken { first_divergent_event_id, expected_hash, actual_hash, events_walked } }`, `verify_workspace`, and `verify_and_emit` which appends one `audit_chain_verified` or `audit_chain_broken` event per agent per call. The verifier never mutates application data — it only emits its own audit-trail events (`source = "audit_chain_verifier"`). New `pcy audit verify [--agent <uuid>] [--workspace <id>]` CLI exits with code `2` when any chain is broken, raw JSON to stdout, one-line-per-agent summary to stderr. New workspace-admin-gated HTTP endpoints `POST /api/audit/chain/verify` and `POST /api/audit/chain/verify/agents/{id}` return the same shape (`{ agents: [...], all_verified: bool }`); the per-agent route 404s on cross-workspace lookups and 403s on non-admin callers.
- _(AC-78)_ **Startup integrity gate.** `enforce_audit_chain_floor_at_startup(pool, relaxed, allow_unsafe)` runs in `src/main.rs` after migrations and before listener bind. On any broken chain it logs `audit_chain_broken` and exits with code `5` (distinct from the AC-84 sandbox-floor exit `4` and the CLI exit `2`). Operators who must boot against a knowingly-broken chain — for example to recover via `docs/runbooks/audit_chain_recovery.md` — can arm both `OPEN_PINCERY_AUDIT_CHAIN_FLOOR=relaxed` and `OPEN_PINCERY_ALLOW_UNSAFE=true`; the gate then emits one `audit_chain_floor_relaxed` event per affected agent and proceeds, so the override is itself part of the audit trail. Either flag alone is refused.
- _(AC-77)_ **Seccomp default-deny allowlist.** Replaced the 11-entry denylist (`mismatch_action=Allow`) with a 58-entry default-deny allowlist sourced empirically from `tests/fixtures/seccomp/observed_syscalls.txt` (kernel 6.6 / glibc 2.39 / x86*64 strace capture of the AC-76 happy-path corpus) plus 17 manually-justified additions (`exit_group`, `clone3`, `prctl`, `futex`, sleep / signal / fs-introspection helpers). `Enforce` mode now sets `mismatch_action=KillProcess` (SIGSYS, exit 159) and `Audit` mode sets `mismatch_action=Log`; both use `match_action=Allow`. `SYS_clone` was moved out of the bare allowlist into an argument filter (`MaskedEq((CLONE_NEWUSER|CLONE_NEWNS) as u64, 0)`) so ordinary thread / process creation is allowed but namespace re-creation falls through to the kill action. `clone3(2)` is bare-allow with namespace lockout delegated to AC-86 (`bwrap --disable-userns` + `--cap-drop ALL` + UID drop) per the readiness `T-AC77-4` rationale. A 17-entry `ESCAPE_PRIMITIVES` negative control (`bpf`, `mount`, `umount2`, `pivot_root`, `init/finit/delete_module`, `kexec*_`, `reboot`, `ptrace`, `io*uring*_`, `perf_event_open`, `name_to_handle_at`, `open_by_handle_at`) is asserted absent from the allowlist at install time, and a size guard refuses to install when the allowlist drifts outside `[40, 120]`.
- _(AC-77)_ **`sandbox_syscall_denied` event surface.** New `src/observability/seccomp_audit.rs` module (`SANDBOX_SYSCALL_DENIED_EVENT`, `SeccompAuditRecord`, `SeccompAuditContext`, `parse_seccomp_audit_record`, `sandbox_syscall_denied_payload`, `append_sandbox_syscall_denied_event`). `bwrap.rs` and `ProcessExecutor` translate signal-induced terminations via the POSIX `128 + signum` convention so SIGSYS surfaces as `exit_code=159` to callers. The dispatch path in `runtime::tools` detects this and emits one event per SIGSYS-terminated tool invocation with `record_correlated=false` and `syscall_nr=-1` until the AUDIT_SECCOMP netlink correlation lands as a follow-up sub-slice.
- _(AC-77)_ New `tests/seccomp_allowlist_test.rs` with three integration tests: happy-path workload coverage (7 commands), namespace-creation primitive blocked by SIGSYS, and a control test that proves the SIGSYS exit is attributable to seccomp rather than another layer. Each test self-skips with an explicit reason if `bwrap` / Landlock / cgroup-v2 preconditions are missing.
- _(AC-77)_ New empirical capture script `scripts/capture_seccomp_corpus.sh` plus reproducible fixture (`tests/fixtures/seccomp/observed_syscalls.txt`, `additions.txt`, `README.md`) so future tool-catalog expansions can re-derive the allowlist via `./scripts/devshell.sh ./scripts/capture_seccomp_corpus.sh > tests/fixtures/seccomp/observed_syscalls.txt`.

## [1.0.1](https://github.com/RCSnyder/open-pincery/compare/v1.0.0...v1.0.1) - 2026-04-21

### Fixed

- _(license)_ dual-license under MIT OR Apache-2.0

### Other

- _(expand)_ v6.1 pre-expand synthesis — external inputs + D1/D2
- _(expand)_ v6 pre-expand synthesis — canonical north star

## [1.0.0](https://github.com/RCSnyder/open-pincery/releases/tag/v1.0.0) - 2026-04-20

### Added

- _(cli)_ add 'pcy demo' for one-command end-to-end smoke test
- _(auth)_ add /api/login endpoint for session token recovery
- _(build)_ v5 operator onramp (AC-28..AC-33)
- _(build)_ v5 slice 1+2 compose + .env.example rewrite with regression tests
- _(build)_ v4 slice 5 deliver vanilla JS control plane (AC-26)
- _(build)_ v4 slice 4 add pcy CLI binary and shared API client (AC-25)
- _(build)_ v4 slice 3 add webhook secret rotation endpoint (AC-24)
- _(build)_ v4 slice 2 enforce budget cap at wake acquire (AC-23)
- _(build)_ v4 slice 1 non-root runtime image (AC-22)
- _(hooks)_ auto-rustfmt on edits + fmt-check gate before git commit
- _(build)_ v3 slice 6 — signed release artifacts with SBOM (AC-20)
- _(build)_ v3 slice 4-5 — CI workflow (AC-16) + operator runbooks (AC-21)
- _(build)_ v3 slice 3 — Prometheus metrics (AC-18)
- _(build)_ v3 slice 1-2 — JSON logging (AC-17) + health/ready split (AC-19)
- _(build)_ implement v2 features AC-11 through AC-15
- _(build)_ implement dashboard UI — bootstrap, agent management, event log
- _(build)_ complete BUILD phase — all 15 tests pass, all 10 ACs covered
- _(build)_ add tests for AC-4, AC-5, AC-7 + fix llm_call/projection schema alignment
- _(build)_ implement full application skeleton - Slice 1 complete

### Fixed

- _(ci)_ checkout main branch in release-plz (not detached HEAD)
- _(ci)_ allow CDLA-Permissive-2.0 license in cargo-deny
- _(build)_ address REVIEW v4 findings and finalize v4 BUILD state
- _(review)_ address v3 review findings (1 Critical + 5 Required + 2 Consider)
- _(review)_ address all Critical and Required review findings
- _(build)_ resolve 7 audit findings from struct/migration mismatches and API gaps
- _(build)_ address REVIEW findings — 2 critical, 6 required fixes

### Other

- _(release)_ cut 1.0.0 and wire up automated releases
- _(input)_ add improvement-ideas brainstorm
- _(build)_ fix Docker build for Rust 1.88 toolchain
- _(deploy)_ v5 delivery — log + DELIVERY.md finalized
- _(analyze)_ v5 readiness — READY verdict
- _(design)_ v5 design addendum — operator onramp contract
- _(expand)_ v5 scope — operator onramp
- _(deploy)_ v4 delivered — README, DELIVERY, log updated
- _(reconcile)_ sync v4 scaffolding with shipped code
- _(build)_ record v4 BUILD gate pass evidence
- _(build)_ narrow sqlx features and refresh lockfile
- _(build)_ add v4 API stability contract (AC-27)
- _(build)_ add static Dockerfile guard for AC-22
- _(iterate)_ v4 readiness — READY
- _(iterate)_ v4 design — CLI/UI/safety integration points
- _(iterate)_ v4 scope — usable self-host (CLI + UI + safety hardening)
- _(hooks)_ split by concern + block destructive commands
- _(deploy)_ v3 delivery — README + DELIVERY.md + log
- rustfmt wrap assert! in json logging test
- _(review)_ log RECONCILE + REVIEW pass 2 PASS
- _(reconcile)_ align design + readiness with v3 code post-review
- _(iterate)_ v3 scope, design, readiness — observability and release hygiene
- _(deploy)_ complete v2 delivery
- _(reconcile)_ fix v2 scaffolding drift
- _(analyze)_ v2 readiness.md — READY verdict, AC-11 through AC-15
- _(iterate)_ version scope.md and design.md for v2 — operational readiness
- update README with accurate quick start and project structure
- _(deploy)_ README + DELIVERY.md, post-deploy gate PASS
- _(verify)_ post-verify gate PASS — all 10 ACs verified, 17/17 tests pass
- _(reconcile)_ fix structural drift across scaffolding documents
- _(build)_ add tests for AC-6, AC-8, AC-9
- _(build)_ add integration tests for AC-1, AC-2, AC-3, AC-10
- _(analyze)_ produce readiness.md — READY verdict
- _(design)_ define architecture for Open Pincery v1
- _(expand)_ define scope for Open Pincery v1 agent runtime
- rewrite README for open-pincery and update LICENSE copyright
- _(input)_ add arch pdf, tla+ spec, and additional arch .md files
- Initial commit
