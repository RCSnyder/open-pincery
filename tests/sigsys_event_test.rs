//! AC-77 / G2c runtime evidence (review R1):
//!
//! Verify that when a sandboxed shell tool exits with code 159
//! (POSIX 128 + SIGSYS=31, i.e. the kernel's response to a default-deny
//! seccomp violation), the dispatch path emits exactly one
//! `sandbox_syscall_denied` event into the `events` table with the
//! payload shape readiness AC-77 row 4 specifies.
//!
//! This is a unit-style integration test of the dispatch branch added
//! in `src/runtime/tools.rs` -- it uses a mock `ToolExecutor` that
//! synthesizes the SIGSYS exit code, so it does NOT need bwrap or
//! a privileged Linux container; just a Postgres pool. The complementary
//! live test (`tests/seccomp_allowlist_test.rs::unshare_blocked_by_default_deny_allowlist`)
//! exercises the same code path with a real kernel SIGSYS on CI.
//!
//! The whole file is gated to `target_os = "linux"` because the
//! SIGSYS-detection branch in `src/runtime/tools.rs` is itself
//! `#[cfg(target_os = "linux")]`. On non-Linux hosts the dispatch
//! path simply does not emit `sandbox_syscall_denied`, so this test's
//! assertions would be false-negative.

#![cfg(target_os = "linux")]

mod common;

use async_trait::async_trait;
use open_pincery::models::{agent, event, user, workspace};
use open_pincery::runtime::capability::PermissionMode;
use open_pincery::runtime::llm::{FunctionCall, ToolCallRequest};
use open_pincery::runtime::sandbox::{ExecResult, SandboxProfile, ShellCommand, ToolExecutor};
use open_pincery::runtime::tools::{self, ToolResult};
use open_pincery::runtime::vault::Vault;
use std::sync::Arc;
use uuid::Uuid;

const SIGSYS_EXIT_CODE: i32 = 159; // 128 + SIGSYS(31), POSIX

/// Mock executor that always reports a SIGSYS-induced termination.
/// Mirrors what `bwrap.rs` and `ProcessExecutor` produce when the
/// kernel kills the bwrapped child for a disallowed syscall.
#[derive(Clone, Default)]
struct SigsysExecutor;

#[async_trait]
impl ToolExecutor for SigsysExecutor {
    async fn run(&self, _cmd: &ShellCommand, _profile: &SandboxProfile) -> ExecResult {
        ExecResult::Ok {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: SIGSYS_EXIT_CODE,
            audit_pids: Vec::new(),
        }
    }
}

