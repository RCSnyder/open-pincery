//! AC-88 / Slice G0f: deterministic Landlock audit parsing proof.
//!
//! Live kernel audit capture is gated by ABI and audit permissions, so
//! these tests pin the parser, fallback, and event-payload contract in
//! a way every host can run.

mod common;

use open_pincery::models::{agent, event, user, workspace};
use open_pincery::observability::landlock_audit::append_landlock_denials_within;
#[cfg(target_os = "linux")]
use open_pincery::observability::landlock_audit::invocation_audit_source_from_end;
use open_pincery::observability::landlock_audit::{
    append_available_landlock_denials, append_landlock_denied_event, audit_log_unavailable_for_abi,
    audit_record_matches_context, landlock_denied_payload, parse_audit_record,
    parse_available_landlock_records, AuditLogFileSource, AuditRecordSource, LandlockAuditContext,
};
#[cfg(target_os = "linux")]
use open_pincery::runtime::sandbox::init_policy::LANDLOCK_AUDIT_ABI_FLOOR;
use std::collections::VecDeque;
use std::io;
use std::io::Write;
use uuid::Uuid;

struct FixtureSource {
    records: Vec<String>,
}

impl AuditRecordSource for FixtureSource {
    fn read_available_records(&mut self) -> io::Result<Vec<String>> {
        Ok(std::mem::take(&mut self.records))
    }
}

struct ScheduledFixtureSource {
    batches: VecDeque<Vec<String>>,
}

impl AuditRecordSource for ScheduledFixtureSource {
    fn read_available_records(&mut self) -> io::Result<Vec<String>> {
        Ok(self.batches.pop_front().unwrap_or_default())
    }
}

fn test_audit_context(
    agent_id: Uuid,
    wake_id: Option<Uuid>,
    audit_pids: Vec<u32>,
) -> LandlockAuditContext {
    LandlockAuditContext {
        agent_id,
        wake_id,
        tool_name: "shell".into(),
        audit_pids,
        invocation_started_at_millis: None,
        invocation_finished_at_millis: None,
    }
}

#[test]
fn parses_landlock_denied_audit_record() {
    let line = concat!(
        "type=LANDLOCK_DENIED msg=audit(1777298358.207:42): ",
        "pid=4242 comm=\"cat\" syscall=\"openat\" ",
        "path=\"/etc/shadow\" requested_access=\"read_file\""
    );

    let record = parse_audit_record(line).expect("LANDLOCK audit record should parse");

    assert_eq!(record.pid, Some(4242));
    assert_eq!(record.parent_pid, None);
    assert_eq!(record.audit_epoch_millis, Some(1_777_298_358_207));
    assert_eq!(record.denied_path, "/etc/shadow");
    assert_eq!(record.requested_access, "read_file");
    assert_eq!(record.syscall, "openat");
}

#[test]
fn parses_parent_pid_for_short_lived_child_correlation() {
    let line = concat!(
        "type=LANDLOCK_DENIED msg=audit(1777298358.207:42): ",
        "ppid=4242 pid=4243 comm=\"cat\" syscall=\"openat\" ",
        "path=\"/etc/shadow\" requested_access=\"read_file\""
    );

    let record = parse_audit_record(line).expect("LANDLOCK audit record should parse");

    assert_eq!(record.pid, Some(4243));
    assert_eq!(record.parent_pid, Some(4242));
}

#[test]
fn parses_pid_with_token_boundary_not_ppid_substring() {
    let line = concat!(
        "type=LANDLOCK_DENIED msg=audit(1777298358.207:42): ",
        "ppid=31337 pid=4242 comm=\"cat\" syscall=\"openat\" ",
        "path=\"/etc/shadow\" requested_access=\"read_file\""
    );

    let record = parse_audit_record(line).expect("LANDLOCK audit record should parse");

    assert_eq!(record.pid, Some(4242));
}

#[test]
fn ignores_non_landlock_audit_records() {
    let line = "type=SYSCALL msg=audit(1777298358.207:43): pid=4242 comm=\"cat\"";
    assert_eq!(parse_audit_record(line), None);
}

