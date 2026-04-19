use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct LlmCall {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub wake_id: Option<Uuid>,
    pub model: String,
    pub purpose: String,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub duration_ms: Option<i32>,
    pub created_at: DateTime<Utc>,
}

pub async fn insert_llm_call(
    pool: &PgPool,
    agent_id: Uuid,
    wake_id: Option<Uuid>,
    model: &str,
    purpose: &str,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    duration_ms: Option<i32>,
    prompts: &[(String, String)], // (role, content)
) -> Result<LlmCall, AppError> {
    let mut tx = pool.begin().await.map_err(|e| AppError::Database(e))?;

    let call = sqlx::query_as::<_, LlmCall>(
        "INSERT INTO llm_calls (agent_id, wake_id, model, purpose, input_tokens, output_tokens, duration_ms)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING *"
    )
    .bind(agent_id)
    .bind(wake_id)
    .bind(model)
    .bind(purpose)
    .bind(input_tokens)
    .bind(output_tokens)
    .bind(duration_ms)
    .fetch_one(&mut *tx)
    .await?;

    for (i, (role, content)) in prompts.iter().enumerate() {
        sqlx::query(
            "INSERT INTO llm_call_prompts (llm_call_id, ordinal, role, content)
             VALUES ($1, $2, $3, $4)"
        )
        .bind(call.id)
        .bind(i as i32)
        .bind(role)
        .bind(content)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await.map_err(|e| AppError::Database(e))?;
    Ok(call)
}
