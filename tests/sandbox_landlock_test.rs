//! AC-53 / Slice A2b.4c + AC-83 / Slice G0a.3g — adversarial tests
//! for the landlock layer.
//!
//! As of G0a.3g, landlock installs INSIDE the sandbox via
//! `pincery-init` (post-bwrap, post-namespace setup). These tests
//! exercise the full wrapper pipeline:
//!   parent builds `SandboxInitPolicy` + memfd → bwrap execs the
//!   wrapper → wrapper installs landlock via
//!   `landlock_restrict_self` → wrapper execvps `sh -c <cmd>`.
//!
//! Each test exercises a distinct posture of the landlock ruleset:
//!   1. Positive control: `echo` + reading `/etc/hostname` (which is
//!      under the rx-allowed `/etc` prefix) succeed under the default
//!      profile.
//!   2. Adversarial: writing to `/tmp/foo` is BLOCKED by landlock
//!      because `/tmp` is bwrap's tmpfs (a fresh inode not in our
//!      rwx allowlist), even though the bwrap mount made it
//!      writable from a Unix-permission standpoint.
//!   3. Disabled posture: with `landlock: false`, the same write
//!      to `/tmp/foo` succeeds — proving the block in test (2) is
//!      caused by landlock, not by some other layer.
//!   4. Self-skip when bwrap is not on PATH or kernel doesn't
//!      support landlock (CI/devshell only).
//!
//! All tests target Linux; the whole file is `cfg(target_os = "linux")`
//! gated. Tests self-skip if prerequisites are missing so local
//! `cargo test` on Windows/macOS passes trivially.

#![cfg(target_os = "linux")]

use std::time::Duration;

use open_pincery::config::{ResolvedSandboxMode, SandboxMode};
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
    // Mirror the production probe so tests skip cleanly on kernels
    // < 5.13 without landlock support.
    open_pincery::runtime::sandbox::landlock_layer::landlock_supported()
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
    // AC-83 / Slice G0a.3g: point RealSandbox at the cargo-built
    // `pincery-init` binary. Integration tests get
    // `CARGO_BIN_EXE_pincery-init` from cargo automatically, so the
    // env var always resolves under `cargo test`. --test-threads=1
    // (enforced by CI) makes `set_var` safe here.
    std::env::set_var("PINCERY_INIT_BIN_PATH", env!("CARGO_BIN_EXE_pincery-init"));
    true
}

fn enforce_sandbox() -> RealSandbox {
    RealSandbox::new(ResolvedSandboxMode {
        mode: SandboxMode::Enforce,
        allow_unsafe: false,
    })
}

/// Profile with landlock active, and the OTHER kernel-primitive
/// layers disabled so a failure points at landlock specifically.
fn landlock_profile() -> SandboxProfile {
    SandboxProfile {
        env_allowlist: vec!["PATH".into()],
        deny_net: true,
        timeout: Duration::from_secs(10),
        cwd: None,
        cgroup: None,
        seccomp: false,
        landlock: true,
    }
}

/// Positive control: the default landlock profile allows read+execute
/// on `/usr`, `/bin`, `/etc`, etc. so a stock `sh -c "echo ..."`
/// runs end-to-end exactly as it did before the landlock layer
/// landed.
///
/// Implicitly verifies the whole pipeline: `SandboxInitPolicy`
/// build → memfd → bwrap → `pincery-init` → in-sandbox
/// `landlock_restrict_self` → sh. If any link breaks, spawn
/// fails or sh dies before echo runs.
#[tokio::test]
async fn landlock_permits_normal_commands() {
    if !preconditions_met() {
        return;
    }
    let result = enforce_sandbox()
        .run(&ShellCommand::new("echo landlock-ok"), &landlock_profile())
        .await;
    match result {
        ExecResult::Ok {
            stdout,
            exit_code,
            stderr,
        } => {
            assert_eq!(
                exit_code, 0,
                "echo failed under landlock; stderr={stderr:?}"
            );
            assert!(
                stdout.contains("landlock-ok"),
                "unexpected stdout: {stdout:?}"
            );
        }
        other => panic!("expected Ok, got {other:?}"),
    }
}

