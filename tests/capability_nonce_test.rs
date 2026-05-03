//! AC-80: capability nonce admission gate — adversarial integration tests.
//!
//! These exercise the production mint -> consume path against a real
//! Postgres pool to confirm the gate cannot be bypassed by replay,
//! cross-wake reuse, expiry, cross-workspace reuse, or shape mismatch.
//!
//! Each test owns its `(wake_id, workspace_id)` pair so that nonce
//! rows from sibling tests cannot cross-contaminate even when the
//! `capability_nonces` table is shared across the suite.

mod common;

use open_pincery::models::{agent, event, user, workspace};
use open_pincery::runtime::capability_nonce::{
    self, CapabilityNonceTicket, RejectionReason, CAPABILITY_NONCE_LEN,
};
use uuid::Uuid;

// --- Layer 1: pure consume semantics against the live table -----------

#[tokio::test]
async fn valid_nonce_consumes_once_then_replay_is_rejected() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "ac80-replay@test.local", "AC80")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac80-replay-org", "ac80-replay-org", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac80-replay-ws", "ac80-replay-ws", u.id)
        .await
        .unwrap();
    let wake_id = Uuid::new_v4();
    let args = r#"{"command":"echo hi"}"#;

    let ticket = capability_nonce::mint(&pool, wake_id, ws.id, "shell", args)
        .await
        .expect("mint");

    // First consume: success.
    capability_nonce::consume(
        &pool,
        &ticket.nonce,
        wake_id,
        ws.id,
        "shell",
        &ticket.capability_shape,
    )
    .await
    .expect("first consume must succeed");

    // Second consume: T-AC80-2 — Replay rejection.
    let err = capability_nonce::consume(
        &pool,
        &ticket.nonce,
        wake_id,
        ws.id,
        "shell",
        &ticket.capability_shape,
    )
    .await
    .expect_err("replay must reject");
    assert_eq!(err, RejectionReason::Replay);
}

#[tokio::test]
async fn cross_wake_reuse_is_rejected() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "ac80-xwake@test.local", "AC80")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac80-xwake-org", "ac80-xwake-org", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac80-xwake-ws", "ac80-xwake-ws", u.id)
        .await
        .unwrap();
    let wake_a = Uuid::new_v4();
    let wake_b = Uuid::new_v4();
    let args = r#"{"command":"echo a"}"#;

    let ticket = capability_nonce::mint(&pool, wake_a, ws.id, "shell", args)
        .await
        .expect("mint");

    // T-AC80-3: a ticket minted under wake_a cannot be consumed under wake_b.
    let err = capability_nonce::consume(
        &pool,
        &ticket.nonce,
        wake_b,
        ws.id,
        "shell",
        &ticket.capability_shape,
    )
    .await
    .expect_err("cross-wake reuse must reject");
    assert_eq!(err, RejectionReason::CrossWake);
}

#[tokio::test]
async fn cross_workspace_reuse_is_rejected() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "ac80-xws@test.local", "AC80")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac80-xws-org", "ac80-xws-org", u.id)
        .await
        .unwrap();
    let ws_a = workspace::create_workspace(&pool, org.id, "ac80-xws-a", "ac80-xws-a", u.id)
        .await
        .unwrap();
    let ws_b = workspace::create_workspace(&pool, org.id, "ac80-xws-b", "ac80-xws-b", u.id)
        .await
        .unwrap();
    let wake_id = Uuid::new_v4();
    let args = r#"{"command":"echo x"}"#;

    let ticket = capability_nonce::mint(&pool, wake_id, ws_a.id, "shell", args)
        .await
        .expect("mint");

    // T-AC80-4: a ticket minted under ws_a cannot be consumed under ws_b.
    // The lookup is keyed on (workspace_id, nonce) so the row simply
    // does not exist for ws_b — Unknown is the correct reason.
    let err = capability_nonce::consume(
        &pool,
        &ticket.nonce,
        wake_id,
        ws_b.id,
        "shell",
        &ticket.capability_shape,
    )
    .await
    .expect_err("cross-workspace reuse must reject");
    assert_eq!(err, RejectionReason::Unknown);
}

