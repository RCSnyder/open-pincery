# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Security

- *(AC-77)* **Seccomp default-deny allowlist.** Replaced the 11-entry denylist (`mismatch_action=Allow`) with a 58-entry default-deny allowlist sourced empirically from `tests/fixtures/seccomp/observed_syscalls.txt` (kernel 6.6 / glibc 2.39 / x86_64 strace capture of the AC-76 happy-path corpus) plus 17 manually-justified additions (`exit_group`, `clone3`, `prctl`, `futex`, sleep / signal / fs-introspection helpers). `Enforce` mode now sets `mismatch_action=KillProcess` (SIGSYS, exit 159) and `Audit` mode sets `mismatch_action=Log`; both use `match_action=Allow`. `SYS_clone` was moved out of the bare allowlist into an argument filter (`MaskedEq((CLONE_NEWUSER|CLONE_NEWNS) as u64, 0)`) so ordinary thread / process creation is allowed but namespace re-creation falls through to the kill action. `clone3(2)` is bare-allow with namespace lockout delegated to AC-86 (`bwrap --disable-userns` + `--cap-drop ALL` + UID drop) per the readiness `T-AC77-4` rationale. A 17-entry `ESCAPE_PRIMITIVES` negative control (`bpf`, `mount`, `umount2`, `pivot_root`, `init/finit/delete_module`, `kexec_*`, `reboot`, `ptrace`, `io_uring_*`, `perf_event_open`, `name_to_handle_at`, `open_by_handle_at`) is asserted absent from the allowlist at install time, and a size guard refuses to install when the allowlist drifts outside `[40, 120]`.
- *(AC-77)* **`sandbox_syscall_denied` event surface.** New `src/observability/seccomp_audit.rs` module (`SANDBOX_SYSCALL_DENIED_EVENT`, `SeccompAuditRecord`, `SeccompAuditContext`, `parse_seccomp_audit_record`, `sandbox_syscall_denied_payload`, `append_sandbox_syscall_denied_event`). `bwrap.rs` and `ProcessExecutor` translate signal-induced terminations via the POSIX `128 + signum` convention so SIGSYS surfaces as `exit_code=159` to callers. The dispatch path in `runtime::tools` detects this and emits one event per SIGSYS-terminated tool invocation with `record_correlated=false` and `syscall_nr=-1` until the AUDIT_SECCOMP netlink correlation lands as a follow-up sub-slice.
- *(AC-77)* New `tests/seccomp_allowlist_test.rs` with three integration tests: happy-path workload coverage (7 commands), namespace-creation primitive blocked by SIGSYS, and a control test that proves the SIGSYS exit is attributable to seccomp rather than another layer. Each test self-skips with an explicit reason if `bwrap` / Landlock / cgroup-v2 preconditions are missing.
- *(AC-77)* New empirical capture script `scripts/capture_seccomp_corpus.sh` plus reproducible fixture (`tests/fixtures/seccomp/observed_syscalls.txt`, `additions.txt`, `README.md`) so future tool-catalog expansions can re-derive the allowlist via `./scripts/devshell.sh ./scripts/capture_seccomp_corpus.sh > tests/fixtures/seccomp/observed_syscalls.txt`.

## [1.0.1](https://github.com/RCSnyder/open-pincery/compare/v1.0.0...v1.0.1) - 2026-04-21

### Fixed

- *(license)* dual-license under MIT OR Apache-2.0

### Other

- *(expand)* v6.1 pre-expand synthesis — external inputs + D1/D2
- *(expand)* v6 pre-expand synthesis — canonical north star

## [1.0.0](https://github.com/RCSnyder/open-pincery/releases/tag/v1.0.0) - 2026-04-20

### Added

