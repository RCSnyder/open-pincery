//! AC-53: slirp4netns user-mode network namespace.
//!
//! Stub — populated in Slice A2b.4. Will fork `slirp4netns` against
//! the sandboxed pid's net namespace, enforcing the egress allowlist
//! declared in the tool's `SandboxProfile`.
