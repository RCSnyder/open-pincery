//! AC-88 / Slice G0f: Landlock audit parsing and event bridge.
//!
//! The live kernel source is host-dependent (`CAP_AUDIT_READ`, auditd,
//! or readable `/var/log/audit/audit.log`), but the record parser and
//! event payload are deterministic. This module keeps those pieces
//! testable without requiring a live audit subsystem.

#[cfg(target_os = "linux")]
use super::landlock_audit_netlink::AuditNetlinkSource;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{event, event::Event};
use crate::runtime::sandbox::init_policy::LANDLOCK_AUDIT_ABI_FLOOR;

pub const LANDLOCK_DENIED_EVENT: &str = "landlock_denied";
pub const AUDIT_LOG_UNAVAILABLE_EVENT: &str = "audit_log_unavailable";
pub const AUDIT_LOG_PATH_ENV: &str = "OPEN_PINCERY_LANDLOCK_AUDIT_LOG";
pub const DEFAULT_AUDIT_LOG_PATH: &str = "/var/log/audit/audit.log";
const AUDIT_CORRELATION_CLOCK_SKEW_MILLIS: u128 = 250;
const AUDIT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const AUDIT_POST_APPEND_QUIET_PERIOD: Duration = Duration::from_millis(250);

static AUDIT_SOURCE_WARNING_EMITTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandlockAuditRecord {
    pub pid: Option<u32>,
    pub parent_pid: Option<u32>,
    pub audit_epoch_millis: Option<u128>,
    pub denied_path: String,
    pub requested_access: String,
    pub syscall: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditLogUnavailable {
    pub landlock_abi: Option<u32>,
    pub required_abi: u32,
    pub sandbox_still_enforced: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandlockAuditContext {
    pub agent_id: Uuid,
    pub wake_id: Option<Uuid>,
    pub tool_name: String,
    pub audit_pids: Vec<u32>,
    pub invocation_started_at_millis: Option<u128>,
    pub invocation_finished_at_millis: Option<u128>,
}

#[derive(Debug, Clone)]
pub struct AuditLogFileSource {
    path: PathBuf,
    offset: u64,
}

pub trait AuditRecordSource {
    fn read_available_records(&mut self) -> io::Result<Vec<String>>;
}

impl AuditLogFileSource {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            offset: 0,
        }
    }

    pub fn from_current_end(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let offset = File::open(&path)?.seek(SeekFrom::End(0))?;
        Ok(Self { path, offset })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl AuditRecordSource for AuditLogFileSource {
    fn read_available_records(&mut self) -> io::Result<Vec<String>> {
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(self.offset))?;
        let mut reader = BufReader::new(file);
        let mut lines = Vec::new();
        let mut bytes_read = 0u64;
        loop {
            let mut line = String::new();
            let n = reader.read_line(&mut line)?;
            if n == 0 {
                break;
            }
            bytes_read += n as u64;
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }
            lines.push(line);
        }
        self.offset += bytes_read;
        Ok(lines)
    }
}

#[derive(Debug)]
pub enum InvocationAuditSource {
    #[cfg(target_os = "linux")]
    Netlink(AuditNetlinkSource),
    File(AuditLogFileSource),
}

impl AuditRecordSource for InvocationAuditSource {
    fn read_available_records(&mut self) -> io::Result<Vec<String>> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Netlink(source) => source.read_available_records(),
            Self::File(source) => source.read_available_records(),
        }
    }
}

pub fn invocation_audit_source_from_end() -> io::Result<InvocationAuditSource> {
    #[cfg(target_os = "linux")]
    {
        let netlink_error = match AuditNetlinkSource::new() {
            Ok(source) => return Ok(InvocationAuditSource::Netlink(source)),
            Err(error) => error,
        };
        AuditLogFileSource::from_current_end(configured_audit_log_path())
            .map(InvocationAuditSource::File)
            .map_err(|file_error| {
                io::Error::new(
                    file_error.kind(),
                    format!(
                        "audit netlink unavailable: {netlink_error}; audit log fallback unavailable: {file_error}"
                    ),
                )
            })
    }

    #[cfg(not(target_os = "linux"))]
    {
        AuditLogFileSource::from_current_end(configured_audit_log_path())
            .map(InvocationAuditSource::File)
    }
}

pub fn configured_audit_log_path() -> PathBuf {
    std::env::var("OPEN_PINCERY_LANDLOCK_AUDIT_LOG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_AUDIT_LOG_PATH))
}

pub fn audit_source_warning_should_emit_once() -> bool {
    !AUDIT_SOURCE_WARNING_EMITTED.swap(true, Ordering::Relaxed)
}

pub fn audit_log_unavailable_for_abi(landlock_abi: Option<u32>) -> Option<AuditLogUnavailable> {
    match landlock_abi {
        Some(found) if found >= LANDLOCK_AUDIT_ABI_FLOOR => None,
        Some(found) => Some(AuditLogUnavailable {
            landlock_abi: Some(found),
            required_abi: LANDLOCK_AUDIT_ABI_FLOOR,
            sandbox_still_enforced: true,
            reason: format!("Landlock ABI {found} does not support audit logging flags"),
        }),
        None => Some(AuditLogUnavailable {
            landlock_abi: None,
            required_abi: LANDLOCK_AUDIT_ABI_FLOOR,
            sandbox_still_enforced: true,
            reason: "Landlock ABI unavailable for audit logging".into(),
        }),
    }
}

pub fn current_epoch_millis() -> Option<u128> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
}

