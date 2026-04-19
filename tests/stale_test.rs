mod common;

use open_pincery::models::{agent, user, workspace};

/// AC-8: Stale agents are detected and recovered
#[tokio::test]
async fn test_stale_agent_detection_and_recovery() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "stale@test.com", "Stale")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "stale", "stale", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "stale", "stale", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "stale-agent", ws.id, u.id)
        .await
        .unwrap();

    // Acquire wake
    agent::acquire_wake(&pool, a.id).await.unwrap();

    // Manually backdate the wake_started_at to simulate staleness
    sqlx::query("UPDATE agents SET wake_started_at = NOW() - INTERVAL '3 hours' WHERE id = $1")
        .bind(a.id)
        .execute(&pool)
        .await
        .unwrap();

    // Find stale agents (threshold: 2 hours)
    let stale = agent::find_stale_agents(&pool, 2).await.unwrap();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].id, a.id);

    // Force release
    agent::force_release(&pool, a.id).await.unwrap();

    let refreshed = agent::get_agent(&pool, a.id).await.unwrap().unwrap();
    assert_eq!(refreshed.status, "asleep");
    assert!(refreshed.wake_id.is_none());

    // Should no longer be stale
    let stale = agent::find_stale_agents(&pool, 2).await.unwrap();
    assert!(stale.is_empty());
}
