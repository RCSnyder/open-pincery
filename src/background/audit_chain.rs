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
use sha2::{Digest, Sha256};
use uuid::Uuid;

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
