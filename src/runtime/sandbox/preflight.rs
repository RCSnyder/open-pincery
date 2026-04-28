//! AC-84 / Slice G0b.1: kernel ABI floor preflight.
//!
//! Runs once at server startup and asserts every kernel primitive
//! the v9 sandbox depends on is present and recent enough. Any
//! failure aborts startup with exit code 4 (distinct from config
//! errors, DB errors, and normal process termination) and a
//! `sandbox_kernel_floor_unmet` log record naming the missing
//! requirement.
//!
//! ## Why preflight at server start (not per-call)
//!
//! The kernel surface cannot change under a running process.
//! Checking once at `pincery-server` startup means:
//!
//! - Operators get actionable errors before the first sandboxed
//!   call instead of the Nth, making deploy-time failures obvious.
//! - The per-call spawn path stays hot — no probe syscalls per
//!   invocation.
//! - A host that fails the floor cannot accidentally ship
//!   `landlock=audit` tool runs that silently no-op; the process
//!   refuses to start instead.
//!
//! ## Relaxed opt-out (`OPEN_PINCERY_SANDBOX_FLOOR=relaxed`)
//!
//! Operators running on kernels older than the floor can set
//! `OPEN_PINCERY_SANDBOX_FLOOR=relaxed` to downgrade the Landlock
//! requirement from ABI ≥ 6 to ABI ≥ 1. This surfaces a
//! `sandbox_floor_relaxed` startup warning and REQUIRES
//! `OPEN_PINCERY_ALLOW_UNSAFE=true` as a second confirmation — the
//! same pattern used for `SandboxMode::Disabled` in `config.rs`.
//! Relaxed mode does not skip the other preflight checks (seccomp,
//! cgroup v2, userns, bwrap); those are hard requirements for the
//! sandbox to boot at all.
//!
//! ## Testability
//!
//! All five kernel probes are routed through the [`KernelProbe`]
//! trait. Production uses [`RealKernelProbe`], which binds directly
//! to the relevant syscalls / filesystem reads. Tests use a
//! handwritten [`StubKernelProbe`] to exercise each rejection branch
//! without needing a kernel that actually lacks the feature. The
//! trait boundary is the only seam between this module and the
//! kernel.
//!
//! ## Slice scope (G0b.1)
//!
//! This slice ships the module + real probe + stub probe + unit
//! tests. It does NOT wire `assert_kernel_floor` into `main.rs`.
//! Slice G0b.2 adds the wiring, the exit-4 translation, and
//! documentation updates.

/// Minimum required Landlock ABI in strict (default) mode.
///
/// ABI 6 landed in Linux 6.7. It adds IPC scoping primitives
/// (`LANDLOCK_SCOPE_ABSTRACT_UNIX_SOCKET`,
/// `LANDLOCK_SCOPE_SIGNAL`) which AC-87 relies on, so the floor
/// is set to the ABI that provides the highest-requirement feature
/// in the v9 threat model. Older ABIs can be opted into via the
/// relaxed path but then the scope flags drop and AC-87 emits a
/// `sandbox_scope_unavailable` warning at startup.
pub const LANDLOCK_ABI_FLOOR: u32 = 6;

/// Minimum required Landlock ABI when the operator has opted into
/// `OPEN_PINCERY_SANDBOX_FLOOR=relaxed` + `OPEN_PINCERY_ALLOW_UNSAFE=true`.
///
/// ABI 1 is the original Landlock (Linux 5.13). Below this, the
/// landlock syscall returns ENOSYS and the sandbox has no
/// filesystem LSM layer at all — we still refuse to run even in
/// relaxed mode.
pub const LANDLOCK_ABI_RELAXED_FLOOR: u32 = 1;

/// Minimum required `bwrap --version` output, parsed as
/// `(major, minor, patch)`. 0.8.0 introduced the `--cap-drop`
/// flag AC-86 relies on; it is also the first release with the
/// post-CVE-2020-5291 argv handling fix. Lower versions cannot
/// produce a sandbox that satisfies AC-53's A2b.3 evidence gate.
pub const BWRAP_MIN_VERSION: (u32, u32, u32) = (0, 8, 0);

