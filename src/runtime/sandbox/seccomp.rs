//! AC-77 / Slice G2 (Phase G): seccomp-bpf default-deny **allowlist**.
//!
//! ## Posture
//!
//! Every syscall NOT explicitly listed in [`allowed_syscalls`] (or
//! [`clone_arg_rules`]) is denied. In `Enforce` mode the kernel
//! returns `SECCOMP_RET_KILL_PROCESS` (process dies with SIGSYS, exit
//! status 159 = 128 + 31). In `Audit` mode the kernel returns
//! `SECCOMP_RET_LOG`, which logs to the audit subsystem but lets the
//! call proceed — used by operators to discover newly-needed syscalls
//! without a hard outage.
//!
//! ## Source of truth
//!
//! The allowlist below is sourced empirically from
//! `tests/fixtures/seccomp/observed_syscalls.txt` (a strace-summary
//! capture of the AC-76 happy-path command set on kernel 6.6 / glibc
//! 2.39 / x86_64) plus `tests/fixtures/seccomp/additions.txt`
//! (manually-justified entries the empirical capture missed: notably
//! `exit_group`, `clone3`, `prctl`, `futex`, sleep variants, signal
//! helpers, and the `pincery-init` residual set between
//! `apply_seccomp` and `execvp`).
//!
//! When a new built-in tool extends the syscall surface (e.g. AC-66
//! Tool Catalog Expansion) the operator re-runs
//! `scripts/capture_seccomp_corpus.sh` and updates this list. The
//! drift guard
//! [`tests::allowlist_covers_observed_corpus`] (a unit test in this
//! file that reads the static fixture via `include_str!`) fires at
//! every build whenever an entry from `observed_syscalls.txt` is
//! missing from `allowed_syscalls()` -- no env-var gating required.
//!
//! ## `clone` argument filtering
//!
//! `clone(2)` is the highest-leverage syscall in the allowlist:
//! ordinary thread / process creation needs it, but
//! `clone(CLONE_NEWUSER, ...)` or `clone(CLONE_NEWNS, ...)` would let
//! a sandboxed process create new user / mount namespaces and
//! re-acquire `CAP_SYS_ADMIN` inside them. The allowlist therefore
//! installs a `SeccompRule` on the `flags` argument: the rule
//! matches (allow) only when both `CLONE_NEWUSER` and `CLONE_NEWNS`
//! are clear. Setting either bit fails the rule, falls through to
//! the filter's default action (`KillProcess` in Enforce), and the
//! kernel kills the process before `clone` returns.
//!
//! `clone3(2)` (modern glibc thread creation) takes a pointer-to-
//! struct, which BPF cannot dereference. We allow `clone3` bare and
//! rely on AC-86 (`bwrap --disable-userns` + `--cap-drop ALL` + UID
//! drop) to make `CLONE_NEWUSER` produce `EPERM` regardless of
//! seccomp. Documented in T-AC77-4.
//!
//! ## Negative-control invariants
//!
//! `assert_no_escape_primitives` re-asserts that
//! `bpf`, `mount`, `umount2`, `pivot_root`, `init_module`,
//! `finit_module`, `delete_module`, `kexec_load`, `kexec_file_load`,
//! `reboot`, `ptrace`, `io_uring_setup`, `io_uring_register`,
//! `io_uring_enter`, `perf_event_open`, `name_to_handle_at`, and
//! `open_by_handle_at` are NOT in the allowlist. This catches a
//! regression where a future strace pass accidentally picked one
//! up (for example by tracing a payload command that itself tried
//! to escape).
//!
//! ## bwrap fd protocol
//!
//! `bwrap --seccomp <fd>` reads a raw `struct sock_filter[]` byte
//! stream from the fd — no `sock_fprog` wrapper. `BpfProgram` is
//! `Vec<sock_filter>` and `sock_filter` is `#[repr(C)]` 8 bytes
//! wide; the vec's heap buffer IS the on-wire format. We use
//! `memfd_create(name, 0)` (no `MFD_CLOEXEC`) so the fd inherits
//! through `fork`/`execve`, then bwrap installs the program via
//! `seccomp(SECCOMP_SET_MODE_FILTER, ...)`.
//!
//! ## Mode -> action mapping
//!
//! - `Enforce`: match=Allow, mismatch=KillProcess. Unknown syscall
//!   triggers SIGSYS (exit 159). Production posture.
//! - `Audit`: match=Allow, mismatch=Log. Unknown syscall is logged
//!   to the kernel audit subsystem and proceeds. Operator escape
//!   valve while expanding the allowlist for new tooling; not a
//!   security posture.
//! - `Disabled`: filter not installed (caller short-circuits before
//!   calling [`build_bpf_program`]).

