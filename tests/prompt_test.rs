mod common;

use open_pincery::models::{agent, user, workspace};
use open_pincery::runtime::prompt;

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

    let assembled = prompt::assemble_prompt(&pool, a.id, 200, 20, 100000)
        .await
        .unwrap();

    assert!(!assembled.system_prompt.is_empty());
    assert!(assembled.system_prompt.contains("You are an AI agent"));
    assert!(!assembled.messages.is_empty());
    assert!(!assembled.tools.is_empty());
    assert!(assembled.tools.iter().any(|t| t.function.name == "shell"));
    assert!(assembled.tools.iter().any(|t| t.function.name == "sleep"));
}
