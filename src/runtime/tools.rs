use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

use super::capability::{self, PermissionMode};
use super::llm::{FunctionDef, ToolCallRequest, ToolDefinition};
use crate::error::AppError;
use crate::models::event;
use crate::observability::metrics as m;

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
                description: "Execute a shell command and return stdout/stderr.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The shell command to execute"
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
    ]
}

#[derive(Deserialize)]
struct ShellArgs {
    command: String,
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
    wake_id: Uuid,
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
            info!(command = %parsed.command, "Executing shell tool");
            execute_shell(&parsed.command).await
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

async fn execute_shell(command: &str) -> ToolResult {
    use tokio::process::Command;

    let output = Command::new("sh").arg("-c").arg(command).output().await;

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let combined = format!(
                "exit_code: {}\nstdout:\n{}\nstderr:\n{}",
                out.status.code().unwrap_or(-1),
                stdout,
                stderr
            );
            // Truncate to prevent massive outputs
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
        Err(e) => ToolResult::Error(format!("Shell execution failed: {e}")),
    }
}
