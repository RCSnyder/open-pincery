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
        vault_key_b64: common::TEST_VAULT_KEY_B64.into(),
        sandbox: open_pincery::config::ResolvedSandboxMode::default(),
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

    // Every split module must load with 200 and import-resolve as ES modules.
    // Each assertion also confirms the known load-bearing contract for its
    // module — not an implementation detail — so innocuous refactors don't
    // break the suite.
    let module_contracts = [
        ("/js/app.js", "renderLogin"),
        ("/js/state.js", "pollEvents"),
        ("/js/ui.js", "currentRoute"),
        ("/js/views/login.js", "api.bootstrap"),
        ("/js/views/agents.js", "api.createAgent"),
        ("/js/views/detail.js", "api.sendMessage"),
        ("/js/views/settings.js", "api.rotateWebhookSecret"),
    ];
    for (path, contract) in module_contracts {
        let resp = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "{path} status");
        let body = String::from_utf8(
            resp.into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(
            body.contains(contract),
            "{path} missing contract {contract}"
        );
    }

    // api.js carries the webhook rotate endpoint — asserted explicitly because
    // AC-24 wire-up depends on this exact path.
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

#[tokio::test]
async fn event_stream_can_resume_without_duplicate_history() {
    let pool = common::test_pool().await;

    let admin = user::create_local_admin(&pool, "ui-stream@test.com", "UI Stream")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ui-stream", "ui-stream", admin.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ui-stream", "ui-stream", admin.id)
        .await
        .unwrap();
    workspace::add_org_membership(&pool, org.id, admin.id, "owner")
        .await
        .unwrap();
    workspace::add_workspace_membership(&pool, ws.id, admin.id, "owner")
        .await
        .unwrap();
    let ag = agent::create_agent(&pool, "ui-stream-agent", ws.id, admin.id)
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
    event::append_event(
        &pool,
        ag.id,
        "message_received",
        "human",
        None,
        None,
        None,
        None,
        Some("three"),
        None,
    )
    .await
    .unwrap();

    let state = AppState::new(pool.clone(), test_config());
    let app = api::router(state);
    let token_hash = open_pincery::auth::hash_token("stream-test-token");
    open_pincery::models::user::create_session(&pool, admin.id, &token_hash, "local_admin")
        .await
        .unwrap();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/agents/{}/events?limit=200", ag.id))
                .header(header::AUTHORIZATION, "Bearer stream-test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let initial = json["events"].as_array().unwrap();
    let mut ordered: Vec<serde_json::Value> = initial.to_vec();
    ordered.reverse();
    assert_eq!(ordered.len(), 3);
    assert_eq!(ordered[0]["content"], "one");
    assert_eq!(ordered[1]["content"], "two");
    assert_eq!(ordered[2]["content"], "three");

    let since_id = ordered[2]["id"].as_str().unwrap().to_string();

    event::append_event(
        &pool,
        ag.id,
        "message_received",
        "human",
        None,
        None,
        None,
        None,
        Some("four"),
        None,
    )
    .await
    .unwrap();

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/agents/{}/events?since={}&limit=200",
                    ag.id, since_id
                ))
                .header(header::AUTHORIZATION, "Bearer stream-test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let incremental = json["events"].as_array().unwrap();
    assert_eq!(incremental.len(), 1);
    assert_eq!(incremental[0]["content"], "four");

    ordered.extend(incremental.iter().cloned());
    let contents: Vec<&str> = ordered
        .iter()
        .map(|ev| ev["content"].as_str().unwrap())
        .collect();
    assert_eq!(contents, vec!["one", "two", "three", "four"]);
}
