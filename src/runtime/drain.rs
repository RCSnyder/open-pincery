use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{agent, event};

/// Check if new events arrived during the wake, and if so, re-acquire.
/// Returns true if a new wake was started (drain re-acquired).
pub async fn check_drain(
    pool: &PgPool,
    agent_id: Uuid,
    wake_started_at: chrono::DateTime<chrono::Utc>,
) -> Result<bool, AppError> {
    let has_new = event::has_pending_events(pool, agent_id, wake_started_at).await?;

    if has_new {
        info!(agent_id = %agent_id, "Drain check: new events found, re-acquiring");
        let reacquired = agent::drain_reacquire(pool, agent_id).await?;
        return Ok(reacquired.is_some());
    }

    info!(agent_id = %agent_id, "Drain check: no new events, releasing to asleep");
    agent::release_to_asleep(pool, agent_id).await?;
    Ok(false)
}