- *(cli)* add 'pcy demo' for one-command end-to-end smoke test
- *(auth)* add /api/login endpoint for session token recovery
- *(build)* v5 operator onramp (AC-28..AC-33)
- *(build)* v5 slice 1+2 compose + .env.example rewrite with regression tests
- *(build)* v4 slice 5 deliver vanilla JS control plane (AC-26)
- *(build)* v4 slice 4 add pcy CLI binary and shared API client (AC-25)
- *(build)* v4 slice 3 add webhook secret rotation endpoint (AC-24)
- *(build)* v4 slice 2 enforce budget cap at wake acquire (AC-23)
- *(build)* v4 slice 1 non-root runtime image (AC-22)
- *(hooks)* auto-rustfmt on edits + fmt-check gate before git commit
- *(build)* v3 slice 6 — signed release artifacts with SBOM (AC-20)
- *(build)* v3 slice 4-5 — CI workflow (AC-16) + operator runbooks (AC-21)
- *(build)* v3 slice 3 — Prometheus metrics (AC-18)
- *(build)* v3 slice 1-2 — JSON logging (AC-17) + health/ready split (AC-19)
- *(build)* implement v2 features AC-11 through AC-15
- *(build)* implement dashboard UI — bootstrap, agent management, event log
- *(build)* complete BUILD phase — all 15 tests pass, all 10 ACs covered
- *(build)* add tests for AC-4, AC-5, AC-7 + fix llm_call/projection schema alignment
- *(build)* implement full application skeleton - Slice 1 complete

### Fixed

- *(ci)* checkout main branch in release-plz (not detached HEAD)
- *(ci)* allow CDLA-Permissive-2.0 license in cargo-deny
- *(build)* address REVIEW v4 findings and finalize v4 BUILD state
- *(review)* address v3 review findings (1 Critical + 5 Required + 2 Consider)
- *(review)* address all Critical and Required review findings
- *(build)* resolve 7 audit findings from struct/migration mismatches and API gaps
- *(build)* address REVIEW findings — 2 critical, 6 required fixes

### Other

- *(release)* cut 1.0.0 and wire up automated releases
- *(input)* add improvement-ideas brainstorm
- *(build)* fix Docker build for Rust 1.88 toolchain
- *(deploy)* v5 delivery — log + DELIVERY.md finalized
- *(analyze)* v5 readiness — READY verdict
- *(design)* v5 design addendum — operator onramp contract
- *(expand)* v5 scope — operator onramp
- *(deploy)* v4 delivered — README, DELIVERY, log updated
- *(reconcile)* sync v4 scaffolding with shipped code
- *(build)* record v4 BUILD gate pass evidence
- *(build)* narrow sqlx features and refresh lockfile
- *(build)* add v4 API stability contract (AC-27)
- *(build)* add static Dockerfile guard for AC-22
- *(iterate)* v4 readiness — READY
- *(iterate)* v4 design — CLI/UI/safety integration points
- *(iterate)* v4 scope — usable self-host (CLI + UI + safety hardening)
- *(hooks)* split by concern + block destructive commands
- *(deploy)* v3 delivery — README + DELIVERY.md + log
- rustfmt wrap assert! in json logging test
- *(review)* log RECONCILE + REVIEW pass 2 PASS
- *(reconcile)* align design + readiness with v3 code post-review
- *(iterate)* v3 scope, design, readiness — observability and release hygiene
- *(deploy)* complete v2 delivery
- *(reconcile)* fix v2 scaffolding drift
- *(analyze)* v2 readiness.md — READY verdict, AC-11 through AC-15
- *(iterate)* version scope.md and design.md for v2 — operational readiness
- update README with accurate quick start and project structure
- *(deploy)* README + DELIVERY.md, post-deploy gate PASS
- *(verify)* post-verify gate PASS — all 10 ACs verified, 17/17 tests pass
- *(reconcile)* fix structural drift across scaffolding documents
- *(build)* add tests for AC-6, AC-8, AC-9
- *(build)* add integration tests for AC-1, AC-2, AC-3, AC-10
- *(analyze)* produce readiness.md — READY verdict
- *(design)* define architecture for Open Pincery v1
- *(expand)* define scope for Open Pincery v1 agent runtime
- rewrite README for open-pincery and update LICENSE copyright
- *(input)* add arch pdf, tla+ spec, and additional arch .md files
- Initial commit
