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

    // AC-79 (T-AC79-4..6 / G4d): per-wake counter of consecutive
    // schema-invalid LLM responses. Schema-invalid responses do NOT
    // count against `iteration_cap` (no tool dispatched, no
    // `agent.wake_iteration_count` increment) so a misbehaving model
    // cannot starve a well-behaved retry. After
    // `config.schema_invalid_retry_cap` consecutive invalids, the wake
    // terminates with `FailureAuditPending`.
    let mut schema_invalid_attempts: u32 = 0;

    // AC-79 (T-AC79-10 / G4e): per-wake counter of dispatched tool
    // calls. Distinct from `iteration_cap` (which gates wake
    // iterations / `agents.wake_iteration_count`). On exhaustion the
    // wake emits `tool_call_rate_limit_exceeded` once and terminates
    // with `FailureAuditPending`. Schema-invalid retries (T-AC79-4)
    // do NOT increment this counter — the call was never dispatched.
    let mut tool_calls_this_wake: u32 = 0;

    'wake: loop {
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
            // T-AC79-8 payload: `where_found` audit tag plus the count
            // of tool calls the model attempted in the offending
            // response (so an operator can tell whether the LLM tried
            // to smuggle 1 call or 50 — distinguishes a single-shot
            // probe from a barrage). Canary value and surrounding
            // bytes are NEVER persisted. Encoded as JSON so future
            // fields can be added without breaking parsers.
            let attempted_tool_calls = response
                .choices
                .first()
                .and_then(|c| c.message.tool_calls.as_ref().map(|t| t.len()))
                .unwrap_or(0);
            let payload = format!(
                "{{\"where_found\":{},\"model_attempted_tool_calls\":{}}}",
                serde_json::Value::String(echo.where_found.clone()),
                attempted_tool_calls,
            );
            event::append_event(
                pool,
                agent_id,
                "prompt_injection_suspected",
                "runtime",
                Some(wake_id),
                None,
                None,
                None,
                Some(&payload),
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

        // AC-79 (T-AC79-4..6 / G4d): JSON Schema validation of every
        // claimed tool call BEFORE any `tool_call` event is appended
        // and BEFORE `dispatch_tool` is invoked. The model cannot
        // smuggle malformed arguments past the schema floor.
        //
        // Aside on multi-choice: schema validation, like the dispatcher
        // below, intentionally only inspects `choices.first()` — the
        // response that the wake loop will actually consume. If
        // multi-choice dispatch is ever added, both this gate and
        // `dispatch_tool` need to widen together.
        if let Some(tcs) = &choice.message.tool_calls {
            let mut invalid: Option<(String, String)> = None; // (tool_name, why)
            for tc in tcs {
                if let Err(why) =
                    tools::validate_tool_call_arguments(&tc.function.name, &tc.function.arguments)
                {
                    invalid = Some((tc.function.name.clone(), why));
                    break;
                }
            }
            if let Some((tool_name, why)) = invalid {
                schema_invalid_attempts += 1;
                warn!(
                    agent_id = %agent_id,
                    wake_id = %wake_id,
                    attempt = schema_invalid_attempts,
                    cap = config.schema_invalid_retry_cap,
                    tool_name = %tool_name,
                    reason = %why,
                    "AC-79: model response failed JSON-Schema validation"
                );
                // T-AC79-9 payload: structured JSON with `tool_name`,
                // `schema_errors` (single-element list with the first
                // structural failure — never the offending argument
                // bytes themselves; those may be attacker-controlled),
                // `attempt` (1-indexed), and `retry_cap`. From the log
                // alone an auditor can tell whether an event was
                // attempt 1 of 3 (recoverable) or attempt 3 of 3
                // (terminal). NOTE: any text in `choice.message.content`
                // accompanying the invalid tool calls is intentionally
                // dropped — emitting an `assistant_message` for a
                // response we are about to retry would pollute the
                // audit chain with a half-emission.
                let payload = format!(
                    "{{\"tool_name\":{},\"schema_errors\":[{}],\"attempt\":{},\"retry_cap\":{}}}",
                    serde_json::Value::String(tool_name.clone()),
                    serde_json::Value::String(why.clone()),
                    schema_invalid_attempts,
                    config.schema_invalid_retry_cap,
                );
                event::append_event(
                    pool,
                    agent_id,
                    "model_response_schema_invalid",
                    "runtime",
                    Some(wake_id),
                    Some(&tool_name),
                    None,
                    None,
                    Some(&payload),
                    None,
                )
                .await?;
                if schema_invalid_attempts >= config.schema_invalid_retry_cap {
                    termination_reason = "FailureAuditPending".to_string();
                    break;
                }
                // Schema-invalid retries do NOT increment
                // `iteration_cap` (no tool dispatched). Loop back to
                // re-run the LLM with the same prompt.
                continue;
            }
            // All tool calls validated — reset the counter so a single
            // recovered response unsticks the wake.
            schema_invalid_attempts = 0;
        }

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
                // AC-79 (T-AC79-10 / G4e): per-wake tool-call rate
                // limit. Evaluated BEFORE any dispatch and BEFORE the
                // `tool_call` event append so a wake that hits the
                // cap leaves an audit log of N dispatches +
                // exactly one `tool_call_rate_limit_exceeded`. The
                // limit is independent of `iteration_cap` (different
                // quantities); both checks are evaluated and whichever
                // fires first terminates first.
                if tool_calls_this_wake >= config.tool_call_rate_limit_per_wake {
                    warn!(
                        agent_id = %agent_id,
                        wake_id = %wake_id,
                        limit = config.tool_call_rate_limit_per_wake,
                        attempted = tool_calls_this_wake + 1,
                        "AC-79: per-wake tool-call rate limit exceeded"
                    );
                    let payload = format!(
                        "{{\"limit\":{},\"attempted\":{}}}",
                        config.tool_call_rate_limit_per_wake,
                        tool_calls_this_wake + 1,
                    );
                    event::append_event(
                        pool,
                        agent_id,
                        "tool_call_rate_limit_exceeded",
                        "runtime",
                        Some(wake_id),
                        None,
                        None,
                        None,
                        Some(&payload),
                        None,
                    )
                    .await?;
                    termination_reason = "FailureAuditPending".to_string();
                    break 'wake;
                }

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
                // AC-79 (T-AC79-10): only count tool calls that
                // actually dispatched. Schema-invalid retries
                // (T-AC79-4) bypass this branch entirely.
                tool_calls_this_wake = tool_calls_this_wake.saturating_add(1);
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
    use rand::rngs::OsRng;
    use rand::TryRngCore;
    let mut nonce_bytes = [0u8; 16];
    let mut canary_bytes = [0u8; 16];
    // Direct OsRng (T-AC79-1: "derived from OsRng"). `try_fill_bytes`
    // surfaces a CSPRNG failure rather than panicking — but on every
    // supported platform it is infallible, so we just unwrap. If the
    // OS RNG genuinely fails the wake cannot proceed safely.
    OsRng
        .try_fill_bytes(&mut nonce_bytes)
        .expect("AC-79 T-AC79-1: OsRng must produce a wake nonce");
    OsRng
        .try_fill_bytes(&mut canary_bytes)
        .expect("AC-79 T-AC79-1: OsRng must produce a wake canary");
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
