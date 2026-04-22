mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use hmac::{Hmac, Mac};
use http_body_util::BodyExt;
use open_pincery::api::{self, AppState};
use open_pincery::config::Config;
use open_pincery::models::workspace;
use sha2::Sha256;
use tower::ServiceExt;

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
        sandbox: open_pincery::config::ResolvedSandboxMode::default(),
    }
}

fn sign(secret: &str, payload: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(payload);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

#[tokio::test]
async fn test_rotate_webhook_secret_invalidates_old_secret() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool.clone(), test_config());
    let app = api::router(state);

    // Bootstrap to get auth token.
    let req = Request::builder()
        .method("POST")
        .uri("/api/bootstrap")
        .header(header::AUTHORIZATION, "Bearer test-token")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"email":"rotate@test.com","display_name":"Rotate"}"#,
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["session_token"].as_str().unwrap().to_string();

    // Create agent and capture initial webhook secret.
    let req = Request::builder()
        .method("POST")
        .uri("/api/agents")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name":"rotate-agent"}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let agent_id = json["id"].as_str().unwrap().to_string();
    let old_secret = json["webhook_secret"].as_str().unwrap().to_string();

    // Rotate webhook secret.
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/agents/{agent_id}/webhook/rotate"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let new_secret = json["webhook_secret"].as_str().unwrap().to_string();
    assert_ne!(old_secret, new_secret);

    let payload = br#"{"content":"hello","source":"webhook-test"}"#;

    // Old secret should fail webhook auth.
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/agents/{agent_id}/webhooks"))
        .header("x-webhook-signature", sign(&old_secret, payload))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_vec()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // New secret should authenticate and be accepted.
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/agents/{agent_id}/webhooks"))
        .header("x-webhook-signature", sign(&new_secret, payload))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_vec()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM events
         WHERE agent_id = $1
           AND event_type = 'webhook_secret_rotated'",
    )
    .bind(agent_id.parse::<uuid::Uuid>().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_rotate_webhook_secret_is_forbidden_outside_workspace() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool.clone(), test_config());
    let app = api::router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/api/bootstrap")
        .header(header::AUTHORIZATION, "Bearer test-token")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"email":"owner@test.com","display_name":"Owner"}"#,
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let owner_token = json["session_token"].as_str().unwrap().to_string();

    let req = Request::builder()
        .method("POST")
        .uri("/api/agents")
        .header(header::AUTHORIZATION, format!("Bearer {owner_token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name":"owner-agent"}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let agent_id = json["id"].as_str().unwrap().parse::<uuid::Uuid>().unwrap();
    let original_secret = json["webhook_secret"].as_str().unwrap().to_string();

    let outsider_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, display_name, auth_provider, auth_subject)
         VALUES ($1, $2, $3, $4)
         RETURNING id",
    )
    .bind("outsider-rotate@test.com")
    .bind("Outsider Rotate")
    .bind("local_test")
    .bind("outsider_rotate")
    .fetch_one(&pool)
    .await
    .unwrap();
    let outsider_org = workspace::create_organization(
        &pool,
        "outsider-rotate-org",
        "outsider-rotate-org",
        outsider_id,
    )
    .await
    .unwrap();
    let outsider_ws = workspace::create_workspace(
        &pool,
        outsider_org.id,
        "outsider-rotate-ws",
        "outsider-rotate-ws",
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
    let outsider_token_hash = open_pincery::auth::hash_token("outsider-rotate-token");
    open_pincery::models::user::create_session(
        &pool,
        outsider_id,
        &outsider_token_hash,
        "local_test",
    )
    .await
    .unwrap();

    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/agents/{agent_id}/webhook/rotate"))
        .header(header::AUTHORIZATION, "Bearer outsider-rotate-token")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let stored_secret: String =
        sqlx::query_scalar("SELECT webhook_secret FROM agents WHERE id = $1")
            .bind(agent_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored_secret, original_secret);

    let rotate_events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM events
         WHERE agent_id = $1
           AND event_type = 'webhook_secret_rotated'",
    )
    .bind(agent_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rotate_events, 0);
}
