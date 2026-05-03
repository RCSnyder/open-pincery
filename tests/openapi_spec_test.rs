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
        schema_invalid_retry_cap: 3,
        stale_wake_hours: 2,
        wake_summary_limit: 20,
        event_window_limit: 200,
        vault_key_b64: TEST_VAULT_KEY_B64.into(),
        sandbox: open_pincery::config::ResolvedSandboxMode::default(),
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

/// AC-44 / AC-52a — every public API path declared in `api::router()`
/// must appear in the OpenAPI document, modulo the operational /
/// spec-itself allowlist.
#[tokio::test]
async fn openapi_covers_every_public_route() {
    let (_, body) = fetch_openapi_json().await;
    let paths = body
        .get("paths")
        .and_then(Value::as_object)
        .expect("paths object");

    // The exhaustive public-API path list. Any new route added to
    // `src/api/*` must be appended here AND carry `#[utoipa::path]`.
    // Allowlist (intentionally NOT in OpenAPI): /health, /ready,
    // /openapi.json, /openapi.yaml, static file fallback.
    let expected: &[&str] = &[
        "/api/bootstrap",
        "/api/login",
        "/api/me",
        "/api/agents",
        "/api/agents/{id}",
        "/api/agents/{id}/webhook/rotate",
        "/api/agents/{id}/messages",
        "/api/agents/{id}/events",
        "/api/agents/{id}/webhooks",
        "/api/workspaces/{id}/credentials",
        "/api/workspaces/{id}/credentials/{name}",
    ];

    let mut missing: Vec<&str> = Vec::new();
    for p in expected {
        if !paths.contains_key(*p) {
            missing.push(p);
        }
    }
    assert!(
        missing.is_empty(),
        "OpenAPI missing expected paths: {missing:?}. Annotate the handler with #[utoipa::path] and register it in ApiDoc::paths(...)."
    );
}

/// AC-44 / AC-52a — grep-style lint: every `.route(...)` declared in
/// `src/api/*.rs` must live beside an `#[utoipa::path]` annotation in
/// the same file. This keeps the OpenAPI document from silently going
/// out of sync with the router at the source-code level, independent
/// of the aggregator's `paths(...)` list.
#[test]
fn every_api_route_handler_is_utoipa_annotated() {
    use std::fs;
    use std::path::Path;

    // Handlers that are intentionally not part of the public OpenAPI
    // contract (operational only, or the spec itself). `mod.rs` holds
    // only `/health` and `/ready`, which are operational by design.
    let allowlist_files: &[&str] = &["health.rs", "openapi.rs", "mod.rs"];

    let api_dir = Path::new("src/api");
    let entries = fs::read_dir(api_dir).expect("read src/api");

    let mut offenders: Vec<String> = Vec::new();

    for entry in entries {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !file_name.ends_with(".rs") {
            continue;
        }
        if allowlist_files.contains(&file_name) {
            continue;
        }
        let contents = fs::read_to_string(&path).expect("read api source");

        // Count `.route(` occurrences and `#[utoipa::path` occurrences.
        // A file with at least one route must have at least as many
        // utoipa path annotations as distinct route-registration calls.
        let route_count = contents.matches(".route(").count();
        if route_count == 0 {
            continue;
        }
        let utoipa_count = contents.matches("#[utoipa::path").count();
        if utoipa_count < route_count {
            offenders.push(format!(
                "{file_name}: {route_count} .route(...) calls but only {utoipa_count} #[utoipa::path] annotations"
            ));
        }
    }

    assert!(
        offenders.is_empty(),
        "AC-52a: every handler registered via `.route(...)` must carry `#[utoipa::path]`.\n{}",
        offenders.join("\n")
    );
}
