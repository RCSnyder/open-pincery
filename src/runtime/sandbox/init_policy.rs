//! AC-83 / Slice G0a.1: `SandboxInitPolicy` — IPC shape between
//! `pincery-server` (parent) and `pincery-init` (in-sandbox wrapper).
//!
//! ## Why this module exists
//!
//! The 2026-04-22 distinguished-engineer audit
//! ([docs/security/sandbox-architecture-audit.md]) proved that
//! installing Landlock on the **parent** via `Command::pre_exec` is
//! architecturally broken: Landlock V1+ unconditionally denies
//! `mount(2)` (kernel.org `userspace-api/landlock.html` §"Current
//! limitations") and Landlock domains are inherited via `clone(2)`
//! (§"Inheritance"), so bwrap EPERMs on its first
//! `mount(NULL, "/", MS_SLAVE | MS_REC, NULL)` call.
//!
//! The fix (AC-83) is to install every kernel restriction — Landlock,
//! seccomp-bpf, prctl(NO_NEW_PRIVS), setresuid/setresgid, capset —
//! **inside** the sandbox, after bwrap finishes namespace + mount
//! setup, via a musl-static exec wrapper. The wrapper is
//! `--ro-bind`ed into every sandbox and replaces the user command's
//! argv[0]; it reads its policy from an inherited memfd, applies the
//! restrictions in order, then `execvp`s the user's real argv.
//!
//! Matches the architectural pattern used by Flatpak, Firejail, and
//! the official [`rust-landlock` `sandboxer.rs` example].
//!
//! ## What this module ships
//!
//! A single struct `SandboxInitPolicy` that is serialized by
//! the parent and deserialized by the wrapper. This module is
//! the **only** cross-binary type boundary; everything else stays
//! private to each binary. Because both binaries ship from the same
//! git SHA, we do not need a stable schema or versioning.
//!
//! ### Slice G0a.1 (this slice)
//!
//! - Struct definition with serde derives.
//! - JSON round-trip unit test proving
//!   `deserialize(serialize(x)) == x`.
//! - No wrapper binary, no bwrap wiring. Those are G0a.2 and G0a.3.
//!
//! ### Out of scope for G0a (tracked in later slices)
//!
//! - AC-84 / Slice G0b: kernel ABI floor preflight field.
//! - AC-85 / Slice G0c: `require_fully_enforced` becomes non-optional.
//! - AC-87 / Slice G0e: `ipc_scopes` bitmap (Landlock ABI >= 6).
//! - AC-88 / Slice G0f: `kernel_audit_log` toggle for
//!   `LANDLOCK_RESTRICT_SELF_LOG_NEW_EXEC_ON`.

//! ## Wire format
//!
//! Serialized as JSON via `serde_json`. Bincode would be more
//! compact, but the v1 line is flagged unmaintained by
//! RUSTSEC-2025-0141 and the v2 rewrite is a breaking API change
//! that would add a new direct dependency solely for this one IPC
//! boundary. `serde_json` is already a transitive dep (axum/utoipa)
//! and the payload is bounded at well under 1 KB in practice, so
//! the text-format cost is irrelevant. An operator inspecting a
//! failing sandbox's core dump also gets a human-readable policy
//! for free. The `seccomp_bpf` byte blob is encoded as a JSON array
//! of small integers — verbose but correct and lossless.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Policy bytes crossed from parent -> pincery-init via a memfd.
///
/// This struct is **exclusively** for cross-binary IPC. No other
/// code should construct one outside `RealSandbox::build_init_policy`
/// (parent) and `pincery_init::read_policy` (wrapper).
///
/// ### Field notes
///
/// - `landlock_rx_paths` / `landlock_rwx_paths`: the same two vectors
///   from [`crate::runtime::sandbox::landlock_layer::LandlockProfile`],
///   copied rather than referenced so the IPC type owns its bytes.
///   Paths are evaluated inside the sandbox namespace and must be
///   visible there (bwrap's `--ro-bind` / `--bind` arrangements
///   mirror these).
/// - `seccomp_bpf`: a raw `sock_filter[]` byte blob already produced
///   by `seccompiler`. The wrapper passes it straight to
///   `prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, ...)` after
///   `prctl(PR_SET_NO_NEW_PRIVS)`. Empty = no seccomp install
///   requested.
/// - `target_uid` / `target_gid`: applied via
///   `setresgid -> setgroups(0, NULL) -> setresuid` as defense-in-
///   depth on top of bwrap's `--uid`/`--gid` flags (AC-86).
/// - `require_fully_enforced`: when `true`, the wrapper requests
///   `CompatLevel::HardRequirement` and must observe
///   `RestrictionStatus { ruleset: FullyEnforced, no_new_privs: true }`
///   or `_exit(125)` with a `not_fully_enforced` error JSON. Enforce
///   mode sets this to `true`; Audit mode sets it to `false` so the
///   wrapper can emit `sandbox_partial_enforcement` and proceed.
/// - `user_argv`: the argv the wrapper `execvp`s after all
///   restrictions are in place. First element is the program name
///   (same semantics as `execvp(3)`). Must be non-empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxInitPolicy {
    pub landlock_rx_paths: Vec<PathBuf>,
    pub landlock_rwx_paths: Vec<PathBuf>,
    pub seccomp_bpf: Vec<u8>,
    pub target_uid: u32,
    pub target_gid: u32,
    pub require_fully_enforced: bool,
    pub user_argv: Vec<String>,
}

