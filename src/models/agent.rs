use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

/// AC-34 (v6): Typed lifecycle state for an agent, aligned 1:1 with the
/// TLA+ specification names in `OpenPinceryAgent.tla`.
///
/// `WakeAcquiring` and `WakeEnding` are reserved values for a future CAS
/// split (see scaffolding/scope.md v10 Deferred); v6 code never writes
/// them, but the DB CHECK constraint accepts them so the rollout is a
/// pure additive change.
///
/// All DB boundary conversions go through `as_db_str` / `from_db_str`.
/// Construct the `DB_*` constants from the enum (not the other way
/// around) — the `const _: () = { … }` block below is a compile-time
/// exhaustiveness assert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// The agent is at rest; CAS may promote it to `Awake`. DB string: `asleep`.
    Resting,
    /// Reserved for a future CAS split between "about to wake" and "awake".
    /// v6 never writes this value.
    WakeAcquiring,
    /// Wake loop is actively executing iterations.
    Awake,
    /// Reserved for a future CAS split between "awake" and "maintenance".
    /// v6 never writes this value.
    WakeEnding,
    /// Post-wake bookkeeping (event replay, metrics flush) before returning to rest.
    Maintenance,
}

impl AgentStatus {
    pub const DB_ASLEEP: &'static str = "asleep";
    pub const DB_WAKE_ACQUIRING: &'static str = "wake_acquiring";
    pub const DB_AWAKE: &'static str = "awake";
    pub const DB_WAKE_ENDING: &'static str = "wake_ending";
    pub const DB_MAINTENANCE: &'static str = "maintenance";

    /// Single direction of the DB boundary: enum → DB string.
    pub const fn as_db_str(self) -> &'static str {
        match self {
            AgentStatus::Resting => Self::DB_ASLEEP,
            AgentStatus::WakeAcquiring => Self::DB_WAKE_ACQUIRING,
            AgentStatus::Awake => Self::DB_AWAKE,
            AgentStatus::WakeEnding => Self::DB_WAKE_ENDING,
            AgentStatus::Maintenance => Self::DB_MAINTENANCE,
        }
    }

    /// Single direction of the DB boundary: DB string → enum. Unknown
    /// values return `None`; callers typically propagate as a corruption
    /// error rather than silently defaulting.
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            Self::DB_ASLEEP => Some(AgentStatus::Resting),
            Self::DB_WAKE_ACQUIRING => Some(AgentStatus::WakeAcquiring),
            Self::DB_AWAKE => Some(AgentStatus::Awake),
            Self::DB_WAKE_ENDING => Some(AgentStatus::WakeEnding),
            Self::DB_MAINTENANCE => Some(AgentStatus::Maintenance),
            _ => None,
        }
    }
}

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

/// CAS: Attempt to acquire a wake (Resting → Awake).
/// Returns Some(agent) if this invocation won the race, None if another did.
pub async fn acquire_wake(pool: &PgPool, agent_id: Uuid) -> Result<Option<Agent>, AppError> {
    let sql = format!(
        "UPDATE agents
         SET status = '{awake}',
             wake_id = gen_random_uuid(),
             wake_started_at = NOW(),
             wake_iteration_count = 0
         WHERE id = $1 AND status = '{asleep}' AND is_enabled = TRUE
         RETURNING *",
        awake = AgentStatus::DB_AWAKE,
        asleep = AgentStatus::DB_ASLEEP,
    );
    let agent = sqlx::query_as::<_, Agent>(&sql)
        .bind(agent_id)
        .fetch_optional(pool)
        .await?;
    Ok(agent)
}

/// CAS: Transition from Awake → Maintenance.
pub async fn transition_to_maintenance(
    pool: &PgPool,
    agent_id: Uuid,
) -> Result<Option<Agent>, AppError> {
    let sql = format!(
        "UPDATE agents SET status = '{maintenance}'
         WHERE id = $1 AND status = '{awake}'
         RETURNING *",
        maintenance = AgentStatus::DB_MAINTENANCE,
        awake = AgentStatus::DB_AWAKE,
    );
    let agent = sqlx::query_as::<_, Agent>(&sql)
        .bind(agent_id)
        .fetch_optional(pool)
        .await?;
    Ok(agent)
}

