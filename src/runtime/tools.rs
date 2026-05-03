use super::capability::{self, PermissionMode};
use super::llm::{FunctionDef, ToolCallRequest, ToolDefinition};
use super::sandbox::{ExecResult, SandboxProfile, ShellCommand, ToolExecutor};
use super::vault::{SealedCredential, Vault};
use crate::error::AppError;
use crate::models::{credential, event};
#[cfg(target_os = "linux")]
use crate::observability::landlock_audit;
use crate::observability::metrics as m;
#[cfg(target_os = "linux")]
use crate::observability::seccomp_audit;
#[cfg(target_os = "linux")]
use crate::runtime::sandbox::preflight::{KernelProbe, RealKernelProbe};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

/// AC-43 (v7): prefix tagging env-var values that must be resolved from
/// the workspace credential vault before the child process is spawned.
/// Anything after the colon is the credential name; it MUST match
/// [`crate::models::credential::validate_name`] (`[a-z0-9_]{1,64}`).
pub const PLACEHOLDER_PREFIX: &str = "PLACEHOLDER:";

pub enum ToolResult {
    Output(String),
    Sleep,
    Error(String),
}

/// AC-79 (v9 Phase G G4d): JSON-Schema validation of LLM tool-call
/// arguments before [`dispatch_tool`] is allowed to run. Returns:
/// - `Ok(())` if `tool_name` is registered AND the arguments string
///   parses as JSON AND the parsed value satisfies the tool's
///   `parameters` schema from [`tool_definitions`].
/// - `Err(why)` with a human-readable reason otherwise. The wake loop
///   uses this string as the `model_response_schema_invalid` event's
///   `content` payload (NOT the arguments themselves — those bytes
///   may be attacker-controlled and live too long in `events`).
///
/// Validators are compiled once per process from `tool_definitions()`
/// and cached. `tool_definitions()` is the single source of truth for
/// tool argument schemas — the existing `serde_json::from_str::<ShellArgs>`
/// inside `dispatch_tool` stays as defense-in-depth (Rust-shape binding)
/// and runs strictly downstream of this validator.
pub fn validate_tool_call_arguments(tool_name: &str, arguments: &str) -> Result<(), String> {
    use std::sync::OnceLock;
    static CACHE: OnceLock<HashMap<String, jsonschema::Validator>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        let mut m = HashMap::new();
        for def in tool_definitions() {
            // `jsonschema::validator_for` infers the draft from `$schema`
            // or falls back to draft 2020-12. Tool schemas are
            // hand-authored object shapes; if compilation ever fails
            // we fail-closed by simply not inserting the entry, which
            // forces the validator to report the tool as unknown.
            if let Ok(v) = jsonschema::validator_for(&def.function.parameters) {
                m.insert(def.function.name.clone(), v);
            }
        }
        m
    });

    let validator = cache
        .get(tool_name)
        .ok_or_else(|| format!("unknown tool name: {tool_name}"))?;
    let parsed: serde_json::Value = serde_json::from_str(arguments)
        .map_err(|e| format!("tool arguments are not valid JSON: {e}"))?;
    if !validator.is_valid(&parsed) {
        // Collect the first schema error for an actionable message.
        // Boundaries are tight: we record the structural reason, not
        // the offending value bytes (which may be attacker-controlled).
        let first = validator.iter_errors(&parsed).next();
        let reason = first
            .map(|e| format!("{} at {}", e, e.instance_path))
            .unwrap_or_else(|| "schema mismatch (no detail)".to_string());
        return Err(format!("schema mismatch for tool {tool_name}: {reason}"));
    }
    Ok(())
}

#[cfg(test)]
mod validator_tests {
    use super::*;

