//! AC-53 / Slice A2b.4b: seccomp-bpf syscall filter (layer 3 of 6).
//!
//! ## What this slice ships: a denylist, not yet a true allowlist
//!
//! The design calls for a default-deny allowlist. In practice, a
//! hand-rolled syscall allowlist that keeps `sh -c echo` alive on
//! stock glibc is dozens of syscalls deep (ld.so dynamic linking,
//! rseq, clone3, ...) and extraordinarily easy to get wrong — one
//! missing syscall and the entire sandbox SIGKILLs the shell before
//! it executes the command. Shipping that in one slice would either
//! break the existing bwrap smoke tests or paper over real policy
//! errors with an `Allow` fall-through.
//!
//! Instead, this slice ships a **targeted denylist** for the subset of
//! Linux syscalls that are the primary escape primitives an attacker
//! would use from inside the sandbox: `mount`, `umount2`, `pivot_root`,
//! `reboot`, `init_module`, `finit_module`, `delete_module`, `kexec_load`,
//! `kexec_file_load`, `bpf`, `ptrace`. Every other syscall is allowed.
//!
//! This:
//! 1. **Proves the full pipeline end-to-end** — `SeccompFilter →
//!    BpfProgram → memfd → bwrap --seccomp <fd> → kernel` — with a
//!    kernel-visible adversarial signal (SIGSYS on `mount`).
//! 2. **Delivers real security today** — every syscall in the denylist
//!    is listed in readiness.md's 12-payload escape suite.
//! 3. **Leaves a clean tightening path** — the next sub-slice
//!    (A2b.4b-hardening, scheduled after the 12-payload escape test
//!    suite lands in `tests/sandbox_escape_test.rs`) flips the default
//!    to `KillProcess` and builds the real allowlist from an empirical
//!    list of syscalls observed during the passing smoke tests.
//!
//! ## Mode semantics
//!
//! - `Enforce` — denied syscalls return `SeccompAction::KillProcess`
//!   (kernel signal SIGSYS, exit code 159).
//! - `Audit` — denied syscalls return `SeccompAction::Log` (syscall
//!   succeeds or fails on its own merits, but kernel logs `audit:
//!   type=1326 ...` to syslog). Matches the cgroup-layer Audit posture.
//! - `Disabled` — seccomp layer is not installed (same as
//!   `SandboxProfile.seccomp = false`).
//!
//! ## bwrap fd protocol
//!
//! bwrap's `--seccomp <fd>` expects raw `struct sock_filter[]` bytes
//! on the fd — NOT a `struct sock_fprog` wrapper, NOT any framing.
//! That's exactly the memory layout of `BpfProgram = Vec<sock_filter>`
//! since `sock_filter` is `#[repr(C)]` 8 bytes wide (via libc).
//!
//! We use `libc::memfd_create(name, 0)` (no MFD_CLOEXEC) so the fd
//! inherits through `fork()`/`execve()`. bwrap then reads it and
//! installs the program via `seccomp(SECCOMP_SET_MODE_FILTER, ...)`.

#![cfg(target_os = "linux")]

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

use seccompiler::{BpfProgram, SeccompAction, SeccompFilter};

use crate::config::SandboxMode;

/// Syscalls this slice unconditionally denies. Ordered alphabetically
/// for review diffs; order does not affect runtime semantics because
/// each syscall gets an empty rule vec (= always matches by syscall
/// number).
///
/// Rationale for each:
/// - `bpf` — loading eBPF can open kernel kprobe/tracepoint paths.
/// - `delete_module` — kernel module unload.
/// - `finit_module` — kernel module load via fd.
/// - `init_module` — kernel module load.
/// - `kexec_file_load` / `kexec_load` — reboot into a new kernel image.
/// - `mount` — remount fs, mount /proc with less-restricted opts.
/// - `pivot_root` — change root filesystem.
/// - `ptrace` — attach to arbitrary processes in same pid ns.
/// - `reboot` — reboot the host (blocked at kernel level but still worth refusing).
/// - `umount2` — unmount filesystems.
fn denied_syscalls() -> Vec<i64> {
    vec![
        libc::SYS_bpf,
        libc::SYS_delete_module,
        libc::SYS_finit_module,
        libc::SYS_init_module,
        libc::SYS_kexec_file_load,
        libc::SYS_kexec_load,
        libc::SYS_mount,
        libc::SYS_pivot_root,
        libc::SYS_ptrace,
        libc::SYS_reboot,
        libc::SYS_umount2,
    ]
}