#[test]
fn source_reader_filters_and_parses_landlock_records() {
    let mut source = FixtureSource {
        records: vec![
            "type=SYSCALL msg=audit(1777298358.207:43): pid=4242 comm=\"cat\"".into(),
            concat!(
                "type=LANDLOCK_DENIED msg=audit(1777298358.207:44): ",
                "pid=4242 syscall=openat name=\"/root/.ssh/id_rsa\" requested=read_file"
            )
            .into(),
        ],
    };

    let records = parse_available_landlock_records(&mut source).expect("source should read");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].denied_path, "/root/.ssh/id_rsa");
    assert_eq!(records[0].requested_access, "read_file");
    assert_eq!(records[0].syscall, "openat");
}

#[test]
fn file_source_from_current_end_does_not_replay_old_records() {
    let mut file = tempfile::NamedTempFile::new().expect("temp audit fixture");
    writeln!(
        file,
        "type=LANDLOCK_DENIED msg=audit(1777298358.207:44): pid=1111 syscall=openat name=\"/old\" requested=read_file"
    )
    .expect("write old record");

    let mut source = AuditLogFileSource::from_current_end(file.path()).expect("cursor at EOF");
    writeln!(
        file,
        "type=LANDLOCK_DENIED msg=audit(1777298358.207:45): pid=2222 syscall=openat name=\"/new\" requested=read_file"
    )
    .expect("write new record");

    let records = parse_available_landlock_records(&mut source).expect("source should read");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].pid, Some(2222));
    assert_eq!(records[0].denied_path, "/new");
}

#[test]
fn abi6_reports_audit_log_unavailable_without_disabling_sandbox() {
    let unavailable =
        audit_log_unavailable_for_abi(Some(6)).expect("ABI 6 should degrade only audit visibility");

    assert_eq!(unavailable.landlock_abi, Some(6));
    assert_eq!(unavailable.required_abi, 7);
    assert!(unavailable.sandbox_still_enforced);
    assert_eq!(audit_log_unavailable_for_abi(Some(7)), None);
}

#[test]
fn landlock_denied_payload_carries_tool_context_and_denial_fields() {
    let agent_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    let wake_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
    let context = test_audit_context(agent_id, Some(wake_id), vec![4242]);
    let record = parse_audit_record(
        "type=LANDLOCK_DENIED msg=audit(1777298358.207:44): pid=4242 syscall=openat name=\"/root/.ssh/id_rsa\" requested=read_file",
    )
    .expect("fixture should parse");

    let payload = landlock_denied_payload(&context, &record);
    let json: serde_json::Value = serde_json::from_str(&payload).expect("payload is JSON");

    assert_eq!(json["tool_name"], "shell");
    assert_eq!(json["agent_id"], agent_id.to_string());
    assert_eq!(json["wake_id"], wake_id.to_string());
    assert_eq!(json["correlation_pids"][0], 4242);
    assert_eq!(json["audit_pid"], 4242);
    assert_eq!(json["audit_epoch_millis"].as_u64(), Some(1_777_298_358_207));
    assert_eq!(json["denied_path"], "/root/.ssh/id_rsa");
    assert_eq!(json["requested_access"], "read_file");
    assert_eq!(json["syscall"], "openat");
}

#[tokio::test]
async fn append_available_denials_only_writes_pid_correlated_records() {
    let pool = common::test_pool().await;
    let u = user::create_local_admin(&pool, "ac88-correlation@test.com", "AC88 Correlation")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac88-correlation", "ac88", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac88-correlation", "ac88", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "ac88-agent", ws.id, u.id)
        .await
        .unwrap();
    let wake_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
    let context = test_audit_context(a.id, Some(wake_id), vec![31337, 4242]);
    let mut source = FixtureSource {
        records: vec![
            concat!(
                "type=LANDLOCK_DENIED msg=audit(1777298358.207:44): ",
                "pid=9999 syscall=openat name=\"/other\" requested=read_file"
            )
            .into(),
            concat!(
                "type=LANDLOCK_DENIED msg=audit(1777298358.207:45): ",
                "pid=4242 syscall=openat name=\"/tmp/ac88_denied\" requested=write_file"
            )
            .into(),
        ],
    };

    let appended = append_available_landlock_denials(&pool, &context, &mut source)
        .await
        .unwrap();
    let events = event::recent_events(&pool, a.id, 10).await.unwrap();

    assert_eq!(appended, 1);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "landlock_denied");
    assert_eq!(events[0].tool_name.as_deref(), Some("shell"));
    assert!(events[0]
        .tool_input
        .as_deref()
        .expect("payload")
        .contains("/tmp/ac88_denied"));
}