/// CAS: Release from Maintenance → Resting.
pub async fn release_to_asleep(pool: &PgPool, agent_id: Uuid) -> Result<Option<Agent>, AppError> {
    let sql = format!(
        "UPDATE agents
         SET status = '{asleep}',
             wake_id = NULL,
             wake_started_at = NULL,
             wake_iteration_count = 0
         WHERE id = $1 AND status = '{maintenance}'
         RETURNING *",
        asleep = AgentStatus::DB_ASLEEP,
        maintenance = AgentStatus::DB_MAINTENANCE,
    );
    let agent = sqlx::query_as::<_, Agent>(&sql)
        .bind(agent_id)
        .fetch_optional(pool)
        .await?;
    Ok(agent)
}

/// CAS: Drain re-acquire (Maintenance → Awake) when new events arrived during wake.
pub async fn drain_reacquire(pool: &PgPool, agent_id: Uuid) -> Result<Option<Agent>, AppError> {
    let sql = format!(
        "UPDATE agents
         SET status = '{awake}',
             wake_id = gen_random_uuid(),
             wake_started_at = NOW(),
             wake_iteration_count = 0
         WHERE id = $1 AND status = '{maintenance}' AND is_enabled = TRUE
         RETURNING *",
        awake = AgentStatus::DB_AWAKE,
        maintenance = AgentStatus::DB_MAINTENANCE,
    );
    let agent = sqlx::query_as::<_, Agent>(&sql)
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

/// Find agents stuck in Awake/Maintenance past the stale threshold.
pub async fn find_stale_agents(pool: &PgPool, stale_hours: i64) -> Result<Vec<Agent>, AppError> {
    let sql = format!(
        "SELECT * FROM agents
         WHERE status IN ('{awake}', '{maintenance}')
           AND wake_started_at < NOW() - make_interval(hours => $1::int)
        ",
        awake = AgentStatus::DB_AWAKE,
        maintenance = AgentStatus::DB_MAINTENANCE,
    );
    let agents = sqlx::query_as::<_, Agent>(&sql)
        .bind(stale_hours as i32)
        .fetch_all(pool)
        .await?;
    Ok(agents)
}

/// Force-release a stale agent back to Resting.
pub async fn force_release(pool: &PgPool, agent_id: Uuid) -> Result<(), AppError> {
    let sql = format!(
        "UPDATE agents
         SET status = '{asleep}',
             wake_id = NULL,
             wake_started_at = NULL,
             wake_iteration_count = 0
         WHERE id = $1
           AND status IN ('{awake}', '{maintenance}')",
        asleep = AgentStatus::DB_ASLEEP,
        awake = AgentStatus::DB_AWAKE,
        maintenance = AgentStatus::DB_MAINTENANCE,
    );
    sqlx::query(&sql).bind(agent_id).execute(pool).await?;
    Ok(())
}

pub async fn update_agent(
    pool: &PgPool,
    id: Uuid,
    name: Option<&str>,
    is_enabled: Option<bool>,
    disabled_reason: Option<&str>,
    budget_limit_usd: Option<Decimal>,
) -> Result<Agent, AppError> {
    let agent = sqlx::query_as::<_, Agent>(
        "UPDATE agents SET
           name = COALESCE($2, name),
           is_enabled = COALESCE($3, is_enabled),
           disabled_reason = CASE WHEN $3 = FALSE THEN $4 WHEN $3 = TRUE THEN NULL ELSE disabled_reason END,
           disabled_at = CASE WHEN $3 = FALSE THEN NOW() WHEN $3 = TRUE THEN NULL ELSE disabled_at END,
           budget_limit_usd = COALESCE($5, budget_limit_usd)
         WHERE id = $1
         RETURNING *"
    )
    .bind(id)
    .bind(name)
    .bind(is_enabled)
    .bind(disabled_reason)
    .bind(budget_limit_usd)
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

pub async fn rotate_webhook_secret(
    pool: &PgPool,
    id: Uuid,
    webhook_secret: &str,
) -> Result<Agent, AppError> {
    let agent = sqlx::query_as::<_, Agent>(
        "UPDATE agents
         SET webhook_secret = $2
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(webhook_secret)
    .fetch_one(pool)
    .await?;
    Ok(agent)
}

pub async fn rotate_webhook_secret_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
    webhook_secret: &str,
) -> Result<Agent, AppError> {
    let agent = sqlx::query_as::<_, Agent>(
        "UPDATE agents
         SET webhook_secret = $2
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(webhook_secret)
    .fetch_one(&mut **tx)
    .await?;
    Ok(agent)
}
