use sqlx::PgPool;
use uuid::Uuid;

use super::llm::ChatMessage;
use crate::error::AppError;
use crate::models::{event, projection, prompt_template};

pub struct AssembledPrompt {
    pub system_prompt: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<super::llm::ToolDefinition>,
}

pub async fn assemble_prompt(
    pool: &PgPool,
    agent_id: Uuid,
    event_window: i64,
    wake_summary_limit: i64,
    max_prompt_chars: usize,
) -> Result<AssembledPrompt, AppError> {
    // 1. Load system prompt template
    let template = prompt_template::find_active(pool, "wake_system_prompt")
        .await?
        .ok_or_else(|| AppError::Internal("Missing wake_system_prompt template".into()))?;

    let mut system_prompt = template.template;

    // 2. Add current time
    system_prompt.push_str(&format!(
        "\n\nCurrent time: {}",
        chrono::Utc::now().to_rfc3339()
    ));

    // 3. Add agent identity and work list from latest projection
    if let Some(proj) = projection::latest_projection(pool, agent_id).await? {
        system_prompt.push_str(&format!("\n\n## Identity\n{}", proj.identity));
        system_prompt.push_str(&format!("\n\n## Work List\n{}", proj.work_list.clone()));
    }

    // 4. Add wake summaries
    let summaries = projection::recent_wake_summaries(pool, agent_id, wake_summary_limit).await?;
    if !summaries.is_empty() {
        system_prompt.push_str("\n\n## Recent Wake Summaries");
        for s in summaries.iter().rev() {
            system_prompt.push_str(&format!("\n- {}", s.summary));
        }
    }

    // 5. Truncate system prompt if needed
    if system_prompt.len() > max_prompt_chars {
        system_prompt.truncate(max_prompt_chars);
        system_prompt.push_str("\n...[truncated]");
    }

    // 6. Convert recent events to chat messages
    let events = event::recent_events(pool, agent_id, event_window).await?;
    let mut messages = Vec::new();
    let mut last_tool_call_id: Option<String> = None;

    // Events come in DESC order, reverse for chronological
    for ev in events.into_iter().rev() {
        match ev.event_type.as_str() {
            "message_received" => {
                messages.push(ChatMessage {
                    role: "user".into(),
                    content: ev.content,
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
            "tool_call" => {
                if let (Some(tool_name), Some(tool_input)) = (&ev.tool_name, &ev.tool_input) {
                    let tool_call_id = ev.id.to_string();
                    last_tool_call_id = Some(tool_call_id.clone());
                    messages.push(ChatMessage {
                        role: "assistant".into(),
                        content: None,
                        tool_calls: Some(vec![super::llm::ToolCallRequest {
                            id: tool_call_id,
                            call_type: "function".into(),
                            function: super::llm::FunctionCall {
                                name: tool_name.clone(),
                                arguments: tool_input.to_string(),
                            },
                        }]),
                        tool_call_id: None,
                    });
                }
            }
            "tool_result" => {
                let output = ev.tool_output.or(ev.content).unwrap_or_default();
                // Find matching tool_call event ID
                let tool_call_id = last_tool_call_id.take().unwrap_or_default();
                messages.push(ChatMessage {
                    role: "tool".into(),
                    content: Some(output),
                    tool_calls: None,
                    tool_call_id: Some(tool_call_id),
                });
            }
            "assistant_message" => {
                messages.push(ChatMessage {
                    role: "assistant".into(),
                    content: ev.content,
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
            _ => {} // Skip other event types
        }
    }

    let tools = super::tools::tool_definitions();

    Ok(AssembledPrompt {
        system_prompt,
        messages,
        tools,
    })
}
