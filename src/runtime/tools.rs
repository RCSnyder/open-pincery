use serde::Deserialize;
use serde_json::json;
use tracing::info;

use super::llm::{FunctionDef, ToolCallRequest, ToolDefinition};
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

pub async fn dispatch_tool(tool_call: &ToolCallRequest) -> ToolResult {
    let name = &tool_call.function.name;
    let args = &tool_call.function.arguments;
    metrics::counter!(m::TOOL_CALL, "tool" => name.clone()).increment(1);

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
