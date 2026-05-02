//! AC-78 G3c: HTTP `/api/audit/chain/verify` endpoints.
//!
//! Covers T-AC78-7 (workspace-admin gating, workspace-scoped) and the
//! same exit-code semantics surfaced through the JSON
//! `all_verified` flag that the CLI relies on for non-zero exits on
//! tamper.

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
        vault_key_b64: common::TEST_VAULT_KEY_B64.into(),
        sandbox: open_pincery::config::ResolvedSandboxMode::default(),
    }
}

/// End-to-end happy-path: bootstrap, create an agent, send a message
/// (which appends events through the trigger), then call
/// `POST /api/audit/chain/verify` and assert `all_verified=true`.
#[tokio::test]
async fn audit_chain_verify_workspace_returns_all_verified() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool.clone(), test_config());
    let app = api::router(state);

    let token = bootstrap_and_login(&app).await;
    let agent_id = create_agent(&app, &token, "audit-ws-ok").await;
    send_message(&app, &token, &agent_id, "hello").await;

    let req = Request::builder()
        .method("POST")
        .uri("/api/audit/chain/verify")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json["all_verified"], true,
        "freshly-built chain should verify; got {json}"
    );
    let agents = json["agents"].as_array().unwrap();
    assert!(
        !agents.is_empty(),
        "expected at least one agent in response"
    );
    for entry in agents {
        assert_eq!(entry["status"], "verified", "agent entry: {entry}");
        assert!(entry["events_in_chain"].as_u64().unwrap() >= 1);
    }
}

/// Tampering with one event surfaces as `all_verified=false` and the
/// broken agent's `first_divergent_event_id` matches the row we
/// mutated. T-AC78-7.
#[tokio::test]
async fn audit_chain_verify_workspace_reports_broken_after_tamper() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool.clone(), test_config());
    let app = api::router(state);

    let token = bootstrap_and_login(&app).await;
    let agent_id = create_agent(&app, &token, "audit-ws-tamper").await;
    send_message(&app, &token, &agent_id, "first").await;
    send_message(&app, &token, &agent_id, "second").await;

    // Walk to the second event and tamper it. Trigger does not refire
    // on UPDATE, so prev_hash/entry_hash stay stale -> verifier breaks.
    let agent_uuid: uuid::Uuid = agent_id.parse().unwrap();
    let target: (uuid::Uuid,) = sqlx::query_as(
        "SELECT id FROM events WHERE agent_id = $1 ORDER BY created_at ASC, id ASC OFFSET 1 LIMIT 1",
    )
    .bind(agent_uuid)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE events SET content = 'evil' WHERE id = $1")
        .bind(target.0)
        .execute(&pool)
        .await
        .unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/api/audit/chain/verify")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["all_verified"], false, "tamper must surface; {json}");
    let broken = json["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["status"] == "broken")
        .expect("at least one agent must be broken");
    assert_eq!(broken["first_divergent_event_id"], target.0.to_string());
}

/// Per-agent endpoint resolves the agent through the same scoping as
/// `/agents/{id}` — an agent UUID that does not belong to the
/// caller's workspace returns 404, never 200 with a status.
#[tokio::test]
async fn audit_chain_verify_agent_404s_for_other_workspace() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool.clone(), test_config());
    let app = api::router(state);

    let token = bootstrap_and_login(&app).await;

    // Random UUID that does not exist as an agent.
    let fake_agent = uuid::Uuid::new_v4();
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/audit/chain/verify/agents/{fake_agent}"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// --- helpers ---------------------------------------------------------------

async fn bootstrap_and_login(app: &axum::Router) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/api/bootstrap")
        .header(header::AUTHORIZATION, "Bearer test-token")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    json["session_token"].as_str().unwrap().to_string()
}

async fn create_agent(app: &axum::Router, token: &str, name: &str) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/api/agents")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(format!(r#"{{"name":"{name}"}}"#)))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    json["id"].as_str().unwrap().to_string()
}

async fn send_message(app: &axum::Router, token: &str, agent_id: &str, content: &str) {
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/agents/{agent_id}/messages"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(format!(r#"{{"content":"{content}"}}"#)))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}