#[tokio::test]
async fn expired_nonce_is_rejected() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "ac80-exp@test.local", "AC80")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac80-exp-org", "ac80-exp-org", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac80-exp-ws", "ac80-exp-ws", u.id)
        .await
        .unwrap();
    let wake_id = Uuid::new_v4();
    let args = r#"{"command":"echo expire"}"#;

    let ticket = capability_nonce::mint(&pool, wake_id, ws.id, "shell", args)
        .await
        .expect("mint");

    // T-AC80-5: backdate the row so expires_at is in the past, then
    // confirm consume rejects with Expired.
    sqlx::query(
        "UPDATE capability_nonces SET expires_at = now() - interval '1 second' \
         WHERE workspace_id = $1 AND nonce = $2",
    )
    .bind(ws.id)
    .bind(&ticket.nonce[..])
    .execute(&pool)
    .await
    .expect("backdate expires_at");

    let err = capability_nonce::consume(
        &pool,
        &ticket.nonce,
        wake_id,
        ws.id,
        "shell",
        &ticket.capability_shape,
    )
    .await
    .expect_err("expired ticket must reject");
    assert_eq!(err, RejectionReason::Expired);
}

#[tokio::test]
async fn shape_mismatch_is_rejected() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "ac80-shape@test.local", "AC80")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac80-shape-org", "ac80-shape-org", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac80-shape-ws", "ac80-shape-ws", u.id)
        .await
        .unwrap();
    let wake_id = Uuid::new_v4();

    // Mint for one set of args, then try to consume claiming a
    // different capability_shape. T-AC80-6: the gate must catch
    // post-authorization argument tampering.
    let ticket = capability_nonce::mint(&pool, wake_id, ws.id, "shell", r#"{"command":"echo a"}"#)
        .await
        .expect("mint");

    let tampered_shape = capability_nonce::capability_shape(r#"{"command":"echo b"}"#);
    assert_ne!(
        tampered_shape, ticket.capability_shape,
        "test precondition: shapes must differ"
    );

    let err = capability_nonce::consume(
        &pool,
        &ticket.nonce,
        wake_id,
        ws.id,
        "shell",
        &tampered_shape,
    )
    .await
    .expect_err("shape mismatch must reject");
    assert_eq!(err, RejectionReason::ShapeMismatch);
}

#[tokio::test]
async fn unknown_nonce_is_rejected() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "ac80-unk@test.local", "AC80")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac80-unk-org", "ac80-unk-org", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac80-unk-ws", "ac80-unk-ws", u.id)
        .await
        .unwrap();

    // T-AC80-7: a fabricated nonce that was never minted must reject.
    let phony = [0xAAu8; CAPABILITY_NONCE_LEN];
    let err = capability_nonce::consume(
        &pool,
        &phony,
        Uuid::new_v4(),
        ws.id,
        "shell",
        "doesnotmatter",
    )
    .await
    .expect_err("unknown nonce must reject");
    assert_eq!(err, RejectionReason::Unknown);
}

// --- Layer 2: dispatch_tool integration with audit-trail event ---------

