# Readiness: Open Pincery — current slice pointer

> Current admission gate: **Phase G Slice G2 / AC-77 (Seccomp
> Default-Deny Allowlist)**. The AC-77 addendum is appended below the
> AC-76 / G1c addendum that preceded it. AC-76 closed at 12/12 on
> 2026-04-30 (CI run `25197562247`) — all four payload categories
> (FS, privesc, resource, network) runtime-verified. G1b (privesc)
> closed CI-green at `8935fd7` on 2026-04-29; its addendum is retained
> verbatim further down. G1a (FS), G1c (resource), G1d (network), and
> G0f / AC-88 addenda are all retained as historical record.

---

# Readiness: Open Pincery — v9 Phase G1c (AC-76 Sandbox Escape Suite — Resource Category)

> This addendum covers Slice G1c only. G1a (FS) and G1b (privesc)
> closed at commits `dd10a8b` / `4913c6e` and `8935fd7` respectively.
> Slice G1d adds the network category; Slice G1e adds the synthesized
> `sandbox_blocked` event emitter and the AC-53 closure gate.

## Verdict

READY for Slice G1c / AC-76 (resource category). Builds on the G1a +
G1b runtime harness (`tests/sandbox_escape_test.rs::{preconditions_met,
enforce_sandbox, escape_profile, assert_payload_blocked}`) with one
contained extension: `escape_profile()` now installs the production
cgroup v2 limits (`memory.max=512 MiB`, `pids.max=64`) so the
resource payloads have something to be capped _by_. Without this
extension the resource category cannot fail-closed because the prior
slices ran with `cgroup: None`. `preconditions_met()` is widened to
also require `cgroup_v2_writable()`, mirroring `sandbox_cgroup_test.rs`
— the privileged CI runner already satisfies this gate (the existing
`cgroup_pids_max_limits_fork_count` test passes there). Network
category and `sandbox_blocked` event remain explicitly deferred to
G1d/G1e.

## Truths

- **T-AC76-G1c-1** Slice G1c extends `tests/sandbox_escape_test.rs`
  with three additional `#[tokio::test]` functions covering the
  resource-exhaustion category named in `scaffolding/scope.md` AC-76.
  The harness primitives (`preconditions_met`, `enforce_sandbox`,
  `escape_profile`, `assert_payload_blocked`) are reused; only
  `escape_profile()` and `preconditions_met()` are extended (see
  T-AC76-G1c-2 and T-AC76-G1c-3). No `src/` changes.
- **T-AC76-G1c-2** `escape_profile()` is upgraded to the
  production-equivalent cgroup posture: `memory_max_bytes =
Some(512 * 1024 * 1024)`, `pids_max = Some(64)`,
  `cpu_max_micros = None` (CPU cap is not adversarially probed in
  G1c). This matches the AC-53 production limits documented in
  `scaffolding/scope.md` and the existing
  `sandbox_cgroup_test.rs::cgroup_permits_command_under_caps`
  baseline. The G1a (FS, 4) and G1b (privesc, 3) tests inherit the
  upgraded profile; none of them allocate anywhere near 512 MiB or
  spawn anywhere near 64 processes, so they remain green.
- **T-AC76-G1c-3** `preconditions_met()` gains a fourth gate after
  the existing bwrap / landlock-supported / Landlock-ABI-floor gates:
  `cgroup_v2_writable()` from `runtime::sandbox::cgroup`. Without
  this, applying `escape_profile()` would cause `Enforce`-mode
  fail-closed (`ExecResult::Err`) on hosts where the test process
  cannot `mkdir` under `/sys/fs/cgroup`, which would mask real
  blocks behind a harness error. Self-skip with an explicit reason
  to keep CI logs clear.
- **T-AC76-G1c-4** The three resource payloads are:
  - `resource_fork_bomb_blocked` — classic recursive shell
    fork-bomb (`bomb(){ bomb|bomb& };bomb`) wrapped in `timeout 4s`
    so the test does not hang. With `pids.max=64` the kernel
    refuses additional `fork(2)` calls with EAGAIN; the shell
    surfaces "Resource temporarily unavailable" / "Cannot fork" in
    stderr. Even if `timeout` itself signals the bomb, the suite
    must observe a denial signature OR a non-zero exit.
  - `resource_memory_balloon_blocked` — `head -c 600M /dev/zero |
tr '\\0' 'a' >/tmp/big` allocates ≈600 MiB into the bwrap
    tmpfs, exceeding `memory.max=512 MiB`. cgroup v2 OOM-kills the
    pipeline; the parent shell prints "Killed" to stderr and the
    overall exit is non-zero (typically 137 = 128 + SIGKILL).
  - `resource_pid_exhaustion_blocked` — flat backgrounded loop
    (`for i in $(seq 1 200); do sleep 60 & done; wait`) wrapped in
    `timeout 4s`. Distinct from fork-bomb in shape (linear, not
    recursive) but exercises the same `pids.max` cap; surfaces the
    same EAGAIN signature. We accept either kernel-level signature
    OR a non-zero exit from `timeout`'s SIGTERM as the block.
- **T-AC76-G1c-5** Each payload runs through the production
  `RealSandbox::run()` `Enforce` path (`SandboxMode::Enforce`,
  `allow_unsafe = false`). The same two-check assertion shape as
  G1a/G1b applies: non-zero exit AND a denial-signature match. The
  memory-balloon test additionally accepts `"killed"` and the
  string `"137"` (some shells print the exit code) as denial
  signatures, since the OOM-kill path produces a SIGKILL rather
  than a userspace error message.
- **T-AC76-G1c-6** G1c does not weaken the AC-83..AC-88 enforcement
  floor, the AC-77 seccomp denylist, or the AC-86 uid-drop. The new
  cgroup limits are additive on top of every prior layer.
- **T-AC76-G1c-7** G1c is cross-platform-buildable and Linux-runnable
  (`#![cfg(target_os = "linux")]` is unchanged). The privileged CI
  `sandbox real-bwrap smoke` job is the runtime proof.

## Key Links (AC -> Design -> Test -> Proof)

- **L-AC76-G1c-1** `AC-76` -> `escape_profile()` upgraded with
  `CgroupLimits { memory_max_bytes: Some(512 MiB), pids_max:
Some(64), .. }` -> tests `resource_fork_bomb_blocked`,
  `resource_memory_balloon_blocked`, `resource_pid_exhaustion_blocked`
  -> CI privileged sandbox-smoke job runs the suite under cgroup v2.
- **L-AC76-G1c-2** `AC-76` -> `preconditions_met()` adds
  `cgroup_v2_writable()` gate -> all G1a/G1b/G1c tests self-skip on
  hosts without writable cgroup v2 -> privileged CI runner satisfies
  the gate (proven by existing `sandbox_cgroup_test.rs` runs).
