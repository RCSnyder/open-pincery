//! AC-53 / Phase G0: Landlock filesystem, IPC-scope, and audit controls.
//!
//! ## What this slice ships
//!
//! Installs Landlock from `pincery-init` after bwrap has finished mount
//! namespace setup, restricting reads/writes to a small set of known-safe
//! paths. Production profile: read+execute on standard rootfs paths
//! (`/usr`, `/bin`, `/sbin`, `/lib`, `/lib64`, `/etc`, `/sys`),
//! read+write+execute on `/proc`, `/dev`, and the per-call cwd workspace.
//!
//! ## Why pincery-init
//!
//! Landlock applies to the calling thread/task, survives `execve(2)`,
//! and is inherited by descendants. Installing it in the parent-side
//! `pre_exec` hook restricted bwrap itself and broke its mount setup;
//! installing it inside `pincery-init` keeps the parent and bwrap setup
//! unrestricted while still constraining the user command and descendants.
//!
//! - The parent (pincery) is not restricted.
//! - bwrap can complete namespace setup before Landlock is applied.
//! - The shell command and its descendants inherit the restrictions.
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
//! Kernel Landlock has no permissive audit mode. In Audit mode we still
//! install enforce-style if the kernel supports it; on failure (kernel
//! < 5.13) we log and proceed. In Enforce mode, unavailability fails
//! closed. AC-88 additionally requests the ABI-7 denied-operation audit
//! flag when available; ABI 6 keeps enforcement and degrades only audit
//! visibility.

#![cfg(target_os = "linux")]

use std::ffi::CString;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub use crate::runtime::sandbox::init_policy::{
    LANDLOCK_RESTRICT_SELF_LOG_NEW_EXEC_ON, LANDLOCK_SCOPE_ABSTRACT_UNIX_SOCKET,
    LANDLOCK_SCOPE_ALL, LANDLOCK_SCOPE_SIGNAL,
};
pub use landlock::RulesetStatus;
use landlock::{
    Access, AccessFs, CompatLevel, Compatible, PathBeneath, PathFd, Ruleset, RulesetAttr,
    RulesetCreatedAttr, ABI,
};

const LANDLOCK_RULE_PATH_BENEATH: u32 = 1;
const LANDLOCK_ACCESS_FS_EXECUTE: u64 = 1 << 0;
const LANDLOCK_ACCESS_FS_WRITE_FILE: u64 = 1 << 1;
const LANDLOCK_ACCESS_FS_READ_FILE: u64 = 1 << 2;
const LANDLOCK_ACCESS_FS_READ_DIR: u64 = 1 << 3;
const LANDLOCK_ACCESS_FS_REMOVE_DIR: u64 = 1 << 4;
const LANDLOCK_ACCESS_FS_REMOVE_FILE: u64 = 1 << 5;
const LANDLOCK_ACCESS_FS_MAKE_CHAR: u64 = 1 << 6;
const LANDLOCK_ACCESS_FS_MAKE_DIR: u64 = 1 << 7;
const LANDLOCK_ACCESS_FS_MAKE_REG: u64 = 1 << 8;
const LANDLOCK_ACCESS_FS_MAKE_SOCK: u64 = 1 << 9;
const LANDLOCK_ACCESS_FS_MAKE_FIFO: u64 = 1 << 10;
const LANDLOCK_ACCESS_FS_MAKE_BLOCK: u64 = 1 << 11;
const LANDLOCK_ACCESS_FS_MAKE_SYM: u64 = 1 << 12;
const LANDLOCK_ACCESS_FS_V1_READ: u64 =
    LANDLOCK_ACCESS_FS_EXECUTE | LANDLOCK_ACCESS_FS_READ_FILE | LANDLOCK_ACCESS_FS_READ_DIR;
