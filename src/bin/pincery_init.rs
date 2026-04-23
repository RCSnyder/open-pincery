//! AC-83: `pincery-init` exec wrapper.
//!
//! This binary runs **inside** the bwrap sandbox after mount/namespace
//! setup is complete. It reads a serialized [`SandboxInitPolicy`] from
//! an inherited file descriptor, applies every restriction in the
//! mandated order (readiness T-G0a-6), then `execvp`s the user's real
//! argv.
//!
//! ## Coverage by slice
//!
//! - G0a.2 (shipped): argv parse, policy read, decode, summary log,
//!   `execvp`. No restrictions.
//! - G0a.3a (shipped): `prctl(PR_SET_NO_NEW_PRIVS, 1)` + verify.
//! - G0a.3b (shipped): drop r/e/s uid+gid via
//!   `setresgid -> setgroups(0, NULL) -> setresuid` with
//!   `getresuid`/`getresgid` verification. Short-circuits when already
//!   at target (host-test accommodation; does not fire inside bwrap).
//! - G0a.3c (shipped): install seccomp filter via
//!   `prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, &sock_fprog)`.
//!   Verifies install via `/proc/self/status`'s `Seccomp:` line.
//! - G0a.3d (shipped): install landlock filesystem ruleset via
//!   `runtime::sandbox::landlock_layer::install_landlock`, placed
//!   BEFORE seccomp so the filter does not need to allow the
//!   `landlock_*` syscalls. Gate on at least one rx/rwx path.
//! - G0a.3e (this slice): if `policy.require_fully_enforced` is
//!   `true`, verify that every requested layer actually enforced:
//!   landlock status is `RulesetStatus::FullyEnforced`, seccomp
//!   mode in `/proc/self/status` is `2`, `NoNewPrivs` is `1`.
//!   Fails closed with `InitError::VerifyPolicy` otherwise.
//! - G0a.3f..h (pending): JSON fd-3 error channel → RealSandbox
//!   rewiring → default flip + un-ignore landlock tests.
//!
//! ## Still out of scope until later sub-slices
//!
//! - Fail-closed JSON error channel on fd 3. The current code still
//!   uses stderr + exit 125 for any pre-exec failure; G0a.3f reshapes
//!   that into the structured JSON channel the parent can parse.
//! - musl-static linking. The wrapper is dynamically linked
//!   (see readiness T-G0a-1 and the G0a-followup tracking item).
//!
//! ## Why a separate binary (not argv[0] dispatch)
//!
//! Per readiness clarification 1: a dedicated `[[bin]]` target keeps
//! the build graph and `ps` output unambiguous, and lets us strip the
//! wrapper down to `panic = "abort"` + minimal deps later without
//! pulling server-side code along for the ride.

#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!(
        "pincery-init: this binary is Linux-only (requires Landlock + \
         seccomp-bpf kernel surfaces)"
    );
    std::process::exit(1);
}

