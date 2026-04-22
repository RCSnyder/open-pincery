//! AC-53 / AC-65 / Slice A2b.4a: cgroup v2 resource caps.
//!
//! Creates a transient unified-hierarchy cgroup per sandbox invocation
//! under `/sys/fs/cgroup/pincery-<uuid_v4>/`, applies cpu/memory/pids
//! caps, attaches the spawned bwrap child, and cleans up on Drop.
//!
//! ## Why raw file I/O, not `cgroups-rs`
//!
//! cgroup v2 is a flat pseudo-filesystem interface: `mkdir`, write
//! decimal strings into `memory.max` / `pids.max` / `cpu.max`, write
//! PIDs into `cgroup.procs`, `rmdir` when the last task exits. A thin
//! `std::fs` wrapper is clearer in both code and `strace` output than
//! a third-party crate that also tries to support cgroup v1 and the
//! systemd D-Bus surface.
//!
//! ## Delegation / privilege
//!
//! Creating a subcgroup directly under `/sys/fs/cgroup/` requires
//! either root, CAP_SYS_ADMIN in the current userns, or a systemd
//! unit with `Delegate=yes`. The probe [`cgroup_v2_writable`] reports
//! this in O(1) so callers can fail closed in Enforce mode and tests
//! can self-skip on unprivileged hosts (mirrors the `bwrap_available`
//! pattern in `sandbox_real_smoke.rs`).
//!
//! ## Drop safety
//!
//! A cgroup directory can only be removed when no tasks remain in it.
//! The caller MUST drop the [`CgroupGuard`] only after the attached
//! child process has been reaped (`wait_with_output().await`); by that
//! point `cgroup.procs` is empty and `rmdir(2)` succeeds. Cleanup
//! errors are swallowed in `Drop`: a leaked pseudo-dir is reaped by
//! [`sweep_leaked_cgroups`] at next startup rather than panicking a
//! destructor.
//!
//! ## Platform gating
//!
//! [`CgroupLimits`] is pure data and compiles on every platform so
//! that [`crate::runtime::sandbox::SandboxProfile`] has a single,
//! stable shape. [`CgroupGuard`], [`cgroup_v2_writable`], and
//! [`sweep_leaked_cgroups`] touch `/sys/fs/cgroup` and are therefore
//! gated `cfg(target_os = "linux")`.

#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::io;
#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};

/// cgroup v2 unified-hierarchy mount point (systemd-default on all
/// supported distros: Ubuntu 22.04+, Debian 12+, Fedora 31+).
#[cfg(target_os = "linux")]
const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// Prefix for every cgroup this crate creates. Used both for
/// construction (`pincery-<uuid>`) and startup sweep matching.
#[cfg(target_os = "linux")]
const PREFIX: &str = "pincery-";

/// Per-invocation resource caps. `None` fields leave the corresponding
/// cgroup v2 interface file at its inherited default.
///
/// Ordering of writes is deterministic: memory → pids → cpu. This
/// matches how most tests assert on stderr ordering after an OOM
/// (memory.max trips first because it's the cheapest signal).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CgroupLimits {
    /// Memory ceiling in bytes — written to `memory.max`. Exceeding
    /// this triggers the cgroup OOM killer and terminates tasks in
    /// the cgroup with SIGKILL.
    pub memory_max_bytes: Option<u64>,
    /// Max concurrent processes/threads — written to `pids.max`.
    /// Further `clone(2)` / `fork(2)` calls return `EAGAIN` once the
    /// limit is reached.
    pub pids_max: Option<u64>,
    /// CPU bandwidth as `(quota_us, period_us)` — written to `cpu.max`
    /// as `"<quota> <period>"`. Example: `(50_000, 100_000)` = 50%
    /// of one core averaged over a 100ms window.
    pub cpu_max_micros: Option<(u64, u64)>,
}

impl CgroupLimits {
    /// Pure helper for unit tests and diagnostics — returns the exact
    /// `(filename, content)` pairs this limits set would write, in
    /// the order `apply_limits` writes them.
    pub fn planned_writes(&self) -> Vec<(&'static str, String)> {
        let mut writes = Vec::new();
        if let Some(bytes) = self.memory_max_bytes {
            writes.push(("memory.max", bytes.to_string()));
        }
        if let Some(pids) = self.pids_max {
            writes.push(("pids.max", pids.to_string()));
        }
        if let Some((quota, period)) = self.cpu_max_micros {
            writes.push(("cpu.max", format!("{quota} {period}")));
        }
        writes
    }
}

/// A live cgroup directory under `/sys/fs/cgroup/pincery-<uuid>/`.
/// Drop removes the cgroup dir; see module docs for ordering rules.
#[cfg(target_os = "linux")]
#[derive(Debug)]
pub struct CgroupGuard {
    path: PathBuf,
}

