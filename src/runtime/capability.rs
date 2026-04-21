//! AC-35 (v6): Tool capability classification + permission-mode gate.
//!
//! This module is the single authority that decides whether a given tool
//! invocation is allowed for an agent operating in a specific permission
//! mode. The gate runs BEFORE the executor is consulted, so a denied call
//! never spawns a process, never hits the network, and never mutates state
//! outside the event log.
//!
//! Design constraints:
//!
//! 1. **Closed by default.** Any tool name not explicitly mapped in
//!    [`required_for`] is treated as [`ToolCapability::Destructive`], the
//!    strictest class. Adding a new tool without updating the map denies
//!    it under every mode except `Yolo`.
//! 2. **Unknown permission modes collapse to `Locked`.** A corrupted or
//!    never-set `agents.permission_mode` value can never accidentally
//!    widen privilege.
//! 3. **The gate table is total.** `mode_allows` is defined for every one
//!    of the 3 × 5 = 15 `(PermissionMode, ToolCapability)` pairs.

use serde::{Deserialize, Serialize};

/// The kind of side effect a tool call requests. Ordered roughly from
/// least to most dangerous; see [`mode_allows`] for the full gate table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCapability {
    /// Reads from the local filesystem or in-process state only.
    ReadLocal,
    /// Writes to the local filesystem.
    WriteLocal,
    /// Spawns a subprocess on the host.
    ExecuteLocal,
    /// Contacts the network.
    Network,
    /// Anything else. The strictest classification; unknown tools land here.
    Destructive,
}

/// The operator-chosen trust level for an agent. Stored as a lowercase
/// string in `agents.permission_mode`; the canonical enum here is the only
/// source of truth the gate consults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// Accept all classes without further check.
    Yolo,
    /// Accept everything except [`ToolCapability::Destructive`].
    Supervised,
    /// Accept only [`ToolCapability::ReadLocal`].
    Locked,
}

impl PermissionMode {
    /// Map the DB string to the enum. Unknown/corrupted values fail closed
    /// as [`PermissionMode::Locked`] so a misconfigured row cannot grant
    /// more privilege than the operator intended.
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "yolo" => PermissionMode::Yolo,
            "supervised" => PermissionMode::Supervised,
            "locked" => PermissionMode::Locked,
            _ => PermissionMode::Locked,
        }
    }
}

/// Classify a tool by name. Tools not listed here are treated as
/// [`ToolCapability::Destructive`] (closed-by-default).
pub fn required_for(tool_name: &str) -> ToolCapability {
    match tool_name {
        "shell" => ToolCapability::ExecuteLocal,
        "plan" => ToolCapability::ReadLocal,
        "sleep" => ToolCapability::ReadLocal,
        _ => ToolCapability::Destructive,
    }
}

/// The 15-cell gate. Returns `true` if `mode` is allowed to invoke a tool
/// classified as `capability`.
pub const fn mode_allows(mode: PermissionMode, capability: ToolCapability) -> bool {
    match (mode, capability) {
        // Yolo — no restrictions.
        (PermissionMode::Yolo, _) => true,

        // Supervised — block only Destructive; allow the other four.
        (PermissionMode::Supervised, ToolCapability::Destructive) => false,
        (PermissionMode::Supervised, _) => true,

        // Locked — allow ReadLocal only; deny the other four.
        (PermissionMode::Locked, ToolCapability::ReadLocal) => true,
        (PermissionMode::Locked, _) => false,
    }
}
