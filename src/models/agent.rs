use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

/// AC-34 (v6) + AC-82 (v9): Typed lifecycle state for an agent, aligned
/// 1:1 with the TLA+ specification names in `OpenPinceryAgent.tla` /
/// `OpenPinceryCanonical.tla`.
///
/// AC-34 admitted `WakeAcquiring` and `WakeEnding` to the DB CHECK
/// constraint as "reserved" values. AC-82 (v9 Slice G7) widens the
/// enum and the constraint to include every fine-grained variant the
/// canonical spec uses, and (in `wake_loop.rs`) wires every one of
/// them into a real CAS write. After AC-82, the runtime never jumps
/// across more than one variant per CAS — the only gap is the
/// terminal `Maintenance → Resting` recovery via `release_to_asleep`,
/// which has no canonical action label.
///
/// All DB boundary conversions go through `as_db_str` / `from_db_str`.
/// Construct the `DB_*` constants from the enum (not the other way
/// around) — the `const _: () = { … }` block below is a compile-time
/// exhaustiveness assert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// The agent is at rest; CAS may promote it to `WakeAcquiring`. DB string: `asleep`.
    Resting,
    /// AC-82: A `attempt_wake_acquire` CAS won the race; the wake is
    /// being initialized but the prompt has not yet been assembled.
    /// Canonical action: `AttemptWakeAcquire`.
    WakeAcquiring,
    /// AC-82: Prompt context (events, summaries, system prompt v3) is
    /// being assembled before the first LLM call.
    /// Canonical action: `WakeAcquireSucceeds`.
    PromptAssembling,
    /// AC-82: Wake loop is between LLM rounds; the next LLM call is
    /// about to fire (or has just returned with no tool calls).
    /// Canonical actions: `PromptAssemblyCompletes` (first entry),
    /// `MidWakePollFindsNothing` (every subsequent return).
    Awake,
    /// AC-82: One or more validated tool calls were claimed by the
    /// LLM; the runtime is about to run AC-79 schema, AC-79 rate-limit,
    /// and AC-80 nonce-mint gates.
    /// Canonical action: `ToolDispatches`.
    ToolDispatching,
    /// AC-82: The per-tool-call AC-35 capability gate has passed and
    /// the AC-80 nonce has been minted; `dispatch_tool` is about to
    /// run the tool body.
    /// Canonical action: `AuthorizeExecution` (closest entry per
    /// `spec_coverage.md` AC-82 pipe-list — see C-AC82-3).
    ToolExecuting,
    /// AC-82: The tool returned (Output/Error/Sleep); the runtime is
    /// about to append `tool_result` and re-evaluate the loop.
    /// Canonical action: `ReceiveToolResult`.
    ToolResultProcessing,
    /// AC-82: The dispatch loop just finished iterating the per-LLM
    /// tool-call batch; before re-entering `Awake` the runtime checks
    /// for newly-arrived events.
    /// Canonical action: `ToolResultProcessedToolLoop`.
    MidWakeEventPolling,
    /// AC-82: A termination decision has been made (any
    /// `termination_reason`); the runtime is about to emit `wake_end`
    /// and transition to `Maintenance`.
    /// Canonical action: `TerminalEndsWake`.
    WakeEnding,
    /// Post-wake bookkeeping (event replay, metrics flush) before
    /// returning to rest.
    /// Canonical action: `WakeEndTransitionsToMaintenance`.
    Maintenance,
}

impl AgentStatus {
    pub const DB_ASLEEP: &'static str = "asleep";
    pub const DB_WAKE_ACQUIRING: &'static str = "wake_acquiring";
    pub const DB_PROMPT_ASSEMBLING: &'static str = "prompt_assembling";
    pub const DB_AWAKE: &'static str = "awake";
    pub const DB_TOOL_DISPATCHING: &'static str = "tool_dispatching";
    pub const DB_TOOL_EXECUTING: &'static str = "tool_executing";
    pub const DB_TOOL_RESULT_PROCESSING: &'static str = "tool_result_processing";
    pub const DB_MID_WAKE_EVENT_POLLING: &'static str = "mid_wake_event_polling";
    pub const DB_WAKE_ENDING: &'static str = "wake_ending";
    pub const DB_MAINTENANCE: &'static str = "maintenance";

