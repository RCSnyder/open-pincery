use sqlx::PgPool;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::config::Config;
use crate::models::{agent, event};

/// Periodically check for and recover stale wakes.
pub async fn start_stale_recovery(pool: PgPool, config: Arc<Config>, shutdown: CancellationToken) {
    let interval_secs = 60; // Check every minute
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                info!("Stale recovery shutting down");
                return;
            }
            _ = interval.tick() => {}
        }

        match agent::find_stale_agents(&pool, config.stale_wake_hours as i64).await {
            Ok(stale_agents) => {
                for a in stale_agents {
                    info!(agent_id = %a.id, wake_id = ?a.wake_id, "Recovering stale agent");
                    if let Err(e) = agent::force_release(&pool, a.id).await {
                        error!(agent_id = %a.id, error = %e, "Failed to recover stale agent");
                    } else {
                        // Record stale_wake_recovery event
                        let _ = event::append_event(
                            &pool, a.id, "stale_wake_recovery", "system",
                            a.wake_id, None, None, None,
                            Some("Agent recovered from stale wake"), None,
                        ).await;
                    }
                }
            }
            Err(e) => {
                error!("Stale recovery query failed: {e}");
            }
        }
    }
}
