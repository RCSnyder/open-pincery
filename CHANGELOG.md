# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.1.0](https://github.com/RCSnyder/open-pincery/compare/v1.0.1...v1.1.0) - 2026-05-11

### Added

- *(build)* AC-91 pcy backup / pcy restore (v9.1 V91-S6)
- *(build)* AC-93 pcy provider — per-workspace LLM provider rows
- *(build)* AC-92 docs/onboarding.md — one-page first-run gate
- *(build)* AC-90 pcy doctor — 8-check self-diagnosis with table/json output
- *(build)* AC-89 pcy init — bootstrap .env with strong random secrets
- *(build)* AC-94 honesty pass — README five-row security table + DELIVERY v9.0
- *(build)* AC-82 G7f+G7g — status-write CAS lint + spec_coverage Inv_TerminalSuccession
- *(build)* AC-82 G7c+G7d — tool-loop transitions + terminal CAS chain
- *(build)* AC-82 G7b — wake-loop entry chain emits lifecycle_transition events
- *(build)* AC-82 G7a — fine-grained lifecycle CAS helpers + migration
- *(build)* AC-81 binding commitments — spec_coverage table + commit-msg hook + lint
- *(verify)* AC-80 closed — capability nonce admission gate verified
- *(build)* AC-80 G5b+G5c — wake_loop mints, dispatch_tool consumes
- *(build)* AC-80 G5a — capability_nonce module + migration + unit tests
- *(build)* AC-79 G4e per-wake tool-call rate limit + adversarial integration tests + CHANGELOG
- *(build)* AC-79 G4d jsonschema validation + retry cap + FailureAuditPending
- *(build)* AC-79 G4b+G4c canary emission + echo scan + injection termination
- *(build)* AC-79 G4a wake_system_prompt v3 + per-wake nonce/canary plumbing
- *(build)* G3d audit-chain startup gate refuses to boot on broken chain
- *(build)* G3c — pcy audit verify CLI + POST /api/audit/chain/verify
- *(build)* G3b AC-78 verifier + workspace pass + audit_chain events
- *(build)* G3a AC-78 event hash chain migration + Rust verifier scaffold
- *(build)* G3a AC-78 event-log hash chain migration + Rust verifier
- *(build)* AC-77 / Slices G2e+G2f - corpus-subset guard + CHANGELOG
- *(build)* AC-77 / Slice G2c - sandbox_syscall_denied event on SIGSYS
- *(build)* AC-77 / Slice G2b - default-deny allowlist + clone arg-filter
- *(build)* AC-77 / Slice G2a - empirical seccomp syscall corpus
- *(build)* G1d network-category escape payloads (AC-76 12/12)
- *(build)* G1c.x.2 wire memory.max probe into startup gate
- *(build)* AC-76 / Slice G1c.x - empirical memory.max enforcement probe
- *(build)* AC-76 / Slice G1c - resource payloads (fork-bomb, memory-balloon, pid-exhaustion)
- *(build)* AC-76 / Slice G1b — privesc payloads (setuid, CAP_SYS_ADMIN, user-ns)
- *(build)* AC-76 G1a sandbox escape suite — FS category
- *(build)* implement ac-88 landlock audit integration
- *(build)* enforce landlock IPC scopes
- *(build)* wire AC-84 startup preflight + exit-4 contract
- *(build)* G0b.1 kernel ABI floor preflight module (AC-84)
- *(build)* G0a.3h flip SandboxProfile::default to landlock=true
- *(build)* G0a.3g wire pincery-init into bwrap for landlock
- *(build)* JSON error channel on --error-fd for pincery-init (G0a.3f)
- *(build)* FullyEnforced verification inside pincery-init (G0a.3e)
- *(build)* install landlock inside pincery-init (G0a.3d)
- *(sandbox)* Slice G0a.3c -- seccomp filter install inside pincery-init (AC-83)
- *(sandbox)* Slice G0a.3b -- drop uid/gid inside pincery-init (AC-83)
- *(sandbox)* Slice G0a.3a -- prctl(NO_NEW_PRIVS) inside pincery-init (AC-83)
- *(sandbox)* Slice G0a.2 -- pincery-init binary skeleton (AC-83)
- *(sandbox)* Slice G0a.1 — SandboxInitPolicy IPC module (AC-83)
- *(build)* add landlock LSM filter (AC-53 layer 4 of 6, slice A2b.4c)
- *(sandbox)* Slice A2b.4b seccomp-bpf denylist (layer 3 of 6)
- *(sandbox)* Slice A2b.4a cgroup v2 resource caps (layer 2 of 6)
- *(scripts)* allow relocating cargo and devshell caches off system drive
- *(runtime)* AC-53 RealSandbox via bwrap + build_executor factory (Slice A2b.3)
- *(build)* AC-53 prep -- Linux sandbox crate gate (Slice A2b.1)
- *(build)* AC-73 sandbox mode config flag (Slice A2a)
- *(build)* AC-54 SECURITY.md threat model (Slice A1)
- *(build)* AC-75 cross-platform devshell (Slice A0)
- *(cli)* remove pcy bootstrap subcommand; login is sole auth verb (AC-45)
- *(build)* AC-52b v8.0 -- cli_naming_test + about docstrings
- *(build)* AC-51 v8.0 -- pcy completion via clap_complete
- *(build)* AC-47 v8.0 -- credential list honours --output
- *(build)* AC-45/AC-48 v8.0 -- idempotent pcy login + pcy whoami
- *(build)* AC-47 slice 2e-a -- root Cli --output/--no-color flags
- *(build)* AC-46/AC-48 slice 2d-i -- context noun
- *(build)* AC-48 slice 2c -- named contexts + v4 to v8 migration
- *(build)* AC-46 slice 2b -- name-or-UUID resolver
- *(build)* AC-47 slice 2a -- CLI output renderer foundation
- *(build)* AC-44 slice 1b -- full handler OpenAPI coverage
- *(build)* AC-44 slice 1a — OpenAPI 3.1 spec endpoint
- *(build)* AC-43 v7 slice 6 -- PLACEHOLDER credential resolution
- *(build)* AC-42 v7 slice 5 -- hardened wake_system_prompt v2
- *(build)* AC-41 v7 slice 4 -- list_credentials tool
- *(build)* AC-40 v7 slice 3 -- pcy credential CLI
- *(build)* AC-39 v7 slice 2 -- credentials REST API
- *(build)* AC-38 v7 slice 1 -- AES-256-GCM credential vault
- *(build)* AC-36 v6 slice 4 -- ToolExecutor trait + ProcessExecutor sandbox
- *(build)* AC-35 v6 slice 3 -- capability gate wired in front of dispatch_tool
- *(build)* AC-34 v6 slice 2 -- typed AgentStatus + TLA-aligned DB values
- *(build)* AC-37 v6 slice 1 -- zero-advisory cargo deny floor