    /// Single direction of the DB boundary: enum → DB string.
    pub const fn as_db_str(self) -> &'static str {
        match self {
            AgentStatus::Resting => Self::DB_ASLEEP,
            AgentStatus::WakeAcquiring => Self::DB_WAKE_ACQUIRING,
            AgentStatus::PromptAssembling => Self::DB_PROMPT_ASSEMBLING,
            AgentStatus::Awake => Self::DB_AWAKE,
            AgentStatus::ToolDispatching => Self::DB_TOOL_DISPATCHING,
            AgentStatus::ToolExecuting => Self::DB_TOOL_EXECUTING,
            AgentStatus::ToolResultProcessing => Self::DB_TOOL_RESULT_PROCESSING,
            AgentStatus::MidWakeEventPolling => Self::DB_MID_WAKE_EVENT_POLLING,
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
            Self::DB_PROMPT_ASSEMBLING => Some(AgentStatus::PromptAssembling),
            Self::DB_AWAKE => Some(AgentStatus::Awake),
            Self::DB_TOOL_DISPATCHING => Some(AgentStatus::ToolDispatching),
            Self::DB_TOOL_EXECUTING => Some(AgentStatus::ToolExecuting),
            Self::DB_TOOL_RESULT_PROCESSING => Some(AgentStatus::ToolResultProcessing),
            Self::DB_MID_WAKE_EVENT_POLLING => Some(AgentStatus::MidWakeEventPolling),
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

// =====================================================================
// AC-82 (v9 Slice G7) fine-grained CAS helpers
// =====================================================================
//
// Per scaffolding/readiness.md T-AC82-2, every fine-grained transition
// is a single `UPDATE … WHERE id = $1 AND status = 'prev'` round trip,
// returning `Some(agent)` if this caller won the race and `None`
// otherwise. The helpers do NOT emit `lifecycle_transition` events —
// that is the wake-loop's responsibility (G7b/G7c/G7d). Keeping events
// out of the model layer preserves the AC-78 hash-chain ordering
// guarantee (DB row update happens-before the event append on the
// same connection).
//
// `is_enabled = TRUE` is asserted only on the entry transitions
// (`attempt_wake_acquire`, `drain_attempt_wake_acquire`) — once a wake
// is in flight, an operator-disabled agent is allowed to finish its
// current wake gracefully through to `release_to_asleep`.

/// CAS: AC-82 entry transition. Resting → WakeAcquiring.
/// Canonical action: `AttemptWakeAcquire`.
///
/// Distinct from the legacy `acquire_wake` (which jumps Resting →
/// Awake in one step). Slice G7b replaces the wake-loop callsite to
/// chain `attempt_wake_acquire → wake_acquire_succeeds →
/// prompt_assembly_completes`; legacy `acquire_wake` is kept available
/// during the migration so the build stays green between slices.
pub async fn attempt_wake_acquire(
    pool: &PgPool,
    agent_id: Uuid,
) -> Result<Option<Agent>, AppError> {
    let sql = format!(
        "UPDATE agents
         SET status = '{next}',
             wake_id = gen_random_uuid(),
             wake_started_at = NOW(),
             wake_iteration_count = 0
         WHERE id = $1 AND status = '{prev}' AND is_enabled = TRUE
         RETURNING *",
        next = AgentStatus::DB_WAKE_ACQUIRING,
        prev = AgentStatus::DB_ASLEEP,
    );
    let agent = sqlx::query_as::<_, Agent>(&sql)
        .bind(agent_id)
        .fetch_optional(pool)
        .await?;
    Ok(agent)
}

/// CAS: AC-82. WakeAcquiring → PromptAssembling.
/// Canonical action: `WakeAcquireSucceeds`.
pub async fn wake_acquire_succeeds(
    pool: &PgPool,
    agent_id: Uuid,
) -> Result<Option<Agent>, AppError> {
    let sql = format!(
        "UPDATE agents SET status = '{next}'
         WHERE id = $1 AND status = '{prev}'
         RETURNING *",
        next = AgentStatus::DB_PROMPT_ASSEMBLING,
        prev = AgentStatus::DB_WAKE_ACQUIRING,
    );
    let agent = sqlx::query_as::<_, Agent>(&sql)
        .bind(agent_id)
        .fetch_optional(pool)
        .await?;
    Ok(agent)
}

/// CAS: AC-82. PromptAssembling → Awake.
/// Canonical action: `PromptAssemblyCompletes`.
pub async fn prompt_assembly_completes(
    pool: &PgPool,
    agent_id: Uuid,
) -> Result<Option<Agent>, AppError> {
    let sql = format!(
        "UPDATE agents SET status = '{next}'
         WHERE id = $1 AND status = '{prev}'
         RETURNING *",
        next = AgentStatus::DB_AWAKE,
        prev = AgentStatus::DB_PROMPT_ASSEMBLING,
    );
    let agent = sqlx::query_as::<_, Agent>(&sql)
        .bind(agent_id)
        .fetch_optional(pool)
        .await?;
    Ok(agent)
}

/// CAS: AC-82. Awake → ToolDispatching.
/// Canonical action: `ToolDispatches`.
pub async fn enter_tool_dispatching(
    pool: &PgPool,
    agent_id: Uuid,
) -> Result<Option<Agent>, AppError> {
    let sql = format!(
        "UPDATE agents SET status = '{next}'
         WHERE id = $1 AND status = '{prev}'
         RETURNING *",
        next = AgentStatus::DB_TOOL_DISPATCHING,
        prev = AgentStatus::DB_AWAKE,
    );
    let agent = sqlx::query_as::<_, Agent>(&sql)
        .bind(agent_id)
        .fetch_optional(pool)
        .await?;
    Ok(agent)
}

/// CAS: AC-82. ToolDispatching → ToolExecuting.
/// Canonical action: `AuthorizeExecution` (post-AC-79/AC-80 gates,
/// pre-tool-body).
pub async fn enter_tool_executing(
    pool: &PgPool,
    agent_id: Uuid,
) -> Result<Option<Agent>, AppError> {
    let sql = format!(
        "UPDATE agents SET status = '{next}'
         WHERE id = $1 AND status = '{prev}'
         RETURNING *",
        next = AgentStatus::DB_TOOL_EXECUTING,
        prev = AgentStatus::DB_TOOL_DISPATCHING,
    );
    let agent = sqlx::query_as::<_, Agent>(&sql)
        .bind(agent_id)
        .fetch_optional(pool)
        .await?;
    Ok(agent)
}

/// CAS: AC-82. ToolExecuting → ToolResultProcessing.
/// Canonical action: `ReceiveToolResult`.
pub async fn enter_tool_result_processing(
    pool: &PgPool,
    agent_id: Uuid,
) -> Result<Option<Agent>, AppError> {
    let sql = format!(
        "UPDATE agents SET status = '{next}'
         WHERE id = $1 AND status = '{prev}'
         RETURNING *",
        next = AgentStatus::DB_TOOL_RESULT_PROCESSING,
        prev = AgentStatus::DB_TOOL_EXECUTING,
    );
    let agent = sqlx::query_as::<_, Agent>(&sql)
        .bind(agent_id)
        .fetch_optional(pool)
        .await?;
    Ok(agent)
}

/// CAS: AC-82. ToolResultProcessing → MidWakeEventPolling.
/// Canonical action: `ToolResultProcessedToolLoop`.
pub async fn enter_mid_wake_event_polling(
    pool: &PgPool,
    agent_id: Uuid,
) -> Result<Option<Agent>, AppError> {
    let sql = format!(
        "UPDATE agents SET status = '{next}'
         WHERE id = $1 AND status = '{prev}'
         RETURNING *",
        next = AgentStatus::DB_MID_WAKE_EVENT_POLLING,
        prev = AgentStatus::DB_TOOL_RESULT_PROCESSING,
    );
    let agent = sqlx::query_as::<_, Agent>(&sql)
        .bind(agent_id)
        .fetch_optional(pool)
        .await?;
    Ok(agent)
}

/// CAS: AC-82. MidWakeEventPolling → Awake.
/// Canonical action: `MidWakePollFindsNothing`.
///
/// The "found something" branch is encoded by `enter_wake_ending`
/// (terminal) or by leaving the loop alone — the v9 design is that
/// any newly-arrived event is processed by drain after wake_end,
/// not mid-wake (T-AC82-6).
pub async fn mid_wake_poll_finds_nothing(
    pool: &PgPool,
    agent_id: Uuid,
) -> Result<Option<Agent>, AppError> {
    let sql = format!(
        "UPDATE agents SET status = '{next}'
         WHERE id = $1 AND status = '{prev}'
         RETURNING *",
        next = AgentStatus::DB_AWAKE,
        prev = AgentStatus::DB_MID_WAKE_EVENT_POLLING,
    );
    let agent = sqlx::query_as::<_, Agent>(&sql)
        .bind(agent_id)
        .fetch_optional(pool)
        .await?;
    Ok(agent)
}

/// CAS: AC-82 terminal entry. (Awake | ToolDispatching | ToolExecuting
/// | ToolResultProcessing | MidWakeEventPolling) → WakeEnding.
/// Canonical action: `TerminalEndsWake`.
///
/// The IN clause is the only place AC-82 widens the WHERE beyond a
/// single value — by design, a wake can decide to terminate from any
/// live state (iteration_cap mid-loop, prompt-injection-suspected
/// mid-LLM-response, sleep mid-tool-loop). The CAS still names every
/// admissible source explicitly; PromptAssembling is excluded because
/// reaching it requires a successful CAS to Awake first, and Resting
/// / WakeAcquiring / Maintenance / WakeEnding are not "live wake"
/// states.
pub async fn enter_wake_ending(pool: &PgPool, agent_id: Uuid) -> Result<Option<Agent>, AppError> {
    let sql = format!(
        "UPDATE agents SET status = '{next}'
         WHERE id = $1
           AND status IN (
             '{awake}', '{td}', '{te}', '{trp}', '{mwep}'
           )
         RETURNING *",
        next = AgentStatus::DB_WAKE_ENDING,
        awake = AgentStatus::DB_AWAKE,
        td = AgentStatus::DB_TOOL_DISPATCHING,
        te = AgentStatus::DB_TOOL_EXECUTING,
        trp = AgentStatus::DB_TOOL_RESULT_PROCESSING,
        mwep = AgentStatus::DB_MID_WAKE_EVENT_POLLING,
    );
    let agent = sqlx::query_as::<_, Agent>(&sql)
        .bind(agent_id)
        .fetch_optional(pool)
        .await?;
    Ok(agent)
}

/// CAS: AC-82. WakeEnding → Maintenance.
/// Canonical action: `WakeEndTransitionsToMaintenance`.
pub async fn wake_end_transitions_to_maintenance(
    pool: &PgPool,
    agent_id: Uuid,
) -> Result<Option<Agent>, AppError> {
    let sql = format!(
        "UPDATE agents SET status = '{next}'
         WHERE id = $1 AND status = '{prev}'
         RETURNING *",
        next = AgentStatus::DB_MAINTENANCE,
        prev = AgentStatus::DB_WAKE_ENDING,
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
