//! AC-78 Event-Log Hash Chain — Rust verifier and canonical pre-image.
//!
//! The Postgres trigger in `migrations/20260501000001_add_event_hash_chain.sql`
//! computes per-agent SHA-256 chain entries on INSERT. This module
//! produces a **byte-identical** canonical pre-image so a Rust verifier
//! can recompute every entry hash and detect post-insert tampering.
//!
//! # Canonical pre-image
//!
//! For each event, the pre-image is the previous entry's hash bytes
//! (decoded from hex; empty for the genesis event of an agent),
//! concatenated with length-prefixed UTF-8 encodings of every
//! immutable field, in this fixed order:
//!
//! 1. `event_type`           (TEXT, never NULL)
//! 2. `agent_id::text`       (UUID, never NULL)
//! 3. `source`               (TEXT, NULL → `""`)
//! 4. `wake_id::text`        (UUID, NULL → `""`)
//! 5. `tool_name`            (TEXT, NULL → `""`)
//! 6. `tool_input`           (TEXT, NULL → `""`)
//! 7. `tool_output`          (TEXT, NULL → `""`)
//! 8. `content`              (TEXT, NULL → `""`)
//! 9. `termination_reason`   (TEXT, NULL → `""`)
//!
//! followed by a single length prefix `int4be(8)` and the
//! `created_at` timestamp encoded as `int8be` of microseconds since
//! the UNIX epoch. Each length prefix is `u32` big-endian.
//!
//! # Why length-prefixed
//!
//! Any delimiter-based scheme (`|`, NUL, JSON) is ambiguous on
//! arbitrary text content. Length-prefixed concatenation has no
//! collisions and trivially matches Postgres `int4send(length(b))`.

use chrono::{DateTime, Utc};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::event;

/// Event type emitted when a per-agent chain verifies cleanly.
pub const EVENT_AUDIT_CHAIN_VERIFIED: &str = "audit_chain_verified";

/// Event type emitted when a per-agent chain is broken (tamper detected).
pub const EVENT_AUDIT_CHAIN_BROKEN: &str = "audit_chain_broken";

/// Source value used by the verifier when appending its own events.
pub const VERIFIER_EVENT_SOURCE: &str = "runtime";

/// Append a length-prefixed UTF-8 encoding of `field` to `out`.
///
/// `None` is encoded as a zero-length empty string, matching the
/// Postgres `coalesce(field, '')` behaviour in the trigger.
fn push_field(out: &mut Vec<u8>, field: Option<&str>) {
    let bytes = field.unwrap_or("").as_bytes();
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
}

/// Canonical pre-image bytes for a single event. Mirrors the
/// `events_chain_canonical_payload` SQL function.
#[allow(clippy::too_many_arguments)]
pub fn canonical_payload(
    event_type: &str,
    agent_id: Uuid,
    source: Option<&str>,
    wake_id: Option<Uuid>,
    tool_name: Option<&str>,
    tool_input: Option<&str>,
    tool_output: Option<&str>,
    content: Option<&str>,
    termination_reason: Option<&str>,
    created_at: DateTime<Utc>,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);
    push_field(&mut out, Some(event_type));
    push_field(&mut out, Some(&agent_id.to_string()));
    push_field(&mut out, source);
    let wake_str = wake_id.map(|w| w.to_string());
    push_field(&mut out, wake_str.as_deref());
    push_field(&mut out, tool_name);
    push_field(&mut out, tool_input);
    push_field(&mut out, tool_output);
    push_field(&mut out, content);
    push_field(&mut out, termination_reason);

    let micros = created_at.timestamp_micros();
    out.extend_from_slice(&(8u32).to_be_bytes());
    out.extend_from_slice(&micros.to_be_bytes());

    out
}

/// Compute the entry hash given the prior entry's hex-encoded hash
/// (empty string for genesis) and the canonical payload bytes.
///
/// Returns the hex-encoded SHA-256 hash, lowercase (Postgres
/// `encode(..., 'hex')` produces lowercase).
pub fn compute_entry_hash(prev_hash_hex: &str, payload: &[u8]) -> String {
    let prev_bytes = if prev_hash_hex.is_empty() {
        Vec::new()
    } else {
        hex::decode(prev_hash_hex).unwrap_or_default()
    };
    let mut hasher = Sha256::new();
    hasher.update(&prev_bytes);
    hasher.update(payload);
    let digest = hasher.finalize();
    hex::encode(digest)
}

