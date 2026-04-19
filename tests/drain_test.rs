mod common;

use open_pincery::models::{agent, event, user, workspace};
use open_pincery::runtime::drain;

/// AC-9: Drain check detects new events and re-acquires
#[tokio::test]
async fn test_drain_reacquires_on_new_events() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "drain@test.com", "Drain")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "drain", "drain", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "drain", "drain", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "drain-agent", ws.id, u.id)
        .await
        .unwrap();

    // Acquire wake
    let acquired = agent::acquire_wake(&pool, a.id).await.unwrap().unwrap();
    let wake_started = acquired.wake_started_at.unwrap();

    // Transition to maintenance (drain check happens in maintenance)
    agent::transition_to_maintenance(&pool, a.id).await.unwrap();

    // Add a new event AFTER wake started
    event::append_event(
        &pool,
        a.id,
        "message_received",
        "human",
        None,
        None,
        None,
        None,
        Some("new msg"),
        None,
    )
    .await
    .unwrap();

    // Drain check should detect the new event and re-acquire
    let reacquired = drain::check_drain(&pool, a.id, wake_started).await.unwrap();
    assert!(reacquired);

    // Agent should be awake again
    let refreshed = agent::get_agent(&pool, a.id).await.unwrap().unwrap();
    assert_eq!(refreshed.status, "awake");
}

/// AC-9: Drain check with no new events releases to asleep
#[tokio::test]
async fn test_drain_releases_when_no_new_events() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "drain2@test.com", "Drain2")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "drain2", "drain2", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "drain2", "drain2", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "drain-agent-2", ws.id, u.id)
        .await
        .unwrap();

    // Acquire, transition to maintenance
    agent::acquire_wake(&pool, a.id).await.unwrap();
    agent::transition_to_maintenance(&pool, a.id).await.unwrap();

    // No new events — drain should release to asleep
    let reacquired = drain::check_drain(&pool, a.id, chrono::Utc::now())
        .await
        .unwrap();
    assert!(!reacquired);

    let refreshed = agent::get_agent(&pool, a.id).await.unwrap().unwrap();
    assert_eq!(refreshed.status, "asleep");
}
