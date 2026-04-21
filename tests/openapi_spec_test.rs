//! AC-44 (v8): `/openapi.json` serves a valid OpenAPI 3.1 document.
//!
//! Slice 1a: smoke-level coverage. Slice 1b extends with full-route
//! coverage diff and the "every handler is annotated" lint.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

use open_pincery::api::{self, AppState};
use open_pincery::config::Config;

const TEST_VAULT_KEY_B64: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

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
        vault_key_b64: TEST_VAULT_KEY_B64.into(),
    }
}

async fn fetch_openapi_json() -> (StatusCode, Value) {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://noop:noop@127.0.0.1:1/noop")
        .expect("connect_lazy");
    let state = AppState::new(pool, test_config());
    let app = api::router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router responded");

    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).expect("/openapi.json returned non-JSON body");
    (status, body)
}

#[tokio::test]
async fn openapi_json_is_served() {
    // AC-44
    let (status, body) = fetch_openapi_json().await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_object());
}

#[tokio::test]
async fn openapi_declares_3_1_0() {
    // AC-44
    let (_, body) = fetch_openapi_json().await;
    let version = body
        .get("openapi")
        .and_then(Value::as_str)
        .expect("openapi field");
    assert_eq!(version, "3.1.0", "AC-44 requires 3.1.0");
}

#[tokio::test]
async fn openapi_has_info_title_and_version() {
    // AC-44
    let (_, body) = fetch_openapi_json().await;
    let info = body.get("info").expect("info");
    assert_eq!(
        info.get("title").and_then(Value::as_str),
        Some("Open Pincery API")
    );
    assert_eq!(
        info.get("version").and_then(Value::as_str),
        Some(env!("CARGO_PKG_VERSION"))
    );
}

#[tokio::test]
async fn openapi_declares_bearer_auth() {
    // AC-44
    let (_, body) = fetch_openapi_json().await;
    let s = body
        .pointer("/components/securitySchemes/bearerAuth")
        .expect("bearerAuth scheme");
    assert_eq!(s.get("type").and_then(Value::as_str), Some("http"));
    assert_eq!(s.get("scheme").and_then(Value::as_str), Some("bearer"));
}

#[tokio::test]
async fn openapi_includes_me_endpoint() {
    // AC-44 — slice 1a minimum; slice 1b extends to every route.
    let (_, body) = fetch_openapi_json().await;
    let paths = body.get("paths").expect("paths");
    assert!(paths.get("/api/me").is_some(), "/api/me must be annotated");
}