#[tokio::test]
async fn dispatch_tool_emits_capability_nonce_rejected_on_replay() {
    use open_pincery::runtime::capability::PermissionMode;
    use open_pincery::runtime::llm::{FunctionCall, ToolCallRequest};
    use open_pincery::runtime::sandbox::{ProcessExecutor, ToolExecutor};
    use open_pincery::runtime::tools::{self, ToolResult};
    use open_pincery::runtime::vault::Vault;
    use std::sync::Arc;

    let pool = common::test_pool().await;
    let u = user::create_local_admin(&pool, "ac80-replay-evt@test.local", "AC80")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac80-rev-org", "ac80-rev-org", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac80-rev-ws", "ac80-rev-ws", u.id)
        .await
        .unwrap();
    let agent_row = agent::create_agent(&pool, "ac80-rev-agent", ws.id, u.id)
        .await
        .unwrap();
    let wake_id = Uuid::new_v4();

    let executor: Arc<dyn ToolExecutor> = Arc::new(ProcessExecutor);
    let vault = Arc::new(Vault::from_base64(common::TEST_VAULT_KEY_B64).unwrap());
    let tc = ToolCallRequest {
        id: "call-replay".into(),
        call_type: "function".into(),
        function: FunctionCall {
            name: "list_credentials".into(),
            arguments: "{}".into(),
        },
    };

    // First dispatch consumes the ticket cleanly.
    let ticket = capability_nonce::mint(
        &pool,
        wake_id,
        ws.id,
        &tc.function.name,
        &tc.function.arguments,
    )
    .await
    .expect("mint");
    let _ = tools::dispatch_tool(
        &tc,
        PermissionMode::Locked,
        &pool,
        agent_row.id,
        ws.id,
        wake_id,
        &executor,
        &vault,
        &ticket,
    )
    .await;

    // Replay the same ticket — dispatch_tool must reject before any
    // tool-specific work runs and must emit exactly one
    // capability_nonce_rejected event with reason="replay".
    let result = tools::dispatch_tool(
        &tc,
        PermissionMode::Locked,
        &pool,
        agent_row.id,
        ws.id,
        wake_id,
        &executor,
        &vault,
        &ticket,
    )
    .await;
    match result {
        ToolResult::Error(msg) => assert!(
            msg.contains("capability nonce rejected"),
            "replay must surface capability-nonce error; got {msg:?}"
        ),
        other => panic!(
            "expected Error, got {:?} on replay",
            match other {
                ToolResult::Output(_) => "Output",
                ToolResult::Sleep => "Sleep",
                ToolResult::Error(_) => "Error",
            }
        ),
    }

    let events = event::recent_events(&pool, agent_row.id, 50).await.unwrap();
    let rejected: Vec<_> = events
        .iter()
        .filter(|e| e.event_type == "capability_nonce_rejected" && e.wake_id == Some(wake_id))
        .collect();
    assert_eq!(
        rejected.len(),
        1,
        "exactly one capability_nonce_rejected event expected on replay; got {}",
        rejected.len()
    );
    let payload_str = rejected[0]
        .content
        .as_deref()
        .expect("capability_nonce_rejected must carry a content payload");
    let payload: serde_json::Value =
        serde_json::from_str(payload_str).expect("payload must be JSON");
    assert_eq!(
        payload.get("reason").and_then(|v| v.as_str()),
        Some("replay"),
        "replay must surface reason=replay; got {payload}"
    );
    assert_eq!(rejected[0].source, "runtime");
}

#[tokio::test]
async fn ac35_denied_call_does_not_consume_nonce() {
    use open_pincery::runtime::capability::PermissionMode;
    use open_pincery::runtime::llm::{FunctionCall, ToolCallRequest};
    use open_pincery::runtime::sandbox::{ProcessExecutor, ToolExecutor};
    use open_pincery::runtime::tools::{self, ToolResult};
    use open_pincery::runtime::vault::Vault;
    use std::sync::Arc;

    let pool = common::test_pool().await;
    let u = user::create_local_admin(&pool, "ac80-ac35@test.local", "AC80")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac80-ac35-org", "ac80-ac35-org", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac80-ac35-ws", "ac80-ac35-ws", u.id)
        .await
        .unwrap();
    let agent_row = agent::create_agent(&pool, "ac80-ac35-agent", ws.id, u.id)
        .await
        .unwrap();
    let wake_id = Uuid::new_v4();

    let executor: Arc<dyn ToolExecutor> = Arc::new(ProcessExecutor);
    let vault = Arc::new(Vault::from_base64(common::TEST_VAULT_KEY_B64).unwrap());
    let tc = ToolCallRequest {
        id: "call-ac35".into(),
        call_type: "function".into(),
        function: FunctionCall {
            name: "shell".into(),
            arguments: "{\"command\":\"true\"}".into(),
        },
    };

    let ticket = capability_nonce::mint(
        &pool,
        wake_id,
        ws.id,
        &tc.function.name,
        &tc.function.arguments,
    )
    .await
    .expect("mint");

    // Locked mode denies shell at AC-35 — BEFORE the AC-80 consume
    // runs (T-AC80-11). The ticket must remain unconsumed and stay
    // usable for a subsequent legitimate call.
    let result = tools::dispatch_tool(
        &tc,
        PermissionMode::Locked,
        &pool,
        agent_row.id,
        ws.id,
        wake_id,
        &executor,
        &vault,
        &ticket,
    )
    .await;
    match result {
        ToolResult::Error(msg) => assert!(
            msg.contains("disallowed"),
            "AC-35 denial must surface; got {msg:?}"
        ),
        _ => panic!("AC-35 denial must produce ToolResult::Error"),
    }

    // The nonce row must still be unconsumed: a follow-up consume
    // succeeds, proving the AC-35 denial took the ticket out of the
    // dispatch path before it reached the consume step.
    capability_nonce::consume(
        &pool,
        &ticket.nonce,
        wake_id,
        ws.id,
        "shell",
        &ticket.capability_shape,
    )
    .await
    .expect("ticket must remain unconsumed after AC-35 denial");

    // No capability_nonce_rejected event was emitted by the denied call.
    let events = event::recent_events(&pool, agent_row.id, 50).await.unwrap();
    assert!(
        !events
            .iter()
            .any(|e| e.event_type == "capability_nonce_rejected" && e.wake_id == Some(wake_id)),
        "AC-35-denied call must NOT emit capability_nonce_rejected"
    );
}

