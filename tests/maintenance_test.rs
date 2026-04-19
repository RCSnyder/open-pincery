mod common;

use open_pincery::models::{agent, event, projection, user, workspace};
use open_pincery::runtime::{llm::LlmClient, maintenance};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// AC-5: Maintenance updates projection after wake
#[tokio::test]
async fn test_maintenance_creates_projection() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "maint@test.com", "Maint")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "maint", "maint", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "maint", "maint", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "maint-agent", ws.id, u.id)
        .await
        .unwrap();

    // Add an event for context
    event::append_event(
        &pool,
        a.id,
        "message_received",
        "human",
        None,
        None,
        None,
        None,
        Some("do something"),
        None,
    )
    .await
    .unwrap();

    let acquired = agent::acquire_wake(&pool, a.id).await.unwrap().unwrap();
    let wake_id = acquired.wake_id.unwrap();

    // Mock LLM returning maintenance JSON
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "maint-1",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": serde_json::json!({
                        "identity": "I am a test agent",
                        "work_list": ["task 1", "task 2"],
                        "summary": "Processed a greeting"
                    }).to_string()
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 200,
                "completion_tokens": 50,
                "total_tokens": 250
            }
        })))
        .mount(&mock_server)
        .await;

    let llm = LlmClient::new(
        mock_server.uri(),
        "fake-key".into(),
        "test-model".into(),
        "test-model".into(),
    );

    maintenance::run_maintenance(&pool, &llm, a.id, wake_id)
        .await
        .unwrap();

    // Verify projection was created
    let proj = projection::latest_projection(&pool, a.id).await.unwrap();
    assert!(proj.is_some());
    let proj = proj.unwrap();
    assert_eq!(proj.identity, "I am a test agent");
    assert_eq!(proj.version, 1);

    // Verify wake summary was created
    let summaries = projection::recent_wake_summaries(&pool, a.id, 10)
        .await
        .unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].summary, "Processed a greeting");
}
