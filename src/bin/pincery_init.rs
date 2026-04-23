//! AC-83 / Slice G0a.2: `pincery-init` exec wrapper — skeleton.
//!
//! This binary runs **inside** the bwrap sandbox after mount/namespace
//! setup is complete. It reads a serialized [`SandboxInitPolicy`] from
//! an inherited file descriptor, then `execvp`s the user's real argv.
//!
//! ## Slice scope (G0a.2)
//!
//! This slice ships the skeleton only:
//!
//! 1. Parse argv: `pincery-init --policy-fd <N> -- <user_argv...>`.
//! 2. Read the policy from fd N to EOF.
//! 3. Deserialize into [`SandboxInitPolicy`] (log a summary to stderr
//!    so operators and integration tests can observe the parse).
//! 4. `execvp` the user argv. No restrictions are installed yet.
//!
//! ## Out of scope until G0a.3
//!
//! - prctl(NO_NEW_PRIVS), setresuid/setresgid, seccomp install,
//!   landlock_restrict_self, FullyEnforced verification. All of those
//!   land in Slice G0a.3 in the exact order mandated by readiness
//!   T-G0a-6, gated by a matching four-case integration test suite.
//! - Fail-closed JSON error channel on fd 3. G0a.2 uses stderr + exit
//!   125 for any pre-exec failure; G0a.3 reshapes that into the
//!   structured JSON channel the parent can parse.
//! - musl-static linking. The wrapper is dynamically linked for G0a.2
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

    /// Apply every restriction in the order mandated by readiness
    /// T-G0a-6. Slice G0a.3a ships only step 1 (`prctl(NO_NEW_PRIVS)`)
    /// and the matching verify. Subsequent sub-slices fill in:
    ///
    /// - G0a.3b: setresgid → setgroups → setresuid (steps 2).
    /// - G0a.3c: seccomp BPF (step 3).
    /// - G0a.3d: landlock_restrict_self with TSYNC (step 4).
    /// - G0a.3e: FullyEnforced verification (step 5).
    ///
    /// Order is load-bearing: seccomp MUST come after NO_NEW_PRIVS
    /// (unprivileged filter load), and FullyEnforced verification
    /// MUST come after both landlock and seccomp. Callers must not
    /// permute this function's body.
    fn apply_policy(_policy: &SandboxInitPolicy) -> Result<(), InitError> {
        apply_no_new_privs()?;
        // TODO(G0a.3b): setresgid / setgroups / setresuid using
        //   policy.target_uid / policy.target_gid.
        // TODO(G0a.3c): seccomp install when !policy.seccomp_bpf.is_empty().
        // TODO(G0a.3d): landlock_restrict_self when either rx_paths
        //   or rwx_paths is non-empty, using the existing
        //   `runtime::sandbox::landlock_layer::install_landlock` once
        //   it gains a TSYNC flag.
        verify_no_new_privs()?;
        // TODO(G0a.3e): if policy.require_fully_enforced, confirm
        //   the landlock RulesetStatus is FullyEnforced.
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
