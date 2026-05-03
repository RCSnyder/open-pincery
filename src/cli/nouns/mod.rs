//! AC-46 (v8): noun-verb CLI tree.
//!
//! Each sub-module implements one noun with its verb subcommands
//! (`list`, `get`, `create`, `delete`, noun-specific verbs). Verbs
//! accept `--output` and funnel output through [`crate::cli::output`]
//! and, where they take a target, resolve name-or-UUID input through
//! [`crate::cli::resolve`].
//!
//! Slice 2d lands the nouns incrementally:
//! - **2d-i** (this commit): `context` (pure on-disk, no HTTP).
//! - **2d-ii**: `credential` + shim delegate from legacy `pcy credential`.
//! - **2d-iii**: `agent`, `budget`, `event`, `message` shims.
//! - **2e**: root `Cli` gains `--context` / `--output` / `--no-color`.
//!
//! [`warn_deprecated`] is shared by every shim that forwards a legacy
//! top-level command (`pcy bootstrap`, `pcy message`, `pcy events`)
//! to its new verb. It emits exactly one `warning:` line on stderr —
//! the AC-46 byte-identical-stdout test depends on the warning going
//! to **stderr**, not stdout.

pub mod context;

/// Print exactly one deprecation warning to stderr. Used by the
/// legacy shim commands to notify operators without polluting stdout.
///
/// Format: `warning: '<old>' is deprecated; use '<new>'`.
pub fn warn_deprecated(old: &str, new: &str) {
    eprintln!("warning: '{old}' is deprecated; use '{new}'");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warn_deprecated_is_pure_string_behaviour() {
        // Smoke check: the helper is one line, no panic, no stdout
        // writes. The actual stderr-byte-equality assertions live in
        // the per-shim integration tests in Slice 2d-ii.
        warn_deprecated("bootstrap", "auth login");
    }
}