/// Abstraction over the five kernel primitives the preflight
/// probes for. Implementors should keep each probe read-only and
/// deterministic — `assert_kernel_floor` calls them in sequence and
/// each probe is allowed to be invoked zero or multiple times.
pub trait KernelProbe {
    /// Best Landlock ABI supported by the running kernel, or
    /// `None` if the `landlock_create_ruleset` syscall returns
    /// `ENOSYS` (kernels < 5.13 without landlock support).
    fn landlock_abi(&self) -> Option<u32>;

    /// Whether `prctl(PR_GET_SECCOMP)` returns without error.
    /// A false value indicates seccomp-bpf is either unavailable
    /// or disabled at kernel-config time (rare in 2026 but
    /// possible on hardened embedded builds).
    fn seccomp_available(&self) -> bool;

    /// Whether cgroup v2 is mounted at `/sys/fs/cgroup`. The
    /// canonical marker is the presence of
    /// `/sys/fs/cgroup/cgroup.controllers`, which only exists on
    /// a unified cgroup v2 hierarchy.
    fn cgroup_v2_mounted(&self) -> bool;

    /// Whether the Debian/Ubuntu-specific unprivileged-userns gate
    /// allows non-root callers to create user namespaces. Upstream
    /// kernels do not expose this file; absence is treated as allowed.
    /// Root-owned processes bypass this gate.
    fn unprivileged_userns_clone_allowed(&self) -> bool;

    /// Whether the upstream user namespace quota is positive. Root
    /// does NOT bypass this requirement because `max_user_namespaces=0`
    /// prevents even root-started bwrap from creating a user namespace.
    fn userns_quota_available(&self) -> bool;

    /// Best `bwrap --version` detected on `$PATH`, parsed as
    /// `(major, minor, patch)`. `None` means either bwrap is not
    /// on `$PATH` or the version string could not be parsed.
    fn bwrap_version(&self) -> Option<(u32, u32, u32)>;

    /// Whether the running process is uid 0 (real uid). Used to
    /// short-circuit the Debian/Ubuntu unprivileged-userns gate.
    fn is_root(&self) -> bool;
}

/// Environment-derived inputs to `assert_kernel_floor`. Kept as a
/// struct (not raw env reads) so tests can construct any
/// combination of flags without touching the process environment.
#[derive(Debug, Clone, Copy, Default)]
pub struct FloorEnv {
    /// `OPEN_PINCERY_SANDBOX_FLOOR=relaxed` — downgrades Landlock
    /// requirement from ABI 6 to ABI 1.
    pub relaxed: bool,
    /// `OPEN_PINCERY_ALLOW_UNSAFE=true` — required companion to
    /// `relaxed`. Same pattern as `config.rs` uses for
    /// `SandboxMode::Disabled`.
    pub allow_unsafe: bool,
}

impl FloorEnv {
    /// Read the relevant env vars from the process environment.
    /// Exposed so `main.rs` and integration tests can both funnel
    /// through the same parse logic.
    pub fn from_env() -> Self {
        let relaxed = std::env::var("OPEN_PINCERY_SANDBOX_FLOOR")
            .ok()
            .map(|v| v.trim().eq_ignore_ascii_case("relaxed"))
            .unwrap_or(false);
        let allow_unsafe = std::env::var("OPEN_PINCERY_ALLOW_UNSAFE")
            .ok()
            .map(|v| v.trim().eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        Self {
            relaxed,
            allow_unsafe,
        }
    }
}

/// Result of a successful preflight run. `Relaxed` carries the
/// observed Landlock ABI so the caller can log it alongside the
/// `sandbox_floor_relaxed` warning event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloorOutcome {
    /// All five checks met the strict floor.
    Passed { landlock_abi: u32 },
    /// Relaxed path taken: Landlock ABI is below the strict floor
    /// but ≥ the relaxed floor, and the operator has confirmed the
    /// downgrade via both env vars.
    Relaxed { landlock_abi: u32 },
}