const LANDLOCK_ACCESS_FS_V1_ALL: u64 = LANDLOCK_ACCESS_FS_V1_READ
    | LANDLOCK_ACCESS_FS_WRITE_FILE
    | LANDLOCK_ACCESS_FS_REMOVE_DIR
    | LANDLOCK_ACCESS_FS_REMOVE_FILE
    | LANDLOCK_ACCESS_FS_MAKE_CHAR
    | LANDLOCK_ACCESS_FS_MAKE_DIR
    | LANDLOCK_ACCESS_FS_MAKE_REG
    | LANDLOCK_ACCESS_FS_MAKE_SOCK
    | LANDLOCK_ACCESS_FS_MAKE_FIFO
    | LANDLOCK_ACCESS_FS_MAKE_BLOCK
    | LANDLOCK_ACCESS_FS_MAKE_SYM;

#[derive(Debug, PartialEq, Eq)]
pub struct LandlockRestrictionStatus {
    pub ruleset: RulesetStatus,
    pub no_new_privs: bool,
}

impl From<landlock::RestrictionStatus> for LandlockRestrictionStatus {
    fn from(status: landlock::RestrictionStatus) -> Self {
        Self {
            ruleset: status.ruleset,
            no_new_privs: status.no_new_privs,
        }
    }
}

#[repr(C)]
struct RawRulesetAttr {
    handled_access_fs: u64,
    handled_access_net: u64,
    scoped: u64,
}

#[repr(C)]
struct RawPathBeneathAttr {
    allowed_access: u64,
    parent_fd: i32,
}

/// How strictly unsupported Landlock features should be handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LandlockCompatibility {
    /// Best-effort mode: use whatever the kernel can enforce and let
    /// the caller inspect `RestrictionStatus` afterward.
    BestEffort,
    /// Production enforce mode: unsupported requested features are a
    /// hard error, and partial ruleset status is rejected.
    HardRequirement,
}

impl LandlockCompatibility {
    fn compat_level(self) -> CompatLevel {
        match self {
            Self::BestEffort => CompatLevel::BestEffort,
            Self::HardRequirement => CompatLevel::HardRequirement,
        }
    }
}

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
/// MUST be called from a short-lived child path such as `pincery-init`
/// after bwrap has finished namespace and mount setup. Landlock is
/// inherited across `execve(2)`, so the wrapper can restrict itself
/// and then exec the user command without affecting the parent.
/// Returns the final [`RestrictionStatus`] on success, or a string
/// error describing what failed.
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
/// `RestrictionStatus { ruleset: FullyEnforced, no_new_privs: true }`
/// means the kernel honored every access bit we requested and set
/// no-new-privileges. `RulesetStatus::PartiallyEnforced` means the
/// kernel supports landlock but not every bit; it is only accepted
/// with [`LandlockCompatibility::BestEffort`]. `NotEnforced` is
/// converted to an `Err` internally since it indicates the ruleset
/// never took effect.
pub fn install_landlock(
    profile: &LandlockProfile,
    compatibility: LandlockCompatibility,
) -> Result<LandlockRestrictionStatus, String> {
    install_landlock_with_restrict_flags(profile, compatibility, 0)
}

pub fn install_landlock_with_restrict_flags(
    profile: &LandlockProfile,
    compatibility: LandlockCompatibility,
    restrict_flags: u32,
) -> Result<LandlockRestrictionStatus, String> {
    if restrict_flags != 0 {
        return install_landlock_raw(profile, compatibility, restrict_flags);
    }

    let abi = ABI::V1;
    let access_all = AccessFs::from_all(abi);
    let access_read = AccessFs::from_read(abi);

    let mut ruleset = Ruleset::default()
        .set_compatibility(compatibility.compat_level())
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

    validate_restriction_status(status, compatibility)
}

