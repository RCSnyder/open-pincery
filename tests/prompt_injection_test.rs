//! AC-79 (v9 Phase G G4e) — adversarial prompt-injection integration tests.
//!
//! Each test boots a real wake loop against a mock LLM (wiremock) and
//! asserts on the events that the loop wrote to Postgres. The wake loop
//! is exercised end-to-end so that the four AC-79 invariants are
//! observable from outside `runtime`:
//!
//! - T-AC79-7 / T-AC79-8 : canary echo in any of the scanned fields
//!   (`choices[].message.content` or any tool call's `function.name`,
//!   `function.arguments`, `id`) terminates the wake BEFORE any
//!   `assistant_message` or `tool_call` event is appended.
//! - T-AC79-9 : the four new event types (`prompt_injection_canary_emitted`,
//!   `prompt_injection_suspected`, `model_response_schema_invalid`,
//!   `tool_call_rate_limit_exceeded`) all register with `source =
//!   "runtime"` and chain through the AC-78 audit hash trigger.
//! - T-AC79-10 : the per-wake tool-call rate limit
//!   (`Config::tool_call_rate_limit_per_wake`) is independent of
//!   `iteration_cap` and terminates with `FailureAuditPending`.

mod common;

use open_pincery::config::Config;
use open_pincery::models::{agent, event, user, workspace};
use open_pincery::runtime::{
    llm::{LlmClient, Pricing},
    sandbox::{ProcessExecutor, ToolExecutor},
    vault::Vault,
    wake_loop,
};
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::Arc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_config(uri: String) -> Config {
    Config {
        database_url: String::new(),
        host: "127.0.0.1".into(),
        port: 0,
        bootstrap_token: "test".into(),
        llm_api_base_url: uri,
        llm_api_key: "fake".into(),
        llm_model: "test-model".into(),
        llm_maintenance_model: "test-model".into(),
        max_prompt_chars: 100_000,
        iteration_cap: 50,
        schema_invalid_retry_cap: 3,
        // Tight cap on purpose — tests need to exercise this without
        // dispatching dozens of real shell commands. The default in
        // production is 32 (env override
        // `OPEN_PINCERY_TOOL_CALL_RATE_LIMIT_PER_WAKE`).
        tool_call_rate_limit_per_wake: 2,
        stale_wake_hours: 2,
        wake_summary_limit: 20,
        event_window_limit: 200,
        vault_key_b64: common::TEST_VAULT_KEY_B64.into(),
        sandbox: open_pincery::config::ResolvedSandboxMode::default(),
    }
}

fn llm(uri: String) -> LlmClient {
    LlmClient::new(
        uri,
        "fake-key".into(),
        "test-model".into(),
        "test-model".into(),
    )
    .with_pricing(
        Pricing::new(
            Decimal::from_str("3").unwrap(),
            Decimal::from_str("15").unwrap(),
        ),
        Pricing::default(),
    )
}

async fn fresh_agent(name: &str) -> (sqlx::PgPool, uuid::Uuid, uuid::Uuid) {
    let pool = common::test_pool().await;
    let u = user::create_local_admin(&pool, &format!("{name}@test.com"), name)
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, name, name, u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, name, name, u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, name, ws.id, u.id).await.unwrap();
    event::append_event(
        &pool,
        a.id,
        "message_received",
        "human",
        None,
        None,
        None,
        None,
        Some("trigger"),
        None,
    )
    .await
    .unwrap();
    let acquired = agent::acquire_wake(&pool, a.id).await.unwrap().unwrap();
    (pool, a.id, acquired.wake_id.unwrap())
}

