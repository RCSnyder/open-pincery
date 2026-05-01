//! AC-78 Event-Log Hash Chain — DB-backed integration tests (G3a).
//!
//! Validates the migration's trigger output against the Rust
//! canonical-pre-image (`compute_entry_hash` + `canonical_payload`),
//! plus per-agent chain isolation, NOT NULL columns, and genesis
//! semantics.
//!
//! G3b/G3c/G3d/G3e tests live alongside this file and are added in
//! later slices.

mod common;

use open_pincery::background::audit_chain;
use open_pincery::models::{agent, event, user, workspace};
use sqlx::Row;

/// T-AC78-1 / G3a: the genesis event for a new agent has
/// `prev_hash = ''` and `entry_hash` matches the Rust reference.
#[tokio::test]
async fn genesis_event_uses_empty_prev_hash() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "ac78gen@test.com", "Gen")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac78gen", "ac78gen", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac78gen", "ac78gen", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "ac78-gen-agent", ws.id, u.id)
        .await
        .unwrap();

    let e = event::append_event(
        &pool,
        a.id,
        "message_received",
        "human",
        None,
        None,
        None,
        None,
        Some("genesis"),
        None,
    )
    .await
    .unwrap();

    let row = sqlx::query("SELECT prev_hash, entry_hash, created_at FROM events WHERE id = $1")
        .bind(e.id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let prev_hash: String = row.get("prev_hash");
    let entry_hash: String = row.get("entry_hash");
    let created_at: chrono::DateTime<chrono::Utc> = row.get("created_at");

    assert_eq!(prev_hash, "", "genesis prev_hash must be empty");
    assert_eq!(entry_hash.len(), 64, "sha256 hex is 64 chars");

    // Recompute via Rust and compare bytes-for-bytes against trigger.
    let payload = audit_chain::canonical_payload(
        "message_received",
        a.id,
        Some("human"),
        None,
        None,
        None,
        None,
        Some("genesis"),
        None,
        created_at,
    );
    let want = audit_chain::compute_entry_hash("", &payload);
    assert_eq!(entry_hash, want, "Rust pre-image must match trigger output");
}

/// T-AC78-2 / G3a: the second event for an agent uses the first
/// event's `entry_hash` as its `prev_hash`, and its own `entry_hash`
/// chains forward correctly.
#[tokio::test]
async fn trigger_assigns_prev_hash_from_previous_event() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "ac78chain@test.com", "Chain")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac78chain", "ac78chain", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac78chain", "ac78chain", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "ac78-chain-agent", ws.id, u.id)
        .await
        .unwrap();

    let e1 = event::append_event(
        &pool,
        a.id,
        "message_received",
        "human",
        None,
        None,
        None,
        None,
        Some("one"),
        None,
    )
    .await
    .unwrap();
    let e2 = event::append_event(
        &pool,
        a.id,
        "message_received",
        "human",
        None,
        None,
        None,
        None,
        Some("two"),
        None,
    )
    .await
    .unwrap();

    let row1 = sqlx::query("SELECT entry_hash FROM events WHERE id = $1")
        .bind(e1.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let row2 = sqlx::query("SELECT prev_hash, entry_hash, created_at FROM events WHERE id = $1")
        .bind(e2.id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let h1: String = row1.get("entry_hash");
    let prev2: String = row2.get("prev_hash");
    let h2: String = row2.get("entry_hash");
    let ts2: chrono::DateTime<chrono::Utc> = row2.get("created_at");

    assert_eq!(
        prev2, h1,
        "second event's prev_hash must equal first's entry_hash"
    );

    let payload = audit_chain::canonical_payload(
        "message_received",
        a.id,
        Some("human"),
        None,
        None,
        None,
        None,
        Some("two"),
        None,
        ts2,
    );
    let want = audit_chain::compute_entry_hash(&h1, &payload);
    assert_eq!(h2, want);
}

/// T-AC78-1 / G3a: chain root is per-agent — events for agent A do
/// not influence the prev_hash of agent B's first event.
#[tokio::test]
async fn chain_is_per_agent() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "ac78perag@test.com", "PerAg")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac78perag", "ac78perag", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac78perag", "ac78perag", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "ac78-a", ws.id, u.id)
        .await
        .unwrap();
    let b = agent::create_agent(&pool, "ac78-b", ws.id, u.id)
        .await
        .unwrap();

    // Insert several events on agent a.
    for content in ["a1", "a2", "a3"] {
        event::append_event(
            &pool,
            a.id,
            "message_received",
            "human",
            None,
            None,
            None,
            None,
            Some(content),
            None,
        )
        .await
        .unwrap();
    }

    // First event on agent b — should still be genesis (prev_hash="").
    let eb = event::append_event(
        &pool,
        b.id,
        "message_received",
        "human",
        None,
        None,
        None,
        None,
        Some("b1"),
        None,
    )
    .await
    .unwrap();

    let prev: String = sqlx::query_scalar("SELECT prev_hash FROM events WHERE id = $1")
        .bind(eb.id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(prev, "", "agent B's first event must be its own genesis");
}

/// T-AC78-9 / G3a: prev_hash and entry_hash columns are NOT NULL after
/// the migration (proves the SET NOT NULL step landed).
#[tokio::test]
async fn hash_columns_are_not_null() {
    let pool = common::test_pool().await;

    let prev_nullable: bool = sqlx::query_scalar(
        "SELECT is_nullable = 'YES' FROM information_schema.columns
         WHERE table_name = 'events' AND column_name = 'prev_hash'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let entry_nullable: bool = sqlx::query_scalar(
        "SELECT is_nullable = 'YES' FROM information_schema.columns
         WHERE table_name = 'events' AND column_name = 'entry_hash'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(!prev_nullable, "events.prev_hash must be NOT NULL");
    assert!(!entry_nullable, "events.entry_hash must be NOT NULL");
}
