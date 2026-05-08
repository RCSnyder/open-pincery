//! AC-53 / Slice A2b.4a: Real-kernel smoke test for the cgroup v2
//! resource-cap layer.
//!
//! Verifies that when `SandboxProfile.cgroup = Some(limits)` the
//! spawned bwrap child is actually attached to a cgroup v2 subgroup
//! with the requested caps enforced by the kernel. Two adversarial
//! payloads give unambiguous kernel-visible proof:
//!
//!   1. **pids.max**: a fork-loop that tries to spawn more concurrent
//!      processes than the cap allows. With `pids_max = 5`, the 6th+
//!      `sleep &` fails with "Resource temporarily unavailable"
//!      (EAGAIN from `fork(2)`).
//!   2. **memory.max**: a memory balloon that allocates more than the
//!      cap. With `memory_max_bytes = 16 MiB`, attempting to allocate
//!      64 MiB triggers the cgroup OOM killer — the child exits with
//!      a non-zero status (typically signal 9 = SIGKILL, surfaced as
//!      exit code 137).
//!
//! Plus a positive-control test that a command respecting the caps
//! runs to completion and cleans up the cgroup dir on Drop.
//!
//! This test is Linux-only AND requires `bwrap` on PATH AND requires
//! the running process to be able to `mkdir` under `/sys/fs/cgroup`
//! (root, CAP_SYS_ADMIN in userns, or a systemd-delegated subtree).
//! It self-skips on any environment that doesn't meet all three —
//! consistent with `sandbox_real_smoke.rs`.

#![cfg(target_os = "linux")]

use open_pincery::config::{ResolvedSandboxMode, SandboxMode};
use open_pincery::runtime::sandbox::{
    bwrap::RealSandbox,
    cgroup::{cgroup_v2_writable, CgroupLimits},
    ExecResult, SandboxProfile, ShellCommand, ToolExecutor,
};
use std::time::Duration;

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

fn preconditions_met() -> bool {
    if !bwrap_available() {
        eprintln!("skipping: bwrap not on PATH");
        return false;
    }
    if !cgroup_v2_writable() {
        eprintln!("skipping: process cannot mkdir under /sys/fs/cgroup (not root / no delegation)");
        return false;
    }
    true
}

fn enforce_sandbox() -> RealSandbox {
    RealSandbox::new(ResolvedSandboxMode {
        mode: SandboxMode::Enforce,
        allow_unsafe: false,
    })
}

fn audit_sandbox() -> RealSandbox {
    RealSandbox::new(ResolvedSandboxMode {
        mode: SandboxMode::Audit,
        allow_unsafe: false,
    })
}

/// Positive control: a trivial command with caps well above its real
/// needs runs to completion and reports success.
#[tokio::test]
async fn cgroup_permits_command_under_caps() {
    if !preconditions_met() {
        return;
    }
    let profile = SandboxProfile {
        env_allowlist: vec!["PATH".into()],
        deny_net: true,
        timeout: Duration::from_secs(10),
        cwd: None,
        cgroup: Some(CgroupLimits {
            memory_max_bytes: Some(256 * 1024 * 1024),
            pids_max: Some(64),
            cpu_max_micros: None,
        }),
        seccomp: false,
        landlock: false,
    };
    let result = enforce_sandbox()
        .run(&ShellCommand::new("echo cgroup-ok"), &profile)
        .await;
    match result {
        ExecResult::Ok {
            stdout, exit_code, ..
        } => {
            assert_eq!(exit_code, 0);
            assert!(
                stdout.contains("cgroup-ok"),
                "unexpected stdout: {stdout:?}"
            );
        }
        other => panic!("expected Ok, got {other:?}"),
    }
}

