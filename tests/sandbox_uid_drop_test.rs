//! AC-86 / Slice G0d: UID and capability drop proof for the real
//! bwrap + `pincery-init` path.
//!
//! The unit tests in `bwrap.rs` pin the argv/policy contract. These
//! integration tests exercise the observable Linux process state after
//! bwrap has created the namespace and `pincery-init` has applied its
//! defense-in-depth policy.

#![cfg(target_os = "linux")]

use std::time::Duration;

use open_pincery::config::{ResolvedSandboxMode, SandboxMode};
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
    let probe = RealKernelProbe;
    probe
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
        eprintln!("skipping: Landlock ABI below AC-86 strict floor {LANDLOCK_ABI_FLOOR}");
        return false;
    }
    std::env::set_var("PINCERY_INIT_BIN_PATH", env!("CARGO_BIN_EXE_pincery-init"));
    true
}

fn enforce_sandbox() -> RealSandbox {
    RealSandbox::new(ResolvedSandboxMode {
        mode: SandboxMode::Enforce,
        allow_unsafe: false,
    })
}

fn uid_drop_profile() -> SandboxProfile {
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

#[tokio::test]
async fn real_sandbox_runs_as_nobody_with_empty_effective_caps() {
    if !preconditions_met() {
        return;
    }

    let script = r#"
        uid=$(id -u)
        gid=$(id -g)
        cap=$(awk '/^CapEff:/ {print $2}' /proc/self/status)
        if unshare -U true >/dev/null 2>&1; then
          userns=allowed
        else
          userns=blocked
        fi
        printf 'uid=%s\ngid=%s\ncap=%s\nuserns=%s\n' "$uid" "$gid" "$cap" "$userns"
    "#;

    let result = enforce_sandbox()
        .run(&ShellCommand::new(script), &uid_drop_profile())
        .await;
    match result {
        ExecResult::Ok {
            stdout,
            stderr,
            exit_code,
            ..
        } => {
            assert_eq!(exit_code, 0, "identity probe failed; stderr={stderr:?}");
            assert!(
                stdout.contains("uid=65534"),
                "expected uid=65534: {stdout:?}"
            );
            assert!(
                stdout.contains("gid=65534"),
                "expected gid=65534: {stdout:?}"
            );
            assert!(
                stdout.contains("cap=0000000000000000"),
                "expected empty effective capability set: {stdout:?}"
            );
            assert!(
                stdout.contains("userns=blocked"),
                "nested user namespace creation must be blocked: {stdout:?}"
            );
        }
        other => panic!("expected Ok identity probe, got {other:?}"),
    }
}

#[tokio::test]
async fn sandbox_uid_zero_override_requires_allow_unsafe() {
    std::env::set_var("OPEN_PINCERY_SANDBOX_UID", "0");
    std::env::set_var("OPEN_PINCERY_SANDBOX_GID", "0");
    std::env::remove_var("OPEN_PINCERY_ALLOW_UNSAFE");

    let result = enforce_sandbox()
        .run(&ShellCommand::new("echo must-not-run"), &uid_drop_profile())
        .await;

    std::env::remove_var("OPEN_PINCERY_SANDBOX_UID");
    std::env::remove_var("OPEN_PINCERY_SANDBOX_GID");

    match result {
        ExecResult::Err(reason) => {
            assert!(
                reason.contains("OPEN_PINCERY_SANDBOX_UID=0")
                    && reason.contains("OPEN_PINCERY_ALLOW_UNSAFE=true"),
                "UID 0 override must fail with the unsafe opt-in guidance, got: {reason}"
            );
        }
        other => panic!("uid 0 override should fail before spawn, got {other:?}"),
    }
}