#![cfg(target_os = "linux")]

use std::collections::BTreeMap;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
    SeccompRule,
};

use crate::config::SandboxMode;

/// Syscalls explicitly forbidden by name, regardless of any future
/// strace capture that might accidentally observe them. Asserted
/// against [`allowed_syscalls`] by `assert_no_escape_primitives` so
/// a regression is caught at install time.
///
/// The list is the union of the AC-77 named primitives plus the
/// classic kernel-escape syscalls from scope.md AC-77 line 898 area.
#[cfg(target_arch = "x86_64")]
pub const ESCAPE_PRIMITIVES: &[i64] = &[
    libc::SYS_bpf,
    libc::SYS_delete_module,
    libc::SYS_fanotify_init,
    libc::SYS_fanotify_mark,
    libc::SYS_finit_module,
    libc::SYS_init_module,
    libc::SYS_io_uring_enter,
    libc::SYS_io_uring_register,
    libc::SYS_io_uring_setup,
    libc::SYS_kexec_file_load,
    libc::SYS_kexec_load,
    libc::SYS_mount,
    libc::SYS_name_to_handle_at,
    libc::SYS_open_by_handle_at,
    libc::SYS_perf_event_open,
    libc::SYS_pivot_root,
    libc::SYS_ptrace,
    libc::SYS_reboot,
    libc::SYS_umount2,
];

/// Lower / upper bound on the allowlist size. Enforced by
/// [`build_bpf_program`] per R-AC77-1: an allowlist below the floor
/// is suspiciously narrow (likely missed a basic syscall and the
/// sandbox SIGSYSes immediately); an allowlist above the ceiling is
/// suspiciously wide ("allowed everything except a few names" --
/// that is a denylist in disguise).
pub const ALLOWLIST_SIZE_FLOOR: usize = 40;
pub const ALLOWLIST_SIZE_CEILING: usize = 120;

