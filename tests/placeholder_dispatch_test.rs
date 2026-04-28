//! AC-43 (v7): `PLACEHOLDER:<name>` env resolution in the shell tool.
//!
//! The dispatch layer is the ONLY place that sees plaintext. These
//! tests pin the four guarantees from `scaffolding/design.md` v7:
//!
//! 1. An existing credential is unsealed and injected into the child
//!    process environment under the requested key.
//! 2. A missing credential emits a `credential_unresolved` event
//!    (name + reason only — no value) and returns a tool error.
//!    The executor is NEVER invoked.
//! 3. A revoked credential behaves identically to a missing one —
//!    same closed-fail path, same reason.
//! 4. Neither the plaintext value nor the resolution failure reason
//!    ever appears in `tool_capability_denied` events or in the
//!    `ToolResult` error string beyond the literal credential name.

mod common;

use async_trait::async_trait;
use open_pincery::models::{agent, credential, event, user, workspace};
use open_pincery::runtime::capability::PermissionMode;
use open_pincery::runtime::llm::{FunctionCall, ToolCallRequest};
use open_pincery::runtime::sandbox::{ExecResult, SandboxProfile, ShellCommand, ToolExecutor};
use open_pincery::runtime::tools::{self, ToolResult};
use open_pincery::runtime::vault::Vault;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// A mock executor that records every `ShellCommand` it receives so we
/// can assert on the exact env map that would have been passed to the
/// child. Never actually spawns anything.
#[derive(Clone, Default)]
struct RecordingExecutor {
    calls: Arc<Mutex<Vec<ShellCommand>>>,
}

#[async_trait]
impl ToolExecutor for RecordingExecutor {
    async fn run(&self, cmd: &ShellCommand, _profile: &SandboxProfile) -> ExecResult {
        self.calls.lock().await.push(cmd.clone());
        ExecResult::Ok {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
            audit_pids: Vec::new(),
        }
    }
}

async fn dispatch_shell_with_env(
    pool: &sqlx::PgPool,
    vault: &Arc<Vault>,
    exec: &RecordingExecutor,
    agent_id: Uuid,
    wake_id: Uuid,
    workspace_id: Uuid,
    env: serde_json::Value,
) -> ToolResult {
    let executor: Arc<dyn ToolExecutor> = Arc::new(exec.clone());
    let args = serde_json::json!({ "command": "echo hello", "env": env }).to_string();
    let tc = ToolCallRequest {
        id: "call-placeholder".into(),
        call_type: "function".into(),
        function: FunctionCall {
            name: "shell".into(),
            arguments: args,
        },
    };
    tools::dispatch_tool(
        &tc,
        PermissionMode::Yolo, // shell requires ExecuteLocal
        pool,
        agent_id,
        workspace_id,
        wake_id,
        &executor,
        vault,
    )
    .await
}

