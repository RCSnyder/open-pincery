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
    fn build_bwrap_args(cwd: &str, command: &str, deny_net: bool) -> Vec<String> {
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

        let bwrap_args = Self::build_bwrap_args(&cwd_str, &cmd.command, profile.deny_net);

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
        let args = RealSandbox::build_bwrap_args("/tmp/work", "echo hi", true);
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
        let args = RealSandbox::build_bwrap_args("/tmp/work", "echo hi", false);
        assert!(
            !args.iter().any(|a| a == "--unshare-net"),
            "deny_net=false must not emit --unshare-net: {args:?}"
        );
    }

    #[test]
    fn bwrap_args_bind_and_chdir_cwd() {
        let args = RealSandbox::build_bwrap_args("/tmp/work", "echo hi", true);
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
        let args = RealSandbox::build_bwrap_args("/tmp/work", "echo hello && exit 0", true);
        let tail = &args[args.len() - 4..];
        assert_eq!(tail, &["--", "sh", "-c", "echo hello && exit 0"]);
    }

    #[test]
    fn bwrap_args_mount_tmpfs_and_proc_and_dev() {
        let args = RealSandbox::build_bwrap_args("/tmp/work", "true", true);
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