#[tokio::test]
async fn capability_shape_helper_is_publicly_callable() {
    // Sanity guard for downstream tooling: the canonical-JSON helper
    // must be callable from outside the crate so external auditors
    // can recompute the shape from raw event data.
    let a = capability_nonce::capability_shape(r#"{"a":1,"b":2}"#);
    let b = capability_nonce::capability_shape(r#"{"b":2,"a":1}"#);
    assert_eq!(a, b, "shape must be key-order-independent");
}

// Confirm the public surface of CapabilityNonceTicket has not silently
// regressed: callers in tests construct tickets via `mint`, and the
// fields are required to thread into `dispatch_tool`.
#[allow(dead_code)]
fn _ticket_field_visibility_smoke(
    t: &CapabilityNonceTicket,
) -> (&[u8; CAPABILITY_NONCE_LEN], &str) {
    (&t.nonce, t.capability_shape.as_str())
}

// --- Layer 3: REVIEW-fix-1 — coverage-table follow-ups ----------------

/// L-AC80-3 / R-AC80-2 / T-AC80-3+T-AC80-11: race two concurrent
/// `consume` calls against a single nonce row and confirm exactly one
/// returns `Ok(())` and the other returns `RejectionReason::Replay`.
/// The single atomic `UPDATE … WHERE consumed_at IS NULL …` plus the
/// `UNIQUE (workspace_id, nonce)` row lock is what makes this true;
/// without an explicit race test the property is only a code-shape
/// argument.
#[tokio::test]
async fn concurrent_consume_attempts_serialize() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "ac80-race@test.local", "AC80")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac80-race-org", "ac80-race-org", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac80-race-ws", "ac80-race-ws", u.id)
        .await
        .unwrap();
    let wake_id = Uuid::new_v4();
    let args = r#"{"command":"echo race"}"#;

    let ticket = capability_nonce::mint(&pool, wake_id, ws.id, "shell", args)
        .await
        .expect("mint");

    // Spawn two concurrent consumes against the same row. With the
    // single-statement `UPDATE … WHERE consumed_at IS NULL` predicate
    // the first task that grabs the row lock commits with consumed_at
    // set; the second blocks on the row lock and, on its turn, sees
    // consumed_at IS NOT NULL — predicate evaluates to false and the
    // RETURNING clause yields zero rows, surfacing as Replay.
    let pool_a = pool.clone();
    let pool_b = pool.clone();
    let nonce_a = ticket.nonce;
    let nonce_b = ticket.nonce;
    let shape_a = ticket.capability_shape.clone();
    let shape_b = ticket.capability_shape.clone();

    let h_a = tokio::spawn(async move {
        capability_nonce::consume(&pool_a, &nonce_a, wake_id, ws.id, "shell", &shape_a).await
    });
    let h_b = tokio::spawn(async move {
        capability_nonce::consume(&pool_b, &nonce_b, wake_id, ws.id, "shell", &shape_b).await
    });

    let r_a = h_a.await.expect("task a panic");
    let r_b = h_b.await.expect("task b panic");

    let oks = [&r_a, &r_b].iter().filter(|r| r.is_ok()).count();
    let replays = [&r_a, &r_b]
        .iter()
        .filter(|r| matches!(r, Err(RejectionReason::Replay)))
        .count();
    assert_eq!(
        oks, 1,
        "exactly one concurrent consume must succeed; got {oks} (a={r_a:?}, b={r_b:?})"
    );
    assert_eq!(
        replays, 1,
        "the losing concurrent consume must surface as Replay; got {replays} (a={r_a:?}, b={r_b:?})"
    );

    // T-AC80-3 row-shape invariant: exactly one row, with consumed_at IS NOT NULL.
    let cnt: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM capability_nonces \
         WHERE workspace_id = $1 AND nonce = $2 AND consumed_at IS NOT NULL",
    )
    .bind(ws.id)
    .bind(&ticket.nonce[..])
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(cnt.0, 1, "row must be marked consumed exactly once");
}

