mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use open_pincery::config::Config;
use open_pincery::models::{agent, event, user, workspace};
use open_pincery::runtime::llm::LlmClient;
use rust_decimal::Decimal;
use tokio_util::sync::CancellationToken;

/// AC-23: When budget is exhausted, listener refuses wake acquisition, records
/// exactly one budget_exceeded event, and does not insert llm_calls.
#[tokio::test]
async fn test_budget_exceeded_blocks_wake_and_llm_call() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "budget@test.com", "Budget")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "budget", "budget", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "budget", "budget", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "budget-agent", ws.id, u.id)
        .await
        .unwrap();

    // Force exhausted budget state before triggering wake.
    sqlx::query(
        "UPDATE agents
         SET budget_limit_usd = $1, budget_used_usd = $2
         WHERE id = $3",
    )
    .bind(Decimal::new(1, 6))
    .bind(Decimal::new(2, 6))
    .bind(a.id)
    .execute(&pool)
    .await
    .unwrap();

    event::append_event(
        &pool,
        a.id,
        "message_received",
        "human",
        None,
        None,
        None,
        None,
        Some("wake please"),
        None,
    )
    .await
    .unwrap();

    let config = Arc::new(Config {
        database_url: String::new(),
        host: "127.0.0.1".into(),
        port: 0,
        bootstrap_token: "test".into(),
        llm_api_base_url: "http://127.0.0.1:1".into(),
        llm_api_key: "fake".into(),
        llm_model: "test-model".into(),
        llm_maintenance_model: "test-model".into(),
        max_prompt_chars: 100000,
        iteration_cap: 50,
        stale_wake_hours: 2,
        wake_summary_limit: 20,
        event_window_limit: 200,
    });

    // Should never be called when budget is exhausted.
    let llm = Arc::new(LlmClient::new(
        "http://127.0.0.1:1".into(),
        "fake".into(),
        "test-model".into(),
        "test-model".into(),
    ));

    let shutdown = CancellationToken::new();
    let alive = Arc::new(AtomicBool::new(false));

    let listener_task = tokio::spawn(open_pincery::background::listener::start_listener(
        pool.clone(),
        config,
        llm,
        shutdown.clone(),
        alive.clone(),
    ));

    // Wait until the listener has subscribed before publishing NOTIFY.
    for _ in 0..20 {
        if alive.load(Ordering::Relaxed) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    sqlx::query("SELECT pg_notify('agent_wake', $1::text)")
        .bind(a.id.to_string())
        .execute(&pool)
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(400)).await;

    let llm_call_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM llm_calls WHERE agent_id = $1")
            .bind(a.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(llm_call_count, 0, "no llm call should be recorded");

    let budget_events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM events
         WHERE agent_id = $1
           AND event_type = 'budget_exceeded'",
    )
    .bind(a.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        budget_events, 1,
        "exactly one budget_exceeded event expected"
    );

    let current = agent::get_agent(&pool, a.id).await.unwrap().unwrap();
    assert_eq!(current.status, "asleep");

    shutdown.cancel();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), listener_task).await;
}
