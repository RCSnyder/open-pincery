//! AC-83 integration proof: the `pincery-init` wrapper parses an
//! inherited policy fd, applies restrictions, and `execvp`s the user
//! argv.
//!
//! Coverage by slice:
//!
//! - G0a.2 (shipped): parse + log + exec, no restrictions.
//! - G0a.3a (shipped): `prctl(PR_SET_NO_NEW_PRIVS, 1)` before exec
//!   — proved via `/proc/self/status`.
//! - G0a.3b (shipped): drop uid/gid via
//!   `setresgid -> setgroups(0, NULL) -> setresuid`, short-circuiting
//!   when already at target. Proved via stderr short-circuit log +
//!   `id -u`/`id -g` inside the user program.
//! - G0a.3c (shipped): install seccomp filter via
//!   `prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER)`. Proved via
//!   `/proc/self/status`'s `Seccomp: 2` line + a misaligned-bytes
//!   negative case.
//! - G0a.3d (shipped): install landlock filesystem ruleset via
//!   `runtime::sandbox::landlock_layer::install_landlock`. Proved
//!   by observing that a write to a path OUTSIDE
//!   `policy.landlock_rwx_paths` fails while a write INSIDE
//!   succeeds.
//! - G0a.3e (this slice): when `policy.require_fully_enforced` is
//!   true, verify landlock `FullyEnforced` + seccomp filter + NNP.
//!   Proved by (a) a happy-path policy with a real landlock install
//!   that succeeds and execs cleanly, (b) a negative case that arms
//!   `OPEN_PINCERY_INIT_FORCE_PARTIAL=1` alongside
//!   `OPEN_PINCERY_ALLOW_UNSAFE=true` to downgrade the observed
//!   status to `PartiallyEnforced`, expecting exit 125 with a
//!   descriptive stderr line.
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

/// A minimal valid seccomp filter: one instruction that returns
/// `SECCOMP_RET_ALLOW` unconditionally. Serialized as a single
/// `struct sock_filter` (8 bytes): `code=BPF_RET|BPF_K (0x06)`,
/// `jt=0`, `jf=0`, `k=SECCOMP_RET_ALLOW (0x7fff0000)`.
///
/// This is what the G0a.3c integration test feeds the wrapper: it
/// installs cleanly (proves the apply path and `/proc/self/status`
/// verify work), never kills the user program (so the test binary
/// can read `/proc` and print), and is byte-for-byte identical to
/// what a real `seccompiler` run would emit for a trivial allow.
fn allow_all_seccomp_bytes() -> Vec<u8> {
    // code (u16 LE) | jt (u8) | jf (u8) | k (u32 LE)
    // 0x0006         | 0x00    | 0x00    | 0x7fff0000
    vec![0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0x7f]
}