### Fixed

- *(ci)* allowlist VAULT_KEY_BASE64 as INTERNAL_ONLY env var (AC-29 + AC-91)
- *(ci)* allowlist 'provider remove' for --yes flag (AC-93)
- *(ci)* clippy assertions_on_constants + AC-40 rpassword allowlist for pcy init
- *(review)* align onboarding doc + doctor docstring to 7 checks
- *(review)* amend AC-90 to 7 checks, add AC-93 resolver test
- *(build)* AC-91 review-fix — wire --include-vault-key into restore + 0o600 modes
- *(build)* AC-82 review-fixes (1 Critical + 3 Required + 1 Consider)
- *(build)* AC-82 G7a clippy doc_overindented_list_items
- *(build)* AC-81 clippy for_kv_map — iterate keys() in spec_coverage_lint
- *(build)* AC-80 REVIEW-fix-1 — concurrent test + AC-78 chain walk + schema shape + doc-comment
- *(build)* AC-80 G5d type alias for classify_rejection row tuple
- *(verify)* AC-79 verify-fix-1 — allowlist new env keys in AC-29 orphan check
- *(build)* AC-79 review-fix-1 — structured event payloads, OsRng, v3 active-template proof, 3 scope-verbatim adversarial tests
- *(build)* G3-review address REVIEW Required findings on AC-78
- *(build)* G3d add OPEN_PINCERY_AUDIT_CHAIN_FLOOR to .env.example
- *(test)* G3c — bootstrap returns 201 CREATED in audit_api_test
- *(build)* G3b verify-fix-2 — enforce monotonic created_at in chain trigger
- *(build)* G3b verify-fix-1 — advisory lock + payload arg-order
- *(verify)* accept Timeout as fork-bomb denial outcome (AC-77)
- *(verify)* relax AC-77 audit_mode test to negative-only assertion
- *(verify)* AC-77 escape-test scaffolding accepts SIGSYS denials
- *(verify)* add AC-77 Landlock syscalls (444/445/446) per kernel dmesg
- *(verify)* add AC-77 capget+capset (syscall 126) per kernel dmesg
- *(verify)* add AC-77 getresgid (syscall 120) per kernel dmesg evidence
- *(verify)* correct AC-77 syscall identification - 118=getresuid not setresgid
- *(verify)* allow setresgid in AC-77 seccomp filter (kernel-evidence)
- *(verify)* widen AC-77 allowlist for Rust runtime + glibc-2.39 residuals
- *(build)* AC-77 review re-fix - cfg-gate sigsys_event_test to linux + log G2c.2 review-deferreds
- *(build)* AC-77 review fixes - R1 SIGSYS event test, R2 audit-mode test, R3 fanotify negative control
- *(build)* AC-76 / Slice G1c cleanup - clippy + memory-balloon unconditional skip (BLOCKED)
- *(build)* AC-76 / Slice G1c round 3 - memory controller delegation probe
- *(build)* AC-76 / Slice G1c round 2 - dd buffer + SURVIVORS pattern
- *(build)* AC-76 / Slice G1c round 1 - reshape resource payloads after CI feedback
- *(build)* G1b round 1 - privesc test correctness (CI cb8521b failures)
- *(build)* G1b round 1 — privesc test correctness (CI cb8521b failures)
- *(sandbox)* per-path Landlock access mask; correct bwrap /etc guard test
- *(sandbox)* narrow /etc bind+landlock to public allowlist (closes G1a /etc/shadow escape)
- *(verify)* repair ac-88 linux ci failures
- *(sandbox)* block nested user namespaces
- *(sandbox)* keep AC-86 test helpers test-only
- *(sandbox)* drop bwrap uid and capabilities
- *(sandbox)* simplify landlock status validation
- *(sandbox)* avoid constructing landlock restriction status
- *(sandbox)* require full Landlock enforcement in production
- *(test)* locate server binary in AC-84 integration tests
- *(sandbox)* enforce AC-84 userns quota evidence
- *(test)* accept AC-84 event on stdout or stderr in CI
- *(build)* register OPEN_PINCERY_SANDBOX_FLOOR in env contract (AC-29)
- *(build)* set PINCERY_INIT_BIN_PATH in sandbox_real_smoke preflight
- *(build)* G0a.3g add /dev to default landlock rwx paths
- *(build)* G0a.3g populate policy.user_argv with sh -c <cmd>
- *(build)* make write_error_channel pub so main can call it (G0a.3f)
- *(build)* allowlist OPEN_PINCERY_INIT_FORCE_PARTIAL for AC-29
- *(build)* expand sample_policy landlock paths for G0a.3d enforcement
- *(build)* clippy doc_lazy_continuation in G0a.3d test docstring
- *(sandbox)* satisfy clippy::manual_is_multiple_of in apply_seccomp
- *(sandbox)* derive Debug on ParsedArgs (unwrap_err needs T: Debug)
- *(sandbox)* allow dead_code on ParsedArgs.user_argv (G0a.2 clippy fix)
- *(sandbox)* swap bincode -> serde_json in SandboxInitPolicy (deny failure fix)
- *(sandbox)* interim — disable landlock by default per AC-53 amendment
- *(sandbox)* add / to landlock rx allowlist so bwrap setup succeeds
- *(tla)* repair side-spec parse, include in CI, keep path filter
- *(build)* landlock must grant rwx on /proc for bwrap uid-map writes
- *(build)* remove unused CommandExt import (tokio's pre_exec is inherent)
- *(sandbox)* clippy doc-list-overindent + manual-c-str-literal in seccomp.rs
- *(sandbox)* clippy manual-range-contains in A2b.4a pids test
- *(devshell)* MSYS path + TTY auto-detect for Windows git-bash; log A2b.3 evidence closure
- *(ci)* close AC-53 evidence gate — patch rustls-webpki, allow unpriv userns, rotate deny.toml ignore
- *(build)* allowlist v8.0 env-var additions in env_example_test
- *(build)* clippy -- allow too_many_arguments on dispatch_tool and prefer flatten in leak scan
- *(build)* close v6 review R1/R2 — widen sudo check + add AppState.executor
- *(build)* AC-37 -- allow RUSTSEC-2023-0071 as a single documented exception

### Other

- *(deploy)* v9.1 onboarding gate delivery handoff
- *(verify)* align Doctor clap docstring to 7 checks
- *(reconcile)* v9.1 onboarding gate 7-axis drift sweep — REPAIRED
- *(analyze)* produce v9.1 readiness.md (AC-89..AC-94)
- *(iterate)* version scope to v9.1 — Onboarding Gate
- *(post_v9_audits)* add nwave convo + v9 audit
- *(readme)* v9.0 ship status update
- *(deploy)* DELIVERY.md AC-82 close + v9.0 ship-gate clear
- *(reconcile)* AC-82 7-axis drift sweep — REPAIRED
- *(build)* log G7c..G7g AC-82 BUILD-complete checkpoint
- *(build)* log AC-82 G7b checkpoint
- *(build)* log AC-82 G7a checkpoint
- *(analyze)* AC-82 readiness — Fire Reserved Lifecycle States READY
- *(deploy)* AC-81 closed — Binding Commitments delivered
- *(reconcile)* AC-81 7-axis drift sweep — REPAIRED
- *(analyze)* AC-81 readiness — Binding Commitments (spec_coverage + commit-msg hook) READY
- *(reconcile)* AC-80 7-axis drift sweep — REPAIRED
- *(review)* AC-80 REVIEW round 2 PASS
- *(build)* AC-80 G5e — CHANGELOG and DELIVERY entries for capability nonce gate
- *(build)* AC-80 G5d — capability nonce adversarial integration tests
- *(analyze)* AC-80 capability nonce/freshness — READY
- *(input)* tool-landscape audit 2026-05 — OpenShell, Sandcastle, pi-mono, Founder OS
- *(deploy)* AC-79 closed — Prompt-Injection Defense Floor delivered
- *(reconcile)* AC-79 7-axis drift sweep — REPAIRED
- *(review)* AC-79 REVIEW PASS @ 91ecfb8 — fix-cycle 1 closed all Critical/Required findings
- *(analyze)* AC-79 Prompt-Injection Defense Floor admission
- *(deliver)* document AC-78 event-log hash chain in DELIVERY.md
- *(reconcile)* AC-78 7-axis drift sweep — REPAIRED
- *(log)* G3-review entry; memory: AC-78 review-fix state @b412025
- *(build)* G3e log entry — AC-78 BUILD complete (G3a..G3e all green)
- *(build)* G3e AC-78 audit-chain recovery runbook + CHANGELOG
- *(build)* G3d log entry — audit-chain startup gate CI 25241912717 green
- *(build)* G3c CLOSED — log entry for pcy audit verify CLI + HTTP
- *(build)* log AC-78 G3b CLOSED at b961955
- *(build)* log AC-78 G3a PASS at bf9c6b5
- *(analyze)* readiness for AC-78 event-log hash chain
- *(design)* add v9 G3 AC-78 event-log hash chain design slice
- *(reconcile)* log AC-77 verify-fix-2 reconcile pass (REPAIRED)
- *(reconcile)* align AC-77 audit-mode coverage row with shipped test
- *(reconcile)* align design.md AC-77 allowlist counts with shipped state
- *(verify)* log AC-77 verify-fix-2 PASS entry
- *(sandbox-smoke)* capture kernel seccomp/audit log for AC-77 diagnosis
- *(verify)* log AC-77 verify-fix attempt 1 PARTIAL + BLOCKED post-mortem
- *(reconcile)* align design.md and audit doc counts with verify-fix
- *(reconcile)* align design.md and audit doc with AC-77 shipped state
- *(build)* log AC-77 / G2d+G2e+G2f PASS at 81571db / 5982ab3
- *(build)* AC-77 / Slice G2d - seccomp allowlist integration tests
- *(build)* log AC-77 / G2c PASS at a96499e
- *(build)* log AC-77 / G2b PASS at a89d4a5
- *(analyze)* log post-analyze PASS for AC-77
- *(analyze)* AC-77 readiness — seccomp default-deny allowlist
- *(verify)* G1d CI-green at 25197562247 - AC-76 closes 12/12
- *(ci)* install iputils-ping in sandbox-smoke runner
- *(scope)* document strategic security gaps + close G1c memory-balloon entry
- *(verify)* G1c.x.2 green on CI 25196202744
- *(verify)* G1c.x green on CI 25193943507
- *(verify)* G1b green on CI 25141721367 (8935fd7)
- *(verify)* G1a / AC-76 FS category green on CI dd10a8b
- *(sandbox)* include sandbox_escape_test in privileged smoke job
- *(analyze)* open AC-76 / Slice G1a sandbox escape suite readiness
- *(windows)* default local target dirs to repo-local paths
- *(verify)* record ac-88 ci evidence
- *(verify)* close AC-87 with CI evidence
- *(build)* pin landlock scope fallback event
- *(verify)* record AC-86 sandbox proof
- *(verify)* close AC-85 with CI evidence
- *(verify)* close AC-84 with CI evidence
- *(build)* log Slice G0a.1 SandboxInitPolicy
- *(analyze)* readiness addendum for Slice G0a (AC-83 pincery-init)
- *(expand)* v9 sandbox architecture rework — AC-83..AC-88, Phase G0
- *(sandbox)* --no-fail-fast so every real-bwrap binary runs
- *(sandbox)* accept dash 'Cannot fork' message as pids.max evidence
- *(sandbox)* run AC-53 real-bwrap tests inside --privileged container
- *(sandbox)* run smoke binary under sudo, skip in unprivileged test job
- *(sandbox)* grant bwrap cap_sys_admin+cap_sys_chroot file caps
- *(sandbox)* make / private before bwrap to unblock MS_SLAVE on hosted runners
- *(sandbox)* surface bwrap stderr in smoke test panics
- *(scope)* add AC-76..AC-82 as v9 release blockers from TLA+ + security audit
- *(ci)* remove extra lines
- *(tla)* add SANY parse + TLC simulation CI for canonical spec
- *(spec)* v3.3 TLC-driven correctness pass (B5-B10)
- *(log)* record Slice A2b.4b CI evidence (10/10 seccomp tests + 72 lib tests green on run 24801274092)
- *(log)* record Slice A2b.4a CI evidence (4/4 cgroup tests green on run 24799988428)
- *(log)* record Slice A2b.3 second-channel evidence (local devshell bwrap smoke 5/5)
- install bwrap userland + dedicated sandbox-smoke job (AC-53 evidence gate)
- *(build)* log Slice A2b.3 checkpoint
- *(build)* log Slice A2b.1 + A2b.2 checkpoint
- *(runtime)* split sandbox.rs into sandbox/ module (Slice A2b.2)
- *(clippy)* fix Rust 1.94 lints (derivable_impls, doc_lazy_continuation)
- *(build)* log Slice A2a PASS
- *(build)* verify AC-75 Linux parity + relax Docker floor
- *(build)* log Slice A1 PASS
- *(build)* log Slice A0 PASS
- *(audit)* fix post-audit plan inconsistencies
- *(audit)* v9 audit addendum — 3 new ACs (AC-73/74/75) + 15 in-slice hardening items
- *(design,analyze)* v9 trust-gate architecture + readiness map
- *(expand)* v9 scope revision — clarifications resolved; AC-53/65 upgraded; AC-71/72 added
- *(expand)* v9 scope -- solo-founder trust gate (AC-53..AC-70)
- *(build)* log v8.0 landing + DELIVERY.md v8.0 section
- *(build)* log AC-47 slice 2e-a -- root Cli output/no-color flags
- *(build)* log slice 2d-i -- context noun
- *(build)* log slice 2c -- named contexts + v4 to v8 migration
- *(build)* log BUILD v8 Slice 2 (partial - sub-slices 2a+2b)
- *(build)* log BUILD v8 Slice 1 (AC-44 OpenAPI coverage)
- *(analyze)* v8 readiness — READY
- *(scope)* normalize v8 stack table whitespace
- *(design)* v8 — Unified API Surface architecture
- *(v8-prep)* v8 exploration artifacts — CLI distribution + release matrix
- *(expand)* v8 scope — Unified API Surface (schema-driven CLI, MCP, distribution)
- *(v7)* RECONCILE + DEPLOY -- log and DELIVERY for v7
- *(design)* v7 design addendum + readiness READY
- *(expand)* scope v7 — credential vault & reasoner-secret refusal
- *(deploy)* v6 DELIVERY.md refresh + post-DEPLOY gate PASS
- *(verify)* v6 post-VERIFY gate PASS
- *(reconcile)* v6 seven-axis audit — align design.md + readiness.md with shipped code
- *(build)* v6 post-BUILD gate PASS + cargo audit observation
- *(analyze)* v6 readiness READY -- 4-slice build order starting with AC-37
- *(design)* v6 addendum — capability foundations & security baseline
- *(expand)* scope v6 — capability foundations & security baseline

### Security

- _(AC-81)_ **Binding commitments — spec coverage + commit-msg hook.** New `scaffolding/spec_coverage.md` is the single source of truth that maps every v9 acceptance criterion (AC-53..AC-88) to the canonical TLA+ action(s) in `docs/input/OpenPinceryCanonical.tla` `Next ==` it implements (or `—` for pure docs/UI/CLI surface) plus any invariant the AC makes real. New `tests/spec_coverage_lint.rs` mechanically validates the table: every cited action exists in the canonical `Next` disjunction, every AC-53..AC-88 row is present, no duplicates, no empty action cells. New `.github/hooks/commit-msg-spec-ref` is a path-conditional commit-msg hook that rejects any commit whose staged diff touches `src/runtime/**` or `src/api/**` unless the message body contains at least one `canonical_action=<Name>` trailer where `<Name>` appears in `scaffolding/spec_coverage.md`. Commits that touch neither path (scope edits, docs, CI, tooling) are accepted with no trailer requirement. `scripts/devshell.sh` installs the hook into `.git/hooks/commit-msg` idempotently — only when no hook is present, or when the present hook is the unmodified `commit-msg.sample`. User-customized hooks are never overwritten. `tests/spec_hook_test.rs` drives the hook end-to-end with synthetic `(message, staged-diff)` fixtures (5 cases: rejects runtime change without trailer, accepts runtime change with valid trailer, accepts docs-only commit, rejects unknown canonical action, devshell installer is idempotent and respects user customization). Closes the spec-drift loophole that let v9 ship runtime code without explicit traceability back to the canonical model.

- _(AC-80)_ **Capability nonce / freshness gate.** Closes the AC-35 replay window: every `IssueToolCall` must now present a one-shot nonce minted at the matching `AuthorizeExecution` boundary. New `capability_nonces` table (`migrations/20260501000003_create_capability_nonces.sql`) stores `(id, wake_id, tool_name, capability_shape, nonce, expires_at, consumed_at, workspace_id, created_at)` with `UNIQUE (workspace_id, nonce)` plus an `expires_at` index. New `src/runtime/capability_nonce.rs` module exposes `mint`, `consume`, `RejectionReason::{Replay, CrossWake, Expired, ShapeMismatch, Unknown}`, the public `capability_shape` SHA-256-of-canonical-JSON helper (sorted keys, no whitespace), and the constants `CAPABILITY_NONCE_LEN = 16` and `CAPABILITY_NONCE_TTL_SECS = 60`. Random bytes come from `OsRng` via `rand::TryRngCore::try_fill_bytes`. The wake loop (`src/runtime/wake_loop.rs::run_wake_loop`) mints a fresh ticket per claimed tool call AFTER AC-79 schema validation and the per-wake rate-limit gate, BEFORE `tools::dispatch_tool`. `dispatch_tool` gains a 9th parameter `&CapabilityNonceTicket`; the atomic `UPDATE ... SET consumed_at = now() WHERE consumed_at IS NULL AND expires_at > now() RETURNING id` runs AFTER the AC-35 capability gate (so an AC-35 denial does NOT consume the ticket) and BEFORE the per-tool match arms. On rejection the runtime emits exactly one `capability_nonce_rejected` event (TRUSTED, `source = "runtime"`, content payload `{wake_id, tool_name, reason}` where `reason` is the lowercase RejectionReason) and short-circuits dispatch with `ToolResult::Error("capability nonce rejected")` — no per-tool side effect ever runs. The new event type chains through the AC-78 audit hash trigger transparently (`event::append_event` signature unchanged). 7 module unit tests + 9 adversarial integration tests in `tests/capability_nonce_test.rs` cover replay, cross-wake, cross-workspace, expiry (via UPDATE backdate), shape mismatch, unknown nonce, the runtime event payload shape on replay, and the AC-35-denied-does-not-consume invariant. Periodic background sweep of expired/consumed rows is deferred to v9.1; the unique index makes accumulated rows unreachable from production code paths in the meantime.

- _(AC-79)_ **Prompt-injection defense floor.** Wake-loop prompts now wrap every untrusted-class event payload (`message_received`, `tool_result`, `memory_read`, `wake_summary_loaded`) in a per-wake `<<untrusted:NONCE>>...<<end:NONCE>>` envelope (32 hex chars from the OS CSPRNG, never persisted) and the `wake_system_prompt` template v3 (migration `20260501000002_add_prompt_injection_floor.sql`) instructs the model to treat that envelope as data, not instructions. A per-wake canary `<<canary:HEX>>` (32 hex chars, also unpersisted) is appended to the system prompt after the truncation guard so a tight prompt budget cannot drop it; one `prompt_injection_canary_emitted` event records that a canary was minted (payload contains the `wake_id` only — never the value or nonce). Every LLM response is scanned for the canary across `choices[].message.content` and every tool call's `function.name`, `function.arguments`, and `id`; on echo the wake emits `prompt_injection_suspected` with a structural `where_found` audit tag (e.g. `choice[0].tool_calls[0].function.arguments`) and terminates BEFORE any `assistant_message` or `tool_call` event is appended — the attacker's instruction never reaches the sandbox.
- _(AC-79)_ **JSON-Schema tool-call validation + retry cap.** Every claimed tool call's `function.arguments` is validated against the tool's `parameters` JSON Schema BEFORE `runtime::tools::dispatch_tool` is invoked. Validators are compiled once via `OnceLock<HashMap<String, jsonschema::Validator>>` from `tool_definitions()` (single source of truth); the existing `serde_json::from_str::<ShellArgs>` binding stays as defense-in-depth strictly downstream. On any failure the wake emits `model_response_schema_invalid` (content = structural reason only — tool name + first schema-error path; never the offending argument bytes) and re-runs the LLM without dispatching ANY tool from that response. Schema-invalid retries do NOT count against `iteration_cap`. After `Config::schema_invalid_retry_cap` consecutive invalids (default 3, env `OPEN_PINCERY_SCHEMA_INVALID_RETRY_CAP`, 0 rejected) the wake terminates with `termination_reason = "FailureAuditPending"`. A single recovered response resets the counter so transient drift unsticks.
- _(AC-79)_ **Per-wake tool-call rate limit.** New `Config::tool_call_rate_limit_per_wake` (default 32, env `OPEN_PINCERY_TOOL_CALL_RATE_LIMIT_PER_WAKE`, 0 rejected) caps the number of tool calls dispatched per wake, independent of `iteration_cap` (different quantities; whichever fires first terminates first). On exhaustion the wake emits exactly one `tool_call_rate_limit_exceeded` event with payload `{limit, attempted}` and terminates with `FailureAuditPending`. Schema-invalid retries do NOT increment the counter — the call was never dispatched. The four AC-79 event types (`prompt_injection_canary_emitted`, `prompt_injection_suspected`, `model_response_schema_invalid`, `tool_call_rate_limit_exceeded`) all register with `source = "runtime"`, all chain through the AC-78 audit hash trigger transparently, and none carries the canary value, the delimiter nonce, or any untrusted content body. `event::append_event` is unchanged (T-AC78-10 invariant preserved).

- _(AC-78)_ **Event-log hash chain.** Every row in `events` now carries `prev_hash` and `entry_hash` columns, populated atomically by a per-agent `BEFORE INSERT` trigger (migration `20260501000001_add_event_hash_chain.sql`). The trigger uses `pg_advisory_xact_lock(44224, hashtext(agent_id::text))` plus `SELECT FOR UPDATE` against the prior tail to serialize concurrent inserts at genesis and a strict-monotonic `created_at` bump to disambiguate microsecond ties. The canonical pre-image is length-prefixed (4-byte BE length + UTF-8 bytes per field, then 8-byte BE micros for `created_at`) so reordering or partial fields cannot collide. A `DO`-block backfill walks every existing agent's history in `(created_at, id)` order before `prev_hash`/`entry_hash` are set `NOT NULL`.
- _(AC-78)_ **Verifier and CLI / HTTP surface.** New `src/background/audit_chain.rs` exposes `verify_audit_chain(pool, agent_id) -> ChainStatus { Verified | Broken { first_divergent_event_id, expected_hash, actual_hash, events_walked } }`, `verify_workspace`, and `verify_and_emit` which appends one `audit_chain_verified` or `audit_chain_broken` event per agent per call. The verifier never mutates application data — it only emits its own audit-trail events (`source = "runtime"`, distinguishable by `event_type` of `audit_chain_verified` or `audit_chain_broken`). New `pcy audit verify [--agent <uuid>] [--workspace <id>]` CLI exits with code `2` when any chain is broken, raw JSON to stdout, one-line-per-agent summary to stderr. New workspace-admin-gated HTTP endpoints `POST /api/audit/chain/verify` and `POST /api/audit/chain/verify/agents/{id}` return the same shape (`{ agents: [...], all_verified: bool }`); the per-agent route 404s on cross-workspace lookups and 403s on non-admin callers.
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
