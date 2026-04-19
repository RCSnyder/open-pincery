use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

use super::llm::{ChatMessage, LlmClient};
use crate::error::AppError;
use crate::models::{event, llm_call, projection, prompt_template};

/// Run the maintenance cycle: call LLM with maintenance prompt, update projections.
pub async fn run_maintenance(
    pool: &PgPool,
    llm: &LlmClient,
    agent_id: Uuid,
    wake_id: Uuid,
) -> Result<(), AppError> {
    info!(agent_id = %agent_id, "Running maintenance");

    let template = prompt_template::find_active(pool, "maintenance_prompt")
        .await?
        .ok_or_else(|| AppError::Internal("Missing maintenance_prompt template".into()))?;

    // Gather recent events for context 
    let events = event::recent_events(pool, agent_id, 50).await?;
    let mut event_summary = String::new();
    for ev in events.iter().rev() {
        event_summary.push_str(&format!(
            "[{}] {}: {}\n",
            ev.event_type,
            ev.source,
            ev.content.as_deref().unwrap_or("")
        ));
    }

    // Get current projection
    let current_proj = projection::latest_projection(pool, agent_id).await?;
    let current_identity = current_proj
        .as_ref()
        .map(|p| p.identity.as_str())
        .unwrap_or("No identity set yet.");
    let current_work_list = current_proj
        .as_ref()
        .map(|p| serde_json::to_string_pretty(&p.work_list).unwrap_or_default())
        .unwrap_or_else(|| "[]".into());
    let current_version = current_proj.as_ref().map(|p| p.version).unwrap_or(0);

    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: Some(template.content),
            tool_calls: None,
            tool_call_id: None,
        },
        ChatMessage {
            role: "user".into(),
            content: Some(format!(
                "Current identity:\n{}\n\nCurrent work list:\n{}\n\nRecent events:\n{}",
                current_identity, current_work_list, event_summary
            )),
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let response = llm
        .chat(messages.clone(), None, Some(&llm.maintenance_model))
        .await?;

    // Record LLM call
    let usage = response.usage.as_ref();
    let prompt_pairs: Vec<(String, String)> = messages
        .iter()
        .map(|m| (m.role.clone(), m.content.clone().unwrap_or_default()))
        .collect();
    llm_call::insert_llm_call(
        pool,
        agent_id,
        Some(wake_id),
        &llm.maintenance_model,
        "maintenance",
        usage.map(|u| u.prompt_tokens),
        usage.map(|u| u.completion_tokens),
        None,
        &prompt_pairs,
    )
    .await?;

    // Parse maintenance response
    if let Some(choice) = response.choices.first() {
        if let Some(text) = &choice.message.content {
            // Try to parse as JSON
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
                let identity = parsed
                    .get("identity")
                    .and_then(|v| v.as_str())
                    .unwrap_or(current_identity);
                let work_list = parsed
                    .get("work_list")
                    .cloned()
                    .unwrap_or(serde_json::json!([]));
                let summary = parsed
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No summary provided.");

                // Insert new projection
                projection::insert_projection(
                    pool,
                    agent_id,
                    identity,
                    &work_list,
                    current_version + 1,
                )
                .await?;

                // Insert wake summary
                projection::insert_wake_summary(pool, agent_id, wake_id, summary).await?;

                info!(agent_id = %agent_id, version = current_version + 1, "Maintenance projection updated");
            } else {
                // If not valid JSON, just record as summary
                projection::insert_wake_summary(pool, agent_id, wake_id, text).await?;
            }
        }
    }

    Ok(())
}
