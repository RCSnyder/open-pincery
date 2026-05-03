mod common;

use open_pincery::models::{agent, user, workspace};
use open_pincery::runtime::prompt::{self, WakePromptContext};

/// AC-3: Prompt assembly produces system prompt + messages + tools
#[tokio::test]
async fn test_prompt_assembly() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "prompt@test.com", "Prompt")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "prompt", "prompt", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "prompt", "prompt", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "prompt-agent", ws.id, u.id)
        .await
        .unwrap();

    // Add a message event
    open_pincery::models::event::append_event(
        &pool,
        a.id,
        "message_received",
        "human",
        None,
        None,
        None,
        None,
        Some("What's up?"),
        None,
    )
    .await
    .unwrap();

    let ctx = WakePromptContext {
        wake_nonce: "deadbeefcafebabe1122334455667788".into(),
        canary_hex: "0011223344556677aabbccddeeff0011".into(),
    };
    let assembled = prompt::assemble_prompt(&pool, a.id, 200, 20, 100000, &ctx)
        .await
        .unwrap();

    assert!(!assembled.system_prompt.is_empty());
    assert!(assembled.system_prompt.contains("You are an AI agent"));
    assert!(!assembled.messages.is_empty());
    assert!(!assembled.tools.is_empty());
    assert!(assembled.tools.iter().any(|t| t.function.name == "shell"));
    assert!(assembled.tools.iter().any(|t| t.function.name == "sleep"));

    // AC-79 T-AC79-1: canary embedded in system prompt; untrusted message
    // wrapped with per-wake nonce delimiters.
    assert!(
        assembled
            .system_prompt
            .contains("<<canary:0011223344556677aabbccddeeff0011>>"),
        "canary must be present in assembled system_prompt"
    );
    let user_msg = assembled
        .messages
        .iter()
        .find(|m| m.role == "user")
        .expect("user message present");
    let content = user_msg.content.as_deref().unwrap_or("");
    assert!(
        content.contains("<<untrusted:deadbeefcafebabe1122334455667788>>")
            && content.contains("<<end:deadbeefcafebabe1122334455667788>>")
            && content.contains("What's up?"),
        "message_received content must be wrapped in <<untrusted:NONCE>>...<<end:NONCE>>; got {content:?}"
    );
}