/// Build a minimal but realistic policy payload. G0a.2 doesn't
/// interpret any of these fields; it just round-trips the bytes.
///
/// Starting in G0a.3b the wrapper actively drops privileges to
/// `target_uid`/`target_gid`. Host integration tests cannot obtain
/// `CAP_SETUID`, so we set the target to the **current** euid/egid.
/// The wrapper's `apply_drop_privs` short-circuits when already at
/// target, which exercises the full code path without requiring
/// privileges. The real bwrap path (G0a.3g) runs the wrapper as
/// namespace-root and drops to an unprivileged uid there.
///
/// Starting in G0a.3c `seccomp_bpf` must be a valid `sock_filter[]`
/// byte stream (multiple of 8). `allow_all_seccomp_bytes()` gives us
/// a one-instruction allow filter that verifies cleanly via
/// `/proc/self/status` without blocking the user program.
///
/// Starting in G0a.3d the wrapper actively installs a landlock
/// filesystem ruleset derived from the path vectors, so the lists
/// here MUST be sufficient for every downstream operation:
///
/// - Rootfs rx: `/usr`, `/bin`, `/lib`, `/lib64`, `/etc`, `/sys` so
///   `/bin/sh`, `/bin/true`, `grep`, `id`, etc. can load shared
///   libraries and read locale / nsswitch config.
/// - `/proc` as rwx: `apply_seccomp` reads `/proc/self/status` to
///   verify the filter, and test user programs read `/proc` too.
///   /proc needs write access for things like
///   `/proc/self/oom_score_adj`; keeping it rwx is defense-in-depth.
/// - `/tmp` as rwx: a handful of standard unix tools touch /tmp
///   during startup; keeping it writable avoids flaky failures on
///   the runner without adding relevant surface (no untrusted code
///   runs under these tests).
///
/// The dedicated landlock test (G0a.3d) uses a narrower list to
/// actually prove enforcement. `sample_policy` is the "everything
/// the tests need to run" list.
fn sample_policy(user_argv: Vec<String>) -> SandboxInitPolicy {
    // SAFETY: pure getters.
    let cur_uid = unsafe { libc::geteuid() };
    let cur_gid = unsafe { libc::getegid() };
    SandboxInitPolicy {
        landlock_rx_paths: vec![
            PathBuf::from("/usr"),
            PathBuf::from("/bin"),
            PathBuf::from("/lib"),
            PathBuf::from("/lib64"),
            PathBuf::from("/etc"),
            PathBuf::from("/sys"),
        ],
        landlock_rwx_paths: vec![PathBuf::from("/proc"), PathBuf::from("/tmp")],
        seccomp_bpf: allow_all_seccomp_bytes(),
        target_uid: cur_uid,
        target_gid: cur_gid,
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
        stderr.contains("rx_paths=6"),
        "summary should reflect the 6 rx paths `sample_policy` supplies, got:\n{stderr}",
    );
    // SAFETY: pure getter.
    let cur_uid = unsafe { libc::geteuid() };
    assert!(
        stderr.contains(&format!("target_uid={cur_uid}")),
        "summary should reflect target_uid={cur_uid} (matches current \
         euid for host-test drop short-circuit), got:\n{stderr}",
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

/// Slice G0a.3a proof: before exec, the wrapper must apply
/// `prctl(PR_SET_NO_NEW_PRIVS, 1)`. This is step 1 of the T-G0a-6
/// pipeline and the prerequisite for the unprivileged seccomp filter
/// load that lands in G0a.3c. The user program observes the flag via
/// `/proc/self/status`'s `NoNewPrivs:` line (always present on any
/// kernel the wrapper targets).
///
/// Why this works as a proof: `/proc/self/status` reads are populated
/// by the kernel from the task's thread_info at the moment of the
/// open, so there is no way to fake this from userspace. If
/// `NoNewPrivs:\t1` appears, `PR_SET_NO_NEW_PRIVS` was honored before
/// `execvp`.
#[test]
fn skeleton_applies_no_new_privs_before_exec() {
    if !PathBuf::from("/bin/sh").exists() {
        eprintln!("skipping: /bin/sh not present on this host");
        return;
    }

    // The user argv reads its own no_new_privs bit out of /proc and
    // echoes the matching status line to stdout. `grep` exits 0 if it
    // matched, 1 otherwise — we assert on exit 0 AND the stdout
    // content so we catch both "flag missing entirely" and "flag
    // present but 0".
    let user_argv = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "grep ^NoNewPrivs: /proc/self/status".to_string(),
    ];
    let policy = sample_policy(user_argv);
    let bytes = policy.to_bytes().expect("serialize policy");

    let policy_fd = make_policy_memfd(&bytes);

    let mut cmd = Command::new(pincery_init_bin());
    cmd.args([
        "--policy-fd",
        "3",
        "--",
        "/bin/sh",
        "-c",
        "grep ^NoNewPrivs: /proc/self/status",
    ])
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
    assert!(
        output.status.success(),
        "pincery-init failed: status={:?} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    // `/proc/self/status` uses a tab between the label and value:
    // `NoNewPrivs:\t1\n`. Match both the `1` suffix and the prefix so
    // a stray `NoNewPrivs: 0` can't sneak through on mismatched
    // whitespace.
    assert!(
        stdout.contains("NoNewPrivs:\t1") || stdout.contains("NoNewPrivs: 1"),
        "expected NoNewPrivs=1 in /proc/self/status after wrapper apply; got:\n{stdout}",
    );
    assert!(
        !stdout.contains("NoNewPrivs:\t0") && !stdout.contains("NoNewPrivs: 0"),
        "NoNewPrivs is 0 — prctl(PR_SET_NO_NEW_PRIVS) did not take effect before exec; \
         stdout:\n{stdout}",
    );
}

/// Slice G0a.3b proof: before exec, the wrapper must drop real/
/// effective/saved uid+gid to `policy.target_uid`/`policy.target_gid`
/// via `setresgid -> setgroups(0, NULL) -> setresuid`. Host tests
/// cannot obtain `CAP_SETUID`, so `sample_policy` uses the current
/// euid/egid and the wrapper short-circuits. We assert two things:
///
/// 1. The short-circuit log line appears on stderr (proves the code
///    path ran and saw the matching target).
/// 2. `id -u` / `id -g` inside the user program report the expected
///    uid/gid (proves the wrapper did NOT accidentally change them).
///
/// The bwrap-path proof (actually dropping from namespace-root to
/// an unprivileged uid) lands in the G0a.3g integration.
#[test]
fn skeleton_short_circuits_drop_when_already_at_target() {
    if !PathBuf::from("/bin/sh").exists() {
        eprintln!("skipping: /bin/sh not present on this host");
        return;
    }

    // SAFETY: pure getters.
    let cur_uid = unsafe { libc::geteuid() };
    let cur_gid = unsafe { libc::getegid() };

    let user_argv = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "printf '%s %s' \"$(id -u)\" \"$(id -g)\"".to_string(),
    ];
    let policy = sample_policy(user_argv);
    let bytes = policy.to_bytes().expect("serialize policy");

    let policy_fd = make_policy_memfd(&bytes);

    let mut cmd = Command::new(pincery_init_bin());
    cmd.args([
        "--policy-fd",
        "3",
        "--",
        "/bin/sh",
        "-c",
        "printf '%s %s' \"$(id -u)\" \"$(id -g)\"",
    ])
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
    assert!(
        output.status.success(),
        "pincery-init failed: status={:?} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("drop_privs short-circuit"),
        "expected drop_privs short-circuit log line (target matches \
         current euid/egid); stderr:\n{stderr}",
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = format!("{cur_uid} {cur_gid}");
    assert_eq!(
        stdout.trim(),
        expected,
        "user program's id output should match current euid/egid (the \
         wrapper short-circuited the drop); got stdout={stdout:?} \
         stderr={stderr}",
    );
}

/// Slice G0a.3c proof: before exec, the wrapper must install the
/// seccomp filter supplied in `policy.seccomp_bpf` via
/// `prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, ...)`. The user
/// program reads its own `/proc/self/status`'s `Seccomp:` line:
///
/// - `0` = disabled (filter not installed)
/// - `1` = strict mode (not what we install)
/// - `2` = filter mode (what the wrapper must produce)
///
/// We assert `Seccomp:\t2` — any other value means the install did
/// not happen, the filter was replaced, or the kernel silently
/// downgraded us.
///
/// The user program runs `/bin/sh` which in turn runs `grep` + reads
/// `/proc`. For those to all succeed under our filter, the filter
/// must allow every syscall those binaries make. We use a trivial
/// `SECCOMP_RET_ALLOW` one-instruction program (`allow_all_seccomp_bytes`)
/// — real production policies go through `seccompiler`'s allowlist
/// compile; that path is covered by separate unit tests for the
/// `runtime::sandbox::seccomp` module.
#[test]
fn skeleton_installs_seccomp_filter_before_exec() {
    if !PathBuf::from("/bin/sh").exists() {
        eprintln!("skipping: /bin/sh not present on this host");
        return;
    }

    let user_argv = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "grep ^Seccomp: /proc/self/status".to_string(),
    ];
    let policy = sample_policy(user_argv);
    let bytes = policy.to_bytes().expect("serialize policy");

    let policy_fd = make_policy_memfd(&bytes);

    let mut cmd = Command::new(pincery_init_bin());
    cmd.args([
        "--policy-fd",
        "3",
        "--",
        "/bin/sh",
        "-c",
        "grep ^Seccomp: /proc/self/status",
    ])
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
    assert!(
        output.status.success(),
        "pincery-init failed: status={:?} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Seccomp:\t2") || stdout.contains("Seccomp: 2"),
        "expected Seccomp=2 (filter mode) in /proc/self/status after \
         wrapper install; got:\n{stdout}",
    );
    // Defense against the filter getting silently unset or downgraded.
    for bad in ["Seccomp:\t0", "Seccomp: 0", "Seccomp:\t1", "Seccomp: 1"] {
        assert!(
            !stdout.contains(bad),
            "/proc/self/status shows {bad}, meaning the filter was \
             never installed or was downgraded; stdout:\n{stdout}",
        );
    }
}

