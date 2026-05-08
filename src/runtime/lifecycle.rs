//! AC-82 (v9 Slice G7b+) — `lifecycle_transition` event emission helper.
//!
//! Every fine-grained CAS write to `agents.status` is paired with one
//! `lifecycle_transition` event so the AC-78 hash chain transparently
//! records the canonical-spec action that drove the transition.
//!
//! # Payload
//!
//! Per `T-AC82-3` / `R-AC82-4`, the event `content` column carries a
//! **canonical-JSON** object with exactly four keys, alphabetically
//! sorted, no whitespace, no trailing newline:
//!
//! ```json
//! {"canonical_action":"AttemptWakeAcquire","from":"asleep","to":"wake_acquiring","wake_id":"…"}
//! ```
//!
//! Canonical encoding matters because `events.content` feeds the
//! AC-78 SHA-256 chain pre-image; any whitespace or key-order drift
//! would invalidate the chain across deployments. This module hand-
//! builds the JSON string so we never depend on `serde_json`'s
//! key-order behaviour (which is implementation-defined for
//! `serde_json::Value` maps).
//!
//! # Source
//!
//! All `lifecycle_transition` events use `source = "runtime"` —
//! they are produced by the agent runtime, not by the agent or a
//! human operator (mirrors AC-78/AC-79/AC-80 conventions).

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::event;

/// Event type string for every lifecycle transition row.
pub const EVENT_TYPE: &str = "lifecycle_transition";
/// `source` column value — mirrors AC-78/AC-79/AC-80 runtime events.
pub const EVENT_SOURCE: &str = "runtime";

/// Build the canonical-JSON content string for one transition. Hand-
/// written so key order is deterministic regardless of `serde_json`
/// internal map ordering. Strings are quoted with `serde_json::Value`
/// to guarantee correct UTF-8 / control-character escaping.
fn canonical_payload(from: &str, to: &str, action: &str, wake_id: Uuid) -> String {
    use serde_json::Value;
    format!(
        "{{\"canonical_action\":{},\"from\":{},\"to\":{},\"wake_id\":{}}}",
        Value::String(action.to_string()),
        Value::String(from.to_string()),
        Value::String(to.to_string()),
        Value::String(wake_id.to_string()),
    )
}

/// Append a `lifecycle_transition` event for one CAS write. Callers
/// invoke this **after** the CAS helper returns `Some(_)` so the
/// event row corresponds to a real status change (T-AC82-2 forbids
/// emitting the event without the CAS).
pub async fn emit(
    pool: &PgPool,
    agent_id: Uuid,
    wake_id: Uuid,
    from: &str,
    to: &str,
    canonical_action: &str,
) -> Result<(), AppError> {
    let payload = canonical_payload(from, to, canonical_action, wake_id);
    event::append_event(
        pool,
        agent_id,
        EVENT_TYPE,
        EVENT_SOURCE,
        Some(wake_id),
        None,
        None,
        None,
        Some(&payload),
        None,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_has_canonical_key_order() {
        let wake = Uuid::nil();
        let p = canonical_payload("asleep", "wake_acquiring", "AttemptWakeAcquire", wake);
        // Keys are alphabetical: canonical_action < from < to < wake_id.
        assert_eq!(
            p,
            format!(
                "{{\"canonical_action\":\"AttemptWakeAcquire\",\"from\":\"asleep\",\"to\":\"wake_acquiring\",\"wake_id\":\"{wake}\"}}"
            )
        );
    }

    #[test]
    fn payload_round_trips_as_json() {
        let wake = Uuid::nil();
        let p = canonical_payload("awake", "tool_dispatching", "ToolDispatches", wake);
        let v: serde_json::Value = serde_json::from_str(&p).expect("must parse");
        assert_eq!(v["canonical_action"], "ToolDispatches");
        assert_eq!(v["from"], "awake");
        assert_eq!(v["to"], "tool_dispatching");
        assert_eq!(v["wake_id"], wake.to_string());
    }

    #[test]
    fn payload_is_byte_stable() {
        // Pin: identical inputs produce identical bytes (chain
        // reproducibility across deployments — R-AC82-4).
        let w = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let a = canonical_payload("awake", "wake_ending", "TerminalEndsWake", w);
        let b = canonical_payload("awake", "wake_ending", "TerminalEndsWake", w);
        assert_eq!(a, b);
    }
}