#[tokio::test]
async fn ac43_existing_placeholder_is_resolved_into_child_env() {
    let pool = common::test_pool().await;
    let vault = Arc::new(Vault::from_base64(common::TEST_VAULT_KEY_B64).unwrap());

    let u = user::create_local_admin(&pool, "alice@test.local", "Alice")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "org", "org", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "w", "w", u.id)
        .await
        .unwrap();

    let secret_value = b"sk_live_abc123_super_secret";
    let sealed = vault.seal(ws.id, "stripe_key", secret_value).unwrap();
    credential::create(
        &pool,
        ws.id,
        "stripe_key",
        &sealed.ciphertext,
        &sealed.nonce,
        u.id,
    )
    .await
    .unwrap();

    let exec = RecordingExecutor::default();
    let agent_row = agent::create_agent(&pool, "ac43-ok", ws.id, u.id)
        .await
        .unwrap();
    let agent_id = agent_row.id;
    let wake_id = Uuid::new_v4();
    let result = dispatch_shell_with_env(
        &pool,
        &vault,
        &exec,
        agent_id,
        wake_id,
        ws.id,
        serde_json::json!({
            "STRIPE_KEY": "PLACEHOLDER:stripe_key",
            "PATH_THROUGH": "literal_value",
        }),
    )
    .await;

    match &result {
        ToolResult::Output(_) => {}
        other => {
            let msg = match other {
                ToolResult::Error(s) => s.as_str(),
                ToolResult::Sleep => "sleep",
                _ => unreachable!(),
            };
            panic!("expected Output, got: {msg}");
        }
    }

    // Exactly one call recorded, env contains resolved plaintext.
    let calls = exec.calls.lock().await.clone();
    assert_eq!(calls.len(), 1, "executor should have been called once");
    let observed = &calls[0].env;
    assert_eq!(
        observed.get("STRIPE_KEY").map(String::as_str),
        Some("sk_live_abc123_super_secret"),
        "resolved plaintext did not reach child env; got {observed:?}"
    );
    assert_eq!(
        observed.get("PATH_THROUGH").map(String::as_str),
        Some("literal_value"),
        "non-PLACEHOLDER values must pass through unchanged; got {observed:?}"
    );

    // No `credential_unresolved` event on success.
    let n: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM events
         WHERE agent_id = $1 AND event_type = 'credential_unresolved'",
    )
    .bind(agent_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n.0, 0, "must not emit credential_unresolved on success");
}

#[tokio::test]
async fn ac43_missing_credential_fails_closed_and_emits_event() {
    let pool = common::test_pool().await;
    let vault = Arc::new(Vault::from_base64(common::TEST_VAULT_KEY_B64).unwrap());

    let u = user::create_local_admin(&pool, "bob@test.local", "Bob")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "org", "org", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "w", "w", u.id)
        .await
        .unwrap();

    let exec = RecordingExecutor::default();
    let agent_row = agent::create_agent(&pool, "ac43-missing", ws.id, u.id)
        .await
        .unwrap();
    let agent_id = agent_row.id;
    let wake_id = Uuid::new_v4();
    let result = dispatch_shell_with_env(
        &pool,
        &vault,
        &exec,
        agent_id,
        wake_id,
        ws.id,
        serde_json::json!({ "API_KEY": "PLACEHOLDER:nonexistent" }),
    )
    .await;

    let msg = match result {
        ToolResult::Error(s) => s,
        ToolResult::Output(s) => panic!("expected Error, got Output: {s}"),
        ToolResult::Sleep => panic!("expected Error, got Sleep"),
    };
    assert!(
        msg.contains("nonexistent"),
        "error should name the missing credential; got {msg:?}"
    );
    assert!(
        msg.contains("credential not found"),
        "error should identify the cause; got {msg:?}"
    );

    // Executor never invoked — closed-fail happens BEFORE spawn.
    assert!(
        exec.calls.lock().await.is_empty(),
        "executor must NOT be invoked when a placeholder cannot be resolved"
    );

    // One credential_unresolved event with reason=missing_or_revoked.
    let rows: Vec<event::Event> = sqlx::query_as(
        "SELECT * FROM events
         WHERE agent_id = $1 AND event_type = 'credential_unresolved'
         ORDER BY created_at",
    )
    .bind(agent_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "exactly one credential_unresolved event expected; got {rows:?}"
    );
    let payload = rows[0]
        .tool_input
        .as_ref()
        .expect("event must carry the payload");
    assert!(payload.contains("nonexistent"));
    assert!(payload.contains("missing_or_revoked"));
}

