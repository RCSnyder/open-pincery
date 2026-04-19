mod common;

use open_pincery::models::event;
use open_pincery::models::user;
use open_pincery::models::workspace;
use open_pincery::models::agent;

/// AC-2: Events are append-only and queryable
#[tokio::test]
async fn test_event_log_append_and_query() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "evtest@test.com", "EvTest").await.unwrap();
    let org = workspace::create_organization(&pool, "evtest", "evtest", u.id).await.unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "evtest", "evtest", u.id).await.unwrap();
    let a = agent::create_agent(&pool, "ev-agent", ws.id, u.id).await.unwrap();

    // Append events
    let e1 = event::append_event(
        &pool, a.id, "message_received", "human", None, None, None, None, Some("hello"), None,
    ).await.unwrap();

    let e2 = event::append_event(
        &pool, a.id, "message_received", "human", None, None, None, None, Some("world"), None,
    ).await.unwrap();

    assert_ne!(e1.id, e2.id);

    // Query recent events (DESC order)
    let events = event::recent_events(&pool, a.id, 10).await.unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].content.as_deref(), Some("world")); // most recent first

    // has_pending_events
    let has = event::has_pending_events(&pool, a.id, e1.created_at).await.unwrap();
    assert!(has); // e2 is after e1
}
