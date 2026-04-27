//! AC-53 / Slice A2b.3: Bubblewrap (`bwrap`) namespace wrapper.
//!
//! `RealSandbox` wraps every tool invocation in a `bwrap` child that
//! composes user, pid, mount, uts, ipc, cgroup, and network namespaces
//! with a read-only rootfs overlay. This is layer 1 of the six-layer
//! sandbox (bwrap → cgroup v2 → landlock → seccomp-bpf → uid/cap drop
//! → slirp4netns egress). Layers 2–6 land in Slice A2b.4.
//!
//! Network isolation in this slice is blunt: `--unshare-net` with no
//! loopback interface, which kills everything including DNS. Slice
//! A2b.4 swaps this for `slirp4netns` + an allowlist. Callers that
//! need network (the LLM HTTP client path) currently bypass the
//! executor entirely; AC-67 (proxy gate) will route them through a
//! sandboxed egress broker in a later slice.
//!
//! This file only compiles on Linux. On Windows/macOS the module
//! exists but contains nothing — the factory in `mod.rs` degrades to
//! `ProcessExecutor` via `cfg`-gated arms.

#![cfg(target_os = "linux")]

use async_trait::async_trait;
use std::fs::File;
use std::io::{self, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::config::{ResolvedSandboxMode, SandboxMode};

use super::cgroup::CgroupGuard;
use super::init_policy::SandboxInitPolicy;
use super::landlock_layer::LandlockProfile;
use super::preflight::{KernelProbe, RealKernelProbe, LANDLOCK_ABI_FLOOR};
use super::{is_rejected_pattern, ExecResult, SandboxProfile, ShellCommand, ToolExecutor};

/// AC-83 / Slice G0a.3g: parameters needed to splice `pincery-init`
/// into bwrap's argv. The wrapper binary is ro-bind-mounted at a
/// well-known path inside the sandbox, then bwrap's argv tail is
/// rewritten from `-- sh -c <cmd>` to
/// `-- /sandbox/init --policy-fd N -- sh -c <cmd>`. The policy
/// memfd must be a non-CLOEXEC fd held alive by the caller
/// until after `wait_with_output` so the kernel has the bytes
/// available when the wrapper reads them.
pub(super) struct PinceryInitWiring {
    /// Absolute host path of the `pincery-init` binary. The
    /// parent resolves this via [`pincery_init_bin_path`].
    pub(super) host_path: String,
    /// Raw fd number (non-CLOEXEC) of the policy memfd that the
    /// wrapper will read its serialized [`SandboxInitPolicy`] from.
    pub(super) policy_fd: RawFd,
}

/// Well-known path where the `pincery-init` binary is ro-bind-
/// mounted inside every bwrap sandbox. Kept in one place so the
/// argv rewrite and the `--ro-bind` flag can never disagree.
const SANDBOX_INIT_PATH: &str = "/sandbox/init";
const DEFAULT_SANDBOX_UID: u32 = 65534;
const DEFAULT_SANDBOX_GID: u32 = 65534;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SandboxIdentity {
    uid: u32,
    gid: u32,
}

impl Default for SandboxIdentity {
    fn default() -> Self {
        Self {
            uid: DEFAULT_SANDBOX_UID,
            gid: DEFAULT_SANDBOX_GID,
        }
    }
}

fn parse_optional_id(raw: Option<&str>, env_key: &str) -> Result<Option<u32>, String> {
    let Some(raw) = raw else { return Ok(None) };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parsed = trimmed
        .parse::<u32>()
        .map_err(|e| format!("{env_key}={raw:?} is not a valid u32: {e}"))?;
    Ok(Some(parsed))
}

fn resolve_sandbox_identity_from_raw(
    uid_raw: Option<&str>,
    gid_raw: Option<&str>,
    allow_unsafe: bool,
) -> Result<SandboxIdentity, String> {
    let uid =
        parse_optional_id(uid_raw, "OPEN_PINCERY_SANDBOX_UID")?.unwrap_or(DEFAULT_SANDBOX_UID);
    let gid =
        parse_optional_id(gid_raw, "OPEN_PINCERY_SANDBOX_GID")?.unwrap_or(DEFAULT_SANDBOX_GID);

    if uid == 0 && !allow_unsafe {
        return Err(
            "OPEN_PINCERY_SANDBOX_UID=0 requires OPEN_PINCERY_ALLOW_UNSAFE=true (refusing to run)"
                .into(),
        );
    }
    if gid == 0 && !allow_unsafe {
        return Err(
            "OPEN_PINCERY_SANDBOX_GID=0 requires OPEN_PINCERY_ALLOW_UNSAFE=true (refusing to run)"
                .into(),
        );
    }

    Ok(SandboxIdentity { uid, gid })
}

fn resolve_sandbox_identity(allow_unsafe: bool) -> Result<SandboxIdentity, String> {
    let uid_raw = std::env::var("OPEN_PINCERY_SANDBOX_UID").ok();
    let gid_raw = std::env::var("OPEN_PINCERY_SANDBOX_GID").ok();
    resolve_sandbox_identity_from_raw(uid_raw.as_deref(), gid_raw.as_deref(), allow_unsafe)
}

/// Resolve the on-host path of the `pincery-init` binary.
///
/// Resolution order:
///   1. `PINCERY_INIT_BIN_PATH` environment variable — explicit
///      operator override, also used by integration tests to point
///      at `env!("CARGO_BIN_EXE_pincery-init")`.
///   2. `current_exe().parent()/pincery-init` — sibling of the
///      running `open-pincery` binary. Matches the layout cargo
///      produces for `cargo install` and the devshell's
///      `/usr/local/bin` deploy.
///
/// Returns a `String` error (not `io::Error`) so the caller can
/// thread it into an `ExecResult::Err` without an extra conversion.
pub(super) fn pincery_init_bin_path() -> Result<PathBuf, String> {
    if let Some(override_path) = std::env::var_os("PINCERY_INIT_BIN_PATH") {
        return Ok(PathBuf::from(override_path));
    }
    let current = std::env::current_exe().map_err(|e| format!("current_exe() failed: {e}"))?;
    let parent = current
        .parent()
        .ok_or_else(|| format!("current_exe has no parent: {}", current.display()))?;
    Ok(parent.join("pincery-init"))
}

/// Build the `SandboxInitPolicy` that the in-sandbox wrapper will
/// apply. Seccomp still stays on bwrap's `--seccomp <fd>` path;
/// AC-86 sets the policy target uid/gid to the same identity bwrap
/// applies with `--uid` / `--gid` so the wrapper can re-assert the
/// drop as defense-in-depth.
///
/// `user_argv` is populated with `["sh", "-c", cmd]` to match the
/// bwrap argv tail. `pincery-init::run_inner` execvps from
/// `policy.user_argv`, NOT from the CLI tail, so the two sources
/// MUST carry identical argv (an empty `user_argv` makes
/// `parse_args` reject the CLI form with `user argv after '--'
/// must be non-empty`, which is what G0a.3g's first CI run tripped
/// on).
fn build_init_policy_with_identity(
    cwd: &Path,
    cmd: &str,
    mode: SandboxMode,
    identity: SandboxIdentity,
) -> SandboxInitPolicy {
    let landlock = LandlockProfile::default_for_cwd(cwd);
    SandboxInitPolicy {
        landlock_rx_paths: landlock.rx_paths,
        landlock_rwx_paths: landlock.rwx_paths,
        seccomp_bpf: Vec::new(),
        target_uid: identity.uid,
        target_gid: identity.gid,
        require_fully_enforced: matches!(mode, SandboxMode::Enforce),
        user_argv: vec!["sh".into(), "-c".into(), cmd.into()],
    }
}

#[cfg(test)]
fn build_init_policy(cwd: &Path, cmd: &str, mode: SandboxMode) -> SandboxInitPolicy {
    build_init_policy_with_identity(cwd, cmd, mode, SandboxIdentity::default())
}

fn landlock_abi_below_required_floor() -> Option<String> {
    let probe = RealKernelProbe;
    match probe.landlock_abi() {
        Some(found) if found >= LANDLOCK_ABI_FLOOR => None,
        Some(found) => Some(format!(
            "Landlock ABI {found} is below required ABI {LANDLOCK_ABI_FLOOR}"
        )),
        None => Some("Landlock unsupported: landlock_create_ruleset returned ENOSYS".into()),
    }
}

/// Write the serialized `SandboxInitPolicy` into a fresh non-CLOEXEC
/// memfd, rewound to offset 0 so the wrapper's first `read(2)`
/// sees byte 0. Mirrors
/// [`super::seccomp::write_bpf_to_memfd`] but for JSON bytes.
fn write_policy_to_memfd(bytes: &[u8]) -> io::Result<OwnedFd> {
    let name = c"pincery-init-policy";
    // SAFETY: libc FFI; static C-string ptr; constant flags (0 so
    // the fd inherits across execve without MFD_CLOEXEC).
    let raw = unsafe { libc::memfd_create(name.as_ptr(), 0) };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `raw` is a fresh kernel-allocated fd with no existing
    // owner; `OwnedFd` takes exclusive ownership.
    let owned = unsafe { OwnedFd::from_raw_fd(raw) };
    // Go through `File` for a batteries-included `write_all` +
    // `seek` API, then hand ownership back as `OwnedFd`.
    let mut file: File = owned.into();
    file.write_all(bytes)?;
    file.seek(SeekFrom::Start(0))?;
    Ok(OwnedFd::from(file))
}

/// Linux-only bubblewrap-backed executor.
///
/// Holds the resolved sandbox mode so Slice A2b.4 can plumb
/// `enforce` vs `audit` down into the seccomp filter action
/// (KILL_PROCESS vs LOG) without touching the trait signature.
#[derive(Debug, Clone)]
pub struct RealSandbox {
    /// Preserved for A2b.4 — seccomp/landlock layers will read
    /// `mode` to select KILL_PROCESS vs LOG. For A2b.3 the mode is
    /// only relevant at factory-selection time.
    #[allow(dead_code)]
    pub(super) sandbox: ResolvedSandboxMode,
}

impl RealSandbox {
    pub fn new(sandbox: ResolvedSandboxMode) -> Self {
        Self { sandbox }
    }

    /// Build the `bwrap` argv for a given cwd + command. Extracted
    /// for unit-testability — the flag list is large and order-
    /// sensitive, so a pure function keeps the per-flag rationale
    /// close to the test that pins it.
    ///
    /// `seccomp_fd` is the raw fd number of a memfd containing a
    /// compiled `sock_filter[]` BPF program, if the seccomp layer is
    /// active for this invocation (Slice A2b.4b). When `Some`, we
    /// insert `--seccomp <fd>` into bwrap's argv; bwrap reads the
    /// program from that fd and installs it via `seccomp(2)` right
    /// before `execve`-ing the user shell.
    ///
    /// `init_wiring` (AC-83 / Slice G0a.3g) threads `pincery-init`
    /// into the argv: the binary is ro-bind-mounted at
    /// `/sandbox/init`, and the argv tail is rewritten from
    /// `-- sh -c <cmd>` to
    /// `-- /sandbox/init --policy-fd N -- sh -c <cmd>`. When `None`
    /// the argv tail is the pre-G0a shape (direct `sh -c <cmd>`).
    #[cfg(test)]
    fn build_bwrap_args(
        cwd: &str,
        command: &str,
        deny_net: bool,
        seccomp_fd: Option<std::os::fd::RawFd>,
        init_wiring: Option<&PinceryInitWiring>,
    ) -> Vec<String> {
        Self::build_bwrap_args_with_identity(
            cwd,
            command,
            deny_net,
            seccomp_fd,
            init_wiring,
            SandboxIdentity::default(),
        )
    }

    fn build_bwrap_args_with_identity(
        cwd: &str,
        command: &str,
        deny_net: bool,
        seccomp_fd: Option<std::os::fd::RawFd>,
        init_wiring: Option<&PinceryInitWiring>,
        identity: SandboxIdentity,
    ) -> Vec<String> {
        let mut args: Vec<String> = vec![
            // Clean up if the parent dies mid-execution.
            "--die-with-parent".into(),
            // Fresh ns for each axis. `--unshare-all` would also
            // imply `--share-net`-toggleable semantics; we prefer
            // explicit per-axis flags so the posture is auditable.
            "--unshare-user".into(),
            "--uid".into(),
            identity.uid.to_string(),
            "--gid".into(),
            identity.gid.to_string(),
            "--cap-drop".into(),
            "ALL".into(),
            "--unshare-pid".into(),
            "--unshare-ipc".into(),
            "--unshare-uts".into(),
            // cgroup ns exists on newer kernels (5.7+). Use -try so
            // older hosts still work — A2b.4's cgroup v2 layer
            // asserts availability separately.
            "--unshare-cgroup-try".into(),
            // Detach from controlling tty so Ctrl-C on the parent
            // doesn't propagate into the sandbox.
            "--new-session".into(),
            "--hostname".into(),
            "sandbox".into(),
            // Read-only rootfs overlay. `-try` variants skip paths
            // that don't exist on minimalist images.
            "--ro-bind".into(),
            "/usr".into(),
            "/usr".into(),
            "--ro-bind-try".into(),
            "/bin".into(),
            "/bin".into(),
            "--ro-bind-try".into(),
            "/sbin".into(),
            "/sbin".into(),
            "--ro-bind-try".into(),
            "/lib".into(),
            "/lib".into(),
            "--ro-bind-try".into(),
            "/lib64".into(),
            "/lib64".into(),
            "--ro-bind-try".into(),
            "/etc".into(),
            "/etc".into(),
            // Standard dynamic mounts.
            "--proc".into(),
            "/proc".into(),
            "--dev".into(),
            "/dev".into(),
            "--tmpfs".into(),
            "/tmp".into(),
            // Writable workspace directory — the tempdir the
            // executor minted, or the explicit cwd the caller
            // pinned via SandboxProfile.
            "--bind".into(),
            cwd.into(),
            cwd.into(),
            "--chdir".into(),
            cwd.into(),
        ];
        if deny_net {
            // Blunt network kill — no loopback, no DNS, no egress.
            // Slice A2b.4 swaps this for slirp4netns + allowlist.
            args.push("--unshare-net".into());
        }
        if let Some(fd) = seccomp_fd {
            // bwrap reads a raw `sock_filter[]` program from this fd
            // and installs it via seccomp(SECCOMP_SET_MODE_FILTER, ...)
            // right before execve. Must come before `--` so bwrap
            // parses it as its own flag. The fd must be inheritable
            // (not CLOEXEC) and alive when bwrap execs — we hold the
            // OwnedFd in `run()` through `wait_with_output`.
            args.push("--seccomp".into());
            args.push(fd.to_string());
        }
        if let Some(wiring) = init_wiring {
            // Mount the pincery-init binary at a fixed well-known
            // path inside the sandbox. `--ro-bind` auto-creates
            // missing parent dirs in bwrap's root tmpfs.
            args.push("--ro-bind".into());
            args.push(wiring.host_path.clone());
            args.push(SANDBOX_INIT_PATH.into());
            // Argv tail: exec the wrapper, hand it the policy fd,
            // then pass the user shell command as the wrapper's
            // `user_argv` (everything after the inner `--`).
            args.extend([
                "--".into(),
                SANDBOX_INIT_PATH.into(),
                "--policy-fd".into(),
                wiring.policy_fd.to_string(),
                "--".into(),
                "sh".into(),
                "-c".into(),
                command.into(),
            ]);
        } else {
            args.extend(["--".into(), "sh".into(), "-c".into(), command.into()]);
        }
        args
    }

    /// AC-53 / Slice A2b.4a helper: create a `pincery-<uuid>` cgroup,
    /// apply `limits`, and attach the spawned bwrap child's PID. Split
    /// out from `run` to keep the error branches readable and to make
    /// both steps (create, attach) share a single error type.
    ///
    /// Returns the live [`CgroupGuard`] on success — caller must keep
    /// it alive until after the child is reaped so `Drop`-time `rmdir`
    /// fires on an empty cgroup.
    fn attach_cgroup_to_child(
        &self,
        limits: &super::CgroupLimits,
        child: &tokio::process::Child,
    ) -> Result<CgroupGuard, String> {
        let pid = child
            .id()
            .ok_or_else(|| "bwrap child has no pid (already exited?)".to_string())?;
        let guard = CgroupGuard::new(limits)
            .map_err(|e| format!("cgroup create failed: {e} (is /sys/fs/cgroup writable?)"))?;
        guard
            .attach_pid(pid)
            .map_err(|e| format!("cgroup attach pid {pid} failed: {e}"))?;
        Ok(guard)
    }
}

#[async_trait]
impl ToolExecutor for RealSandbox {
    async fn run(&self, cmd: &ShellCommand, profile: &SandboxProfile) -> ExecResult {
        // Pre-flight sudo reject — same posture as ProcessExecutor.
        // Bubblewrap drops all capabilities anyway, but we surface a
        // clear Rejected reason for the audit log rather than letting
        // it blow up inside the sandbox.
        if is_rejected_pattern(&cmd.command) {
            return ExecResult::Rejected("sudo is not permitted".into());
        }

        // Fresh tempdir per call if caller did not pin one.
        let _tmp_guard;
        let cwd = match &profile.cwd {
            Some(p) => p.clone(),
            None => match tempfile::tempdir() {
                Ok(t) => {
                    let p = t.path().to_path_buf();
                    _tmp_guard = t;
                    p
                }
                Err(e) => return ExecResult::Err(format!("tempdir failed: {e}")),
            },
        };
        let cwd_str = match cwd.to_str() {
            Some(s) => s.to_string(),
            None => {
                return ExecResult::Err(format!(
                    "sandbox cwd is not valid UTF-8: {}",
                    cwd.display()
                ));
            }
        };

        let sandbox_identity = match resolve_sandbox_identity(self.sandbox.allow_unsafe) {
            Ok(identity) => identity,
            Err(reason) => return ExecResult::Err(reason),
        };

        // AC-53 / Slice A2b.4b: seccomp-bpf layer.
        //
        // Built BEFORE spawn because the fd number must land in the
        // bwrap argv. The OwnedFd is bound to `_seccomp_fd_guard` and
        // kept alive until after `wait_with_output` — bwrap reads the
        // program from the memfd during its setup phase, but we don't
        // have a signal for "bwrap is done reading", so the safe
        // upper bound is "child has exited".
        //
        // Fail-closed in Enforce; log-and-proceed in Audit. Mirrors
        // the cgroup layer's posture.
        let (_seccomp_fd_guard, seccomp_fd_arg): (
            Option<std::os::fd::OwnedFd>,
            Option<std::os::fd::RawFd>,
        ) = if profile.seccomp {
            match super::seccomp::compose_seccomp_fd(self.sandbox.mode) {
                Ok((fd, raw)) => (Some(fd), Some(raw)),
                Err(reason) => match self.sandbox.mode {
                    SandboxMode::Enforce => {
                        return ExecResult::Err(format!(
                            "seccomp layer failed (enforce mode, failing closed): {reason}"
                        ));
                    }
                    SandboxMode::Audit | SandboxMode::Disabled => {
                        tracing::warn!(
                            target = "sandbox.seccomp",
                            reason = %reason,
                            mode = ?self.sandbox.mode,
                            "seccomp layer failed; proceeding without it (audit mode)"
                        );
                        (None, None)
                    }
                },
            }
        } else {
            (None, None)
        };

        // AC-83 / Slice G0a.3g: landlock now installs INSIDE the
        // sandbox via `pincery-init`, post-namespace setup. The old
        // parent `pre_exec` install path is architecturally broken
        // on MS_SLAVE-locked systemd hosts (see
        // docs/security/sandbox-architecture-audit.md); the wrapper
        // path is the replacement.
        //
        // Mode posture mirrors seccomp + cgroup:
        //   - Enforce + landlock unsupported  → fail closed.
        //   - Enforce + landlock supported    → wire pincery-init.
        //   - Audit + unsupported             → log, proceed without
        //                                       the wrapper (direct
        //                                       `sh -c` argv tail).
        //
        // `_init_policy_fd` owns the non-CLOEXEC memfd and must
        // outlive `wait_with_output` — the wrapper reads its policy
        // from it during startup.
        let (_init_policy_fd, init_wiring): (Option<OwnedFd>, Option<PinceryInitWiring>) =
            if profile.landlock {
                if let Some(reason) = landlock_abi_below_required_floor() {
                    match self.sandbox.mode {
                        SandboxMode::Enforce => {
                            return ExecResult::Err(format!(
                                "landlock ABI floor unmet (enforce mode, failing closed): {reason}"
                            ));
                        }
                        SandboxMode::Audit | SandboxMode::Disabled => {
                            tracing::warn!(
                                target = "sandbox.landlock",
                                event = "sandbox_partial_enforcement",
                                reason = %reason,
                                mode = ?self.sandbox.mode,
                                "landlock ABI below strict floor; proceeding in audit/disabled mode"
                            );
                        }
                    }
                }
                if !super::landlock_layer::landlock_supported() {
                    match self.sandbox.mode {
                        SandboxMode::Enforce => {
                            return ExecResult::Err(
                                "landlock layer unsupported (kernel < 5.13, enforce mode, \
                             failing closed)"
                                    .into(),
                            );
                        }
                        SandboxMode::Audit | SandboxMode::Disabled => {
                            tracing::warn!(
                                target = "sandbox.landlock",
                                mode = ?self.sandbox.mode,
                                "landlock unsupported (kernel < 5.13); proceeding without it"
                            );
                            (None, None)
                        }
                    }
                } else {
                    let host_path = match pincery_init_bin_path() {
                        Ok(p) => p,
                        Err(reason) => {
                            return ExecResult::Err(format!(
                                "pincery-init bin path resolution failed: {reason}"
                            ));
                        }
                    };
                    if !host_path.exists() {
                        return ExecResult::Err(format!(
                            "pincery-init binary not found at {} (set PINCERY_INIT_BIN_PATH to \
                         override)",
                            host_path.display()
                        ));
                    }
                    let host_path_str = match host_path.to_str() {
                        Some(s) => s.to_string(),
                        None => {
                            return ExecResult::Err(format!(
                                "pincery-init bin path is not valid UTF-8: {}",
                                host_path.display()
                            ));
                        }
                    };
                    let policy = build_init_policy_with_identity(
                        &cwd,
                        &cmd.command,
                        self.sandbox.mode,
                        sandbox_identity,
                    );
                    let policy_bytes = match policy.to_bytes() {
                        Ok(b) => b,
                        Err(e) => {
                            return ExecResult::Err(format!(
                                "serialize SandboxInitPolicy failed: {e}"
                            ));
                        }
                    };
                    let fd = match write_policy_to_memfd(&policy_bytes) {
                        Ok(fd) => fd,
                        Err(e) => {
                            return ExecResult::Err(format!(
                                "policy memfd_create/write failed: {e}"
                            ));
                        }
                    };
                    let raw = fd.as_raw_fd();
                    (
                        Some(fd),
                        Some(PinceryInitWiring {
                            host_path: host_path_str,
                            policy_fd: raw,
                        }),
                    )
                }
            } else {
                (None, None)
            };

        let bwrap_args = Self::build_bwrap_args_with_identity(
            &cwd_str,
            &cmd.command,
            profile.deny_net,
            seccomp_fd_arg,
            init_wiring.as_ref(),
            sandbox_identity,
        );

        let mut command = tokio::process::Command::new("bwrap");
        command
            .args(&bwrap_args)
            .env_clear()
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .kill_on_drop(true);

        // Env allowlist — copied from the parent just like
        // `ProcessExecutor`. bwrap inherits this env.
        for key in &profile.env_allowlist {
            if let Ok(v) = std::env::var(key) {
                command.env(key, v);
            }
        }
        // AC-43: caller-supplied env (typically resolved credential
        // plaintexts) wins over the allowlist when names collide.
        for (k, v) in &cmd.env {
            command.env(k, v);
        }

        let child = match command.spawn() {
            Ok(c) => c,
            Err(e) => {
                // Most common case: `bwrap` is not on PATH. Surface
                // a specific error so operators know to install the
                // devshell image rather than chasing a generic spawn
                // failure.
                return ExecResult::Err(format!("bwrap spawn failed: {e}"));
            }
        };

        // AC-53 / Slice A2b.4a: cgroup v2 resource caps.
        //
        // Layer 2 of the six-layer sandbox. The guard lives through
        // the entire wait so `Drop` cleanup fires AFTER the child is
        // reaped (cgroup v2 refuses rmdir until cgroup.procs is empty).
        //
        // Fail-closed semantics: in Enforce mode any cgroup init or
        // attach error terminates the already-spawned bwrap child and
        // surfaces as `ExecResult::Err`. In Audit mode we log and
        // continue without a cgroup, matching the seccomp LOG posture.
        let _cgroup_guard: Option<CgroupGuard> = match &profile.cgroup {
            None => None,
            Some(limits) => match self.attach_cgroup_to_child(limits, &child) {
                Ok(guard) => Some(guard),
                Err(reason) => match self.sandbox.mode {
                    SandboxMode::Enforce => {
                        // `kill_on_drop(true)` on the Command + the
                        // end-of-scope drop of `child` terminates the
                        // bwrap process before we return. No explicit
                        // kill() needed.
                        return ExecResult::Err(format!(
                            "cgroup layer failed (enforce mode, failing closed): {reason}"
                        ));
                    }
                    SandboxMode::Audit | SandboxMode::Disabled => {
                        tracing::warn!(
                            target = "sandbox.cgroup",
                            reason = %reason,
                            mode = ?self.sandbox.mode,
                            "cgroup layer failed; proceeding without it (audit mode)"
                        );
                        None
                    }
                },
            },
        };

        match tokio::time::timeout(profile.timeout, child.wait_with_output()).await {
            Ok(Ok(out)) => ExecResult::Ok {
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                exit_code: out.status.code().unwrap_or(-1),
            },
            Ok(Err(e)) => ExecResult::Err(format!("wait failed: {e}")),
            Err(_elapsed) => ExecResult::Timeout,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bwrap_args_include_each_required_namespace_flag() {
        let args = RealSandbox::build_bwrap_args("/tmp/work", "echo hi", true, None, None);
        // The order matters less than the presence — bwrap processes
        // flags in sequence, but each namespace flag is independent.
        for flag in [
            "--die-with-parent",
            "--unshare-user",
            "--unshare-pid",
            "--unshare-ipc",
            "--unshare-uts",
            "--unshare-cgroup-try",
            "--unshare-net",
            "--new-session",
        ] {
            assert!(
                args.iter().any(|a| a == flag),
                "missing required bwrap flag {flag}: {args:?}"
            );
        }
    }

    #[test]
    fn bwrap_args_drop_to_nobody_and_clear_capabilities() {
        let args = RealSandbox::build_bwrap_args("/tmp/work", "echo hi", true, None, None);

        assert!(
            args.windows(2).any(|w| w == ["--uid", "65534"]),
            "AC-86 requires bwrap to run every sandbox as uid 65534: {args:?}"
        );
        assert!(
            args.windows(2).any(|w| w == ["--gid", "65534"]),
            "AC-86 requires bwrap to run every sandbox as gid 65534: {args:?}"
        );
        assert!(
            args.windows(2).any(|w| w == ["--cap-drop", "ALL"]),
            "AC-86 requires bwrap to drop every capability: {args:?}"
        );

        let uid_idx = args.iter().position(|a| a == "--uid").expect("--uid");
        let gid_idx = args.iter().position(|a| a == "--gid").expect("--gid");
        let cap_idx = args
            .iter()
            .position(|a| a == "--cap-drop")
            .expect("--cap-drop");
        let sep = args.iter().position(|a| a == "--").expect("-- separator");
        assert!(
            uid_idx < sep && gid_idx < sep && cap_idx < sep,
            "uid/gid/cap-drop must be parsed by bwrap, not forwarded to sh: {args:?}"
        );
    }

    #[test]
    fn bwrap_args_keep_net_namespace_inherited_when_deny_net_is_false() {
        let args = RealSandbox::build_bwrap_args("/tmp/work", "echo hi", false, None, None);
        assert!(
            !args.iter().any(|a| a == "--unshare-net"),
            "deny_net=false must not emit --unshare-net: {args:?}"
        );
    }

    #[test]
    fn bwrap_args_bind_and_chdir_cwd() {
        let args = RealSandbox::build_bwrap_args("/tmp/work", "echo hi", true, None, None);
        let bind_idx = args
            .iter()
            .position(|a| a == "--bind")
            .expect("--bind flag must be present");
        assert_eq!(args[bind_idx + 1], "/tmp/work");
        assert_eq!(args[bind_idx + 2], "/tmp/work");
        let chdir_idx = args
            .iter()
            .position(|a| a == "--chdir")
            .expect("--chdir flag must be present");
        assert_eq!(args[chdir_idx + 1], "/tmp/work");
    }

    #[test]
    fn bwrap_args_terminate_with_shell_invocation() {
        let args =
            RealSandbox::build_bwrap_args("/tmp/work", "echo hello && exit 0", true, None, None);
        let tail = &args[args.len() - 4..];
        assert_eq!(tail, &["--", "sh", "-c", "echo hello && exit 0"]);
    }

    #[test]
    fn bwrap_args_emit_seccomp_flag_when_fd_provided() {
        let args = RealSandbox::build_bwrap_args("/tmp/work", "true", true, Some(7), None);
        let idx = args
            .iter()
            .position(|a| a == "--seccomp")
            .expect("--seccomp must be present when fd provided");
        assert_eq!(args[idx + 1], "7");
        // Must come BEFORE the `--` separator so bwrap parses it as
        // its own flag rather than forwarding it to sh.
        let sep = args.iter().position(|a| a == "--").expect("-- separator");
        assert!(
            idx < sep,
            "--seccomp must precede `--`: idx={idx} sep={sep} args={args:?}"
        );
    }

    #[test]
    fn bwrap_args_omit_seccomp_flag_when_fd_absent() {
        let args = RealSandbox::build_bwrap_args("/tmp/work", "true", true, None, None);
        assert!(
            !args.iter().any(|a| a == "--seccomp"),
            "seccomp_fd=None must not emit --seccomp: {args:?}"
        );
    }

    #[test]
    fn bwrap_args_mount_tmpfs_and_proc_and_dev() {
        let args = RealSandbox::build_bwrap_args("/tmp/work", "true", true, None, None);
        assert!(
            args.windows(2).any(|w| w == ["--tmpfs", "/tmp"]),
            "tmpfs on /tmp missing"
        );
        assert!(
            args.windows(2).any(|w| w == ["--proc", "/proc"]),
            "proc on /proc missing"
        );
        assert!(
            args.windows(2).any(|w| w == ["--dev", "/dev"]),
            "dev on /dev missing"
        );
    }

    #[test]
    fn bwrap_args_rewrite_tail_through_pincery_init_when_wired() {
        let wiring = PinceryInitWiring {
            host_path: "/host/bin/pincery-init".into(),
            policy_fd: 9,
        };
        let args =
            RealSandbox::build_bwrap_args("/tmp/work", "echo hello", true, None, Some(&wiring));
        // --ro-bind of the wrapper binary must be present.
        let rb = args
            .windows(3)
            .position(|w| w == ["--ro-bind", "/host/bin/pincery-init", SANDBOX_INIT_PATH])
            .expect("--ro-bind for pincery-init missing");
        // Argv tail must exec the wrapper with --policy-fd, then
        // pass sh -c <cmd> after the inner `--`.
        let tail = &args[args.len() - 8..];
        assert_eq!(
            tail,
            &[
                "--",
                SANDBOX_INIT_PATH,
                "--policy-fd",
                "9",
                "--",
                "sh",
                "-c",
                "echo hello",
            ]
        );
        // The wrapper ro-bind must come before bwrap's outer `--`.
        let sep = args
            .iter()
            .rposition(|a| a == "--")
            .expect("outer -- separator");
        assert!(
            rb < sep,
            "pincery-init ro-bind must precede outer `--`: rb={rb} sep={sep}"
        );
    }

    #[test]
    fn bwrap_args_skip_pincery_init_when_wiring_absent() {
        let args = RealSandbox::build_bwrap_args("/tmp/work", "true", true, None, None);
        assert!(
            !args.iter().any(|a| a == SANDBOX_INIT_PATH),
            "wiring=None must not mention {SANDBOX_INIT_PATH}: {args:?}"
        );
        assert!(
            !args.iter().any(|a| a == "--policy-fd"),
            "wiring=None must not emit --policy-fd: {args:?}"
        );
    }

    #[test]
    fn pincery_init_bin_path_respects_env_override() {
        // Env override is the operator/test contract; the fallback
        // path (current_exe sibling) is harder to assert portably
        // and already exercised implicitly by the landlock
        // integration suite.
        std::env::set_var("PINCERY_INIT_BIN_PATH", "/opt/custom/pincery-init");
        let resolved = pincery_init_bin_path().expect("env override must resolve");
        assert_eq!(resolved, PathBuf::from("/opt/custom/pincery-init"));
        std::env::remove_var("PINCERY_INIT_BIN_PATH");
    }

    #[test]
    fn init_policy_requires_fully_enforced_in_enforce_mode() {
        let policy = build_init_policy(Path::new("/tmp/work"), "true", SandboxMode::Enforce);
        assert!(
            policy.require_fully_enforced,
            "enforce mode must make pincery-init reject partial Landlock enforcement"
        );
    }

    #[test]
    fn init_policy_targets_nobody_for_ac86_defense_in_depth() {
        let policy = build_init_policy(Path::new("/tmp/work"), "true", SandboxMode::Enforce);
        assert_eq!(policy.target_uid, 65534, "AC-86 target uid must be nobody");
        assert_eq!(
            policy.target_gid, 65534,
            "AC-86 target gid must be nogroup/nobody"
        );
    }

    #[test]
    fn sandbox_identity_accepts_nonzero_env_overrides() {
        let identity = resolve_sandbox_identity_from_raw(Some("1001"), Some("1002"), false)
            .expect("nonzero override should be accepted");
        assert_eq!(
            identity,
            SandboxIdentity {
                uid: 1001,
                gid: 1002
            }
        );
    }

    #[test]
    fn sandbox_identity_rejects_uid_zero_without_allow_unsafe() {
        let err = resolve_sandbox_identity_from_raw(Some("0"), Some("0"), false)
            .expect_err("uid 0 must be rejected without allow_unsafe");
        assert!(err.contains("OPEN_PINCERY_SANDBOX_UID=0"), "{err}");
    }

    #[test]
    fn sandbox_identity_allows_uid_zero_with_allow_unsafe() {
        let identity = resolve_sandbox_identity_from_raw(Some("0"), Some("0"), true)
            .expect("allow_unsafe should permit explicit root override");
        assert_eq!(identity, SandboxIdentity { uid: 0, gid: 0 });
    }

    #[test]
    fn init_policy_allows_partial_enforcement_in_audit_mode() {
        let policy = build_init_policy(Path::new("/tmp/work"), "true", SandboxMode::Audit);
        assert!(
            !policy.require_fully_enforced,
            "audit mode must let pincery-init proceed and report sandbox_partial_enforcement"
        );
    }
}
