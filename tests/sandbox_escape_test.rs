//! AC-76 / Slice G1a — sandbox escape suite, filesystem category.
//!
//! AC-76 ships a 12-payload adversarial matrix across four categories
//! (FS / privesc / resource / network). G1a covers only the four
//! filesystem payloads named by `scaffolding/scope.md`:
//!
//!   1. `cat /etc/shadow` — uid-drop denial inside the sandbox.
//!   2. `ls -la /proc/1/root` — PID-namespace plus uid-drop denial.
//!   3. `dd if=/dev/sda of=/dev/null bs=1 count=1` — bwrap `--dev`
//!      tmpfs exposes no `/dev/sda` device node.
//!   4. `mount --bind /etc /mnt` — seccomp denylist plus cap-drop
//!      block `mount(2)`.
//!
//! G1b extends the suite with the three privesc payloads named by
//! AC-76 in `scaffolding/scope.md`: setuid exec, `CAP_SYS_ADMIN`
//! syscall, and user-namespace elevation. Every privesc test reuses
//! the G1a precondition gate, `escape_profile()`, and
//! `assert_payload_blocked` helper without modification.
//!
//! Each payload runs through `RealSandbox` in `Enforce` mode with every
//! defence layer turned on (`deny_net=true`, `seccomp=true`,
//! `landlock=true`). Every assertion has TWO checks: a non-zero
//! `exit_code` AND a denial signature in stdout/stderr that proves the
//! failure is sandbox-attributed (matching the readiness G1a key links).
//!
//! Binds canonical TLA+ actions `ProvisionSandbox`, `ScopeFilesystem`,
//! `BindShellPolicy`, and `AttestSandbox`. `ScopeNetwork` lands with
//! G1d. The synthesized cross-layer `sandbox_blocked` event is tracked
//! as G1e (see readiness G1a addendum, T-AC76-G1a-3) and lands after
//! all four categories exist so the layer-attribution heuristic is
//! exercised against real evidence from every category.
//!
//! Live runs require Linux + `bwrap` on `$PATH` + Landlock supported
//! at ABI >= `LANDLOCK_ABI_FLOOR`, plus a path to the cargo-built
//! `pincery-init` binary. When any precondition is missing, every
//! test self-skips with an explicit evidence line. The whole file is
//! `#![cfg(target_os = "linux")]`-gated, so Windows/macOS `cargo test`
//! compiles to zero tests.

#![cfg(target_os = "linux")]

use std::time::Duration;

use open_pincery::config::{ResolvedSandboxMode, SandboxMode};
use open_pincery::runtime::sandbox::cgroup::{
    cgroup_v2_writable, probe_memory_max_enforcement, CgroupLimits, MemoryProbeOutcome,
};
use open_pincery::runtime::sandbox::preflight::{KernelProbe, RealKernelProbe, LANDLOCK_ABI_FLOOR};
use open_pincery::runtime::sandbox::{
    bwrap::RealSandbox, ExecResult, SandboxProfile, ShellCommand, ToolExecutor,
};