#[tokio::test]
async fn sigsys_exit_emits_sandbox_syscall_denied_event() {
    let pool = common::test_pool().await;
    let vault = Arc::new(Vault::from_base64(common::TEST_VAULT_KEY_B64).unwrap());

    let u = user::create_local_admin(&pool, "ac77@test.local", "AC77")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac77-org", "ac77-org", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac77-ws", "ac77-ws", u.id)
        .await
        .unwrap();
    let agent_row = agent::create_agent(&pool, "ac77-sigsys", ws.id, u.id)
        .await
        .unwrap();
    let agent_id = agent_row.id;
    let wake_id = Uuid::new_v4();

    let executor: Arc<dyn ToolExecutor> = Arc::new(SigsysExecutor);
    let tc = ToolCallRequest {
        id: "call-sigsys".into(),
        call_type: "function".into(),
        function: FunctionCall {
            name: "shell".into(),
            arguments: serde_json::json!({ "command": "unshare -U /bin/true" }).to_string(),
        },
    };
    let result = tools::dispatch_tool(
        &tc,
        PermissionMode::Yolo, // shell needs ExecuteLocal at minimum
        &pool,
        agent_id,
        ws.id,
        wake_id,
        &executor,
        &vault,
    )
    .await;

    // Tool returned (we report whatever the executor reported back to
    // the agent; SIGSYS does not short-circuit the response shape).
    match &result {
        ToolResult::Output(_) | ToolResult::Error(_) => {}
        _ => panic!("unexpected ToolResult variant (sleep/yield)"),
    }

    // Exactly one sandbox_syscall_denied event for this agent/wake.
    let n: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM events
         WHERE agent_id = $1
           AND wake_id = $2
           AND event_type = 'sandbox_syscall_denied'",
    )
    .bind(agent_id)
    .bind(wake_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        n.0, 1,
        "exactly one sandbox_syscall_denied event must be emitted on SIGSYS exit"
    );

    // Payload shape is serialized as JSON into the `tool_input` column
    // by `event::append_event` (the events table is intentionally
    // string-typed; payload-as-JSON-string mirrors landlock_denied).
    let rows: Vec<event::Event> = sqlx::query_as(
        "SELECT * FROM events
         WHERE agent_id = $1 AND event_type = 'sandbox_syscall_denied'",
    )
    .bind(agent_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    let row = &rows[0];
    assert_eq!(row.source, "runtime", "event.source must be 'runtime'");
    assert_eq!(
        row.tool_name.as_deref(),
        Some("shell"),
        "event.tool_name must be 'shell': {:?}",
        row.tool_name
    );
    let payload_str = row
        .tool_input
        .as_deref()
        .expect("payload JSON must be in tool_input");
    let payload: serde_json::Value = serde_json::from_str(payload_str)
        .unwrap_or_else(|e| panic!("payload {payload_str:?} did not parse as JSON: {e}"));
    assert_eq!(
        payload.get("tool_name").and_then(|v| v.as_str()),
        Some("shell"),
        "payload.tool_name must be 'shell': {payload}"
    );
    assert_eq!(
        payload.get("syscall_nr").and_then(|v| v.as_i64()),
        Some(-1),
        "payload.syscall_nr must be -1 sentinel until AUDIT_SECCOMP correlation lands: {payload}"
    );
    assert_eq!(
        payload.get("record_correlated").and_then(|v| v.as_bool()),
        Some(false),
        "payload.record_correlated must be false until AUDIT_SECCOMP correlation lands: {payload}"
    );
    assert_eq!(
        payload.get("agent_id").and_then(|v| v.as_str()),
        Some(agent_id.to_string().as_str()),
        "payload.agent_id must match dispatching agent: {payload}"
    );
    assert_eq!(
        payload.get("wake_id").and_then(|v| v.as_str()),
        Some(wake_id.to_string().as_str()),
        "payload.wake_id must match dispatching wake: {payload}"
    );
}

/// False-positive guard: a non-SIGSYS exit (e.g. exit 2 from a shell
/// command failure) must NOT emit a sandbox_syscall_denied event,
/// otherwise dashboards would over-report seccomp denials.
#[tokio::test]
async fn non_sigsys_exit_does_not_emit_sandbox_syscall_denied() {
    #[derive(Clone, Default)]
    struct FailingExecutor;

    #[async_trait]
    impl ToolExecutor for FailingExecutor {
        async fn run(&self, _cmd: &ShellCommand, _profile: &SandboxProfile) -> ExecResult {
            ExecResult::Ok {
                stdout: String::new(),
                stderr: "command failed".into(),
                exit_code: 2,
                audit_pids: Vec::new(),
            }
        }
    }

    let pool = common::test_pool().await;
    let vault = Arc::new(Vault::from_base64(common::TEST_VAULT_KEY_B64).unwrap());

    let u = user::create_local_admin(&pool, "ac77b@test.local", "AC77b")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac77b-org", "ac77b-org", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac77b-ws", "ac77b-ws", u.id)
        .await
        .unwrap();
    let agent_row = agent::create_agent(&pool, "ac77-noflag", ws.id, u.id)
        .await
        .unwrap();
    let agent_id = agent_row.id;
    let wake_id = Uuid::new_v4();

    let executor: Arc<dyn ToolExecutor> = Arc::new(FailingExecutor);
    let tc = ToolCallRequest {
        id: "call-fail".into(),
        call_type: "function".into(),
        function: FunctionCall {
            name: "shell".into(),
            arguments: serde_json::json!({ "command": "false" }).to_string(),
        },
    };
    let _ = tools::dispatch_tool(
        &tc,
        PermissionMode::Yolo,
        &pool,
        agent_id,
        ws.id,
        wake_id,
        &executor,
        &vault,
    )
    .await;

    let n: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM events
         WHERE agent_id = $1 AND event_type = 'sandbox_syscall_denied'",
    )
    .bind(agent_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        n.0, 0,
        "non-SIGSYS exit (code 2) must not emit sandbox_syscall_denied"
    );
}