#[cfg(target_os = "linux")]
impl CgroupGuard {
    /// Create a new cgroup and apply `limits`.
    ///
    /// Returns `Err` if the current process cannot write to
    /// `/sys/fs/cgroup` (not root, no CAP_SYS_ADMIN, no systemd
    /// delegation, or cgroup v1 host). Callers should treat this as
    /// a fail-closed signal in Enforce mode.
    pub fn new(limits: &CgroupLimits) -> io::Result<Self> {
        let name = format!("{PREFIX}{}", uuid::Uuid::new_v4());
        let path = Path::new(CGROUP_ROOT).join(&name);
        fs::create_dir(&path)?;
        let guard = Self { path };
        if let Err(e) = guard.apply_limits(limits) {
            // Best-effort cleanup of the just-created cgroup before
            // bubbling the apply error — otherwise we leak a pincery-*
            // directory on every failed limits write.
            let _ = fs::remove_dir(&guard.path);
            return Err(e);
        }
        Ok(guard)
    }

    fn apply_limits(&self, limits: &CgroupLimits) -> io::Result<()> {
        for (file, content) in limits.planned_writes() {
            fs::write(self.path.join(file), content)?;
        }
        Ok(())
    }

    /// Attach a PID to this cgroup by writing to `cgroup.procs`.
    /// Must be called AFTER `Command::spawn` returns the child's PID.
    pub fn attach_pid(&self, pid: u32) -> io::Result<()> {
        fs::write(self.path.join("cgroup.procs"), pid.to_string())
    }

    /// Path to the live cgroup directory (for diagnostics / tests).
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(target_os = "linux")]
impl Drop for CgroupGuard {
    fn drop(&mut self) {
        // Cgroup v2 requires an empty cgroup to rmdir. Callers ensure
        // the attached child is reaped before drop; we swallow errors
        // deliberately — `sweep_leaked_cgroups` reaps any stragglers
        // on next startup. Panicking a destructor would abort the
        // tokio worker.
        let _ = fs::remove_dir(&self.path);
    }
}

/// Probe whether cgroup v2 is mounted AND this process can create
/// subcgroups. O(1): attempts `mkdir` of a throwaway probe dir, then
/// `rmdir`. Used both by runtime fail-closed logic and by tests to
/// self-skip on unprivileged hosts.
#[cfg(target_os = "linux")]
pub fn cgroup_v2_writable() -> bool {
    let probe = Path::new(CGROUP_ROOT).join(format!(
        "{PREFIX}probe-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    match fs::create_dir(&probe) {
        Ok(()) => {
            let _ = fs::remove_dir(&probe);
            true
        }
        Err(_) => false,
    }
}

/// One-shot startup sweep of leaked `pincery-*` cgroups from prior
/// crashed executions. Returns the number of directories successfully
/// removed. Silently skips entries that still have tasks in them
/// (another live pincery process, or a genuinely wedged child).
///
/// Intended to be called once on server boot. Idempotent.
#[cfg(target_os = "linux")]
pub fn sweep_leaked_cgroups() -> io::Result<usize> {
    let root = Path::new(CGROUP_ROOT);
    if !root.exists() {
        return Ok(0);
    }
    let mut removed = 0usize;
    for entry in fs::read_dir(root)? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with(PREFIX) {
            continue;
        }
        if fs::remove_dir(entry.path()).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_default_has_no_caps() {
        let d = CgroupLimits::default();
        assert!(d.memory_max_bytes.is_none());
        assert!(d.pids_max.is_none());
        assert!(d.cpu_max_micros.is_none());
        assert!(d.planned_writes().is_empty());
    }

    #[test]
    fn planned_writes_order_is_memory_pids_cpu() {
        let limits = CgroupLimits {
            memory_max_bytes: Some(64 * 1024 * 1024),
            pids_max: Some(16),
            cpu_max_micros: Some((50_000, 100_000)),
        };
        let writes = limits.planned_writes();
        assert_eq!(writes.len(), 3);
        assert_eq!(writes[0].0, "memory.max");
        assert_eq!(writes[0].1, "67108864");
        assert_eq!(writes[1].0, "pids.max");
        assert_eq!(writes[1].1, "16");
        assert_eq!(writes[2].0, "cpu.max");
        assert_eq!(writes[2].1, "50000 100000");
    }

    #[test]
    fn planned_writes_skips_none_fields() {
        let limits = CgroupLimits {
            memory_max_bytes: None,
            pids_max: Some(5),
            cpu_max_micros: None,
        };
        let writes = limits.planned_writes();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].0, "pids.max");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn cgroup_guard_new_fails_closed_when_unprivileged() {
        // Fail-closed is the whole point in Enforce mode. On almost
        // every CI host this process cannot mkdir under /sys/fs/cgroup.
        // Skip when the host IS delegated (e.g. privileged container
        // or systemd Delegate=yes) — in that case `new()` will
        // legitimately succeed and a `CgroupGuard` will auto-clean.
        if cgroup_v2_writable() {
            eprintln!("skipping: cgroup v2 is writable here (privileged host)");
            return;
        }
        let result = CgroupGuard::new(&CgroupLimits {
            memory_max_bytes: Some(1024 * 1024 * 64),
            pids_max: Some(10),
            cpu_max_micros: None,
        });
        assert!(
            result.is_err(),
            "expected io::Error on unprivileged host, got Ok"
        );
    }
}