/// Outcome of walking one agent's hash chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainStatus {
    /// Every event's stored `entry_hash` matched the recomputed value.
    Verified {
        events_in_chain: u64,
        last_entry_hash: String,
    },
    /// First mismatch encountered. `expected_hash` is what the chain
    /// should have been (recomputed from the prior entry); `actual_hash`
    /// is what the row currently stores.
    Broken {
        first_divergent_event_id: Uuid,
        expected_hash: String,
        actual_hash: String,
        events_walked: u64,
    },
}

/// Walk every event for `agent_id` in `(created_at, id)` order,
/// recomputing the hash chain in Rust and comparing against the
/// stored `entry_hash` column.
///
/// **Read-only**: the function never UPDATEs or DELETEs. T-AC78-11
/// invariant.
pub async fn verify_audit_chain(pool: &PgPool, agent_id: Uuid) -> Result<ChainStatus, AppError> {
    let rows = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            String,
            String,
            Option<Uuid>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            DateTime<Utc>,
        ),
    >(
        "SELECT id, prev_hash, entry_hash, event_type, source, wake_id,
                tool_name, tool_input, tool_output, content,
                termination_reason, created_at
         FROM events
         WHERE agent_id = $1
         ORDER BY created_at ASC, id ASC",
    )
    .bind(agent_id)
    .fetch_all(pool)
    .await?;

    let mut expected_prev = String::new();
    let mut last_entry_hash = String::new();
    let mut walked: u64 = 0;

    for (
        id,
        prev_hash,
        entry_hash,
        event_type,
        source,
        wake_id,
        tool_name,
        tool_input,
        tool_output,
        content,
        termination_reason,
        created_at,
    ) in rows
    {
        // Detect prev_hash tampering before recomputing.
        if prev_hash != expected_prev {
            return Ok(ChainStatus::Broken {
                first_divergent_event_id: id,
                expected_hash: expected_prev,
                actual_hash: prev_hash,
                events_walked: walked,
            });
        }

        let payload = canonical_payload(
            &event_type,
            agent_id,
            Some(&source),
            wake_id,
            tool_name.as_deref(),
            tool_input.as_deref(),
            tool_output.as_deref(),
            content.as_deref(),
            termination_reason.as_deref(),
            created_at,
        );
        let recomputed = compute_entry_hash(&prev_hash, &payload);

        if recomputed != entry_hash {
            return Ok(ChainStatus::Broken {
                first_divergent_event_id: id,
                expected_hash: recomputed,
                actual_hash: entry_hash,
                events_walked: walked,
            });
        }

        expected_prev = entry_hash.clone();
        last_entry_hash = entry_hash;
        walked += 1;
    }

    Ok(ChainStatus::Verified {
        events_in_chain: walked,
        last_entry_hash,
    })
}

/// Verify `agent_id`'s chain and append exactly one
/// `audit_chain_verified` or `audit_chain_broken` event recording the
/// outcome. Returns the same `ChainStatus`.
///
/// The emitted event itself extends the chain (the trigger fills its
/// `prev_hash`/`entry_hash` from the previous tail), so calling this
/// twice in succession produces a clean chain on the second call.
pub async fn verify_and_emit(pool: &PgPool, agent_id: Uuid) -> Result<ChainStatus, AppError> {
    let status = verify_audit_chain(pool, agent_id).await?;

    match &status {
        ChainStatus::Verified {
            events_in_chain,
            last_entry_hash,
        } => {
            let payload = json!({
                "agent_id": agent_id,
                "events_in_chain": events_in_chain,
                "last_entry_hash": last_entry_hash,
            });
            event::append_event(
                pool,
                agent_id,
                EVENT_AUDIT_CHAIN_VERIFIED,
                VERIFIER_EVENT_SOURCE,
                None,
                None,
                None,
                Some(&payload.to_string()),
                None,
                None,
            )
            .await?;
        }
        ChainStatus::Broken {
            first_divergent_event_id,
            expected_hash,
            actual_hash,
            events_walked,
        } => {
            let payload = json!({
                "agent_id": agent_id,
                "first_divergent_event_id": first_divergent_event_id,
                "expected_hash": expected_hash,
                "actual_hash": actual_hash,
                "events_walked": events_walked,
            });
            event::append_event(
                pool,
                agent_id,
                EVENT_AUDIT_CHAIN_BROKEN,
                VERIFIER_EVENT_SOURCE,
                None,
                None,
                None,
                Some(&payload.to_string()),
                None,
                None,
            )
            .await?;
        }
    }

    Ok(status)
}

