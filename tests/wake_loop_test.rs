mod common;

use open_pincery::models::{agent, event, user, workspace};
use open_pincery::runtime::{llm::LlmClient, wake_loop};
use open_pincery::config::Config;
use wiremock::{Mock, MockServer, ResponseTemplate};
use wiremock::matchers::{method, path};

fn test_config(iteration_cap: i32) -> Config {
    Config {
        database_url: String::new(),
        host: "127.0.0.1".into(),
        port: 0,
        bootstrap_token: "test".into(),
        llm_api_base_url: String::new(), // set per test
        llm_api_key: "fake".into(),
        llm_model: "test-model".into(),
        llm_maintenance_model: "test-model".into(),
        max_prompt_chars: 100000,
        iteration_cap,
        stale_wake_hours: 2,
        wake_summary_limit: 20,
        event_window_limit: 200,
    }
}

/// AC-4: Wake loop calls LLM and terminates on sleep tool
#[tokio::test]
async fn test_wake_loop_sleep_termination() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "wake@test.com", "Wake").await.unwrap();
    let org = workspace::create_organization(&pool, "wake", "wake", u.id).await.unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "wake", "wake", u.id).await.unwrap();
    let a = agent::create_agent(&pool, "wake-agent", ws.id, u.id).await.unwrap();

    // Add a message
    event::append_event(
        &pool, a.id, "message_received", "human", None, None, None, None, Some("hello"), None,
    ).await.unwrap();

    // Acquire wake
    let acquired = agent::acquire_wake(&pool, a.id).await.unwrap().unwrap();
    let wake_id = acquired.wake_id.unwrap();

    // Mock LLM - respond with sleep tool call
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "test-1",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "sleep",
                            "arguments": "{}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 10,
                "total_tokens": 110
            }
        })))
        .mount(&mock_server)
        .await;

    let config = test_config(50);
    let llm = LlmClient::new(
        mock_server.uri(),
        "fake-key".into(),
        "test-model".into(),
        "test-model".into(),
    );

    let reason = wake_loop::run_wake_loop(&pool, &llm, &config, a.id, wake_id).await.unwrap();
    assert_eq!(reason, "sleep");

    // Verify wake_end event was recorded
    let events = event::recent_events(&pool, a.id, 100).await.unwrap();
    let wake_ends: Vec<_> = events.iter().filter(|e| e.event_type == "wake_end").collect();
    assert_eq!(wake_ends.len(), 1);
    assert_eq!(wake_ends[0].termination_reason.as_deref(), Some("sleep"));
}

/// AC-4: Wake loop respects iteration cap
#[tokio::test]
async fn test_wake_loop_iteration_cap() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "cap@test.com", "Cap").await.unwrap();
    let org = workspace::create_organization(&pool, "cap", "cap", u.id).await.unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "cap", "cap", u.id).await.unwrap();
    let a = agent::create_agent(&pool, "cap-agent", ws.id, u.id).await.unwrap();

    event::append_event(
        &pool, a.id, "message_received", "human", None, None, None, None, Some("hello"), None,
    ).await.unwrap();

    let acquired = agent::acquire_wake(&pool, a.id).await.unwrap().unwrap();
    let wake_id = acquired.wake_id.unwrap();

    // Set iteration count to cap
    for _ in 0..3 {
        agent::increment_iteration(&pool, a.id).await.unwrap();
    }

    let mock_server = MockServer::start().await;
    // LLM won't be called because iteration cap is hit immediately

    let config = test_config(3); // cap at 3
    let llm = LlmClient::new(
        mock_server.uri(),
        "fake-key".into(),
        "test-model".into(),
        "test-model".into(),
    );

    let reason = wake_loop::run_wake_loop(&pool, &llm, &config, a.id, wake_id).await.unwrap();
    assert_eq!(reason, "iteration_cap");
}
