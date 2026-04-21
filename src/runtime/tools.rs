use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use super::capability::{self, PermissionMode};
use super::llm::{FunctionDef, ToolCallRequest, ToolDefinition};
use super::sandbox::{ExecResult, SandboxProfile, ShellCommand, ToolExecutor};
use super::vault::{SealedCredential, Vault};
use crate::error::AppError;
use crate::models::{credential, event};
use crate::observability::metrics as m;

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

            execute_shell(executor, parsed.command, resolved_env).await
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

async fn execute_shell(
    executor: &Arc<dyn ToolExecutor>,
    command: String,
    env: HashMap<String, String>,
) -> ToolResult {
    let result = executor
        .run(&ShellCommand { command, env }, &SandboxProfile::default())
        .await;

    match result {
        ExecResult::Ok {
            stdout,
            stderr,
            exit_code,
        } => {
            let combined = format!(
                "exit_code: {}\nstdout:\n{}\nstderr:\n{}",
                exit_code, stdout, stderr
            );
            let truncated = if combined.len() > 50000 {
                let mut boundary = 50000;
                while boundary > 0 && !combined.is_char_boundary(boundary) {
                    boundary -= 1;
                }
                format!("{}...[truncated]", &combined[..boundary])
            } else {
                combined
            };
            ToolResult::Output(truncated)
        }
        ExecResult::Timeout => ToolResult::Error("Shell execution timed out".into()),
        ExecResult::Rejected(reason) => {
            ToolResult::Error(format!("Shell execution rejected: {reason}"))
        }
        ExecResult::Err(e) => ToolResult::Error(format!("Shell execution failed: {e}")),
    }
}