#[tokio::test]
async fn ac43_revoked_credential_is_treated_as_missing() {
    let pool = common::test_pool().await;
    let vault = Arc::new(Vault::from_base64(common::TEST_VAULT_KEY_B64).unwrap());

    let u = user::create_local_admin(&pool, "carol@test.local", "Carol")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "org", "org", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "w", "w", u.id)
        .await
        .unwrap();

    // Create, then revoke — find_active must return None.
    let sealed = vault.seal(ws.id, "tombstoned", b"old_value").unwrap();
    credential::create(
        &pool,
        ws.id,
        "tombstoned",
        &sealed.ciphertext,
        &sealed.nonce,
        u.id,
    )
    .await
    .unwrap();
    let revoked = credential::revoke(&pool, ws.id, "tombstoned")
        .await
        .unwrap();
    assert!(revoked, "revoke() must report one affected row");

    let exec = RecordingExecutor::default();
    let agent_row = agent::create_agent(&pool, "ac43-revoked", ws.id, u.id)
        .await
        .unwrap();
    let agent_id = agent_row.id;
    let wake_id = Uuid::new_v4();
    let result = dispatch_shell_with_env(
        &pool,
        &vault,
        &exec,
        agent_id,
        wake_id,
        ws.id,
        serde_json::json!({ "OLD_KEY": "PLACEHOLDER:tombstoned" }),
    )
    .await;

    match &result {
        ToolResult::Error(msg) => {
            assert!(msg.contains("tombstoned"));
        }
        other => {
            let s = match other {
                ToolResult::Output(s) => s.as_str(),
                _ => "sleep",
            };
            panic!("expected Error, got: {s}");
        }
    }
    assert!(
        exec.calls.lock().await.is_empty(),
        "revoked credential must block child spawn"
    );

    // Event emitted with the closed-fail reason.
    let rows: Vec<event::Event> = sqlx::query_as(
        "SELECT * FROM events
         WHERE agent_id = $1 AND event_type = 'credential_unresolved'",
    )
    .bind(agent_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    let payload = rows[0].tool_input.as_ref().unwrap();
    assert!(payload.contains("missing_or_revoked"));
    // Old plaintext MUST NOT leak into the event payload.
    assert!(
        !payload.contains("old_value"),
        "plaintext leaked into event payload: {payload}"
    );
}

#[tokio::test]
async fn ac43_plaintext_never_appears_in_events_on_success() {
    let pool = common::test_pool().await;
    let vault = Arc::new(Vault::from_base64(common::TEST_VAULT_KEY_B64).unwrap());

    let u = user::create_local_admin(&pool, "dave@test.local", "Dave")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "org", "org", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "w", "w", u.id)
        .await
        .unwrap();

    // Highly distinctive plaintext so any leak is unambiguous.
    let secret = b"LEAK_CANARY_UNIQUE_STRING_97531";
    let sealed = vault.seal(ws.id, "leakcheck", secret).unwrap();
    credential::create(
        &pool,
        ws.id,
        "leakcheck",
        &sealed.ciphertext,
        &sealed.nonce,
        u.id,
    )
    .await
    .unwrap();

    let exec = RecordingExecutor::default();
    let agent_row = agent::create_agent(&pool, "ac43-leak", ws.id, u.id)
        .await
        .unwrap();
    let agent_id = agent_row.id;
    let wake_id = Uuid::new_v4();
    let _ = dispatch_shell_with_env(
        &pool,
        &vault,
        &exec,
        agent_id,
        wake_id,
        ws.id,
        serde_json::json!({ "LEAK": "PLACEHOLDER:leakcheck" }),
    )
    .await;

    // Scan every event for this agent. The plaintext must NEVER appear.
    let rows: Vec<event::Event> = sqlx::query_as("SELECT * FROM events WHERE agent_id = $1")
        .bind(agent_id)
        .fetch_all(&pool)
        .await
        .unwrap();

    for row in &rows {
        for s in [
            row.tool_input.as_deref(),
            row.tool_output.as_deref(),
            row.content.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            assert!(
                !s.contains("LEAK_CANARY_UNIQUE_STRING_97531"),
                "plaintext leaked into event '{}': {s}",
                row.event_type
            );
        }
    }

    // And the resolved env did reach the executor — sanity check that
    // we did not accidentally short-circuit the resolution path.
    let calls = exec.calls.lock().await.clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].env.get("LEAK").map(String::as_str),
        Some("LEAK_CANARY_UNIQUE_STRING_97531"),
    );
    let _: HashMap<_, _> = calls[0].env.clone();
}
