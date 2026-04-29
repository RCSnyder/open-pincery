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
    // Integration tests get `CARGO_BIN_EXE_pincery-init` from cargo
    // automatically. `--test-threads=1` is enforced by the privileged
    // CI sandbox-smoke job, which makes `set_var` safe here.
    std::env::set_var("PINCERY_INIT_BIN_PATH", env!("CARGO_BIN_EXE_pincery-init"));
    true
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
fn escape_profile() -> SandboxProfile {
    SandboxProfile {
        env_allowlist: vec!["PATH".into()],
        deny_net: true,
        timeout: Duration::from_secs(10),
        cwd: None,
        cgroup: None,
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

/// FS-1 / read `/etc/shadow`: the file exists inside the sandbox
/// because bwrap `--ro-bind /etc /etc` exposes it, but its mode
/// (0640 root:shadow) is unreadable by UID 65534 (AC-86 uid drop).
/// Landlock's rx allowance on `/etc` does not bypass DAC.
#[tokio::test]
async fn fs_etc_shadow_read_blocked() {
    if !preconditions_met() {
        return;
    }
    assert_payload_blocked(
        "fs/etc-shadow-read",
        // 2>&1 so we see the "Permission denied" diagnostic on
        // stdout even when the cat redirect target is /dev/null.
        "cat /etc/shadow 2>&1; echo exit=$?",
        // Only kernel/coreutils denial diagnostics — bare
        // "exit=N" tokens are deliberately excluded so a missing
        // binary or shell syntax error cannot pass for a block.
        &["permission denied"],
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
        "ls -la /proc/1/root 2>&1; echo exit=$?",
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
        // bounded if (somehow) the open ever succeeded.
        "dd if=/dev/sda of=/dev/null bs=1 count=1 2>&1; echo exit=$?",
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
        // We need /mnt to exist as a target; create it inside the
        // sandbox tmpfs first. The mount itself is what we expect
        // to fail.
        "mkdir -p /tmp/mnt-target 2>/dev/null; \
         mount --bind /etc /tmp/mnt-target 2>&1; \
         echo exit=$?",
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
