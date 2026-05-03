//! AC-80: Capability Nonce / Freshness (Phase G Slice G5).
//!
//! Binds every `IssueToolCall` to a freshly-minted, single-use,
//! 60-second-expiring nonce so a captured wake transcript or a
//! compromised wake cannot replay yesterday's authorization.
//!
//! Mint site: `src/runtime/wake_loop.rs::run_wake_loop`, AFTER the
//! AC-79 JSON-Schema validation gate and the per-wake tool-call
//! rate-limit check, immediately BEFORE `tools::dispatch_tool`. This
//! is the canonical TLA+ `AuthorizeExecution` boundary as ratified
//! by readiness R-AC80-7: minting downstream of AC-79 keeps
//! schema-invalid replays from leaving orphan rows in
//! `capability_nonces`, which strengthens T-AC80-12 (storage-growth
//! attack-multiplier) without weakening T-AC80-1..6. See
//! `docs/input/OpenPinceryCanonical.tla` line 845 for the abstract
//! action and `scaffolding/readiness.md` (R-AC80-7) for the
//! placement rationale.
//!
//! Consume site: `src/runtime/tools.rs::dispatch_tool`, AFTER the
//! AC-35 capability-mode gate and BEFORE any per-tool argument
//! deserialization or executor side effect. The consume statement
//! is a single atomic `UPDATE … RETURNING id`; a zero-row result
//! is the rejection signal (no separate SELECT-then-UPDATE race).
//!
//! Workspace-scoped (T-AC80-6): every row carries `workspace_id NOT
//! NULL` and the consume predicate pins it, so a leaked nonce from
//! workspace A cannot be redeemed under workspace B.
//!
//! Truth set: see `scaffolding/readiness.md` `## AC-80 Readiness`
//! (T-AC80-1..12). Closes canonical TODO G7 + G11.
//!
//! Storage growth: rows are not actively swept in v9.0. The consume
//! predicate filters `expires_at > now()` so expired rows are
//! unreachable. Periodic background sweep deferred to v9.1.

use rand::rngs::OsRng;
use rand::TryRngCore;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

/// 60-second TTL bound on every minted nonce. Hardcoded; no env knob
/// in v9.0 (T-AC80-5). Operators wanting a different bound take it
/// up in v9.1.
pub const CAPABILITY_NONCE_TTL_SECS: i64 = 60;

/// 16 random bytes from `OsRng`. Bytea-persisted (T-AC80-1).
pub const CAPABILITY_NONCE_LEN: usize = 16;

/// A 16-byte capability nonce paired with the canonical-shape hash
/// of the LLM-proposed tool arguments. Both halves are required at
/// consume time so a nonce minted for `shell { command: "ls" }`
/// cannot redeem `shell { command: "rm -rf /" }` even within the
/// same wake/tool (T-AC80-1, R-AC80-4).
#[derive(Debug, Clone)]
pub struct CapabilityNonceTicket {
    pub nonce: [u8; CAPABILITY_NONCE_LEN],
    pub capability_shape: String,
}

/// Reason a `consume` rejection is reported in the
/// `capability_nonce_rejected` event payload (T-AC80-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionReason {
    /// Nonce row exists but `consumed_at IS NOT NULL` already.
    Replay,
    /// Nonce row exists but `wake_id` does not match the consumer.
    CrossWake,
    /// Nonce row exists but `expires_at <= now()`.
    Expired,
    /// Nonce row exists but `capability_shape` does not match.
    ShapeMismatch,
    /// No row matched `(workspace_id, nonce)` at all (cross-workspace
    /// attempt, never minted, or already swept).
    Unknown,
}

impl RejectionReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Replay => "replay",
            Self::CrossWake => "cross_wake",
            Self::Expired => "expired",
            Self::ShapeMismatch => "shape_mismatch",
            Self::Unknown => "unknown",
        }
    }
}

/// Compute the canonical-shape hash of the LLM-proposed tool args.
///
/// Deterministic across mint and consume: the LLM-supplied
/// `arguments` JSON string is parsed, re-serialized with
/// sorted-keys + no whitespace canonicalization, then SHA-256-hashed
/// into a lowercase 64-char hex string.
///
/// On parse failure the raw byte content is hashed directly so that
/// non-JSON arguments still bind a stable shape (the AC-79 schema
/// guard separately rejects malformed JSON before consume runs).
pub fn capability_shape(args_json: &str) -> String {
    let canonical = match serde_json::from_str::<serde_json::Value>(args_json) {
        Ok(v) => canonical_json(&v),
        Err(_) => args_json.to_string(),
    };
    let mut h = Sha256::new();
    h.update(canonical.as_bytes());
    hex_lower(&h.finalize())
}