/// Every way the preflight can reject the running environment.
/// The `Display` impl renders the operator-facing message;
/// `main.rs` will emit this alongside a `sandbox_kernel_floor_unmet`
/// structured log record and exit 4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FloorError {
    /// Landlock syscall returned ENOSYS (kernel < 5.13). No
    /// filesystem LSM available at all — refuse to run even in
    /// relaxed mode.
    LandlockUnsupported,
    /// Landlock ABI is below the active floor.
    LandlockTooOld { found: u32, required: u32 },
    /// `prctl(PR_GET_SECCOMP)` returned an error. Kernel was built
    /// without `CONFIG_SECCOMP_FILTER`.
    SeccompUnavailable,
    /// `/sys/fs/cgroup/cgroup.controllers` not present — either
    /// cgroups are absent or the host is still on cgroup v1.
    CgroupV2NotMounted,
    /// `/proc/sys/kernel/unprivileged_userns_clone` reads as `0`
    /// and the caller is not uid 0. bwrap cannot create its user
    /// namespace.
    UnprivilegedUsernsDisabled,
    /// bwrap not found on `$PATH`, or its version string could
    /// not be parsed.
    BwrapMissing,
    /// bwrap is present but older than `BWRAP_MIN_VERSION`. AC-86
    /// cap-drop flag not available.
    BwrapTooOld {
        found: (u32, u32, u32),
        required: (u32, u32, u32),
    },
    /// `OPEN_PINCERY_SANDBOX_FLOOR=relaxed` was set but
    /// `OPEN_PINCERY_ALLOW_UNSAFE=true` was not. The relaxed path
    /// explicitly requires a second confirmation.
    RelaxedWithoutAllowUnsafe,
}

impl std::fmt::Display for FloorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LandlockUnsupported => write!(
                f,
                "Landlock syscall returned ENOSYS — kernel < 5.13 has no \
                 filesystem LSM support. Sandbox cannot boot."
            ),
            Self::LandlockTooOld { found, required } => write!(
                f,
                "Landlock ABI {found} on host; sandbox requires ABI >= \
                 {required}. Upgrade to Linux >= 6.7, or set \
                 OPEN_PINCERY_SANDBOX_FLOOR=relaxed (with \
                 OPEN_PINCERY_ALLOW_UNSAFE=true) to downgrade."
            ),
            Self::SeccompUnavailable => write!(
                f,
                "seccomp-bpf not available on this kernel \
                 (CONFIG_SECCOMP_FILTER). Sandbox cannot boot."
            ),
            Self::CgroupV2NotMounted => write!(
                f,
                "cgroup v2 not mounted at /sys/fs/cgroup. Sandbox \
                 resource limits require a unified cgroup v2 hierarchy."
            ),
            Self::UnprivilegedUsernsDisabled => write!(
                f,
                "Unprivileged user namespaces are disabled \
                  (/proc/sys/kernel/unprivileged_userns_clone = 0 or \
                  /proc/sys/user/max_user_namespaces = 0/unreadable). \
                  Non-root callers require unprivileged_userns_clone=1; \
                  all callers require max_user_namespaces > 0. bwrap cannot create its \
                 sandbox namespace."
            ),
            Self::BwrapMissing => write!(
                f,
                "bwrap not found on $PATH or version unreadable. \
                 Install bubblewrap >= {}.{}.{}.",
                BWRAP_MIN_VERSION.0, BWRAP_MIN_VERSION.1, BWRAP_MIN_VERSION.2
            ),
            Self::BwrapTooOld { found, required } => write!(
                f,
                "bwrap {}.{}.{} is older than required {}.{}.{}. \
                 Upgrade bubblewrap to pick up --cap-drop and the \
                 post-CVE-2020-5291 argv-handling fix.",
                found.0, found.1, found.2, required.0, required.1, required.2
            ),
            Self::RelaxedWithoutAllowUnsafe => write!(
                f,
                "OPEN_PINCERY_SANDBOX_FLOOR=relaxed requires \
                 OPEN_PINCERY_ALLOW_UNSAFE=true as a second \
                 confirmation. Refusing to run."
            ),
        }
    }
}

