use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Event {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub event_type: String,
    pub source: String,
    pub wake_id: Option<Uuid>,
    pub tool_name: Option<String>,
    pub tool_input: Option<String>,
    pub tool_output: Option<String>,
    pub content: Option<String>,
    pub termination_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[allow(clippy::too_many_arguments)]
pub async fn append_event(
    pool: &PgPool,
    agent_id: Uuid,
    event_type: &str,
    source: &str,
    wake_id: Option<Uuid>,
    tool_name: Option<&str>,
    tool_input: Option<&str>,
    tool_output: Option<&str>,
    content: Option<&str>,
    termination_reason: Option<&str>,
) -> Result<Event, AppError> {
    let event = sqlx::query_as::<_, Event>(
        "INSERT INTO events (agent_id, event_type, source, wake_id, tool_name, tool_input, tool_output, content, termination_reason)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING *"
    )
    .bind(agent_id)
    .bind(event_type)
    .bind(source)
    .bind(wake_id)
    .bind(tool_name)
    .bind(tool_input)
    .bind(tool_output)
    .bind(content)
    .bind(termination_reason)
    .fetch_one(pool)
    .await?;
    Ok(event)
}

#[allow(clippy::too_many_arguments)]
pub async fn append_event_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    agent_id: Uuid,
    event_type: &str,
    source: &str,
    wake_id: Option<Uuid>,
    tool_name: Option<&str>,
    tool_input: Option<&str>,
    tool_output: Option<&str>,
    content: Option<&str>,
    termination_reason: Option<&str>,
) -> Result<Event, AppError> {
    let event = sqlx::query_as::<_, Event>(
        "INSERT INTO events (agent_id, event_type, source, wake_id, tool_name, tool_input, tool_output, content, termination_reason)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING *",
    )
    .bind(agent_id)
    .bind(event_type)
    .bind(source)
    .bind(wake_id)
    .bind(tool_name)
    .bind(tool_input)
    .bind(tool_output)
    .bind(content)
    .bind(termination_reason)
    .fetch_one(&mut **tx)
    .await?;
    Ok(event)
}

pub async fn recent_events(
    pool: &PgPool,
    agent_id: Uuid,
    limit: i64,
) -> Result<Vec<Event>, AppError> {
    let events = sqlx::query_as::<_, Event>(
        "SELECT * FROM events
         WHERE agent_id = $1
         ORDER BY created_at DESC
         LIMIT $2",
    )
    .bind(agent_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(events)
}

pub async fn events_since(
    pool: &PgPool,
    agent_id: Uuid,
    since: DateTime<Utc>,
) -> Result<Vec<Event>, AppError> {
    let events = sqlx::query_as::<_, Event>(
        "SELECT * FROM events
         WHERE agent_id = $1 AND created_at > $2
         ORDER BY created_at ASC",
    )
    .bind(agent_id)
    .bind(since)
    .fetch_all(pool)
    .await?;
    Ok(events)
}

pub async fn has_pending_events(
    pool: &PgPool,
    agent_id: Uuid,
    since: DateTime<Utc>,
) -> Result<bool, AppError> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM events
         WHERE agent_id = $1 AND created_at > $2 AND event_type = 'message_received'",
    )
    .bind(agent_id)
    .bind(since)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

pub async fn events_since_id(
    pool: &PgPool,
    agent_id: Uuid,
    since_id: Uuid,
    limit: i64,
) -> Result<Vec<Event>, AppError> {
    let events = sqlx::query_as::<_, Event>(
        "SELECT e.*
         FROM events e
         WHERE e.agent_id = $1
           AND e.created_at > COALESCE(
               (
                 SELECT created_at
                 FROM events
                 WHERE id = $2 AND agent_id = $1
               ),
               to_timestamp(0)
           )
         ORDER BY e.created_at ASC
         LIMIT $3",
    )
    .bind(agent_id)
    .bind(since_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(events)
}
