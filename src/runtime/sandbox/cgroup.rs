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

/// Outcome of [`probe_memory_max_enforcement`]. The empirical probe
/// distinguishes three cases that the cheap subtree-control parser
/// cannot:
///
/// - [`MemoryProbeOutcome::Enforced`] — the kernel actually killed
///   an over-allocator inside a freshly-created `pincery-probe-*`
///   cgroup. memory.max is fully enforced on this host. Safe to
///   trust the cap inside production sandbox cgroups.
/// - [`MemoryProbeOutcome::NotEnforced`] — the over-allocator
///   completed normally despite a cap eight times smaller than the
///   allocation. Writing `memory.max` is silently a no-op on this
///   host. Common causes (in order of frequency):
///     1. Memory controller not delegated to the unified hierarchy
///        (`memory` missing from `cgroup.subtree_control`). Affects
///        most non-systemd-managed Docker containers.
///     2. Swap accounting disabled (`memory.swap.max=max` plus
///        plenty of free swap). The kernel pages out instead of
///        OOM-killing.
///     3. Kernel built without `CONFIG_MEMCG`. Rare in 2026.
/// - [`MemoryProbeOutcome::Skipped`] — the probe could not run at
///   all (no cgroup write access, `dd` missing on `$PATH`, fork
///   failure). Treat as unverified; do not infer enforcement
///   either way.
///
/// The probe is *empirical*, not heuristic: it runs the same
/// allocation pattern an attacker would attempt and observes the
/// kernel's response. A `Enforced` result therefore proves the
/// production capability rather than approximating it.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryProbeOutcome {
    /// Kernel SIGKILLed the over-allocator. `memory.max` works on
    /// this host. Production sandbox `memory_max_bytes` caps will
    /// be honored.
    Enforced,
    /// Over-allocator completed normally despite the cap. The
    /// `evidence` string includes the observed exit code or signal
    /// and the cap/allocation sizes used. Operators should treat
    /// `SandboxProfile.cgroup.memory_max_bytes` as advisory until
    /// the underlying delegation/swap-accounting/kernel issue is
    /// fixed.
    NotEnforced { evidence: String },
    /// Probe could not execute. Caller decides whether to fail
    /// closed (refuse Enforce mode) or proceed with a warning.
    Skipped { reason: String },
}

