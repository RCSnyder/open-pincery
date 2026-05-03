use rust_decimal::Decimal;
use sqlx::postgres::PgListener;
use sqlx::PgPool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::error::AppError;
use crate::models::{agent, event};
use crate::runtime::sandbox::ToolExecutor;
use crate::runtime::vault::Vault;
use crate::runtime::{drain, llm::LlmClient, maintenance, wake_loop};

/// Spawn the LISTEN/NOTIFY handler that triggers wakes.
///
/// `alive` is set to `true` after the LISTEN succeeds and back to `false`
/// before the function returns (for any reason), so `/ready` (AC-19)
/// accurately reflects the task's current state.
pub async fn start_listener(
    pool: PgPool,
    config: Arc<Config>,
    llm: Arc<LlmClient>,
    executor: Arc<dyn ToolExecutor>,
    vault: Arc<Vault>,
    shutdown: CancellationToken,
    alive: Arc<AtomicBool>,
) {
    // Guard: always clear `alive` when this function returns, no matter the path.
    struct AliveGuard(Arc<AtomicBool>);
    impl Drop for AliveGuard {
        fn drop(&mut self) {
            self.0.store(false, Ordering::Relaxed);
        }
    }
    let _guard = AliveGuard(alive.clone());

    // We listen on a wildcard pattern — but PgListener requires exact channel names.
    // Instead, we'll listen on a general channel and agents will NOTIFY on it.
    // Actually, Postgres LISTEN doesn't support wildcards. We use a single channel.
    let mut listener = match PgListener::connect_with(&pool).await {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to create PG listener: {e}");
            return;
        }
    };

    if let Err(e) = listener.listen("agent_wake").await {
        error!("Failed to listen on agent_wake channel: {e}");
        return;
    }

    alive.store(true, Ordering::Relaxed);
    info!("Background listener started on channel 'agent_wake'");

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                info!("Background listener shutting down");
                return;
            }
            result = listener.recv() => {
                match result {
                    Ok(notification) => {
                        let payload = notification.payload().to_string();
                        info!(payload = %payload, "Received wake notification");

                        let agent_id = match uuid::Uuid::parse_str(&payload) {
                            Ok(id) => id,
                            Err(e) => {
                                warn!(payload = %payload, "Invalid agent_id in notification: {e}");
                                continue;
                            }
                        };

                        let pool = pool.clone();
                        let config = config.clone();
                        let llm = llm.clone();
                        let executor = executor.clone();
                        let vault = vault.clone();

                        tokio::spawn(async move {
                            if let Err(e) = handle_wake(pool, config, llm, executor, vault, agent_id).await {
                                error!(agent_id = %agent_id, error = %e, "Wake handler failed");
                            }
                        });
                    }
                    Err(e) => {
                        error!("Listener error: {e}");
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                }
            }
        }
    }
}

async fn handle_wake(
    pool: PgPool,
    config: Arc<Config>,
    llm: Arc<LlmClient>,
    executor: Arc<dyn ToolExecutor>,
    vault: Arc<Vault>,
    agent_id: uuid::Uuid,
) -> Result<(), AppError> {
    // AC-23: Enforce hard budget caps before CAS wake acquisition.
    let candidate = agent::get_agent(&pool, agent_id)
        .await?
        .ok_or(AppError::NotFound("Agent not found for wake".into()))?;

    if candidate.budget_limit_usd > Decimal::ZERO
        && candidate.budget_used_usd >= candidate.budget_limit_usd
    {
        let payload = serde_json::json!({
            "limit_usd": candidate.budget_limit_usd,
            "used_usd": candidate.budget_used_usd,
        })
        .to_string();

        event::append_event(
            &pool,
            agent_id,
            "budget_exceeded",
            "runtime",
            None,
            None,
            None,
            None,
            Some(&payload),
            None,
        )
        .await?;

        info!(
            agent_id = %agent_id,
            limit_usd = %candidate.budget_limit_usd,
            used_usd = %candidate.budget_used_usd,
            "Skipping wake due to budget cap"
        );
        return Ok(());
    }

    // Attempt CAS acquisition
    let acquired = agent::acquire_wake(&pool, agent_id).await?;
    let agent_data = match acquired {
        Some(a) => a,
        None => {
            info!(agent_id = %agent_id, "CAS acquisition failed (agent not asleep or already acquired)");
            return Ok(());
        }
    };

    let wake_id = agent_data.wake_id.unwrap();
    let wake_started_at = agent_data.wake_started_at.unwrap();

    // Run wake loop
    let _reason =
        wake_loop::run_wake_loop(&pool, &llm, &config, agent_id, wake_id, &executor, &vault)
            .await?;

    // Transition to maintenance
    agent::transition_to_maintenance(&pool, agent_id).await?;

    // Run maintenance
    maintenance::run_maintenance(&pool, &llm, agent_id, wake_id).await?;

    // Drain check
    let reacquired = drain::check_drain(&pool, agent_id, wake_started_at).await?;
    if reacquired {
        // Recursion: the new wake will be handled by a fresh task
        let new_agent = agent::get_agent(&pool, agent_id)
            .await?
            .ok_or(AppError::NotFound("Agent not found after drain".into()))?;
        if let (Some(new_wake_id), Some(_new_wake_started)) =
            (new_agent.wake_id, new_agent.wake_started_at)
        {
            let _reason = wake_loop::run_wake_loop(
                &pool,
                &llm,
                &config,
                agent_id,
                new_wake_id,
                &executor,
                &vault,
            )
            .await?;
            agent::transition_to_maintenance(&pool, agent_id).await?;
            maintenance::run_maintenance(&pool, &llm, agent_id, new_wake_id).await?;
            // Final release — no further drain for simplicity in v1
            agent::release_to_asleep(&pool, agent_id).await?;
        }
    }

    Ok(())
}
