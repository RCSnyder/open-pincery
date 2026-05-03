//! AC-36: ProcessExecutor sandbox guarantees.
//!
//! These are pure in-process tests — no DB, no fixtures. They pin the
//! three invariants that BUILD Slice 4 is supposed to deliver:
//!
//!   1. **Environment isolation**: a var set in the parent that is NOT on
//!      the allowlist must not appear in the child's env.
//!   2. **Wall-clock timeout**: a command that exceeds `profile.timeout`
//!      must return `ExecResult::Timeout` and not hang the caller.
//!   3. **Pre-flight rejection**: `sudo ...` is rejected *before* a
//!      process is spawned — verified by proving a side-effecting probe
//!      was never created.

use open_pincery::runtime::sandbox::{
    ExecResult, ProcessExecutor, SandboxProfile, ShellCommand, ToolExecutor,
};
use std::time::Duration;

#[tokio::test]
async fn env_is_scrubbed_to_allowlist() {
    // Set a parent-process var that is NOT on the default allowlist.
    // SAFETY: single-threaded by virtue of #[tokio::test] default;
    // the value is only read by our child and asserted on below.
    unsafe { std::env::set_var("PINCERY_SECRET_NOT_ALLOWED", "leaked-if-seen") };

    let exec = ProcessExecutor;
    let result = exec
        .run(
            &ShellCommand {
                command: "printenv | sort".into(),
                ..Default::default()
            },
            &SandboxProfile::default(),
        )
        .await;

    match result {
        ExecResult::Ok { stdout, .. } => {
            assert!(
                !stdout.contains("PINCERY_SECRET_NOT_ALLOWED"),
                "env var leaked into child: {stdout}"
            );
            assert!(
                !stdout.contains("leaked-if-seen"),
                "env value leaked into child: {stdout}"
            );
        }
        other => panic!("expected Ok, got {other:?}"),
    }
}

#[tokio::test]
async fn timeout_returns_timeout_not_hang() {
    let exec = ProcessExecutor;
    let profile = SandboxProfile {
        timeout: Duration::from_millis(300),
        ..SandboxProfile::default()
    };
    let started = std::time::Instant::now();
    let result = exec
        .run(
            &ShellCommand {
                command: "sleep 30".into(),
                ..Default::default()
            },
            &profile,
        )
        .await;
    let elapsed = started.elapsed();

    assert!(
        matches!(result, ExecResult::Timeout),
        "expected Timeout, got {result:?}"
    );
    // Proves the timeout actually fired — we did not wait 30s.
    assert!(
        elapsed < Duration::from_secs(5),
        "timeout did not interrupt child; waited {elapsed:?}"
    );
}

#[tokio::test]
async fn sudo_is_rejected_pre_spawn() {
    let probe = std::env::temp_dir().join("pincery_sudo_probe_rejected");
    let _ = std::fs::remove_file(&probe);

    let exec = ProcessExecutor;
    let result = exec
        .run(
            &ShellCommand {
                // If the reject check is bypassed, this would create the probe file.
                command: format!("sudo touch {}", probe.display()),
                ..Default::default()
            },
            &SandboxProfile::default(),
        )
        .await;

    assert!(
        matches!(result, ExecResult::Rejected(_)),
        "expected Rejected, got {result:?}"
    );
    assert!(
        !probe.exists(),
        "sudo rejection must happen BEFORE spawn — probe file exists: {}",
        probe.display()
    );
}

#[tokio::test]
async fn bare_sudo_is_rejected() {
    let exec = ProcessExecutor;
    let result = exec
        .run(
            &ShellCommand {
                command: "sudo".into(),
                ..Default::default()
            },
            &SandboxProfile::default(),
        )
        .await;
    assert!(
        matches!(result, ExecResult::Rejected(_)),
        "bare `sudo` must be rejected; got {result:?}"
    );
}

/// Regression for AC-36 scope wording: the scope says the sudo check
/// must trip on commands *containing* `sudo`, not just commands that
/// start with it. Before the fix, `echo ok && sudo touch <probe>` would
/// spawn a shell and run the RHS unimpeded.
#[tokio::test]
async fn sudo_in_chained_command_is_rejected() {
    let probe = std::env::temp_dir().join("pincery_sudo_probe_chained");
    let _ = std::fs::remove_file(&probe);

    let exec = ProcessExecutor;
    let result = exec
        .run(
            &ShellCommand {
                command: format!("echo ok && sudo touch {}", probe.display()),
                ..Default::default()
            },
            &SandboxProfile::default(),
        )
        .await;

    assert!(
        matches!(result, ExecResult::Rejected(_)),
        "chained `&& sudo …` must be rejected; got {result:?}"
    );
    assert!(
        !probe.exists(),
        "rejection must happen BEFORE spawn — probe file exists: {}",
        probe.display()
    );
}

#[tokio::test]
async fn ok_command_reports_exit_and_stdout() {
    let exec = ProcessExecutor;
    let result = exec
        .run(
            &ShellCommand {
                command: "echo hello-from-sandbox".into(),
                ..Default::default()
            },
            &SandboxProfile::default(),
        )
        .await;
    match result {
        ExecResult::Ok {
            stdout, exit_code, ..
        } => {
            assert_eq!(exit_code, 0);
            assert!(
                stdout.contains("hello-from-sandbox"),
                "stdout missing expected line: {stdout:?}"
            );
        }
        other => panic!("expected Ok, got {other:?}"),
    }
}
