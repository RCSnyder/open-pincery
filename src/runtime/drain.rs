use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::agent::AgentStatus;
use crate::models::{agent, event};

/// Check if new events arrived during the wake, and if so, re-acquire.
/// Returns true if a new wake was started (drain re-acquired).
///
/// AC-82 (T-AC82-6 / G7b): the legacy `Maintenance → Awake` jump is
/// replaced by `drain_attempt_wake_acquire` (`Maintenance →
/// WakeAcquiring`). The remaining `WakeAcquiring → PromptAssembling →
/// Awake` hops fire at the top of `run_wake_loop` (shared with the
/// fresh-wake path), so the listener's post-drain
/// `wake_loop::run_wake_loop` call gets the same entry contract as
/// the first wake. Slice G7e adds full per-step lifecycle event
/// emission for drain; G7b emits only the entry event.
pub async fn check_drain(
    pool: &PgPool,
    agent_id: Uuid,
    wake_started_at: chrono::DateTime<chrono::Utc>,
) -> Result<bool, AppError> {
    let has_new = event::has_pending_events(pool, agent_id, wake_started_at).await?;

    if has_new {
        info!(agent_id = %agent_id, "Drain check: new events found, re-acquiring");
        let reacquired = agent::drain_attempt_wake_acquire(pool, agent_id).await?;
        if let Some(a) = &reacquired {
            // AC-82 (T-AC82-3 / G7b): one `lifecycle_transition` row
            // per CAS write. The drain entry uses the same canonical
            // action label as the fresh-wake entry — the spec
            // distinguishes the two only by the prior state (`from`).
            if let Some(new_wake_id) = a.wake_id {
                crate::runtime::lifecycle::emit(
                    pool,
                    agent_id,
                    new_wake_id,
                    AgentStatus::DB_MAINTENANCE,
                    AgentStatus::DB_WAKE_ACQUIRING,
                    "AttemptWakeAcquire",
                )
                .await?;
            }
        }
        return Ok(reacquired.is_some());
    }

    info!(agent_id = %agent_id, "Drain check: no new events, releasing to asleep");
    agent::release_to_asleep(pool, agent_id).await?;
    Ok(false)
}
