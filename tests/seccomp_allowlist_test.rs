//! AC-77 / Slice G2d — seccomp default-deny allowlist integration tests.
//!
//! These are the runtime-evidence tests for the AC-77 readiness
//! coverage table. They exercise the production `RealSandbox` with
//! `seccomp: true` against the new allowlist and assert that:
//!
//! 1. Every happy-path workload that the empirical strace corpus
//!    (`tests/fixtures/seccomp/observed_syscalls.txt`) was sourced
//!    from still exits cleanly. This is the load-bearing canary: if
//!    the allowlist is too narrow, a basic command SIGSYSes (exit
//!    159) and the assertion fires.
//!
//! 2. A namespace-creation primitive that is NOT on the allowlist
//!    (`unshare(2)`) terminates with SIGSYS in `Enforce` mode —
//!    proving the default-deny posture is real, not just a renamed
//!    denylist.
//!
//! Per readiness AC-77 G2d, the granular per-syscall blockers
//! (`bpf`, `io_uring_setup`, `perf_event_open`, etc.) require small
//! C/Rust helper binaries to issue the syscall directly. The strict
//! ESCAPE_PRIMITIVES negative-control unit test in
//! `src/runtime/sandbox/seccomp.rs` already proves those numbers are
//! absent from the compiled BPF program; these integration tests
//! prove that the program actually loads, the kernel accepts it, and
//! the kill-on-mismatch behaviour fires for a real syscall.
//!
//! Live runs require the same preconditions as `sandbox_escape_test`
//! (Linux + bwrap + Landlock floor + cgroup v2 writable). Each test
//! self-skips with an explicit reason if a precondition is missing.

#![cfg(target_os = "linux")]

use std::time::Duration;

use open_pincery::config::{ResolvedSandboxMode, SandboxMode};
use open_pincery::observability::seccomp_audit::SIGSYS_EXIT_CODE;
use open_pincery::runtime::sandbox::cgroup::{cgroup_v2_writable, CgroupLimits};
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

fn preconditions_met() -> bool {
    if !bwrap_available() {
        eprintln!("skipping: bwrap not on PATH");
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
    if !cgroup_v2_writable() {
        eprintln!("skipping: process cannot mkdir under /sys/fs/cgroup");
        return false;
    }
    std::env::set_var("PINCERY_INIT_BIN_PATH", env!("CARGO_BIN_EXE_pincery-init"));
    true
}

fn seccomp_profile(seccomp_on: bool) -> SandboxProfile {
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
        seccomp: seccomp_on,
        landlock: true,
    }
}

fn sandbox(mode: SandboxMode) -> RealSandbox {
    RealSandbox::new(ResolvedSandboxMode {
        mode,
        allow_unsafe: false,
    })
}

fn binary_in_sandbox(name: &str) -> bool {
    // Probe the bwrap mount tree the same way it will appear at run
    // time. If `command -v <name>` returns 0, the binary is reachable.
    let probe = ShellCommand::new(format!("command -v {name} >/dev/null 2>&1"));
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let result = runtime.block_on(async {
        sandbox(SandboxMode::Enforce)
            .run(&probe, &seccomp_profile(true))
            .await
    });
    matches!(result, ExecResult::Ok { exit_code: 0, .. })
}

/// AC-77 G2d.1: the default-deny allowlist must accept the happy-path
/// command surface used by AC-76 fixture capture. If this test
/// SIGSYSes, the allowlist is too narrow and we have shipped a hard
/// outage.
#[tokio::test]
async fn allowlist_covers_happy_path_workloads() {
    if !preconditions_met() {
        return;
    }

    // Mirrors a subset of `scripts/capture_seccomp_corpus.sh` — the
    // intersection that does not require binaries the bwrap rootfs
    // might not expose.
    let workloads: &[(&str, &str)] = &[
        ("echo", "echo hello"),
        ("true", "/bin/true"),
        ("noop_subshell", "sh -c ':'"),
        ("nested_sh", "sh -c 'sh -c true'"),
        ("read_random", "head -c 64 /dev/urandom | wc -c"),
        ("seq_pipe", "seq 1 5 | wc -l"),
        ("id", "id -u"),
    ];

    for (name, cmd) in workloads {
        let result = sandbox(SandboxMode::Enforce)
            .run(&ShellCommand::new(*cmd), &seccomp_profile(true))
            .await;
        match result {
            ExecResult::Ok {
                exit_code,
                stdout,
                stderr,
                ..
            } => {
                assert_ne!(
                    exit_code, SIGSYS_EXIT_CODE,
                    "[{name}] happy-path workload SIGSYSed under allowlist; stdout={stdout:?} stderr={stderr:?}"
                );
                assert_eq!(
                    exit_code, 0,
                    "[{name}] happy-path workload exited {exit_code}; stdout={stdout:?} stderr={stderr:?}"
                );
            }
            other => panic!("[{name}] expected ExecResult::Ok, got {other:?}"),
        }
    }
}