/// Recursive canonical-JSON serializer: object keys are sorted
/// lexicographically; no whitespace; numbers, strings, bools, null,
/// arrays preserve `serde_json` semantics.
fn canonical_json(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\""))
        }
        serde_json::Value::Array(items) => {
            let parts: Vec<String> = items.iter().map(canonical_json).collect();
            format!("[{}]", parts.join(","))
        }
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let parts: Vec<String> = keys
                .into_iter()
                .map(|k| {
                    let key = serde_json::to_string(k).unwrap_or_else(|_| format!("\"{k}\""));
                    let val = canonical_json(&map[k]);
                    format!("{key}:{val}")
                })
                .collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Mint a fresh nonce at the `AuthorizeExecution` boundary.
///
/// Returns the 16-byte nonce + the capability_shape that the consumer
/// must present. Inserts one row with
/// `expires_at = now() + INTERVAL '60 seconds'`. Caller threads the
/// returned ticket into its subsequent `dispatch_tool` invocation.
pub async fn mint(
    pool: &PgPool,
    wake_id: Uuid,
    workspace_id: Uuid,
    tool_name: &str,
    args_json: &str,
) -> Result<CapabilityNonceTicket, sqlx::Error> {
    let mut nonce = [0u8; CAPABILITY_NONCE_LEN];
    OsRng
        .try_fill_bytes(&mut nonce)
        .expect("AC-80 T-AC80-1: OsRng must produce a capability nonce");
    let shape = capability_shape(args_json);

    sqlx::query(
        "INSERT INTO capability_nonces \
         (wake_id, tool_name, capability_shape, nonce, expires_at, workspace_id) \
         VALUES ($1, $2, $3, $4, now() + ($5 || ' seconds')::interval, $6)",
    )
    .bind(wake_id)
    .bind(tool_name)
    .bind(&shape)
    .bind(&nonce[..])
    .bind(CAPABILITY_NONCE_TTL_SECS.to_string())
    .bind(workspace_id)
    .execute(pool)
    .await?;

    Ok(CapabilityNonceTicket {
        nonce,
        capability_shape: shape,
    })
}

/// Atomically consume a nonce at the `IssueToolCall` boundary.
///
/// Returns `Ok(())` on success. On any rejection — replay, cross-wake,
/// expired, shape mismatch, unknown, wrong workspace — returns
/// `Err(reason)`; caller emits `capability_nonce_rejected` and aborts
/// dispatch. The single-statement UPDATE serializes against concurrent
/// consumes (T-AC80-3, R-AC80-2).
pub async fn consume(
    pool: &PgPool,
    nonce: &[u8; CAPABILITY_NONCE_LEN],
    wake_id: Uuid,
    workspace_id: Uuid,
    tool_name: &str,
    capability_shape: &str,
) -> Result<(), RejectionReason> {
    // First the success path: a single atomic UPDATE.
    let row: Option<(Uuid,)> = sqlx::query_as(
        "UPDATE capability_nonces \
            SET consumed_at = now() \
          WHERE nonce = $1 \
            AND wake_id = $2 \
            AND tool_name = $3 \
            AND capability_shape = $4 \
            AND workspace_id = $5 \
            AND consumed_at IS NULL \
            AND expires_at > now() \
         RETURNING id",
    )
    .bind(&nonce[..])
    .bind(wake_id)
    .bind(tool_name)
    .bind(capability_shape)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| RejectionReason::Unknown)?;

    if row.is_some() {
        return Ok(());
    }

    // Zero-row consume — classify why so the rejection event payload
    // names a specific reason.
    Err(classify_rejection(
        pool,
        nonce,
        wake_id,
        workspace_id,
        tool_name,
        capability_shape,
    )
    .await)
}

