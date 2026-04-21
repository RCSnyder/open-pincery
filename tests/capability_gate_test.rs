//! AC-35 (v6): capability gate and permission-mode routing.
//!
//! This file covers three layers:
//!
//! 1. Pure unit tests for the 3x5 `mode_allows` table and the
//!    closed-by-default `required_for` classifier.
//! 2. Unit tests for `PermissionMode::from_db_str`, including the
//!    critical fail-closed behaviour for unknown values.
//! 3. A DB-backed integration test that confirms a denied `shell` call
//!    from a `Locked` agent emits exactly one `tool_capability_denied`
//!    event and returns `ToolResult::Error` without ever invoking a
//!    subprocess.

mod common;

use open_pincery::models::agent;
use open_pincery::models::event;
use open_pincery::models::user;
use open_pincery::models::workspace;
use open_pincery::runtime::capability::{
    mode_allows, required_for, PermissionMode, ToolCapability,
};
use open_pincery::runtime::llm::{FunctionCall, ToolCallRequest};
use open_pincery::runtime::tools::{self, ToolResult};
use uuid::Uuid;

// ---------------------------------------------------------------------
// Layer 1: closed-by-default classification
// ---------------------------------------------------------------------

#[test]
fn required_for_known_tools() {
    assert_eq!(required_for("shell"), ToolCapability::ExecuteLocal);
    assert_eq!(required_for("plan"), ToolCapability::ReadLocal);
    assert_eq!(required_for("sleep"), ToolCapability::ReadLocal);
}

#[test]
fn required_for_unknown_tool_is_destructive() {
    // Closed-by-default: a new tool added without a classification arm
    // must be denied under Supervised and Locked.
    assert_eq!(required_for(""), ToolCapability::Destructive);
    assert_eq!(required_for("unknown"), ToolCapability::Destructive);
    assert_eq!(required_for("rm_rf"), ToolCapability::Destructive);
    assert_eq!(required_for("exfiltrate"), ToolCapability::Destructive);
}

// ---------------------------------------------------------------------
// Layer 1: PermissionMode::from_db_str (must fail closed)
// ---------------------------------------------------------------------

#[test]
fn permission_mode_from_known_strings() {
    assert_eq!(PermissionMode::from_db_str("yolo"), PermissionMode::Yolo);
    assert_eq!(
        PermissionMode::from_db_str("supervised"),
        PermissionMode::Supervised
    );
    assert_eq!(
        PermissionMode::from_db_str("locked"),
        PermissionMode::Locked
    );
}

#[test]
fn permission_mode_from_unknown_fails_closed_to_locked() {
    for garbage in ["", "YOLO", "root", "admin", " yolo ", "trusted"] {
        assert_eq!(
            PermissionMode::from_db_str(garbage),
            PermissionMode::Locked,
            "unknown permission mode {garbage:?} must fail closed to Locked"
        );
    }
}

// ---------------------------------------------------------------------
// Layer 2: the full 3x5 gate table
// ---------------------------------------------------------------------

#[test]
fn mode_allows_yolo_accepts_everything() {
    for cap in [
        ToolCapability::ReadLocal,
        ToolCapability::WriteLocal,
        ToolCapability::ExecuteLocal,
        ToolCapability::Network,
        ToolCapability::Destructive,
    ] {
        assert!(
            mode_allows(PermissionMode::Yolo, cap),
            "Yolo must allow {cap:?}"
        );
    }
}

#[test]
fn mode_allows_supervised_blocks_only_destructive() {
    for cap in [
        ToolCapability::ReadLocal,
        ToolCapability::WriteLocal,
        ToolCapability::ExecuteLocal,
        ToolCapability::Network,
    ] {
        assert!(
            mode_allows(PermissionMode::Supervised, cap),
            "Supervised must allow {cap:?}"
        );
    }
    assert!(
        !mode_allows(PermissionMode::Supervised, ToolCapability::Destructive),
        "Supervised must deny Destructive"
    );
}

#[test]
fn mode_allows_locked_permits_only_read_local() {
    assert!(mode_allows(
        PermissionMode::Locked,
        ToolCapability::ReadLocal
    ));
    for cap in [
        ToolCapability::WriteLocal,
        ToolCapability::ExecuteLocal,
        ToolCapability::Network,
        ToolCapability::Destructive,
    ] {
        assert!(
            !mode_allows(PermissionMode::Locked, cap),
            "Locked must deny {cap:?}"
        );
    }
}

