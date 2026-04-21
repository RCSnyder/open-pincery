mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use open_pincery::api::{self, AppState};
use open_pincery::config::Config;

/// AC-10: Bootstrap creates admin user, org, workspace, and returns session token
#[tokio::test]
async fn test_bootstrap_flow() {
    let pool = common::test_pool().await;

    let config = Config {
        database_url: String::new(),
        host: "127.0.0.1".into(),
        port: 0,
        bootstrap_token: "test-token-123".into(),
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
    };

    let state = AppState::new(pool.clone(), config.clone());

    let app = api::router(state);

    // Bootstrap with correct token
    let req = Request::builder()
        .method("POST")
        .uri("/api/bootstrap")
        .header(header::AUTHORIZATION, "Bearer test-token-123")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("user_id").is_some());
    assert!(json.get("session_token").is_some());
    let session_token = json["session_token"].as_str().unwrap();

    // Use session token to create an agent
    let req = Request::builder()
        .method("POST")
        .uri("/api/agents")
        .header(header::AUTHORIZATION, format!("Bearer {session_token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name": "my-agent"}"#))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Double bootstrap should fail
    let req = Request::builder()
        .method("POST")
        .uri("/api/bootstrap")
        .header(header::AUTHORIZATION, "Bearer test-token-123")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

/// AC-10: Bootstrap with wrong token returns 401
#[tokio::test]
async fn test_bootstrap_wrong_token() {
    let pool = common::test_pool().await;

    let config = Config {
        database_url: String::new(),
        host: "127.0.0.1".into(),
        port: 0,
        bootstrap_token: "correct-token".into(),
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
    };

    let state = AppState::new(pool, config);
    let app = api::router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/api/bootstrap")
        .header(header::AUTHORIZATION, "Bearer wrong-token")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
