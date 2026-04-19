use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentProjection {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub identity: String,
    pub work_list: String,
    pub version: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct WakeSummary {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub wake_id: Uuid,
    pub summary: String,
    pub created_at: DateTime<Utc>,
}

pub async fn insert_projection(
    pool: &PgPool,
    agent_id: Uuid,
    identity: &str,
    work_list: &str,
    version: i32,
) -> Result<AgentProjection, AppError> {
    let proj = sqlx::query_as::<_, AgentProjection>(
        "INSERT INTO agent_projections (agent_id, identity, work_list, version)
         VALUES ($1, $2, $3, $4)
         RETURNING *"
    )
    .bind(agent_id)
    .bind(identity)
    .bind(work_list)
    .bind(version)
    .fetch_one(pool)
    .await?;
    Ok(proj)
}

pub async fn latest_projection(
    pool: &PgPool,
    agent_id: Uuid,
) -> Result<Option<AgentProjection>, AppError> {
    let proj = sqlx::query_as::<_, AgentProjection>(
        "SELECT * FROM agent_projections
         WHERE agent_id = $1
         ORDER BY version DESC
         LIMIT 1"
    )
    .bind(agent_id)
    .fetch_optional(pool)
    .await?;
    Ok(proj)
}

pub async fn insert_wake_summary(
    pool: &PgPool,
    agent_id: Uuid,
    wake_id: Uuid,
    summary: &str,
) -> Result<WakeSummary, AppError> {
    let ws = sqlx::query_as::<_, WakeSummary>(
        "INSERT INTO wake_summaries (agent_id, wake_id, summary)
         VALUES ($1, $2, $3)
         RETURNING *"
    )
    .bind(agent_id)
    .bind(wake_id)
    .bind(summary)
    .fetch_one(pool)
    .await?;
    Ok(ws)
}

pub async fn recent_wake_summaries(
    pool: &PgPool,
    agent_id: Uuid,
    limit: i64,
) -> Result<Vec<WakeSummary>, AppError> {
    let summaries = sqlx::query_as::<_, WakeSummary>(
        "SELECT * FROM wake_summaries
         WHERE agent_id = $1
         ORDER BY created_at DESC
         LIMIT $2"
    )
    .bind(agent_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(summaries)
}