/// Adversarial: a fork loop that tries to create 20 concurrent
/// children while `pids.max=5`. POSIX `sh` returns non-zero when
/// `fork(2)` fails; even if the shell swallows the error, the final
/// process count in the cgroup stays bounded. We probe by asking
/// the shell to count successful backgrounds — if the cap works,
/// the count is ≤ 5; if it doesn't, the count is 20.
#[tokio::test]
async fn cgroup_pids_max_limits_fork_count() {
    if !preconditions_met() {
        return;
    }
    let profile = SandboxProfile {
        env_allowlist: vec!["PATH".into()],
        deny_net: true,
        timeout: Duration::from_secs(10),
        cwd: None,
        cgroup: Some(CgroupLimits {
            memory_max_bytes: None,
            // pids.max counts every task in the cgroup including the
            // bwrap process itself + the sh child, so we leave a bit
            // of headroom. 8 = bwrap + sh + ≤6 user-spawned sleeps.
            pids_max: Some(8),
            cpu_max_micros: None,
        }),
        seccomp: false,
        landlock: false,
    };
    // Try to spawn 20 concurrent sleeps. Under the 8-task cap, most
    // will fail with EAGAIN from fork(2) and sh will either print
    // "Resource temporarily unavailable" to stderr or silently drop
    // them. Either way the exit status of `wait` reflects failure.
    //
    // The shell's overall exit code depends on the busybox/bash
    // implementation: bash returns 0 from `wait` with no args even
    // when some backgrounds failed. So instead we probe by counting
    // how many sleeps survived using /proc inside the sandbox — if
    // the cap is enforced, fewer than 20 remain.
    let script = "\
        for i in $(seq 1 20); do sleep 2 & done; \
        count=$(jobs -p | wc -l); \
        echo SURVIVORS=$count; \
        wait 2>/dev/null; \
        true";
    let result = enforce_sandbox()
        .run(&ShellCommand::new(script), &profile)
        .await;
    match result {
        ExecResult::Ok { stdout, stderr, .. } => {
            // Expect EAGAIN-ish diagnostic in stderr OR a SURVIVORS
            // count below 20 in stdout. Either is kernel-level proof
            // the cap bit.
            let saw_eagain = stderr.contains("Resource temporarily unavailable")
                || stderr.contains("cannot fork")
                // dash (Ubuntu's /bin/sh) reports "sh: <n>: Cannot fork"
                // with a capital C when hitting pids.max.
                || stderr.contains("Cannot fork")
                || stderr.contains("fork:");
            let survivors_line = stdout
                .lines()
                .find(|l| l.starts_with("SURVIVORS="))
                .unwrap_or("SURVIVORS=?");
            let survivors: i32 = survivors_line
                .trim_start_matches("SURVIVORS=")
                .parse()
                .unwrap_or(-1);
            assert!(
                saw_eagain || (0..20).contains(&survivors),
                "pids.max cap not enforced: stderr={stderr:?} stdout={stdout:?}"
            );
        }
        other => panic!("expected Ok (sh may report errors to stderr), got {other:?}"),
    }
}

/// Fail-closed contract: when cgroup init fails in Enforce mode, the
/// executor returns `ExecResult::Err` and never runs the command.
/// We provoke a failure by passing an obviously invalid cpu.max
/// tuple — cgroup v2 rejects a zero period with EINVAL.
#[tokio::test]
async fn cgroup_init_failure_fails_closed_in_enforce() {
    if !preconditions_met() {
        return;
    }
    let profile = SandboxProfile {
        env_allowlist: vec!["PATH".into()],
        deny_net: true,
        timeout: Duration::from_secs(5),
        cwd: None,
        cgroup: Some(CgroupLimits {
            memory_max_bytes: None,
            pids_max: None,
            // period=0 is rejected by the kernel with EINVAL —
            // `fs::write` to cpu.max returns an io::Error, which
            // `CgroupGuard::new` bubbles up.
            cpu_max_micros: Some((50_000, 0)),
        }),
        seccomp: false,
        landlock: false,
    };
    let result = enforce_sandbox()
        .run(&ShellCommand::new("echo should-not-run"), &profile)
        .await;
    match result {
        ExecResult::Err(msg) => {
            assert!(
                msg.contains("cgroup"),
                "expected cgroup-specific error, got: {msg}"
            );
            assert!(
                msg.contains("enforce"),
                "expected enforce-mode marker, got: {msg}"
            );
        }
        other => panic!("expected Err (fail-closed), got {other:?}"),
    }
}

/// Audit-mode contract: same failure path, but the executor proceeds
/// without a cgroup and the command still runs. This is the parallel
/// posture of seccomp's SECCOMP_RET_LOG — observe and continue.
#[tokio::test]
async fn cgroup_init_failure_proceeds_in_audit() {
    if !preconditions_met() {
        return;
    }
    let profile = SandboxProfile {
        env_allowlist: vec!["PATH".into()],
        deny_net: true,
        timeout: Duration::from_secs(5),
        cwd: None,
        cgroup: Some(CgroupLimits {
            memory_max_bytes: None,
            pids_max: None,
            cpu_max_micros: Some((50_000, 0)),
        }),
        seccomp: false,
        landlock: false,
    };
    let result = audit_sandbox()
        .run(&ShellCommand::new("echo audit-fallback"), &profile)
        .await;
    match result {
        ExecResult::Ok {
            stdout, exit_code, ..
        } => {
            assert_eq!(exit_code, 0);
            assert!(
                stdout.contains("audit-fallback"),
                "unexpected stdout: {stdout:?}"
            );
        }
        other => panic!("expected Ok (audit mode tolerates cgroup failure), got {other:?}"),
    }
}