/// L-AC80-7 / T-AC80-7: the `capability_nonce_rejected` event must
/// chain through the AC-78 SHA-256 audit hash trigger transparently.
/// A REVIEW finding flagged that the existing replay-event test only
/// asserts the row exists — it does not walk the chain. This test
/// closes that gap by emitting at least one of every AC-78-eligible
/// event type for an agent (genesis + a `capability_nonce_rejected`)
/// and then running `verify_audit_chain` to prove `Verified`.
#[tokio::test]
async fn capability_nonce_rejected_chains_through_audit_hash() {
    use open_pincery::background::audit_chain::{verify_audit_chain, ChainStatus};
    use open_pincery::runtime::capability::PermissionMode;
    use open_pincery::runtime::llm::{FunctionCall, ToolCallRequest};
    use open_pincery::runtime::sandbox::{ProcessExecutor, ToolExecutor};
    use open_pincery::runtime::tools;
    use open_pincery::runtime::vault::Vault;
    use std::sync::Arc;

    let pool = common::test_pool().await;
    let u = user::create_local_admin(&pool, "ac80-chain@test.local", "AC80")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac80-chain-org", "ac80-chain-org", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac80-chain-ws", "ac80-chain-ws", u.id)
        .await
        .unwrap();
    let agent_row = agent::create_agent(&pool, "ac80-chain-agent", ws.id, u.id)
        .await
        .unwrap();
    let wake_id = Uuid::new_v4();

    let executor: Arc<dyn ToolExecutor> = Arc::new(ProcessExecutor);
    let vault = Arc::new(Vault::from_base64(common::TEST_VAULT_KEY_B64).unwrap());
    let tc = ToolCallRequest {
        id: "call-chain".into(),
        call_type: "function".into(),
        function: FunctionCall {
            name: "list_credentials".into(),
            arguments: "{}".into(),
        },
    };

    // Mint + first dispatch consumes the ticket cleanly (emits at
    // least a `tool_call` and `tool_result`-class events via the
    // happy path).
    let ticket = capability_nonce::mint(
        &pool,
        wake_id,
        ws.id,
        &tc.function.name,
        &tc.function.arguments,
    )
    .await
    .expect("mint");
    let _ = tools::dispatch_tool(
        &tc,
        PermissionMode::Locked,
        &pool,
        agent_row.id,
        ws.id,
        wake_id,
        &executor,
        &vault,
        &ticket,
    )
    .await;

    // Replay the same ticket — `dispatch_tool` MUST emit exactly one
    // `capability_nonce_rejected` event via `event::append_event`,
    // which fires the AC-78 BEFORE INSERT trigger that fills
    // `prev_hash` / `entry_hash`.
    let _ = tools::dispatch_tool(
        &tc,
        PermissionMode::Locked,
        &pool,
        agent_row.id,
        ws.id,
        wake_id,
        &executor,
        &vault,
        &ticket,
    )
    .await;

    // Sanity: the rejection event landed.
    let rejected_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM events \
         WHERE agent_id = $1 AND event_type = 'capability_nonce_rejected'",
    )
    .bind(agent_row.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        rejected_count.0, 1,
        "must emit exactly one capability_nonce_rejected event"
    );

    // Walk the chain and confirm every event — including the new
    // `capability_nonce_rejected` row — has matching prev_hash /
    // entry_hash. T-AC80-7 / T-AC78-10.
    let status = verify_audit_chain(&pool, agent_row.id)
        .await
        .expect("verify_audit_chain must not error");
    match status {
        ChainStatus::Verified {
            events_in_chain,
            last_entry_hash,
        } => {
            assert!(
                events_in_chain >= 1,
                "chain must contain at least the rejection event; got {events_in_chain}"
            );
            assert!(
                !last_entry_hash.is_empty(),
                "Verified must yield a non-empty last_entry_hash"
            );
        }
        ChainStatus::Broken {
            first_divergent_event_id,
            expected_hash,
            actual_hash,
            events_walked,
        } => panic!(
            "AC-78 chain MUST verify after capability_nonce_rejected: \
             broken at event {first_divergent_event_id} \
             (expected={expected_hash}, actual={actual_hash}, walked={events_walked})"
        ),
    }
}

