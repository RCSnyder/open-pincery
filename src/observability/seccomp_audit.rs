//! AC-77 / Slice G2c: `sandbox_syscall_denied` event emission.
//!
//! When a sandboxed child is killed by SIGSYS (exit signal 31, exit
//! code 128+31=159 under POSIX/shell convention), the kernel's
//! seccomp filter has refused a syscall in `Enforce` mode. This
//! module owns:
//!
//! - the audit-record parser for `type=SECCOMP` lines (used when an
//!   AUDIT_SECCOMP record is available via the AC-88 audit-netlink
//!   reader -- correlation is wired up in a follow-up slice),
//! - the `sandbox_syscall_denied` event payload schema,
//! - the database emit helper.
//!
//! The current G2c surface emits ONE `sandbox_syscall_denied` event
//! per SIGSYS-terminated tool invocation with `syscall_nr = -1`
//! (fallback per readiness AC-77 G2c "or -1 fallback") when no audit
//! record is available. The follow-up slice plugs in the unified
//! audit-records pass that drains both LANDLOCK and SECCOMP records
//! from the same `InvocationAuditSource` and replaces the -1
//! fallback with the kernel-reported syscall number.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{event, event::Event};

/// Event type string written to the `events` table for every SIGSYS-
/// terminated tool invocation.
pub const SANDBOX_SYSCALL_DENIED_EVENT: &str = "sandbox_syscall_denied";

/// POSIX/shell convention: a process killed by signal `N` is reported
/// as exit code `128 + N`. SIGSYS is signal 31, hence 159.
pub const SIGSYS_EXIT_CODE: i32 = 159;

/// Sentinel `syscall_nr` written when no AUDIT_SECCOMP record is
/// available to correlate with the SIGSYS termination. Down-stream
/// dashboards and tests use the literal value.
pub const SYSCALL_NR_UNKNOWN: i64 = -1;

/// Parsed AUDIT_SECCOMP record. Populated by [`parse_seccomp_audit_record`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeccompAuditRecord {
    pub pid: Option<u32>,
    pub syscall_nr: i64,
    pub syscall_name: Option<String>,
    pub audit_epoch_millis: Option<u128>,
}

/// Correlation context for `sandbox_syscall_denied` events. Mirrors
/// `LandlockAuditContext` so the unified-pass refactor (next slice)
/// can share most of the bookkeeping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeccompAuditContext {
    pub agent_id: Uuid,
    pub wake_id: Option<Uuid>,
    pub tool_name: String,
    pub audit_pids: Vec<u32>,
}

/// Parse a single line of the audit log. Returns `Some` only when the
/// line is a `type=SECCOMP` (or contains `seccomp` token) record and
/// at least one of `syscall=` / `syscall_nr=` is present.
///
/// Field extraction reuses the same `key=` / `key="..."` parser as
/// `landlock_audit` so quoted-and-unquoted forms behave identically.
pub fn parse_seccomp_audit_record(line: &str) -> Option<SeccompAuditRecord> {
    let is_seccomp =
        line.contains("type=SECCOMP") || line.contains("type=1326") || line.contains("seccomp=");
    if !is_seccomp {
        return None;
    }
    let syscall_nr_raw = first_field(line, &["syscall", "syscall_nr", "scall"])?;
    let syscall_nr = syscall_nr_raw.parse::<i64>().ok()?;
    let syscall_name = first_field(line, &["syscall_name", "scall_name", "name"]);
    let pid = field_value(line, "pid").and_then(|p| p.parse::<u32>().ok());
    let audit_epoch_millis = audit_timestamp_millis(line);
    Some(SeccompAuditRecord {
        pid,
        syscall_nr,
        syscall_name,
        audit_epoch_millis,
    })
}

/// Build the JSON payload string written to `events.tool_input` for a
/// `sandbox_syscall_denied` event.
pub fn sandbox_syscall_denied_payload(
    context: &SeccompAuditContext,
    record: Option<&SeccompAuditRecord>,
) -> String {
    #[derive(Serialize)]
    struct Payload<'a> {
        tool_name: &'a str,
        agent_id: String,
        wake_id: Option<String>,
        correlation_pids: &'a [u32],
        syscall_nr: i64,
        syscall_name: Option<&'a str>,
        audit_pid: Option<u32>,
        audit_epoch_millis: Option<u128>,
        record_correlated: bool,
    }
    let payload = Payload {
        tool_name: &context.tool_name,
        agent_id: context.agent_id.to_string(),
        wake_id: context.wake_id.map(|id| id.to_string()),
        correlation_pids: &context.audit_pids,
        syscall_nr: record.map(|r| r.syscall_nr).unwrap_or(SYSCALL_NR_UNKNOWN),
        syscall_name: record.and_then(|r| r.syscall_name.as_deref()),
        audit_pid: record.and_then(|r| r.pid),
        audit_epoch_millis: record.and_then(|r| r.audit_epoch_millis),
        record_correlated: record.is_some(),
    };
    serde_json::to_string(&payload).expect("sandbox_syscall_denied payload serializes")
}

