//! AC-36: Sandbox-aware tool executor.
//!
//! Provides a single `ToolExecutor` trait plus a `ProcessExecutor`
//! implementation that is the ONLY place in the codebase allowed to
//! spawn child processes. Everything else (including `tools::dispatch_tool`)
//! goes through a `&Arc<dyn ToolExecutor>`.
//!
//! The default profile enforces three guarantees:
//!   1. **Environment isolation** — `env_clear()` then only allowlisted vars.
//!   2. **Wall-clock timeout** — `tokio::time::timeout` around the child.
//!   3. **Pre-flight reject** — well-known escalation paths (e.g. `sudo`)
//!      fail without ever being spawned.
//!
//! Network isolation on the default profile is advisory: we record
//! `deny_net = true` for audit, but true namespace-level isolation is
//! left to the host (seccomp/bwrap/namespaces). This trait allows a
//! future `NamespacedExecutor` to be swapped in without touching callers.

use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ShellCommand {
    pub command: String,
}

#[derive(Debug, Clone)]
pub struct SandboxProfile {
    /// Names of env vars copied from the parent process into the child.
    pub env_allowlist: Vec<String>,
    /// Advisory flag for network isolation (host-level enforcement TBD).
    pub deny_net: bool,
    /// Wall-clock timeout for the child.
    pub timeout: Duration,
    /// Working directory. `None` means "create a fresh tempdir".
    pub cwd: Option<PathBuf>,
}

impl Default for SandboxProfile {
    fn default() -> Self {
        Self {
            env_allowlist: vec!["PATH".into()],
            deny_net: true,
            timeout: Duration::from_secs(30),
            cwd: None,
        }
    }
}

#[derive(Debug)]
pub enum ExecResult {
    Ok {
        stdout: String,
        stderr: String,
        exit_code: i32,
    },
    Timeout,
    Rejected(String),
    Err(String),
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn run(&self, cmd: &ShellCommand, profile: &SandboxProfile) -> ExecResult;
}

pub struct ProcessExecutor;

#[async_trait]
impl ToolExecutor for ProcessExecutor {
    async fn run(&self, cmd: &ShellCommand, profile: &SandboxProfile) -> ExecResult {
        // Pre-flight: reject well-known escalation without spawning.
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

        // Build the command with cleared env + allowlist.
        let mut command = tokio::process::Command::new("sh");
        command
            .arg("-c")
            .arg(&cmd.command)
            .env_clear()
            .current_dir(&cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .kill_on_drop(true);

        for key in &profile.env_allowlist {
            if let Ok(v) = std::env::var(key) {
                command.env(key, v);
            }
        }

        let child = match command.spawn() {
            Ok(c) => c,
            Err(e) => return ExecResult::Err(format!("spawn failed: {e}")),
        };

        match tokio::time::timeout(profile.timeout, child.wait_with_output()).await {
            Ok(Ok(out)) => ExecResult::Ok {
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                exit_code: out.status.code().unwrap_or(-1),
            },
            Ok(Err(e)) => ExecResult::Err(format!("wait failed: {e}")),
            Err(_elapsed) => {
                // Best-effort kill. The child's resources are released when
                // its handle drops at the end of this arm.
                ExecResult::Timeout
            }
        }
    }
}

fn is_rejected_pattern(command: &str) -> bool {
    let trimmed = command.trim_start();
    trimmed.starts_with("sudo ") || trimmed == "sudo"
}
