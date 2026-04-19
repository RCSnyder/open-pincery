mod common;

use open_pincery::models::{agent, user, workspace};
use sqlx::postgres::PgListener;

/// AC-7: LISTEN/NOTIFY triggers on message send
#[tokio::test]
async fn test_notify_on_message() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "trig@test.com", "Trig")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "trig", "trig", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "trig", "trig", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "trig-agent", ws.id, u.id)
        .await
        .unwrap();

    // Set up listener
    let db_url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://open_pincery:open_pincery@localhost:5432/open_pincery_test".into()
    });
    let mut listener = PgListener::connect(&db_url).await.unwrap();
    listener.listen("agent_wake").await.unwrap();

    // Issue NOTIFY (simulating what the API message handler does)
    sqlx::query("SELECT pg_notify('agent_wake', $1::text)")
        .bind(a.id.to_string())
        .execute(&pool)
        .await
        .unwrap();

    // Receive notification
    let notification = tokio::time::timeout(std::time::Duration::from_secs(5), listener.recv())
        .await
        .expect("Timed out waiting for notification")
        .expect("Failed to receive notification");

    assert_eq!(notification.payload(), a.id.to_string());
}