/// Append a single `sandbox_syscall_denied` event. Used both when an
/// audit record correlates and when only the SIGSYS-fallback fires.
pub async fn append_sandbox_syscall_denied_event(
    pool: &PgPool,
    context: &SeccompAuditContext,
    record: Option<&SeccompAuditRecord>,
) -> Result<Event, AppError> {
    let payload = sandbox_syscall_denied_payload(context, record);
    event::append_event(
        pool,
        context.agent_id,
        SANDBOX_SYSCALL_DENIED_EVENT,
        "runtime",
        context.wake_id,
        Some(&context.tool_name),
        Some(&payload),
        None,
        None,
        None,
    )
    .await
}

// --- internal field parsers (mirrors landlock_audit private helpers) ------
//
// Kept private to avoid leaking parser internals; the unified-pass
// refactor will consolidate these into a single shared module.

fn audit_timestamp_millis(line: &str) -> Option<u128> {
    let start = line.find("audit(")? + "audit(".len();
    let tail = &line[start..];
    let end = tail.find(':').or_else(|| tail.find(')'))?;
    parse_epoch_millis(&tail[..end])
}

fn parse_epoch_millis(raw: &str) -> Option<u128> {
    let (seconds, fraction) = raw.split_once('.').unwrap_or((raw, ""));
    let seconds = seconds.parse::<u128>().ok()?;
    let mut millis = String::from(fraction.get(..fraction.len().min(3)).unwrap_or(""));
    while millis.len() < 3 {
        millis.push('0');
    }
    let millis = millis.parse::<u128>().ok()?;
    Some(seconds.saturating_mul(1000).saturating_add(millis))
}

fn first_field(line: &str, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| field_value(line, key))
}

fn field_value(line: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=");
    let mut search_from = 0usize;
    let start = loop {
        let relative = line[search_from..].find(&needle)?;
        let found = search_from + relative;
        if found == 0 || line[..found].ends_with([' ', '\t', '\n', '\r', ':', ',']) {
            break found + needle.len();
        }
        search_from = found + needle.len();
    };
    let tail = &line[start..];
    if let Some(quoted) = tail.strip_prefix('"') {
        let end = quoted.find('"')?;
        Some(quoted[..end].to_string())
    } else {
        let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
        Some(tail[..end].trim_end_matches(',').to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> SeccompAuditContext {
        SeccompAuditContext {
            agent_id: Uuid::nil(),
            wake_id: Some(Uuid::nil()),
            tool_name: "shell".into(),
            audit_pids: vec![1234],
        }
    }

    #[test]
    fn sigsys_exit_code_is_128_plus_31() {
        assert_eq!(SIGSYS_EXIT_CODE, 159);
    }

    #[test]
    fn parses_typical_audit_seccomp_record() {
        let line = r#"type=SECCOMP msg=audit(1714521600.123:42): auid=1000 uid=65534 gid=65534 ses=1 pid=1234 comm="ping" exe="/usr/bin/ping" sig=31 arch=c000003e syscall=41 compat=0 ip=0x7fabcdef code=0x80000000"#;
        let record = parse_seccomp_audit_record(line).expect("should parse SECCOMP");
        assert_eq!(record.syscall_nr, 41); // SYS_socket on x86_64
        assert_eq!(record.pid, Some(1234));
        assert_eq!(record.audit_epoch_millis, Some(1714521600123));
    }

    #[test]
    fn rejects_non_seccomp_audit_lines() {
        let landlock =
            r#"type=LANDLOCK_ACCESS msg=audit(1.0:1): pid=1 syscall=257 path="/etc/shadow""#;
        assert!(parse_seccomp_audit_record(landlock).is_none());
        let avc = r#"type=AVC msg=audit(1.0:1): apparmor="DENIED""#;
        assert!(parse_seccomp_audit_record(avc).is_none());
    }

    #[test]
    fn payload_with_correlated_record_sets_record_correlated_true() {
        let record = SeccompAuditRecord {
            pid: Some(1234),
            syscall_nr: 321,
            syscall_name: Some("bpf".into()),
            audit_epoch_millis: Some(1_714_521_600_123),
        };
        let json = sandbox_syscall_denied_payload(&ctx(), Some(&record));
        assert!(json.contains("\"syscall_nr\":321"));
        assert!(json.contains("\"syscall_name\":\"bpf\""));
        assert!(json.contains("\"record_correlated\":true"));
    }

    #[test]
    fn payload_without_record_emits_minus_one_sentinel() {
        let json = sandbox_syscall_denied_payload(&ctx(), None);
        assert!(json.contains("\"syscall_nr\":-1"));
        assert!(json.contains("\"record_correlated\":false"));
        assert!(json.contains("\"syscall_name\":null"));
    }
}