/// Slice G0a.3c negative case: a seccomp_bpf payload whose length is
/// not a multiple of `sizeof(struct sock_filter)` must fail-closed
/// with exit 125 — NOT silently truncate or install a bogus filter.
/// This is the pre-kernel guard; the kernel would also reject it,
/// but failing early with a clear error keeps the blast radius
/// observable in the wrapper's own stderr.
#[test]
fn skeleton_rejects_misaligned_seccomp_bpf() {
    if !PathBuf::from("/bin/true").exists() {
        eprintln!("skipping: /bin/true not present on this host");
        return;
    }

    let mut policy = sample_policy(vec!["/bin/true".to_string()]);
    // 7 bytes — not a multiple of 8.
    policy.seccomp_bpf = vec![1, 2, 3, 4, 5, 6, 7];
    let bytes = policy.to_bytes().expect("serialize policy");

    let policy_fd = make_policy_memfd(&bytes);

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
        "misaligned seccomp_bpf should exit 125, got status={:?} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("sock_filter") || stderr.contains("seccomp_bpf length"),
        "stderr should name the seccomp alignment failure, got:\n{stderr}",
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

/// Slice G0a.3d proof: before exec, the wrapper must install a
/// landlock filesystem ruleset derived from the policy's rx + rwx
/// path lists. After install, the forked user program inherits the
/// landlock domain (per the kernel's landlock userspace API docs,
/// section on inheritance), so writes to paths outside the rwx list
/// must fail with `EACCES`.
///
/// We assert both sides of the ruleset:
/// 1. `touch <workspace>/allowed` succeeds (workspace is in rwx).
/// 2. `touch /tmp/pincery-g0a3d-forbidden-<pid>` fails (/tmp is NOT
///    in rwx — only the specific workspace path is).
///
/// Why running the wrapper directly on the host is a valid proof:
/// landlock is a kernel LSM that restricts any task with an active
/// domain. We don't need bwrap + namespaces to prove enforcement —
/// we just need a kernel >= 5.13 (all supported CI runners and
/// modern Docker Desktop images satisfy this). The real bwrap path
/// (G0a.3g) stitches this wrapper into the `--ro-bind` + memfd
/// plumbing; the ruleset semantics proved here are unchanged.
#[test]
fn skeleton_installs_landlock_restricts_fs_before_exec() {
    if !PathBuf::from("/bin/sh").exists() {
        eprintln!("skipping: /bin/sh not present on this host");
        return;
    }
    // Kernel must support landlock. On unsupported kernels
    // `install_landlock` returns Err and the wrapper fails with
    // exit 125 — that's the correct fail-closed behavior, but the
    // test's expected observable is successful exit + blocked write,
    // so skip cleanly.
    if !open_pincery::runtime::sandbox::landlock_layer::landlock_supported() {
        eprintln!("skipping: landlock not supported by this kernel");
        return;
    }

    let workspace = tempfile::tempdir().expect("create workspace tempdir");
    let forbidden =
        std::env::temp_dir().join(format!("pincery-g0a3d-forbidden-{}", std::process::id()));
    // Scrub any stale leftover from a previous run.
    let _ = std::fs::remove_file(&forbidden);

    // Allowed paths for the user program: standard rootfs rx plus
    // /proc as rwx (apply_seccomp reads /proc/self/status after
    // landlock applies) plus the workspace. /sys is rx so the
    // C library's various probe paths don't trip an unrelated
    // failure.
    let mut policy = sample_policy(vec![]);
    policy.landlock_rx_paths = vec![
        PathBuf::from("/usr"),
        PathBuf::from("/bin"),
        PathBuf::from("/lib"),
        PathBuf::from("/lib64"),
        PathBuf::from("/etc"),
        PathBuf::from("/sys"),
    ];
    policy.landlock_rwx_paths = vec![PathBuf::from("/proc"), workspace.path().to_path_buf()];

    // Shell script: attempt write inside rwx, then write outside
    // rwx. Print which half blocked. We use `||` so the second
    // touch's non-zero exit does not fail the whole script —
    // landlock returns EACCES, which sh surfaces as exit 1, which
    // the `||` branch catches.
    let script = format!(
        "touch {workspace}/allowed && ( touch {forbidden} 2>/dev/null && echo LEAKED || echo BLOCKED )",
        workspace = workspace.path().display(),
        forbidden = forbidden.display(),
    );
    policy.user_argv = vec!["/bin/sh".to_string(), "-c".to_string(), script.clone()];

    let bytes = policy.to_bytes().expect("serialize policy");
    let policy_fd = make_policy_memfd(&bytes);

    let mut cmd = Command::new(pincery_init_bin());
    cmd.args(["--policy-fd", "3", "--", "/bin/sh", "-c", &script])
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
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "pincery-init failed: status={:?} stdout={stdout} stderr={stderr}",
        output.status,
    );
    assert!(
        stdout.contains("BLOCKED"),
        "expected landlock to block write to {} (which is outside the \
         rwx_paths set); stdout={stdout} stderr={stderr}",
        forbidden.display(),
    );
    assert!(
        !stdout.contains("LEAKED"),
        "write to forbidden path succeeded — landlock did NOT enforce; \
         stdout={stdout} stderr={stderr}",
    );
    assert!(
        !forbidden.exists(),
        "forbidden path {} exists on disk; landlock did not block",
        forbidden.display(),
    );
    assert!(
        workspace.path().join("allowed").exists(),
        "write to rwx workspace {} failed — rwx rule not granting \
         access; stderr={stderr}",
        workspace.path().display(),
    );
}