    #[test]
    fn shell_call_with_command_passes() {
        assert!(validate_tool_call_arguments("shell", r#"{"command":"ls"}"#).is_ok());
    }

    #[test]
    fn shell_call_missing_command_fails() {
        let err = validate_tool_call_arguments("shell", "{}").unwrap_err();
        assert!(
            err.contains("schema mismatch for tool shell"),
            "expected schema mismatch error, got: {err}"
        );
    }

    #[test]
    fn shell_call_with_non_json_fails() {
        let err = validate_tool_call_arguments("shell", "not-json").unwrap_err();
        assert!(err.contains("not valid JSON"));
    }

    #[test]
    fn unknown_tool_name_fails() {
        let err = validate_tool_call_arguments("rm_rf", r#"{"path":"/"}"#).unwrap_err();
        assert!(err.contains("unknown tool name: rm_rf"));
    }

    #[test]
    fn sleep_with_no_args_passes() {
        assert!(validate_tool_call_arguments("sleep", "{}").is_ok());
    }

    #[test]
    fn plan_requires_content() {
        assert!(validate_tool_call_arguments("plan", r#"{"content":"x"}"#).is_ok());
        assert!(validate_tool_call_arguments("plan", "{}").is_err());
    }
}

pub fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDef {
                name: "shell".into(),
                description: "Execute a shell command and return stdout/stderr. Use the optional `env` map to inject environment variables into the child process. To use a stored credential, set an env value to `PLACEHOLDER:<credential_name>` and the runtime will substitute the decrypted value at exec time — the plaintext never passes through the model.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The shell command to execute"
                        },
                        "env": {
                            "type": "object",
                            "description": "Environment variables for the child process. Values of the form `PLACEHOLDER:<name>` are resolved from the workspace credential vault.",
                            "additionalProperties": {"type": "string"}
                        }
                    },
                    "required": ["command"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDef {
                name: "plan".into(),
                description: "Record a plan or intention. This is a no-op tool for the agent to express its reasoning.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "content": {
                            "type": "string",
                            "description": "The plan or intention to record"
                        }
                    },
                    "required": ["content"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDef {
                name: "sleep".into(),
                description: "End the current wake cycle. Call this when there is nothing more to do.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        // AC-41 (v7): list active credential NAMES for the current
        // workspace. The response never contains ciphertext, nonces,
        // or plaintext values — agents learn only which credentials
        // exist so they can construct `PLACEHOLDER:<name>` tokens
        // (AC-43) and let the sandbox resolve them at exec time.
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDef {
                name: "list_credentials".into(),
                description: "List the names of credentials stored in the workspace vault. Returns only names and metadata; values are never returned. To USE a credential, place `PLACEHOLDER:<name>` into a shell command's env and the runtime will substitute the value at exec time.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
    ]
}

#[derive(Deserialize)]
struct ShellArgs {
    command: String,
    /// AC-43 (v7): optional env map. Values may contain
    /// `PLACEHOLDER:<name>` tokens that are resolved against the
    /// workspace credential vault before the child is spawned.
    #[serde(default)]
    env: HashMap<String, String>,
}

#[derive(Deserialize)]
struct PlanArgs {
    content: String,
}

// Intentional: this is the single tool-dispatch seam and each argument
// is a distinct authorization/identity/capability concern (see AC-35,
// AC-41, AC-43 in scaffolding/design.md). Grouping into a struct would
// hide those concerns without clarifying them.
#[allow(clippy::too_many_arguments)]
pub async fn dispatch_tool(
    tool_call: &ToolCallRequest,
    mode: PermissionMode,
    pool: &PgPool,
    agent_id: Uuid,
    workspace_id: Uuid,
    wake_id: Uuid,
    executor: &Arc<dyn ToolExecutor>,
    vault: &Arc<Vault>,
) -> ToolResult {
    let name = &tool_call.function.name;
    let args = &tool_call.function.arguments;
    metrics::counter!(m::TOOL_CALL, "tool" => name.clone()).increment(1);

    // AC-35: capability gate. Runs BEFORE any side effect so a denied call
    // never spawns a process or reaches the network. Unknown tools land on
    // ToolCapability::Destructive (closed-by-default).
    let required = capability::required_for(name);
    if !capability::mode_allows(mode, required) {
        warn!(
            tool = %name,
            required_capability = ?required,
            permission_mode = ?mode,
            "Tool call denied by permission mode"
        );
        let payload = json!({
            "required_capability": required,
            "permission_mode": mode,
        })
        .to_string();
        // Best-effort audit trail. If the insert fails we still return
        // Error so the wake loop does not accidentally proceed with the
        // disallowed call.
        if let Err(e) = append_denied_event(pool, agent_id, wake_id, name, &payload).await {
            warn!(error = %e, "Failed to append tool_capability_denied event");
        }
        return ToolResult::Error("tool disallowed by permission mode".into());
    }

    match name.as_str() {
        "shell" => {
            let parsed: ShellArgs = match serde_json::from_str(args) {
                Ok(a) => a,
                Err(e) => return ToolResult::Error(format!("Invalid shell args: {e}")),
            };
            info!(command = %parsed.command, env_keys = parsed.env.len(), "Executing shell tool");

            // AC-43 (v7): resolve PLACEHOLDER:<name> env values from the
            // workspace vault BEFORE the child is spawned. Plaintext
            // never enters tool_call events, logs, or return values.
            // A missing or revoked credential emits a
            // `credential_unresolved` event (name only, no value) and
            // fails the tool call closed.
            let resolved_env = match resolve_env_placeholders(
                &parsed.env,
                pool,
                vault,
                agent_id,
                wake_id,
                workspace_id,
            )
            .await
            {
                Ok(map) => map,
                Err(msg) => return ToolResult::Error(msg),
            };

            #[cfg(target_os = "linux")]
            let mut audit_source = {
                let landlock_abi = RealKernelProbe.landlock_abi();
                if let Some(unavailable) =
                    landlock_audit::audit_log_unavailable_for_abi(landlock_abi)
                {
                    if landlock_audit::audit_source_warning_should_emit_once() {
                        warn!(
                            event = landlock_audit::AUDIT_LOG_UNAVAILABLE_EVENT,
                            landlock_abi = ?unavailable.landlock_abi,
                            required_abi = unavailable.required_abi,
                            reason = %unavailable.reason,
                            tool = %name,
                            "Landlock audit reader disabled; sandbox enforcement continues"
                        );
                    }
                    None
                } else {
                    match landlock_audit::invocation_audit_source_from_end() {
                        Ok(source) => Some(source),
                        Err(e) => {
                            if landlock_audit::audit_source_warning_should_emit_once() {
                                warn!(
                                    event = landlock_audit::AUDIT_LOG_UNAVAILABLE_EVENT,
                                    error = %e,
                                    tool = %name,
                                    "Landlock audit source unavailable; sandbox enforcement continues"
                                );
                            }
                            None
                        }
                    }
                }
            };

            #[cfg(target_os = "linux")]
            let invocation_started_at_millis = landlock_audit::current_epoch_millis();

            let execution = execute_shell(executor, parsed.command, resolved_env).await;

            #[cfg(target_os = "linux")]
            let invocation_finished_at_millis = landlock_audit::current_epoch_millis();

            #[cfg(target_os = "linux")]
            if let Some(source) = audit_source.as_mut() {
                let audit_context = landlock_audit::LandlockAuditContext {
                    agent_id,
                    wake_id: Some(wake_id),
                    tool_name: name.clone(),
                    audit_pids: execution.audit_pids.clone(),
                    invocation_started_at_millis,
                    invocation_finished_at_millis,
                };
                match landlock_audit::append_landlock_denials_within(
                    pool,
                    &audit_context,
                    source,
                    std::time::Duration::from_secs(2),
                )
                .await
                {
                    Ok(appended) if appended > 0 => {
                        info!(count = appended, tool = %name, "Appended Landlock denial audit events");
                    }
                    Ok(_) => {}
                    Err(e) if landlock_audit::audit_source_warning_should_emit_once() => {
                        warn!(
                            event = landlock_audit::AUDIT_LOG_UNAVAILABLE_EVENT,
                            error = %e,
                            tool = %name,
                            "Landlock audit source unavailable; sandbox enforcement continues"
                        );
                    }
                    Err(_) => {}
                }
            }

            // AC-77 / Slice G2c: SIGSYS-induced termination signals a
            // seccomp default-deny hit. Emit a `sandbox_syscall_denied`
            // event with `syscall_nr = -1` fallback. Audit-netlink
            // AUDIT_SECCOMP correlation lands in the follow-up sub-slice.
            #[cfg(target_os = "linux")]
            if execution.exit_code == seccomp_audit::SIGSYS_EXIT_CODE {
                let seccomp_ctx = seccomp_audit::SeccompAuditContext {
                    agent_id,
                    wake_id: Some(wake_id),
                    tool_name: name.clone(),
                    audit_pids: execution.audit_pids.clone(),
                };
                match seccomp_audit::append_sandbox_syscall_denied_event(pool, &seccomp_ctx, None)
                    .await
                {
                    Ok(_) => {
                        info!(
                            event = seccomp_audit::SANDBOX_SYSCALL_DENIED_EVENT,
                            tool = %name,
                            exit_code = execution.exit_code,
                            "Appended sandbox_syscall_denied event (SIGSYS observed; syscall_nr=-1 fallback)"
                        );
                    }
                    Err(e) => warn!(
                        error = %e,
                        tool = %name,
                        "Failed to append sandbox_syscall_denied event"
                    ),
                }
            }

            execution.result
        }
        "plan" => {
            let parsed: PlanArgs = match serde_json::from_str(args) {
                Ok(a) => a,
                Err(e) => return ToolResult::Error(format!("Invalid plan args: {e}")),
            };
            info!(plan = %parsed.content, "Plan recorded");
            ToolResult::Output(format!("Plan recorded: {}", parsed.content))
        }
        "sleep" => {
            info!("Agent called sleep tool");
            ToolResult::Sleep
        }
        // AC-41 (v7): vault projection. Returns `{"credentials": [{"name": "...",
        // "created_at": "..."}]}` — names only. We deliberately do not
        // expose ciphertext or plaintext. Failures surface as a benign
        // ToolResult::Error so the model can retry or sleep.
        "list_credentials" => {
            match crate::models::credential::list_active(pool, workspace_id).await {
                Ok(rows) => {
                    let names: Vec<_> = rows
                        .into_iter()
                        .map(|r| {
                            json!({
                                "name": r.name,
                                "created_at": r.created_at,
                            })
                        })
                        .collect();
                    let body = json!({ "credentials": names }).to_string();
                    ToolResult::Output(body)
                }
                Err(e) => {
                    warn!(error = %e, "list_credentials tool failed");
                    ToolResult::Error(format!("list_credentials failed: {e}"))
                }
            }
        }
        other => ToolResult::Error(format!("Unknown tool: {other}")),
    }
}

async fn append_denied_event(
    pool: &PgPool,
    agent_id: Uuid,
    wake_id: Uuid,
    tool_name: &str,
    payload: &str,
) -> Result<(), AppError> {
    event::append_event(
        pool,
        agent_id,
        "tool_capability_denied",
        "runtime",
        Some(wake_id),
        Some(tool_name),
        Some(payload),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

/// AC-43 (v7): walk the shell env map and replace every
/// `PLACEHOLDER:<name>` value with the decrypted plaintext from the
/// workspace vault. On ANY failure (missing, revoked, auth failure,
/// invalid name, non-UTF-8 plaintext) emit a `credential_unresolved`
/// event (name + reason only — never the value) and return a
/// caller-visible error. Non-PLACEHOLDER values pass through unchanged.
async fn resolve_env_placeholders(
    env: &HashMap<String, String>,
    pool: &PgPool,
    vault: &Arc<Vault>,
    agent_id: Uuid,
    wake_id: Uuid,
    workspace_id: Uuid,
) -> Result<HashMap<String, String>, String> {
    let mut resolved: HashMap<String, String> = HashMap::with_capacity(env.len());
    for (key, value) in env {
        if let Some(name) = value.strip_prefix(PLACEHOLDER_PREFIX) {
            match credential::find_active(pool, workspace_id, name).await {
                Ok(Some(row)) => {
                    // Convert DB Vec<u8> nonce → fixed [u8;12]. A malformed
                    // row is treated as an auth failure — same closed-fail
                    // path as a revoked credential.
                    let nonce_arr: [u8; 12] = match row.nonce.as_slice().try_into() {
                        Ok(a) => a,
                        Err(_) => {
                            emit_credential_unresolved(
                                pool,
                                agent_id,
                                wake_id,
                                name,
                                "invalid_nonce",
                            )
                            .await;
                            return Err(format!("credential not found: {name}"));
                        }
                    };
                    let sealed = SealedCredential {
                        nonce: nonce_arr,
                        ciphertext: row.ciphertext,
                    };
                    match vault.open(workspace_id, name, &sealed) {
                        Ok(plaintext) => match String::from_utf8(plaintext) {
                            Ok(s) => {
                                resolved.insert(key.clone(), s);
                            }
                            Err(_) => {
                                emit_credential_unresolved(
                                    pool, agent_id, wake_id, name, "non_utf8",
                                )
                                .await;
                                return Err(format!("credential not found: {name}"));
                            }
                        },
                        Err(_) => {
                            emit_credential_unresolved(
                                pool,
                                agent_id,
                                wake_id,
                                name,
                                "authentication_failed",
                            )
                            .await;
                            return Err(format!("credential not found: {name}"));
                        }
                    }
                }
                Ok(None) => {
                    emit_credential_unresolved(pool, agent_id, wake_id, name, "missing_or_revoked")
                        .await;
                    return Err(format!("credential not found: {name}"));
                }
                Err(e) => {
                    warn!(error = %e, credential_name = %name, "credential lookup failed");
                    emit_credential_unresolved(pool, agent_id, wake_id, name, "lookup_error").await;
                    return Err(format!("credential not found: {name}"));
                }
            }
        } else {
            resolved.insert(key.clone(), value.clone());
        }
    }
    Ok(resolved)
}

/// Best-effort audit trail for a failed credential resolution. Only
/// writes the name + reason — never the attempted value and never the
/// plaintext. Event log write failures are logged and swallowed so the
/// caller still gets the closed-fail path.
async fn emit_credential_unresolved(
    pool: &PgPool,
    agent_id: Uuid,
    wake_id: Uuid,
    name: &str,
    reason: &str,
) {
    let payload = json!({ "name": name, "reason": reason }).to_string();
    if let Err(e) = event::append_event(
        pool,
        agent_id,
        "credential_unresolved",
        "runtime",
        Some(wake_id),
        Some("shell"),
        Some(&payload),
        None,
        None,
        None,
    )
    .await
    {
        warn!(error = %e, credential_name = %name, reason = %reason,
              "Failed to append credential_unresolved event");
    }
}

struct ShellExecution {
    result: ToolResult,
    /// Final exit code surfaced to the caller. Signal-induced
    /// terminations are translated to `128 + signum` (POSIX
    /// convention) so SIGSYS appears as 159. -1 if no code and no
    /// signal could be observed (e.g. wait error / timeout).
    exit_code: i32,
    #[cfg(target_os = "linux")]
    audit_pids: Vec<u32>,
}

async fn execute_shell(
    executor: &Arc<dyn ToolExecutor>,
    command: String,
    env: HashMap<String, String>,
) -> ShellExecution {
    let result = executor
        .run(&ShellCommand { command, env }, &SandboxProfile::default())
        .await;

    match result {
        ExecResult::Ok {
            stdout,
            stderr,
            exit_code,
            audit_pids: _audit_pids,
        } => {
            let combined = format!("exit_code: {exit_code}\nstdout:\n{stdout}\nstderr:\n{stderr}");
            let truncated = if combined.len() > 50000 {
                let mut boundary = 50000;
                while boundary > 0 && !combined.is_char_boundary(boundary) {
                    boundary -= 1;
                }
                format!("{}...[truncated]", &combined[..boundary])
            } else {
                combined
            };
            ShellExecution {
                result: ToolResult::Output(truncated),
                exit_code,
                #[cfg(target_os = "linux")]
                audit_pids: _audit_pids,
            }
        }
        ExecResult::Timeout => ShellExecution {
            result: ToolResult::Error("Shell execution timed out".into()),
            exit_code: -1,
            #[cfg(target_os = "linux")]
            audit_pids: Vec::new(),
        },
        ExecResult::Rejected(reason) => ShellExecution {
            result: ToolResult::Error(format!("Shell execution rejected: {reason}")),
            exit_code: -1,
            #[cfg(target_os = "linux")]
            audit_pids: Vec::new(),
        },
        ExecResult::Err(e) => ShellExecution {
            result: ToolResult::Error(format!("Shell execution failed: {e}")),
            exit_code: -1,
            #[cfg(target_os = "linux")]
            audit_pids: Vec::new(),
        },
    }
}