/// AC-77 G2d.2: `unshare(2)` is NOT on the allowlist. A child that
/// calls `unshare -U /bin/true` should be terminated by SIGSYS
/// (exit 159) in `Enforce` mode. This is the load-bearing
/// default-deny test: it proves the filter actually kills, not just
/// logs, when a real syscall outside the allowlist is invoked.
#[tokio::test]
async fn unshare_blocked_by_default_deny_allowlist() {
    if !preconditions_met() {
        return;
    }
    if !binary_in_sandbox("unshare") {
        eprintln!("skipping: unshare(1) not in sandbox PATH");
        return;
    }

    let result = sandbox(SandboxMode::Enforce)
        .run(
            &ShellCommand::new("unshare -U /bin/true"),
            &seccomp_profile(true),
        )
        .await;
    match result {
        ExecResult::Ok {
            exit_code,
            stdout,
            stderr,
            ..
        } => {
            assert_eq!(
                exit_code, SIGSYS_EXIT_CODE,
                "expected SIGSYS (exit {SIGSYS_EXIT_CODE}); got exit_code={exit_code}; \
                 stdout={stdout:?} stderr={stderr:?}"
            );
        }
        other => panic!("expected ExecResult::Ok, got {other:?}"),
    }
}

/// AC-77 G2d.3: with `seccomp: false`, the same `unshare` call must
/// NOT exit 159 — it would still be blocked by AC-86 (uid-drop +
/// cap-drop) returning EPERM, but the failure path is different and
/// the exit code distinguishes them. This guards against a false
/// positive where the test in G2d.2 is "passing" because some other
/// layer is killing the child for unrelated reasons.
#[tokio::test]
async fn unshare_does_not_sigsys_when_seccomp_disabled() {
    if !preconditions_met() {
        return;
    }
    if !binary_in_sandbox("unshare") {
        eprintln!("skipping: unshare(1) not in sandbox PATH");
        return;
    }

    let result = sandbox(SandboxMode::Enforce)
        .run(
            &ShellCommand::new("unshare -U /bin/true"),
            &seccomp_profile(false),
        )
        .await;
    match result {
        ExecResult::Ok { exit_code, .. } => {
            assert_ne!(
                exit_code, SIGSYS_EXIT_CODE,
                "child SIGSYSed even though seccomp was off — another layer is forging \
                 a 159 exit and the G2d.2 assertion is meaningless"
            );
        }
        other => panic!("expected ExecResult::Ok, got {other:?}"),
    }
}

/// AC-77 G2d.4 (review R2): in `SandboxMode::Audit` the same
/// `unshare(2)` call that would SIGSYS in Enforce mode must instead
/// proceed to completion. The seccomp filter built for Audit mode
/// uses `mismatch_action=Log` + `match_action=Allow`, so the kernel
/// logs the violation to the audit subsystem but lets the syscall
/// run. `unshare -U /bin/true` succeeds (uid 65534 inside the bwrap
/// userns can re-unshare its own user namespace) and `/bin/true`
/// returns 0.
///
/// This is the runtime evidence backing readiness `T-AC77-1`'s
/// "Audit -> Log" half. Without this test, only the BPF-program
/// shape is verified (that Enforce and Audit produce *different*
/// programs); kernel-level semantics remain unverified.
#[tokio::test]
async fn audit_mode_logs_instead_of_killing() {
    if !preconditions_met() {
        return;
    }
    if !binary_in_sandbox("unshare") {
        eprintln!("skipping: unshare(1) not in sandbox PATH");
        return;
    }

    let result = sandbox(SandboxMode::Audit)
        .run(
            &ShellCommand::new("unshare -U /bin/true"),
            &seccomp_profile(true),
        )
        .await;
    match result {
        ExecResult::Ok {
            exit_code,
            stdout,
            stderr,
            ..
        } => {
            assert_ne!(
                exit_code, SIGSYS_EXIT_CODE,
                "Audit mode must NOT kill on disallowed syscall; got SIGSYS \
                 (exit {SIGSYS_EXIT_CODE}). The kernel is treating the filter as \
                 Enforce-style KillProcess instead of Log. \
                 stdout={stdout:?} stderr={stderr:?}"
            );
            // unshare -U /bin/true should now actually succeed since
            // the kernel logs and allows. /bin/true returns 0.
            assert_eq!(
                exit_code, 0,
                "Audit mode passthrough should let unshare -U /bin/true \
                 proceed to /bin/true (exit 0); got exit_code={exit_code}; \
                 stdout={stdout:?} stderr={stderr:?}"
            );
        }
        other => panic!("expected ExecResult::Ok, got {other:?}"),
    }
}