/// Slice G0a.3e happy path: when `require_fully_enforced=true`, a
/// policy that actually installs landlock + seccomp + NNP must pass
/// the final verify and reach exec. The observable is successful
/// exit + expected stdout from the user program.
#[test]
fn skeleton_fully_enforced_passes_when_all_layers_enforce() {
    if !PathBuf::from("/bin/sh").exists() {
        eprintln!("skipping: /bin/sh not present on this host");
        return;
    }
    if !open_pincery::runtime::sandbox::landlock_layer::landlock_supported() {
        eprintln!("skipping: landlock not supported by this kernel");
        return;
    }

    let mut policy = sample_policy(vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "printf fully-enforced-ok".to_string(),
    ]);
    policy.require_fully_enforced = true;

    let bytes = policy.to_bytes().expect("serialize policy");
    let policy_fd = make_policy_memfd(&bytes);

    let mut cmd = Command::new(pincery_init_bin());
    cmd.args([
        "--policy-fd",
        "3",
        "--",
        "/bin/sh",
        "-c",
        "printf fully-enforced-ok",
    ])
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
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "pincery-init failed under require_fully_enforced=true: \
         status={:?} stdout={stdout} stderr={stderr}",
        output.status,
    );
    assert_eq!(
        stdout.trim(),
        "fully-enforced-ok",
        "user program did not run (verify rejected or exec failed); \
         stderr={stderr}",
    );
}