#[cfg(target_os = "linux")]
fn main() {
    match run() {
        Ok(never) => match never {},
        Err(e) => {
            // G0a.2: stderr + exit 125. G0a.3 will replace this with
            // a structured JSON error on fd 3.
            eprintln!("pincery-init: {e}");
            std::process::exit(125);
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::ffi::OsString;
    use std::fs::File;
    use std::io::Read;
    use std::os::fd::FromRawFd;
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    use open_pincery::runtime::sandbox::init_policy::SandboxInitPolicy;

    /// The `!` return type would be ideal here, but it is still
    /// unstable on stable Rust, so we return an uninhabited enum that
    /// [`main`](super::main) matches exhaustively.
    pub enum Never {}

    /// Error surfaces emitted by the wrapper before `execvp`. In G0a.2
    /// these are printed to stderr; G0a.3 serializes them to JSON on
    /// fd 3. Kept flat: the wrapper has no recovery path.
    #[derive(Debug)]
    pub enum InitError {
        Usage(String),
        BadPolicyFd(String),
        ReadPolicy(std::io::Error),
        DecodePolicy(String),
        /// Applying a kernel restriction (prctl / seccomp / landlock
        /// / setres*id) failed. Message carries the stage + errno.
        ApplyPolicy(String),
        /// Post-apply verification failed (e.g. `no_new_privs` is 0
        /// after `PR_SET_NO_NEW_PRIVS`, or landlock is only
        /// `PartiallyEnforced` under a policy that required
        /// `FullyEnforced`).
        VerifyPolicy(String),
        /// `execvp` returned — which only happens on failure (success
        /// replaces the process image and never returns).
        Exec(std::io::Error),
    }

    impl std::fmt::Display for InitError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Usage(msg) => write!(f, "usage error: {msg}"),
                Self::BadPolicyFd(msg) => write!(f, "invalid --policy-fd: {msg}"),
                Self::ReadPolicy(e) => write!(f, "reading policy fd: {e}"),
                Self::DecodePolicy(msg) => write!(f, "decoding policy: {msg}"),
                Self::ApplyPolicy(msg) => write!(f, "applying policy: {msg}"),
                Self::VerifyPolicy(msg) => write!(f, "verifying policy: {msg}"),
                Self::Exec(e) => write!(f, "execvp user argv: {e}"),
            }
        }
    }

    impl std::error::Error for InitError {}

    /// Apply `prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)` to the current
    /// thread (and by inheritance all threads of this process — the
    /// wrapper is single-threaded at this point). After this call,
    /// `execve` will not grant setuid/setgid/file-capability bits,
    /// which is the prerequisite for loading a seccomp filter as an
    /// unprivileged user (AC-85 / Slice G0a.3b-c) and for any
    /// hardened LSM that uses no_new_privs as an anti-escalation
    /// signal.
    ///
    /// Slice G0a.3a only: this is the first kernel restriction
    /// ever applied from inside the wrapper. It is idempotent, never
    /// fails on a supported kernel (≥ 3.5), and is observable via
    /// `/proc/self/status`'s `NoNewPrivs:` line — which the
    /// integration test in `tests/pincery_init_skeleton_test.rs`
    /// pins on.
    fn apply_no_new_privs() -> Result<(), InitError> {
        // SAFETY: `prctl` with these arguments has no memory effects;
        // the return value is checked below. `PR_SET_NO_NEW_PRIVS`
        // accepts only (1, 0, 0, 0) per prctl(2).
        let rc = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1u64, 0u64, 0u64, 0u64) };
        if rc == 0 {
            Ok(())
        } else {
            Err(InitError::ApplyPolicy(format!(
                "prctl(PR_SET_NO_NEW_PRIVS, 1): {}",
                std::io::Error::last_os_error()
            )))
        }
    }

    /// Verify after the fact that `no_new_privs` is set to 1 via
    /// `prctl(PR_GET_NO_NEW_PRIVS)`. This is a belt-and-braces check
    /// that `apply_no_new_privs` was actually honored by the kernel
    /// (it always is on ≥ 3.5, but future verify steps — FullyEnforced
    /// landlock (G0a.3e) — follow the same pattern, so keep the
    /// symmetry early).
    fn verify_no_new_privs() -> Result<(), InitError> {
        // SAFETY: pure getter; return value is 0 or 1 on success,
        // negative on error.
        let rc = unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0u64, 0u64, 0u64, 0u64) };
        match rc {
            1 => Ok(()),
            0 => Err(InitError::VerifyPolicy(
                "no_new_privs is 0 after apply (kernel did not honor prctl)".into(),
            )),
            _ => Err(InitError::VerifyPolicy(format!(
                "prctl(PR_GET_NO_NEW_PRIVS): {}",
                std::io::Error::last_os_error()
            ))),
        }
    }

    /// Drop real/effective/saved uid+gid to `policy.target_uid` /
    /// `policy.target_gid` and clear supplementary groups. Step 2 of
    /// the T-G0a-6 pipeline. Order within the step is fixed:
    ///
    /// 1. `setresgid(gid, gid, gid)` — must come first. After
    ///    `setresuid` we lose `CAP_SETGID` (if we ever had it), so
    ///    the gid change would be rejected.
    /// 2. `setgroups(0, NULL)` — clear supplementary groups. Also
    ///    requires `CAP_SETGID`, also must precede `setresuid`.
    /// 3. `setresuid(uid, uid, uid)` — finally drop uid. No-op
    ///    verification via `getresuid` / `getresgid` confirms all
    ///    three slots (real/effective/saved) took.
    ///
    /// ## Short-circuit when already at target
    ///
    /// If the current euid/egid already equal the target, this is
    /// a no-op. This is load-bearing for two reasons:
    ///
    /// - Host-level integration tests (which cannot obtain
    ///   `CAP_SETUID`) set `target_uid == geteuid()` so the step is
    ///   skipped and the rest of the pipeline still gets exercised.
    /// - In the real bwrap path (G0a.3g) the wrapper is namespace-
    ///   root (`euid == 0`) and the target is a non-zero unprivileged
    ///   uid, so the short-circuit does NOT fire and the full drop
    ///   runs. This is the only path that actually matters for
    ///   AC-86's privilege isolation; the host-test short-circuit is
    ///   purely a testability accommodation.
    ///
    /// `setgroups(0, NULL)` is skipped alongside the uid/gid change
    /// in the short-circuit case because it also requires
    /// `CAP_SETGID`; calling it as an unprivileged user returns EPERM.
    fn apply_drop_privs(policy: &SandboxInitPolicy) -> Result<(), InitError> {
        // SAFETY: pure getters with no arguments.
        let cur_uid = unsafe { libc::geteuid() };
        let cur_gid = unsafe { libc::getegid() };

        if cur_uid == policy.target_uid && cur_gid == policy.target_gid {
            eprintln!(
                "pincery-init: drop_privs short-circuit (already at \
                 uid={cur_uid} gid={cur_gid})"
            );
            return Ok(());
        }

        let gid: libc::gid_t = policy.target_gid;
        let uid: libc::uid_t = policy.target_uid;

        // Step 1: setresgid(gid, gid, gid). Must precede setresuid.
        // SAFETY: libc FFI; return value is checked.
        let rc = unsafe { libc::setresgid(gid, gid, gid) };
        if rc != 0 {
            return Err(InitError::ApplyPolicy(format!(
                "setresgid({gid}, {gid}, {gid}): {}",
                std::io::Error::last_os_error()
            )));
        }

        // Step 2: clear supplementary groups. Passing a NULL list with
        // size 0 is the canonical way to drop to the empty set per
        // setgroups(2).
        // SAFETY: libc FFI with documented (size=0, list=NULL) form.
        let rc = unsafe { libc::setgroups(0, std::ptr::null()) };
        if rc != 0 {
            return Err(InitError::ApplyPolicy(format!(
                "setgroups(0, NULL): {}",
                std::io::Error::last_os_error()
            )));
        }

        // Step 3: setresuid(uid, uid, uid). This is the point of no
        // return — if we had CAP_SETUID before this call, we don't
        // after it.
        // SAFETY: libc FFI; return value is checked.
        let rc = unsafe { libc::setresuid(uid, uid, uid) };
        if rc != 0 {
            return Err(InitError::ApplyPolicy(format!(
                "setresuid({uid}, {uid}, {uid}): {}",
                std::io::Error::last_os_error()
            )));
        }

        // Belt-and-braces verify: all three uid slots match the
        // target. This catches a kernel that silently dropped a
        // component (which should never happen, but fail loudly if
        // it does).
        let mut ruid: libc::uid_t = libc::uid_t::MAX;
        let mut euid: libc::uid_t = libc::uid_t::MAX;
        let mut suid: libc::uid_t = libc::uid_t::MAX;
        // SAFETY: libc FFI; pointers are to locals that outlive the call.
        let rc = unsafe { libc::getresuid(&mut ruid, &mut euid, &mut suid) };
        if rc != 0 {
            return Err(InitError::VerifyPolicy(format!(
                "getresuid after drop: {}",
                std::io::Error::last_os_error()
            )));
        }
        if ruid != uid || euid != uid || suid != uid {
            return Err(InitError::VerifyPolicy(format!(
                "uid slots mismatch after setresuid: r={ruid} e={euid} s={suid} want={uid}"
            )));
        }

        let mut rgid: libc::gid_t = libc::gid_t::MAX;
        let mut egid: libc::gid_t = libc::gid_t::MAX;
        let mut sgid: libc::gid_t = libc::gid_t::MAX;
        // SAFETY: libc FFI; pointers are to locals that outlive the call.
        let rc = unsafe { libc::getresgid(&mut rgid, &mut egid, &mut sgid) };
        if rc != 0 {
            return Err(InitError::VerifyPolicy(format!(
                "getresgid after drop: {}",
                std::io::Error::last_os_error()
            )));
        }
        if rgid != gid || egid != gid || sgid != gid {
            return Err(InitError::VerifyPolicy(format!(
                "gid slots mismatch after setresgid: r={rgid} e={egid} s={sgid} want={gid}"
            )));
        }

        Ok(())
    }

    /// Install the seccomp-bpf filter from `policy.seccomp_bpf` via
    /// `prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, &fprog)`. Step 3
    /// of the T-G0a-6 pipeline.
    ///
    /// ## Input format
    ///
    /// `policy.seccomp_bpf` is the raw `struct sock_filter[]` byte
    /// stream produced by `runtime::sandbox::seccomp::compose_seccomp_program`
    /// — no `sock_fprog` wrapper, no framing. Each instruction is
    /// 8 bytes (`__u16 code; __u8 jt; __u8 jf; __u32 k`), so the
    /// byte length MUST be a non-zero multiple of 8, and the
    /// instruction count MUST fit in `u16` (kernel limit is
    /// `BPF_MAXINSNS = 32768`, which is well inside `u16::MAX`). The
    /// wrapper enforces both invariants and fails with
    /// `InitError::ApplyPolicy` on violation.
    ///
    /// ## Prerequisite ordering
    ///
    /// `PR_SET_SECCOMP` with `SECCOMP_MODE_FILTER` requires either
    /// `CAP_SYS_ADMIN` or `no_new_privs=1`. `apply_no_new_privs`
    /// (G0a.3a) runs first in `apply_policy`, so we always take the
    /// unprivileged path. If that ever regresses, the prctl here
    /// returns `EACCES` and the wrapper fails closed.
    ///
    /// ## Empty-filter case
    ///
    /// If `policy.seccomp_bpf` is empty we log and skip. The parent
    /// only populates the field when `SandboxProfile.seccomp = true`;
    /// an empty field means "no seccomp layer requested". G0a.3e's
    /// FullyEnforced verify will refuse to accept an empty filter
    /// when `require_fully_enforced = true`.
    ///
    /// ## Verification
    ///
    /// After install, `/proc/self/status`'s `Seccomp:` line must read
    /// 2 (`SECCOMP_MODE_FILTER`). Any other value means the filter
    /// was not installed or was replaced. The integration test pins
    /// on this observable.
    fn apply_seccomp(policy: &SandboxInitPolicy) -> Result<(), InitError> {
        if policy.seccomp_bpf.is_empty() {
            eprintln!("pincery-init: seccomp skipped (policy.seccomp_bpf is empty)");
            return Ok(());
        }

        const SOCK_FILTER_SIZE: usize = std::mem::size_of::<libc::sock_filter>();
        let bytes = &policy.seccomp_bpf;
        if !bytes.len().is_multiple_of(SOCK_FILTER_SIZE) {
            return Err(InitError::ApplyPolicy(format!(
                "seccomp_bpf length {} not a multiple of sock_filter size {}",
                bytes.len(),
                SOCK_FILTER_SIZE,
            )));
        }
        let insn_count = bytes.len() / SOCK_FILTER_SIZE;
        if insn_count == 0 || insn_count > u16::MAX as usize {
            return Err(InitError::ApplyPolicy(format!(
                "seccomp_bpf instruction count {insn_count} out of range (1..=u16::MAX)",
            )));
        }

        // Build a `sock_fprog` pointing at the policy's bytes. The
        // kernel copies the filter into its own memory during the
        // prctl call, so the pointer only needs to stay valid for
        // the duration of the syscall — `bytes` is owned by the
        // `policy` reference which outlives this call.
        let fprog = libc::sock_fprog {
            len: insn_count as u16,
            filter: bytes.as_ptr() as *mut libc::sock_filter,
        };

        // SAFETY: `PR_SET_SECCOMP` with `SECCOMP_MODE_FILTER` and a
        // valid `sock_fprog` pointer. The struct lives on the stack
        // for the duration of the call; the kernel copies before
        // returning.
        let rc = unsafe {
            libc::prctl(
                libc::PR_SET_SECCOMP,
                libc::SECCOMP_MODE_FILTER as libc::c_ulong,
                &fprog as *const libc::sock_fprog as libc::c_ulong,
                0u64,
                0u64,
            )
        };
        if rc != 0 {
            return Err(InitError::ApplyPolicy(format!(
                "prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER): {}",
                std::io::Error::last_os_error()
            )));
        }

        // Verify via /proc/self/status. Reading /proc requires `openat`
        // and `read` — both must be in the allowlist, which they are
        // by construction in `compose_seccomp_program` (they're part
        // of the host-io baseline). If they weren't, the filter would
        // have killed or blocked us before we got here.
        let status = std::fs::read_to_string("/proc/self/status")
            .map_err(|e| InitError::VerifyPolicy(format!("read /proc/self/status: {e}")))?;
        let mode = status
            .lines()
            .find_map(|l| l.strip_prefix("Seccomp:"))
            .map(str::trim)
            .ok_or_else(|| {
                InitError::VerifyPolicy("no Seccomp: line in /proc/self/status".into())
            })?;
        if mode != "2" {
            return Err(InitError::VerifyPolicy(format!(
                "Seccomp mode in /proc/self/status is {mode:?}, expected \"2\" (filter)"
            )));
        }

        Ok(())
    }

    /// Install the landlock filesystem ruleset derived from
    /// `policy.landlock_rx_paths` + `policy.landlock_rwx_paths`.
    /// Step 3 of the T-G0a-6 pipeline (placed BEFORE seccomp so
    /// the seccomp filter does not need to allow the `landlock_*`
    /// syscalls — landlock only restricts filesystem access, and
    /// installing it before the seccomp layer means the filter
    /// catches any post-landlock syscall regardless of fs outcome).
    ///
    /// ## Empty-policy short-circuit
    ///
    /// When both path lists are empty we log and skip, returning
    /// `Ok(None)`. The parent populates the lists only when
    /// `SandboxProfile.landlock = true`, so empty lists mean
    /// "landlock layer not requested". G0a.3e's FullyEnforced verify
    /// refuses to accept empty lists when `require_fully_enforced`
    /// is `true`.
    ///
    /// ## Return value
    ///
    /// `Ok(Some(status))` when the ruleset was installed —
    /// `G0a.3e`'s verify step compares `status` against
    /// `RulesetStatus::FullyEnforced`. `Ok(None)` when install was
    /// skipped (empty policy).
    ///
    /// ## Test-only PartiallyEnforced override
    ///
    /// To exercise the FullyEnforced verify rejection path without
    /// a kernel that actually downgrades to `PartiallyEnforced`,
    /// setting both `OPEN_PINCERY_ALLOW_UNSAFE=true` AND
    /// `OPEN_PINCERY_INIT_FORCE_PARTIAL=1` makes this function
    /// return `Ok(Some(RulesetStatus::PartiallyEnforced))` after the
    /// real install. The two-flag gate matches the existing
    /// `ResolvedSandboxMode` unsafe-opt-in pattern (see `config.rs`)
    /// so the knob cannot be armed by a single env-var typo in
    /// production.
    ///
    /// ## Why we reuse `landlock_layer::install_landlock`
    ///
    /// The existing function is already the single implementation
    /// of the landlock apply path — it builds an `ABI::V1` ruleset,
    /// adds each path as a `PathBeneath` rule (rx = read access,
    /// rwx = all access), calls `restrict_self`, and rejects a
    /// `NotEnforced` result. Duplicating that logic inside the
    /// wrapper would be a pure copy-paste; the G0a fix is
    /// architectural (install inside the sandbox, not in the
    /// parent's `pre_exec`), not algorithmic.
    ///
    /// ## TSYNC note
    ///
    /// Readiness T-G0a-6 lists `LANDLOCK_RESTRICT_SELF_TSYNC` as
    /// the intended flag. `landlock = "0.4"` exposes
    /// `restrict_self()` with flags fixed at 0, and the wrapper
    /// here is single-threaded (no threads exist until after
    /// `execvp`), so TSYNC would be a no-op anyway — landlock
    /// domains are already inherited across `execve` for the
    /// calling task (kernel.org `userspace-api/landlock.html`
    /// §"Inheritance"). If the wrapper ever grows a pre-exec
    /// thread, a raw `syscall(SYS_landlock_restrict_self, fd,
    /// LANDLOCK_RESTRICT_SELF_TSYNC)` shim must replace the crate
    /// call.
    fn apply_landlock(
        policy: &SandboxInitPolicy,
    ) -> Result<Option<open_pincery::runtime::sandbox::landlock_layer::RulesetStatus>, InitError>
    {
        use open_pincery::runtime::sandbox::landlock_layer::{
            install_landlock, LandlockProfile, RulesetStatus,
        };

        if policy.landlock_rx_paths.is_empty() && policy.landlock_rwx_paths.is_empty() {
            eprintln!("pincery-init: landlock skipped (no rx/rwx paths in policy)");
            return Ok(None);
        }

        let profile = LandlockProfile {
            rx_paths: policy.landlock_rx_paths.clone(),
            rwx_paths: policy.landlock_rwx_paths.clone(),
        };

        let mut status = install_landlock(&profile)
            .map_err(|e| InitError::ApplyPolicy(format!("landlock: {e}")))?;

        // Test-only override: force a PartiallyEnforced observation
        // for the FullyEnforced verify negative case. Requires BOTH
        // OPEN_PINCERY_ALLOW_UNSAFE=true AND
        // OPEN_PINCERY_INIT_FORCE_PARTIAL=1 — the double gate is
        // deliberate so the knob cannot trigger on a single env-var
        // typo.
        let unsafe_ok = std::env::var("OPEN_PINCERY_ALLOW_UNSAFE")
            .map(|v| v == "true")
            .unwrap_or(false);
        let force_partial = std::env::var("OPEN_PINCERY_INIT_FORCE_PARTIAL")
            .map(|v| v == "1")
            .unwrap_or(false);
        if unsafe_ok && force_partial {
            eprintln!(
                "pincery-init: OPEN_PINCERY_INIT_FORCE_PARTIAL override active \
                 (landlock status downgraded to PartiallyEnforced for test)"
            );
            status = RulesetStatus::PartiallyEnforced;
        }

        Ok(Some(status))
    }

    /// Verify that every layer the policy requested is actually
    /// FullyEnforced. Step 5 of the T-G0a-6 pipeline. No-op when
    /// `policy.require_fully_enforced` is `false` (v9 default; AC-85
    /// flips this to `true` for production).
    ///
    /// When `require_fully_enforced` is `true`, all three checks run
    /// and ALL must pass:
    ///
    /// 1. Landlock: if `policy.landlock_rx_paths` or
    ///    `landlock_rwx_paths` is non-empty, `landlock_status` must
    ///    be `Some(RulesetStatus::FullyEnforced)`. Anything else
    ///    (including `None`, meaning the apply short-circuited)
    ///    fails.
    /// 2. Seccomp: if `policy.seccomp_bpf` is non-empty,
    ///    `/proc/self/status`'s `Seccomp:` line must read `2`
    ///    (filter mode). This re-reads /proc rather than trusting a
    ///    stashed value from `apply_seccomp` so a silent downgrade
    ///    between steps would still be caught.
    /// 3. NoNewPrivs: `prctl(PR_GET_NO_NEW_PRIVS)` must return `1`
    ///    unconditionally. Handled by the same
    ///    [`verify_no_new_privs`] function used today, called at
    ///    the end of `apply_policy`.
    ///
    /// Failures surface as `InitError::VerifyPolicy` with a message
    /// naming the specific failing layer, so the fd-3 JSON channel
    /// (G0a.3f) can surface a structured `not_fully_enforced` event.
    fn verify_fully_enforced(
        policy: &SandboxInitPolicy,
        landlock_status: Option<open_pincery::runtime::sandbox::landlock_layer::RulesetStatus>,
    ) -> Result<(), InitError> {
        use open_pincery::runtime::sandbox::landlock_layer::RulesetStatus;

        if !policy.require_fully_enforced {
            return Ok(());
        }

        let landlock_requested =
            !policy.landlock_rx_paths.is_empty() || !policy.landlock_rwx_paths.is_empty();
        if landlock_requested {
            match landlock_status {
                Some(RulesetStatus::FullyEnforced) => {}
                Some(other) => {
                    return Err(InitError::VerifyPolicy(format!(
                        "landlock not FullyEnforced: status={other:?} \
                         (require_fully_enforced=true)"
                    )));
                }
                None => {
                    return Err(InitError::VerifyPolicy(
                        "landlock was skipped but require_fully_enforced=true".into(),
                    ));
                }
            }
        }

        if !policy.seccomp_bpf.is_empty() {
            // Re-read /proc/self/status so a downgrade between
            // apply_seccomp and here is still caught. The re-read is
            // cheap and matches the defense-in-depth posture of the
            // uid/gid getresuid verification.
            let status = std::fs::read_to_string("/proc/self/status").map_err(|e| {
                InitError::VerifyPolicy(format!("read /proc/self/status for seccomp verify: {e}"))
            })?;
            let mode = status
                .lines()
                .find_map(|l| l.strip_prefix("Seccomp:"))
                .map(str::trim)
                .ok_or_else(|| {
                    InitError::VerifyPolicy(
                        "no Seccomp: line in /proc/self/status (require_fully_enforced)".into(),
                    )
                })?;
            if mode != "2" {
                return Err(InitError::VerifyPolicy(format!(
                    "seccomp not in filter mode: Seccomp={mode:?} \
                     (require_fully_enforced=true)"
                )));
            }
        }

        Ok(())
    }

    /// Apply every restriction in the order mandated by readiness
    /// T-G0a-6. Slices G0a.3a+b+c+d+e ship steps 1–5; G0a.3f..h
    /// rewire the wrapper into bwrap and flip the default profile.
    ///
    /// Order is load-bearing: seccomp MUST come after NO_NEW_PRIVS
    /// (unprivileged filter load), drop_privs MUST come before
    /// landlock (so the ruleset applies to the unprivileged
    /// identity), landlock MUST come before seccomp (so the filter
    /// does not need to allow `landlock_*` syscalls), and
    /// FullyEnforced verification MUST come last so it observes the
    /// final state of every layer. Callers must not permute this
    /// function's body.
    fn apply_policy(policy: &SandboxInitPolicy) -> Result<(), InitError> {
        apply_no_new_privs()?;
        apply_drop_privs(policy)?;
        let landlock_status = apply_landlock(policy)?;
        apply_seccomp(policy)?;
        verify_no_new_privs()?;
        verify_fully_enforced(policy, landlock_status)?;
        Ok(())
    }

    /// Parsed argv after stripping the wrapper's own flags.
    ///
    /// `user_argv` is captured but not yet consumed in G0a.2 — the
    /// wrapper execs `policy.user_argv` (from the parent-signed IPC
    /// payload) as the single source of truth. G0a.3 will cross-check
    /// `parsed.user_argv == policy.user_argv` as an integrity guard
    /// per readiness T-G0a-3 ("parent writes argv into both the
    /// bwrap argv AND the policy; they MUST match"). Allowing dead
    /// code here keeps the field + parser shape stable so that
    /// integrity check is a one-line addition rather than a parser
    /// rewrite.
    #[derive(Debug)]
    struct ParsedArgs {
        policy_fd: i32,
        #[allow(dead_code)]
        user_argv: Vec<OsString>,
    }

    /// Hand-rolled argv parsing. We avoid pulling `clap` into the
    /// wrapper because:
    ///
    /// - The wrapper has exactly one call pattern (the parent
    ///   controls argv); there is no interactive user to help.
    /// - Slice G0a-followup needs this binary musl-static, and the
    ///   smaller the dep tree the easier that becomes.
    /// - Any parse error here means the parent is broken; we want the
    ///   simplest possible failure path.
    fn parse_args<I>(args: I) -> Result<ParsedArgs, InitError>
    where
        I: IntoIterator<Item = OsString>,
    {
        let mut iter = args.into_iter();
        // Skip argv[0] (our own name).
        let _ = iter.next();

        let mut policy_fd: Option<i32> = None;
        let mut user_argv: Option<Vec<OsString>> = None;

        while let Some(arg) = iter.next() {
            if arg == "--policy-fd" {
                let raw = iter
                    .next()
                    .ok_or_else(|| InitError::Usage("--policy-fd requires a value".into()))?;
                let s = raw.to_string_lossy();
                let n: i32 = s.parse().map_err(|e: std::num::ParseIntError| {
                    InitError::BadPolicyFd(format!("{raw:?}: {e}"))
                })?;
                if n < 0 {
                    return Err(InitError::BadPolicyFd(format!("negative fd {n}")));
                }
                policy_fd = Some(n);
            } else if arg == "--" {
                user_argv = Some(iter.collect());
                break;
            } else {
                return Err(InitError::Usage(format!(
                    "unexpected argument {arg:?}; expected --policy-fd or --"
                )));
            }
        }

        let policy_fd = policy_fd.ok_or_else(|| InitError::Usage("missing --policy-fd".into()))?;
        let user_argv =
            user_argv.ok_or_else(|| InitError::Usage("missing `--` before user argv".into()))?;
        if user_argv.is_empty() {
            return Err(InitError::Usage(
                "user argv after `--` must be non-empty".into(),
            ));
        }

        Ok(ParsedArgs {
            policy_fd,
            user_argv,
        })
    }

    /// Read the entire policy fd into memory. The fd is consumed
    /// (closed on drop) once we return — G0a.3 may change this when
    /// fd 3 is repurposed as the JSON error channel after the policy
    /// is decoded.
    fn read_policy_bytes(fd: i32) -> Result<Vec<u8>, InitError> {
        // Safety: the parent is the sole owner of this fd number
        // inside the child address space (it was placed there by
        // bwrap fd inheritance, or in tests by a `pre_exec` dup2).
        // Wrapping in `File` takes exclusive ownership, which is the
        // correct lifetime model.
        let mut file = unsafe { File::from_raw_fd(fd) };
        let mut buf = Vec::with_capacity(1024);
        file.read_to_end(&mut buf).map_err(InitError::ReadPolicy)?;
        Ok(buf)
    }

    /// Log a one-line, operator-friendly summary of the parsed policy
    /// to stderr. This is the only observable side effect of G0a.2
    /// other than the eventual `execvp`, so integration tests pin on
    /// it to prove the policy was parsed.
    fn log_policy_summary(policy: &SandboxInitPolicy) {
        eprintln!(
            "pincery-init: parsed policy rx_paths={} rwx_paths={} \
             seccomp_bytes={} target_uid={} target_gid={} \
             require_fully_enforced={} user_argv_len={}",
            policy.landlock_rx_paths.len(),
            policy.landlock_rwx_paths.len(),
            policy.seccomp_bpf.len(),
            policy.target_uid,
            policy.target_gid,
            policy.require_fully_enforced,
            policy.user_argv.len(),
        );
    }

    /// `execvp` the user argv. On success this function replaces the
    /// process image and never returns; on failure it surfaces the
    /// errno to the caller.
    fn exec_user_argv(argv: Vec<OsString>) -> InitError {
        // `argv[0]` is the program name passed to execvp (path
        // resolution honors `$PATH` exactly like execvp(3)).
        let mut iter = argv.into_iter();
        let program = iter.next().expect("parse_args rejected empty argv");
        let rest: Vec<OsString> = iter.collect();

        // `Command::exec` calls `execvp` under the hood and preserves
        // all inherited, non-CLOEXEC file descriptors — exactly what
        // we need once G0a.3 wires fd 3 as an error channel.
        let err = Command::new(program).args(rest).exec();
        InitError::Exec(err)
    }

    pub fn run() -> Result<Never, InitError> {
        let parsed = parse_args(std::env::args_os())?;
        let bytes = read_policy_bytes(parsed.policy_fd)?;
        let policy = SandboxInitPolicy::from_bytes(&bytes)
            .map_err(|e| InitError::DecodePolicy(e.to_string()))?;
        log_policy_summary(&policy);

        // Slice G0a.3a: apply step 1 of the T-G0a-6 pipeline
        // (prctl(NO_NEW_PRIVS)) and verify it stuck. Subsequent
        // sub-slices add setres*id → seccomp → landlock → FullyEnforced
        // in this exact spot, before the exec.
        apply_policy(&policy)?;

        // We intentionally use `policy.user_argv` (not the raw argv
        // after `--`) so the single source of truth for what gets
        // executed is the parent-signed policy struct. The parent
        // writes argv into both the bwrap argv AND the policy; they
        // MUST match and G0a.3g will assert this.
        Err(exec_user_argv(
            policy.user_argv.into_iter().map(OsString::from).collect(),
        ))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::ffi::OsString;

        fn args(strs: &[&str]) -> Vec<OsString> {
            strs.iter().map(|s| OsString::from(*s)).collect()
        }

        #[test]
        fn parse_args_accepts_canonical_form() {
            let parsed = parse_args(args(&[
                "pincery-init",
                "--policy-fd",
                "3",
                "--",
                "/bin/sh",
                "-c",
                "echo hi",
            ]))
            .unwrap();
            assert_eq!(parsed.policy_fd, 3);
            assert_eq!(
                parsed.user_argv,
                vec![
                    OsString::from("/bin/sh"),
                    OsString::from("-c"),
                    OsString::from("echo hi"),
                ]
            );
        }

        #[test]
        fn parse_args_rejects_missing_policy_fd() {
            let err = parse_args(args(&["pincery-init", "--", "/bin/true"])).unwrap_err();
            assert!(matches!(err, InitError::Usage(_)), "got {err:?}");
        }

        #[test]
        fn parse_args_rejects_missing_double_dash() {
            let err = parse_args(args(&["pincery-init", "--policy-fd", "3"])).unwrap_err();
            assert!(matches!(err, InitError::Usage(_)), "got {err:?}");
        }

        #[test]
        fn parse_args_rejects_empty_user_argv() {
            let err = parse_args(args(&["pincery-init", "--policy-fd", "3", "--"])).unwrap_err();
            assert!(matches!(err, InitError::Usage(_)), "got {err:?}");
        }

        #[test]
        fn parse_args_rejects_non_numeric_fd() {
            let err = parse_args(args(&[
                "pincery-init",
                "--policy-fd",
                "abc",
                "--",
                "/bin/true",
            ]))
            .unwrap_err();
            assert!(matches!(err, InitError::BadPolicyFd(_)), "got {err:?}");
        }

        #[test]
        fn parse_args_rejects_negative_fd() {
            let err = parse_args(args(&[
                "pincery-init",
                "--policy-fd",
                "-1",
                "--",
                "/bin/true",
            ]))
            .unwrap_err();
            assert!(matches!(err, InitError::BadPolicyFd(_)), "got {err:?}");
        }
    }
}

#[cfg(target_os = "linux")]
use linux::run;