pub fn parse_audit_record(line: &str) -> Option<LandlockAuditRecord> {
    if !line.contains("LANDLOCK") {
        return None;
    }

    let denied_path = first_field(line, &["path", "name", "denied_path"])?;
    let requested_access = first_field(
        line,
        &[
            "requested_access",
            "requested",
            "accesses",
            "access",
            "denied_access",
        ],
    )?;
    let syscall = first_field(line, &["syscall", "syscall_name", "op"])?;
    let pid = field_value(line, "pid").and_then(|pid| pid.parse::<u32>().ok());
    let parent_pid =
        first_field(line, &["ppid", "parent_pid"]).and_then(|pid| pid.parse::<u32>().ok());
    let audit_epoch_millis = audit_timestamp_millis(line);

    Some(LandlockAuditRecord {
        pid,
        parent_pid,
        audit_epoch_millis,
        denied_path,
        requested_access,
        syscall,
    })
}

pub fn parse_available_landlock_records<S>(source: &mut S) -> io::Result<Vec<LandlockAuditRecord>>
where
    S: AuditRecordSource + ?Sized,
{
    source.read_available_records().map(|lines| {
        lines
            .iter()
            .filter_map(|line| parse_audit_record(line))
            .collect()
    })
}

pub fn landlock_denied_payload(
    context: &LandlockAuditContext,
    record: &LandlockAuditRecord,
) -> String {
    #[derive(Serialize)]
    struct Payload<'a> {
        tool_name: &'a str,
        agent_id: String,
        wake_id: Option<String>,
        correlation_pids: &'a [u32],
        audit_pid: Option<u32>,
        audit_parent_pid: Option<u32>,
        audit_epoch_millis: Option<u128>,
        denied_path: &'a str,
        requested_access: &'a str,
        syscall: &'a str,
    }

    let payload = Payload {
        tool_name: &context.tool_name,
        agent_id: context.agent_id.to_string(),
        wake_id: context.wake_id.map(|id| id.to_string()),
        correlation_pids: &context.audit_pids,
        audit_pid: record.pid,
        audit_parent_pid: record.parent_pid,
        audit_epoch_millis: record.audit_epoch_millis,
        denied_path: &record.denied_path,
        requested_access: &record.requested_access,
        syscall: &record.syscall,
    };
    serde_json::to_string(&payload).expect("landlock_denied payload serializes")
}

pub fn audit_record_matches_context(
    context: &LandlockAuditContext,
    record: &LandlockAuditRecord,
) -> bool {
    audit_pid_matches_context(context, record) && audit_time_matches_context(context, record)
}

fn audit_pid_matches_context(context: &LandlockAuditContext, record: &LandlockAuditRecord) -> bool {
    record
        .pid
        .is_some_and(|found| context.audit_pids.contains(&found))
        || record
            .parent_pid
            .is_some_and(|found| context.audit_pids.contains(&found))
}

fn audit_time_matches_context(
    context: &LandlockAuditContext,
    record: &LandlockAuditRecord,
) -> bool {
    if context.invocation_started_at_millis.is_none()
        && context.invocation_finished_at_millis.is_none()
    {
        return true;
    }

    let Some(record_millis) = record.audit_epoch_millis else {
        return false;
    };
    if let Some(started_at) = context.invocation_started_at_millis {
        let lower_bound = started_at.saturating_sub(AUDIT_CORRELATION_CLOCK_SKEW_MILLIS);
        if record_millis < lower_bound {
            return false;
        }
    }
    if let Some(finished_at) = context.invocation_finished_at_millis {
        let upper_bound = finished_at.saturating_add(AUDIT_CORRELATION_CLOCK_SKEW_MILLIS);
        if record_millis > upper_bound {
            return false;
        }
    }
    true
}

pub async fn append_landlock_denied_event(
    pool: &PgPool,
    context: &LandlockAuditContext,
    record: &LandlockAuditRecord,
) -> Result<Event, AppError> {
    if !audit_record_matches_context(context, record) {
        return Err(AppError::Internal(
            "refusing to append uncorrelated Landlock audit record".into(),
        ));
    }

    let payload = landlock_denied_payload(context, record);
    event::append_event(
        pool,
        context.agent_id,
        LANDLOCK_DENIED_EVENT,
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

pub async fn append_available_landlock_denials<S>(
    pool: &PgPool,
    context: &LandlockAuditContext,
    source: &mut S,
) -> Result<usize, AppError>
where
    S: AuditRecordSource + ?Sized,
{
    let records = parse_available_landlock_records(source)
        .map_err(|e| AppError::Internal(format!("read landlock audit source failed: {e}")))?;
    let mut appended = 0usize;
    for record in records {
        if !audit_record_matches_context(context, &record) {
            continue;
        }
        append_landlock_denied_event(pool, context, &record).await?;
        appended += 1;
    }
    Ok(appended)
}

pub async fn append_landlock_denials_within<S>(
    pool: &PgPool,
    context: &LandlockAuditContext,
    source: &mut S,
    window: Duration,
) -> Result<usize, AppError>
where
    S: AuditRecordSource + ?Sized,
{
    let deadline = tokio::time::Instant::now() + window;
    let mut total = 0usize;
    let mut quiet_deadline = None;
    loop {
        let appended = append_available_landlock_denials(pool, context, source).await?;
        if appended > 0 {
            total += appended;
            quiet_deadline = Some(tokio::time::Instant::now() + AUDIT_POST_APPEND_QUIET_PERIOD);
        }

        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Ok(total);
        }
        if quiet_deadline.is_some_and(|deadline| now >= deadline) {
            return Ok(total);
        }

        let next_deadline = quiet_deadline
            .map(|quiet| quiet.min(deadline))
            .unwrap_or(deadline);
        let sleep_for = next_deadline
            .saturating_duration_since(now)
            .min(AUDIT_POLL_INTERVAL);
        tokio::time::sleep(sleep_for).await;
    }
}

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
