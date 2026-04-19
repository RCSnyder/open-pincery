use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Agent {
    pub id: Uuid,
    pub name: String,
    pub workspace_id: Uuid,
    pub owner_id: Uuid,
    pub status: String,
    pub wake_id: Option<Uuid>,
    pub wake_started_at: Option<DateTime<Utc>>,
    pub wake_iteration_count: i32,
    pub permission_mode: String,
    pub is_enabled: bool,
    pub disabled_reason: Option<String>,
    pub disabled_at: Option<DateTime<Utc>>,
    pub budget_limit_usd: Decimal,
    pub budget_used_usd: Decimal,
    pub webhook_secret: String,
    pub created_at: DateTime<Utc>,
}

pub async fn create_agent(
    pool: &PgPool,
    name: &str,
    workspace_id: Uuid,
    owner_id: Uuid,
) -> Result<Agent, AppError> {
    let agent = sqlx::query_as::<_, Agent>(
        "INSERT INTO agents (name, workspace_id, owner_id)
         VALUES ($1, $2, $3)
         RETURNING *",
    )
    .bind(name)
    .bind(workspace_id)
    .bind(owner_id)
    .fetch_one(pool)
    .await?;
    Ok(agent)
}

pub async fn get_agent(pool: &PgPool, id: Uuid) -> Result<Option<Agent>, AppError> {
    let agent = sqlx::query_as::<_, Agent>("SELECT * FROM agents WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(agent)
}

pub async fn list_agents(pool: &PgPool, workspace_id: Uuid) -> Result<Vec<Agent>, AppError> {
    let agents = sqlx::query_as::<_, Agent>(
        "SELECT * FROM agents WHERE workspace_id = $1 ORDER BY created_at DESC",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;
    Ok(agents)
}

/// CAS: Attempt to acquire a wake (asleep → awake).
/// Returns Some(agent) if this invocation won the race, None if another did.
pub async fn acquire_wake(pool: &PgPool, agent_id: Uuid) -> Result<Option<Agent>, AppError> {
    let agent = sqlx::query_as::<_, Agent>(
        "UPDATE agents
         SET status = 'awake',
             wake_id = gen_random_uuid(),
             wake_started_at = NOW(),
             wake_iteration_count = 0
         WHERE id = $1 AND status = 'asleep' AND is_enabled = TRUE
         RETURNING *",
    )
    .bind(agent_id)
    .fetch_optional(pool)
    .await?;
    Ok(agent)
}

/// CAS: Transition from awake → maintenance.
pub async fn transition_to_maintenance(
    pool: &PgPool,
    agent_id: Uuid,
) -> Result<Option<Agent>, AppError> {
    let agent = sqlx::query_as::<_, Agent>(
        "UPDATE agents SET status = 'maintenance'
         WHERE id = $1 AND status = 'awake'
         RETURNING *",
    )
    .bind(agent_id)
    .fetch_optional(pool)
    .await?;
    Ok(agent)
}

/// CAS: Release from maintenance → asleep.
pub async fn release_to_asleep(pool: &PgPool, agent_id: Uuid) -> Result<Option<Agent>, AppError> {
    let agent = sqlx::query_as::<_, Agent>(
        "UPDATE agents
         SET status = 'asleep',
             wake_id = NULL,
             wake_started_at = NULL,
             wake_iteration_count = 0
         WHERE id = $1 AND status = 'maintenance'
         RETURNING *",
    )
    .bind(agent_id)
    .fetch_optional(pool)
    .await?;
    Ok(agent)
}

/// CAS: Drain re-acquire (maintenance → awake) when new events arrived during wake.
pub async fn drain_reacquire(pool: &PgPool, agent_id: Uuid) -> Result<Option<Agent>, AppError> {
    let agent = sqlx::query_as::<_, Agent>(
        "UPDATE agents
         SET status = 'awake',
             wake_id = gen_random_uuid(),
             wake_started_at = NOW(),
             wake_iteration_count = 0
         WHERE id = $1 AND status = 'maintenance' AND is_enabled = TRUE
         RETURNING *",
    )
    .bind(agent_id)
    .fetch_optional(pool)
    .await?;
    Ok(agent)
}

/// Increment iteration count after a tool call.
pub async fn increment_iteration(pool: &PgPool, agent_id: Uuid) -> Result<i32, AppError> {
    let row = sqlx::query_scalar::<_, i32>(
        "UPDATE agents
         SET wake_iteration_count = wake_iteration_count + 1
         WHERE id = $1
         RETURNING wake_iteration_count",
    )
    .bind(agent_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Find agents stuck in awake/maintenance past the stale threshold.
pub async fn find_stale_agents(pool: &PgPool, stale_hours: i64) -> Result<Vec<Agent>, AppError> {
    let agents = sqlx::query_as::<_, Agent>(
        "SELECT * FROM agents
         WHERE status IN ('awake', 'maintenance')
           AND wake_started_at < NOW() - make_interval(hours => $1::int)
        ",
    )
    .bind(stale_hours as i32)
    .fetch_all(pool)
    .await?;
    Ok(agents)
}

/// Force-release a stale agent back to asleep.
pub async fn force_release(pool: &PgPool, agent_id: Uuid) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE agents
         SET status = 'asleep',
             wake_id = NULL,
             wake_started_at = NULL,
             wake_iteration_count = 0
         WHERE id = $1
           AND status IN ('awake', 'maintenance')",
    )
    .bind(agent_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_agent(
    pool: &PgPool,
    id: Uuid,
    name: Option<&str>,
    is_enabled: Option<bool>,
    disabled_reason: Option<&str>,
) -> Result<Agent, AppError> {
    let agent = sqlx::query_as::<_, Agent>(
        "UPDATE agents SET
           name = COALESCE($2, name),
           is_enabled = COALESCE($3, is_enabled),
           disabled_reason = CASE WHEN $3 = FALSE THEN $4 WHEN $3 = TRUE THEN NULL ELSE disabled_reason END,
           disabled_at = CASE WHEN $3 = FALSE THEN NOW() WHEN $3 = TRUE THEN NULL ELSE disabled_at END
         WHERE id = $1
         RETURNING *"
    )
    .bind(id)
    .bind(name)
    .bind(is_enabled)
    .bind(disabled_reason)
    .fetch_one(pool)
    .await?;
    Ok(agent)
}

pub async fn soft_delete_agent(pool: &PgPool, id: Uuid) -> Result<Agent, AppError> {
    let agent = sqlx::query_as::<_, Agent>(
        "UPDATE agents SET
           is_enabled = FALSE,
           disabled_reason = 'deleted',
           disabled_at = NOW()
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(agent)
}