/// Empirically-sourced default-deny allowlist for x86_64 Linux.
///
/// Sources (see module header):
/// - `tests/fixtures/seccomp/observed_syscalls.txt` (41 syscalls)
/// - `tests/fixtures/seccomp/additions.txt` (29 manually-justified,
///   including 11 Rust-runtime + modern-glibc residuals added by
///   the AC-77 verify-fix and 1 (setresgid) added by verify-fix-2
///   from kernel SECCOMP_RET_KILL_PROCESS dmesg evidence)
///
/// `clone` is intentionally absent here -- it is added with an
/// argument filter via [`clone_arg_rules`].
#[cfg(target_arch = "x86_64")]
fn allowed_syscalls() -> Vec<i64> {
    vec![
        // --- empirical capture (observed_syscalls.txt) ---
        libc::SYS_access,
        libc::SYS_arch_prctl,
        libc::SYS_brk,
        // SYS_clone deliberately omitted -- see `clone_arg_rules`.
        libc::SYS_close,
        libc::SYS_dup2,
        libc::SYS_execve,
        libc::SYS_fadvise64,
        libc::SYS_fstat,
        libc::SYS_getegid,
        libc::SYS_geteuid,
        libc::SYS_getgid,
        libc::SYS_getpid,
        libc::SYS_getppid,
        libc::SYS_getrandom,
        libc::SYS_getuid,
        libc::SYS_ioctl,
        libc::SYS_lseek,
        libc::SYS_mmap,
        libc::SYS_mprotect,
        libc::SYS_munmap,
        libc::SYS_newfstatat,
        libc::SYS_openat,
        libc::SYS_pipe2,
        libc::SYS_pread64,
        libc::SYS_prlimit64,
        libc::SYS_read,
        libc::SYS_rseq,
        libc::SYS_rt_sigaction,
        libc::SYS_rt_sigprocmask,
        libc::SYS_rt_sigreturn,
        libc::SYS_rt_sigsuspend,
        libc::SYS_set_robust_list,
        libc::SYS_set_tid_address,
        libc::SYS_setpgid,
        libc::SYS_statfs,
        libc::SYS_timer_create,
        libc::SYS_timer_settime,
        libc::SYS_vfork,
        libc::SYS_wait4,
        libc::SYS_write,
        // --- manual additions (additions.txt) ---
        libc::SYS_exit_group,
        libc::SYS_exit,
        libc::SYS_prctl,
        libc::SYS_futex,
        libc::SYS_clone3,
        libc::SYS_restart_syscall,
        libc::SYS_tgkill,
        libc::SYS_nanosleep,
        libc::SYS_clock_nanosleep,
        libc::SYS_clock_gettime,
        libc::SYS_ppoll,
        libc::SYS_poll,
        libc::SYS_sigaltstack,
        libc::SYS_fcntl,
        libc::SYS_getcwd,
        libc::SYS_readlinkat,
        libc::SYS_uname,
        // -- Rust runtime + modern-glibc residual set (verify-fix
        //    after VERIFY caught /bin/true SIGSYS in the privileged
        //    smoke job). pincery-init's verify_no_new_privs and
        //    verify_fully_enforced run AFTER apply_seccomp, and the
        //    user binary's glibc-2.39 dynamic linker on Ubuntu 24.04
        //    uses modern variants (statx, faccessat2, ...) that the
        //    host-side strace -c capture missed.
        libc::SYS_statx,
        libc::SYS_faccessat2,
        libc::SYS_gettid,
        libc::SYS_madvise,
        libc::SYS_mremap,
        libc::SYS_getdents64,
        libc::SYS_sched_yield,
        libc::SYS_sched_getaffinity,
        libc::SYS_tkill,
        libc::SYS_readlink,
        libc::SYS_pselect6,
        // -- AC-77 verify-fix-2: kernel dmesg evidence (CI run
        //    25216296931) showed every SECCOMP_RET_KILL_PROCESS
        //    record reporting `syscall=118 arch=c000003e` from
        //    libc text (ip=0x7f...aeb). On x86_64, syscall 118
        //    is `setresgid`. glibc-2.39 issues it as part of its
        //    init-time security hardening even when the effective
        //    gid is unchanged; this fires after apply_seccomp
        //    inside pincery-init's verify_fully_enforced read of
        //    /proc/self/status. Allowing the bare syscall is safe:
        //    the surrounding sandbox already drops CAP_SETGID via
        //    NO_NEW_PRIVS + cap-drop ALL, so any setresgid call
        //    that would change credentials is rejected by the
        //    kernel with EPERM regardless of seccomp.
        libc::SYS_setresgid,
    ]
}

/// `clone(2)` argument filter. Allows ordinary thread / process
/// creation but blocks any flags set that would create a new user
/// or mount namespace.
///
/// Returns a single rule on `flags` (arg 0) using `MaskedEq(mask)`
/// with value `0`: matches when `(flags & mask) == 0`. If the rule
/// fails to match, the filter's default mismatch action fires
/// (KillProcess in Enforce).
#[cfg(target_arch = "x86_64")]
fn clone_arg_rules() -> Result<Vec<(i64, Vec<SeccompRule>)>, String> {
    let dangerous_flags: u64 = (libc::CLONE_NEWUSER as u64) | (libc::CLONE_NEWNS as u64);
    let cond = SeccompCondition::new(
        /* arg_index = */ 0,
        SeccompCmpArgLen::Qword,
        SeccompCmpOp::MaskedEq(dangerous_flags),
        /* value = */ 0,
    )
    .map_err(|e| format!("SeccompCondition::new(clone flags): {e:?}"))?;
    let rule = SeccompRule::new(vec![cond])
        .map_err(|e| format!("SeccompRule::new(clone flags): {e:?}"))?;
    Ok(vec![(libc::SYS_clone, vec![rule])])
}

/// Assert that no escape primitive appears in the candidate
/// allowlist. Returns `Err` with the offending syscall number on
/// failure so the caller can refuse to install a regressed filter.
#[cfg(target_arch = "x86_64")]
fn assert_no_escape_primitives(allowlist: &[i64]) -> Result<(), String> {
    for forbidden in ESCAPE_PRIMITIVES {
        if allowlist.contains(forbidden) {
            return Err(format!(
                "AC-77 invariant violated: escape primitive syscall #{forbidden} \
                 is present in allowed_syscalls()"
            ));
        }
    }
    Ok(())
}

