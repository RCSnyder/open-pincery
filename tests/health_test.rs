//! AC-19: health/readiness split.
//!
//! - `/health` stays 200 purely when the process is up (liveness).
//! - `/ready` returns 200 only when DB is reachable, migrations are
//!   applied, and BOTH background tasks (listener + stale recovery)
//!   have signalled alive; otherwise 503 with a `failing` field.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::atomic::Ordering;
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

async fn json_body(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn health_always_returns_200() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool, test_config());
    // Do NOT flip either alive flag — health must still be 200.
    let app = api::router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn ready_503_when_both_background_tasks_not_alive() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool, test_config());
    // Both flags stay false.
    let app = api::router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = json_body(resp).await;
    assert_eq!(body["status"], "not_ready");
    assert_eq!(body["failing"], "background_tasks");
}

#[tokio::test]
async fn ready_503_when_only_listener_down() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool, test_config());
    state.stale_alive.store(true, Ordering::Relaxed);
    // listener_alive stays false.
    let app = api::router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = json_body(resp).await;
    assert_eq!(body["failing"], "background_task:listener");
}

#[tokio::test]
async fn ready_503_when_only_stale_down() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool, test_config());
    state.listener_alive.store(true, Ordering::Relaxed);
    // stale_alive stays false.
    let app = api::router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = json_body(resp).await;
    assert_eq!(body["failing"], "background_task:stale_recovery");
}

#[tokio::test]
async fn ready_200_when_db_migrations_and_both_alive() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool, test_config());
    state.listener_alive.store(true, Ordering::Relaxed);
    state.stale_alive.store(true, Ordering::Relaxed);
    let app = api::router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["status"], "ready");
}

#[tokio::test]
async fn ready_503_when_db_unreachable() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool.clone(), test_config());
    state.listener_alive.store(true, Ordering::Relaxed);
    state.stale_alive.store(true, Ordering::Relaxed);
    pool.close().await;
    let app = api::router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = json_body(resp).await;
    assert_eq!(body["failing"], "database");
}