/// Per-agent verification result inside a workspace pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentChainResult {
    pub agent_id: Uuid,
    pub status: ChainStatus,
}

/// Walk every agent in `workspace_id` and call [`verify_and_emit`]
/// once per agent. Returns the per-agent outcomes in agent-id order.
///
/// Used by the startup gate (G3d) and the `pcy audit verify` CLI
/// (G3c). The function is read-mostly: it only writes the verifier's
/// own `audit_chain_*` events, never mutates pre-existing rows.
pub async fn verify_workspace(
    pool: &PgPool,
    workspace_id: Uuid,
) -> Result<Vec<AgentChainResult>, AppError> {
    let agents: Vec<(Uuid,)> =
        sqlx::query_as("SELECT id FROM agents WHERE workspace_id = $1 ORDER BY id ASC")
            .bind(workspace_id)
            .fetch_all(pool)
            .await?;

    let mut out = Vec::with_capacity(agents.len());
    for (agent_id,) in agents {
        let status = verify_and_emit(pool, agent_id).await?;
        out.push(AgentChainResult { agent_id, status });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_uuid(byte: u8) -> Uuid {
        let bytes = [byte; 16];
        Uuid::from_bytes(bytes)
    }

    #[test]
    fn push_field_zero_length_for_none() {
        let mut out = Vec::new();
        push_field(&mut out, None);
        assert_eq!(out, vec![0, 0, 0, 0]);
    }

    #[test]
    fn push_field_writes_big_endian_length_then_bytes() {
        let mut out = Vec::new();
        push_field(&mut out, Some("abc"));
        assert_eq!(out, vec![0, 0, 0, 3, b'a', b'b', b'c']);
    }

    #[test]
    fn canonical_payload_is_deterministic_and_distinct_per_field() {
        let agent = fixed_uuid(0xAA);
        let ts = DateTime::from_timestamp_micros(1_700_000_000_000_000).unwrap();
        let p1 = canonical_payload("e", agent, None, None, None, None, None, None, None, ts);
        let p2 = canonical_payload("e", agent, None, None, None, None, None, None, None, ts);
        let p3 = canonical_payload("E", agent, None, None, None, None, None, None, None, ts);
        assert_eq!(p1, p2, "deterministic for identical inputs");
        assert_ne!(p1, p3, "differs when event_type changes");
    }

    #[test]
    fn timestamp_encoded_as_big_endian_micros_at_tail() {
        let agent = fixed_uuid(0xAA);
        let ts = DateTime::from_timestamp_micros(0x0123_4567_89AB_CDEF).unwrap();
        let payload = canonical_payload("e", agent, None, None, None, None, None, None, None, ts);
        // Last 4 bytes before the i64 are the length prefix `int4be(8)`.
        let n = payload.len();
        assert_eq!(&payload[n - 12..n - 8], &[0, 0, 0, 8]);
        assert_eq!(&payload[n - 8..], &0x0123_4567_89AB_CDEFi64.to_be_bytes());
    }

    #[test]
    fn compute_entry_hash_genesis_treats_empty_prev_as_no_bytes() {
        let h_genesis = compute_entry_hash("", &[1, 2, 3]);
        // Reference: sha256(0x010203) produced by Python hashlib —
        // 039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81
        assert_eq!(
            h_genesis,
            "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81"
        );
    }

    #[test]
    fn compute_entry_hash_chains_prev() {
        let prev = "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81";
        let h = compute_entry_hash(prev, &[]);
        // sha256 of prev decoded bytes == known reference.
        let mut hasher = Sha256::new();
        hasher.update(hex::decode(prev).unwrap());
        let want = hex::encode(hasher.finalize());
        assert_eq!(h, want);
    }
}