fn bwrap_available() -> bool {
    if std::env::var_os("OPEN_PINCERY_SKIP_REAL_BWRAP").is_some() {
        return false;
    }
    std::process::Command::new("bwrap")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn landlock_available() -> bool {
    open_pincery::runtime::sandbox::landlock_layer::landlock_supported()
}

fn strict_landlock_floor_available() -> bool {
    RealKernelProbe
        .landlock_abi()
        .map(|abi| abi >= LANDLOCK_ABI_FLOOR)
        .unwrap_or(false)
}

/// Mirror of the gate used by `sandbox_landlock_test.rs`. Every
/// test in this file calls this first so we never run against a
/// degraded posture (relaxed-floor, missing wrapper, missing kernel
/// support). On a missing precondition we print an explicit skip
/// reason so CI logs say *why* a test self-skipped.
fn preconditions_met() -> bool {
    if !bwrap_available() {
        eprintln!("skipping: bwrap not on PATH (OPEN_PINCERY_SKIP_REAL_BWRAP set?)");
        return false;
    }
    if !landlock_available() {
        eprintln!("skipping: kernel does not support landlock (need >= 5.13)");
        return false;
    }
    if !strict_landlock_floor_available() {
        eprintln!("skipping: Landlock ABI below AC-84/AC-85 strict floor {LANDLOCK_ABI_FLOOR}");
        return false;
    }
    // G1c: `escape_profile()` now installs production cgroup v2
    // limits, so the test process must be able to `mkdir` under
    // `/sys/fs/cgroup`. Without this gate, Enforce-mode would
    // fail-closed with `ExecResult::Err`, masking real blocks behind
    // a harness error. Privileged CI satisfies this gate; local
    // dev hosts without delegation will self-skip with this reason.
    if !cgroup_v2_writable() {
        eprintln!("skipping: process cannot mkdir under /sys/fs/cgroup (not root / no delegation)");
        return false;
    }
    // Integration tests get `CARGO_BIN_EXE_pincery-init` from cargo
    // automatically. `--test-threads=1` is enforced by the privileged
    // CI sandbox-smoke job, which makes `set_var` safe here.
    std::env::set_var("PINCERY_INIT_BIN_PATH", env!("CARGO_BIN_EXE_pincery-init"));
    true
}

/// Probe whether the host has delegated the cgroup v2 `memory`
/// controller to children of `/sys/fs/cgroup/`. The cgroup v2
/// controller-delegation contract: a controller can only enforce
/// limits on *children* of a cgroup if that controller is enabled
/// in the cgroup's `cgroup.subtree_control`. Writing to
/// `memory.max` on a child cgroup whose parent does NOT have
/// `+memory` in subtree_control succeeds at the file level (the
/// kernel accepts the write) but the limit is a no-op \u2014 the kernel
/// does not enforce memory accounting for that cgroup.
///
/// `cgroup_v2_writable()` (used as a precondition for all G1c tests)
/// checks only that we can `mkdir` under `/sys/fs/cgroup/`; it does
/// NOT check whether each individual controller is delegated. The
/// memory-balloon test needs this stronger check, since pids.max
/// happens to be delegated in environments where memory.max is
/// not (privileged Docker-in-Docker CI runners are a common case).
///
/// Returns `true` if `memory` appears in the host's
/// `/sys/fs/cgroup/cgroup.subtree_control`. Returns `false` on any
/// I/O error or when the controller is missing — the test then
/// self-skips with an explicit log line rather than producing a
/// false-positive pass.
fn memory_controller_delegated() -> bool {
    match std::fs::read_to_string("/sys/fs/cgroup/cgroup.subtree_control") {
        Ok(contents) => contents.split_whitespace().any(|c| c == "memory"),
        Err(_) => false,
    }
}

fn enforce_sandbox() -> RealSandbox {
    RealSandbox::new(ResolvedSandboxMode {
        mode: SandboxMode::Enforce,
        allow_unsafe: false,
    })
}

/// Production-equivalent profile: every defence layer on. Adversarial
/// tests that succeed under this profile prove a real escape, not a
/// configuration weakness.
///
/// G1c upgrade: cgroup v2 limits now match the AC-53 production
/// posture documented in `scaffolding/scope.md` (`memory.max=512M`,
/// `pids.max=64`). G1a (FS) and G1b (privesc) tests inherit these
/// caps; none of their payloads come anywhere near the limits, so
/// they remain green. The resource-category G1c tests rely on these
/// caps to fail-closed — without them the kernel has no reason to
/// refuse a fork-bomb or a 600 MiB allocation.
fn escape_profile() -> SandboxProfile {
    SandboxProfile {
        env_allowlist: vec!["PATH".into()],
        deny_net: true,
        timeout: Duration::from_secs(10),
        cwd: None,
        cgroup: Some(CgroupLimits {
            memory_max_bytes: Some(512 * 1024 * 1024),
            pids_max: Some(64),
            cpu_max_micros: None,
        }),
        seccomp: true,
        landlock: true,
    }
}

/// Run a payload through the production sandbox and assert that
/// (a) the sandboxed shell exited non-zero, AND (b) at least one of
/// the named denial signatures appears in stdout or stderr.
///
/// Exit-code-only assertions are too weak — a missing binary or a
/// shell syntax error also exits non-zero without proving the sandbox
/// did anything. Requiring a denial signature (e.g. "Permission
/// denied", "Operation not permitted", "No such device") forces the
/// suite to observe an actual block.
async fn assert_payload_blocked(payload_name: &str, command: &str, denial_signatures: &[&str]) {
    let result = enforce_sandbox()
        .run(&ShellCommand::new(command), &escape_profile())
        .await;
    match result {
        ExecResult::Ok {
            exit_code,
            stdout,
            stderr,
            ..
        } => {
            assert_ne!(
                exit_code, 0,
                "[{payload_name}] sandbox payload exited 0 (escape!); stdout={stdout:?} stderr={stderr:?}"
            );
            let combined = format!("{stdout}\n{stderr}");
            let lower = combined.to_ascii_lowercase();
            let matched = denial_signatures
                .iter()
                .any(|sig| lower.contains(&sig.to_ascii_lowercase()));
            assert!(
                matched,
                "[{payload_name}] non-zero exit but no denial signature {denial_signatures:?}; stdout={stdout:?} stderr={stderr:?}"
            );
        }
        // Timeout / Rejected / Err are also non-success outcomes, but
        // they don't prove the sandbox blocked the payload — they
        // could mask a bug in the harness or a missing dependency.
        // Surface them as failures so the suite does not green-by-skip.
        other => panic!("[{payload_name}] expected ExecResult::Ok, got {other:?}"),
    }
}

/// FS-1 / read `/etc/shadow`: after the G1a /etc-narrowing, the
/// sandbox view of `/etc` is a tmpfs containing only the public
/// runtime allowlist (see
/// `runtime::sandbox::landlock_layer::ETC_ALLOWLIST`). `/etc/shadow`
/// is therefore not bind-mounted and resolves to ENOENT. Even if a
/// future regression re-exposed it, AC-86 uid-drop (65534) plus
/// Landlock would still need to deny the read; this test passes when
/// any of those layers (combined) blocks the open.
#[tokio::test]
async fn fs_etc_shadow_read_blocked() {
    if !preconditions_met() {
        return;
    }
    assert_payload_blocked(
        "fs/etc-shadow-read",
        // 2>&1 so coreutils' diagnostic is visible alongside the
        // (empty) stdout. No `; echo exit=$?` -- that would mask
        // cat's non-zero exit behind echo's exit 0. The shell's
        // exit code now reflects the last (and only) command.
        "cat /etc/shadow 2>&1",
        // After narrowing the bind allowlist, the file simply does
        // not exist inside the sandbox (ENOENT). "permission
        // denied" is retained as a defence-in-depth signature in
        // case future hardening puts the file behind DAC + Landlock
        // instead of removing it from the view entirely.
        &["no such file or directory", "permission denied"],
    )
    .await;
}

/// FS-2 / walk `/proc/1/root`: bwrap unshares the PID namespace so
/// pid 1 inside the sandbox is the wrapper. UID 65534 (AC-86) cannot
/// dereference the `/proc/<pid>/root` symlink for a process it does
/// not own, so the read fails with EACCES.
#[tokio::test]
async fn fs_proc_1_root_walk_blocked() {
    if !preconditions_met() {
        return;
    }
    assert_payload_blocked(
        "fs/proc-1-root-walk",
        "ls -la /proc/1/root 2>&1",
        &["permission denied", "no such file or directory"],
    )
    .await;
}

/// FS-3 / open `/dev/sda`: bwrap mounts a tmpfs at `/dev` containing
/// only the safe device subset (null/zero/random/urandom/tty); raw
/// disk nodes such as `/dev/sda` simply do not exist inside the
/// sandbox. Even if they did, AC-86 cap-drop removes `CAP_SYS_RAWIO`.
#[tokio::test]
async fn fs_dev_sda_open_blocked() {
    if !preconditions_met() {
        return;
    }
    assert_payload_blocked(
        "fs/dev-sda-open",
        // `dd` writes its diagnostic to stderr; redirect to stdout so
        // we can match either path. `count=1 bs=1` keeps the test
        // bounded if (somehow) the open ever succeeded. No trailing
        // `echo exit=$?` -- the shell's exit reflects dd directly.
        "dd if=/dev/sda of=/dev/null bs=1 count=1 2>&1",
        &[
            "no such file or directory",
            "no such device",
            "permission denied",
            "cannot open",
            "failed to open",
        ],
    )
    .await;
}

/// FS-4 / mount-namespace break: a successful `mount(2)` would let
/// the sandboxed process re-attach a host filesystem under the
/// sandboxed view. Three layers stand in the way: the AC-77 seccomp
/// denylist refuses `mount(2)`, AC-86 cap-drop removes
/// `CAP_SYS_ADMIN`, and bwrap unshares the mount namespace so any
/// granted mount would not affect the host. The shell-level
/// invocation is expected to fail before any host view leaks back.
#[tokio::test]
async fn fs_mount_ns_break_blocked() {
    if !preconditions_met() {
        return;
    }
    assert_payload_blocked(
        "fs/mount-ns-break",
        // Create the target inside the sandbox tmpfs first; the
        // mount itself is what we expect to fail. We bind two
        // commands with `;` (not `&&`) so the test still asserts
        // the failing command's exit status, and we capture mount's
        // exit explicitly so the trailing `mkdir` (if it ran last)
        // can never mask a successful escape.
        "mkdir -p /tmp/mnt-target 2>/dev/null; \
         mount --bind /etc /tmp/mnt-target 2>&1; \
         status=$?; exit \"$status\"",
        &[
            "operation not permitted",
            "must be superuser",
            "permission denied",
            "bad system call",
            "only root can",
        ],
    )
    .await;
}

// -------------------------------------------------------------------
// AC-76 / Slice G1b — privesc category (3 payloads).
//
// Each test below targets one privilege-escalation primitive named
// in scope.md AC-76. They all run through `escape_profile()` (every
// defence layer on) under the production `RealSandbox` `Enforce`
// path, with the same two-check assertion shape as G1a (non-zero
// exit AND a denial signature). See readiness G1b addendum
// (T-AC76-G1b-2 / L-AC76-G1b-1..3) for the per-payload rationale.
// -------------------------------------------------------------------

/// Privesc-1 / setuid exec: even if a setuid-root binary is reachable
/// inside the sandbox view, `execve` cannot elevate. `pincery-init`
/// sets `PR_SET_NO_NEW_PRIVS`; bwrap also drops to uid 65534 in a
/// new user namespace where the host root is unmapped (AC-86). The
/// payload exec's a candidate setuid binary then prints `id -u`; the
/// effective uid must remain 65534, OR every candidate binary must
/// be absent from the sandbox view (also a valid block — the binary
/// cannot escalate what is unreachable).
#[tokio::test]
async fn privesc_setuid_exec_blocked() {
    if !preconditions_met() {
        return;
    }
    assert_payload_blocked(
        "privesc/setuid-exec",
        // `su -c 'id -u'` is the canonical setuid escalation probe;
        // we fall through to `passwd --status` if `su` is missing.
        // (`sudo` is excluded: the sandbox preflight rejects any
        // command containing `sudo` before exec — itself a valid
        // and stronger block, but it short-circuits this runtime
        // probe via `ExecResult::Rejected` before we can observe a
        // denial signature.) The trailing `id -u` proves the
        // running uid; it MUST stay at 65534 (or the candidate
        // binaries MUST all refuse / be absent). `2>&1` so refusal
        // diagnostics reach stdout. We force a non-zero exit so the
        // shared `assert_payload_blocked` non-zero-exit guard fires;
        // signature match then proves the block.
        "(su -c 'id -u' 2>&1 || \
          passwd --status 2>&1 || \
          echo \"all setuid candidates refused/absent\"); \
         id_after=$(id -u); \
         echo \"final-uid=$id_after\"; \
         test \"$id_after\" -eq 0 && exit 0; \
         exit 1",
        &[
            "must be run from a terminal",
            "permission denied",
            "no such file or directory",
            "operation not permitted",
            "authentication failure",
            "must be setuid",
            "may not be used",
            "all setuid candidates refused/absent",
            // Defence-in-depth: even if the candidate ran, the
            // trailing `final-uid=65534` proves no elevation
            // occurred. Match on the unprivileged uid string so a
            // hypothetical successful exit-0 path with uid 0
            // cannot match this signature.
            "final-uid=65534",
        ],
    )
    .await;
}

/// Privesc-2 / `CAP_SYS_ADMIN` syscall: AC-86 cap-drop ALL removes
/// `CAP_SYS_ADMIN` from the bounding set, and AC-77's seccomp
/// denylist provides defence-in-depth against `mount`/`pivot_root`.
/// `chroot` requires `CAP_SYS_CHROOT` (a subset of the privileged
/// surface); `unshare --user --map-root-user` requires
/// `CAP_SETUID`/`CAP_SETGID` in the parent namespace, which the
/// sandbox has also dropped. Both probes must fail.
#[tokio::test]
async fn privesc_cap_sys_admin_blocked() {
    if !preconditions_met() {
        return;
    }
    assert_payload_blocked(
        "privesc/cap-sys-admin",
        // Try chroot first (the most direct CAP_SYS_ADMIN-adjacent
        // primitive); fall through to unshare's map-root-user. If
        // either succeeds we EXIT 0 so the shared assertion's
        // non-zero-exit guard fires (escape!). If both denied we
        // EXIT 1 with the "both privileged probes denied" tag so
        // the assertion's non-zero+signature check passes.
        "if chroot /tmp /bin/true 2>&1; then \
           echo \"escape: chroot succeeded\"; exit 0; \
         elif unshare --user --map-root-user /bin/true 2>&1; then \
           echo \"escape: map-root-user succeeded\"; exit 0; \
         else \
           echo \"both privileged probes denied\"; exit 1; \
         fi",
        &[
            "operation not permitted",
            "permission denied",
            "must be superuser",
            "bad system call",
            "only root can",
            "no space left on device",
            "both privileged probes denied",
        ],
    )
    .await;
}

/// Privesc-3 / user-namespace elevation: with
/// `kernel.apparmor_restrict_unprivileged_userns=0` set on the
/// privileged CI host, `unshare -U` itself can succeed; the escape
/// would require chaining it to `mount`/`pivot_root`/setuid-exec,
/// all of which the seccomp denylist + `PR_SET_NO_NEW_PRIVS` block.
/// We probe by entering a new user namespace with `-r` (mapping
/// host-uid to in-ns root) then attempting `mount --bind`; the
/// mount must EPERM, surfacing the layered defence.
#[tokio::test]
async fn privesc_user_ns_elevation_blocked() {
    if !preconditions_met() {
        return;
    }
    assert_payload_blocked(
        "privesc/user-ns-elevation",
        // If the unshare itself fails (some kernels still require
        // userns-restrict relaxation), that's a valid block. If it
        // succeeds, the chained mount must fail. We capture and
        // propagate the inner shell's exit through `status=$?`.
        "unshare -U -r /bin/sh -c 'id -u; mount --bind /etc /mnt 2>&1' 2>&1; \
         status=$?; \
         exit \"$status\"",
        &[
            "operation not permitted",
            "permission denied",
            "must be superuser",
            "bad system call",
            "no such file or directory",
            // `unshare(2)` returning ENOSPC is the kernel refusing
            // to create another user namespace (per-user limit /
            // userns-restrict). That is a valid block — the
            // payload could not even enter the new userns to chain
            // mount(2).
            "no space left on device",
            "only root can",
        ],
    )
    .await;
}

// -------------------------------------------------------------------
// AC-76 / Slice G1c — resource-exhaustion category (3 payloads).
//
// Each test below targets one resource-exhaustion primitive named in
// scope.md AC-76. They all run through `escape_profile()` (every
// defence layer on, including the G1c-upgraded cgroup v2 limits
// `memory.max=512M` / `pids.max=64`) under the production
// `RealSandbox` `Enforce` path, with the same two-check assertion
// shape as G1a/G1b (non-zero exit AND a denial signature). See
// readiness G1c addendum (T-AC76-G1c-1..7 / L-AC76-G1c-1..3) for
// the per-payload rationale.
// -------------------------------------------------------------------

/// Resource-1 / fork-bomb: a recursive shell function that pipes its
/// own output to another invocation of itself. The classic shape
/// `b(){ b|b& };b` does NOT work for our purposes — the trailing
/// `&` backgrounds the recursion so the parent shell returns 0
/// immediately, before the kernel pids cap has a chance to bite.
/// The synchronous form `b(){ b|b; };b` keeps the pipeline in the
/// foreground; each invocation foreground-forks two children
/// running `b`. When `pids.max=64` is exhausted, dash emits
/// "Cannot fork" to stderr and the entire process tree exits
/// non-zero. We wrap in `timeout 4s` so the test cannot hang even
/// if (somehow) the cap is not enforced — the signature list does
/// NOT include plain SIGTERM ("Terminated" / exit 124), so a
/// timeout-only kill without an EAGAIN signature would FAIL the
/// test (per readiness scope-reduction risk).
#[tokio::test]
async fn resource_fork_bomb_blocked() {
    if !preconditions_met() {
        return;
    }
    assert_payload_blocked(
        "resource/fork-bomb",
        // Synchronous recursive fork-bomb. Define `b`, then call
        // `b` so each invocation foreground-forks two children
        // (`b|b` is a foreground pipeline). `2>&1` so EAGAIN /
        // "Cannot fork" diagnostics reach stdout. `timeout 4s`
        // bounds the test.
        "set +m; \
         timeout 4s sh -c 'b(){ b|b; }; b' 2>&1; \
         status=$?; \
         echo \"timeout-status=$status\"; \
         exit \"$status\"",
        &[
            "resource temporarily unavailable",
            "cannot fork",
            "fork:",
            "no more processes",
        ],
    )
    .await;
}

/// Resource-2 / memory-balloon: allocate ≈600 MiB of anonymous
/// process memory, exceeding `memory.max=512 MiB`. We deliberately
/// avoid writing to `/tmp` (the bwrap tmpfs is owned by the
/// launcher uid, not by uid 65534 — a stronger property of AC-86).
///
/// **Precondition:** the memory controller must be enabled in the
/// host cgroup hierarchy (`memory` listed in
/// `/sys/fs/cgroup/cgroup.subtree_control`). Without delegation,
/// writing to `memory.max` succeeds at the file level but is a
/// no-op — the kernel does not enforce the cap. CI runners that do
/// not delegate the memory controller cause this test to self-skip
/// with an explicit diagnostic. Tracked separately from AC-76 as a
/// runtime/infrastructure gap (see `scaffolding/scope.md` Deferred).
#[tokio::test]
async fn resource_memory_balloon_blocked() {
    if !preconditions_met() {
        return;
    }
    // AC-76 / G1c.x: the test now gates on an empirical runtime
    // probe (`probe_memory_max_enforcement`) rather than the cheap
    // `subtree_control` parser. The cheap parser was insufficient:
    // CI runs 25142773968 / 25142973309 demonstrated `memory` IS
    // listed in `/sys/fs/cgroup/cgroup.subtree_control` on the
    // privileged sandbox-smoke runner, yet the kernel still does
    // not OOM-kill a too-large allocation. Only an empirical probe
    // (8 MiB cap, 64 MiB dd allocation, observe SIGKILL) reliably
    // distinguishes "enforced" from "delegated but ignored". The
    // probe runs ONCE here so any host where memory.max is honored
    // exercises this test for real, while hosts with the broken
    // delegation/swap/overlayfs interaction self-skip with concrete
    // empirical evidence rather than blind silence.
    let _ = memory_controller_delegated; // retained for diagnostic comparison
    match probe_memory_max_enforcement() {
        MemoryProbeOutcome::Enforced => {
            // Fall through to the real assertion below.
        }
        MemoryProbeOutcome::NotEnforced { evidence } => {
            eprintln!(
                "skipping resource/memory-balloon: memory.max NOT enforced on this host. \
                 Empirical evidence: {evidence}. \
                 Common causes: memory controller not delegated to subtree_control, \
                 swap accounting (memory.swap.max=max with available swap), or Docker \
                 overlayfs interaction. See scope.md Deferred for the runtime/CI fix."
            );
            return;
        }
        MemoryProbeOutcome::Skipped { reason } => {
            eprintln!("skipping resource/memory-balloon: probe could not run — {reason}");
            return;
        }
    }
    // Probe says enforced — run the real adversarial assertion.
    // `dd if=/dev/zero of=/dev/null bs=600M count=1` allocates a
    // single 600 MiB anonymous buffer for one I/O block. The
    // allocation happens in dd's address space *before* any
    // read/write, so cgroup memory accounting catches it
    // immediately. We deliberately avoid writing to `/tmp` (the
    // bwrap tmpfs is owned by the launcher uid, not by uid 65534
    // — a stronger property of AC-86).
    assert_payload_blocked(
        "resource/memory-balloon",
        "dd if=/dev/zero of=/dev/null bs=600M count=1 2>&1; \
         status=$?; \
         echo \"alloc-status=$status\"; \
         exit \"$status\"",
        &[
            "killed",
            "out of memory",
            "alloc-status=137",
            "alloc-status=143",
            "memory exhausted",
            "cannot allocate memory",
        ],
    )
    .await;
}

/// Resource-3 / pid-exhaustion: a flat backgrounded loop that tries
/// to spawn 200 sleep processes. Distinct from the recursive
/// fork-bomb in shape (linear, not exponential). dash silently
/// swallows fork failures from `&` (returns 0 unconditionally) and
/// optimizes `(:)` to skip the subshell fork (single-builtin
/// optimization), so per-iteration fork canaries do not work.
/// Instead, we follow the pattern from
/// `tests/sandbox_cgroup_test.rs::cgroup_pids_max_limits_fork_count`:
/// spawn N background jobs, then count survivors via `jobs -p`.
/// When `pids.max=64` is enforced, the survivor count is bounded
/// well below 200. We make the assertion deterministic by having
/// the script *itself* exit non-zero with a sentinel diagnostic
/// when the survivor count is undercount, rather than relying on
/// the shell printing kernel diagnostics.
#[tokio::test]
async fn resource_pid_exhaustion_blocked() {
    if !preconditions_met() {
        return;
    }
    assert_payload_blocked(
        "resource/pid-exhaustion",
        // Linear backgrounded fork loop with explicit survivor
        // count. `jobs -p` enumerates live background PIDs; under
        // pids.max=64 the count is bounded (cgroup also includes
        // the bwrap parent, sh, sleep procs — so 200 attempts
        // produce at most ~60 live sleeps). The script prints
        // SURVIVORS=$count and exits non-zero with a sentinel if
        // the count is below the request, giving us a
        // deterministic non-zero exit + signature substring.
        // `set +m` silences job-control noise.
        "set +m; \
         timeout 4s sh -c 'for i in $(seq 1 200); do sleep 60 & done; count=$(jobs -p | wc -l); echo SURVIVORS=$count; if [ \"$count\" -lt 200 ]; then echo PID_CAP_BIT survivors=$count requested=200; exit 1; fi; exit 0' 2>&1; \
         status=$?; \
         echo \"timeout-status=$status\"; \
         exit \"$status\"",
        &[
            // Primary deterministic sentinel: the script itself
            // detected an undercount and exited non-zero.
            "pid_cap_bit",
            // Fallback: kernel-level diagnostic, in case the
            // shell happens to surface fork errors before we
            // reach the survivor count step.
            "resource temporarily unavailable",
            "cannot fork",
            "fork:",
            "no more processes",
        ],
    )
    .await;
}