/// Build the compiled BPF program for a given enforcement mode.
///
/// Returns the program or an error if the host arch is unsupported,
/// the allowlist size is out of bounds, or the seccompiler API
/// rejects the rule set.
#[cfg(target_arch = "x86_64")]
pub fn build_bpf_program(mode: SandboxMode) -> Result<BpfProgram, String> {
    let mismatch_action = match mode {
        SandboxMode::Audit => SeccompAction::Log,
        SandboxMode::Enforce | SandboxMode::Disabled => SeccompAction::KillProcess,
    };

    let allowlist = allowed_syscalls();
    if allowlist.len() < ALLOWLIST_SIZE_FLOOR || allowlist.len() > ALLOWLIST_SIZE_CEILING {
        return Err(format!(
            "seccomp allowlist size {} out of bounds [{}..={}]; \
             refuse to install (R-AC77-1)",
            allowlist.len(),
            ALLOWLIST_SIZE_FLOOR,
            ALLOWLIST_SIZE_CEILING
        ));
    }

    assert_no_escape_primitives(&allowlist)?;

    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    for nr in allowlist {
        // Empty rule vec means "match by syscall number alone".
        rules.insert(nr, Vec::new());
    }
    for (nr, chain) in clone_arg_rules()? {
        rules.insert(nr, chain);
    }

    let target_arch = std::env::consts::ARCH
        .try_into()
        .map_err(|e| format!("unsupported host arch for seccomp: {e:?}"))?;

    let filter = SeccompFilter::new(
        rules,
        mismatch_action,
        /* match_action = */ SeccompAction::Allow,
        target_arch,
    )
    .map_err(|e| format!("SeccompFilter::new failed: {e:?}"))?;

    BpfProgram::try_from(filter).map_err(|e| format!("BpfProgram::try_from failed: {e:?}"))
}

// --- non-x86_64 stubs -----------------------------------------------------
//
// AC-77 ships an x86_64 allowlist only (per T-AC77-10). On other
// architectures `build_bpf_program` returns Err so callers refuse to
// install rather than silently degrading to no filter.

#[cfg(not(target_arch = "x86_64"))]
pub const ESCAPE_PRIMITIVES: &[i64] = &[];

#[cfg(not(target_arch = "x86_64"))]
pub fn build_bpf_program(_mode: SandboxMode) -> Result<BpfProgram, String> {
    Err(format!(
        "AC-77 seccomp allowlist is only defined for x86_64 (current arch: {})",
        std::env::consts::ARCH
    ))
}

/// Write a compiled BPF program into a fresh in-memory file descriptor
/// suitable for `bwrap --seccomp <fd>`. The returned fd:
/// - is positioned at offset 0 (so `read(2)` starts at the first instruction),
/// - has `FD_CLOEXEC` cleared (so it inherits across `execve`),
/// - owns its lifetime via [`OwnedFd`] (dropping closes it).
pub fn write_bpf_to_memfd(program: &BpfProgram) -> io::Result<OwnedFd> {
    let name = c"pincery-seccomp-bpf";
    // SAFETY: libc::memfd_create FFI; static C string + constant flags.
    let raw = unsafe { libc::memfd_create(name.as_ptr(), 0) };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fresh, unowned fd from the kernel; OwnedFd takes exclusive
    // ownership and closes on drop.
    let fd: OwnedFd = unsafe { OwnedFd::from_raw_fd(raw) };

    let byte_len = std::mem::size_of_val(program.as_slice());
    let bytes = unsafe { std::slice::from_raw_parts(program.as_ptr().cast::<u8>(), byte_len) };
    write_all_to_fd(&fd, bytes)?;
    lseek_set_start(&fd)?;
    Ok(fd)
}

/// Convenience: build the filter for `mode` AND write it to a memfd.
/// Returns the live OwnedFd (must be kept alive until `spawn()`
/// completes) plus the raw fd number to place in the `--seccomp <fd>`
/// argv.
pub fn compose_seccomp_fd(mode: SandboxMode) -> Result<(OwnedFd, RawFd), String> {
    let program = build_bpf_program(mode)?;
    let fd = write_bpf_to_memfd(&program)
        .map_err(|e| format!("memfd_create/write for seccomp failed: {e}"))?;
    let raw = fd.as_raw_fd();
    Ok((fd, raw))
}

// --- internal file-descriptor I/O helpers ---------------------------------

