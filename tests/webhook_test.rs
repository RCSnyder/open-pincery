mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use hmac::{Hmac, Mac};
use sha2::Sha256;

use open_pincery::api::{self, AppState};
use open_pincery::config::Config;
use open_pincery::models::{agent, event, user, workspace};

fn test_config() -> Config {
    Config {
        database_url: String::new(),
        host: "127.0.0.1".into(),
        port: 0,
        bootstrap_token: "test-token".into(),
        llm_api_base_url: "http://localhost:9999".into(),
        llm_api_key: "fake".into(),
        llm_model: "test".into(),
        llm_maintenance_model: "test".into(),
        max_prompt_chars: 100000,
        iteration_cap: 50,
        stale_wake_hours: 2,
        wake_summary_limit: 20,
        event_window_limit: 200,
        vault_key_b64: common::TEST_VAULT_KEY_B64.into(),
    }
}

fn compute_hmac(secret: &str, payload: &[u8]) -> String {
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload);
    let result = mac.finalize();
    format!("sha256={}", hex::encode(result.into_bytes()))
}

/// AC-14: Valid signed webhook creates event and returns 202.
#[tokio::test]
async fn test_webhook_valid_signature() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "wh@test.com", "WH")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "wh", "wh", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "wh", "wh", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "wh-agent", ws.id, u.id)
        .await
        .unwrap();

    let state = AppState::new(pool.clone(), test_config());
    let app = api::router(state);

    let payload = r#"{"content":"hello from webhook"}"#;
    let signature = compute_hmac(&a.webhook_secret, payload.as_bytes());

    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/agents/{}/webhooks", a.id))
        .header("content-type", "application/json")
        .header("x-webhook-signature", &signature)
        .header("x-idempotency-key", "unique-key-1")
        .body(Body::from(payload))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Verify event was created
    let events = event::recent_events(&pool, a.id, 100).await.unwrap();
    assert!(events.iter().any(|e| e.event_type == "webhook_received"));
}

/// AC-14: Bad signature returns 401.
#[tokio::test]
async fn test_webhook_bad_signature() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "wh2@test.com", "WH2")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "wh2", "wh2", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "wh2", "wh2", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "wh-agent2", ws.id, u.id)
        .await
        .unwrap();

    let state = AppState::new(pool.clone(), test_config());
    let app = api::router(state);

    let payload = r#"{"content":"hello"}"#;

    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/agents/{}/webhooks", a.id))
        .header("content-type", "application/json")
        .header(
            "x-webhook-signature",
            "sha256=0000000000000000000000000000000000000000000000000000000000000000",
        )
        .header("x-idempotency-key", "key-bad")
        .body(Body::from(payload))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// AC-14: Duplicate idempotency key returns 200 without duplicate event.
#[tokio::test]
async fn test_webhook_idempotency_dedup() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "wh3@test.com", "WH3")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "wh3", "wh3", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "wh3", "wh3", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "wh-agent3", ws.id, u.id)
        .await
        .unwrap();

    let state = AppState::new(pool.clone(), test_config());
    let app = api::router(state);

    let payload = r#"{"content":"dedup test"}"#;
    let signature = compute_hmac(&a.webhook_secret, payload.as_bytes());
    let idem_key = "dedup-key-1";

    // First request — should be accepted
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/agents/{}/webhooks", a.id))
        .header("content-type", "application/json")
        .header("x-webhook-signature", &signature)
        .header("x-idempotency-key", idem_key)
        .body(Body::from(payload))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Second request with same key — should be 200 (duplicate)
    let req2 = Request::builder()
        .method("POST")
        .uri(format!("/api/agents/{}/webhooks", a.id))
        .header("content-type", "application/json")
        .header("x-webhook-signature", &signature)
        .header("x-idempotency-key", idem_key)
        .body(Body::from(payload))
        .unwrap();

    let resp2 = app.oneshot(req2).await.unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);

    // Verify only one event was created
    let events = event::recent_events(&pool, a.id, 100).await.unwrap();
    let webhook_events: Vec<_> = events
        .iter()
        .filter(|e| e.event_type == "webhook_received")
        .collect();
    assert_eq!(
        webhook_events.len(),
        1,
        "Should have exactly 1 webhook event, not duplicated"
    );
}
