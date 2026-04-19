mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
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

/// AC-13: Unauthenticated endpoints return 429 after 10 requests/min from one IP.
#[tokio::test]
async fn test_unauth_rate_limit() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool, test_config());
    let app = api::router(state);

    // The bootstrap endpoint is rate-limited at 10/min for unauthenticated.
    // Send 11 requests; the 11th should get 429.
    for i in 0..11 {
        let req = Request::builder()
            .method("POST")
            .uri("/api/bootstrap")
            .header("content-type", "application/json")
            .header("authorization", "Bearer wrong-token")
            .body(Body::from(r#"{"email":"test@test.com","display_name":"Test"}"#))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();

        if i < 10 {
            // First 10 should NOT be 429 (they'll be 401 because wrong token, but not rate-limited)
            assert_ne!(
                resp.status(),
                StatusCode::TOO_MANY_REQUESTS,
                "Request {i} should not be rate-limited"
            );
        } else {
            // 11th should be 429
            assert_eq!(
                resp.status(),
                StatusCode::TOO_MANY_REQUESTS,
                "Request {i} should be rate-limited"
            );
        }
    }
}
