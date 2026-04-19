use sqlx::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

use super::llm::{ChatMessage, LlmClient};
use super::prompt;
use super::tools::{self, ToolResult};
use crate::config::Config;
use crate::error::AppError;
use crate::models::{agent, event, llm_call};

/// Run the full wake loop for an agent that has already been CAS-acquired.
pub async fn run_wake_loop(
    pool: &PgPool,
    llm: &LlmClient,
    config: &Config,
    agent_id: Uuid,
    wake_id: Uuid,
) -> Result<String, AppError> {
    info!(agent_id = %agent_id, wake_id = %wake_id, "Starting wake loop");

    // Record wake_start event
    event::append_event(
        pool, agent_id, "wake_start", "agent", Some(wake_id),
        None, None, None, None, None,
    ).await?;

    #[allow(unused_assignments)]
    let mut termination_reason = String::new();

    loop {
        // Check iteration cap
        let current = agent::get_agent(pool, agent_id)
            .await?
            .ok_or(AppError::NotFound("Agent disappeared".into()))?;
        if current.wake_iteration_count >= config.iteration_cap as i32 {
            termination_reason = "iteration_cap".to_string();
            warn!(agent_id = %agent_id, "Hit iteration cap");
            break;
        }

        // Assemble prompt
        let assembled = prompt::assemble_prompt(
            pool,
            agent_id,
            config.event_window_limit as i64,
            config.wake_summary_limit as i64,
            config.max_prompt_chars,
        )
        .await?;

        // Build messages with system prompt
        let mut messages = vec![ChatMessage {
            role: "system".into(),
            content: Some(assembled.system_prompt),
            tool_calls: None,
            tool_call_id: None,
        }];
        messages.extend(assembled.messages);

        // Call LLM
        let response = match llm.chat(messages.clone(), Some(assembled.tools), None).await {
            Ok(r) => r,
            Err(e) => {
                warn!(agent_id = %agent_id, error = %e, "LLM call failed");
                termination_reason = "llm_error".to_string();
                break;
            }
        };

        // Record LLM call
        let usage = response.usage.as_ref();
        let prompt_pairs: Vec<(String, String)> = messages
            .iter()
            .map(|m| (m.role.clone(), m.content.clone().unwrap_or_default()))
            .collect();
        llm_call::insert_llm_call(
            pool,
            agent_id,
            wake_id,
            &llm.model,
            "wake_loop",
            usage.map(|u| u.prompt_tokens),
            usage.map(|u| u.completion_tokens),
            None,
            &prompt_pairs,
        )
        .await?;

        let choice = match response.choices.first() {
            Some(c) => c,
            None => {
                termination_reason = "empty_response".to_string();
                break;
            }
        };

        // Handle text response
        if let Some(text) = &choice.message.content {
            event::append_event(
                pool,
                agent_id,
                "assistant_message",
                "agent",
                Some(wake_id),
                None,
                None,
                None,
                Some(text),
                None,
            )
            .await?;
        }

        // Handle tool calls
        if let Some(tool_calls) = &choice.message.tool_calls {
            for tc in tool_calls {
                // Record tool call event
                event::append_event(
                    pool,
                    agent_id,
                    "tool_call",
                    "agent",
                    Some(wake_id),
                    Some(&tc.function.name),
                    Some(&tc.function.arguments),
                    None,
                    None,
                    None,
                )
                .await?;

                // Dispatch tool
                let result = tools::dispatch_tool(tc).await;

                match result {
                    ToolResult::Sleep => {
                        event::append_event(
                            pool,
                            agent_id,
                            "tool_result",
                            "agent",
                            Some(wake_id),
                            Some("sleep"),
                            None,
                            Some("going to sleep"),
                            None,
                            None,
                        )
                        .await?;
                        termination_reason = "sleep".to_string();
                        // Record wake end
                        event::append_event(
                            pool,
                            agent_id,
                            "wake_end",
                            "agent",
                            Some(wake_id),
                            None,
                            None,
                            None,
                            None,
                            Some(&termination_reason),
                        )
                        .await?;
                        return Ok(termination_reason);
                    }
                    ToolResult::Output(output) => {
                        event::append_event(
                            pool,
                            agent_id,
                            "tool_result",
                            "agent",
                            Some(wake_id),
                            Some(&tc.function.name),
                            None,
                            Some(&output),
                            None,
                            None,
                        )
                        .await?;
                    }
                    ToolResult::Error(err) => {
                        event::append_event(
                            pool,
                            agent_id,
                            "tool_result",
                            "agent",
                            Some(wake_id),
                            Some(&tc.function.name),
                            None,
                            Some(&err),
                            None,
                            None,
                        )
                        .await?;
                    }
                }

                // Increment iteration
                agent::increment_iteration(pool, agent_id).await?;
            }
        } else if choice.finish_reason == "stop" {
            // No tool calls and stop — agent is done
            termination_reason = "completed".to_string();
            break;
        }
    }

    // Record wake end event
    event::append_event(
        pool,
        agent_id,
        "wake_end",
        "agent",
        Some(wake_id),
        None,
        None,
        None,
        None,
        Some(&termination_reason),
    )
    .await?;

    info!(agent_id = %agent_id, wake_id = %wake_id, reason = %termination_reason, "Wake loop ended");
    Ok(termination_reason)
}