- **L-AC76-G1c-3** `AC-76` -> `assert_payload_blocked` extended
  signature lists per-payload (memory-balloon adds "killed", "137",
  "out of memory"; fork-bomb / pid-exhaustion add "resource
  temporarily unavailable", "cannot fork", "fork:") -> CI logs
  surface the actual kernel diagnostic for each payload.

## Acceptance Criteria Coverage (G1c slice)

| AC    | Truth(s)        | Planned test                                                                          | Planned runtime proof                           |
| ----- | --------------- | ------------------------------------------------------------------------------------- | ----------------------------------------------- |
| AC-76 | T-AC76-G1c-1..5 | `resource_fork_bomb_blocked` (pids.max EAGAIN OR signaled-by-timeout)                 | CI sandbox-smoke job: non-zero exit + signature |
| AC-76 | T-AC76-G1c-1..5 | `resource_memory_balloon_blocked` (cgroup OOM kill SIGKILL)                           | CI sandbox-smoke job: non-zero exit + "killed"  |
| AC-76 | T-AC76-G1c-1..5 | `resource_pid_exhaustion_blocked` (pids.max EAGAIN OR signaled-by-timeout)            | CI sandbox-smoke job: non-zero exit + signature |
| AC-76 | T-AC76-G1c-2    | (existing) G1a/G1b suite re-runs under upgraded `escape_profile()` with cgroup limits | CI sandbox-smoke job: 7 prior tests stay green  |
| AC-76 | T-AC76-G1c-3    | `preconditions_met()` self-skips on cgroup-unwritable hosts                           | CI logs print explicit skip reason              |

## Scope Reduction Risks

- **Risk: pids.max false-pass via timeout.** If the CI runner's
  `pids.max` is not enforced (e.g. cgroup v2 not mounted, delegation
  broken), the fork-bomb / pid-exhaustion tests could green via
  `timeout 4s` killing the shell with SIGTERM (non-zero exit) without
  any kernel-level denial. **Mitigation**: signature lists require an
  EAGAIN-shaped diagnostic ("resource temporarily unavailable",
  "cannot fork", "fork:") — `timeout`'s SIGTERM does not produce
  these. If neither a signature nor an exit code matches, the test
  fails. The harness exit-code-preservation fix from G1a (no `;
echo exit=$?`) ensures the shell's exit reflects the real outcome.
- **Risk: memory-balloon false-pass via missing /dev/zero.** Bwrap
  bind-mounts a minimal `/dev` tmpfs containing only the safe device
  subset; `/dev/zero` is in that subset (per AC-86). If a future
  hardening removes `/dev/zero`, the test would ENOENT-skip without
  proving the memory cap. **Mitigation**: signature list includes
  "no such file or directory" so the failure mode is still surfaced
  as a block (defence-in-depth: missing primitive = stronger isolation
  than the target). A follow-up FYI is acceptable but not blocking.
- **Risk: G1a/G1b regression from the cgroup upgrade.** Adding cgroup
  limits to `escape_profile()` could surface latent issues in the
  prior suite (e.g. memory bloat in `dd if=/dev/sda` retries).
  **Mitigation**: 512 MiB + 64 PIDs is far more headroom than any
  G1a/G1b payload needs (each is a single short-lived shell). If a
  prior test regresses, it is a real cgroup-init bug, not a scope
  reduction.

## Clarifications Needed

None for G1c. AC-76 in `scaffolding/scope.md` names the three
resource payloads verbatim ("fork-bomb, memory-balloon,
pid-exhaustion") and the production cgroup limits.

## Build Order

- **G1c.1** Extend `escape_profile()` with `Some(CgroupLimits { memory_max_bytes: Some(512 MiB), pids_max: Some(64), cpu_max_micros: None })`.
- **G1c.2** Extend `preconditions_met()` to also require `cgroup_v2_writable()`.
- **G1c.3** Add `resource_fork_bomb_blocked`, `resource_memory_balloon_blocked`, `resource_pid_exhaustion_blocked` tests with documented denial-signature lists.
- **G1c.4** Local verify (fmt + clippy + check); commit + push; watch CI privileged sandbox-smoke job for runtime proof.

## Complexity Exceptions

None. File size after G1c is expected ~510-540 lines, just over the
~450-line G1e split trigger noted in earlier addenda. Splitting is
deferred to G1e per the prior plan; G1c does not, on its own, justify
the structural change.

---

# Readiness: Open Pincery — v9 Phase G Slice G2 (AC-77 Seccomp Default-Deny Allowlist)

> This addendum covers Slice G2 / AC-77 only. It is the admission gate
> immediately after AC-76 closed 12/12 on 2026-04-30 (CI run
> `25197562247`). Prior addenda (G1a/b/c/d, G0f, G0a..e) are retained
> verbatim below as historical record and are not re-opened by this
> gate. AC-77 replaces the existing 11-syscall **denylist** in
> `src/runtime/sandbox/seccomp.rs` with a default-deny **allowlist**
> sourced from empirical strace of the actually-executed v9 workloads.

## Verdict

READY for Slice G2 / AC-77, conditional on the clarifications below
being resolved as part of slice **G2a** (baseline allowlist capture)
rather than blocking entry. The clarifications are bounded — they
affect _which_ syscalls land in the allowlist's tail, not the
pass/fail meaning of AC-77 itself, which is "default-deny + named
permitted set + SIGSYS on unknown + `sandbox_syscall_denied` event +
the AC-76 12-payload corpus stays green". The current denylist
(`denied_syscalls()` returning 11 entries with
`mismatch_action = SeccompAction::Allow`) is a known interim per the
file's own header comment; AC-77 is the planned tightening path.

The seccomp install is already plumbed through **two** call sites that
both consume the same source-of-truth in
`src/runtime/sandbox/seccomp.rs`:

1. The pre-AC-83 path: `bwrap --seccomp <fd>` via
   `compose_seccomp_fd()` in `src/runtime/sandbox/bwrap.rs:530`.
2. The current AC-83 path: `pincery-init` reads the BPF byte stream
   from `SandboxInitPolicy.seccomp_bpf` and installs it via
   `prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, ...)` in
   `src/bin/pincery_init.rs::apply_seccomp` (verified post-install via
   `/proc/self/status` `Seccomp: 2`).

Both paths share `denied_syscalls()` -> `build_bpf_program()` ->
`SeccompFilter::new(...)`. AC-77 changes ONLY the input (allowlist
table) and the `mismatch_action` (`Allow` -> `KillProcess`) and the
match action posture; both call sites pick up the change without
structural edits.

## Truths

- **T-AC77-1** After AC-77 lands, the seccomp BPF program installed
  inside every sandboxed child (via `pincery-init`) is a
  **default-deny allowlist**: `SeccompFilter::new(rules,
mismatch_action = KillProcess (Enforce) / Log (Audit), match_action
  = Allow, target_arch)`. The current posture (mismatch=Allow,
  match=KillProcess on a small denied set) is inverted. SIGSYS (signal
  31, exit code 159) is the kernel-visible denial signature for any
  syscall not on the list in Enforce mode.
- **T-AC77-2** The allowlist is **sourced empirically** from the
  observed syscall set of the actually-executed v9 happy-path
  workloads, not hand-rolled from documentation. The corpus is, at
  minimum: (a) the existing AC-53 / AC-76 happy-path command set
  (`echo`, `sh -c`, `/bin/true`, `head -c`, `seq`, `dd`, `cat`,
  `id -u`, `command -v`, `unshare`, `mount` probes); (b) glibc/musl
  startup syscalls observed in the same runs (`execve`, `brk`,
  `arch_prctl`, `set_tid_address`, `set_robust_list`, `rseq`,
  `mmap`/`mprotect`/`munmap`, `read`/`write`/`close`, `openat`,
  `newfstatat`/`fstat`, `getrandom`, `prlimit64`, `clock_*`, `futex`,
  `exit_group`); (c) `pincery-init`'s own residual syscalls between
  the seccomp install and `execvp` of the user binary. AC-66's
  business tools (`http_get`, `file_read`, `db_query`) are NOT in v9
  scope yet (Phase F1, deferred) — see Clarification C-AC77-3 for
  how the source comment / commit body must reflect that.
- **T-AC77-3** The allowlist explicitly does NOT include the
  scope.md AC-77-named escape-class syscalls regardless of whether
  they appear in any strace: `io_uring_setup`, `io_uring_register`,
  `io_uring_enter`, `bpf`, `perf_event_open`, `name_to_handle_at`,
  `open_by_handle_at`, all `fanotify_*`, plus the existing AC-77
  denylist set (`mount`, `umount2`, `pivot_root`, `init_module`,
  `finit_module`, `delete_module`, `kexec_load`, `kexec_file_load`,
  `reboot`, `ptrace`). These names are deliberately enumerated as a
  negative-test fixture so the allowlist test asserts they remain
  blocked even if a future strace pass accidentally captures one.
- **T-AC77-4** `clone` and `clone3` require **argument filtering**,
  not bare allow. `clone(CLONE_NEWUSER)` and `clone(CLONE_NEWNS)`
  must be denied; `clone` for ordinary thread creation
  (`CLONE_VM | CLONE_FS | CLONE_FILES | ...` without the namespace
  bits) must be allowed because tokio / glibc threads use it. This
  uses `seccompiler::SeccompRule` with `SeccompCondition` on the
  `flags` argument (arg-0). `clone3` takes a pointer-to-struct,
  which BPF cannot dereference — for `clone3`, the allowlist permits
  it bare and relies on AC-86 (UID drop + cap drop + bwrap
  `--disable-userns`) to make namespace creation EPERM regardless.
  This is documented in the seccomp module header so the next
  reviewer understands the trade.
- **T-AC77-5** `mismatch_action = SECCOMP_RET_KILL_PROCESS` (not
  `RET_ERRNO`). scope.md G2 line 898 is explicit: "SIGSYS unknown
  syscalls; mismatch action `SECCOMP_RET_KILL_PROCESS` (not
  `RET_ERRNO`)". A SIGSYS-killed process is observable from the
  parent (non-zero exit; AC-76 harness already keys off this); an
  ENOSYS would silently degrade a denied syscall into a normal
  errno path, which would mask escape attempts in audit logs.
- **T-AC77-6** A new event type `sandbox_syscall_denied` is
  registered in `src/models/event.rs` (or wherever event types
  live; reconcile during BUILD) with payload `{syscall_nr: i64,
  syscall_name: Option<String>, tool_name: Option<String>,
  agent_id: Option<Uuid>, wake_id: Option<Uuid>}`. Emission requires
  observing the SIGSYS — the parent sees `WIFSIGNALED && WTERMSIG ==
  SIGSYS` from `wait_with_output` and writes the event before
  returning the `ExecResult`. The parent also reads
  `/proc/<pid>/status`'s `SigQ` or, more reliably, parses the
  audit netlink `AUDIT_SECCOMP` record (see
  `src/observability/landlock_audit.rs` for the netlink reader
  pattern shipped by AC-88) to extract `syscall_nr`. If the audit
  source is unavailable (ABI gap), `syscall_nr = -1` and
  `syscall_name = None` are recorded, with the SIGSYS exit alone
  proving the denial. Tool / agent / wake context comes from the
  call-site (`SandboxedExecutor`) via the existing context
  threading; absence is allowed for pre-AC-66 callers.
- **T-AC77-7** All 12 AC-76 payloads remain blocked under the new
  allowlist with their existing denial signatures preserved OR
  upgraded. Concretely: (a) FS-1..4 (path-shaped denials via
  Landlock and bwrap mount-ns) are unaffected — Landlock is enforced
  via `landlock_restrict_self` which is on the allowlist;
  (b) Privesc-1..3 (setuid exec, `CAP_SYS_ADMIN`, user-ns
  elevation) tighten — `unshare(CLONE_NEWUSER)` and `clone(...,
  CLONE_NEWUSER, ...)` now SIGSYS at the seccomp layer BEFORE
  reaching bwrap's `--disable-userns` EPERM (defense-in-depth, both
  signatures accepted); `setuid` of a real-root binary is already
  EPERM via cap-drop, unaffected; (c) Resource-1..3 (fork-bomb,
  memory-balloon, pid-exhaustion) — `fork`/`clone`/`execve` are on
  the allowlist (must be), so cgroup v2 remains the enforcing layer
  exactly as in G1c; (d) Network-1..3 — raw socket via `socket(2)`
  with `AF_PACKET` or `SOCK_RAW` is in scope for AC-77 to deny via
  arg filtering on `socket(domain, type, ...)` — see
  Clarification C-AC77-2.
- **T-AC77-8** AC-77 preserves the existing **mode posture**.
  `Enforce` -> kill, `Audit` -> log, `Disabled` -> no-install,
  exactly as `seccomp.rs::build_bpf_program` already branches.
  The `OPEN_PINCERY_SANDBOX_FLOOR=relaxed` + `ALLOW_UNSAFE=true`
  escape valve from AC-84 does NOT relax the seccomp layer — the
  relaxed floor only downgrades Landlock ABI (per
  `scaffolding/design.md` Open Question resolution at design.md:2467
  area). Operators who need to shed seccomp specifically must use
  `SandboxProfile.seccomp = false` or `SandboxMode::Disabled`,
  exactly as today.
- **T-AC77-9** AC-77 does NOT relax the AC-83/AC-85
  FullyEnforced-or-refuse posture. `pincery-init` already verifies
  `Seccomp: 2` in `/proc/self/status` after `apply_seccomp`; an
  empty `policy.seccomp_bpf` is currently log-and-skip but, when
  combined with `SandboxProfile.seccomp = true`,
  `compose_seccomp_program` MUST return a non-empty allowlist
  program. This is preserved.
- **T-AC77-10** AC-77 is **architecture-aware via seccompiler**.
  `seccompiler::SeccompFilter::new(... target_arch ...)` already
  reads `std::env::consts::ARCH`. Adding allowlist entries by name
  (`libc::SYS_*` constants) means the resolution to the right
  syscall number for x86_64 / aarch64 happens at compile time per
  arch. CI (x86_64) is the primary proof; aarch64 is not in v9 CI.
  The test asserts on `cfg(target_arch = "x86_64")` only; aarch64
  is a follow-up validated via devshell when an operator runs there.

## Key Links (AC -> Design -> Test -> Proof)

- **L-AC77-1** `AC-77` -> `src/runtime/sandbox/seccomp.rs` rewrite:
  `denied_syscalls()` -> `allowed_syscalls()` (Vec<i64>) +
  `clone_namespace_arg_rules()` (Vec<(i64, Vec<SeccompRule>)>) ->
  `build_bpf_program(SandboxMode)` returns inverted-posture filter
  -> consumed unchanged by both `compose_seccomp_fd` (bwrap path) and
  `compose_seccomp_program` (pincery-init path) -> **planned test**
  `tests/seccomp_allowlist_test.rs::allowlist_blocks_io_uring_setup`
  asserts SIGSYS exit; **runtime proof** CI privileged
  `sandbox real-bwrap smoke` job runs the test in the production
  pipeline.
- **L-AC77-2** `AC-77` -> `tests/seccomp_allowlist_test.rs`
  positive-control test re-runs the AC-76 12-payload corpus
  (`tests/sandbox_escape_test.rs::*` re-execution under the new
  allowlist) -> **planned proof** CI privileged sandbox-smoke job
  shows all 12 payloads still blocked with the same or upgraded
  signature.
- **L-AC77-3** `AC-77` -> `src/models/event.rs`
  `EventType::SandboxSyscallDenied` registration -> **planned test**
  `tests/event_log_test.rs` (or a new `seccomp_event_test.rs`)
  asserts the event type round-trips through the DB and lint
  catalog -> **runtime proof** CI's existing event-type lint job
  + a sandbox-smoke run that triggers a SIGSYS and asserts the
  corresponding event row appears in the test DB.
- **L-AC77-4** `AC-77` -> `src/runtime/sandbox/bwrap.rs`
  `RealSandbox::run` post-wait branch: detect SIGSYS exit, capture
  `syscall_nr` from audit netlink (or `None` fallback), call
  `models::event::append_event(SandboxSyscallDenied { ... })` ->
  **planned test** unit test on the SIGSYS-detection helper +
  integration test that runs an `io_uring_setup` payload and
  asserts the event appears -> **runtime proof** CI sandbox-smoke
  privileged job.
- **L-AC77-5** `AC-77` -> `scaffolding/design.md` "external
  integrations" row for `seccompiler` (line 2444) — already says
  "syscall allowlist", which is now finally true. No design.md
  edit required by ANALYZE; BUILD's RECONCILE phase will confirm.
- **L-AC77-6** `AC-77` -> AC-76 G1b privesc tests
  (`privesc_setuid_exec_blocked`, `privesc_cap_sys_admin_blocked`,
  `privesc_user_ns_elevation_blocked` in
  `tests/sandbox_escape_test.rs`) -> existing assertion lists
  already accept `bad system call`, signal-31 / 159 exit, and the
  bwrap EPERM strings; SIGSYS upgrades or matches each of them ->
  **runtime proof** CI sandbox-smoke job's existing G1b assertions
  remain green without test edits. If any signature requires
  widening, that is captured under Slice G2c.

## Acceptance Criteria Coverage (AC-77 slice)

| AC    | Truth(s)            | Planned test                                                                                                                                                                                                                            | Planned runtime proof                                                                          |
| ----- | ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| AC-77 | T-AC77-1, T-5, T-10 | `tests/seccomp_allowlist_test.rs::allowlist_program_uses_default_deny` (build the BpfProgram, decompile/inspect via seccompiler API or `bpfvm`-style golden, assert mismatch_action == KillProcess in Enforce, == Log in Audit)         | CI sandbox-smoke job logs program metadata at install                                          |
| AC-77 | T-AC77-2, T-3       | `tests/seccomp_allowlist_test.rs::allowlist_covers_happy_path_workloads` (run each AC-76 happy-path command via `RealSandbox::run` under `SandboxMode::Enforce`; assert exit 0)                                                         | CI sandbox-smoke job: existing AC-76 happy-path baseline (currently green) remains green       |
| AC-77 | T-AC77-3, T-4       | `tests/seccomp_allowlist_test.rs::allowlist_blocks_io_uring_setup` + `..._blocks_bpf` + `..._blocks_perf_event_open` + `..._blocks_user_ns_clone` (each payload SIGSYS-exits 159 with `bad system call` in stderr)                      | CI sandbox-smoke job: 4 new SIGSYS payloads green                                              |
| AC-77 | T-AC77-6, T-4       | `tests/seccomp_allowlist_test.rs::sigsys_emits_sandbox_syscall_denied_event` (trigger denied syscall; assert `events` table row appears with `event_type = sandbox_syscall_denied` and `payload.syscall_nr` is the expected number)     | CI sandbox-smoke job: event row visible in test DB after run                                   |
| AC-77 | T-AC77-7            | (existing, no edits) `tests/sandbox_escape_test.rs::*` 12-payload suite re-runs under the new allowlist                                                                                                                                 | CI run on the AC-77 PR shows all 12 G1a/b/c/d payloads remain blocked (no signature regression) |
| AC-77 | T-AC77-8, T-9       | `tests/seccomp_allowlist_test.rs::audit_mode_logs_instead_of_killing` (negative-only assertion: run a disallowed syscall under `SandboxMode::Audit` and assert exit_code != SIGSYS_EXIT_CODE; the kernel-level Audit Log mismatch action is unit-tested by `seccomp.rs::build_program_audit_uses_log_on_mismatch` + `enforce_and_audit_programs_differ`. Event-row emission on SIGSYS is covered separately by `tests/sigsys_event_test.rs::sigsys_exit_emits_sandbox_syscall_denied_event` on the Enforce path) | CI sandbox-smoke job: audit-mode payload exits without SIGSYS                                  |

## Scope Reduction Risks

- **R-AC77-1 (highest) — "Inverted denylist" allowlist.** The
  cheapest path to a green BUILD is to take the current 11-entry
  denylist, name it "denied" inside an otherwise-`Allow` filter
  with `mismatch_action = KillProcess`, and call it an allowlist.
  This would technically flip the posture but ship a one-syscall
  effective allowlist that immediately SIGSYS-kills `echo`. The
  inverse failure mode is shipping an allowlist so wide it is
  effectively a denylist (e.g. allowing the entire `_x86_64`
  syscall surface and only excluding the named escape primitives).
  **Mitigation**: AC coverage row 2 (happy-path) AND row 3 (denied
  primitives) are both required-green. The test
  `allowlist_program_uses_default_deny` additionally asserts the
  allowlist size is in `60..=120` syscalls (a hand-rolled cap that
  rejects "allow everything"). If a real workload pushes past 120,
  the cap is raised in scope.md, not silently in BUILD.
- **R-AC77-2 — Empirical strace omitted in favor of "common-sense
  defaults".** The scope.md text is explicit: "sourced from
  empirical strace of the 12 built-in tools". BUILD may be tempted
  to skip the strace pass and hand-roll a list from the
  `seccomp-defaults`-style examples in seccompiler docs. **Mitigation**:
  Slice G2a's deliverable is a checked-in trace artifact under
  `tests/fixtures/seccomp/strace_*.txt` (or equivalent) that
  the allowlist source comments cite by line; the regen-on-new-tool
  diff-fail (scope.md AC-77 part 3) is wired in Slice G2c. If the
  fixture is missing, REVIEW must reject the slice.
- **R-AC77-3 — `clone` allowed bare, namespace bits not filtered.**
  The seccompiler arg-filter is non-trivial; the cheapest BUILD is
  `(SYS_clone, vec![])` -> bare allow. This would mean an attacker
  inside the sandbox could in principle call
  `clone(CLONE_NEWUSER|CLONE_NEWNS, ...)` and the seccomp layer
  would not fire — defense would fall entirely to AC-86's
  `--disable-userns` and bwrap's mount-ns lock. **Mitigation**:
  T-AC77-4 makes the arg filter non-optional; the
  `allowlist_blocks_user_ns_clone` test asserts SIGSYS rather than
  EPERM (which would prove only the bwrap layer fired). For
  `clone3`, the bare allow is documented and tied to AC-86 in the
  module header, not silently accepted.
- **R-AC77-4 — `sandbox_syscall_denied` event silently dropped.**
  The cheapest path to "tests pass" is to plumb the event type
  through the catalog but not actually emit it — the AC-76 SIGSYS
  exit code alone would still close the existing tests.
  **Mitigation**: AC coverage row 4 makes the event-row assertion
  required-green. The event must contain a non-null `syscall_nr`
  on kernels with audit netlink available (T-AC77-6); on kernels
  without, `-1` is documented and tested with a stubbed audit
  source (mirroring the AC-88 stubbing pattern).

## Clarifications Needed

- **C-AC77-1 — Audit netlink correlation reuse.** AC-77 needs the
  syscall number to populate `sandbox_syscall_denied.syscall_nr`.
  The AC-88 reader (`src/observability/landlock_audit.rs`) already
  consumes `AUDIT_LANDLOCK_*` records; `AUDIT_SECCOMP` is a
  different record type but on the same netlink socket.
  **Bounded assumption for ANALYZE**: the AC-88 reader is extended
  in Slice G2b to also surface `AUDIT_SECCOMP` records, reusing
  the same PID/timestamp correlation strategy. If extending the
  reader proves architecturally awkward, a parallel reader in
  `src/observability/seccomp_audit.rs` is acceptable; either way,
  the assumption is "the netlink path generalizes". This does not
  change the pass/fail meaning of AC-77 — the fallback
  (`syscall_nr = -1`, exit-code-only proof) is also tested.
- **C-AC77-2 — Raw socket denial layer.** AC-76 G1d's network
  payload `raw_socket_open_blocked` currently relies on AC-86's
  `--cap-drop ALL` removing `CAP_NET_RAW`; the kernel returns
  EPERM from `socket(AF_INET, SOCK_RAW, ...)`. Should AC-77
  additionally arg-filter `socket(2)` to deny `AF_PACKET` and
  `SOCK_RAW` at the seccomp layer (defense-in-depth, two-layer
  block)? **Bounded assumption for ANALYZE**: NO for v9 — AC-77
  ships the syscall-number allowlist + the named negative-fixture
  set; `socket` arg-filtering is deferred to a v10 hardening pass
  and noted as such in the seccomp module header. AC-76 G1d's
  test continues to assert the existing EPERM signature; nothing
  regresses. If REVIEW disagrees, `socket` arg-filter is a Slice
  G2c addition, not a re-ANALYZE.
- **C-AC77-3 — AC-66 business tools not yet in v9.** scope.md
  AC-77 names "the 12 built-in tools" and lists `http_get`,
  `file_read`, `db_query`. AC-66 (Tool Catalog Expansion) is
  Phase F1 and has not landed in v9 yet (Phase G is still mid-flight).
  **Bounded assumption for ANALYZE**: the v9 AC-77 allowlist is
  derived from the workloads that ACTUALLY run in v9 today (AC-53
  command shapes, AC-76 escape-suite payloads, pincery-init,
  glibc/musl startup). When AC-66 lands, its slice is responsible
  for re-running the strace-and-diff regen and adding any new
  syscalls under the AC-77 source-of-truth (this is the diff-fail
  test scope.md calls for). The AC-77 source comment must call
  this out so the next ANALYZE for AC-66 picks it up.
- **C-AC77-4 — libseccomp vs seccompiler.** scope.md "Stack" line
  lists both `libseccomp` and `seccompiler` as the choice; the
  current code uses `seccompiler` only (no `libseccomp` system
  dep). **Bounded assumption for ANALYZE**: stay on `seccompiler`
  — it already supports allowlists and arg-filter rules, the BPF
  emission path is proven by AC-53 / G0a, and adding a
  `libseccomp` system-binary dep would break devshell parity for
  Mac/Windows contributors. design.md line 2444 says
  "`libseccomp` / `seccompiler` crate" — the slash is interpreted
  as "either is acceptable; we ship seccompiler". No design.md
  edit required.
- **C-AC77-5 — `clone3` bare-allow with cap+userns lockout.**
  Documented above (T-AC77-4) — calling out here so the next
  reviewer sees it without hunting through Truths. The
  alternative (deny `clone3` entirely) is rejected because tokio
  may use `clone3` for thread creation on glibc >= 2.34 and
  recent kernels; denying it would SIGSYS the runtime startup.
  Bounded; not blocking.

## Build Order

- **G2a — Empirical syscall corpus capture.** Run `strace -f -e
trace=all -o trace.txt` against each AC-76 happy-path command
  inside the production sandbox profile (devshell + privileged CI
  job). Aggregate the union of syscalls observed. Check the
  fixture into `tests/fixtures/seccomp/` with a manifest naming
  the source command and the kernel/glibc version that produced
  it. No `src/` changes.
- **G2b — Allowlist source-of-truth + filter rewrite.** Edit
  `src/runtime/sandbox/seccomp.rs`: replace `denied_syscalls()`
  with `allowed_syscalls()` (alphabetized `Vec<i64>`) and a
  `clone_namespace_arg_rules()` helper returning
  `Vec<(i64, Vec<SeccompRule>)>`. Flip
  `SeccompFilter::new(...mismatch_action..., ...match_action...)`
  to `mismatch_action = KillProcess` (Enforce) / `Log` (Audit) and
  `match_action = Allow`. Module header rewritten to document
  the new posture; the "denylist not yet a true allowlist"
  comment is replaced with the allowlist rationale. Both
  `compose_seccomp_fd` and `compose_seccomp_program` (consumed by
  bwrap and pincery-init respectively) are unchanged.
- **G2c — `sandbox_syscall_denied` event + audit netlink
  extension.** Register `EventType::SandboxSyscallDenied` in
  `src/models/event.rs` (or `src/models/events.rs`). Extend
  `src/observability/landlock_audit.rs` (or add a sibling
  `seccomp_audit.rs`) to surface `AUDIT_SECCOMP` records,
  populate `syscall_nr`, and emit one event per SIGSYS observed.
  Wire the parent post-wait branch in `src/runtime/sandbox/bwrap.rs`
  to emit the event when `WIFSIGNALED && WTERMSIG == SIGSYS`,
  using the audit reader for `syscall_nr` when available and
  `-1` otherwise.
- **G2d — Test suite + AC-76 corpus re-verification.** Add
  `tests/seccomp_allowlist_test.rs` with the seven planned tests
  (program-shape, happy-path, four SIGSYS payloads, audit-mode,
  event emission). Re-run the full AC-76 12-payload suite under
  the new allowlist; if any signature widens (e.g. a privesc
  payload now exits 159 instead of bwrap-EPERM), the
  `assert_payload_blocked` signature list is extended in the same
  slice. CI privileged sandbox-smoke job is the runtime proof.
- **G2e — Regen-on-new-tool diff-fail wiring.** A test
  `tests/seccomp_allowlist_test.rs::allowlist_matches_observed_corpus`
  re-runs the strace pass at test time (when
  `OPEN_PINCERY_RUN_AC77_REGEN=1` is set, mirroring the AC-84
  positive-test gate pattern) and diff-fails if the observed
  syscall set is not a subset of the allowlist. Default-off so
  unprivileged CI doesn't regress; on the privileged sandbox
  smoke job, the gate flag is set.
- **G2f — Documentation pass.** Update the `seccomp.rs` module
  header to describe the allowlist as the new ground truth (no
  more "shipped a denylist" disclaimer); add a one-line entry to
  `CHANGELOG.md` under v9 Phase G; update the `seccomp` row in
  `scaffolding/design.md` external-integrations table during
  RECONCILE if the test-strategy column needs widening
  ("Unit + adversarial allowlist test + 12-payload re-verify").

## Complexity Exceptions

- **CE-AC77-1 — `seccomp.rs` may exceed 300 LOC.** The current file
  is ~250 LOC of denylist-shaped code. The allowlist is 60-120
  syscall entries (one line each), plus the `clone` arg-rule
  helper, plus an extended module header documenting the audit
  reader integration and the `clone3`/`socket` deferred-hardening
  notes. Estimated post-G2 size: 450-550 LOC. This is a justified
  exception per scope.md "Complexity Brake" — the BPF program is
  by nature a long flat table; splitting it would harm reviewability.
  If it crosses 600 LOC, the syscall table moves to a JSON fixture
  loaded at compile time, but the v9 expectation is one Rust file.
- **CE-AC77-2 — Audit netlink reader may grow.** Extending the
  AC-88 reader to handle `AUDIT_SECCOMP` records adds ~80-150 LOC
  to `src/observability/landlock_audit.rs` (renamed conceptually
  to `kernel_audit.rs` if the split is taken). The 300-LOC ceiling
  may be approached. If REVIEW finds it exceeds, splitting along
  record-type lines (`landlock_audit.rs` keeps Landlock,
  `seccomp_audit.rs` adds Seccomp, both share a `netlink.rs`
  helper) is the planned refactor.

# Readiness: Open Pincery — v9 Phase G0f (AC-88 Kernel Audit Integration)

> This addendum covers AC-88 only. It was produced after AC-87 VERIFY
> closed and before BUILD began for Slice G0f. RECONCILE on
> 2026-04-28 updated this top section to match the implemented and
> reviewed Slice G0f shape. Historical G0a / v7 / v8 readiness records
> below are preserved verbatim as prior context and are not re-opened by
> this admission gate.

## Verdict

READY for Slice G0f / AC-88. Reconciled after REVIEW: no Critical or
Required findings remain, and the current implementation preserves the
AC-84 / AC-87 Landlock ABI >= 6 enforcement floor while degrading only
audit visibility below ABI 7.

AC-83 through AC-87 have landed according to the experiment log: the
current sandbox path already has the parent -> wrapper
`SandboxInitPolicy` boundary, `RealSandbox` policy construction,
`pincery-init` in-sandbox restriction application, raw Landlock scope
syscalls in `src/runtime/sandbox/landlock.rs`, `src/observability/` as
the observability module root, and `models::event::append_event` as the
append-only event-log write seam. AC-88 is therefore an additive
audit-visibility slice.

## Truths

- **T-AC88-1** AC-88 does not change the enforcement floor shipped by
  AC-84 / AC-87: strict startup still requires Landlock ABI >= 6, and
  ABI < 7 only degrades audit-log availability. It must not disable
  Landlock, relax `require_fully_enforced`, or fall back to the old
  parent-side `pre_exec` install path.
- **T-AC88-2** `SandboxInitPolicy` remains the only parent ->
  `pincery-init` IPC type boundary. It now carries
  `landlock_restrict_flags: u32`. `RealSandbox` sets
  `LANDLOCK_RESTRICT_SELF_LOG_NEW_EXEC_ON` only when
  `KernelProbe::landlock_abi() >= 7`; ABI 6 sends `0` so enforcement
  continues without audit flag support.
- **T-AC88-3** `pincery-init` applies Landlock through
  `install_landlock_with_restrict_flags`. When a nonzero restrict flag
  is requested, `src/runtime/sandbox/landlock.rs` uses the narrow raw
  filesystem ruleset path needed to pass
  `LANDLOCK_RESTRICT_SELF_LOG_NEW_EXEC_ON`, then preserves AC-85
  FullyEnforced validation.
- **T-AC88-4** Audit collection is per shell invocation, not a
  long-lived background daemon. `invocation_audit_source_from_end()`
  tries Linux audit netlink first and then falls back to an audit log
  file opened from EOF. The fallback path is
  `OPEN_PINCERY_LANDLOCK_AUDIT_LOG` when set, otherwise
  `/var/log/audit/audit.log`.
- **T-AC88-5** `src/observability/landlock_audit.rs` translates parsed
  `AUDIT_LANDLOCK_*` denial records into append-only
  `landlock_denied` events through `models::event::append_event`. The
  canonical JSON payload includes the required `{tool_name, agent_id,
denied_path, requested_access, syscall}` fields plus `wake_id`,
  sampled `correlation_pids`, audit `pid`, audit `ppid`/`parent_pid`,
  and audit timestamp when available.
- **T-AC88-6** Kernel audit records must be correlated to real runtime
  context before event emission. The implementation correlates against
  sampled process-tree PIDs from `RealSandbox`, parsed audit `pid`,
  parsed `ppid` / `parent_pid`, and the tool invocation start/finish
  timestamp window. Missing or stale correlation never emits a fake
  `landlock_denied` event.
- **T-AC88-7** `append_landlock_denials_within` polls for bounded
  delayed delivery and continues after each append until the overall
  window expires or a post-append quiet period elapses. This is the
  runtime proof path for audit records that arrive shortly after the
  sandboxed process exits.
- **T-AC88-8** On ABI < 7, or when no readable audit source is
  available, the system emits a one-time structured
  `audit_log_unavailable` warning with the observed ABI/source failure
  reason. Sandbox enforcement continues. Agent-scoped events are
  appended only when a real `{agent_id, wake_id, tool_name}` context
  exists.
- **T-AC88-9** `tests/landlock_audit_test.rs` includes deterministic
  parser, EOF fallback, ABI 6 fallback, delayed polling, parent-PID
  correlation, timestamp-window rejection/acceptance, uncorrelated
  rejection, and live tests gated by explicit ABI/source preconditions.
  `src/observability/landlock_audit_netlink.rs` contains deterministic
  `nlmsghdr` fixture coverage for netlink decoding.
- **T-AC88-10** The deployment path forwards the audit-log fallback
  override into the app container. `docker-compose.yml` passes
  `OPEN_PINCERY_LANDLOCK_AUDIT_LOG` via optional `${VAR:-}`
  interpolation, so AC-88's file fallback is configurable in Docker
  deployments and not only in local host runs.

## Key Links

- **L-AC88-1** [AC-88] -> `SandboxInitPolicy` in
  `src/runtime/sandbox/init_policy.rs` + `RealSandbox` policy builder in
  `src/runtime/sandbox/bwrap.rs` -> unit coverage for ABI 6/7 policy
  construction -> runtime proof that ABI 7 policies request the audit
  flag while ABI 6 emits `audit_log_unavailable` and keeps enforcement.
- **L-AC88-2** [AC-88] -> raw Landlock syscall support in
  `src/runtime/sandbox/landlock.rs` + `pincery-init::apply_landlock` in
  `src/bin/pincery_init.rs` -> unit coverage of the UAPI flag and raw
  path-beneath layout -> runtime proof that denied filesystem access can
  produce kernel audit material on ABI >= 7.
- **L-AC88-3** [AC-88] -> `src/observability/mod.rs`,
  `src/observability/landlock_audit.rs`, and
  `src/observability/landlock_audit_netlink.rs` -> deterministic parser
  aliases, EOF file fallback, and `nlmsghdr` decode tests -> runtime
  proof that parsed denied path/access/syscall fields match fixture and
  live records.
- **L-AC88-4** [AC-88] -> `ExecResult::Ok { audit_pids }` in
  `src/runtime/sandbox/mod.rs`, process-tree PID sampling in
  `src/runtime/sandbox/bwrap.rs`, and tool invocation timestamps in
  `src/runtime/tools.rs` -> tests for parent PID correlation and
  timestamp-window rejection -> runtime proof that stale PID reuse is
  rejected.
- **L-AC88-5** [AC-88] -> `models::event::append_event` in
  `src/models/event.rs` -> `append_landlock_denied_event` and
  `append_landlock_denials_within` in
  `src/observability/landlock_audit.rs` -> event-log assertions in
  `tests/landlock_audit_test.rs` -> runtime proof that correlated
  `landlock_denied` appears in the agent event log for a denied open.
- **L-AC88-6** [AC-88] -> ABI/source fallback in
  `audit_log_unavailable_for_abi` and
  `invocation_audit_source_from_end` -> deterministic ABI 6 test and
  live precondition tests -> proof that unavailable audit visibility is
  explicit and does not disable the sandbox.
- **L-AC88-7** [AC-88] -> `OPEN_PINCERY_LANDLOCK_AUDIT_LOG` fallback
  configuration in `.env.example` and `docker-compose.yml` ->
  `tests/env_example_test.rs` source-to-example coverage and
  `tests/compose_env_test.rs` optional forwarded-var contract plus
  gated `docker compose config` fixture -> proof that the fallback path
  remains operator-configurable in the deployment container.

## Acceptance Criteria Coverage

| AC ID | Build Slice                                                                                                                                         | Test / Proof                                                                                                                                                                                                              | Runtime Verification                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | Status                   |
| ----- | --------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------ |
| AC-88 | G0f: audit flag in policy + raw Landlock restrict flag + per-invocation audit source/parser + correlated event emission + deployment env forwarding | `tests/landlock_audit_test.rs`; `src/observability/landlock_audit_netlink.rs` unit fixture; `src/runtime/sandbox/{init_policy,bwrap,landlock}.rs` unit coverage; `tests/compose_env_test.rs`; `tests/env_example_test.rs` | Local deterministic tests cover parser aliases, EOF fallback, bounded two-record delayed polling, parent-PID correlation, timestamp-window rejection/acceptance, uncorrelated rejection, and ABI 6 audit-unavailable fallback. Linux live tests run when ABI >= 7 and a readable audit source exists; otherwise they skip with explicit evidence while sandbox enforcement remains active. Compose proof covers `OPEN_PINCERY_LANDLOCK_AUDIT_LOG` forwarding with a gated `COMPOSE_AVAILABLE=1` `docker compose config` fixture, and env example coverage keeps the operator-facing knob documented. | Implemented and reviewed |

## Scope Reduction Risks

- **Landlock flag silently omitted**: Guarded by
  `SandboxInitPolicy.landlock_restrict_flags`, ABI 7 policy tests, and
  the raw `install_landlock_with_restrict_flags` path in
  `src/runtime/sandbox/landlock.rs`.
- **Audit reader without event correlation**: Guarded by process-tree PID
  sampling, audit `ppid` / `parent_pid`, invocation timestamp windows,
  and rejection of uncorrelated records before `append_event`.
- **Fixture-only success**: Deterministic tests always run; live audit
  tests remain ABI/source gated and print explicit skip evidence when
  the host cannot provide ABI 7 or audit read access.
- **ABI < 7 treated as sandbox failure**: Guarded by ABI 6 fallback tests
  and `audit_log_unavailable_for_abi(Some(6))` proving
  `sandbox_still_enforced = true`.
- **Log-only implementation**: Guarded by event-log assertions for
  `landlock_denied`; tracing warnings are only for unavailable audit
  visibility.
- **Fake startup event context**: Guarded by code path: startup/source
  unavailability is a structured warning unless a real agent/tool
  context exists.
- **Operator override not reaching deployment**: Guarded by
  `tests/compose_env_test.rs`, which asserts
  `OPEN_PINCERY_LANDLOCK_AUDIT_LOG` is forwarded through
  `docker-compose.yml` and rendered by the live compose fixture.

## Clarifications Needed

- None blocking.
- `audit_log_unavailable` remains a structured warning for startup or
  source failures unless BUILD has a real agent context. Fake agent IDs
  remain forbidden.
- Live audit capture may require `CAP_AUDIT_READ`, auditd/journald
  configuration, or read permission on `/var/log/audit/audit.log` or
  `OPEN_PINCERY_LANDLOCK_AUDIT_LOG`. This affects the runtime proof
  environment, not the pass/fail meaning of AC-88.

## Build Order

1. **G0f.1 - Policy and ABI gate.** Implemented via
   `SandboxInitPolicy.landlock_restrict_flags` and
   `landlock_restrict_flags_for_abi` in `RealSandbox`.
2. **G0f.2 - Raw Landlock restrict flag.** Implemented via
   `install_landlock_with_restrict_flags` and `pincery-init` policy
   application.
3. **G0f.3 - Audit parser/source abstraction.** Implemented in
   `src/observability/landlock_audit.rs` plus Linux netlink source in
   `src/observability/landlock_audit_netlink.rs`.
4. **G0f.4 - Context correlation and event append.** Implemented via
   sampled process-tree PIDs, audit parent PID parsing, timestamp-window
   filtering, and append-only `landlock_denied` events.
5. **G0f.5 - Runtime proofs.** Implemented in
   `tests/landlock_audit_test.rs`; live Linux proofs are gated and
   deterministic proofs always run.
6. **G0f.6 - Deployment env proof.** Implemented by forwarding
   `OPEN_PINCERY_LANDLOCK_AUDIT_LOG` in `docker-compose.yml` and
   guarding it with `tests/compose_env_test.rs` plus env-example
   coverage.

## Complexity Exceptions

- `src/observability/landlock_audit.rs` is allowed up to 450 lines for
  this slice because it contains the source abstraction, parser,
  correlation rules, bounded polling, and event bridge in one cohesive
  module. The Linux-specific netlink framing code is split into
  `src/observability/landlock_audit_netlink.rs`. Split further if the
  audit module grows past 450 lines or mixes unrelated concerns.
- The narrow raw Landlock filesystem ruleset helper in
  `src/runtime/sandbox/landlock.rs` is permitted because the safe crate
  API cannot pass `LANDLOCK_RESTRICT_SELF_LOG_NEW_EXEC_ON`. It remains
  isolated to the kernel-interface layer and covered by Linux-only and
  deterministic tests.
- Linux live audit proof may be CI/kernel-permission gated. This is not
  a scope reduction when deterministic parser/fallback tests always run
  and the live gate is explicit and evidenced.

---

# Readiness: Open Pincery — v9 Phase G1b (AC-76 Sandbox Escape Suite — Privesc Category)

> This addendum covers Slice G1b only. G1a (FS) closed at commits
> `24ac973` → `dd10a8b` → `4913c6e` (CI run `25141121156` green).
> Slices G1c/G1d/G1e add the resource, network categories and the
> AC-53 closure gate. Prior G1a + G0f readiness sections remain the
> authoritative record for those slices.

## Verdict

READY for Slice G1b / AC-76 (privesc category). Builds on the G1a
harness (precondition gate + `escape_profile()` + `assert_payload_blocked`)
and reuses the production `RealSandbox` + `pincery-init` wrapper.
This slice ships the three privesc-category adversarial payloads named
by `scaffolding/scope.md` AC-76: setuid exec, `CAP_SYS_ADMIN` syscall,
user-ns elevation. Resource, network, and the synthesized
`sandbox_blocked` event remain explicitly deferred to G1c/G1d/G1e.

## Truths

- **T-AC76-G1b-1** Slice G1b extends `tests/sandbox_escape_test.rs`
  with three privesc payloads. The shared precondition gate, profile
  helper, and `assert_payload_blocked` from G1a are reused verbatim;
  no new harness code is added.
- **T-AC76-G1b-2** The three privesc payloads are:
  1. **Setuid exec** — verify that even if a setuid-root binary is
     reachable inside the sandbox view, `execve` does not elevate.
     `pincery-init` sets `PR_SET_NO_NEW_PRIVS`; bwrap also sets
     `--unshare-user --uid 65534 --gid 65534` per AC-86. The payload
     attempts `id -u` after exec'ing a candidate setuid binary
     (`/usr/bin/su`, `/usr/bin/sudo`, `/usr/bin/passwd`, `/bin/su`)
     and asserts the effective uid stays at 65534, OR the binary is
     simply not present in the sandbox view (also a valid block).
     Denial signatures: "must be run from a terminal", "permission
     denied", "no such file or directory", "operation not permitted",
     "authentication failure", "must be setuid", "may not be used".
  2. **`CAP_SYS_ADMIN` syscall** — invoke a syscall that requires
     `CAP_SYS_ADMIN` (e.g. `unshare --user --map-root-user true`,
     `chroot /tmp /bin/true`). AC-86 cap-drop removes the capability
     from the bounding set; AC-77's seccomp denylist also blocks
     several admin syscalls. Denial signatures: "operation not
     permitted", "permission denied", "must be superuser", "bad
     system call", "only root can".
  3. **User-namespace elevation** — `unshare -U` to spawn a new user
     namespace where uid 0 is mapped. With `kernel.apparmor_restrict_unprivileged_userns=0`
     set on the privileged CI host this is normally permitted, but
     the in-sandbox `pincery-init` already holds `PR_SET_NO_NEW_PRIVS`
     and the seccomp filter denies the chained `mount`/`pivot_root`
     primitives that make the namespace exploitable. The payload
     `unshare -U -r /bin/true 2>&1; status=$?; exit "$status"`
     asserts either denial of the unshare itself, or denial of the
     follow-on `id -u` showing it cannot reach root. Denial
     signatures: "operation not permitted", "permission denied",
     "bad system call", "no such file", "must be superuser".
- **T-AC76-G1b-3** Each payload runs through the production
  `RealSandbox` `Enforce` mode under `escape_profile()` (every defence
  layer on). No payload sets `OPEN_PINCERY_SANDBOX_FLOOR=relaxed`,
  `OPEN_PINCERY_ALLOW_UNSAFE`, or `OPEN_PINCERY_INIT_FORCE_PARTIAL`.
- **T-AC76-G1b-4** Every assertion has TWO checks: non-zero
  `exit_code` AND a denial-signature match in stdout/stderr.
  Bare-exit-code blocks (which a missing binary would pass) are
  rejected by the harness from G1a.
- **T-AC76-G1b-5** G1b does not weaken the AC-84/AC-85 enforcement
  floor and does not add new runtime code in `src/`. Test-only delta.
- **T-AC76-G1b-6** G1b is cross-platform-buildable and
  Linux-only-runnable. The whole file remains
  `#![cfg(target_os = "linux")]`. Privileged CI `sandbox-smoke` is
  the runtime proof; Docker Desktop devshell self-skips strict-floor
  checks (Landlock ABI Some(3) < floor 6).
- **T-AC76-G1b-7** G1b binds the same canonical TLA+ actions as G1a
  (`ProvisionSandbox`, `ScopeFilesystem`, `BindShellPolicy`,
  `AttestSandbox`); no new bindings. `ScopeNetwork` lands with G1d.

## Key Links

- **L-AC76-G1b-1** [AC-76 privesc / setuid exec] -> AC-86 uid-drop
  to 65534 + `pincery-init` `PR_SET_NO_NEW_PRIVS` + bwrap's
  `--unshare-user --uid 65534` -> a setuid bit on a binary cannot
  escalate during `execve`. Asserted by either denial signature OR
  `id -u` reporting 65534.
- **L-AC76-G1b-2** [AC-76 privesc / CAP_SYS_ADMIN] -> AC-86
  cap-drop ALL removes `CAP_SYS_ADMIN` from the bounding set;
  `unshare --user --map-root-user` / `chroot` therefore EPERM.
  AC-77 seccomp denylist provides defence-in-depth.
- **L-AC76-G1b-3** [AC-76 privesc / user-ns elevation] -> even
  when the kernel permits `unshare -U`, the seccomp denylist
  blocks the `mount`/`pivot_root` primitives needed to weaponise
  the new namespace, and `PR_SET_NO_NEW_PRIVS` prevents
  setuid-exec inside it. Asserted via the chained command's
  exit status.

## Acceptance Criteria Coverage

| AC ID | Build Slice                                                             | Test / Proof                                                                                        | Runtime Verification                                                                                                                                                               | Status                               |
| ----- | ----------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| AC-76 | G1b: 3 privesc payloads (setuid exec, CAP_SYS_ADMIN, user-ns elevation) | `tests/sandbox_escape_test.rs` extended (three new `#[tokio::test]` functions); reuses G1a harness. | Privileged CI `sandbox-smoke` job runs all 7 payloads (4 FS + 3 privesc) and fails if any block is missing or any payload succeeds. Local devshell self-skips strict-floor checks. | G1b in progress; G1c/G1d/G1e queued. |

## Scope Reduction Risks

- **Test passes because the setuid binary is missing**: explicit
  acceptable signature — `assert_payload_blocked` accepts "no such
  file or directory" because _absence_ of the setuid binary inside
  the sandbox is itself a valid block (the binary cannot escalate
  what is not reachable). This is documented in T-AC76-G1b-2 #1.
- **Payload "blocks" because the shell is not present**: guarded by
  the G1a precondition gate (bwrap on PATH, Landlock supported, ABI
  > = floor) plus the privileged CI image which carries coreutils,
  > `unshare`, and `su`/`sudo`/`passwd`.
- **Effective uid not actually checked**: setuid payload uses
  `id -u` _after_ the candidate exec; an unconditional non-zero
  exit from a missing binary still requires a recognised signature.
- **Floor relaxation creep**: T-AC76-G1b-3 explicitly forbids the
  relaxed/allow-unsafe/force-partial knobs.

## Clarifications Needed

- None blocking. The three payloads in scope.md (`setuid exec`,
  `CAP_SYS_ADMIN syscall`, `user-ns elevation`) map 1:1 to the three
  test functions and the three Key Links above. No interpretation
  required.

## Build Order

1. **G1b.1 — Setuid exec payload.** New `#[tokio::test]`
   `privesc_setuid_exec_blocked` in `tests/sandbox_escape_test.rs`.
   Iterates candidate setuid binaries; asserts none yields uid 0.
2. **G1b.2 — `CAP_SYS_ADMIN` syscall payload.** New
   `privesc_cap_sys_admin_blocked` test invoking
   `unshare --user --map-root-user true` (or `chroot`) and asserting
   non-zero exit + denial signature.
3. **G1b.3 — User-ns elevation payload.** New
   `privesc_user_ns_elevation_blocked` test running `unshare -U -r`
   chained to a privileged probe; asserts non-zero exit + denial
   signature.
4. **G1b.4 — Local verify and push.** `cargo fmt --all -- --check`,
   `cargo clippy --all-targets -- -D warnings`, `cargo check --tests`.
   Commit + push; CI privileged `sandbox-smoke` job is the runtime
   proof.

## Complexity Exceptions

- The G1a allowance for `tests/sandbox_escape_test.rs` up to ~250
  lines is consumed (~265 lines after G1a). G1b adds ~70-90 lines,
  putting the file at ~340-360 lines, still under the ~450-line
  trigger to split by category in G1e. No new exceptions.

---

# Readiness: Open Pincery — v9 Phase G1a (AC-76 Sandbox Escape Suite — FS Category)

> This addendum covers Slice G1a only. It opens AC-76 work (Phase G,
> sandbox escape suite). Slices G1b/G1c/G1d/G1e add the privesc,
> resource, network categories and the AC-53 closure gate; each gets
> its own readiness addendum at the start of its build. Prior G0f
> readiness above remains the authoritative record for AC-88.

## Verdict

READY for Slice G1a / AC-76 (FS category). Builds directly on the
fully-landed AC-83..AC-88 sandbox stack: `RealSandbox` + `pincery-init`
wrapper, AC-86 UID-65534 + cap-drop, AC-85 FullyEnforced landlock,
AC-87 IPC scoping, and AC-88 `landlock_denied` audit-event emission.
This slice ships the test harness and the four filesystem-category
adversarial payloads. Privesc / resource / network categories are
explicitly deferred to G1b..G1d. The strict `sandbox_blocked` event
contract from scope.md AC-76 is decomposed: G1a asserts non-zero
exit (behavioral block) and verifies the kernel-attributed
`landlock_denied` event from AC-88 fires when ABI >= 7 is available;
the synthesized cross-layer `sandbox_blocked` emitter is tracked as
G1e (after all four categories' payloads exist and the layer-attribution
heuristic can be exercised against real evidence from each category).

## Truths

- **T-AC76-G1a-1** Slice G1a ships `tests/sandbox_escape_test.rs` with a
  shared precondition gate (`bwrap` on PATH + landlock supported + ABI
  > = `LANDLOCK_ABI_FLOOR` + `OPEN_PINCERY_SKIP_REAL_BWRAP` unset +
  > `PINCERY_INIT_BIN_PATH` resolved from `CARGO_BIN_EXE_pincery-init`),
  > a shared `escape_profile()` that turns on every defense layer
  > (`deny_net=true`, `seccomp=true`, `landlock=true`), and a shared
  > `assert_payload_blocked` helper. When preconditions are not met the
  > test emits an explicit skip line and returns success.
- **T-AC76-G1a-2** G1a covers the four filesystem-category payloads
  named by AC-76: read `/etc/shadow`, walk `/proc/1/root`, open
  `/dev/sda` for read, attempt a `mount` mount-namespace escape. Each
  payload runs through `RealSandbox::run` in `Enforce` mode and
  asserts `ExecResult::Ok { exit_code, stdout, stderr, .. }` with
  `exit_code != 0` AND a denial signature in stdout/stderr that
  proves the failure is sandbox-attributed (e.g. "Permission denied",
  "Operation not permitted", "No such device or address",
  shell-test exit token). Bare exit-code checks alone are too weak;
  every assertion includes at least one positive denial-signature
  match.
- **T-AC76-G1a-3** G1a does NOT yet emit a synthesized `sandbox_blocked`
  event from runtime code. Where AC-88 already wires the kernel-audit
  bridge for filesystem denials on Linux >= 6.7 (ABI >= 7), the harness
  treats the AC-88 `landlock_denied` event as the _kernel-confirmed_
  evidence for the FS category, but does not require it as a hard
  gate (the live audit reader is host-permission gated and may be
  unreadable in some CI environments). The synthesized cross-layer
  `sandbox_blocked` event with `{tool_call_id, payload_category,
denied_by_layer, syscall?, path?}` is tracked as G1e and lands after
  G1b/c/d so the layer-attribution heuristic can be exercised against
  evidence from every category.
- **T-AC76-G1a-4** G1a does not weaken the AC-84/AC-85 enforcement
  floor. Tests use the production `enforce` sandbox path. Tests do
  not set `OPEN_PINCERY_SANDBOX_FLOOR=relaxed`,
  `OPEN_PINCERY_ALLOW_UNSAFE`, or `OPEN_PINCERY_INIT_FORCE_PARTIAL`.
- **T-AC76-G1a-5** G1a is cross-platform-buildable and Linux-only-runnable.
  The whole test file is `#![cfg(target_os = "linux")]`. Windows
  `cargo check` and CI `cargo build` succeed; Windows `cargo test`
  trivially passes (the file compiles to no tests). Linux test runs
  exercise the suite when bwrap is available; otherwise self-skip
  with explicit evidence.
- **T-AC76-G1a-6** G1a payloads are deterministic on the privileged
  CI `sandbox-smoke` job (Ubuntu 24.04, kernel >= 6.8, bwrap, sudo
  available for `apparmor_restrict_unprivileged_userns=0`). Local
  Docker Desktop devshell still self-skips strict-floor checks
  because Docker Desktop's WSL2 kernel reports Landlock ABI Some(3),
  which is below `LANDLOCK_ABI_FLOOR=6`; deterministic compile and
  Windows `cargo test` always run.
- **T-AC76-G1a-7** G1a binds canonical TLA+ actions
  `ProvisionSandbox`, `ScopeFilesystem`, `BindShellPolicy`, and
  `AttestSandbox`. Tests reference these in their module docstring
  so AC-81 (binding commitments) finds them when it lands.
  `ScopeNetwork` binding lands in G1d.
- **T-AC76-G1a-8** (REMEDIATION 2026-04-29) Sandbox `/etc` exposure
  is narrowed to a public-runtime allowlist
  (`runtime::sandbox::landlock_layer::ETC_ALLOWLIST`) shared by both
  the bwrap bind layer and the Landlock rx-grant layer. The previous
  broad `--ro-bind /etc /etc` plus broad Landlock `/etc` rx grant
  was demonstrated to expose `/etc/shadow` to the sandboxed shell on
  the privileged CI smoke run because user-namespace uid 65534 + DAC
  is not a reliable host-secret boundary. Two Rust unit guards
  (`bwrap_args_do_not_bind_broad_or_sensitive_etc`,
  `default_profile_does_not_grant_broad_etc`) plus their lockstep
  positive twins (`*_include_safe_etc_allowlist`,
  `*_grants_safe_etc_allowlist`) keep both layers aligned and
  fail-closed on regression. The G1a harness was simultaneously
  fixed: shell payloads no longer trail `; echo exit=$?`, which
  previously masked the payload's exit code behind echo's exit 0.

## Key Links

- **L-AC76-G1a-1** [AC-76 FS] -> `tests/sandbox_escape_test.rs`
  precondition gate -> existing `RealSandbox::new(ResolvedSandboxMode
{ Enforce, allow_unsafe: false })` + `pincery-init` wrapper ->
  runtime proof on CI `sandbox-smoke` job that all four FS payloads
  exit non-zero with a denial signature.
- **L-AC76-G1a-2** [AC-76 FS / `cat /etc/shadow`] -> bwrap binds
  only the `ETC_ALLOWLIST` set into the sandbox view of `/etc`
  (T-AC76-G1a-8); `/etc/shadow` is therefore absent and `cat`
  resolves to ENOENT under UID 65534. Defence-in-depth: even if a
  future regression re-exposed the file, AC-86 UID drop (mode 0640
  root:shadow) and Landlock would still need to deny the read.
  Assert exit_code != 0 AND stdout/stderr contains "no such file or
  directory" or "permission denied".
- **L-AC76-G1a-3** [AC-76 FS / `/proc/1/root`] -> bwrap `--proc /proc`
  - AC-86 UID drop + new PID namespace -> `ls /proc/1/root` resolves
    to the in-sandbox pid-ns root, but UID 65534 cannot read pid 1's
    root link -> assert exit_code != 0 AND stderr contains "Permission
    denied" or "No such file or directory".
- **L-AC76-G1a-4** [AC-76 FS / `/dev/sda`] -> bwrap `--dev /dev`
  tmpfs only mounts the safe device subset (null/zero/random/tty),
  so `/dev/sda` does not exist inside the sandbox -> assert
  exit_code != 0 AND stderr contains "No such file or directory" or
  "cannot open".
- **L-AC76-G1a-5** [AC-76 FS / mount-ns break] -> AC-77 denylist
  blocks `mount(2)` via seccomp, AC-86 cap-drop removes
  `CAP_SYS_ADMIN`, and bwrap unshares the mount namespace -> shell
  invocation `mount --bind /etc /mnt 2>&1` fails before any host
  view is reattached -> assert exit_code != 0 AND stderr contains
  "Operation not permitted" or "must be superuser".
- **L-AC76-G1a-6** [AC-76 FS] -> AC-88 landlock audit reader
  observes filesystem denials when ABI >= 7 -> `tests/landlock_audit_test.rs`
  already covers the kernel-audit path; G1a does not duplicate the
  audit assertion but documents it as a deeper proof available on
  ABI >= 7 hosts.

## Acceptance Criteria Coverage

| AC ID | Build Slice                                                                                                                                                                                                                            | Test / Proof                                                                            | Runtime Verification                                                                                                                                                                                              | Status                            |
| ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------- |
| AC-76 | G1a: harness + 4 FS payloads (etc/shadow, /proc/1/root, /dev/sda, mount-ns break). G1b: 3 privesc payloads. G1c: 3 resource payloads. G1d: 2 network payloads. G1e: synthesized `sandbox_blocked` event emission + AC-53 closure gate. | `tests/sandbox_escape_test.rs` (G1a only); G1b..G1e add tests/code in their own slices. | Privileged CI `sandbox-smoke` job runs the suite and fails the build if any payload succeeds. Local Docker Desktop devshell self-skips strict-floor checks. Windows `cargo test` compiles the file to zero tests. | G1a in progress; G1b..G1e queued. |

## Scope Reduction Risks

- **Suite that asserts only `exit_code != 0`** without checking denial
  signature: guarded by required stdout/stderr signature match per
  payload (see Key Links L-AC76-G1a-2 .. L-AC76-G1a-5).
- **Payload that "blocks" because the binary is missing on the test
  host**: guarded by self-skip when `bwrap` is missing, plus payloads
  that use coreutils (`cat`, `ls`, `dd`, `mount`) only — all present
  in the privileged CI image and the devshell image.
- **Suite never runs because preconditions always fail**: guarded by
  CI evidence (the `sandbox-smoke` job runs on every CI) — failure to
  reach the privileged path will surface as "0 tests ran" in CI logs
  and a missing skip line. Local devshell preconditions are
  documented but not required for CI green.
- **Synthesized `sandbox_blocked` event silently dropped**: explicitly
  deferred to G1e, called out here so it cannot be quietly omitted.
  G1e cannot land without all four categories existing first, so the
  attribution heuristic is exercised against real evidence.
- **Floor relaxation creep**: guarded by T-AC76-G1a-4 — tests do not
  set any of the `*_RELAXED` / `*_ALLOW_UNSAFE` / `*_FORCE_PARTIAL`
  knobs.

## Clarifications Needed

- None blocking. The strict reading of AC-76 ("every payload MUST emit
  a `sandbox_blocked` event") is decomposed into `landlock_denied`
  (already emitted by AC-88 for FS denials) plus a synthesized
  `sandbox_blocked` emitted by G1e. This decomposition is in scope of
  AC-76 itself (the AC defines the contract; the slice plan defines
  how it lands) and does not change the pass/fail meaning of the AC.

## Build Order

1. **G1a.1 - Test harness skeleton.** Create
   `tests/sandbox_escape_test.rs` with `#![cfg(target_os = "linux")]`,
   precondition gate (mirrors `sandbox_landlock_test.rs`), profile
   helper, and `assert_payload_blocked` helper.
2. **G1a.2 - FS payload: `cat /etc/shadow`.** Assert exit code != 0
   and stderr contains "Permission denied".
3. **G1a.3 - FS payload: `/proc/1/root` walk.** Assert exit code != 0
   and stderr contains "Permission denied" or "No such file".
4. **G1a.4 - FS payload: `/dev/sda` open.** Assert exit code != 0
   and stderr contains "No such file" or "cannot open".
5. **G1a.5 - FS payload: mount-ns break.** Assert exit code != 0
   and stderr contains "Operation not permitted" or "must be
   superuser".
6. **G1a.6 - Verify locally and push.** `cargo fmt --all --
--check`, `cargo clippy --all-targets -- -D warnings`,
   `cargo check --tests`, then commit + push. CI privileged
   `sandbox-smoke` job is the runtime proof.

## Complexity Exceptions

- `tests/sandbox_escape_test.rs` is allowed up to 250 lines for G1a
  (harness + 4 payloads). G1b..G1d will each add ~50-80 more lines;
  if the file passes ~450 lines after G1d the suite gets split by
  category in G1e. Per scope.md design.md note 2: single-file is
  acceptable given shared harness cost.

---

# Readiness: Open Pincery — v9 Phase G0a (AC-83 `pincery-init` Exec Wrapper)

> This addendum supersedes per-slice readiness for Phase G0a only. It was
> produced at the 2026-04-23 EXPAND-addendum checkpoint (commit `4f82cc9`)
> after the distinguished-engineer sandbox audit. The v7 readiness block
> below is preserved verbatim as historical context.

## Verdict

READY for Slice G0a.

All five open decisions from the audit doc are resolvable in-slice without
blocking (see Clarifications below); none change the pass/fail meaning of
AC-83. The interim landlock disable (commit `4cf7bc9`) has already turned
PR #4 CI green, so G0a lands on a known-good baseline. AC-83 has a single
unambiguous invariant — `pincery-init` runs inside the sandbox after bwrap
mount setup and either installs every restriction and execs the user
binary, or fails closed with `_exit(125)` and a structured JSON error.

## Truths (G0a)

Non-negotiable statements that must be true in the shipped Slice G0a:

- **T-G0a-1** A new binary `pincery-init` exists at `src/bin/pincery_init.rs`
  with a `[[bin]]` entry in `Cargo.toml`. It compiles `panic = "abort"`.
  (Musl static-linking infra is deferred to Slice G0a-followup; G0a ships
  a dynamically-linked binary sufficient to validate the architecture
  inside the devshell image, with musl cross-compile tracked as a follow-up
  before v9.0 release.)
- **T-G0a-2** `RealSandbox::run` in `src/runtime/sandbox/bwrap.rs` no
  longer installs Landlock via a parent-side `pre_exec` hook. The
  `landlock` module's `install_landlock` function is removed from the
  parent-spawn path entirely.
- **T-G0a-3** `build_bwrap_args` adds `--ro-bind <host_init_path> /sandbox/init`
  and rewrites the user argv to `["/sandbox/init", "--policy-fd", "3", "--", original_argv...]`.
  The host init path resolves to the workspace's `pincery-init` binary
  via `std::env::current_exe()`-adjacent lookup (Cargo test binary dir in
  tests; `$CARGO_TARGET_DIR/debug|release/pincery-init` in dev; installed
  location in prod; override via `OPEN_PINCERY_INIT_PATH` for CI/test).
- **T-G0a-4** A serde-serializable `SandboxInitPolicy` struct lives in
  a new shared module (`src/runtime/sandbox/init_policy.rs`) and is the
  only type used to cross the parent→wrapper IPC boundary. Wire format
  is JSON via `serde_json` (already a transitive dep; bincode was
  rejected in Slice G0a.1 after `cargo deny` flagged RUSTSEC-2025-0141
  marking bincode v1 unmaintained, and v2 is a breaking API rewrite
  that would add a new direct dep for one IPC boundary). Shape:
  `{ landlock_rx_paths: Vec<PathBuf>, landlock_rwx_paths: Vec<PathBuf>,
seccomp_bpf: Vec<u8>, target_uid: u32, target_gid: u32,
require_fully_enforced: bool, user_argv: Vec<String> }`.
- **T-G0a-5** Policy IPC uses a memfd (`memfd_create("pincery-init-policy", 0)`,
  **not** CLOEXEC). Parent writes the serde_json-serialized policy
  bytes, `lseek(0)`, passes the fd as `--policy-fd 3` in the user argv
  via bwrap fd inheritance. Wrapper reads the entire fd to EOF then
  closes it. Rejected alternatives documented in the slice summary.
- **T-G0a-6** `pincery-init`'s policy application order is exactly:
  prctl(NO_NEW_PRIVS) → setresgid/setgroups/setresuid → (seccomp BPF)
  → (landlock_restrict_self with LANDLOCK_RESTRICT_SELF_TSYNC) →
  verify `RestrictionStatus::FullyEnforced && no_new_privs == 1` → execvp.
  Any non-zero return from any step: write JSON `{"stage":"...", "errno":N,
"message":"..."}` to fd 3, `_exit(125)`.
- **T-G0a-7** When AC-84's preflight lands, the wrapper also asserts the
  kernel ABI floor at startup. In G0a the wrapper accepts any ABI ≥ 1
  (because the interim production default is `landlock=false`, and the
  positive landlock-on tests are `#[ignore]`d until AC-84 ships). Once
  AC-84 lands the floor becomes ABI ≥ 6.
- **T-G0a-8** Slice G0a does **not** implement AC-84, AC-85, AC-86, AC-87,
  AC-88, or the seccomp allowlist rewrite. Each is its own slice. G0a only
  proves the architectural substrate (wrapper + IPC + in-sandbox install
  - fail-closed exit 125) is sound.

**RECONCILED 2026-04-27:** AC-84 / Slice G0b has since landed. The
server now runs `enforce_kernel_floor_at_startup()` from `src/main.rs`
before config loading, DB bootstrap, or listener bind. The landed floor is
Landlock ABI >= 6 in strict mode, ABI >= 1 only under
`OPEN_PINCERY_SANDBOX_FLOOR=relaxed` plus
`OPEN_PINCERY_ALLOW_UNSAFE=true`, seccomp-bpf, cgroup v2,
`/proc/sys/user/max_user_namespaces > 0` for all callers,
Debian/Ubuntu `unprivileged_userns_clone=1` for non-root callers, and
`bwrap >= 0.8.0`. AC-84 proof is `src/runtime/sandbox/preflight.rs`
unit coverage plus `tests/sandbox_preflight_test.rs`; positive process
tests are intentionally gated by `OPEN_PINCERY_RUN_AC84_POSITIVE=1` in
the privileged `sandbox-smoke` CI job. VERIFY closed on GitHub Actions
run `25021024624`, where the privileged sandbox job ran all four
`sandbox_preflight_test.rs` process tests with the positive-evidence gate enabled.

## Key Links — AC → Design → Test → Proof

- **AC-83** (`pincery-init` exec wrapper):
  - Design components: `src/bin/pincery_init.rs` (new); `src/runtime/sandbox/init_policy.rs` (new); `src/runtime/sandbox/bwrap.rs::build_bwrap_args` (amended); `src/runtime/sandbox/bwrap.rs::RealSandbox::run` (remove `pre_exec` landlock); `Cargo.toml` `[[bin]]` entry.
  - Planned tests: `tests/pincery_init_test.rs` with four cases:
    1. `wrapper_execs_user_binary_cleanly` — bwrap → pincery-init → `echo hello` returns stdout="hello\n", exit=0.
    2. `wrapper_surfaces_policy_apply_failure_as_125` — feed a malformed `LandlockProfile` (e.g. nonexistent path with `HardRequirement`), assert exit code 125 and a parseable JSON error on fd 3.
    3. `wrapper_rejects_partial_enforcement` — stub/feature-flag path that forces `RestrictionStatus::PartiallyEnforced`; assert exit 125 + `not_fully_enforced` stage.
    4. `wrapper_is_invisible_to_user_argv` — run `sh -c 'echo $0 && cat /proc/self/comm && printf %s\\n "$@"' arg1 arg2`; assert `$0` == sh/sandbox, argv has no `pincery-init`, `/proc/self/comm` == `sh`.
  - Runtime proof: `cargo test --test pincery_init_test -- --test-threads=1` green on Linux devshell; `cargo test --test sandbox_real_smoke -- --nocapture` still green (regression guard on the interim fix); `journalctl --user` or stderr capture shows the policy-apply failure path emits JSON when induced.

## Acceptance Criteria Coverage (G0a only)

| AC    | Planned Test                                 | Planned Runtime Proof                                                        |
| ----- | -------------------------------------------- | ---------------------------------------------------------------------------- |
| AC-83 | `tests/pincery_init_test.rs` (4 cases above) | Devshell: bwrap → pincery-init → user cmd, with induced failure showing 125. |

## Scope Reduction Risks

- **Musl static-linking deferred**: G0a ships a dynamically-linked wrapper. Before v9.0 release, the wrapper must be musl-static so it runs inside distroless/minimal sandbox rootfs. Tracked as Slice G0a-followup; marked in `DELIVERY.md` "Known Limitations" on merge. This is an explicit deferral, not a silent drop — the wrapper is still installed via `--ro-bind` from the host, so for G0a the host's dynamic linker reachability into the bwrap rootfs is what matters, and bwrap's default `--ro-bind` of `/usr` + `/lib*` already covers that for the target runners.
- **`RestrictionStatus::PartiallyEnforced` rejection**: T-G0a-6 step 5 requires a stubbable feature flag or environment override to make the test case reproducible without a kernel that actually returns `PartiallyEnforced`. Risk: the implementation fakes this with `#[cfg(test)]` and ships untested in production. Mitigation: the stub is an environment variable (`OPEN_PINCERY_INIT_FORCE_PARTIAL=1`) honored only when `OPEN_PINCERY_ALLOW_UNSAFE=true`; same pattern as the existing `ResolvedSandboxMode` gate. Cited in the test and in the audit doc.
- **Don't re-implement AC-85 inside G0a**: The `FullyEnforced`-or-refuse check is AC-85 territory. G0a should **structure** the check so AC-85 only has to flip a constant, but must not promise `HardRequirement` behavior in G0a itself.
- **Don't re-implement AC-86 inside G0a**: bwrap flags `--uid 65534 --gid 65534 --cap-drop ALL` are AC-86. G0a's wrapper calls `setresuid/setresgid` as defense-in-depth only, and it must not fail if the inherited UID is already 65534.

## Clarifications Needed (in-slice, do not block)

The five open decisions from the audit doc resolve as follows for G0a:

1. **Separate binary vs. argv[0] dispatch**: Separate `[[bin]]` target. Simpler build + clearer in `ps`.
2. **IPC transport**: memfd (T-G0a-5). Rejected pipe (inherits awkwardly), rejected env (policy bytes in env are ugly and size-limited).
3. **Static-musl build infra**: deferred to G0a-followup per Scope Reduction Risks above.
4. **Audit netlink**: deferred to AC-88 / Slice G0f. G0a emits no `landlock_denied` events.
5. **Floor advisory vs. hard**: hard in prod per AC-84 when that slice ships; G0a is floor-agnostic.

## Build Order (G0a internal slicing)

G0a is itself broken into three sub-slices, verified in order:

- **G0a.1** — `SandboxInitPolicy` module + serde_json round-trip unit test. No bwrap integration yet. Proof: `cargo test --lib init_policy`.
- **G0a.2** — `pincery_init` binary that reads a policy fd, logs the parsed policy to stderr, and `execvp`s argv without applying any restrictions yet. Proof: host-level run with a hand-crafted policy fd; observe the user binary runs and the policy bytes were parsed.
- **G0a.3** — Wire `RealSandbox::run` to `--ro-bind` the binary + pass `--policy-fd 3`, and implement the full policy-application pipeline in the wrapper. Remove parent `pre_exec` landlock install. Proof: four-case `tests/pincery_init_test.rs` green; `sandbox_real_smoke` still green.

## Complexity Exceptions

- `src/bin/pincery_init.rs` is permitted to exceed 300 lines if the policy-apply pipeline requires it; the wrapper is a kernel-interface layer where clarity of ordering outweighs file-size discipline. Current target: ≤ 250 lines including doc comments.
- The wrapper may use `libc` directly (as `landlock` and `seccompiler` already do); no new abstraction layer is needed for G0a.

---

# Readiness: Open Pincery — v7 (Credential Vault & Reasoner-Secret Refusal)

> This file supersedes the prior v6 readiness record. v6 is shipped; its
> readiness artifact lives in git history (latest commit on the v6 branch
> before the v7 EXPAND commit `a532996`). v7 covers AC-38 through AC-43
> only — AC-1..AC-37 coverage is verified by the shipped v6 suite and is
> not re-planned here.

## Verdict

READY

v7 is strictly additive: a new AES-256-GCM credential vault module, a new
operator-only REST surface, a new CLI command group that never accepts a
secret via argv, a new `list_credentials` tool gated as `ReadLocal`, a
new prompt-template version with explicit vault redirect, and a new
`PLACEHOLDER:<name>` dispatch handshake that reserves the v9 proxy seam.
No existing AC regresses; no existing row is mutated (two additive
migrations). Every AC has unambiguous pass/fail criteria, a named test
file, and a concrete runtime proof path. Scope adjustments documented in
design.md are bounded and preserve every AC's core invariant.

## Truths

Non-negotiable statements that must be true in the shipped v7 system:

- **T-v7-1** `src/runtime/vault.rs` defines `pub struct Vault` with three
  methods: `from_base64(&str) -> Result<Vault, VaultError>`,
  `seal(&self, workspace_id: Uuid, name: &str, plaintext: &[u8]) -> SealedCredential`,
  and `open(&self, workspace_id: Uuid, name: &str, sealed: &SealedCredential) -> Result<Vec<u8>, VaultError>`.
- **T-v7-2** `Vault::seal` uses AES-256-GCM with a fresh 12-byte
  `OsRng`-sourced nonce per call and AAD bytes
  `format!("{workspace_id}:{name}").into_bytes()`.
- **T-v7-3** `Vault::open` returns `VaultError::Authentication` (never
  panics) on any of: tampered ciphertext, tampered nonce, mismatched
  `(workspace_id, name)`, wrong master key.
- **T-v7-4** Master key is loaded exactly once at startup from
  `OPEN_PINCERY_VAULT_KEY` (base64 → 32 bytes). Missing/malformed key
  fails the process with an actionable error before any HTTP listener
  binds.
- **T-v7-5** Migration `20260420000002_create_credentials.sql` creates
  the `credentials` table with columns
  `(id, workspace_id, name, ciphertext, nonce, created_by, created_at, revoked_at)`,
  `CHECK (length(nonce) = 12)`, `CHECK (length(ciphertext) >= 16)`,
  `CHECK (name ~ '^[a-z0-9_]{1,64}$')`, and a unique partial index on
  `(workspace_id, name) WHERE revoked_at IS NULL`.
- **T-v7-6** `POST /api/workspaces/:id/credentials`,
  `GET /api/workspaces/:id/credentials`, and
  `DELETE /api/workspaces/:id/credentials/:name` require
  `workspace_admin` on the target workspace (or `local_admin`);
  non-admin members receive 403 and an `auth_forbidden`-equivalent
  audit row; non-members receive 404.
- **T-v7-7** `GET /api/workspaces/:id/credentials` response body is a
  JSON array of `{name, created_at, created_by}` only. The response
  never contains `value`, `ciphertext`, `nonce`, or any other byte that
  could reconstruct the sealed secret.
- **T-v7-8** `POST` on a duplicate non-revoked `(workspace_id, name)`
  returns 409 Conflict. `DELETE` sets `revoked_at = NOW()` and a
  subsequent `POST` with the same name succeeds.
- **T-v7-9** `credential_added` / `credential_revoked` / `credential_forbidden`
  rows are appended to `auth_audit` with
  `details JSONB = {workspace_id, name, actor_user_id}`. Value bytes
  never appear in any audit row.
- **T-v7-10** `pcy credential add <name>` has no `--value` clap argument.
  `Cli::try_parse_from(["pcy","credential","add","foo","--value","bar"])`
  returns a clap error.
- **T-v7-11** `pcy credential add` in non-`--stdin` mode calls
  `rpassword::prompt_password(...)` (exactly one call site in
  `src/cli/commands/credential.rs`). In `--stdin` mode it reads from
  stdin and trims trailing newline.
- **T-v7-12** `pcy credential list` prints a two-column `NAME CREATED_AT`
  table populated from the `GET` response. It never prints a value.
  `pcy credential revoke <name>` prompts for confirmation unless `--yes`.
- **T-v7-13** `list_credentials` is registered in `tool_definitions()`
  with an empty `parameters` object. `required_for("list_credentials")`
  maps to `ToolCapability::ReadLocal`, so every `PermissionMode` allows it.
- **T-v7-14** Dispatching `list_credentials` returns a
  `ToolResult::Output` whose body is a JSON array of
  `{name, created_at}` scoped to the calling agent's `workspace_id`,
  filtered `revoked_at IS NULL`. Values / ciphertext / nonces are
  absent. Cross-workspace agents see `[]`.
- **T-v7-15** Migration `20260420000003_prompt_template_credentials.sql`
  sets `wake_system_prompt` v1 to `is_active = FALSE` and inserts a v2
  row with `is_active = TRUE` in a single transaction. v2 template text
  contains literal substrings `pcy credential add`, `REFUSE`, and
  `POST /api/workspaces/:id/credentials`.
- **T-v7-16** The one-active-per-name partial unique index is respected
  by the migration: after it runs, exactly one row with
  `name = 'wake_system_prompt'` has `is_active = TRUE`, and it is v2.
- **T-v7-17** `ShellArgs` has an optional `env: HashMap<String, String>`
  (`#[serde(default)]`). The `shell` tool's JSON-Schema `parameters`
  now declares `env` as an optional object-of-string property.
- **T-v7-18** `tools::dispatch_tool` signature is
  `(tool_call, mode, pool, workspace_id, agent_id, wake_id, executor)`.
  `wake_loop::run_wake_loop` reads `agent.workspace_id` and threads it
  per dispatch.
- **T-v7-19** Before invoking the executor for a `shell` call,
  `dispatch_tool` scans the parsed `env` map. For every value starting
  with literal `PLACEHOLDER:`, it looks up the suffix name in
  `credentials` with `revoked_at IS NULL`. On miss/revoked it appends
  exactly one `credential_unresolved` event
  (`event_type = "credential_unresolved"`, `source = "runtime"`,
  `tool_name = "shell"`, `tool_input` JSON
  `{tool_name, credential_name, reason}`), returns
  `ToolResult::Error(format!("credential not found: {name}"))`, and
  never invokes the executor.
- **T-v7-20** On a placeholder hit, the env value passes through
  unchanged to `ProcessExecutor::run`. v7 performs no substitution; the
  child process observes the literal `PLACEHOLDER:<name>` string. This
  is the seam v9 will fill.
- **T-v7-21** No existing v1–v6 AC regresses: CAS lifecycle (AC-1),
  event log (AC-2), prompt assembly (AC-3) — assembly continues to pass
  against the new active template row, wake loop (AC-4), maintenance
  (AC-5), HTTP API (AC-6), wake triggers (AC-7), stale recovery (AC-8),
  drain (AC-9), bootstrap (AC-10), and every v2..v6 AC are unchanged.
- **T-v7-22** `cargo deny check advisories` continues to exit 0 on v7
  HEAD (AC-37 floor preserved). New dependencies (`aes-gcm`,
  `rpassword`) have no known high/critical advisories as of v7 BUILD.

## Key Links

- **AC-38** → scope.md v7 AC-38 → design.md v7 Vault interface →
  `src/runtime/vault.rs` + `migrations/20260420000002_create_credentials.sql` +
  `src/config.rs` (vault key load) + `src/main.rs` (startup failure
  path) → `tests/vault_roundtrip_test.rs` → runtime proof: 100 sealed
  round-trips with distinct nonces; tamper tests return
  `VaultError::Authentication`.
- **AC-39** → scope.md v7 AC-39 → design.md v7 credentials router →
  `src/api/credentials.rs` + `src/api/mod.rs` (workspace_admin helper) +
  `src/models/credential.rs` → `tests/vault_api_test.rs` → runtime
  proof: admin POST/GET/DELETE succeed; non-admin 403; list response
  JSON scan finds zero secret-value bytes; duplicate active name 409;
  revoke-then-readd succeeds.
- **AC-40** → scope.md v7 AC-40 → design.md v7 CLI interface →
  `src/cli/commands/credential.rs` + `src/cli/mod.rs` + `src/api_client.rs`
  → `tests/cli_credential_test.rs` → runtime proof: clap rejects
  `--value`; stdin round-trip succeeds against an `ApiClient`; static
  grep confirms exactly one `rpassword::prompt_password` call site.
- **AC-41** → scope.md v7 AC-41 → design.md v7 list_credentials →
  `src/runtime/tools.rs` (tool def + dispatch arm) +
  `src/runtime/capability.rs` (ReadLocal classification) →
  `tests/list_credentials_tool_test.rs` → runtime proof: workspace-A
  agent sees 2 non-revoked names; workspace-B agent sees `[]`; response
  bytes contain zero occurrences of any stored value.
- **AC-42** → scope.md v7 AC-42 → design.md v7 prompt template →
  `migrations/20260420000003_prompt_template_credentials.sql` →
  `tests/reasoner_refusal_test.rs` → runtime proof: active row is v2;
  template text contains all three required substrings; v1 row still
  exists with `is_active = FALSE`; AC-3 prompt assembly continues to
  pass.
- **AC-43** → scope.md v7 AC-43 → design.md v7 placeholder envelope →
  `src/runtime/tools.rs` (scan + unresolved event) +
  `src/runtime/sandbox.rs` (env pass-through) + `src/runtime/wake_loop.rs`
  (workspace_id thread) → `tests/placeholder_envelope_test.rs` →
  runtime proof: miss → `credential_unresolved` + zero spawns; hit →
  dispatch proceeds, child env contains literal `PLACEHOLDER:<name>`;
  revoke-then-redispatch → `credential_unresolved`.

## Acceptance Criteria Coverage

| AC    | Planned test                          | Planned runtime proof                                                                                                                                 |
| ----- | ------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC-38 | `tests/vault_roundtrip_test.rs`       | 100 seal/open round-trips with distinct nonces; tamper on ciphertext/nonce/name/workspace/key → `VaultError::Authentication`, no panic                |
| AC-39 | `tests/vault_api_test.rs`             | admin POST/GET/DELETE green-paths; non-admin 403; list JSON byte-scan finds zero secret-value matches; 409 on duplicate; revoke-then-readd            |
| AC-40 | `tests/cli_credential_test.rs`        | `Cli::try_parse_from([..."--value","bar"])` returns clap error; `--stdin` round-trip succeeds against an `ApiClient`; grep finds one `rpassword` site |
| AC-41 | `tests/list_credentials_tool_test.rs` | dispatch returns 2 summaries for WS-A (1 revoked filtered), `[]` for WS-B; output bytes contain zero occurrences of any stored value                  |
| AC-42 | `tests/reasoner_refusal_test.rs`      | active `wake_system_prompt` row = v2; template contains `pcy credential add`, `REFUSE`, `POST /api/workspaces/:id/credentials`; v1 preserved inactive |
| AC-43 | `tests/placeholder_envelope_test.rs`  | missing → `credential_unresolved` + zero spawns; hit → child env contains literal `PLACEHOLDER:<name>`; revoked → `credential_unresolved`             |

## Scope Reduction Risks

- **AC-38 — Vault falls back to a fixed test key in production**:
  Tempting to hardcode a dev key for ergonomics. `main.rs` must fail the
  process on missing/malformed `OPEN_PINCERY_VAULT_KEY` before binding
  HTTP. Lockdown test: a CLI/integration run without the env var fails
  with a specific, actionable error message.
- **AC-38 — AAD dropped "for performance"**: Scope locks AAD =
  `{workspace_id}:{name}`. Without AAD, a sealed row could be swapped
  across names or workspaces without detection.
- **AC-38 — 12-byte nonce reused across seals**: OsRng nonce per seal
  is mandatory. The 100-iteration test asserts unique nonces; a constant
  or counter-based nonce would fail it.
- **AC-39 — List endpoint leaks ciphertext "for debugging"**: Response
  schema is frozen at `{name, created_at, created_by}`. The byte-scan
  test asserts zero occurrences of the stored value in the response.
- **AC-39 — Role gate weakened to "any workspace member"**: Scope locks
  `workspace_admin` (or `local_admin`). Non-admin members must receive
  403 with an audit row.
- **AC-39 — Duplicate name silently upserts**: Scope locks 409 on an
  active duplicate. An upsert path would let a compromised session
  silently replace a secret.
- **AC-40 — `--value` argv flag added "for scripting"**: Scope locks
  stdin/TTY-only input. The argv-rejection test asserts the clap shape.
- **AC-40 — `rpassword` replaced with a raw `readline`**: Raw readline
  echoes the secret to the terminal. Scope locks
  `rpassword::prompt_password` in the interactive branch.
- **AC-41 — Tool returns values "for agent convenience"**: Scope locks
  names-only. The payload byte-scan test asserts zero occurrences of
  any stored value.
- **AC-41 — Cross-workspace leakage**: Tempting to use a looser query
  that joins on `created_by` or similar. Scope locks
  `WHERE workspace_id = $1` exactly; cross-workspace agents must see
  `[]`.
- **AC-42 — Prompt v1 mutated in place instead of versioned**: Scope
  locks immutability. v1 row stays; v2 is a new row with `is_active=TRUE`.
- **AC-42 — "Credential Handling" section omits the redirect**: Scope
  locks the literal `pcy credential add` substring in the template
  text. The test fails closed if any of the three required substrings
  is missing.
- **AC-43 — Silent hit without audit**: On miss/revoked, exactly one
  `credential_unresolved` event is appended. No silent error.
- **AC-43 — v7 attempts real substitution**: Scope locks "no
  substitution in v7". The child-env-contains-`PLACEHOLDER:` assertion
  fails if anything else is passed. Real substitution is v9's job.

## Clarifications Needed

None with BUILD impact. Two design-resolved choices (documented under
`design.md` "Scope Adjustments"):

1. `credential_unresolved` `reason` is unified to `"missing_or_revoked"`
   — single query, bounded test relaxation.
2. Workspace-level audit rows land in `auth_audit` (already exists since
   v2), not in `events` (which is `agent_id`-scoped).

## Build Order

Each slice is sized to ship as 1–2 commits. Independent within reason;
later slices depend only on earlier ones' exported types.

1. **Slice 1 — AC-38 Vault module + migration.** Add `aes-gcm = "0.10"`
   to `Cargo.toml`. Create `src/runtime/vault.rs` with `Vault`,
   `SealedCredential`, `VaultError`, `from_base64`, `seal`, `open`.
   Add migration `20260420000002_create_credentials.sql`. Add
   `vault_key: [u8; 32]` to `Config` and load from
   `OPEN_PINCERY_VAULT_KEY`. Write `tests/vault_roundtrip_test.rs`.
   Update `.env.example` and `docker-compose.yml` to forward the new
   env var.
2. **Slice 2 — AC-39 Credentials API + model.** Create
   `src/models/credential.rs` with `Credential` struct and
   `create`/`list_active`/`find_active`/`revoke` helpers. Create
   `src/api/credentials.rs` with the three handlers. Add
   `require_workspace_admin` helper to `src/api/mod.rs`. Mount the
   router. Write `tests/vault_api_test.rs`. Construct a single `Vault`
   instance in `main.rs` and thread it into `AppState`.
3. **Slice 3 — AC-40 CLI command group.** Add `rpassword = "7"`. Create
   `src/cli/commands/credential.rs`. Add `Credential` subcommand to
   `src/cli/mod.rs`. Extend `src/api_client.rs` with
   `create_credential`/`list_credentials`/`revoke_credential`. Write
   `tests/cli_credential_test.rs`.
4. **Slice 4 — AC-41 `list_credentials` tool.** Add classification arm
   to `src/runtime/capability.rs` (extend `required_for`). Add tool
   definition + dispatch arm in `src/runtime/tools.rs`. Thread
   `workspace_id` through `dispatch_tool` signature + `run_wake_loop`
   (this change is shared with AC-43). Write
   `tests/list_credentials_tool_test.rs`. Update the existing
   `tests/capability_gate_test.rs` unit test to cover the new row.
5. **Slice 5 — AC-42 prompt template v2.** Create migration
   `20260420000003_prompt_template_credentials.sql` that deactivates
   v1 and inserts v2 in a single transaction. Write
   `tests/reasoner_refusal_test.rs`.
6. **Slice 6 — AC-43 placeholder envelope.** Extend `ShellArgs` with
   `env`. Extend `ShellCommand` / `SandboxProfile` to pass env through
   to `ProcessExecutor`. Implement the pre-spawn placeholder scan in
   `dispatch_tool` using `credential::find_active`. Write
   `tests/placeholder_envelope_test.rs`.

After Slice 6: `cargo test --all-targets -- --test-threads=1` + `cargo
clippy --all-targets -- -D warnings` + `cargo fmt --all -- --check` +
`cargo deny check advisories` all pass. Then REVIEW.

## Complexity Exceptions

None. File budgets tracked in `design.md` v7 addendum
("Complexity Exceptions" subsection).

---

## v8 Readiness Addendum — Unified API Surface

> v7 is shipped; AC-38..AC-43 coverage is locked by the v7 suite and not
> re-planned here. v8 covers AC-44 through AC-52 only. v8 is
> surface-only: no schema changes, no runtime-semantic changes to any
> existing handler, no change to the authenticated contract shape.
> Every v1–v7 AC must still pass unchanged after v8 BUILD; regressing
> an older AC is a v8 blocker.

### Verdict

READY

Every AC-44..AC-52 has a named design component, a named test file, a
concrete runtime proof path, and an unambiguous pass/fail assertion.
The four design-time scope adjustments (kubectl JSONPath subset, pinned
MCP `2025-06-18`, Windows-via-WSL for `install.sh`, PUT-ban as lint not
arch) sharpen AC semantics without softening any invariant. No
outstanding clarification would change the pass/fail meaning of any
AC. BUILD may begin.

### Truths

Non-negotiable statements that must be true in the shipped v8 system:

- **T-v8-1** `src/api/openapi.rs` defines a single
  `#[derive(utoipa::OpenApi)] pub struct ApiDoc` whose `paths(...)`
  list contains **every** route registered by `api::router()` plus the
  unauth routes (`/api/bootstrap`, `/api/webhooks/*`). `AC-44` lint
  fails closed if the two enumerations diverge.
- **T-v8-2** `GET /openapi.json` returns a JSON body that parses as
  `openapiv3::OpenAPI` with `openapi == "3.1.0"`, shares the `/health`
  rate-limit bucket, is unauthenticated, and sets
  `Content-Type: application/json`. `GET /openapi.yaml` returns the
  YAML serialization with `Content-Type: application/yaml`.
- **T-v8-3** `pcy login` is idempotent: on a fresh server with
  `OPEN_PINCERY_BOOTSTRAP_TOKEN` set it calls `POST /api/bootstrap`;
  on an already-bootstrapped server it calls `POST /api/login`; on
  either path exit is 0 and stdout is exactly one line matching
  `^Logged in to <context> as <email>$`. **Re-running `pcy login`
  against an already-bootstrapped server never surfaces a `409`.**
- **T-v8-4** The clap root `Cli` exposes the v8 nouns
  (`agent credential budget event context auth api completion mcp
whoami login`) plus hidden shim variants (`bootstrap message events`)
  that emit exactly one `warning:` stderr line via
  `nouns::warn_deprecated` and delegate to the new verb. `--help`
  output lists the new tree; `--help --all` (or equivalent) surfaces
  the hidden shims.
- **T-v8-5** Every verb accepting an agent/credential/budget/event
  target resolves via `src/cli/resolve.rs`: valid UUID → single GET
  confirmation; non-UUID → LIST filtered by exact `name` equality;
  multiple matches → exit 2 with a two-column `ID  NAME` table on
  stderr; zero matches → exit 1 with `not found: <needle>` on stderr.
  **Name matching is never a substring or prefix match.**
- **T-v8-6** Every command that prints structured data accepts
  `--output {table|json|yaml|jsonpath=<expr>|name}`. Default is
  `table` when `io::stdout().is_terminal()`, `json` otherwise.
  `NO_COLOR` suppresses ANSI from `table`. `jsonpath=<expr>` evaluates
  through `jsonpath-rust` covering the kubectl subset
  (`.foo.bar`, `.items[*].name`, `.items[0]`, `[?(@.k==v)]`). `name`
  emits one name per line. `--format` is accepted for one release as a
  deprecated alias that warns once.
- **T-v8-7** `~/.config/open-pincery/config.toml` with v4 flat schema
  is auto-migrated on first v8 load: `src/cli/migrate.rs` writes a
  backup at `config.toml.pre-v8` then rewrites to
  `current-context = "default"` + `[contexts.default]` preserving
  `url`/`token`/`workspace_id`/`user_id`. Migration is idempotent
  (second load is a no-op). Context precedence is `--context` flag >
  `OPEN_PINCERY_CONTEXT` env > file `current-context`.
- **T-v8-8** `pcy mcp serve` is a stdio JSON-RPC server speaking MCP
  revision `2025-06-18` with newline-delimited framing. `tools/list`
  returns one tool per `ApiDoc::openapi()` operation named
  `<tag>.<operation>` (e.g. `agent.create`, `credential.list`) with
  `description` from the operation summary and `inputSchema` from the
  request body + path/query parameters. **The tool list is derived
  from `ApiDoc`, never hard-coded.** `tools/call` proxies through
  `ApiClient` using the active context's token; HTTP failures map to
  the fixed error-code table (`-32001` unreachable, `-32002`
  unauthorized, `-32003` rate-limited, `-32004` not-found, `-32000`
  generic). stdout carries only JSON-RPC; debug and framing errors go
  to stderr.
- **T-v8-9** `install.sh` at the repo root, when piped to `bash`,
  detects OS+arch via `uname`, resolves the release tag, downloads the
  matching asset and its `.sha256`, **enforces sha256**, and attempts
  cosign verification. Without `cosign` on `PATH` it prints a
  `warning:` line and proceeds; with `--require-cosign` it exits
  non-zero. `shellcheck -S warning` is clean. sha256 mismatch **always**
  exits non-zero and refuses to install the asset.
- **T-v8-10** `pcy completion {bash|zsh|fish|powershell}` emits a
  non-empty completion script via `clap_complete` containing a
  shell-specific marker (`_pcy`/`#compdef`/`complete -c pcy`/
  `Register-ArgumentCompleter`). README documents the one-line install
  for each shell.
- **T-v8-11** `tests/api_naming_test.rs` walks `ApiDoc::openapi()` at
  test time and asserts: every collection path segment is plural; every
  primary-key path parameter is named `{id}` (explicit allowlist for
  compound keys); every operation has a non-empty summary ≤ 72 chars
  ending without a period; no `PUT` method appears (allowlist is
  empty at v8 ship); no schema uses `format: "uuid-v7"` (only
  `format: "uuid"`). `tests/cli_naming_test.rs` walks the clap
  `Command` tree and asserts: every command/subcommand has a non-empty
  `about`; every leaf that prints data exposes `--output`; no leaf
  exposes `--format` except behind the hidden deprecated alias; no leaf
  uses `--yes` (only `--force`).
- **T-v8-12** `scripts/demo.sh` replaces the former `pcy demo`
  subcommand. `pcy demo` is deleted (not hidden). The smoke script in
  `scripts/smoke.sh` + `scripts/smoke.ps1` invokes `pcy login` and
  asserts `/openapi.json` returns 200.
- **T-v8-13** v1–v7 acceptance criteria remain green after v8 BUILD.
  `cargo test --all-targets -- --test-threads=1`,
  `cargo clippy --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, and `cargo deny check advisories` all
  pass at the post-BUILD gate.

### Key Links

- **AC-44** → scope.md v8 AC-44 → `src/api/openapi.rs` (`ApiDoc`,
  `openapi_router`) + `#[utoipa::path]` annotations on every handler
  in `src/api/{agents,credentials,me,events,messages,webhooks,
bootstrap}.rs` + `src/api/mod.rs` (mount on unauth router) →
  `tests/openapi_spec_test.rs` → runtime proof: in-process
  `api::router()` spin-up; `GET /openapi.json` parses as
  `openapiv3::OpenAPI`; path enumeration diff vs `router()` is empty;
  `Content-Type` is `application/json`; rate-limit bucket is the
  `/health` bucket; YAML variant returns the same document.
- **AC-45** → scope.md v8 AC-45 → `src/cli/commands/login.rs`
  (`run_with_bootstrap`, bootstrap-or-login branch) + `src/cli/mod.rs`
  (sole `Login` variant; no `Bootstrap` variant) →
  `tests/cli_login_idempotent_test.rs` → runtime proof:
  docker-compose fresh reset + `pcy login --bootstrap-token $T` →
  exit 0 with `already_bootstrapped:false`; second `pcy login
--bootstrap-token $T` against same server → exit 0 with
  `already_bootstrapped:true` and **no 409**; `pcy --help` does not
  list `bootstrap` (matches `gh auth login` / `oc login` ergonomic).
- **AC-46** → scope.md v8 AC-46 → `src/cli/mod.rs` (v8 `Commands`
  enum) + `src/cli/nouns/{agent,credential,budget,event,context,auth,
completion,mcp}.rs` + `src/cli/resolve.rs` + `src/cli/commands/mod.rs`
  (shim delegates) → `tests/cli_noun_verb_test.rs` → runtime proof:
  parameterized `(legacy_cmd, new_cmd)` pairs produce byte-identical
  stdout against a common fixture; ambiguous-name case exits 2 with
  two-column `ID  NAME` table on stderr; not-found exits 1; UUID path
  works; hidden shims each emit exactly one deprecation warning.
- **AC-47** → scope.md v8 AC-47 → `src/cli/output.rs` (`OutputFormat`,
  `TableRow`, `render`, `default_for_tty`) + per-noun `TableRow`
  impls in `src/cli/nouns/*` + root `Cli` gains `--output` flag →
  `tests/cli_output_flag_test.rs` → runtime proof: `--output json`
  parses as JSON; `--output yaml` parses as YAML; `--output name`
  emits one name per line; `--output jsonpath='{.items[*].name}'`
  filters correctly over fixture data; PTY fixture confirms TTY
  default is `table` and pipe default is `json`; `NO_COLOR=1`
  suppresses ANSI in `table`; `--format json` warns once then behaves
  as `--output json`; `--yes` warns once then behaves as `--force`.
- **AC-48** → scope.md v8 AC-48 → `src/cli/config.rs` (v8
  `ContextConfig`/`CliConfig`) + `src/cli/migrate.rs` +
  `src/cli/nouns/context.rs` (list/current/use/set/delete) + root
  `Cli` gains `--context` flag → `tests/cli_context_test.rs` →
  runtime proof: v4 flat fixture file on disk → `pcy context list`
  migrates in place, writes `config.toml.pre-v8` backup, idempotent
  on re-run; two-context file → `pcy context use prod` flips
  `current-context`; `--context prod` flag overrides env; env
  overrides file; `pcy whoami` against a context with a bad token
  exits non-zero; atomic save (tempfile + rename) verified by
  mid-write crash simulation.
- **AC-49** → scope.md v8 AC-49 → `src/mcp/mod.rs` (`run_stdio` event
  loop) + `src/mcp/protocol.rs` (`JsonRpcRequest`/`Response`/`Tool`/
  `CallToolResult`) + `src/mcp/tools.rs` (`OpenApiToolRegistry`) +
  `src/mcp/bridge.rs` (tool → HTTP) + `src/cli/nouns/mcp.rs` (`serve`
  verb) + `src/lib.rs` (`pub mod mcp`) → `tests/mcp_smoke_test.rs` →
  runtime proof: spawn `pcy mcp serve` subprocess with a running
  compose; `initialize` returns `serverInfo`/`capabilities`;
  `tools/list` diff against `ApiDoc::openapi()` operations is empty
  (not hard-coded); `tools/call name="agent.create"` creates an agent
  and the corresponding `agent_created` row lands in `events`; stdout
  carries only JSON-RPC (no stray log bytes); framing error on stdin
  is logged to stderr, not stdout.
- **AC-50** → scope.md v8 AC-50 → `install.sh` at repo root +
  `tests/installer_test.rs` (behind `#[cfg(feature = "installer-e2e")]`)
  - `docs/runbooks/cli-install.md` → runtime proof: `bash -n
install.sh` clean; `shellcheck -S warning install.sh` clean;
    local-fixture GitHub mirror drives end-to-end install; sha256
    mismatch exits non-zero and leaves no binary installed; cosign
    absent + `--require-cosign` exits non-zero; cosign absent + default
    prints `warning:` and installs; cosign present with bad signature
    exits non-zero.
- **AC-51** → scope.md v8 AC-51 → `src/cli/nouns/completion.rs`
  (clap_complete dispatch) + `Cargo.toml` (`clap_complete` dev-dep or
  runtime dep per design) + README + `docs/runbooks/cli-install.md` →
  `tests/cli_completion_test.rs` → runtime proof: `pcy completion
bash` exits 0 with non-empty stdout containing `_pcy`; zsh output
  contains `#compdef`; fish contains `complete -c pcy`; powershell
  contains `Register-ArgumentCompleter`.
- **AC-52** → scope.md v8 AC-52 → `tests/api_naming_test.rs` (AC-52a,
  walks `ApiDoc::openapi()`) + `tests/cli_naming_test.rs` (AC-52b,
  walks clap `Command` tree) → runtime proof: every collection
  segment plural; every primary-key param is `{id}` (allowlist
  empty); every operation summary non-empty, ≤ 72 chars, no trailing
  period; no `PUT` appears; no `format: "uuid-v7"` schemas; every
  clap command has `about`; every data-printing leaf exposes
  `--output`; `--format` only under the deprecated alias; `--yes`
  absent outside deprecation.

### Acceptance Criteria Coverage

| AC     | Planned Test                              | Planned Runtime Verification                                                                                                                                   | Status  |
| ------ | ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- |
| AC-44  | `tests/openapi_spec_test.rs`              | `/openapi.json` returns 200 + parses as `openapiv3::OpenAPI`; path diff vs `api::router()` is empty; YAML variant parses; unauth + `/health` rate-limit bucket | Planned |
| AC-45  | `tests/cli_login_idempotent_test.rs`      | Fresh compose + `pcy login` × 2 both exit 0 (no 409); `bootstrap` alias emits one warn; `--help` excludes `bootstrap`; stdout regex match                      | Planned |
| AC-46  | `tests/cli_noun_verb_test.rs`             | Parameterized (legacy, new) pairs → byte-identical stdout; ambiguous-name exit 2 + stderr table; not-found exit 1; UUID path works                             | Planned |
| AC-47  | `tests/cli_output_flag_test.rs`           | json/yaml/name/jsonpath all parse; PTY fixture confirms TTY default table, pipe default json; NO_COLOR suppresses ANSI; `--format`/`--yes` warn                | Planned |
| AC-48  | `tests/cli_context_test.rs`               | v4 flat → v8 migration writes `.pre-v8` backup, idempotent; `use` switches `current-context`; flag > env > file precedence; atomic save                        | Planned |
| AC-49  | `tests/mcp_smoke_test.rs`                 | `pcy mcp serve` subprocess: `initialize` + `tools/list` diff vs `ApiDoc` empty + `tools/call agent.create` → event lands server-side                           | Planned |
| AC-50  | `tests/installer_test.rs` (feature-gated) | `bash -n` + `shellcheck -S warning` clean; fixture-served install succeeds; sha256 mismatch + cosign-required both exit non-zero                               | Planned |
| AC-51  | `tests/cli_completion_test.rs`            | Four shells each exit 0 with non-empty stdout containing shell-specific marker (`_pcy`/`#compdef`/`complete -c pcy`/`Register-Argument…`)                      | Planned |
| AC-52a | `tests/api_naming_test.rs`                | `ApiDoc::openapi()` walk: plural collection paths, `{id}` params, summaries ≤72 no-period, no PUT, no `format:"uuid-v7"`                                       | Planned |
| AC-52b | `tests/cli_naming_test.rs`                | clap `Command` walk: every command has `about`; every data leaf exposes `--output`; `--format`/`--yes` absent outside deprecated shim                          | Planned |

### Scope Reduction Risks

Concrete places BUILD may be tempted to ship a shell/placeholder. Each
is locked by a named assertion in the coverage table above.

- **AC-44 — utoipa annotations skipped on "obvious" endpoints.** Tempting
  to annotate only new routes. `openapi_spec_test.rs`'s path-diff
  assertion fails closed if any route in `api::router()` is absent from
  `ApiDoc::paths(...)`. Webhooks and bootstrap are in scope.
- **AC-44 — `/openapi.json` returns a hand-maintained JSON file.** The
  source of truth must be `ApiDoc::openapi()` serialized at request
  time (or once at startup, cached). A checked-in JSON would drift.
  Test asserts the served document equals `ApiDoc::openapi()` exactly
  after canonicalization.
- **AC-45 — `pcy login` 409s on re-run.** Tempting to just call
  `/api/bootstrap` unconditionally. Scope locks: first call probes (or
  handles `409` by falling through to `/api/login`). Re-run must exit
  0, not "already bootstrapped" non-zero.
- **AC-46 — name-or-UUID resolver only handles UUIDs.** Falling back
  to "not found" for a valid name would silently break operator
  muscle memory. Scope locks: non-UUID input triggers a LIST filtered
  by exact name; ambiguity and zero-match have distinct exit codes
  (2 vs 1). Substring match is **explicitly forbidden**.
- **AC-46 — legacy shim commands become no-ops or error.** Shims must
  delegate and warn once. The parameterized (legacy, new) byte-equal
  test would fail if the shim printed nothing.
- **AC-47 — `--output table` falls through to JSON.** Tempting to
  defer `TableRow` impls ("we have JSON, ship it"). The PTY fixture
  test asserts `table` output structure; absence of headers or a
  JSON object on stdout fails it.
- **AC-47 — `jsonpath` silently accepts unsupported expressions.**
  Scope locks the kubectl subset; unsupported syntax must exit
  non-zero with a specific error, not silently return `[]` or the
  whole document.
- **AC-48 — context migration deferred to a manual command.** Scope
  locks **automatic** migration on first v8 load with backup written
  to `config.toml.pre-v8`. A "`pcy context migrate`" subcommand is
  not a substitute. Migration is idempotent.
- **AC-49 — MCP `tools/list` returns a hard-coded list.** Scope locks
  derivation from `ApiDoc::openapi()`. Smoke test diffs the tool-name
  set against the operation set; any manual list drifts the moment
  a new handler lands.
- **AC-49 — `tools/call` proxies via a shell-out to `pcy` instead of
  `ApiClient`.** Scope locks direct HTTP via `src/mcp/bridge.rs`.
  Shelling out would reparse JSON, double-log, and lose typed errors.
- **AC-50 — `install.sh` skips cosign verification silently when the
  binary is absent.** Scope locks a visible `warning:` stderr line on
  soft-fail and a hard exit under `--require-cosign`. A silent skip
  would make the signing pipeline theater.
- **AC-50 — sha256 mismatch warns and installs anyway.** Scope locks
  non-zero exit with no binary installed on mismatch. Checksum is
  mandatory; cosign is the optional second factor.
- **AC-51 — completion scripts generated but never tested for
  correctness.** Marker-string assertions per shell are the minimum;
  empty stdout or generic stub fails the test.
- **AC-52 — lint tests allowlist every existing violation at ship.**
  Scope locks a clean run: the allowlists for `{id}` compound keys,
  PUT methods, and `--format` usages are **empty** at v8 ship. Any
  future exception requires a justification comment in the allowlist,
  reviewed at REVIEW time.
- **v1–v7 regression risk.** Annotating existing handlers and
  restructuring the CLI tree both touch code the v1–v7 suite
  exercises. BUILD must rerun the full test suite per slice; a slice
  that passes its own new test but breaks an older test is not done.

### Clarifications Needed

None with BUILD impact. The four design-time resolutions below are
bounded and do not change pass/fail for any AC:

1. **AC-47 `jsonpath` is a kubectl-compatible subset**
   (`.foo.bar`, `.items[*].name`, `.items[0]`, `[?(@.k==v)]`) via
   `jsonpath-rust`. Full JQ is reachable via `-o json | jq`. Test
   fixtures only assert the documented subset.
2. **AC-49 MCP spec version is pinned to `2025-06-18`** for v8 ship.
   Version constant + `initialize` response are the only change points
   for a later revision bump.
3. **AC-50 `install.sh` on Windows is supported via git-bash / WSL
   only.** Native PowerShell installer is deferred (`winget` is the
   right seam and lands with the deferred package-manager track).
4. **AC-52 "no `PUT`" is a lint, not an architectural ban.** Allowlist
   is empty at v8 ship; future exceptions are a one-line addition
   with justification comment.

### Build Order

Slices are sized to ship as 1–3 commits each. Dependencies flow top to
bottom; each slice's tests must pass before the next begins, and the
full v1–v7 suite must remain green at every checkpoint.

1. **Slice 1 — AC-44 OpenAPI foundation.** Add `utoipa` + `utoipa-axum`
   to `Cargo.toml`. Create `src/api/openapi.rs` with `ApiDoc` +
   `openapi_router()` + `openapi_json`/`openapi_yaml` handlers + the
   `BearerAuthAddon` security modifier. Add `#[utoipa::path]` on every
   handler in `src/api/{me,agents,credentials,events,messages,
webhooks,bootstrap}.rs` and `#[derive(ToSchema)]` on every DTO.
   Mount `openapi_router()` on the unauth side in `src/api/mod.rs`.
   Write `tests/openapi_spec_test.rs` (spec served, 3.1 parses, route
   diff empty, Content-Type correct, rate-limit shared with `/health`).
   **Unblocks AC-49 and AC-52a.**
2. **Slice 2 — AC-46 CLI restructure + AC-48 contexts + AC-47 output
   flag.** These three land together because they share the root
   `Cli` struct surgery. Create `src/cli/nouns/` (mod.rs +
   agent/credential/budget/event/context/auth/completion/mcp) by
   moving the current command bodies, keeping thin shim variants in
   `src/cli/commands/mod.rs` (`bootstrap_shim`, `message_shim`,
   `events_shim`) that call `warn_deprecated` + delegate. Rewrite
   `src/cli/config.rs` with v8 `ContextConfig`/`CliConfig` and
   `src/cli/migrate.rs` auto-migration + atomic save. Add
   `--context`/`--output` to the root `Cli`. Create `src/cli/output.rs`
   (enum + `TableRow` trait + `render` + `default_for_tty`) and
   per-noun `TableRow` impls. Create `src/cli/resolve.rs` with
   `resolve_agent`/`resolve_credential`/`resolve_event` covering UUID,
   exact-name, ambiguous, not-found. Write
   `tests/cli_noun_verb_test.rs`, `tests/cli_context_test.rs`,
   `tests/cli_output_flag_test.rs`. Keep `src/cli/output.rs` ≤ 250
   lines; push overflow into noun modules.
3. **Slice 3 — AC-45 idempotent login.** Implement `src/cli/nouns/
auth.rs::login` with the bootstrap-or-login decision tree (probe
   `/api/me`, fall through to `/api/bootstrap` on 401 "not
   bootstrapped", fall through to `/api/login` on 409 "already
   bootstrapped"), persist token into active context. Wire
   `bootstrap_shim` to delegate to `login` with one warning. Write
   `tests/cli_login_idempotent_test.rs` (compose fresh + login × 2,
   alias warning count, `--help` exclusion). **Depends on Slice 2
   context storage.**
4. **Slice 4 — AC-49 MCP server.** Create `src/mcp/mod.rs` (stdio event
   loop, `run_stdio`), `src/mcp/protocol.rs` (JsonRpc types +
   newline-delimited framing), `src/mcp/tools.rs` (`OpenApiToolRegistry`
   reading `ApiDoc::openapi()`), `src/mcp/bridge.rs` (tool → HTTP via
   `ApiClient`, error-code table). Wire `src/cli/nouns/mcp.rs::serve`.
   Add `pub mod mcp` to `src/lib.rs`. Write `tests/mcp_smoke_test.rs`
   (subprocess spawn, initialize, tools/list diff vs `ApiDoc`,
   tools/call agent.create → server-side event lands). Keep
   `src/mcp/mod.rs` ≤ 300 lines; beyond that split into `event_loop.rs`
   - `dispatch.rs`. **Depends on Slice 1 (ApiDoc) and Slice 2 (active
     context).**
5. **Slice 5 — AC-50 installer + AC-51 completions.** Finalize
   `install.sh` at repo root (platform detect, release resolve,
   sha256 enforce, cosign verify with soft/hard fail modes, install
   to `$PCY_PREFIX/bin`). Move the former `pcy demo` flow into
   `scripts/demo.sh` and delete `pcy demo`. Implement
   `src/cli/nouns/completion.rs` using `clap_complete`. Add the
   `installer-e2e` feature to `Cargo.toml`. Write
   `tests/installer_test.rs` (feature-gated) with `bash -n` +
   shellcheck + fixture GitHub mirror + sha256 mismatch + cosign
   required gate. Write `tests/cli_completion_test.rs` (four shells
   × marker string). Update README + create
   `docs/runbooks/cli-install.md` and `docs/runbooks/mcp-setup.md`.
   **Independent of Slices 3–4; may overlap if capacity permits.**
6. **Slice 6 — AC-52 lint guardrails.** Last because they audit
   everything that came before. Write `tests/api_naming_test.rs`
   (walks `ApiDoc::openapi()`) and `tests/cli_naming_test.rs` (walks
   clap `Command` tree) with empty allowlists at v8 ship. Fix any
   violations surfaced in Slices 1–5 (expected small: rename any
   `{agentId}` → `{id}`, trim summaries > 72 chars, convert any
   lingering `PUT` → `POST`/`PATCH`). Update the smoke script to
   hit `/openapi.json`.

**Post-Slice-6 gate**: `cargo test --all-targets --
--test-threads=1` + `cargo clippy --all-targets -- -D warnings` +
`cargo fmt --all -- --check` + `cargo deny check advisories` all
pass; the full v1–v7 AC suite is still green; then REVIEW.

### Complexity Exceptions

Carried forward from `design.md` v8 addendum — four bounded
exceptions, each with a hard ceiling and a predefined split plan.

1. **`src/mcp/mod.rs` may exceed the 200-line soft target.** JSON-RPC
   stdio event loops (framing + dispatch + error mapping + graceful
   shutdown) are irreducible below that threshold. **Hard ceiling
   300 lines**; beyond that split into `event_loop.rs` + `dispatch.rs`.
2. **`src/cli/output.rs` hosts the enum + `render` + `TableRow` impls
   for the common cases.** **Hard ceiling 250 lines**; beyond that
   push per-resource `TableRow` impls into their noun modules.
3. **Legacy-shim compatibility duplicates some test paths** (hidden
   `bootstrap`/`message`/`events` commands, `--format`/`--yes` flag
   aliases). Accepted for one release; removed in v1.2.0 along with
   the duplicate tests.
4. **`utoipa::path` annotations above every handler are verbose.**
   Accepted — they are the source of truth for AC-44 (machine-readable
   contract) and AC-52a (schema-layer lints).

No other complexity exceptions beyond the v1–v7 exceptions already
recorded. No new soft-ceiling extensions. No deferred file budgets.

---

## v9 ANALYZE — Trust Gate Readiness (2026-04-22)

**Verdict: READY.** Scope v9 (AC-53..AC-75, 23 ACs) and design.md v9 DESIGN section are consistent. All four scope clarifications are resolved in writing, and the audit addendum adds AC-73/74/75 plus a risk register. Every AC has a named test file and a runtime proof path. Build order is sequenced so each slice gates the next.

### Truths (non-negotiable statements that must be true in shipped v9.0)

1. **No tool call on Linux reaches `execve` without passing through all six sandbox layers** (bwrap, cgroup v2, landlock, seccomp allowlist, cap/uid drop, netns+slirp4netns). Any layer missing → `sandbox_unavailable` error, no execution.
2. **Plaintext credentials never reside in the agent process address space.** The secret proxy (`src/runtime/secret_proxy.rs`) is the sole component with vault-key read access.
3. **Every `sqlx` query in `src/api/` flows through `ScopedPool`.** The tenancy lint fails CI on any direct `sqlx::query*` call in handler code.
4. **Cross-workspace reads return HTTP 404, never 403.** Presence leaks are a tenancy bug.
5. **Session tokens expire.** No session survives past `expires_at`; `/api/sessions/refresh` is the only extension path.
6. **Deposit tokens are single-use and expire in 24h.** No reuse, no long-lived secret URLs.
7. **Every P0 AC has an adversarial test.** Happy-path tests alone do not close a P0.
8. **Sandbox bypass requires explicit dual opt-in.** `OPEN_PINCERY_SANDBOX_MODE=disabled` is invalid unless `OPEN_PINCERY_ALLOW_UNSAFE=true` is also set.

### Key Links (AC → design component → test → runtime proof)

| AC    | Title                              | Design component                                                                | Test file                                                                   | Runtime proof                                                                                                                         |
| ----- | ---------------------------------- | ------------------------------------------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | --------- | ---------------- |
| AC-53 | Industry-leading sandbox           | `src/runtime/sandbox/{mod,bwrap,seccomp,landlock,cgroup,netns}.rs`              | `tests/sandbox_escape_test.rs`                                              | 12 adversarial payloads executed live; every failure emits `sandbox_blocked` event visible via `pcy events <agent>`                   |
| AC-54 | SECURITY.md threat model           | `docs/SECURITY.md` + README link                                                | `tests/security_doc_test.rs`                                                | `curl http://localhost:8080/docs/SECURITY.md` or repo view                                                                            |
| AC-55 | Credential request tool            | `src/runtime/tools.rs` + `src/api/credential_requests.rs` + migration           | `tests/credential_request_tool_test.rs`                                     | Agent emits `credential_requested` event; `pcy credential request list` shows row; `deposit_token` absent from event payload          |
| AC-56 | Deposit page                       | `src/api/deposit.rs` + HTML template                                            | `tests/credential_deposit_test.rs`                                          | Open `/deposit/<token>` in browser → form renders; POST → 303; second POST → 410                                                      |
| AC-57 | Credential inbox (CLI + UI)        | `src/cli/commands/credential_request.rs` + `static/views/credential_inbox.html` | `tests/cli_credential_request_test.rs`                                      | `pcy credential request {list,approve,reject}` verbs work against live DB                                                             |
| AC-58 | Session TTL + refresh + revoke     | `src/api/sessions.rs` + migration                                               | `tests/session_ttl_test.rs`                                                 | `curl` with expired token → 401; `POST /api/sessions/refresh` extends; `pcy session revoke` invalidates                               |
| AC-59 | Users + roles                      | `src/api/users.rs` + `src/cli/commands/user.rs` + migration                     | `tests/rbac_test.rs`                                                        | 3 roles × endpoint matrix: viewer blocked from POST; operator blocked from user-mgmt; admin open                                      |
| AC-60 | Auth README rewrite                | `README.md` Authentication section                                              | `tests/readme_auth_section_test.rs`                                         | README grep asserts three-box diagram + token table                                                                                   |
| AC-61 | UI rebuild (HTMX + Pico)           | `static/{js,css,views}/`                                                        | `tests/ui_smoke_test_v9.rs`                                                 | `curl /login`, `/agents`, `/agents/:id`, `/events`, `/budget`, `/credentials/requests` return 200 with CSP header                     |
| AC-62 | Event search + export              | `src/api/events_export.rs`                                                      | `tests/event_search_export_test.rs`                                         | `curl /api/agents/:id/events.jsonl?q=foo&type=tool_call` streams NDJSON                                                               |
| AC-63 | Cost reports                       | `src/api/cost.rs` + `src/cli/commands/cost.rs`                                  | `tests/cost_report_test.rs`                                                 | `pcy cost <agent> --group-by model` renders table; matches `llm_calls` sum                                                            |
| AC-64 | Retention + archive                | `src/background/retention.rs` + `src/cli/commands/events_archive.rs`            | `tests/event_retention_test.rs`                                             | Seed old events; `pcy events archive --older-than 90d`; rows pruned, gzipped JSONL on disk                                            |
| AC-65 | Multi-tenant enforcement           | `src/tenancy.rs` + every `src/api/*.rs` handler                                 | `tests/multi_tenant_isolation_test.rs` + `tests/tenancy_middleware_test.rs` | 5×5 matrix: alpha token on beta IDs returns 404; SQLi probes return 404; lint fails on bare query                                     |
| AC-66 | Tool catalog expansion             | `src/runtime/tools/{http_get,file_read,db_query}.rs`                            | `tests/tool_catalog_test.rs`                                                | Each tool registered; scoping test asserts host/path/SQL enforcement                                                                  |
| AC-67 | Workspace rate limiting            | `src/background/rate_limit.rs`                                                  | `tests/workspace_rate_limit_test.rs`                                        | 601 calls in 60s → 601st delayed 1s + `rate_limit_exceeded` event                                                                     |
| AC-68 | Ollama bullet                      | `README.md` + config loader                                                     | `tests/ollama_config_test.rs`                                               | README grep asserts bullet; config loader parses `host.docker.internal:11434` URL                                                     |
| AC-69 | Version handshake                  | `src/api/version.rs` + CLI version check                                        | `tests/version_handshake_test.rs`                                           | Stubbed v0.8 server vs v0.9 CLI → warning; v0 server vs v1 CLI → exit 3                                                               |
| AC-70 | Terminology lock                   | README opening paragraph                                                        | `tests/terminology_test.rs`                                                 | Regex assertion over README/DELIVERY/docs asserts no `bot                                                                             | assistant | worker` synonyms |
| AC-71 | Secret injection proxy             | `src/runtime/secret_proxy.rs` + IPC contract                                    | `tests/secret_proxy_test.rs`                                                | Agent memory via `/proc/<pid>/maps` sweep shows no credential bytes; sandboxed child sees value; `secret_injected` event emitted      |
| AC-72 | Per-agent network egress allowlist | `src/runtime/sandbox/netns.rs` + migration + CLI                                | `tests/network_egress_test.rs`                                              | Allowed host `curl` succeeds; denied host blocked + `network_blocked` event in log                                                    |
| AC-73 | Sandbox mode flag                  | `src/config.rs` + `src/runtime/sandbox/mod.rs`                                  | `tests/sandbox_mode_test.rs` + `tests/sandbox_perf_test.rs`                 | `enforce` blocks, `audit` emits `sandbox_would_block`, `disabled` requires `OPEN_PINCERY_ALLOW_UNSAFE=true`; startup self-test passes |
| AC-74 | Credential plaintext hygiene       | `src/observability/redaction.rs` + `src/runtime/secret_proxy.rs`                | `tests/credential_hygiene_test.rs`                                          | Logs redact credential-shaped values; event insert rejects plaintext; dropped buffers zeroized and `mlock`ed                          |
| AC-75 | Cross-platform dev environment     | `Dockerfile.devshell` + `scripts/devshell.{sh,ps1}` + runbooks                  | `tests/devshell_parity_test.rs`                                             | Mac/Windows contributors run `devshell cargo test`; parity test matches Linux verdict                                                 |

### Acceptance Criteria Coverage

Every AC in scope v9 appears in the table above with a planned test and a planned runtime proof. No AC is closed by a unit test alone; every P0 AC is closed by adversarial test + observable event.

### Scope Reduction Risks

1. **AC-53 landlock may be skipped if kernel < 5.13.** Mitigation: CI runs on ubuntu-24.04 (kernel 6.8+); docs document a minimum kernel floor for self-hosters. Scope reduction risk: ZERO on CI; self-hoster risk mitigated by explicit warning event.
2. **AC-65 middleware migration is one large slice.** Temptation: migrate half the endpoints, leave the rest. Guardrail: lint test blocks merge with any unscoped query remaining. REVIEW must confirm lint is active before slice merges.
3. **AC-71 injection-mode `HttpHeader` requires changes in `http_get` tool at the same time.** Risk: shipping secret proxy without the `http_get` integration leaves a half-feature. Slice A2c MUST include `http_get` cutover.
4. **AC-61 UI rebuild temptation to keep hand-rolled hash-routing as a fallback.** Guardrail: `static/js/` is wholesale replaced, not layered.
5. **AC-66 `db_query` read-only enforcement via server-side regex.** Risk: regex is bypassable via `;` stacking or comment-terminated statements. Mitigation: use a read-only role at the Postgres level (`SET TRANSACTION READ ONLY`) as defense-in-depth, regex is belt-and-suspenders.

### Clarifications Needed

None. All four original clarifications were resolved by user on 2026-04-22 and recorded verbatim in `scope.md` under "Clarifications Resolved."

### Build Order

Sequenced per scope.md Build Order. Summary:

- **Phase A** (Security Truth, ~4-5 weeks): A0 devshell → A1 SECURITY.md → A2a sandbox core + mode flag → A2b egress allowlist → A2c secret proxy + hygiene → A3 session TTL → A4 users+roles → A5 auth README
- **Phase B** (Credential Requests, ~1 week): B1 tool+schema → B2 deposit page → B3 CLI+UI inbox
- **Phase C** (UI Rebuild, ~1 week): C1 HTMX+Pico six views
- **Phase E** (Multi-tenant Enforcement, ~2 weeks — blocking v9.0): E1a schema → E1b middleware → E1c endpoint migration → E1d isolation matrix test
- **v9.0 ships here** (Phases A+B+C+E complete = full trust gate)
- **Phase D** (Observability, ~1 week, ships as v9.1): D1 search+export → D2 cost reports → D3 retention+archive
- **Phase F** (Polish, ~1 week, ships as v9.2): F1 tool catalog → F2 rate limit → F3 Ollama → F4 version handshake → F5 terminology lock

Total engineering budget: 8-10 weeks.

### Complexity Exceptions (carried from DESIGN)

1. `src/runtime/sandbox/mod.rs` budget 400 lines (compose + partial-failure cleanup).
2. `tests/sandbox_escape_test.rs` ~500 lines acceptable.
3. AC-65 endpoint-migration slice touches ~25 files at once — required, not optional.
4. `src/tenancy.rs::Binds` is a bespoke subset of `sqlx` binds.

All four are explicit and REVIEW-gated; none is a placeholder waiver.

---

## v9 AUDIT ADDENDUM — Risks, Mitigations, Hardening (2026-04-22T11:00Z)

An adversarial audit of the v9 plan surfaced 18 concrete risks. Three warranted new ACs (AC-73 Sandbox Mode Flag, AC-74 Credential Hygiene, AC-75 Cross-Platform Dev Env). The remaining 15 are hardening details internal to existing ACs, documented below with the mitigation embedded in the slice that owns it.

### Risk Register

| #   | Risk                                                                                                                                          | Owning AC / Slice  | Mitigation                                                                                                                                                                                                                                          | Evidence gate                                                                                        |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------- | ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| 1   | CI runner may not support user namespaces / unprivileged bwrap                                                                                | AC-53 / A2a        | CI job installs `bubblewrap slirp4netns uidmap` explicitly; preflight step greps `/proc/sys/kernel/unprivileged_userns_clone == 1`; if missing, sets it via `sudo sysctl -w`.                                                                       | `ci/sandbox-preflight.yml` green on ubuntu-24.04                                                     |
| 2   | Sandbox startup cost regresses tool-call latency past acceptance                                                                              | AC-73 / A2a        | Hard perf budget 300ms p95, 500ms hard fail. Counter `sandbox_exec_duration_ms` emitted per call; CI runs 100 warm tool calls and asserts histogram.                                                                                                | `tests/sandbox_perf_test.rs`                                                                         |
| 3   | `SANDBOX_MODE=disabled` footgun in production                                                                                                 | AC-73              | Requires paired `OPEN_PINCERY_ALLOW_UNSAFE=true`; emits `sandbox_mode_changed` event at startup; stderr warning every 60s while disabled.                                                                                                           | `tests/sandbox_mode_test.rs`                                                                         |
| 4   | HTMX + CSP incompatibility (inline `hx-on:` handlers require `unsafe-inline`)                                                                 | AC-61 / C1         | Use nonce-based CSP: server generates per-response nonce; HTMX 1.9 `htmx.config.inlineScriptNonce`. No `unsafe-inline`, no `unsafe-eval`.                                                                                                           | `tests/ui_smoke_test_v9.rs` asserts CSP header includes nonce, rejects inline without matching nonce |
| 5   | Deposit page is unauthenticated (AC-56) — vulnerable to CSRF + brute force                                                                    | AC-56 / B2         | Form includes a double-submit token derived from the deposit_token; IP-based rate-limit (10 POSTs/min/IP); every attempt (success OR fail) emits `deposit_attempt` event.                                                                           | `tests/credential_deposit_test.rs` + rate-limit assertion                                            |
| 6   | Session cookie flags missing `HttpOnly` / `Secure` / `SameSite`                                                                               | AC-58 / A3         | `Set-Cookie` contract documented in `src/api/sessions.rs`; `tests/session_cookie_flags_test.rs` asserts all three flags.                                                                                                                            | Cookie flags test green                                                                              |
| 7   | Session token comparison timing attack                                                                                                        | AC-58 / A3         | Use `subtle::ConstantTimeEq` for every session-token compare.                                                                                                                                                                                       | Code review checks for `==` on bytes in session lookup                                               |
| 8   | Existing rows have NULL `workspace_id` on upgrade (AC-65 migrations fail)                                                                     | AC-65 / E1a        | Migration `20260501000001_add_workspace_id_to_sessions.sql` CREATEs a "legacy" default workspace if none exists, backfills all existing rows, THEN adds NOT NULL. Rollback note in migration.                                                       | Migration dry-run on v8 snapshot + `tests/upgrade_from_v8_test.rs`                                   |
| 9   | Tenancy lint false positives (health checks, migrations legitimately use raw `sqlx::query`)                                                   | AC-65 / E1b        | Lint allowlist: files matching `src/db/**` or `src/background/startup/**`; else `#[allow(tenancy::unscoped)]` attribute required with a comment explaining why.                                                                                     | `tests/tenancy_middleware_test.rs` exercises both allow and deny paths                               |
| 10  | Concurrent tool calls collide on cgroup / netns names; leaked cgroups from crashed processes accumulate                                       | AC-53 / A2a        | Naming: `pincery-<uuid_v4>`; on startup, sweep `/sys/fs/cgroup/pincery-*` older than server uptime and remove. Drop-guard on `SandboxHandle` cleans up even on panic.                                                                               | `tests/sandbox_concurrency_test.rs` runs 50 parallel tool calls, asserts no leaked cgroups           |
| 11  | `zeroize` is best-effort; compiler may elide writes                                                                                           | AC-74 / A2c        | Use `zeroize` crate (which marks as `volatile`); `SecretBuffer` wraps `Vec<u8>` with `Drop` + `ZeroizeOnDrop` derives; `#[deny(unsafe_code)]` on the module.                                                                                        | `tests/credential_hygiene_test.rs` does post-drop memory grep                                        |
| 12  | Log redaction layer false negatives on secret-shaped values without obvious names                                                             | AC-74 / A2c        | Dual strategy: (a) name-matching (password, token, secret, bearer, api*key) via regex on log-record keys; (b) length+shape heuristic for values matching `sk-[a-zA-Z0-9]{16,}` / `ghp*[a-zA-Z0-9]{36}`/ JWT tri-dot format. Both yield`<REDACTED>`. | `tests/credential_hygiene_test.rs` test matrix of 6 credential shapes                                |
| 13  | New crates (`landlock`, `seccompiler`, `cgroups-rs`, `zeroize`, `subtle`, `slirp4netns-bindings`) may have questionable licensing/maintenance | AC-73 / A2a        | `deny.toml` updated with explicit allowlist + version pins; `cargo deny check licenses bans advisories sources` in CI; maintenance check: last commit within 12 months.                                                                             | `cargo deny check` green; `deny.toml` diff reviewed                                                  |
| 14  | Dev path on Mac/Windows breaks (kernel primitives Linux-only) → contributors can't test sandbox                                               | AC-75 / A0         | `scripts/devshell.sh` + `.ps1` launches pinned Docker image; parity test re-runs sandbox suite inside devshell from a Linux CI host.                                                                                                                | `tests/devshell_parity_test.rs` + manual Mac/Windows walkthrough                                     |
| 15  | Tool-call plaintext survives in kernel page cache / swap                                                                                      | AC-71 / A2c        | `mlock()` the SecretBuffer region via `region::lock`; document in SECURITY.md that swap must be disabled on prod hosts or encrypted.                                                                                                                | Code review + SECURITY.md section "Deployment Hardening"                                             |
| 16  | AC-65 endpoint migration (25 files at once) too large to review safely                                                                        | AC-65 / E1c        | Pre-slice preparatory slice E1b MUST ship the middleware + lint first; E1c then becomes a mechanical migration where every file-edit follows an identical pattern. REVIEW checks the PATTERN once, then samples 5 files.                            | REVIEW comment log in E1c commit                                                                     |
| 17  | No rollback plan if v9.0 production upgrade fails                                                                                             | Pre-v9 / bootstrap | Tag `v8.0.1-pre-v9-baseline` on current `v6-01_implementation` HEAD; v9.0 release notes include "to roll back: `git checkout v8.0.1-pre-v9-baseline && docker compose down && up --build`".                                                         | Tag exists before first v9 build commit                                                              |
| 18  | No canary / staged rollout for self-hosted operators                                                                                          | AC-73 / A2a        | `SANDBOX_MODE=audit` lets operators run a week in log-only mode, see what WOULD be blocked, adjust allowlists, THEN flip to `enforce`. Documented in DELIVERY.md "v9 Upgrade Playbook".                                                             | Upgrade playbook section exists                                                                      |

### Definition-of-Done Matrix (per slice, enforced by REVIEW)

| Check                                                | Mechanism                                                  | Pass condition                                                                                     |
| ---------------------------------------------------- | ---------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Compiles + clippy clean                              | `cargo clippy --all-targets --all-features -- -D warnings` | Exit 0                                                                                             |
| Every new AC has a named test file that passes       | `cargo test`                                               | All targets green                                                                                  |
| Every P0 AC has an adversarial (not happy-path) test | Manual REVIEW                                              | Test file contains negative assertions that could only pass with the feature correctly implemented |
| New event types registered in `src/models/events.rs` | `tests/event_type_lint.rs`                                 | All new event types enumerated                                                                     |
| New CLI verbs in noun-verb tree                      | `tests/cli_naming_lint.rs` (AC-52b)                        | Lint green                                                                                         |
| Migrations are additive + include backfill           | REVIEW + `tests/upgrade_from_v8_test.rs`                   | No destructive DDL, backfill covers all existing rows                                              |
| `CHANGELOG.md` has Phase-tagged entry                | `tests/changelog_test.rs`                                  | Entry exists with AC-IDs                                                                           |
| `deny.toml` + `cargo deny check`                     | CI                                                         | No unreviewed new crates; licenses allowed                                                         |
| `cargo audit`                                        | CI                                                         | No high/critical advisories                                                                        |
| Threat-model impact signed off                       | REVIEW                                                     | If slice touches auth / sandbox / tenancy, reviewer records impact in commit trailer               |
| Rollback tag                                         | Git                                                        | `v9.0.0-phase<X>-slice<N>` tag on slice commit                                                     |

### Pre-v9 Baseline Tag

Before BUILD Slice A0 merges, tag the current HEAD:

```bash
git tag -a v8.0.1-pre-v9-baseline -m "Last v8 commit before v9 BUILD begins. Rollback target."
git push origin v8.0.1-pre-v9-baseline
```

Rollback recipe documented in `docs/runbooks/rollback_to_v8.md` (create in Slice A0).

### Upgraded Verdict

**READY** — with the 3 new ACs (AC-73/74/75) added and the 15 in-slice risks documented. Scope totals: **23 ACs**, **8-10 weeks**. v9.0 ships after Phases A + B + C + E complete.