impl std::error::Error for FloorError {}

/// Assert that the running host meets the kernel ABI floor.
///
/// ## Ordering
///
/// Checks are run in the following order (first failure wins):
///
/// 1. Relaxed-without-allow-unsafe — fail fast before touching any probe
///    so a misconfigured env aborts immediately.
/// 2. Landlock ABI — the most restrictive kernel requirement and the
///    one most likely to fail on older LTS hosts.
/// 3. seccomp-bpf availability.
/// 4. cgroup v2 mounted.
/// 5. unprivileged userns (or root).
/// 6. bwrap present + version.
///
/// The order is operator-ergonomic: users fix the loudest problem
/// first. Each check is independent so re-running after a fix
/// surfaces the next issue.
pub fn assert_kernel_floor(
    probe: &dyn KernelProbe,
    env: &FloorEnv,
) -> Result<FloorOutcome, FloorError> {
    // Step 1: env-var consistency.
    if env.relaxed && !env.allow_unsafe {
        return Err(FloorError::RelaxedWithoutAllowUnsafe);
    }

    // Step 2: Landlock ABI.
    let landlock_abi = probe
        .landlock_abi()
        .ok_or(FloorError::LandlockUnsupported)?;
    let required = if env.relaxed {
        LANDLOCK_ABI_RELAXED_FLOOR
    } else {
        LANDLOCK_ABI_FLOOR
    };
    if landlock_abi < required {
        return Err(FloorError::LandlockTooOld {
            found: landlock_abi,
            required,
        });
    }

    // Step 3: seccomp-bpf.
    if !probe.seccomp_available() {
        return Err(FloorError::SeccompUnavailable);
    }

    // Step 4: cgroup v2.
    if !probe.cgroup_v2_mounted() {
        return Err(FloorError::CgroupV2NotMounted);
    }

    // Step 5: userns quota AND (Debian/Ubuntu unprivileged gate OR root).
    if !probe.userns_quota_available()
        || (!probe.is_root() && !probe.unprivileged_userns_clone_allowed())
    {
        return Err(FloorError::UnprivilegedUsernsDisabled);
    }

    // Step 6: bwrap version.
    let bwrap = probe.bwrap_version().ok_or(FloorError::BwrapMissing)?;
    if bwrap < BWRAP_MIN_VERSION {
        return Err(FloorError::BwrapTooOld {
            found: bwrap,
            required: BWRAP_MIN_VERSION,
        });
    }

    if env.relaxed {
        Ok(FloorOutcome::Relaxed { landlock_abi })
    } else {
        Ok(FloorOutcome::Passed { landlock_abi })
    }
}

/// Run the kernel-floor startup gate with explicit logging and
/// process-exit semantics expected by AC-84.
///
/// Returns `Ok(())` when startup should continue, or `Err(4)` when
/// startup must abort due to unmet kernel prerequisites.
pub fn enforce_kernel_floor_at_startup() -> Result<(), i32> {
    run_startup_preflight_with(&RealKernelProbe, &FloorEnv::from_env())
}

fn run_startup_preflight_with(probe: &dyn KernelProbe, env: &FloorEnv) -> Result<(), i32> {
    match assert_kernel_floor(probe, env) {
        Ok(FloorOutcome::Passed { landlock_abi }) => {
            tracing::info!(
                event = "sandbox_kernel_floor_ok",
                landlock_abi,
                strict_floor = LANDLOCK_ABI_FLOOR,
                "Kernel sandbox floor preflight passed"
            );
            Ok(())
        }
        Ok(FloorOutcome::Relaxed { landlock_abi }) => {
            tracing::warn!(
                event = "sandbox_floor_relaxed",
                landlock_abi,
                strict_floor = LANDLOCK_ABI_FLOOR,
                relaxed_floor = LANDLOCK_ABI_RELAXED_FLOOR,
                "Kernel floor relaxed via OPEN_PINCERY_SANDBOX_FLOOR=relaxed"
            );
            Ok(())
        }
        Err(error) => {
            tracing::error!(
                event = "sandbox_kernel_floor_unmet",
                error = %error,
                "Kernel sandbox floor preflight failed"
            );
            Err(4)
        }
    }
}

