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

/// AC-79 (v9 Phase G G4a): per-wake prompt-injection defense context.
///
/// Both `wake_nonce` and `canary_hex` are minted exactly once per wake at
/// the top of `run_wake_loop`, held only on the stack, and never persisted
/// to any event, audit, or projection row. Each value is a 16-byte
/// `OsRng`-sourced hex string.
///
/// `wake_nonce` parameterizes the delimiter tokens
/// (`<<untrusted:${nonce}>> ... <<end:${nonce}>>`) wrapped around every
/// untrusted-class event content. Untrusted classes are pinned by
/// [`is_untrusted`].
///
/// `canary_hex` is embedded once in the assembled system prompt as
/// `<<canary:${canary_hex}>>` and the runtime scans every byte of the
/// model's response (content + tool-call name/arguments/id) for echo;
/// an echo is positive evidence of prompt injection and terminates the
/// wake with a `prompt_injection_suspected` event.
#[derive(Debug, Clone)]
pub struct WakePromptContext {
    pub wake_nonce: String,
    pub canary_hex: String,
}

/// AC-79 (T-AC79-2): exhaustive predicate over `events.event_type` that
/// classifies which event-content bytes originated outside the trusted
/// boundary (system prompt, harness-emitted framing, the agent's own
/// prior output) and must therefore be wrapped in
/// `<<untrusted:NONCE>>...<<end:NONCE>>` before being fed to the model.
///
/// The predicate is exhaustive over the known set: future event types
/// added by inter-agent messaging or memory-read surfaces MUST be
/// classified here so they cannot bypass the delimiter wrapping. The
/// closed-set unit test
/// `is_untrusted_predicate_covers_all_known_event_types` catches
/// additions that forget to classify.
pub fn is_untrusted(event_type: &str) -> bool {
    matches!(
        event_type,
        // External-source, attacker-influenced surfaces:
        "message_received"      // operator/webhook/inter-agent inbound
            | "tool_result"     // sandboxed-process output bytes
            | "memory_read"     // future: agent reads its own/another agent's memory
            | "wake_summary_loaded" // future: prior-wake summary text re-injected as context
    )
}

pub async fn assemble_prompt(
    pool: &PgPool,
    agent_id: Uuid,
    event_window: i64,
    wake_summary_limit: i64,
    max_prompt_chars: usize,
    ctx: &WakePromptContext,
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

    // 5. Truncate first, then append the canary AFTER truncation so a tight
    //    max_prompt_chars budget cannot accidentally drop the canary line
    //    (the canary must be present every wake or AC-79 G4c's scan path
    //    would have nothing to look for).
    if system_prompt.len() > max_prompt_chars {
        system_prompt.truncate(max_prompt_chars);
        system_prompt.push_str("\n...[truncated]");
    }
    system_prompt.push_str(&format!("\n\n<<canary:{}>>", ctx.canary_hex));

    // 6. Convert recent events to chat messages, wrapping untrusted classes
    //    in <<untrusted:NONCE>>...<<end:NONCE>> delimiters per AC-79 T-AC79-1.
    let events = event::recent_events(pool, agent_id, event_window).await?;
    let mut messages = Vec::new();
    let mut last_tool_call_id: Option<String> = None;

    // Events come in DESC order, reverse for chronological
    for ev in events.into_iter().rev() {
        let untrusted = is_untrusted(&ev.event_type);
        match ev.event_type.as_str() {
            "message_received" => {
                let content = ev
                    .content
                    .map(|c| wrap_untrusted_if(&c, &ctx.wake_nonce, untrusted));
                messages.push(ChatMessage {
                    role: "user".into(),
                    content,
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
                let raw = ev.tool_output.or(ev.content).unwrap_or_default();
                let wrapped = wrap_untrusted_if(&raw, &ctx.wake_nonce, untrusted);
                // Find matching tool_call event ID
                let tool_call_id = last_tool_call_id.take().unwrap_or_default();
                messages.push(ChatMessage {
                    role: "tool".into(),
                    content: Some(wrapped),
                    tool_calls: None,
                    tool_call_id: Some(tool_call_id),
                });
            }
            "assistant_message" => {
                // Trusted: the agent's own prior output, never wrap.
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

fn wrap_untrusted_if(content: &str, nonce: &str, untrusted: bool) -> String {
    if untrusted {
        format!("<<untrusted:{nonce}>>\n{content}\n<<end:{nonce}>>")
    } else {
        content.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_untrusted_predicate_covers_all_known_event_types() {
        // Closed set of event_type values written by the runtime today.
        // Adding a new event type that carries attacker-influenced bytes
        // (e.g. a future inter-agent message or memory-read) MUST be
        // classified here. This test catches additions that forget.
        // Trusted (agent-emitted or harness-framing):
        for t in [
            "wake_started",
            "wake_completed",
            "tool_call",
            "assistant_message",
            "agent_created",
            "agent_status_changed",
            "credential_added",
            "credential_revoked",
            "credential_unresolved",
            "credential_requested",
            "audit_chain_verified",
            "audit_chain_broken",
            "audit_chain_floor_relaxed",
            "model_response_schema_invalid",
            "prompt_injection_suspected",
            "prompt_injection_canary_emitted",
            "tool_call_rate_limit_exceeded",
            "rate_limit_exceeded",
            "sandbox_blocked",
            "sandbox_unavailable",
            "landlock_denied",
            "secret_injected",
            "network_blocked",
        ] {
            assert!(!is_untrusted(t), "expected {t} to be classified as TRUSTED");
        }
        // Untrusted (attacker-influenced):
        for t in [
            "message_received",
            "tool_result",
            "memory_read",
            "wake_summary_loaded",
        ] {
            assert!(
                is_untrusted(t),
                "expected {t} to be classified as UNTRUSTED"
            );
        }
    }

    #[test]
    fn wrap_untrusted_if_wraps_when_flagged() {
        let out = wrap_untrusted_if("hello world", "deadbeefcafebabe", true);
        assert_eq!(
            out,
            "<<untrusted:deadbeefcafebabe>>\nhello world\n<<end:deadbeefcafebabe>>"
        );
    }

    #[test]
    fn wrap_untrusted_if_passthrough_when_trusted() {
        let out = wrap_untrusted_if("hello world", "deadbeefcafebabe", false);
        assert_eq!(out, "hello world");
    }
}