#[tokio::test]
async fn bounded_poll_appends_all_delayed_correlated_denials() {
    let pool = common::test_pool().await;
    let u = user::create_local_admin(&pool, "ac88-delayed@test.com", "AC88 Delayed")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac88-delayed", "ac88", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac88-delayed", "ac88", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "ac88-delayed-agent", ws.id, u.id)
        .await
        .unwrap();
    let context = test_audit_context(a.id, Some(Uuid::new_v4()), vec![4242]);
    let mut source = ScheduledFixtureSource {
        batches: VecDeque::from([
            Vec::new(),
            Vec::new(),
            vec![concat!(
                "type=LANDLOCK_DENIED msg=audit(1777298358.207:45): ",
                "pid=4242 syscall=openat name=\"/tmp/ac88_delayed_one\" requested=write_file"
            )
            .into()],
            Vec::new(),
            vec![concat!(
                "type=LANDLOCK_DENIED msg=audit(1777298358.257:46): ",
                "pid=4242 syscall=openat name=\"/tmp/ac88_delayed_two\" requested=write_file"
            )
            .into()],
        ]),
    };

    let appended = append_landlock_denials_within(
        &pool,
        &context,
        &mut source,
        std::time::Duration::from_millis(400),
    )
    .await
    .unwrap();

    assert_eq!(appended, 2);
    let events = event::recent_events(&pool, a.id, 10).await.unwrap();
    assert_eq!(events.len(), 2);
    assert!(events
        .iter()
        .all(|event| event.event_type == "landlock_denied"));
    let payloads = events
        .iter()
        .filter_map(|event| event.tool_input.as_deref())
        .collect::<Vec<_>>();
    assert!(payloads
        .iter()
        .any(|payload| payload.contains("/tmp/ac88_delayed_one")));
    assert!(payloads
        .iter()
        .any(|payload| payload.contains("/tmp/ac88_delayed_two")));
}

#[test]
fn parent_pid_correlation_matches_short_lived_child_record() {
    let context = test_audit_context(Uuid::new_v4(), Some(Uuid::new_v4()), vec![4242]);
    let record = parse_audit_record(
        "type=LANDLOCK_DENIED msg=audit(1777298358.207:44): ppid=4242 pid=4243 syscall=openat name=\"/tmp/ac88_child\" requested=write_file",
    )
    .expect("fixture should parse");

    assert!(audit_record_matches_context(&context, &record));
}

#[test]
fn timestamp_window_rejects_reused_pid_after_invocation() {
    let mut context = test_audit_context(Uuid::new_v4(), Some(Uuid::new_v4()), vec![4242]);
    context.invocation_started_at_millis = Some(1_777_298_358_000);
    context.invocation_finished_at_millis = Some(1_777_298_358_100);
    let record = parse_audit_record(
        "type=LANDLOCK_DENIED msg=audit(1777298360.000:44): pid=4242 syscall=openat name=\"/tmp/ac88_reused\" requested=write_file",
    )
    .expect("fixture should parse");

    assert!(!audit_record_matches_context(&context, &record));
}

#[test]
fn timestamp_window_rejects_untimestamped_live_context_records() {
    let mut context = test_audit_context(Uuid::new_v4(), Some(Uuid::new_v4()), vec![4242]);
    context.invocation_started_at_millis = Some(1_777_298_358_000);
    context.invocation_finished_at_millis = Some(1_777_298_358_100);
    let record = parse_audit_record(
        "type=LANDLOCK_DENIED pid=4242 syscall=openat name=\"/tmp/ac88_untimed\" requested=write_file",
    )
    .expect("fixture should parse");

    assert!(!audit_record_matches_context(&context, &record));
}