/// Errors surfaced during IPC serialization or deserialization.
/// Kept deliberately small — the wrapper turns any error here into
/// a JSON line on fd 3 and `_exit(125)`; there is no recovery path.
#[derive(Debug)]
pub enum InitPolicyError {
    /// Serialization or deserialization failed. Message is the
    /// underlying serde error, preserved for operator debugging (it
    /// ends up in the kernel audit log via `landlock_denied` once
    /// AC-88 ships, or on stderr of the failing bwrap invocation in
    /// the meantime).
    Codec(String),
}

impl std::fmt::Display for InitPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InitPolicyError::Codec(msg) => write!(f, "init policy codec error: {msg}"),
        }
    }
}

impl std::error::Error for InitPolicyError {}

impl SandboxInitPolicy {
    /// Serialize to the exact bytes that will be written to the IPC
    /// memfd. Uses `serde_json` (see module docs for rationale). The
    /// output is a single JSON object terminated by an EOF — no
    /// trailing newline, no framing. `from_bytes` is its exact
    /// inverse.
    pub fn to_bytes(&self) -> Result<Vec<u8>, InitPolicyError> {
        serde_json::to_vec(self).map_err(|e| InitPolicyError::Codec(e.to_string()))
    }

    /// Deserialize bytes read from the IPC memfd. The wrapper calls
    /// this once at startup. Any failure here aborts policy
    /// application — the wrapper writes an error JSON to fd 3 and
    /// `_exit(125)`.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, InitPolicyError> {
        serde_json::from_slice(bytes).map_err(|e| InitPolicyError::Codec(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_policy() -> SandboxInitPolicy {
        SandboxInitPolicy {
            landlock_rx_paths: vec![
                PathBuf::from("/usr"),
                PathBuf::from("/bin"),
                PathBuf::from("/lib"),
                PathBuf::from("/lib64"),
                PathBuf::from("/etc"),
            ],
            landlock_rwx_paths: vec![
                PathBuf::from("/proc"),
                PathBuf::from("/tmp/workspace-abc123"),
            ],
            // A nonempty but syntactically arbitrary BPF blob stands
            // in for a real seccompiler output — round-trip does not
            // interpret it.
            seccomp_bpf: vec![0x00, 0x00, 0x20, 0x00, 0x06, 0x00, 0x00, 0x00],
            target_uid: 65534,
            target_gid: 65534,
            require_fully_enforced: false,
            user_argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "echo hello".to_string(),
            ],
        }
    }

    #[test]
    fn json_roundtrip_preserves_every_field() {
        let original = sample_policy();
        let bytes = original.to_bytes().expect("serialize should succeed");
        let restored = SandboxInitPolicy::from_bytes(&bytes).expect("deserialize should succeed");
        assert_eq!(original, restored);
    }

    #[test]
    fn empty_policy_roundtrips() {
        let original = SandboxInitPolicy {
            landlock_rx_paths: vec![],
            landlock_rwx_paths: vec![],
            seccomp_bpf: vec![],
            target_uid: 0,
            target_gid: 0,
            require_fully_enforced: true,
            user_argv: vec!["/bin/true".to_string()],
        };
        let bytes = original.to_bytes().unwrap();
        let restored = SandboxInitPolicy::from_bytes(&bytes).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn malformed_bytes_return_codec_error() {
        // Arbitrary non-JSON bytes.
        let garbage: &[u8] = &[0xff, 0xfe, 0xfd, 0xfc, 0xfb];
        let result = SandboxInitPolicy::from_bytes(garbage);
        assert!(
            matches!(result, Err(InitPolicyError::Codec(_))),
            "garbage should deserialize to Codec error, got {result:?}"
        );
    }

    #[test]
    fn truncated_bytes_return_codec_error() {
        let original = sample_policy();
        let bytes = original.to_bytes().unwrap();
        // Chop off the closing brace — serde_json will report
        // unexpected EOF mid-object.
        let truncated = &bytes[..bytes.len() - 10];
        let result = SandboxInitPolicy::from_bytes(truncated);
        assert!(matches!(result, Err(InitPolicyError::Codec(_))));
    }

    #[test]
    fn distinct_policies_produce_distinct_bytes() {
        let a = sample_policy();
        let mut b = sample_policy();
        b.target_uid = 1000;
        assert_ne!(
            a.to_bytes().unwrap(),
            b.to_bytes().unwrap(),
            "changing one field must change the serialized bytes"
        );
    }
}