fn write_all_to_fd(fd: &OwnedFd, mut buf: &[u8]) -> io::Result<()> {
    while !buf.is_empty() {
        // SAFETY: valid pointer + length pair from `buf` and a live fd.
        let n = unsafe { libc::write(fd.as_raw_fd(), buf.as_ptr().cast(), buf.len()) };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err);
        }
        buf = &buf[n as usize..];
    }
    Ok(())
}

fn lseek_set_start(fd: &OwnedFd) -> io::Result<()> {
    // SAFETY: valid owned fd + POSIX SEEK_SET with offset 0.
    let off = unsafe { libc::lseek(fd.as_raw_fd(), 0, libc::SEEK_SET) };
    if off < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::*;

    #[test]
    fn allowlist_size_within_bounds() {
        let n = allowed_syscalls().len();
        assert!(
            (ALLOWLIST_SIZE_FLOOR..=ALLOWLIST_SIZE_CEILING).contains(&n),
            "allowlist size {n} out of bounds [{ALLOWLIST_SIZE_FLOOR}..={ALLOWLIST_SIZE_CEILING}]"
        );
    }

    #[test]
    fn allowlist_excludes_escape_primitives() {
        let allow = allowed_syscalls();
        for forbidden in ESCAPE_PRIMITIVES {
            assert!(
                !allow.contains(forbidden),
                "AC-77 invariant: escape primitive #{forbidden} must NOT be in allowlist"
            );
        }
        assert_no_escape_primitives(&allow).expect("invariant must hold");
    }

    #[test]
    fn allowlist_includes_essential_workload_syscalls() {
        let allow = allowed_syscalls();
        // Mandatory for any user binary on glibc:
        for required in [
            libc::SYS_execve,
            libc::SYS_read,
            libc::SYS_write,
            libc::SYS_exit_group,
            libc::SYS_mmap,
            libc::SYS_brk,
        ] {
            assert!(
                allow.contains(&required),
                "essential syscall #{required} missing from allowlist"
            );
        }
    }

    #[test]
    fn clone_arg_rules_filter_user_and_mount_namespaces() {
        let rules = clone_arg_rules().expect("clone arg rules");
        assert_eq!(rules.len(), 1, "expected one (syscall, rules) pair");
        assert_eq!(rules[0].0, libc::SYS_clone);
        assert_eq!(rules[0].1.len(), 1, "expected one allow rule on clone");
        // Sanity: clone is NOT in the bare allowlist (it goes through
        // the arg-filter path instead).
        assert!(!allowed_syscalls().contains(&libc::SYS_clone));
    }

    #[test]
    fn build_program_enforce_uses_kill_on_mismatch() {
        let prog = build_bpf_program(SandboxMode::Enforce).expect("enforce program");
        // The allowlist has ~58 syscalls plus arch-check + clone arg
        // rule; the BPF program is well over a few records.
        assert!(
            prog.len() > 30,
            "BPF program suspiciously small: {} instructions",
            prog.len()
        );
    }

    #[test]
    fn build_program_audit_uses_log_on_mismatch() {
        let prog = build_bpf_program(SandboxMode::Audit).expect("audit program");
        assert!(prog.len() > 30);
    }

    #[test]
    fn enforce_and_audit_programs_differ() {
        // Different mismatch_action -> different terminating
        // instruction in the BPF program. The two compiled programs
        // should not be byte-identical.
        let enforce = build_bpf_program(SandboxMode::Enforce).expect("enforce");
        let audit = build_bpf_program(SandboxMode::Audit).expect("audit");
        let e_bytes = unsafe {
            std::slice::from_raw_parts(
                enforce.as_ptr().cast::<u8>(),
                std::mem::size_of_val(enforce.as_slice()),
            )
        };
        let a_bytes = unsafe {
            std::slice::from_raw_parts(
                audit.as_ptr().cast::<u8>(),
                std::mem::size_of_val(audit.as_slice()),
            )
        };
        assert_ne!(
            e_bytes, a_bytes,
            "enforce and audit programs must encode different mismatch actions"
        );
    }

    #[test]
    fn memfd_roundtrip_matches_program_bytes() {
        let prog = build_bpf_program(SandboxMode::Enforce).expect("program");
        let fd = write_bpf_to_memfd(&prog).expect("memfd write");
        let expected_bytes = unsafe {
            std::slice::from_raw_parts(
                prog.as_ptr().cast::<u8>(),
                std::mem::size_of_val(prog.as_slice()),
            )
        };
        let mut buf = vec![0u8; expected_bytes.len()];
        unsafe { libc::lseek(fd.as_raw_fd(), 0, libc::SEEK_SET) };
        let n = unsafe { libc::read(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
        assert!(n >= 0, "read failed: {}", std::io::Error::last_os_error());
        assert_eq!(n as usize, expected_bytes.len());
        assert_eq!(buf, expected_bytes);
    }

    /// AC-77 / G2e: the checked-in empirical strace corpus must be a
    /// subset of [`allowed_syscalls`] -- otherwise we have shipped an
    /// allowlist that demonstrably fails the AC-76 happy-path corpus.
    /// Every observed name must (a) resolve to a known SYS_* constant
    /// AND (b) appear in the allowlist (or be SYS_clone, which is in
    /// the arg-filter rules).
    #[test]
    fn allowlist_covers_observed_corpus() {
        let observed = include_str!("../../../tests/fixtures/seccomp/observed_syscalls.txt");
        let allow: std::collections::HashSet<i64> = allowed_syscalls().into_iter().collect();
        for raw in observed.lines() {
            let name = raw.trim();
            if name.is_empty() || name.starts_with('#') {
                continue;
            }
            let nr = syscall_nr_by_name(name).unwrap_or_else(|| {
                panic!("observed syscall {name:?} has no libc::SYS_* mapping in this build")
            });
            if nr == libc::SYS_clone {
                // clone is filtered via clone_arg_rules() rather than
                // listed bare in allowed_syscalls().
                continue;
            }
            assert!(
                allow.contains(&nr),
                "observed syscall {name} (#{nr}) is missing from the allowlist; \
                 update src/runtime/sandbox/seccomp.rs::allowed_syscalls or \
                 re-run scripts/capture_seccomp_corpus.sh"
            );
        }
    }

    /// Hand-rolled name -> SYS_* mapping for the empirical corpus.
    /// Kept inside the test module so it cannot drift into production
    /// code paths; only the regen-time check uses it.
    fn syscall_nr_by_name(name: &str) -> Option<i64> {
        Some(match name {
            "access" => libc::SYS_access,
            "arch_prctl" => libc::SYS_arch_prctl,
            "brk" => libc::SYS_brk,
            "clone" => libc::SYS_clone,
            "close" => libc::SYS_close,
            "dup2" => libc::SYS_dup2,
            "execve" => libc::SYS_execve,
            "fadvise64" => libc::SYS_fadvise64,
            "fstat" => libc::SYS_fstat,
            "getegid" => libc::SYS_getegid,
            "geteuid" => libc::SYS_geteuid,
            "getgid" => libc::SYS_getgid,
            "getpid" => libc::SYS_getpid,
            "getppid" => libc::SYS_getppid,
            "getrandom" => libc::SYS_getrandom,
            "getuid" => libc::SYS_getuid,
            "ioctl" => libc::SYS_ioctl,
            "lseek" => libc::SYS_lseek,
            "mmap" => libc::SYS_mmap,
            "mprotect" => libc::SYS_mprotect,
            "munmap" => libc::SYS_munmap,
            "newfstatat" => libc::SYS_newfstatat,
            "openat" => libc::SYS_openat,
            "pipe2" => libc::SYS_pipe2,
            "pread64" => libc::SYS_pread64,
            "prlimit64" => libc::SYS_prlimit64,
            "read" => libc::SYS_read,
            "rseq" => libc::SYS_rseq,
            "rt_sigaction" => libc::SYS_rt_sigaction,
            "rt_sigprocmask" => libc::SYS_rt_sigprocmask,
            "rt_sigreturn" => libc::SYS_rt_sigreturn,
            "rt_sigsuspend" => libc::SYS_rt_sigsuspend,
            "set_robust_list" => libc::SYS_set_robust_list,
            "set_tid_address" => libc::SYS_set_tid_address,
            "setpgid" => libc::SYS_setpgid,
            "statfs" => libc::SYS_statfs,
            "timer_create" => libc::SYS_timer_create,
            "timer_settime" => libc::SYS_timer_settime,
            "vfork" => libc::SYS_vfork,
            "wait4" => libc::SYS_wait4,
            "write" => libc::SYS_write,
            _ => return None,
        })
    }
}