/// Empirical runtime probe: verify that cgroup v2 `memory.max`
/// actually causes the kernel to OOM-kill an over-allocator.
///
/// Creates a one-shot `pincery-probe-mem-<uuid>` cgroup with
/// `memory.max=8 MiB`, then spawns `dd if=/dev/zero of=/dev/null
/// bs=64M count=1` attached to that cgroup via a `pre_exec` write
/// to `cgroup.procs`. dd allocates a single 64 MiB block buffer
/// before any read/write — eight times the cap, an unambiguous
/// over-allocation. Three observable outcomes:
///
/// 1. dd is reaped with signal 9 (`SIGKILL`) → memory.max is
///    enforced; return [`MemoryProbeOutcome::Enforced`].
/// 2. dd exits 0 → memory.max was silently ignored; return
///    [`MemoryProbeOutcome::NotEnforced`].
/// 3. The probe could not even start (cgroup write blocked, dd
///    missing) → return [`MemoryProbeOutcome::Skipped`].
///
/// The probe is bounded by dd's own runtime: with `bs=64M count=1`
/// and `/dev/zero`/`/dev/null`, dd completes (or is killed) in
/// well under one second on any machine that can boot Linux. We
/// therefore do not impose an external timeout — a hung dd would
/// indicate a deeper kernel fault that no test-side timeout can
/// usefully recover from.
///
/// ## Why an empirical probe instead of `cgroup.subtree_control`
///
/// The cheap parser (read `cgroup.subtree_control`, look for the
/// `memory` token) is necessary but not sufficient. CI runs
/// 25142773968 / 25142973309 demonstrated that on a privileged
/// Docker host the controller IS listed in `subtree_control`, yet
/// the kernel still does not enforce the cap on the bwrapped
/// hierarchy — likely a swap-accounting / `memory.swap.max` /
/// overlayfs interaction. Only an empirical allocation against a
/// known-too-small cap reliably distinguishes "enforced" from
/// "delegated but ignored".
///
/// ## Slice scope (G1c.x)
///
/// This slice ships the probe + a unit test that exercises the
/// `Skipped` branch on unprivileged hosts. It does NOT yet wire
/// the probe into `assert_kernel_floor` or refuse `Enforce` mode
/// when the probe says `NotEnforced` — a follow-up slice (G1c.x.2)
/// will own that posture decision so this slice stays small. The
/// initial caller is `tests/sandbox_escape_test.rs`'s
/// `resource_memory_balloon_blocked`, which now gates on the probe
/// instead of unconditionally skipping.
#[cfg(target_os = "linux")]
pub fn probe_memory_max_enforcement() -> MemoryProbeOutcome {
    use std::os::unix::process::{CommandExt, ExitStatusExt};
    use std::process::{Command, Stdio};

    if !cgroup_v2_writable() {
        return MemoryProbeOutcome::Skipped {
            reason: "cgroup v2 not writable (need root, CAP_SYS_ADMIN, or systemd Delegate=yes)"
                .into(),
        };
    }

    // 8 MiB cap vs 64 MiB allocation: 8x ratio is unambiguous and
    // small enough that the probe never holds meaningful memory
    // even when `Enforced`.
    const CAP_BYTES: u64 = 8 * 1024 * 1024;
    const ALLOC_BYTES: u64 = 64 * 1024 * 1024;

    let limits = CgroupLimits {
        memory_max_bytes: Some(CAP_BYTES),
        pids_max: None,
        cpu_max_micros: None,
    };
    let guard = match CgroupGuard::new(&limits) {
        Ok(g) => g,
        Err(e) => {
            return MemoryProbeOutcome::Skipped {
                reason: format!("cgroup create/apply failed: {e}"),
            };
        }
    };
    let cgroup_procs = guard.path().join("cgroup.procs");

    let mut cmd = Command::new("dd");
    cmd.arg("if=/dev/zero")
        .arg("of=/dev/null")
        .arg(format!("bs={ALLOC_BYTES}"))
        .arg("count=1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    // SAFETY: `pre_exec` runs in the freshly-forked child, after
    // `fork(2)` returns and before `execve(2)`. Writing the child
    // PID into `cgroup.procs` migrates only the calling task (the
    // child) — the parent stays in its original cgroup and is
    // unaffected by the cap. `fs::write` may allocate, but only in
    // the child's heap, which is bounded by the parent's cgroup
    // (typically uncapped) until the migration takes effect on the
    // very next memory access. We move `cgroup_procs` into the
    // closure so the path lifetime is independent of the parent's
    // `guard`.
    unsafe {
        cmd.pre_exec(move || {
            let pid = std::process::id().to_string();
            std::fs::write(&cgroup_procs, pid)
        });
    }

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return MemoryProbeOutcome::Skipped {
                reason: format!("dd spawn failed: {e} (is /usr/bin/dd installed?)"),
            };
        }
    };
    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => {
            return MemoryProbeOutcome::Skipped {
                reason: format!("dd wait failed: {e}"),
            };
        }
    };

    if let Some(sig) = output.status.signal() {
        if sig == 9 {
            return MemoryProbeOutcome::Enforced;
        }
        return MemoryProbeOutcome::NotEnforced {
            evidence: format!(
                "dd terminated by signal {sig} (expected SIGKILL=9); cap={CAP_BYTES}B alloc={ALLOC_BYTES}B"
            ),
        };
    }
    if let Some(code) = output.status.code() {
        return MemoryProbeOutcome::NotEnforced {
            evidence: format!(
                "dd exited with code {code} after allocating {ALLOC_BYTES}B against a {CAP_BYTES}B cap; cgroup memory.max is not enforced on this host"
            ),
        };
    }
    MemoryProbeOutcome::Skipped {
        reason: "dd terminated with neither signal nor exit code (kernel anomaly)".into(),
    }
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

    #[test]
    #[cfg(target_os = "linux")]
    fn probe_memory_max_enforcement_skipped_when_unprivileged() {
        // The probe MUST NOT panic on unprivileged hosts. On any
        // host where `cgroup_v2_writable()` is false (the common
        // dev/CI case for non-privileged containers and rootless
        // CI), the probe must return `Skipped` with an explicit
        // reason instead of blowing up. Mirrors the
        // `cgroup_guard_new_fails_closed_when_unprivileged` shape.
        if cgroup_v2_writable() {
            // Privileged host: the probe runs for real and we
            // accept either Enforced or NotEnforced — both are
            // valid empirical observations. We only refuse Skipped,
            // which would indicate a bug in cgroup creation rather
            // than an honest probe answer.
            let outcome = probe_memory_max_enforcement();
            match outcome {
                MemoryProbeOutcome::Enforced => {
                    eprintln!("probe: memory.max enforced on this host");
                }
                MemoryProbeOutcome::NotEnforced { evidence } => {
                    eprintln!("probe: memory.max NOT enforced — {evidence}");
                }
                MemoryProbeOutcome::Skipped { reason } => {
                    panic!(
                        "probe must not Skip on a privileged host where cgroup_v2_writable=true: {reason}"
                    );
                }
            }
            return;
        }
        let outcome = probe_memory_max_enforcement();
        match outcome {
            MemoryProbeOutcome::Skipped { reason } => {
                assert!(
                    reason.contains("cgroup v2 not writable"),
                    "expected cgroup-write skip reason, got: {reason}"
                );
            }
            other => panic!(
                "expected Skipped on unprivileged host (cgroup_v2_writable=false), got {other:?}"
            ),
        }
    }
}
