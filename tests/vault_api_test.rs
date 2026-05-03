//! AC-39 (v7): credentials REST API — role gating, name validation,
//! value secrecy, duplicate conflict, revoke + audit trail.

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
        stale_wake_hours: 2,
        wake_summary_limit: 20,
        event_window_limit: 200,
        vault_key_b64: common::TEST_VAULT_KEY_B64.into(),
        sandbox: open_pincery::config::ResolvedSandboxMode::default(),
    }
}

async fn bootstrap_admin(app: &axum::Router) -> (String, String) {
    let req = Request::builder()
        .method("POST")
        .uri("/api/bootstrap")
        .header(header::AUTHORIZATION, "Bearer test-token")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"email":"admin@test.local","display_name":"Admin"}"#,
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert!(
        resp.status().is_success(),
        "bootstrap failed: {}",
        resp.status()
    );
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["session_token"].as_str().unwrap().to_string();
    let ws_id = json["workspace_id"].as_str().unwrap().to_string();
    (token, ws_id)
}

#[tokio::test]
async fn ac39_create_list_revoke_lifecycle() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool.clone(), test_config());
    let app = api::router(state);
    let (token, ws_id) = bootstrap_admin(&app).await;

    // POST create
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/workspaces/{ws_id}/credentials"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"name":"stripe_test","value":"sk_test_verysecret"}"#,
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let body_text = std::str::from_utf8(&body).unwrap();
    // Critical: response must NOT echo the secret or expose sealed bytes.
    assert!(!body_text.contains("sk_test_verysecret"));
    assert!(!body_text.contains("ciphertext"));
    assert!(!body_text.contains("nonce"));

    // GET list — includes the name, excludes the value/ciphertext/nonce.
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/workspaces/{ws_id}/credentials"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = list.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"].as_str().unwrap(), "stripe_test");
    assert!(arr[0]["value"].is_null());
    assert!(arr[0]["ciphertext"].is_null());
    let list_text = std::str::from_utf8(&body).unwrap();
    assert!(!list_text.contains("sk_test_verysecret"));

    // Duplicate name while active → 409
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/workspaces/{ws_id}/credentials"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name":"stripe_test","value":"different"}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // DELETE revoke
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/workspaces/{ws_id}/credentials/stripe_test"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // List now empty
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/workspaces/{ws_id}/credentials"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 0);

    // After revoke the name slot is free — re-create succeeds.
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/workspaces/{ws_id}/credentials"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name":"stripe_test","value":"new-secret"}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Audit trail: credential_added x2 + credential_revoked x1 for this workspace.
    let ws_uuid: uuid::Uuid = ws_id.parse().unwrap();
    let added: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM auth_audit
         WHERE workspace_id = $1 AND event_type = 'credential_added'",
    )
    .bind(ws_uuid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(added.0, 2);
    let revoked: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM auth_audit
         WHERE workspace_id = $1 AND event_type = 'credential_revoked'",
    )
    .bind(ws_uuid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(revoked.0, 1);
}

#[tokio::test]
async fn ac39_invalid_names_and_values_rejected() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool.clone(), test_config());
    let app = api::router(state);
    let (token, ws_id) = bootstrap_admin(&app).await;

    // Uppercase rejected.
    for bad_name in [
        r#"{"name":"STRIPE","value":"x"}"#,
        r#"{"name":"has-dash","value":"x"}"#,
        r#"{"name":"has.dot","value":"x"}"#,
        r#"{"name":"has space","value":"x"}"#,
        r#"{"name":"","value":"x"}"#,
    ] {
        let req = Request::builder()
            .method("POST")
            .uri(format!("/api/workspaces/{ws_id}/credentials"))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(bad_name))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "name payload must be rejected: {bad_name}"
        );
    }

    // Empty value rejected.
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/workspaces/{ws_id}/credentials"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name":"empty","value":""}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Oversize (>8192) rejected.
    let big = "x".repeat(8193);
    let body = format!(r#"{{"name":"big","value":"{big}"}}"#);
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/workspaces/{ws_id}/credentials"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn ac39_non_admin_gets_403_with_audit() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool.clone(), test_config());
    let app = api::router(state);
    let (token, ws_id) = bootstrap_admin(&app).await;

    // Bootstrapped user is workspace owner. Demote them to a non-admin
    // role and verify the gate denies + audits.
    sqlx::query("UPDATE workspace_memberships SET role = 'workspace_reader'")
        .execute(&pool)
        .await
        .unwrap();

    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/workspaces/{ws_id}/credentials"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name":"forbid","value":"x"}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Audit row recorded.
    let forbidden: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM auth_audit WHERE event_type = 'credential_forbidden'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(forbidden.0 >= 1);

    // And no credentials leaked into the table.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM credentials")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
}

#[tokio::test]
async fn ac39_cross_workspace_access_is_404() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool.clone(), test_config());
    let app = api::router(state);
    let (token, _ws_id) = bootstrap_admin(&app).await;

    // Use a random UUID that is not the caller's workspace.
    let other = uuid::Uuid::new_v4();
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/workspaces/{other}/credentials"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name":"cross","value":"x"}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