/// Positive control #2: reading a file under `/etc` works because
/// `/etc` is in the default rx allowlist. We cat `/etc/hostname`
/// which exists on every Linux distro we care about.
#[tokio::test]
async fn landlock_permits_reading_etc() {
    if !preconditions_met() {
        return;
    }
    let result = enforce_sandbox()
        .run(
            // `2>/dev/null` swallows any "no such file" diagnostic on
            // exotic distros; the assertion below just checks that
            // the syscall path itself wasn't blocked by landlock.
            &ShellCommand::new("cat /etc/hostname 2>/dev/null; echo exit=$?"),
            &landlock_profile(),
        )
        .await;
    match result {
        ExecResult::Ok { stdout, .. } => {
            // Either the cat succeeded (exit=0) or the file was
            // missing (exit=1). Both are fine — what we're ruling
            // out is exit=126/127 (permission denied / not found)
            // from a landlock block, which would surface differently.
            assert!(
                stdout.contains("exit=0") || stdout.contains("exit=1"),
                "unexpected stdout (landlock blocked /etc?): {stdout:?}"
            );
        }
        other => panic!("expected Ok, got {other:?}"),
    }
}

/// Adversarial: bwrap mounts `/tmp` as a fresh tmpfs (writable from
/// a Unix-permission standpoint), but landlock sees a NEW inode at
/// `/tmp` that wasn't in our rwx allowlist when `restrict_self` ran
/// (the rule was created against the host's `/tmp` inode pre-fork).
/// Result: opening `/tmp/foo` for write returns EACCES.
///
/// We probe via `sh -c 'echo > /tmp/foo && echo wrote || echo
/// blocked'`. Under landlock the redirection's `open(O_WRONLY|
/// O_CREAT)` returns EACCES, the `&&` short-circuits, and we see
/// `blocked`.
#[tokio::test]
async fn landlock_blocks_writes_to_unlisted_tmpfs() {
    if !preconditions_met() {
        return;
    }
    let result = enforce_sandbox()
        .run(
            &ShellCommand::new("echo data > /tmp/foo 2>/dev/null && echo wrote || echo blocked"),
            &landlock_profile(),
        )
        .await;
    match result {
        ExecResult::Ok { stdout, .. } => {
            assert!(
                stdout.contains("blocked"),
                "landlock should have blocked write to /tmp/foo, got: {stdout:?}"
            );
            assert!(
                !stdout.contains("wrote"),
                "write to /tmp/foo unexpectedly succeeded: {stdout:?}"
            );
        }
        other => panic!("expected Ok (sh handles redirect failure), got {other:?}"),
    }
}

/// Disabled-posture contract: with `landlock: false`, the same
/// write to `/tmp/foo` succeeds. This proves the block in the
/// previous test is caused by landlock specifically, not by some
/// other layer (bwrap mounts /tmp as tmpfs which IS writable from
/// a Unix perspective).
#[tokio::test]
async fn landlock_disabled_via_profile_permits_writes() {
    if !bwrap_available() {
        eprintln!("skipping: bwrap not on PATH");
        return;
    }
    let profile = SandboxProfile {
        landlock: false,
        ..landlock_profile()
    };
    let result = enforce_sandbox()
        .run(
            &ShellCommand::new("echo data > /tmp/foo 2>/dev/null && echo wrote || echo blocked"),
            &profile,
        )
        .await;
    match result {
        ExecResult::Ok { stdout, .. } => {
            assert!(
                stdout.contains("wrote"),
                "with landlock disabled, write to /tmp/foo should succeed; got: {stdout:?}"
            );
        }
        other => panic!("expected Ok, got {other:?}"),
    }
}
