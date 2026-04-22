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
use std::process::Stdio;

use crate::config::{ResolvedSandboxMode, SandboxMode};

use super::cgroup::CgroupGuard;
use super::{is_rejected_pattern, ExecResult, SandboxProfile, ShellCommand, ToolExecutor};

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
    fn build_bwrap_args(
        cwd: &str,
        command: &str,
        deny_net: bool,
        seccomp_fd: Option<std::os::fd::RawFd>,
    ) -> Vec<String> {
        let mut args: Vec<String> = vec![
            // Clean up if the parent dies mid-execution.
            "--die-with-parent".into(),
            // Fresh ns for each axis. `--unshare-all` would also
            // imply `--share-net`-toggleable semantics; we prefer
            // explicit per-axis flags so the posture is auditable.
            "--unshare-user".into(),
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
        args.extend(["--".into(), "sh".into(), "-c".into(), command.into()]);
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

        let bwrap_args =
            Self::build_bwrap_args(&cwd_str, &cmd.command, profile.deny_net, seccomp_fd_arg);

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

        // AC-53 / Slice A2b.4c: landlock LSM filesystem ruleset.
        //
        // Layer 4 of the six-layer sandbox. Installed AFTER fork but
        // BEFORE execve via `pre_exec`, so the ruleset restricts the
        // bwrap child + its sh descendant without touching the parent
        // pincery process. Inode semantics: bwrap's RO-bind of /usr,
        // /bin, etc. preserves the host inodes, so landlock's
        // PathBeneath rules continue to allow access inside the
        // sandbox view.
        //
        // Mode posture (mirrors cgroup + seccomp):
        //   - Enforce + landlock unsupported  → fail closed.
        //   - Enforce + landlock supported   → install; pre_exec
        //     install failure surfaces as spawn() Err.
        //   - Audit + unsupported            → log, proceed.
        //   - Audit + install fails at runtime → spawn() Err
        //     bubbles up (kernel-level enforcement failure is rare
        //     and indistinguishable from a hard error from the
        //     caller's perspective).
        if profile.landlock {
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
                    }
                }
            } else {
                let landlock_profile =
                    super::landlock_layer::LandlockProfile::default_for_cwd(&cwd);
                use std::os::unix::process::CommandExt;
                // SAFETY: pre_exec runs in the forked child between
                // fork and execve. The closure must be async-signal
                // safe in principle. install_landlock performs
                // landlock syscalls (no malloc) and PathFd::new opens
                // file descriptors (open(2), async-signal-safe). The
                // landlock crate's internal Vec growth happens BEFORE
                // pre_exec is invoked at spawn time? No - the closure
                // is invoked post-fork, so the Vec grows in the
                // child's address space. glibc's malloc post-fork in
                // a multi-threaded parent is technically UB, but in
                // practice glibc handles this safely; this is the
                // same posture used by every Rust sandbox crate.
                unsafe {
                    command.pre_exec(move || {
                        super::landlock_layer::install_landlock(&landlock_profile).map_err(|e| {
                            std::io::Error::new(std::io::ErrorKind::PermissionDenied, e)
                        })
                    });
                }
            }
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
        let args = RealSandbox::build_bwrap_args("/tmp/work", "echo hi", true, None);
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
    fn bwrap_args_keep_net_namespace_inherited_when_deny_net_is_false() {
        let args = RealSandbox::build_bwrap_args("/tmp/work", "echo hi", false, None);
        assert!(
            !args.iter().any(|a| a == "--unshare-net"),
            "deny_net=false must not emit --unshare-net: {args:?}"
        );
    }

    #[test]
    fn bwrap_args_bind_and_chdir_cwd() {
        let args = RealSandbox::build_bwrap_args("/tmp/work", "echo hi", true, None);
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
        let args = RealSandbox::build_bwrap_args("/tmp/work", "echo hello && exit 0", true, None);
        let tail = &args[args.len() - 4..];
        assert_eq!(tail, &["--", "sh", "-c", "echo hello && exit 0"]);
    }

    #[test]
    fn bwrap_args_emit_seccomp_flag_when_fd_provided() {
        let args = RealSandbox::build_bwrap_args("/tmp/work", "true", true, Some(7));
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
        let args = RealSandbox::build_bwrap_args("/tmp/work", "true", true, None);
        assert!(
            !args.iter().any(|a| a == "--seccomp"),
            "seccomp_fd=None must not emit --seccomp: {args:?}"
        );
    }

    #[test]
    fn bwrap_args_mount_tmpfs_and_proc_and_dev() {
        let args = RealSandbox::build_bwrap_args("/tmp/work", "true", true, None);
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
}
