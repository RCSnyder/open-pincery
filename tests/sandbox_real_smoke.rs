//! AC-53 / Slice A2b.3: Linux smoke test for the `RealSandbox` wrapper.
//!
//! Verifies five things by actually spawning bwrap-wrapped shells:
//!
//! 1. A trivial `true` command exits with code 0.
//! 2. `echo` produces expected stdout.
//! 3. The `sudo` pre-flight rejects before any bwrap spawn.
//! 4. With `deny_net=true`, the sandboxed process cannot resolve
//!    or connect out — we observe this by checking that `/sys/class/net`
//!    inside the sandbox has only `lo` missing (no interfaces at all).
//! 5. The sandbox process sees a fresh UTS namespace — `hostname`
//!    returns `sandbox`, not the host hostname.
//!
//! This test is Linux-only and additionally requires `bwrap` on PATH.
//! If `bwrap` is absent (e.g. a stock Windows/macOS CI runner or a
//! Linux host without bubblewrap installed), the test self-skips
//! rather than fails — so devs without the devshell image aren't
//! blocked from running the rest of the suite.

#![cfg(target_os = "linux")]

use open_pincery::config::{ResolvedSandboxMode, SandboxMode};
use open_pincery::runtime::sandbox::{
    bwrap::RealSandbox, ExecResult, SandboxProfile, ShellCommand, ToolExecutor,
};
use std::time::Duration;

fn bwrap_available() -> bool {
    std::process::Command::new("bwrap")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn sandbox() -> RealSandbox {
    RealSandbox::new(ResolvedSandboxMode {
        mode: SandboxMode::Enforce,
        allow_unsafe: false,
    })
}

fn profile() -> SandboxProfile {
    SandboxProfile {
        env_allowlist: vec!["PATH".into()],
        deny_net: true,
        timeout: Duration::from_secs(15),
        cwd: None,
        cgroup: None,
        seccomp: true,
        landlock: true,
    }
}

#[tokio::test]
async fn real_sandbox_runs_trivial_true() {
    if !bwrap_available() {
        eprintln!("skipping: bwrap not on PATH");
        return;
    }
    let executor = sandbox();
    let result = executor.run(&ShellCommand::new("true"), &profile()).await;
    match result {
        ExecResult::Ok {
            exit_code,
            stdout,
            stderr,
        } => assert_eq!(
            exit_code, 0,
            "bwrap exited non-zero; stdout={stdout:?} stderr={stderr:?}"
        ),
        other => panic!("expected Ok, got {other:?}"),
    }
}

#[tokio::test]
async fn real_sandbox_echoes_expected_stdout() {
    if !bwrap_available() {
        eprintln!("skipping: bwrap not on PATH");
        return;
    }
    let executor = sandbox();
    let result = executor
        .run(&ShellCommand::new("echo hello-sandbox"), &profile())
        .await;
    match result {
        ExecResult::Ok {
            stdout,
            exit_code,
            stderr,
        } => {
            assert_eq!(
                exit_code, 0,
                "bwrap exited non-zero; stdout={stdout:?} stderr={stderr:?}"
            );
            assert!(
                stdout.contains("hello-sandbox"),
                "stdout missing expected output: stdout={stdout:?} stderr={stderr:?}"
            );
        }
        other => panic!("expected Ok, got {other:?}"),
    }
}

#[tokio::test]
async fn real_sandbox_rejects_sudo_preflight() {
    // Pre-flight reject fires BEFORE bwrap spawn, so this passes
    // even if bwrap is missing. Test still gated by
    // `#[cfg(target_os = "linux")]` to match the rest of the file.
    let executor = sandbox();
    let result = executor
        .run(&ShellCommand::new("sudo whoami"), &profile())
        .await;
    match result {
        ExecResult::Rejected(reason) => {
            assert!(
                reason.contains("sudo"),
                "unexpected reject reason: {reason}"
            )
        }
        other => panic!("expected Rejected, got {other:?}"),
    }
}

#[tokio::test]
async fn real_sandbox_sees_fresh_uts_hostname() {
    if !bwrap_available() {
        eprintln!("skipping: bwrap not on PATH");
        return;
    }
    let executor = sandbox();
    // Read from /proc/sys/kernel/hostname directly — busybox `hostname`
    // may not be on the minimal image PATH.
    let result = executor
        .run(
            &ShellCommand::new("cat /proc/sys/kernel/hostname"),
            &profile(),
        )
        .await;
    match result {
        ExecResult::Ok { stdout, stderr, .. } => {
            assert!(
                stdout.trim() == "sandbox",
                "expected UTS hostname 'sandbox', got stdout={stdout:?} stderr={stderr:?}"
            );
        }
        other => panic!("expected Ok, got {other:?}"),
    }
}

#[tokio::test]
async fn real_sandbox_denies_network_when_deny_net_is_true() {
    if !bwrap_available() {
        eprintln!("skipping: bwrap not on PATH");
        return;
    }
    let executor = sandbox();
    // With --unshare-net the sandboxed process has only a lo
    // interface with no IP configured. `ip -br link` from busybox
    // is not guaranteed; use /sys/class/net listing instead which
    // only requires the tmpfs + proc mount.
    let result = executor
        .run(
            &ShellCommand::new("ls /sys/class/net | tr '\\n' ' '"),
            &profile(),
        )
        .await;
    match result {
        ExecResult::Ok {
            stdout,
            exit_code,
            stderr,
        } => {
            assert_eq!(
                exit_code, 0,
                "bwrap exited non-zero; stdout={stdout:?} stderr={stderr:?}"
            );
            // Only `lo` should remain; eth0/wlan0 would indicate
            // the host's netns leaked through.
            let ifaces: Vec<&str> = stdout.split_whitespace().collect();
            for bad in ["eth0", "wlan0", "ens3", "ens33", "enp0s3", "docker0"] {
                assert!(
                    !ifaces.contains(&bad),
                    "host interface {bad} visible inside sandbox: stdout={stdout:?} stderr={stderr:?}"
                );
            }
        }
        other => panic!("expected Ok, got {other:?}"),
    }
}
