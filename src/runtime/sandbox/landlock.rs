//! AC-53 / Slice A2b.4c: landlock LSM filesystem ruleset (layer 4 of 6).
//!
//! ## What this slice ships
//!
//! Installs a path-based filesystem capability ruleset on the bwrap
//! child via a `pre_exec` hook, restricting reads/writes to a small
//! set of known-safe paths. Production profile: read+execute on
//! standard rootfs paths (`/usr`, `/bin`, `/sbin`, `/lib`, `/lib64`,
//! `/etc`), read+write+execute on the per-call cwd workspace.
//!
//! ## Why pre_exec
//!
//! Landlock applies to the calling thread/task and survives
//! `execve(2)`. We install it AFTER `fork(2)` but BEFORE
//! `execve(bwrap)`, so:
//!
//! - The parent (pincery) is not restricted.
//! - The bwrap child + its `sh` descendant inherit the restrictions.
//!
//! ## Inode semantics
//!
//! Landlock identifies allowed paths by the inode of the file
//! descriptor opened at rule-creation time. When bwrap RO-binds
//! `/usr` to `/usr` inside its mount namespace, the inode is the
//! same as the host's `/usr`, so landlock allows access. When bwrap
//! mounts a fresh tmpfs at `/tmp`, that's a NEW inode and landlock
//! blocks it (which is fine — sh + cat + echo do not need /tmp for
//! the workloads we run).
//!
//! ## Mode semantics
//!
//! Kernel landlock has no audit/log mode in ABI v1-v3. In Audit
//! mode we still install enforce-style if the kernel supports it;
//! on failure (kernel < 5.13) we log and proceed. In Enforce mode,
//! unavailability fails closed - mirrors the cgroup + seccomp
//! posture.

#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};

pub use landlock::RulesetStatus;
use landlock::{
    Access, AccessFs, PathBeneath, PathFd, Ruleset, RulesetAttr, RulesetCreatedAttr, ABI,
};

/// Fixed set of host paths that need read+execute access for stock
/// glibc programs to load and run inside the sandbox. Ordered for
/// review-diff readability; runtime order does not matter.
// Default read+execute allowlist. Includes standard rootfs dirs
// sh + coreutils need to execute, plus /sys for bwrap's cgroup v2
// probes.
//
// NOTE: `/` is deliberately NOT here, but the real defect is
// architectural, not allowlist-shaped. Landlock V1+ unconditionally
// denies mount(2) for any task in a Landlock domain (kernel.org,
// userspace-api/landlock §"Current limitations"), and Landlock
// domains are inherited via clone(2) (§"Inheritance"). Because we
// install the ruleset in a `pre_exec` hook in the parent process,
// the bwrap child inherits the domain and EPERMs on its very first
// `mount(NULL, "/", MS_SLAVE | MS_REC, NULL)` call regardless of
// PathBeneath rules. Adding `/` to the allowlist did not (and could
// not) fix this; it only enlarged the read surface.
//
// The correct fix is to install Landlock INSIDE the sandbox after
// bwrap finishes mount-namespace setup, via a `pincery-init` exec
// wrapper. Tracked as AC-83..AC-88 / Phase G0 in scope.md. Full
// architectural audit: docs/security/sandbox-architecture-audit.md.
// Until G0a lands, production defaults to `landlock=false` and
// emits a `sandbox_landlock_disabled` event (AC-53 amendment).
//
// NOTE: /proc is NOT here because bwrap writes /proc/self/uid_map,
// /proc/self/gid_map, /proc/self/setgroups during user-namespace
// setup. /proc must be in rwx_paths instead. See default_for_cwd.
const ROOTFS_RX_PATHS: &[&str] = &["/usr", "/bin", "/sbin", "/lib", "/lib64", "/etc", "/sys"];

/// Profile describing which paths are allowed and at what access
/// level. Built once per tool call by [`LandlockProfile::default_for_cwd`].
#[derive(Debug, Clone)]
pub struct LandlockProfile {
    /// Paths allowed read+execute (rootfs binaries + libraries).
    pub rx_paths: Vec<PathBuf>,
    /// Paths allowed read+write+execute (cwd workspace + any other
    /// caller-pinned writable directories).
    pub rwx_paths: Vec<PathBuf>,
}

impl LandlockProfile {
    /// Production profile for a tool invocation pinned to `cwd`.
    /// Includes the standard rootfs read+execute paths, `/proc` and
    /// `/dev` as read+write+execute (bwrap writes /proc/self/uid_map
    /// etc. during user-namespace setup, and the user shell needs
    /// `/dev/null`, `/dev/urandom`, etc. as the bwrap `--dev` tmpfs
    /// provides them), and the cwd as read+write+execute.
    pub fn default_for_cwd(cwd: &Path) -> Self {
        Self {
            rx_paths: ROOTFS_RX_PATHS.iter().map(PathBuf::from).collect(),
            rwx_paths: vec![
                PathBuf::from("/proc"),
                PathBuf::from("/dev"),
                cwd.to_path_buf(),
            ],
        }
    }
}

