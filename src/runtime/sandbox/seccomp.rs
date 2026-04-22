//! AC-53: seccomp-bpf default-deny syscall filter.
//!
//! Stub — populated in Slice A2b.4. Will load the JSON allowlist at
//! `profiles/seccomp.json` into a `seccompiler::SeccompFilter`,
//! install it with `SECCOMP_FILTER_FLAG_LOG` in audit mode, and
//! fail-closed on any unlisted syscall.
