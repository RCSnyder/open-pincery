use sqlx::PgPool;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn};
use uuid::Uuid;

use super::capability::PermissionMode;
use super::llm::{ChatMessage, LlmClient};
use super::prompt;
use super::sandbox::ToolExecutor;
use super::tools::{self, ToolResult};
use super::vault::Vault;
use crate::config::Config;
use crate::error::AppError;
use crate::models::{agent, event, llm_call};
use crate::observability::metrics as m;

/// RAII guard that increments `ACTIVE_WAKES` on construction and, on drop,
/// decrements the gauge and records `WAKE_DURATION`. Ensures no termination
/// path can leak the active-wake count or skip the duration histogram.
struct WakeMetricsGuard {
    start: Instant,
}

impl WakeMetricsGuard {
    fn new() -> Self {
        metrics::gauge!(m::ACTIVE_WAKES).increment(1.0);
        Self {
            start: Instant::now(),
        }
    }
}

impl Drop for WakeMetricsGuard {
    fn drop(&mut self) {
        metrics::gauge!(m::ACTIVE_WAKES).decrement(1.0);
        metrics::histogram!(m::WAKE_DURATION).record(self.start.elapsed().as_secs_f64());
    }
}

/// Run the full wake loop for an agent that has already been CAS-acquired.
pub async fn run_wake_loop(
    pool: &PgPool,
    llm: &LlmClient,
    config: &Config,
    agent_id: Uuid,
    wake_id: Uuid,
    executor: &Arc<dyn ToolExecutor>,
    vault: &Arc<Vault>,
) -> Result<String, AppError> {
    info!(agent_id = %agent_id, wake_id = %wake_id, "Starting wake loop");
    metrics::counter!(m::WAKE_STARTED).increment(1);
    let _wake_metrics = WakeMetricsGuard::new();

    // Record wake_start event
    event::append_event(
        pool,
        agent_id,
        "wake_start",
        "agent",
        Some(wake_id),
        None,
        None,
        None,
        None,
        None,
    )
    .await?;

    #[allow(unused_assignments)]
    let mut termination_reason = String::new();

    loop {
        // Check iteration cap
        let current = agent::get_agent(pool, agent_id)
            .await?
            .ok_or(AppError::NotFound("Agent disappeared".into()))?;
        if current.wake_iteration_count >= config.iteration_cap {
            termination_reason = "iteration_cap".to_string();
            warn!(agent_id = %agent_id, "Hit iteration cap");
            break;
        }

        // Assemble prompt
        let assembled = prompt::assemble_prompt(
            pool,
            agent_id,
            config.event_window_limit,
            config.wake_summary_limit,
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
        let response = match llm
            .chat(messages.clone(), Some(assembled.tools), None)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!(agent_id = %agent_id, error = %e, "LLM call failed");
                termination_reason = "llm_error".to_string();
                break;
            }
        };

        // Record LLM call (cost attributed in the same transaction that
        // inserts the row and bumps agents.budget_used_usd — AC-23).
        let usage = response.usage.as_ref();
        let cost_usd = usage.map(|u| llm.estimate_cost(u, false));
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
            cost_usd,
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

                // Dispatch tool (AC-35: per-iteration permission-mode gate
                // runs inside dispatch_tool, using `current.permission_mode`
                // as read this tick so an operator mid-wake lockdown takes
                // effect on the next call).
                let mode = PermissionMode::from_db_str(&current.permission_mode);
                let result = tools::dispatch_tool(
                    tc,
                    mode,
                    pool,
                    agent_id,
                    current.workspace_id,
                    wake_id,
                    executor,
                    vault,
                )
                .await;

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
                        metrics::counter!(m::WAKE_COMPLETED, "reason" => termination_reason.clone()).increment(1);
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
    metrics::counter!(m::WAKE_COMPLETED, "reason" => termination_reason.clone()).increment(1);
    Ok(termination_reason)
}