/// Returns true iff the kernel supports landlock at the ABI level
/// we require. Kernels older than 5.13 return ENOSYS, which surfaces
/// as an error from `Ruleset::default().handle_access(...).create()`.
///
/// This is a cheap probe - it actually creates a ruleset (and drops
/// it without calling `restrict_self`), so it has no side effects on
/// the calling thread.
pub fn landlock_supported() -> bool {
    let result = Ruleset::default()
        .handle_access(AccessFs::from_all(ABI::V1))
        .and_then(|r| r.create());
    result.is_ok()
}

/// Build and apply a landlock ruleset to the calling thread.
///
/// MUST be called from within a `pre_exec` closure (after `fork(2)`,
/// before `execve(2)`) so it restricts the child process but not the
/// parent. Returns the final [`RulesetStatus`] on success, or a
/// string error describing what failed.
///
/// We use `ABI::V1` for maximum kernel compatibility (5.13+). V3
/// adds file-truncation handling but is not required for the
/// workspace model - a write rule already covers truncation in the
/// only sense relevant to us (truncate before write).
///
/// Missing rx paths (e.g. `/lib64` on a pure-multiarch system) are
/// silently skipped - read access to a non-existent path is moot.
/// Missing rwx paths are an error because the cwd MUST exist (we
/// just created it as a tempdir).
///
/// ## Return value
///
/// `RulesetStatus::FullyEnforced` means the kernel honored every
/// access bit we requested. `RulesetStatus::PartiallyEnforced`
/// means the kernel supports landlock but not every bit — harmless
/// under V1 but callers wanting strict safety (see
/// `SandboxInitPolicy::require_fully_enforced`) should reject it.
/// `NotEnforced` is converted to an `Err` internally since it
/// indicates the ruleset never took effect.
pub fn install_landlock(profile: &LandlockProfile) -> Result<RulesetStatus, String> {
    let abi = ABI::V1;
    let access_all = AccessFs::from_all(abi);
    let access_read = AccessFs::from_read(abi);

    let mut ruleset = Ruleset::default()
        .handle_access(access_all)
        .map_err(|e| format!("landlock handle_access failed: {e}"))?
        .create()
        .map_err(|e| format!("landlock create failed: {e}"))?;

    for p in &profile.rx_paths {
        let fd = match PathFd::new(p) {
            Ok(fd) => fd,
            // Path doesn't exist on this distro - fine, skip.
            Err(_) => continue,
        };
        ruleset = ruleset
            .add_rule(PathBeneath::new(fd, access_read))
            .map_err(|e| format!("landlock add_rule({}) failed: {e}", p.display()))?;
    }
    for p in &profile.rwx_paths {
        let fd =
            PathFd::new(p).map_err(|e| format!("landlock PathFd({}) failed: {e}", p.display()))?;
        ruleset = ruleset
            .add_rule(PathBeneath::new(fd, access_all))
            .map_err(|e| format!("landlock add_rule({}) failed: {e}", p.display()))?;
    }

    let status = ruleset
        .restrict_self()
        .map_err(|e| format!("landlock restrict_self failed: {e}"))?;

    match status.ruleset {
        // Either fully or partially is acceptable here - partial
        // means the kernel supports landlock but not at every access
        // bit we asked for, which is harmless given V1's narrow
        // surface. Callers that require strict enforcement compare
        // the returned status against `RulesetStatus::FullyEnforced`.
        RulesetStatus::FullyEnforced | RulesetStatus::PartiallyEnforced => Ok(status.ruleset),
        RulesetStatus::NotEnforced => {
            Err("landlock not enforced (kernel returned NotEnforced status)".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_includes_rootfs_rx_paths() {
        let cwd = PathBuf::from("/tmp/work-landlock-xyz");
        let p = LandlockProfile::default_for_cwd(&cwd);
        for required in ["/usr", "/bin", "/lib", "/etc"] {
            assert!(
                p.rx_paths.iter().any(|x| x == Path::new(required)),
                "default rx paths missing {required}: {:?}",
                p.rx_paths
            );
        }
    }

    #[test]
    fn default_profile_includes_cwd_as_rwx() {
        let cwd = PathBuf::from("/tmp/work-landlock-xyz");
        let p = LandlockProfile::default_for_cwd(&cwd);
        assert!(p.rwx_paths.iter().any(|x| x == &cwd));
    }

    #[test]
    fn default_profile_includes_proc_as_rwx() {
        // bwrap writes /proc/self/uid_map, /proc/self/gid_map,
        // /proc/self/setgroups during user-namespace setup. If
        // /proc is read-only or missing, bwrap fails with EPERM.
        let cwd = PathBuf::from("/tmp/work-landlock-xyz");
        let p = LandlockProfile::default_for_cwd(&cwd);
        assert!(p.rwx_paths.iter().any(|x| x == Path::new("/proc")));
    }

    #[test]
    fn default_profile_does_not_include_cwd_in_rx_paths() {
        // cwd must only appear once, in the rwx list - otherwise a
        // narrower read-only rule could shadow the write rule.
        let cwd = PathBuf::from("/tmp/work-landlock-xyz");
        let p = LandlockProfile::default_for_cwd(&cwd);
        assert!(!p.rx_paths.iter().any(|x| x == &cwd));
    }

    #[test]
    fn landlock_supported_does_not_panic() {
        // We can't assert true/false here - runs on dev hosts (Linux
        // with various kernels) and CI. We're just pinning that the
        // probe is side-effect-free and returns *something*.
        let _ = landlock_supported();
    }
}
