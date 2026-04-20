mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use open_pincery::api::{self, AppState};
use open_pincery::config::Config;
use open_pincery::models::{agent, event, user, workspace};
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
    }
}

#[tokio::test]
async fn ui_static_assets_and_routes_are_present() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool, test_config());
    let app = api::router(state);

    let resp = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let html = String::from_utf8(
        resp.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("id=\"app\""));
    assert!(html.contains("/js/app.js"));

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/js/app.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let js = String::from_utf8(
        resp.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(js.contains("#/login"));
    assert!(js.contains("#/agents"));
    assert!(js.contains("since"));

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/js/api.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let api_js = String::from_utf8(
        resp.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(api_js.contains("/webhook/rotate"));
}

#[tokio::test]
async fn events_endpoint_supports_since_event_id() {
    let pool = common::test_pool().await;

    let admin = user::create_local_admin(&pool, "ui@test.com", "UI")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ui", "ui", admin.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ui", "ui", admin.id)
        .await
        .unwrap();
    workspace::add_org_membership(&pool, org.id, admin.id, "owner")
        .await
        .unwrap();
    workspace::add_workspace_membership(&pool, ws.id, admin.id, "owner")
        .await
        .unwrap();
    let ag = agent::create_agent(&pool, "ui-agent", ws.id, admin.id)
        .await
        .unwrap();

    let e1 = event::append_event(
        &pool,
        ag.id,
        "message_received",
        "human",
        None,
        None,
        None,
        None,
        Some("one"),
        None,
    )
    .await
    .unwrap();
    event::append_event(
        &pool,
        ag.id,
        "message_received",
        "human",
        None,
        None,
        None,
        None,
        Some("two"),
        None,
    )
    .await
    .unwrap();

    let state = AppState::new(pool.clone(), test_config());
    let app = api::router(state);
    let token_hash = open_pincery::auth::hash_token("manual-test-token");
    open_pincery::models::user::create_session(&pool, admin.id, &token_hash, "local_admin")
        .await
        .unwrap();

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/agents/{}/events?since={}&limit=100",
                    ag.id, e1.id
                ))
                .header(header::AUTHORIZATION, "Bearer manual-test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let events = json["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["content"], "two");
}
