mod common;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// AC-11: Background tasks exit cleanly when CancellationToken is cancelled.
#[tokio::test]
async fn test_shutdown_cancels_stale_recovery() {
    let pool = common::test_pool().await;
    let config = std::sync::Arc::new(open_pincery::config::Config::from_env().unwrap_or_else(
        |_| open_pincery::config::Config {
            database_url: String::new(),
            host: "127.0.0.1".into(),
            port: 0,
            bootstrap_token: String::new(),
            llm_api_key: String::new(),
            llm_api_base_url: String::new(),
            llm_model: "test".into(),
            llm_maintenance_model: "test".into(),
            max_prompt_chars: 100000,
            iteration_cap: 50,
            stale_wake_hours: 2,
            wake_summary_limit: 20,
            event_window_limit: 200,
            vault_key_b64: common::TEST_VAULT_KEY_B64.into(),
        },
    ));

    let shutdown = CancellationToken::new();
    let shutdown_clone = shutdown.clone();
    let alive = Arc::new(AtomicBool::new(false));

    let handle = tokio::spawn(async move {
        open_pincery::background::stale::start_stale_recovery(pool, config, shutdown_clone, alive)
            .await;
    });

    // Cancel and verify it stops within 2 seconds
    shutdown.cancel();
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;

    assert!(
        result.is_ok(),
        "Stale recovery should exit within 2 seconds after cancellation"
    );
    assert!(
        result.unwrap().is_ok(),
        "Stale recovery should exit cleanly"
    );
}