/// Slice G0a.3e negative path: when `require_fully_enforced=true`
/// and we arm the unsafe test knob
/// (`OPEN_PINCERY_ALLOW_UNSAFE=true` + `OPEN_PINCERY_INIT_FORCE_PARTIAL=1`)
/// to downgrade the observed landlock status to `PartiallyEnforced`,
/// the wrapper must fail closed with exit 125 and name the failure
/// on stderr.
///
/// This proves the rejection path runs end-to-end without requiring
/// a kernel that actually returns `PartiallyEnforced` — the override
/// gate is deliberately two env vars so it cannot arm in production
/// (see the `apply_landlock` docstring).
#[test]
fn skeleton_fully_enforced_rejects_partial_landlock() {
    if !PathBuf::from("/bin/true").exists() {
        eprintln!("skipping: /bin/true not present on this host");
        return;
    }
    if !open_pincery::runtime::sandbox::landlock_layer::landlock_supported() {
        eprintln!("skipping: landlock not supported by this kernel");
        return;
    }

    let mut policy = sample_policy(vec!["/bin/true".to_string()]);
    policy.require_fully_enforced = true;

    let bytes = policy.to_bytes().expect("serialize policy");
    let policy_fd = make_policy_memfd(&bytes);

    let mut cmd = Command::new(pincery_init_bin());
    cmd.args(["--policy-fd", "3", "--", "/bin/true"])
        .env("OPEN_PINCERY_ALLOW_UNSAFE", "true")
        .env("OPEN_PINCERY_INIT_FORCE_PARTIAL", "1")
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
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(125),
        "forced PartiallyEnforced should fail closed under \
         require_fully_enforced=true; status={:?} stderr={stderr}",
        output.status,
    );
    assert!(
        stderr.contains("FullyEnforced") || stderr.contains("PartiallyEnforced"),
        "stderr should name the FullyEnforced verify failure, got:\n{stderr}",
    );
    assert!(
        stderr.contains("verifying policy") || stderr.contains("VerifyPolicy"),
        "stderr should surface this as a verify-stage failure, got:\n{stderr}",
    );
}