fn install_landlock_raw(
    profile: &LandlockProfile,
    compatibility: LandlockCompatibility,
    restrict_flags: u32,
) -> Result<LandlockRestrictionStatus, String> {
    let attr = RawRulesetAttr {
        handled_access_fs: LANDLOCK_ACCESS_FS_V1_ALL,
        handled_access_net: 0,
        scoped: 0,
    };
    let ruleset_fd = create_ruleset(&attr, "filesystem")?;

    for path in &profile.rx_paths {
        let path_fd = match open_path_fd(path) {
            Ok(fd) => fd,
            Err(_) => continue,
        };
        add_path_rule(&ruleset_fd, &path_fd, LANDLOCK_ACCESS_FS_V1_READ, path)?;
    }
    for path in &profile.rwx_paths {
        let path_fd = open_path_fd(path)?;
        add_path_rule(&ruleset_fd, &path_fd, LANDLOCK_ACCESS_FS_V1_ALL, path)?;
    }

    restrict_ruleset(&ruleset_fd, restrict_flags, "filesystem")?;
    let no_new_privs = no_new_privs_enabled()?;
    let ruleset = RulesetStatus::FullyEnforced;
    validate_restriction_parts(&ruleset, no_new_privs, compatibility)?;
    Ok(LandlockRestrictionStatus {
        ruleset,
        no_new_privs,
    })
}

