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

// ───────────────────────── G3b: verifier + event emission ─────────────────────────

use open_pincery::background::audit_chain::{
    AgentChainResult, ChainStatus, EVENT_AUDIT_CHAIN_BROKEN, EVENT_AUDIT_CHAIN_VERIFIED,
};

/// Helper: build a fresh user/org/workspace/agent for an isolated chain.
async fn fresh_agent(pool: &sqlx::PgPool, slug: &str) -> (uuid::Uuid, uuid::Uuid) {
    let u = user::create_local_admin(pool, &format!("{slug}@test.com"), slug)
        .await
        .unwrap();
    let org = workspace::create_organization(pool, slug, slug, u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(pool, org.id, slug, slug, u.id)
        .await
        .unwrap();
    let a = agent::create_agent(pool, &format!("{slug}-agent"), ws.id, u.id)
        .await
        .unwrap();
    (a.id, ws.id)
}

/// T-AC78-1 (Rust verifier): a clean, multi-event chain verifies.
#[tokio::test]
async fn happy_path_chain_verifies() {
    let pool = common::test_pool().await;
    let (agent_id, _) = fresh_agent(&pool, "ac78happy").await;

    for i in 0..50 {
        event::append_event(
            &pool,
            agent_id,
            "message_received",
            "human",
            None,
            None,
            None,
            None,
            Some(&format!("msg-{i}")),
            None,
        )
        .await
        .unwrap();
    }

    let status = audit_chain::verify_audit_chain(&pool, agent_id)
        .await
        .unwrap();
    match status {
        ChainStatus::Verified {
            events_in_chain,
            last_entry_hash,
        } => {
            assert_eq!(events_in_chain, 50);
            assert_eq!(last_entry_hash.len(), 64);
        }
        ChainStatus::Broken { .. } => panic!("clean chain reported as broken"),
    }
}

/// T-AC78-4: post-insert UPDATE on `content` must be detected by the
/// verifier; `first_divergent_event_id` must point at the tampered row.
#[tokio::test]
async fn manual_update_breaks_chain() {
    let pool = common::test_pool().await;
    let (agent_id, _) = fresh_agent(&pool, "ac78tamp").await;

    let mut ids = Vec::new();
    for i in 0..5 {
        let e = event::append_event(
            &pool,
            agent_id,
            "message_received",
            "human",
            None,
            None,
            None,
            None,
            Some(&format!("msg-{i}")),
            None,
        )
        .await
        .unwrap();
        ids.push(e.id);
    }

    // Tamper with the third event's content. The trigger only fires on
    // INSERT, so prev_hash/entry_hash do NOT change — but the
    // recomputed hash will diverge.
    sqlx::query("UPDATE events SET content = 'tampered' WHERE id = $1")
        .bind(ids[2])
        .execute(&pool)
        .await
        .unwrap();

    let status = audit_chain::verify_audit_chain(&pool, agent_id)
        .await
        .unwrap();
    match status {
        ChainStatus::Broken {
            first_divergent_event_id,
            events_walked,
            ..
        } => {
            assert_eq!(first_divergent_event_id, ids[2]);
            assert_eq!(events_walked, 2, "walked 2 clean events before the tamper");
        }
        ChainStatus::Verified { .. } => panic!("tampered chain reported as verified"),
    }
}

/// T-AC78-3: many concurrent inserts on the same agent serialize via
/// the trigger's `FOR UPDATE` lock and produce an unbroken chain.
#[tokio::test]
async fn concurrent_inserts_preserve_chain() {
    let pool = common::test_pool().await;
    let (agent_id, _) = fresh_agent(&pool, "ac78conc").await;

    let mut handles = Vec::new();
    for task in 0..8 {
        let p = pool.clone();
        let h = tokio::spawn(async move {
            for i in 0..50 {
                event::append_event(
                    &p,
                    agent_id,
                    "message_received",
                    "human",
                    None,
                    None,
                    None,
                    None,
                    Some(&format!("t{task}-{i}")),
                    None,
                )
                .await
                .unwrap();
            }
        });
        handles.push(h);
    }
    for h in handles {
        h.await.unwrap();
    }

    let status = audit_chain::verify_audit_chain(&pool, agent_id)
        .await
        .unwrap();
    match status {
        ChainStatus::Verified {
            events_in_chain, ..
        } => assert_eq!(events_in_chain, 8 * 50),
        ChainStatus::Broken { .. } => panic!("concurrent inserts broke the chain"),
    }
}

/// T-AC78-5 / T-AC78-10: the verifier emits exactly one
/// `audit_chain_verified` event with the expected payload and that
/// event itself extends the chain cleanly.
#[tokio::test]
async fn verifier_emits_audit_chain_verified_event() {
    let pool = common::test_pool().await;
    let (agent_id, _) = fresh_agent(&pool, "ac78emitok").await;

    for i in 0..3 {
        event::append_event(
            &pool,
            agent_id,
            "message_received",
            "human",
            None,
            None,
            None,
            None,
            Some(&format!("msg-{i}")),
            None,
        )
        .await
        .unwrap();
    }

    let status = audit_chain::verify_and_emit(&pool, agent_id).await.unwrap();
    assert!(matches!(status, ChainStatus::Verified { .. }));

    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM events WHERE agent_id = $1 AND event_type = $2")
            .bind(agent_id)
            .bind(EVENT_AUDIT_CHAIN_VERIFIED)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);

    // The newly appended audit_chain_verified event must extend the
    // chain — re-verify and expect Verified again.
    let after = audit_chain::verify_audit_chain(&pool, agent_id)
        .await
        .unwrap();
    assert!(matches!(after, ChainStatus::Verified { events_in_chain, .. } if events_in_chain == 4));
}