/// L-AC80-1 / T-AC80-1+T-AC80-6: schema introspection guard. CI
/// migration success is necessary but weak evidence — this test
/// assert the `capability_nonces` columns, NOT NULL constraints,
/// and the two indexes the migration declares actually exist as
/// shipped.
#[tokio::test]
async fn table_shape_matches_scope() {
    let pool = common::test_pool().await;

    // Columns + nullability. We assert the eight scope-mandated
    // columns (id, wake_id, tool_name, capability_shape, nonce,
    // expires_at, consumed_at, workspace_id) plus created_at, with
    // the right NOT NULL discipline (only consumed_at is nullable).
    let cols: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT column_name, data_type, is_nullable \
           FROM information_schema.columns \
          WHERE table_schema = 'public' AND table_name = 'capability_nonces' \
          ORDER BY column_name",
    )
    .fetch_all(&pool)
    .await
    .expect("columns introspect");

    let names: Vec<&str> = cols.iter().map(|(n, _, _)| n.as_str()).collect();
    for required in [
        "id",
        "wake_id",
        "tool_name",
        "capability_shape",
        "nonce",
        "expires_at",
        "consumed_at",
        "workspace_id",
        "created_at",
    ] {
        assert!(
            names.contains(&required),
            "capability_nonces must declare column {required}; got {names:?}"
        );
    }

    for (name, _ty, nullable) in &cols {
        let must_be_not_null = !matches!(name.as_str(), "consumed_at");
        if must_be_not_null {
            assert_eq!(
                nullable, "NO",
                "column {name} must be NOT NULL per scope/T-AC80-6; got is_nullable={nullable}"
            );
        }
    }

    // Indexes: the unique (workspace_id, nonce) admission-gate index
    // and the (expires_at) sweeper index.
    let indexes: Vec<(String, String)> = sqlx::query_as(
        "SELECT indexname, indexdef FROM pg_indexes \
          WHERE schemaname = 'public' AND tablename = 'capability_nonces'",
    )
    .fetch_all(&pool)
    .await
    .expect("indexes introspect");
    let defs: Vec<&str> = indexes.iter().map(|(_, d)| d.as_str()).collect();
    assert!(
        defs.iter()
            .any(|d| d.contains("UNIQUE") && d.contains("workspace_id") && d.contains("nonce")),
        "must declare UNIQUE (workspace_id, nonce) admission-gate index; got {defs:?}"
    );
    assert!(
        defs.iter()
            .any(|d| d.contains("expires_at") && !d.contains("UNIQUE")),
        "must declare an (expires_at) sweeper index; got {defs:?}"
    );
}
