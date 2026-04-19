use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct LlmCall {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub wake_id: Uuid,
    pub call_type: String,
    pub model: String,
    pub prompt_hash: String,
    pub prompt_template: Option<String>,
    pub prompt_tokens: Option<i32>,
    pub completion_tokens: Option<i32>,
    pub total_tokens: Option<i32>,
    pub cost_usd: Option<rust_decimal::Decimal>,
    pub latency_ms: Option<i32>,
    pub response_hash: String,
    pub finish_reason: Option<String>,
    pub temperature: Option<f64>,
    pub created_at: DateTime<Utc>,
}

pub async fn insert_llm_call(
    pool: &PgPool,
    agent_id: Uuid,
    wake_id: Option<Uuid>,
    model: &str,
    call_type: &str,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    _duration_ms: Option<i32>,
    prompts: &[(String, String)], // (role, content)
) -> Result<Uuid, AppError> {
    let mut tx = pool.begin().await.map_err(|e| AppError::Database(e))?;

    // Compute simple hashes for audit trail
    let prompt_text: String = prompts.iter().map(|(r, c)| format!("{r}:{c}")).collect::<Vec<_>>().join("\n");
    let prompt_hash = format!("{:016x}", {
        let mut h: u64 = 0;
        for b in prompt_text.as_bytes() { h = h.wrapping_mul(31).wrapping_add(*b as u64); }
        h
    });
    let response_hash = "pending".to_string();
    let total = match (input_tokens, output_tokens) {
        (Some(i), Some(o)) => Some(i + o),
        _ => None,
    };

    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO llm_calls (agent_id, wake_id, call_type, model, prompt_hash, response_hash, prompt_tokens, completion_tokens, total_tokens)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING id"
    )
    .bind(agent_id)
    .bind(wake_id)
    .bind(call_type)
    .bind(model)
    .bind(&prompt_hash)
    .bind(&response_hash)
    .bind(input_tokens)
    .bind(output_tokens)
    .bind(total)
    .fetch_one(&mut *tx)
    .await?;

    let call_id = row.0;

    for (i, (_role, content)) in prompts.iter().enumerate() {
        if i == 0 {
            // First prompt is system, rest are messages
            let messages: Vec<serde_json::Value> = prompts[1..].iter().map(|(r, c)| {
                serde_json::json!({"role": r, "content": c})
            }).collect();
            sqlx::query(
                "INSERT INTO llm_call_prompts (llm_call_id, system_prompt, messages_json, response_text)
                 VALUES ($1, $2, $3, $4)"
            )
            .bind(call_id)
            .bind(content)
            .bind(serde_json::Value::Array(messages))
            .bind("")
            .execute(&mut *tx)
            .await?;
            break;
        }
    }

    tx.commit().await.map_err(|e| AppError::Database(e))?;
    Ok(call_id)
}