/// Read-only follow-up that derives the rejection reason. Cheap:
/// single-row lookup against the unique `(workspace_id, nonce)` index.
/// Falls back to `Unknown` on any DB error so the caller still
/// emits a rejection event.
async fn classify_rejection(
    pool: &PgPool,
    nonce: &[u8; CAPABILITY_NONCE_LEN],
    wake_id: Uuid,
    workspace_id: Uuid,
    tool_name: &str,
    capability_shape: &str,
) -> RejectionReason {
    type ClassifyRow = (
        Uuid,
        String,
        String,
        Option<chrono::DateTime<chrono::Utc>>,
        chrono::DateTime<chrono::Utc>,
    );
    let row: Option<ClassifyRow> = match sqlx::query_as::<_, ClassifyRow>(
        "SELECT wake_id, tool_name, capability_shape, consumed_at, expires_at \
               FROM capability_nonces \
              WHERE workspace_id = $1 AND nonce = $2",
    )
    .bind(workspace_id)
    .bind(&nonce[..])
    .fetch_optional(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            // Audit-honesty: a transient DB error during classification
            // is observably indistinguishable from a hostile cross-
            // workspace probe (both surface as RejectionReason::Unknown).
            // Emit a warn so the operator can correlate with infra
            // metrics; the rejection event still goes out so the audit
            // chain stays complete.
            tracing::warn!(
                error = %e,
                workspace_id = %workspace_id,
                wake_id = %wake_id,
                "AC-80 classify_rejection DB error — falling back to Unknown"
            );
            None
        }
    };

    let Some((row_wake, row_tool, row_shape, consumed_at, expires_at)) = row else {
        return RejectionReason::Unknown;
    };

    if consumed_at.is_some() {
        return RejectionReason::Replay;
    }
    if expires_at <= chrono::Utc::now() {
        return RejectionReason::Expired;
    }
    if row_wake != wake_id {
        return RejectionReason::CrossWake;
    }
    if row_tool != tool_name || row_shape != capability_shape {
        return RejectionReason::ShapeMismatch;
    }
    // The success-path UPDATE failed but every column matches and the
    // row is not consumed/expired — should not happen, but stays
    // closed-by-default.
    RejectionReason::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_shape_is_deterministic_for_same_args() {
        let a = capability_shape(r#"{"command":"ls","env":{}}"#);
        let b = capability_shape(r#"{"command":"ls","env":{}}"#);
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        assert!(a
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn capability_shape_ignores_key_order() {
        // {"a":1,"b":2} and {"b":2,"a":1} must hash identically.
        let a = capability_shape(r#"{"a":1,"b":2}"#);
        let b = capability_shape(r#"{"b":2,"a":1}"#);
        assert_eq!(a, b);
    }

    #[test]
    fn capability_shape_distinguishes_argument_values() {
        // The whole point of binding shape: ls != rm -rf /.
        let safe = capability_shape(r#"{"command":"ls /tmp"}"#);
        let evil = capability_shape(r#"{"command":"rm -rf /"}"#);
        assert_ne!(safe, evil);
    }

    #[test]
    fn capability_shape_distinguishes_nested_structures() {
        // R-AC80-4 regression: shape binding must reach into nested
        // env maps, not just top-level keys.
        let a = capability_shape(r#"{"command":"echo","env":{"X":"1"}}"#);
        let b = capability_shape(r#"{"command":"echo","env":{"X":"2"}}"#);
        assert_ne!(a, b);
    }

    #[test]
    fn capability_shape_handles_non_json_args() {
        // A non-JSON argument string still hashes to a stable 64-hex
        // value; AC-79 schema guard rejects parse failures separately.
        let a = capability_shape("not-json-at-all");
        let b = capability_shape("not-json-at-all");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn rejection_reason_strings_match_event_payload_contract() {
        // T-AC80-4: payload `reason` field is one of these literals.
        assert_eq!(RejectionReason::Replay.as_str(), "replay");
        assert_eq!(RejectionReason::CrossWake.as_str(), "cross_wake");
        assert_eq!(RejectionReason::Expired.as_str(), "expired");
        assert_eq!(RejectionReason::ShapeMismatch.as_str(), "shape_mismatch");
        assert_eq!(RejectionReason::Unknown.as_str(), "unknown");
    }

    #[test]
    fn ttl_constant_is_60_seconds() {
        // T-AC80-5: hardcoded 60s. A change here is a v9.1 spec
        // decision; this test is the brake.
        assert_eq!(CAPABILITY_NONCE_TTL_SECS, 60);
    }
}