fn create_ruleset(attr: &RawRulesetAttr, label: &str) -> Result<OwnedFd, String> {
    // SAFETY: `attr` points to a C-compatible value valid for the
    // duration of the syscall; size matches the struct we pass.
    let raw_fd = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            attr as *const RawRulesetAttr,
            std::mem::size_of::<RawRulesetAttr>(),
            0u32,
        )
    };
    if raw_fd < 0 {
        return Err(format!(
            "landlock create {label} ruleset failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    let raw_fd: i32 = raw_fd
        .try_into()
        .map_err(|_| format!("landlock {label} ruleset fd out of range: {raw_fd}"))?;
    // SAFETY: `raw_fd` is a fresh kernel-allocated fd with no owner.
    Ok(unsafe { OwnedFd::from_raw_fd(raw_fd) })
}

fn open_path_fd(path: &Path) -> Result<OwnedFd, String> {
    let c_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| format!("landlock path contains interior NUL: {}", path.display()))?;
    // SAFETY: path is a valid NUL-terminated C string; flags request
    // an O_PATH fd suitable for LANDLOCK_RULE_PATH_BENEATH.
    let raw_fd = unsafe { libc::open(c_path.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
    if raw_fd < 0 {
        return Err(format!(
            "landlock open path {} failed: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: `raw_fd` is a fresh kernel-allocated fd with no owner.
    Ok(unsafe { OwnedFd::from_raw_fd(raw_fd) })
}

fn add_path_rule(
    ruleset_fd: &OwnedFd,
    path_fd: &OwnedFd,
    allowed_access: u64,
    path: &Path,
) -> Result<(), String> {
    let attr = RawPathBeneathAttr {
        allowed_access,
        parent_fd: path_fd.as_raw_fd(),
    };
    // SAFETY: fds are live and owned by the caller; `attr` points to
    // the packed UAPI struct for the duration of the syscall.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_landlock_add_rule,
            ruleset_fd.as_raw_fd(),
            LANDLOCK_RULE_PATH_BENEATH,
            &attr as *const RawPathBeneathAttr,
            0u32,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(format!(
            "landlock add_rule({}) failed: {}",
            path.display(),
            std::io::Error::last_os_error()
        ))
    }
}

fn restrict_ruleset(ruleset_fd: &OwnedFd, flags: u32, label: &str) -> Result<(), String> {
    // SAFETY: pure kernel syscall on a valid ruleset fd. The wrapper
    // has already set no_new_privs and is single-threaded here.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_landlock_restrict_self,
            ruleset_fd.as_raw_fd(),
            flags,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(format!(
            "landlock restrict {label} ruleset failed: {}",
            std::io::Error::last_os_error()
        ))
    }
}

fn no_new_privs_enabled() -> Result<bool, String> {
    // SAFETY: pure getter; return value is 0 or 1 on success,
    // negative on error.
    let rc = unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0u64, 0u64, 0u64, 0u64) };
    match rc {
        1 => Ok(true),
        0 => Ok(false),
        _ => Err(format!(
            "prctl(PR_GET_NO_NEW_PRIVS) failed: {}",
            std::io::Error::last_os_error()
        )),
    }
}

/// Install ABI-6 Landlock IPC scopes on the calling thread.
///
/// `landlock = 0.4` only exposes Landlock features available as of
/// Linux 5.19 and has no builder API for the ABI-6 `scoped` field.
/// AC-87 therefore uses the raw ruleset syscall for this one bitmap
/// while leaving the path-based filesystem ruleset on the safe crate
/// API above. This creates a second Landlock layer with no path or
/// network restrictions and only the requested IPC scopes.
pub fn install_landlock_scopes(scopes: u64) -> Result<(), String> {
    if scopes == 0 {
        return Ok(());
    }

    let attr = RawRulesetAttr {
        handled_access_fs: 0,
        handled_access_net: 0,
        scoped: scopes,
    };
    let ruleset_fd = create_ruleset(&attr, "scoped")?;
    restrict_ruleset(&ruleset_fd, 0, "scoped")
}

fn validate_restriction_status(
    status: landlock::RestrictionStatus,
    compatibility: LandlockCompatibility,
) -> Result<LandlockRestrictionStatus, String> {
    validate_restriction_parts(&status.ruleset, status.no_new_privs, compatibility)
        .map(|()| status.into())
}

fn validate_restriction_parts(
    ruleset: &RulesetStatus,
    no_new_privs: bool,
    compatibility: LandlockCompatibility,
) -> Result<(), String> {
    if ruleset == &RulesetStatus::FullyEnforced {
        if no_new_privs || compatibility == LandlockCompatibility::BestEffort {
            Ok(())
        } else {
            Err(
                "landlock FullyEnforced but no_new_privs=false under HardRequirement compatibility"
                    .into(),
            )
        }
    } else if ruleset == &RulesetStatus::PartiallyEnforced
        && compatibility == LandlockCompatibility::BestEffort
    {
        Ok(())
    } else if ruleset == &RulesetStatus::PartiallyEnforced {
        Err("landlock partially enforced under HardRequirement compatibility".into())
    } else if ruleset == &RulesetStatus::NotEnforced {
        Err("landlock not enforced (kernel returned NotEnforced status)".into())
    } else {
        Err(format!("unknown landlock ruleset status: {ruleset:?}"))
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

    #[test]
    fn ac87_scope_bitmap_contains_abstract_socket_and_signal() {
        assert_eq!(
            LANDLOCK_SCOPE_ALL,
            LANDLOCK_SCOPE_ABSTRACT_UNIX_SOCKET | LANDLOCK_SCOPE_SIGNAL
        );
    }

    #[test]
    fn ac88_audit_flag_matches_kernel_uapi_bit() {
        assert_eq!(LANDLOCK_RESTRICT_SELF_LOG_NEW_EXEC_ON, 1 << 1);
    }

    #[test]
    fn raw_v1_access_masks_match_crate_v1_shape() {
        let abi = ABI::V1;
        assert_eq!(LANDLOCK_ACCESS_FS_V1_ALL, AccessFs::from_all(abi).bits());
        assert_eq!(LANDLOCK_ACCESS_FS_V1_READ, AccessFs::from_read(abi).bits());
    }

    #[test]
    fn raw_path_beneath_attr_matches_kernel_c_layout() {
        assert_eq!(std::mem::size_of::<RawPathBeneathAttr>(), 16);
    }

    #[test]
    fn best_effort_accepts_partially_enforced_status() {
        validate_restriction_parts(
            &RulesetStatus::PartiallyEnforced,
            true,
            LandlockCompatibility::BestEffort,
        )
        .expect("best-effort should accept partial status");
    }

    #[test]
    fn hard_requirement_rejects_partially_enforced_status() {
        let result = validate_restriction_parts(
            &RulesetStatus::PartiallyEnforced,
            true,
            LandlockCompatibility::HardRequirement,
        );
        let error = result.expect_err("partial status must be rejected");
        assert!(
            error.contains("partially enforced"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn hard_requirement_rejects_missing_no_new_privs() {
        let result = validate_restriction_parts(
            &RulesetStatus::FullyEnforced,
            false,
            LandlockCompatibility::HardRequirement,
        );
        let error = result.expect_err("missing no_new_privs must be rejected");
        assert!(
            error.contains("no_new_privs=false"),
            "unexpected error: {error}"
        );
    }
}