/// Build the compiled BPF program for a given enforcement mode.
///
/// - `Enforce` / `Disabled` (only `Enforce` reaches here normally) →
///   denied syscalls get `KillProcess`.
/// - `Audit` → denied syscalls get `Log`.
///
/// Returns the program or an error if the host arch is unsupported.
pub fn build_bpf_program(mode: SandboxMode) -> Result<BpfProgram, String> {
    let match_action = match mode {
        SandboxMode::Audit => SeccompAction::Log,
        // Enforce is the default posture; Disabled callers should not
        // reach this function, but we treat it identically for safety.
        SandboxMode::Enforce | SandboxMode::Disabled => SeccompAction::KillProcess,
    };
    // Every denied syscall maps to an empty rule vec so any invocation
    // matches by syscall number alone.
    let rules = denied_syscalls()
        .into_iter()
        .map(|n| (n, Vec::new()))
        .collect();
    let target_arch = std::env::consts::ARCH
        .try_into()
        .map_err(|e| format!("unsupported host arch for seccomp: {e:?}"))?;
    let filter = SeccompFilter::new(
        rules,
        /* mismatch_action = */ SeccompAction::Allow,
        match_action,
        target_arch,
    )
    .map_err(|e| format!("SeccompFilter::new failed: {e:?}"))?;
    BpfProgram::try_from(filter).map_err(|e| format!("BpfProgram::try_from failed: {e:?}"))
}

/// Write a compiled BPF program into a fresh in-memory file descriptor
/// suitable for `bwrap --seccomp <fd>`. The returned fd:
/// - is positioned at offset 0 (so `read(2)` starts at the first instruction),
/// - has `FD_CLOEXEC` cleared (so it inherits across `execve`),
/// - owns its lifetime via [`OwnedFd`] (dropping closes it).
pub fn write_bpf_to_memfd(program: &BpfProgram) -> io::Result<OwnedFd> {
    // memfd_create with flags=0: no MFD_CLOEXEC, so the fd survives
    // execve by default. bwrap only reads from it; no seal needed.
    let name = c"pincery-seccomp-bpf";
    // SAFETY: libc::memfd_create is an FFI call with no pointer aliasing
    // concerns — we pass a static C string and constant flags.
    let raw = unsafe { libc::memfd_create(name.as_ptr(), 0) };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `raw` is a fresh, unowned fd from the kernel. `OwnedFd`
    // takes exclusive ownership and will `close()` on drop.
    let fd: OwnedFd = unsafe { OwnedFd::from_raw_fd(raw) };

    // Serialize `Vec<sock_filter>` as a contiguous byte slice. `sock_filter`
    // is `#[repr(C)]` (8 bytes: u16+u8+u8+u32 on every supported arch), so
    // the vec's heap buffer IS the on-disk format bwrap expects.
    let byte_len = std::mem::size_of_val(program.as_slice());
    let bytes = unsafe { std::slice::from_raw_parts(program.as_ptr().cast::<u8>(), byte_len) };
    write_all_to_fd(&fd, bytes)?;
    // Rewind so bwrap reads from instruction 0.
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
        // SAFETY: we pass a valid pointer + length pair from `buf` and a
        // live fd owned by `OwnedFd`. The kernel writes at most `len`
        // bytes; we advance `buf` by the returned count.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denylist_contains_expected_escape_primitives() {
        let syscalls = denied_syscalls();
        // Stable subset — any regression would silently weaken the layer.
        for must_have in [
            libc::SYS_mount,
            libc::SYS_reboot,
            libc::SYS_init_module,
            libc::SYS_ptrace,
            libc::SYS_kexec_load,
            libc::SYS_bpf,
            libc::SYS_pivot_root,
        ] {
            assert!(
                syscalls.contains(&must_have),
                "denylist missing syscall #{must_have}"
            );
        }
    }

    #[test]
    fn build_program_enforce_produces_nonempty_bpf() {
        let prog = build_bpf_program(SandboxMode::Enforce).expect("enforce program");
        assert!(!prog.is_empty(), "BPF program must contain instructions");
        // sock_filter is 8 bytes; a real program is well over one record.
        assert!(
            prog.len() > 3,
            "BPF program suspiciously small: {}",
            prog.len()
        );
    }

    #[test]
    fn build_program_audit_produces_nonempty_bpf() {
        let prog = build_bpf_program(SandboxMode::Audit).expect("audit program");
        assert!(!prog.is_empty());
    }

    #[test]
    fn memfd_roundtrip_matches_program_bytes() {
        let prog = build_bpf_program(SandboxMode::Enforce).expect("program");
        let fd = write_bpf_to_memfd(&prog).expect("memfd write");
        // Read it back and compare byte-for-byte.
        let expected_bytes = unsafe {
            std::slice::from_raw_parts(
                prog.as_ptr().cast::<u8>(),
                std::mem::size_of_val(prog.as_slice()),
            )
        };
        let mut buf = vec![0u8; expected_bytes.len()];
        // Rewind (should already be 0) then read.
        unsafe { libc::lseek(fd.as_raw_fd(), 0, libc::SEEK_SET) };
        let n = unsafe { libc::read(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
        assert!(n >= 0, "read failed: {}", std::io::Error::last_os_error());
        assert_eq!(n as usize, expected_bytes.len());
        assert_eq!(buf, expected_bytes);
    }
}