/// Production probe — binds to real kernel/filesystem/process
/// primitives. All probe methods are side-effect-free.
pub struct RealKernelProbe;

impl KernelProbe for RealKernelProbe {
    fn landlock_abi(&self) -> Option<u32> {
        // Per the landlock(7) man page: calling
        // `landlock_create_ruleset(NULL, 0, LANDLOCK_CREATE_RULESET_VERSION)`
        // with the version-query flag returns the highest ABI
        // version supported by the running kernel. ENOSYS means the
        // syscall does not exist at all (kernel < 5.13).
        //
        // The version-query flag is `1u32 << 0`; it is not yet
        // exposed by `libc` so we pass the literal.
        const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1;
        // SAFETY: the syscall is idempotent with no arguments that
        // alias memory. On success returns a non-negative ABI
        // version; on error returns -1 with errno set.
        let rc = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                std::ptr::null::<u8>(),
                0usize,
                LANDLOCK_CREATE_RULESET_VERSION,
            )
        };
        if rc < 0 {
            None
        } else {
            Some(rc as u32)
        }
    }

    fn seccomp_available(&self) -> bool {
        // `prctl(PR_GET_SECCOMP)` returns the current seccomp mode
        // (0, 1, or 2) on success. On kernels without
        // CONFIG_SECCOMP_FILTER the prctl returns -1 / EINVAL.
        // SAFETY: pure getter, no pointer arguments.
        let rc = unsafe { libc::prctl(libc::PR_GET_SECCOMP, 0u64, 0u64, 0u64, 0u64) };
        rc >= 0
    }

    fn cgroup_v2_mounted(&self) -> bool {
        // Unified cgroup v2 hierarchies always expose
        // `cgroup.controllers` at the root. cgroup v1 mounts
        // per-controller subdirs (`/sys/fs/cgroup/memory`, etc.)
        // and never creates this file at the root.
        std::path::Path::new("/sys/fs/cgroup/cgroup.controllers").exists()
    }

    fn unprivileged_userns_clone_allowed(&self) -> bool {
        match std::fs::read_to_string(UNPRIVILEGED_USERNS_CLONE_PATH) {
            Ok(s) => parse_zero_one_sysctl(&s).unwrap_or(false),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
            Err(_) => false,
        }
    }

    fn userns_quota_available(&self) -> bool {
        let max_namespaces = match std::fs::read_to_string(MAX_USER_NAMESPACES_PATH) {
            Ok(s) => s,
            Err(_) => return false,
        };
        userns_quota_available_from_sysctl(Some(&max_namespaces))
    }

    fn bwrap_version(&self) -> Option<(u32, u32, u32)> {
        // `bwrap --version` prints `bubblewrap 0.11.0` on stdout
        // (1 line). Any other shape is unexpected and treated as
        // "unreadable" → BwrapMissing.
        let output = std::process::Command::new("bwrap")
            .arg("--version")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = std::str::from_utf8(&output.stdout).ok()?;
        parse_bwrap_version(stdout)
    }

    fn is_root(&self) -> bool {
        // SAFETY: pure getter.
        unsafe { libc::getuid() == 0 }
    }
}

const UNPRIVILEGED_USERNS_CLONE_PATH: &str = "/proc/sys/kernel/unprivileged_userns_clone";
const MAX_USER_NAMESPACES_PATH: &str = "/proc/sys/user/max_user_namespaces";