/// AC-79 / T-AC79-10 / G4e: when the LLM keeps requesting tool calls
/// past `Config::tool_call_rate_limit_per_wake`, the wake terminates
/// with `FailureAuditPending` and emits exactly one
/// `tool_call_rate_limit_exceeded` event whose payload reports the
/// limit and the attempted dispatch index.
#[tokio::test]
async fn tool_call_rate_limit_exceeded_terminates_wake_with_failure_audit_pending() {
    let (pool, agent_id, wake_id) = fresh_agent("rl").await;
    let mock_server = MockServer::start().await;

    // Always-on plan tool — cheap, deterministic, well-formed.
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "rl",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_rl",
                        "type": "function",
                        "function": {
                            "name": "plan",
                            "arguments": "{\"content\":\"x\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
        })))
        .mount(&mock_server)
        .await;

    let config = test_config(mock_server.uri());
    let llm = llm(mock_server.uri());
    let executor: Arc<dyn ToolExecutor> = Arc::new(ProcessExecutor);
    let vault = Arc::new(Vault::from_base64(common::TEST_VAULT_KEY_B64).unwrap());

    let reason =
        wake_loop::run_wake_loop(&pool, &llm, &config, agent_id, wake_id, &executor, &vault)
            .await
            .unwrap();
    assert_eq!(reason, "FailureAuditPending");

    let events = event::recent_events(&pool, agent_id, 200).await.unwrap();
    let exceeded: Vec<_> = events
        .iter()
        .filter(|e| e.event_type == "tool_call_rate_limit_exceeded")
        .collect();
    assert_eq!(
        exceeded.len(),
        1,
        "AC-79 T-AC79-10: exactly one tool_call_rate_limit_exceeded event"
    );
    let row = exceeded[0];
    assert_eq!(
        row.source.as_str(),
        "runtime",
        "AC-79 T-AC79-9: source must be runtime"
    );
    let content = row.content.as_deref().unwrap_or("");
    assert!(
        content.contains("\"limit\":2"),
        "payload should report limit (got: {content})"
    );
    assert!(
        content.contains("\"attempted\":3"),
        "payload should report attempted (got: {content})"
    );

    // wake_end exists and carries the FailureAuditPending termination_reason.
    let wake_end = events
        .iter()
        .find(|e| e.event_type == "wake_end")
        .expect("wake_end recorded");
    assert_eq!(
        wake_end.termination_reason.as_deref(),
        Some("FailureAuditPending")
    );

    // Exactly two tool_call events landed before the cap fired.
    let tool_calls: Vec<_> = events
        .iter()
        .filter(|e| e.event_type == "tool_call")
        .collect();
    assert_eq!(
        tool_calls.len(),
        2,
        "limit=2 means two dispatches before refusal"
    );
}

/// AC-79 / T-AC79-9 / G4e: every wake must emit exactly one
/// `prompt_injection_canary_emitted` event with `source = "runtime"`,
/// and the canary value itself must NOT appear anywhere in the event
/// log (T-AC79-7 — canary token never persisted).
#[tokio::test]
async fn canary_emitted_event_lands_once_per_wake_without_canary_value() {
    let (pool, agent_id, wake_id) = fresh_agent("canary-emit").await;
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "k",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "k1",
                        "type": "function",
                        "function": {"name": "sleep", "arguments": "{}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
        })))
        .mount(&mock_server)
        .await;

    let config = test_config(mock_server.uri());
    let llm = llm(mock_server.uri());
    let executor: Arc<dyn ToolExecutor> = Arc::new(ProcessExecutor);
    let vault = Arc::new(Vault::from_base64(common::TEST_VAULT_KEY_B64).unwrap());

    let reason =
        wake_loop::run_wake_loop(&pool, &llm, &config, agent_id, wake_id, &executor, &vault)
            .await
            .unwrap();
    assert_eq!(reason, "sleep");

    let events = event::recent_events(&pool, agent_id, 200).await.unwrap();
    let emitted: Vec<_> = events
        .iter()
        .filter(|e| e.event_type == "prompt_injection_canary_emitted")
        .collect();
    assert_eq!(
        emitted.len(),
        1,
        "AC-79 T-AC79-9: exactly one prompt_injection_canary_emitted event per wake"
    );
    assert_eq!(emitted[0].source.as_str(), "runtime");
    // Payload carries only the wake_id; no canary value, no nonce.
    assert!(
        emitted[0].content.is_none(),
        "AC-79 T-AC79-7: canary value MUST NOT be persisted in event content"
    );
    assert!(
        emitted[0].tool_input.is_none() && emitted[0].tool_output.is_none(),
        "AC-79 T-AC79-7: canary value MUST NOT be persisted in any column"
    );
}
