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
        stale_wake_hours: 2,
        wake_summary_limit: 20,
        event_window_limit: 200,
    }
}

/// AC-6: CRUD agents via HTTP API
#[tokio::test]
async fn test_api_crud_agents() {
    let pool = common::test_pool().await;
    let state = AppState { pool: pool.clone(), config: test_config() };
    let app = api::router(state);

    // Bootstrap first
    let req = Request::builder()
        .method("POST")
        .uri("/api/bootstrap")
        .header(header::AUTHORIZATION, "Bearer test-token")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["session_token"].as_str().unwrap().to_string();

    // Create agent
    let req = Request::builder()
        .method("POST")
        .uri("/api/agents")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name": "test-agent"}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let agent: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let agent_id = agent["id"].as_str().unwrap();

    // List agents
    let req = Request::builder()
        .method("GET")
        .uri("/api/agents")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let agents: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(agents.len(), 1);

    // Get agent by ID
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/agents/{agent_id}"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Send message
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/agents/{agent_id}/messages"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"content": "Hello agent"}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Get events
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/agents/{agent_id}/events"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let events: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(events["events"].as_array().unwrap().len(), 1);
}

/// AC-6: Unauthenticated requests are rejected
#[tokio::test]
async fn test_api_requires_auth() {
    let pool = common::test_pool().await;
    let state = AppState { pool, config: test_config() };
    let app = api::router(state);

    let req = Request::builder()
        .method("GET")
        .uri("/api/agents")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