#[tokio::test]
async fn timestamp_window_allows_records_from_invocation() {
    let pool = common::test_pool().await;
    let u = user::create_local_admin(&pool, "ac88-time@test.com", "AC88 Time")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac88-time", "ac88", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac88-time", "ac88", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "ac88-time-agent", ws.id, u.id)
        .await
        .unwrap();
    let mut context = test_audit_context(a.id, Some(Uuid::new_v4()), vec![4242]);
    context.invocation_started_at_millis = Some(1_777_298_358_000);
    context.invocation_finished_at_millis = Some(1_777_298_358_500);
    let mut source = FixtureSource {
        records: vec![
            "type=LANDLOCK_DENIED msg=audit(1777298358.207:45): pid=4242 syscall=openat name=\"/tmp/ac88_timed\" requested=write_file".into(),
        ],
    };

    let appended = append_available_landlock_denials(&pool, &context, &mut source)
        .await
        .unwrap();

    assert_eq!(appended, 1);
    let events = event::recent_events(&pool, a.id, 10).await.unwrap();
    assert!(events[0]
        .tool_input
        .as_deref()
        .expect("payload")
        .contains("/tmp/ac88_timed"));
}

#[tokio::test]
async fn append_landlock_denied_event_rejects_uncorrelated_records() {
    let pool = common::test_pool().await;
    let u = user::create_local_admin(&pool, "ac88-reject@test.com", "AC88 Reject")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac88-reject", "ac88", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac88-reject", "ac88", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "ac88-reject-agent", ws.id, u.id)
        .await
        .unwrap();
    let context = test_audit_context(a.id, Some(Uuid::new_v4()), vec![4242]);
    let record = parse_audit_record(
        "type=LANDLOCK_DENIED msg=audit(1777298358.207:44): pid=9999 syscall=openat name=\"/other\" requested=read_file",
    )
    .expect("fixture should parse");

    assert!(!audit_record_matches_context(&context, &record));
    assert!(append_landlock_denied_event(&pool, &context, &record)
        .await
        .is_err());
    assert!(event::recent_events(&pool, a.id, 10)
        .await
        .unwrap()
        .is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn live_audit_capture_preconditions_are_explicit() {
    use open_pincery::runtime::sandbox::preflight::{KernelProbe, RealKernelProbe};

    let abi = RealKernelProbe.landlock_abi();
    if abi.unwrap_or(0) < LANDLOCK_AUDIT_ABI_FLOOR {
        eprintln!(
            "AC-88 live audit capture skipped: Landlock ABI {abi:?} is below required ABI {LANDLOCK_AUDIT_ABI_FLOOR}"
        );
        return;
    }

    match invocation_audit_source_from_end() {
        Ok(_) => {}
        Err(error) => {
            eprintln!(
                "AC-88 live audit capture skipped: no readable audit netlink or log source ({error})"
            );
        }
    }
}

#[cfg(target_os = "linux")]
fn command_available(name: &str) -> bool {
    std::process::Command::new("sh")
        .args(["-c", &format!("command -v {name} >/dev/null 2>&1")])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn live_sandbox_preconditions_met() -> bool {
    use open_pincery::runtime::sandbox::preflight::{KernelProbe, RealKernelProbe};

    if std::env::var_os("OPEN_PINCERY_SKIP_REAL_BWRAP").is_some() {
        eprintln!("AC-88 live sandbox proof skipped: OPEN_PINCERY_SKIP_REAL_BWRAP is set");
        return false;
    }
    if !command_available("bwrap") {
        eprintln!("AC-88 live sandbox proof skipped: bwrap not on PATH");
        return false;
    }
    if !open_pincery::runtime::sandbox::landlock_layer::landlock_supported() {
        eprintln!("AC-88 live sandbox proof skipped: kernel does not support Landlock");
        return false;
    }
    let abi = RealKernelProbe.landlock_abi();
    if abi.unwrap_or(0) < LANDLOCK_AUDIT_ABI_FLOOR {
        eprintln!(
            "AC-88 live sandbox proof skipped: Landlock ABI {abi:?} is below required ABI {LANDLOCK_AUDIT_ABI_FLOOR}"
        );
        return false;
    }

    std::env::set_var("PINCERY_INIT_BIN_PATH", env!("CARGO_BIN_EXE_pincery-init"));
    true
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn live_denied_open_process_pid_is_captured_for_correlation() {
    use open_pincery::config::{ResolvedSandboxMode, SandboxMode};
    use open_pincery::runtime::sandbox::{
        bwrap::RealSandbox, ExecResult, SandboxProfile, ShellCommand, ToolExecutor,
    };
    use std::time::Duration;

    if !live_sandbox_preconditions_met() {
        return;
    }

    let sandbox = RealSandbox::new(ResolvedSandboxMode {
        mode: SandboxMode::Enforce,
        allow_unsafe: false,
    });
    let profile = SandboxProfile {
        env_allowlist: vec!["PATH".into()],
        deny_net: true,
        timeout: Duration::from_secs(10),
        cwd: None,
        cgroup: None,
        seccomp: false,
        landlock: true,
    };
    let command = concat!(
        "printf 'pid=%s\\n' \"$$\"; ",
        "printf blocked >/tmp/ac88-denied-open || true"
    );

    let result = sandbox.run(&ShellCommand::new(command), &profile).await;

    match result {
        ExecResult::Ok {
            stdout,
            stderr,
            exit_code,
            audit_pids,
        } => {
            assert_eq!(exit_code, 0, "live AC-88 command failed; stderr={stderr:?}");
            let shell_pid = stdout
                .lines()
                .find_map(|line| line.strip_prefix("pid="))
                .and_then(|pid| pid.parse::<u32>().ok())
                .expect("command should print shell pid");
            assert!(
                audit_pids.contains(&shell_pid),
                "process-tree correlation must include the denied-open shell pid {shell_pid}; captured={audit_pids:?}"
            );
        }
        other => panic!("live AC-88 sandbox proof failed before command completion: {other:?}"),
    }
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn live_denied_open_appends_landlock_denied_event_when_audit_source_available() {
    use open_pincery::config::{ResolvedSandboxMode, SandboxMode};
    use open_pincery::runtime::capability::PermissionMode;
    use open_pincery::runtime::llm::{FunctionCall, ToolCallRequest};
    use open_pincery::runtime::sandbox::bwrap::RealSandbox;
    use open_pincery::runtime::sandbox::ToolExecutor;
    use open_pincery::runtime::tools::{self, ToolResult};
    use open_pincery::runtime::vault::Vault;
    use std::sync::Arc;

    if !live_sandbox_preconditions_met() {
        return;
    }
    if let Err(error) = invocation_audit_source_from_end() {
        eprintln!(
            "AC-88 live denial-to-event proof skipped: no readable audit netlink or log source ({error})"
        );
        return;
    }

    let pool = common::test_pool().await;
    let vault = Arc::new(Vault::from_base64(common::TEST_VAULT_KEY_B64).unwrap());
    let u = user::create_local_admin(&pool, "ac88-live@test.com", "AC88 Live")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "ac88-live", "ac88", u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "ac88-live", "ac88", u.id)
        .await
        .unwrap();
    let a = agent::create_agent(&pool, "ac88-live-agent", ws.id, u.id)
        .await
        .unwrap();
    let executor: Arc<dyn ToolExecutor> = Arc::new(RealSandbox::new(ResolvedSandboxMode {
        mode: SandboxMode::Enforce,
        allow_unsafe: false,
    }));
    let tool_call = ToolCallRequest {
        id: "call-ac88-live".into(),
        call_type: "function".into(),
        function: FunctionCall {
            name: "shell".into(),
            arguments: serde_json::json!({
                "command": "printf blocked >/tmp/ac88-live-denied || true"
            })
            .to_string(),
        },
    };

    let result = tools::dispatch_tool(
        &tool_call,
        PermissionMode::Yolo,
        &pool,
        a.id,
        ws.id,
        Uuid::new_v4(),
        &executor,
        &vault,
    )
    .await;

    match result {
        ToolResult::Output(output) => {
            assert!(
                output.contains("Permission denied") || output.contains("Operation not permitted"),
                "live AC-88 command should show a denied write; output={output:?}"
            );
        }
        ToolResult::Error(error) => panic!("live AC-88 shell dispatch failed: {error}"),
        ToolResult::Sleep => panic!("live AC-88 shell dispatch unexpectedly slept"),
    }

    let events = event::recent_events(&pool, a.id, 20).await.unwrap();
    let denied = events
        .iter()
        .find(|event| event.event_type == "landlock_denied")
        .expect("live denied open should append a correlated landlock_denied event");
    let payload = denied.tool_input.as_deref().expect("landlock payload");

    assert!(
        payload.contains("/tmp/ac88-live-denied"),
        "payload={payload}"
    );
    assert!(payload.contains("write_file"), "payload={payload}");
}
