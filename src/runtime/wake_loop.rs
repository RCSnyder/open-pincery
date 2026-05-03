use sqlx::PgPool;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn};
use uuid::Uuid;

use super::capability::PermissionMode;
use super::llm::{ChatMessage, LlmClient};
use super::prompt::{self, WakePromptContext};
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

    // AC-79 (T-AC79-1): mint per-wake prompt-injection defense context.
    // Both nonce and canary are 16-byte hex strings drawn from the OS CSPRNG,
    // held only on the stack for the duration of this wake, and never
    // persisted to any event/audit/projection row.
    let prompt_ctx = mint_wake_prompt_context();

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

    // AC-79 (T-AC79-3 / G4b): emit one `prompt_injection_canary_emitted`
    // event per wake. Source is `"runtime"`. The canary VALUE is never
    // written — only the wake_id column persists, proving "a canary was
    // generated for this wake" without leaking it. Operators auditing
    // AC-79 readiness can grep `event_type = 'prompt_injection_canary_emitted'`
    // and confirm one row per wake.
    event::append_event(
        pool,
        agent_id,
        "prompt_injection_canary_emitted",
        "runtime",
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
            &prompt_ctx,
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

        // AC-79 (T-AC79-4..6 / G4c): scan the LLM response for the per-wake
        // canary value BEFORE any `assistant_message` / `tool_call` event
        // append. The canary appears only in the legitimate system prompt;
        // any echo in the model's response (content, or any tool-call
        // function name/arguments/id) is positive proof of prompt
        // injection. Terminate the wake immediately, BEFORE running any
        // tool, so the attacker's instruction never reaches the sandbox.
        if let Some(echo) = scan_for_canary(&response, &prompt_ctx.canary_hex) {
            warn!(
                agent_id = %agent_id,
                wake_id = %wake_id,
                where_found = %echo.where_found,
                "AC-79: canary echo detected in LLM response — prompt injection suspected"
            );
            // The `content` column records WHERE the echo was found
            // (NOT the canary value itself, NOT the surrounding bytes).
            // The harness response is discarded — no `assistant_message`
            // or `tool_call` event is appended.
            event::append_event(
                pool,
                agent_id,
                "prompt_injection_suspected",
                "runtime",
                Some(wake_id),
                None,
                None,
                None,
                Some(&echo.where_found),
                None,
            )
            .await?;
            termination_reason = "prompt_injection_suspected".to_string();
            break;
        }

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

/// AC-79 (T-AC79-1): mint a fresh `WakePromptContext` for one wake.
///
/// Each wake gets a unique 16-byte (32 hex chars) nonce and canary drawn
/// from the OS CSPRNG. They live on the stack for exactly one wake and
/// are never persisted, logged, or returned across the function boundary.
fn mint_wake_prompt_context() -> WakePromptContext {
    use rand::Rng;
    let mut rng = rand::rng();
    let nonce_bytes: [u8; 16] = rng.random();
    let canary_bytes: [u8; 16] = rng.random();
    WakePromptContext {
        wake_nonce: hex::encode(nonce_bytes),
        canary_hex: hex::encode(canary_bytes),
    }
}

/// AC-79 (T-AC79-4..6 / G4c): result of [`scan_for_canary`].
///
/// `where_found` is a short, audit-friendly tag indicating WHICH field of
/// the LLM response contained the canary echo. It is INTENTIONALLY a tag,
/// not a slice of the surrounding bytes — recording the surrounding bytes
/// risks re-introducing the canary value (or attacker-controlled content)
/// into the event log. The tag is one of:
///
/// - `"choice[N].content"`
/// - `"choice[N].tool_calls[M].function.name"`
/// - `"choice[N].tool_calls[M].function.arguments"`
/// - `"choice[N].tool_calls[M].id"`
struct CanaryEcho {
    where_found: String,
}

/// AC-79 (T-AC79-4..6 / G4c): scan a `ChatResponse` for the per-wake
/// canary value across every field a model could place output into:
/// each choice's text content and every tool call's function name,
/// function arguments, and call id.
///
/// Returns `Some(CanaryEcho{ where_found })` on the FIRST match. The
/// caller is expected to terminate the wake immediately and emit
/// `prompt_injection_suspected` BEFORE running any tool.
fn scan_for_canary(response: &super::llm::ChatResponse, canary_hex: &str) -> Option<CanaryEcho> {
    for (ci, choice) in response.choices.iter().enumerate() {
        if let Some(content) = &choice.message.content {
            if content.contains(canary_hex) {
                return Some(CanaryEcho {
                    where_found: format!("choice[{ci}].content"),
                });
            }
        }
        if let Some(tcs) = &choice.message.tool_calls {
            for (ti, tc) in tcs.iter().enumerate() {
                if tc.function.name.contains(canary_hex) {
                    return Some(CanaryEcho {
                        where_found: format!("choice[{ci}].tool_calls[{ti}].function.name"),
                    });
                }
                if tc.function.arguments.contains(canary_hex) {
                    return Some(CanaryEcho {
                        where_found: format!("choice[{ci}].tool_calls[{ti}].function.arguments"),
                    });
                }
                if tc.id.contains(canary_hex) {
                    return Some(CanaryEcho {
                        where_found: format!("choice[{ci}].tool_calls[{ti}].id"),
                    });
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::llm::{
        ChatResponse, Choice, FunctionCall, ResponseMessage, ToolCallRequest,
    };

    fn mk_response(
        content: Option<&str>,
        tool_calls: Option<Vec<ToolCallRequest>>,
    ) -> ChatResponse {
        ChatResponse {
            id: "test".into(),
            choices: vec![Choice {
                message: ResponseMessage {
                    role: "assistant".into(),
                    content: content.map(|s| s.to_string()),
                    tool_calls,
                },
                finish_reason: "stop".into(),
            }],
            usage: None,
        }
    }

    #[test]
    fn mint_wake_prompt_context_produces_distinct_32_hex_pairs() {
        let a = mint_wake_prompt_context();
        let b = mint_wake_prompt_context();
        assert_eq!(a.wake_nonce.len(), 32);
        assert_eq!(a.canary_hex.len(), 32);
        assert!(a.wake_nonce.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(a.canary_hex.chars().all(|c| c.is_ascii_hexdigit()));
        // Per-wake regeneration: collision probability is 2^-128 per field.
        assert_ne!(a.wake_nonce, b.wake_nonce);
        assert_ne!(a.canary_hex, b.canary_hex);
        // Nonce and canary are independent draws within the same wake.
        assert_ne!(a.wake_nonce, a.canary_hex);
    }

    #[test]
    fn scan_for_canary_returns_none_on_clean_response() {
        let r = mk_response(Some("hello world, no canary here"), None);
        assert!(scan_for_canary(&r, "deadbeefcafebabe1122334455667788").is_none());
    }

    #[test]
    fn scan_for_canary_detects_echo_in_content() {
        let canary = "0011223344556677aabbccddeeff0011";
        let r = mk_response(Some(&format!("here is the secret: {canary}")), None);
        let echo = scan_for_canary(&r, canary).expect("must detect echo");
        assert_eq!(echo.where_found, "choice[0].content");
    }

    #[test]
    fn scan_for_canary_detects_echo_in_tool_call_arguments() {
        let canary = "0011223344556677aabbccddeeff0011";
        let r = mk_response(
            Some("running tool"),
            Some(vec![ToolCallRequest {
                id: "tc-0".into(),
                call_type: "function".into(),
                function: FunctionCall {
                    name: "shell".into(),
                    arguments: format!("{{\"cmd\":\"echo {canary}\"}}"),
                },
            }]),
        );
        let echo = scan_for_canary(&r, canary).expect("must detect echo");
        assert_eq!(
            echo.where_found,
            "choice[0].tool_calls[0].function.arguments"
        );
    }

    #[test]
    fn scan_for_canary_detects_echo_in_tool_call_name() {
        let canary = "0011223344556677aabbccddeeff0011";
        let r = mk_response(
            None,
            Some(vec![ToolCallRequest {
                id: "tc-0".into(),
                call_type: "function".into(),
                function: FunctionCall {
                    name: format!("shell_{canary}"),
                    arguments: "{}".into(),
                },
            }]),
        );
        let echo = scan_for_canary(&r, canary).expect("must detect echo");
        assert_eq!(echo.where_found, "choice[0].tool_calls[0].function.name");
    }

    #[test]
    fn scan_for_canary_detects_echo_in_tool_call_id() {
        let canary = "0011223344556677aabbccddeeff0011";
        let r = mk_response(
            None,
            Some(vec![ToolCallRequest {
                id: format!("tc-{canary}"),
                call_type: "function".into(),
                function: FunctionCall {
                    name: "shell".into(),
                    arguments: "{}".into(),
                },
            }]),
        );
        let echo = scan_for_canary(&r, canary).expect("must detect echo");
        assert_eq!(echo.where_found, "choice[0].tool_calls[0].id");
    }
}