fn parse_zero_one_sysctl(raw: &str) -> Option<bool> {
    match raw.trim() {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

fn parse_max_user_namespaces(raw: &str) -> Option<bool> {
    raw.trim().parse::<u64>().ok().map(|limit| limit > 0)
}

fn userns_quota_available_from_sysctl(max_user_namespaces: Option<&str>) -> bool {
    max_user_namespaces
        .and_then(parse_max_user_namespaces)
        .unwrap_or(false)
}

/// Parse `bubblewrap X.Y.Z` or `X.Y.Z` into a version triple.
/// Returns `None` on any parse failure. Exposed for unit tests.
pub fn parse_bwrap_version(raw: &str) -> Option<(u32, u32, u32)> {
    let first_line = raw.lines().next()?.trim();
    // Accept both `bubblewrap 0.11.0` and bare `0.11.0`.
    let version = first_line
        .strip_prefix("bubblewrap ")
        .unwrap_or(first_line)
        .trim();
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    // Patch may be followed by `-rc1`, `+git`, etc. — take the
    // leading digits only.
    let patch_raw = parts.next()?;
    let patch_digits: String = patch_raw
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if patch_digits.is_empty() {
        return None;
    }
    let patch = patch_digits.parse().ok()?;
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Handwritten stub probe. Each field directly backs one trait
    /// method; tests build the probe with exactly the failure shape
    /// they're exercising.
    struct StubKernelProbe {
        landlock_abi: Option<u32>,
        seccomp_available: bool,
        cgroup_v2_mounted: bool,
        unprivileged_userns_clone_allowed: bool,
        userns_quota_available: bool,
        bwrap_version: Option<(u32, u32, u32)>,
        is_root: bool,
    }

    impl StubKernelProbe {
        /// Canonical "compliant kernel" stub — every probe satisfies
        /// the strict floor. Tests override single fields from here.
        fn compliant() -> Self {
            Self {
                landlock_abi: Some(LANDLOCK_ABI_FLOOR),
                seccomp_available: true,
                cgroup_v2_mounted: true,
                unprivileged_userns_clone_allowed: true,
                userns_quota_available: true,
                bwrap_version: Some(BWRAP_MIN_VERSION),
                is_root: false,
            }
        }
    }

    impl KernelProbe for StubKernelProbe {
        fn landlock_abi(&self) -> Option<u32> {
            self.landlock_abi
        }
        fn seccomp_available(&self) -> bool {
            self.seccomp_available
        }
        fn cgroup_v2_mounted(&self) -> bool {
            self.cgroup_v2_mounted
        }
        fn unprivileged_userns_clone_allowed(&self) -> bool {
            self.unprivileged_userns_clone_allowed
        }
        fn userns_quota_available(&self) -> bool {
            self.userns_quota_available
        }
        fn bwrap_version(&self) -> Option<(u32, u32, u32)> {
            self.bwrap_version
        }
        fn is_root(&self) -> bool {
            self.is_root
        }
    }

    fn strict_env() -> FloorEnv {
        FloorEnv {
            relaxed: false,
            allow_unsafe: false,
        }
    }

    fn relaxed_env() -> FloorEnv {
        FloorEnv {
            relaxed: true,
            allow_unsafe: true,
        }
    }

    #[test]
    fn compliant_kernel_passes_strict() {
        let probe = StubKernelProbe::compliant();
        let outcome = assert_kernel_floor(&probe, &strict_env()).unwrap();
        assert_eq!(
            outcome,
            FloorOutcome::Passed {
                landlock_abi: LANDLOCK_ABI_FLOOR
            }
        );
    }

    #[test]
    fn landlock_syscall_enosys_is_rejected_even_in_relaxed() {
        let probe = StubKernelProbe {
            landlock_abi: None,
            ..StubKernelProbe::compliant()
        };
        // Strict: LandlockUnsupported.
        assert_eq!(
            assert_kernel_floor(&probe, &strict_env()),
            Err(FloorError::LandlockUnsupported)
        );
        // Relaxed: still LandlockUnsupported — the relaxed path
        // downgrades to ABI 1, not to "no landlock at all".
        assert_eq!(
            assert_kernel_floor(&probe, &relaxed_env()),
            Err(FloorError::LandlockUnsupported)
        );
    }

    #[test]
    fn landlock_abi_5_is_rejected_in_strict_mode() {
        let probe = StubKernelProbe {
            landlock_abi: Some(5),
            ..StubKernelProbe::compliant()
        };
        assert_eq!(
            assert_kernel_floor(&probe, &strict_env()),
            Err(FloorError::LandlockTooOld {
                found: 5,
                required: LANDLOCK_ABI_FLOOR
            })
        );
    }

    #[test]
    fn landlock_abi_1_passes_in_relaxed_mode() {
        let probe = StubKernelProbe {
            landlock_abi: Some(1),
            ..StubKernelProbe::compliant()
        };
        let outcome = assert_kernel_floor(&probe, &relaxed_env()).unwrap();
        assert_eq!(outcome, FloorOutcome::Relaxed { landlock_abi: 1 });
    }

    #[test]
    fn relaxed_without_allow_unsafe_is_rejected() {
        let probe = StubKernelProbe::compliant();
        let env = FloorEnv {
            relaxed: true,
            allow_unsafe: false,
        };
        assert_eq!(
            assert_kernel_floor(&probe, &env),
            Err(FloorError::RelaxedWithoutAllowUnsafe)
        );
    }

    #[test]
    fn seccomp_unavailable_is_rejected() {
        let probe = StubKernelProbe {
            seccomp_available: false,
            ..StubKernelProbe::compliant()
        };
        assert_eq!(
            assert_kernel_floor(&probe, &strict_env()),
            Err(FloorError::SeccompUnavailable)
        );
    }

    #[test]
    fn cgroup_v2_missing_is_rejected() {
        let probe = StubKernelProbe {
            cgroup_v2_mounted: false,
            ..StubKernelProbe::compliant()
        };
        assert_eq!(
            assert_kernel_floor(&probe, &strict_env()),
            Err(FloorError::CgroupV2NotMounted)
        );
    }

    #[test]
    fn userns_disabled_non_root_is_rejected() {
        let probe = StubKernelProbe {
            unprivileged_userns_clone_allowed: false,
            userns_quota_available: true,
            is_root: false,
            ..StubKernelProbe::compliant()
        };
        assert_eq!(
            assert_kernel_floor(&probe, &strict_env()),
            Err(FloorError::UnprivilegedUsernsDisabled)
        );
    }

    #[test]
    fn userns_disabled_but_root_passes() {
        // Root can unshare(CLONE_NEWUSER) regardless of the Debian/Ubuntu
        // unprivileged_userns_clone sysctl.
        let probe = StubKernelProbe {
            unprivileged_userns_clone_allowed: false,
            userns_quota_available: true,
            is_root: true,
            ..StubKernelProbe::compliant()
        };
        assert!(assert_kernel_floor(&probe, &strict_env()).is_ok());
    }

    #[test]
    fn userns_quota_disabled_is_rejected_even_for_root() {
        let probe = StubKernelProbe {
            unprivileged_userns_clone_allowed: true,
            userns_quota_available: false,
            is_root: true,
            ..StubKernelProbe::compliant()
        };
        assert_eq!(
            assert_kernel_floor(&probe, &strict_env()),
            Err(FloorError::UnprivilegedUsernsDisabled)
        );
    }

    #[test]
    fn bwrap_missing_is_rejected() {
        let probe = StubKernelProbe {
            bwrap_version: None,
            ..StubKernelProbe::compliant()
        };
        assert_eq!(
            assert_kernel_floor(&probe, &strict_env()),
            Err(FloorError::BwrapMissing)
        );
    }

    #[test]
    fn bwrap_too_old_is_rejected() {
        let probe = StubKernelProbe {
            bwrap_version: Some((0, 7, 0)),
            ..StubKernelProbe::compliant()
        };
        assert_eq!(
            assert_kernel_floor(&probe, &strict_env()),
            Err(FloorError::BwrapTooOld {
                found: (0, 7, 0),
                required: BWRAP_MIN_VERSION
            })
        );
    }

    #[test]
    fn parse_bwrap_version_canonical_form() {
        assert_eq!(parse_bwrap_version("bubblewrap 0.11.0\n"), Some((0, 11, 0)));
    }

    #[test]
    fn parse_bwrap_version_bare_form() {
        assert_eq!(parse_bwrap_version("0.8.2"), Some((0, 8, 2)));
    }

    #[test]
    fn parse_bwrap_version_patch_suffix_is_stripped() {
        assert_eq!(parse_bwrap_version("bubblewrap 0.9.0-rc1"), Some((0, 9, 0)));
    }

    #[test]
    fn parse_bwrap_version_rejects_garbage() {
        assert_eq!(parse_bwrap_version(""), None);
        assert_eq!(parse_bwrap_version("not a version"), None);
        assert_eq!(parse_bwrap_version("0.11"), None); // Missing patch.
    }

    #[test]
    fn userns_sysctls_allow_when_debian_gate_absent_and_namespace_quota_positive() {
        assert!(userns_quota_available_from_sysctl(Some("1024\n")));
    }

    #[test]
    fn userns_sysctls_reject_when_max_user_namespaces_is_zero() {
        assert!(!userns_quota_available_from_sysctl(Some("0\n")));
    }

    #[test]
    fn userns_sysctls_reject_when_debian_gate_disables_userns() {
        assert_eq!(parse_zero_one_sysctl("0\n"), Some(false));
    }

    #[test]
    fn userns_sysctls_reject_malformed_or_missing_quota() {
        assert!(!userns_quota_available_from_sysctl(Some("not-a-number")));
        assert_eq!(parse_zero_one_sysctl("maybe\n"), None);
        assert!(!userns_quota_available_from_sysctl(None));
    }

    #[test]
    fn floor_env_from_env_reads_relaxed_and_allow_unsafe() {
        // Shield test against concurrent env writes: serialise via
        // a simple pair of set/unset and rely on --test-threads=1
        // (CI-enforced) for isolation.
        const FLOOR: &str = "OPEN_PINCERY_SANDBOX_FLOOR";
        const UNSAFE: &str = "OPEN_PINCERY_ALLOW_UNSAFE";
        // SAFETY: single-threaded test; env is process-global.
        unsafe {
            std::env::set_var(FLOOR, "relaxed");
            std::env::set_var(UNSAFE, "true");
        }
        let env = FloorEnv::from_env();
        assert!(env.relaxed);
        assert!(env.allow_unsafe);
        unsafe {
            std::env::remove_var(FLOOR);
            std::env::remove_var(UNSAFE);
        }
        let env = FloorEnv::from_env();
        assert!(!env.relaxed);
        assert!(!env.allow_unsafe);
    }

    /// Production probe sanity: just ensures the real probe does
    /// not panic when exercised. Values are host-dependent; any
    /// assertion about exact outcomes would be flaky on older
    /// kernels in CI.
    #[test]
    fn real_probe_does_not_panic() {
        let probe = RealKernelProbe;
        let _ = probe.landlock_abi();
        let _ = probe.seccomp_available();
        let _ = probe.cgroup_v2_mounted();
        let _ = probe.unprivileged_userns_clone_allowed();
        let _ = probe.userns_quota_available();
        let _ = probe.bwrap_version();
        let _ = probe.is_root();
    }

    #[test]
    fn startup_preflight_returns_exit_4_on_failure() {
        let probe = StubKernelProbe {
            landlock_abi: Some(5),
            ..StubKernelProbe::compliant()
        };
        let rc = run_startup_preflight_with(&probe, &strict_env());
        assert_eq!(rc, Err(4));
    }

    #[test]
    fn startup_preflight_accepts_relaxed_with_allow_unsafe() {
        let probe = StubKernelProbe {
            landlock_abi: Some(1),
            ..StubKernelProbe::compliant()
        };
        let rc = run_startup_preflight_with(&probe, &relaxed_env());
        assert_eq!(rc, Ok(()));
    }
}
