//! AC-83 / Slice G0a.2 integration proof: the `pincery-init` wrapper
//! parses an inherited policy fd, logs a summary to stderr, and
//! `execvp`s the user argv without applying any restrictions.
//!
//! This is the **host-level** proof called for by readiness.md's G0a.2
//! build-order entry:
//!
//! > "host-level run with a hand-crafted policy fd; observe the user
//! > binary runs and the policy bytes were parsed."
//!
//! The full four-case suite (`wrapper_execs_user_binary_cleanly`,
//! `wrapper_surfaces_policy_apply_failure_as_125`,
//! `wrapper_rejects_partial_enforcement`,
//! `wrapper_is_invisible_to_user_argv`) arrives in Slice G0a.3 once
//! the policy-application pipeline is implemented. G0a.2 proves the
//! skeleton works end-to-end so G0a.3 has a known-good baseline.
//!
//! Linux-only: relies on `memfd_create(2)` + fd inheritance via
//! `pre_exec(dup2)`. Windows/macOS compile this file as empty.

#![cfg(target_os = "linux")]

use std::ffi::CString;
use std::io::Write;
use std::os::fd::{FromRawFd, IntoRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use open_pincery::runtime::sandbox::init_policy::SandboxInitPolicy;

/// Path to the `pincery-init` binary built in this test's workspace.
/// Cargo sets `CARGO_BIN_EXE_<bin-name>` for every integration test.
fn pincery_init_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_pincery-init"))
}

/// Create a non-CLOEXEC memfd (so it survives `fork` into the child's
/// `pre_exec` hook) and write `bytes` to offset 0. Returns the raw fd
/// ready to be dup2'd to fd 3 in the child.
fn make_policy_memfd(bytes: &[u8]) -> RawFd {
    // `memfd_create` is not in stable std as of 1.88; go through libc.
    let name = CString::new("pincery-init-policy-test").unwrap();
    // SAFETY: libc FFI with owned nul-terminated name; return value
    // is validated below.
    let fd = unsafe { libc::memfd_create(name.as_ptr(), 0) };
    assert!(
        fd >= 0,
        "memfd_create failed: {}",
        std::io::Error::last_os_error()
    );

    // Wrap as File (takes ownership) so we get Write + automatic
    // close on drop if something below panics before we transfer
    // ownership back out.
    // SAFETY: we just created `fd`; no other owner exists.
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    file.write_all(bytes).expect("write policy bytes");
    // Rewind so the wrapper reads from offset 0.
    let raw = file.into_raw_fd();
    let rc = unsafe { libc::lseek(raw, 0, libc::SEEK_SET) };
    assert_eq!(
        rc,
        0,
        "lseek to 0 failed: {}",
        std::io::Error::last_os_error()
    );
    raw
}

/// Build a minimal but realistic policy payload. G0a.2 doesn't
/// interpret any of these fields; it just round-trips the bytes.
fn sample_policy(user_argv: Vec<String>) -> SandboxInitPolicy {
    SandboxInitPolicy {
        landlock_rx_paths: vec![PathBuf::from("/usr"), PathBuf::from("/bin")],
        landlock_rwx_paths: vec![PathBuf::from("/tmp")],
        seccomp_bpf: vec![0x06, 0x00, 0x00, 0x00],
        target_uid: 65534,
        target_gid: 65534,
        require_fully_enforced: false,
        user_argv,
    }
}

/// Slice G0a.2 proof: hand a memfd-backed policy to the wrapper and
/// confirm that (a) the user binary runs with its expected stdout,
/// (b) exit code is 0, (c) stderr shows the policy was parsed.
#[test]
fn skeleton_parses_policy_then_execs_user_argv() {
    // Prove on-box we actually have /bin/sh before we rely on it.
    // If we don't, skip with a clear reason rather than fail — the
    // CI sandbox devshell guarantees /bin/sh; local dev might not.
    if !PathBuf::from("/bin/sh").exists() {
        eprintln!("skipping: /bin/sh not present on this host");
        return;
    }

    let user_argv = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "printf hello".to_string(),
    ];
    let policy = sample_policy(user_argv);
    let bytes = policy.to_bytes().expect("serialize policy");

    let policy_fd = make_policy_memfd(&bytes);

    let mut cmd = Command::new(pincery_init_bin());
    cmd.args(["--policy-fd", "3", "--", "/bin/sh", "-c", "printf hello"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // dup2 the memfd onto fd 3 in the child before exec. dup2 clears
    // CLOEXEC on the destination fd, so fd 3 survives execvp inside
    // the wrapper binary.
    unsafe {
        cmd.pre_exec(move || {
            if libc::dup2(policy_fd, 3) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let output = cmd.output().expect("spawn pincery-init");

    assert!(
        output.status.success(),
        "pincery-init failed: status={:?} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
    assert_eq!(
        output.stdout,
        b"hello",
        "user argv (/bin/sh -c 'printf hello') should have produced 'hello' on stdout; \
         stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );

    // Stderr must show the policy was parsed — the summary line is
    // the single structured log statement G0a.2 emits. This is the
    // observability contract the four-case G0a.3 suite builds on.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("pincery-init: parsed policy"),
        "expected policy-summary line on stderr, got:\n{stderr}",
    );
    assert!(
        stderr.contains("rx_paths=2"),
        "summary should reflect the 2 rx paths we supplied, got:\n{stderr}",
    );
    assert!(
        stderr.contains("target_uid=65534"),
        "summary should reflect target_uid=65534, got:\n{stderr}",
    );
}

/// Negative case: garbage on the policy fd must exit 125 and surface
/// a decoding error on stderr. This pins the fail-fast contract the
/// parent depends on: any corruption of the IPC channel is loud, not
/// silent.
#[test]
fn skeleton_rejects_garbage_policy_with_exit_125() {
    let garbage = b"\xff\xfe\xfd\xfc not valid json";
    let policy_fd = make_policy_memfd(garbage);

    let mut cmd = Command::new(pincery_init_bin());
    cmd.args(["--policy-fd", "3", "--", "/bin/true"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        cmd.pre_exec(move || {
            if libc::dup2(policy_fd, 3) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let output = cmd.output().expect("spawn pincery-init");

    assert_eq!(
        output.status.code(),
        Some(125),
        "garbage policy should exit 125, got status={:?} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("decoding policy") || stderr.contains("codec"),
        "stderr should name the decode failure, got:\n{stderr}",
    );
}

/// Negative case: missing `--policy-fd` must exit 125 with a usage
/// error. Guards against a future refactor that silently accepts a
/// wrapper call with no policy.
#[test]
fn skeleton_rejects_missing_policy_fd_flag() {
    let output = Command::new(pincery_init_bin())
        .args(["--", "/bin/true"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn pincery-init");

    assert_eq!(output.status.code(), Some(125));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("usage error") || stderr.contains("--policy-fd"),
        "expected usage error on stderr, got:\n{stderr}",
    );
}
