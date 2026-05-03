mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use open_pincery::api::{self, AppState};
use open_pincery::config::Config;
use open_pincery::models::{agent, user, workspace};

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
        stale_wake_hours: 2,
        wake_summary_limit: 20,
        event_window_limit: 200,
        vault_key_b64: common::TEST_VAULT_KEY_B64.into(),
        sandbox: open_pincery::config::ResolvedSandboxMode::default(),
    }
}

/// AC-6: CRUD agents via HTTP API
#[tokio::test]
async fn test_api_crud_agents() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool.clone(), test_config());
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
    let state = AppState::new(pool, test_config());
    let app = api::router(state);

    let req = Request::builder()
        .method("GET")
        .uri("/api/agents")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_agent_routes_are_scoped_to_workspace() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool.clone(), test_config());
    let app = api::router(state);

    let allowed_user = user::create_local_admin(&pool, "allowed@test.com", "Allowed")
        .await
        .unwrap();
    let allowed_org =
        workspace::create_organization(&pool, "allowed-org", "allowed-org", allowed_user.id)
            .await
            .unwrap();
    let allowed_ws = workspace::create_workspace(
        &pool,
        allowed_org.id,
        "allowed-ws",
        "allowed-ws",
        allowed_user.id,
    )
    .await
    .unwrap();
    workspace::add_org_membership(&pool, allowed_org.id, allowed_user.id, "owner")
        .await
        .unwrap();
    workspace::add_workspace_membership(&pool, allowed_ws.id, allowed_user.id, "owner")
        .await
        .unwrap();

    let outsider_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, display_name, auth_provider, auth_subject)
         VALUES ($1, $2, $3, $4)
         RETURNING id",
    )
    .bind("outsider@test.com")
    .bind("Outsider")
    .bind("local_test")
    .bind("outsider")
    .fetch_one(&pool)
    .await
    .unwrap();
    let outsider_org =
        workspace::create_organization(&pool, "outsider-org", "outsider-org", outsider_id)
            .await
            .unwrap();
    let outsider_ws = workspace::create_workspace(
        &pool,
        outsider_org.id,
        "outsider-ws",
        "outsider-ws",
        outsider_id,
    )
    .await
    .unwrap();
    workspace::add_org_membership(&pool, outsider_org.id, outsider_id, "owner")
        .await
        .unwrap();
    workspace::add_workspace_membership(&pool, outsider_ws.id, outsider_id, "owner")
        .await
        .unwrap();

    let outsider_agent = agent::create_agent(&pool, "outsider-agent", outsider_ws.id, outsider_id)
        .await
        .unwrap();

    let token_hash = open_pincery::auth::hash_token("workspace-scope-token");
    user::create_session(&pool, allowed_user.id, &token_hash, "local_admin")
        .await
        .unwrap();

    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/agents/{}", outsider_agent.id))
        .header(header::AUTHORIZATION, "Bearer workspace-scope-token")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let req = Request::builder()
        .method("PATCH")
        .uri(format!("/api/agents/{}", outsider_agent.id))
        .header(header::AUTHORIZATION, "Bearer workspace-scope-token")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name":"renamed"}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/agents/{}/messages", outsider_agent.id))
        .header(header::AUTHORIZATION, "Bearer workspace-scope-token")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"content":"forbidden"}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/agents/{}/events", outsider_agent.id))
        .header(header::AUTHORIZATION, "Bearer workspace-scope-token")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/agents/{}", outsider_agent.id))
        .header(header::AUTHORIZATION, "Bearer workspace-scope-token")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE agent_id = $1")
        .bind(outsider_agent.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(event_count, 0);

    let still_enabled: bool = sqlx::query_scalar("SELECT is_enabled FROM agents WHERE id = $1")
        .bind(outsider_agent.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(still_enabled);
}
