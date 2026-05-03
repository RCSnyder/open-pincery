mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use open_pincery::api::{self, AppState};
use open_pincery::config::Config;

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
        schema_invalid_retry_cap: 3,
        tool_call_rate_limit_per_wake: 32,
        stale_wake_hours: 2,
        wake_summary_limit: 20,
        event_window_limit: 200,
        vault_key_b64: common::TEST_VAULT_KEY_B64.into(),
        sandbox: open_pincery::config::ResolvedSandboxMode::default(),
    }
}

/// AC-15: PATCH /api/agents/:id updates agent name and is_enabled.
#[tokio::test]
async fn test_update_agent() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool.clone(), test_config());
    let app = api::router(state);

    // Bootstrap to get auth token
    let req = Request::builder()
        .method("POST")
        .uri("/api/bootstrap")
        .header(header::AUTHORIZATION, "Bearer test-token")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"email":"mgmt@test.com","display_name":"Mgmt"}"#,
        ))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert!(resp.status().is_success());
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["session_token"].as_str().unwrap().to_string();

    // Create agent
    let req = Request::builder()
        .method("POST")
        .uri("/api/agents")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name":"test-agent"}"#))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let agent_id = json["id"].as_str().unwrap().to_string();

    // PATCH — rename agent
    let req = Request::builder()
        .method("PATCH")
        .uri(format!("/api/agents/{agent_id}"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name":"renamed-agent"}"#))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["name"].as_str().unwrap(), "renamed-agent");
    assert!(json["is_enabled"].as_bool().unwrap());

    // PATCH — disable agent
    let req = Request::builder()
        .method("PATCH")
        .uri(format!("/api/agents/{agent_id}"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"is_enabled":false}"#))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(!json["is_enabled"].as_bool().unwrap());
}

/// AC-15: DELETE /api/agents/:id soft-deletes (disables with reason "deleted").
#[tokio::test]
async fn test_delete_agent() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool.clone(), test_config());
    let app = api::router(state);

    // Bootstrap
    let req = Request::builder()
        .method("POST")
        .uri("/api/bootstrap")
        .header(header::AUTHORIZATION, "Bearer test-token")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"email":"del@test.com","display_name":"Del"}"#,
        ))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert!(resp.status().is_success());
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["session_token"].as_str().unwrap().to_string();

    // Create agent
    let req = Request::builder()
        .method("POST")
        .uri("/api/agents")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name":"doomed-agent"}"#))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let agent_id = json["id"].as_str().unwrap().to_string();

    // DELETE
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/agents/{agent_id}"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(!json["is_enabled"].as_bool().unwrap());
    assert_eq!(json["disabled_reason"].as_str().unwrap(), "deleted");
    // webhook_secret must not be exposed on non-create responses
    assert!(json.get("webhook_secret").is_none());
}