/// T-AC78-5: verifier emits `audit_chain_broken` with the right
/// `first_divergent_event_id` when the chain is broken.
#[tokio::test]
async fn verifier_emits_audit_chain_broken_event_with_correct_id() {
    let pool = common::test_pool().await;
    let (agent_id, _) = fresh_agent(&pool, "ac78emitbroken").await;

    let mut ids = Vec::new();
    for i in 0..4 {
        let e = event::append_event(
            &pool,
            agent_id,
            "message_received",
            "human",
            None,
            None,
            None,
            None,
            Some(&format!("msg-{i}")),
            None,
        )
        .await
        .unwrap();
        ids.push(e.id);
    }

    sqlx::query("UPDATE events SET content = 'evil' WHERE id = $1")
        .bind(ids[1])
        .execute(&pool)
        .await
        .unwrap();

    let status = audit_chain::verify_and_emit(&pool, agent_id).await.unwrap();
    let broken_id = match status {
        ChainStatus::Broken {
            first_divergent_event_id,
            ..
        } => first_divergent_event_id,
        ChainStatus::Verified { .. } => panic!("expected Broken status"),
    };
    assert_eq!(broken_id, ids[1]);

    let row: (String, Option<String>) = sqlx::query_as(
        "SELECT event_type, content FROM events
         WHERE agent_id = $1 AND event_type = $2",
    )
    .bind(agent_id)
    .bind(EVENT_AUDIT_CHAIN_BROKEN)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, EVENT_AUDIT_CHAIN_BROKEN);
    let payload: serde_json::Value = serde_json::from_str(row.1.as_deref().unwrap()).unwrap();
    assert_eq!(
        payload["first_divergent_event_id"],
        serde_json::json!(ids[1])
    );
}

/// T-AC78-11: the verifier is read-only with respect to pre-existing
/// rows. It MUST NOT update or delete any event row when called via
/// `verify_audit_chain` (the read-only entry point).
#[tokio::test]
async fn verifier_does_not_mutate_events() {
    let pool = common::test_pool().await;
    let (agent_id, _) = fresh_agent(&pool, "ac78readonly").await;

    for i in 0..5 {
        event::append_event(
            &pool,
            agent_id,
            "message_received",
            "human",
            None,
            None,
            None,
            None,
            Some(&format!("msg-{i}")),
            None,
        )
        .await
        .unwrap();
    }

    let snapshot: Vec<(uuid::Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT id, entry_hash, content FROM events
         WHERE agent_id = $1 ORDER BY created_at, id",
    )
    .bind(agent_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    let _ = audit_chain::verify_audit_chain(&pool, agent_id)
        .await
        .unwrap();

    let after: Vec<(uuid::Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT id, entry_hash, content FROM events
         WHERE agent_id = $1 ORDER BY created_at, id",
    )
    .bind(agent_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(snapshot, after, "verify_audit_chain must not mutate events");
}

/// G3b workspace verifier: walks every agent in the workspace and
/// returns one result per agent, in agent-id order.
#[tokio::test]
async fn verify_workspace_returns_one_result_per_agent() {
    let pool = common::test_pool().await;
    let u = user::create_local_admin(&pool, "ac78ws@test.com", "Wsv")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac78ws", "ac78ws", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac78ws", "ac78ws", u.id)
        .await
        .unwrap();
    let a1 = agent::create_agent(&pool, "ws-a1", ws.id, u.id)
        .await
        .unwrap();
    let a2 = agent::create_agent(&pool, "ws-a2", ws.id, u.id)
        .await
        .unwrap();

    for &aid in &[a1.id, a2.id] {
        event::append_event(
            &pool,
            aid,
            "message_received",
            "human",
            None,
            None,
            None,
            None,
            Some("hi"),
            None,
        )
        .await
        .unwrap();
    }

    let results = audit_chain::verify_workspace(&pool, ws.id).await.unwrap();
    assert_eq!(results.len(), 2);
    let mut agent_ids: Vec<_> = results
        .iter()
        .map(|r: &AgentChainResult| r.agent_id)
        .collect();
    agent_ids.sort();
    let mut expected = vec![a1.id, a2.id];
    expected.sort();
    assert_eq!(agent_ids, expected);
    for r in &results {
        assert!(matches!(r.status, ChainStatus::Verified { .. }));
    }
}