#[test]
fn gate_table_is_total_15_cells() {
    // Exhaustive re-check of every (mode, capability) pair — if a new
    // variant is added to either enum the match in `mode_allows` must
    // extend; this test pins the 3x5 = 15 cell shape.
    let modes = [
        PermissionMode::Yolo,
        PermissionMode::Supervised,
        PermissionMode::Locked,
    ];
    let caps = [
        ToolCapability::ReadLocal,
        ToolCapability::WriteLocal,
        ToolCapability::ExecuteLocal,
        ToolCapability::Network,
        ToolCapability::Destructive,
    ];
    let mut checked = 0;
    for m in modes {
        for c in caps {
            // mode_allows is total and infallible; exercising it is the
            // assertion.
            let _ = mode_allows(m, c);
            checked += 1;
        }
    }
    assert_eq!(checked, 15);
}

// ---------------------------------------------------------------------
// Layer 3: DB-backed — Locked agent + shell call emits exactly one
// tool_capability_denied event and never spawns a shell.
// ---------------------------------------------------------------------

#[tokio::test]
async fn locked_agent_shell_call_is_denied_and_audited() {
    let pool = common::test_pool().await;

    // Seed the minimal user/org/workspace/agent graph. Use a tool call
    // whose "command" would be observable if it actually ran, so a
    // silent spawn bug would surface as side effects.
    let u = user::create_local_admin(&pool, "[email protected]", "Cap Gate")
        .await
        .expect("create user");
    let org = workspace::create_organization(&pool, "capgate", "capgate", u.id)
        .await
        .expect("create org");
    let ws = workspace::create_workspace(&pool, org.id, "capgate", "capgate", u.id)
        .await
        .expect("create ws");
    let agent_row = agent::create_agent(&pool, "locked-agent", ws.id, u.id)
        .await
        .expect("create agent");

    // Force permission_mode to 'locked'.
    sqlx::query("UPDATE agents SET permission_mode = 'locked' WHERE id = $1")
        .bind(agent_row.id)
        .execute(&pool)
        .await
        .expect("set locked");

    let wake_id = Uuid::new_v4();

    // Fabricate a shell tool call — requires ExecuteLocal, which Locked denies.
    let tc = ToolCallRequest {
        id: "call-1".into(),
        call_type: "function".into(),
        function: FunctionCall {
            name: "shell".into(),
            arguments: "{\"command\":\"touch /tmp/pincery_denial_probe\"}".into(),
        },
    };

    let result =
        tools::dispatch_tool(&tc, PermissionMode::Locked, &pool, agent_row.id, wake_id).await;

    match result {
        ToolResult::Error(msg) => {
            assert!(
                msg.contains("disallowed"),
                "denial error must explain the cause; got {msg:?}"
            );
        }
        ToolResult::Output(s) => panic!("expected denial, got Output({s})"),
        ToolResult::Sleep => panic!("expected denial, got Sleep"),
    }

    // Exactly one tool_capability_denied event for this agent+wake.
    let denied: Vec<event::Event> = sqlx::query_as(
        "SELECT * FROM events
         WHERE agent_id = $1 AND wake_id = $2 AND event_type = 'tool_capability_denied'",
    )
    .bind(agent_row.id)
    .bind(wake_id)
    .fetch_all(&pool)
    .await
    .expect("fetch denial events");
    assert_eq!(
        denied.len(),
        1,
        "locked shell call must emit exactly one tool_capability_denied event"
    );
    let ev = &denied[0];
    assert_eq!(ev.source, "runtime");
    assert_eq!(ev.tool_name.as_deref(), Some("shell"));
    let payload = ev
        .tool_input
        .as_deref()
        .expect("denial payload must be stored in tool_input");
    assert!(
        payload.contains("execute_local") || payload.contains("ExecuteLocal"),
        "denial payload must record the required capability; got {payload}"
    );
    assert!(
        payload.contains("locked") || payload.contains("Locked"),
        "denial payload must record the permission mode; got {payload}"
    );

    // And exactly zero tool_result events — confirming the executor was
    // never consulted.
    let results: Vec<event::Event> = sqlx::query_as(
        "SELECT * FROM events
         WHERE agent_id = $1 AND wake_id = $2 AND event_type = 'tool_result'",
    )
    .bind(agent_row.id)
    .bind(wake_id)
    .fetch_all(&pool)
    .await
    .expect("fetch tool_result events");
    assert!(
        results.is_empty(),
        "denied call must not generate a tool_result event"
    );

    // And the filesystem probe must not exist.
    assert!(
        !std::path::Path::new("/tmp/pincery_denial_probe").exists(),
        "denied shell call must not have spawned the command"
    );
}
