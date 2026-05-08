//! AC-82 (T-AC82-7 / G7f): static lint that pins the invariant
//! "every write to `agents.status` happens through a CAS helper in
//! [`open_pincery::models::agent`]."
//!
//! Why a grep test instead of a Clippy lint or a code review?
//!
//! The TLA+ refinement that AC-82 ships requires that every status
//! transition appears in the spec's `LifecycleAction` set and emits a
//! `lifecycle_transition` event with canonical-JSON content. The CAS
//! helpers in `src/models/agent.rs` are the single chokepoint that
//! enforces both: each helper is a CAS UPDATE with a `WHERE
//! status = '{prev}'` precondition, and each callsite in
//! `src/runtime/wake_loop.rs` / `src/runtime/drain.rs` /
//! `src/background/listener.rs` pairs the CAS with a
//! [`crate::runtime::lifecycle::emit`] call. A drive-by `UPDATE
//! agents SET status = '...'` anywhere else in the tree silently
//! breaks both invariants without surfacing a compile error.
//!
//! This test reads every `.rs` file under `src/` and asserts that
//! the substring `"UPDATE agents SET status"` (case-insensitive,
//! whitespace-normalized) appears only in the allowlisted helper
//! file. New status transitions must be added by extending
//! `agent.rs` with another CAS helper, never by inlining SQL into a
//! caller.
//!
//! See `scaffolding/readiness.md` AC-82 truth T-AC82-7 and risk
//! R-AC82-3 for context.

use std::fs;
use std::path::{Path, PathBuf};

const ALLOWLISTED_FILE: &str = "src/models/agent.rs";
const FORBIDDEN_NEEDLE: &str = "update agents set status";

#[test]
fn assert_status_writes_are_cas_only() {
    let src_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders: Vec<String> = Vec::new();

    walk(&src_root, &mut |path| {
        let rel = path
            .strip_prefix(env!("CARGO_MANIFEST_DIR"))
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if rel == ALLOWLISTED_FILE {
            return;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            return;
        }
        let body = match fs::read_to_string(path) {
            Ok(b) => b,
            Err(_) => return,
        };
        // AC-82 review-fix (Required #2): normalize the WHOLE file
        // (collapse all whitespace including newlines to single
        // spaces, lowercase) before searching, so the multi-line
        // shape `UPDATE agents\n    SET status = ...` (used by
        // every CAS helper in agent.rs) is also caught. A line-scoped
        // lint would miss any future drive-by caller copying that
        // idiom into another module, defeating R-AC82-3.
        let normalized = body
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        if normalized.contains(FORBIDDEN_NEEDLE) {
            offenders.push(rel.clone());
        }
    });

    assert!(
        offenders.is_empty(),
        "AC-82 T-AC82-7 violation: `UPDATE agents SET status` appears outside `{ALLOWLISTED_FILE}`. \
         Every status write must go through a CAS helper in `src/models/agent.rs` so the \
         transition pairs with a `lifecycle_transition` event (T-AC82-3) and respects the \
         multi-source admissibility of `enter_wake_ending` (T-AC82-4). Offenders: {offenders:?}"
    );
}

fn walk(dir: &Path, visit: &mut dyn FnMut(&Path)) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, visit);
        } else {
            visit(&path);
        }
    }
}
