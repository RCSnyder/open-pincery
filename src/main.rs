use open_pincery::{api, background, config, db, runtime};

use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::info;

#[tokio::main]
async fn main() {
    // Load .env if present
    let _ = dotenvy::dotenv();

    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let config = config::Config::from_env().expect("Failed to load configuration");
    let pool = db::create_pool(&config.database_url)
        .await
        .expect("Failed to create database pool");

    db::run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    info!("Migrations complete");

    let llm = Arc::new(runtime::llm::LlmClient::new(
        config.llm_api_base_url.clone(),
        config.llm_api_key.clone(),
        config.llm_model.clone(),
        config.llm_maintenance_model.clone(),
    ));

    let config = Arc::new(config);
    let shutdown = CancellationToken::new();

    // Start background tasks
    let bg_pool = pool.clone();
    let bg_config = config.clone();
    let bg_llm = llm.clone();
    let bg_shutdown = shutdown.clone();
    let listener_handle = tokio::spawn(async move {
        background::listener::start_listener(bg_pool, bg_config, bg_llm, bg_shutdown).await;
    });

    let stale_pool = pool.clone();
    let stale_config = config.clone();
    let stale_shutdown = shutdown.clone();
    let stale_handle = tokio::spawn(async move {
        background::stale::start_stale_recovery(stale_pool, stale_config, stale_shutdown).await;
    });

    // Build API
    let state = api::AppState::new(pool, (*config).clone());

    let app = api::router(state);

    let addr = format!("{}:{}", config.host, config.port);
    info!(addr = %addr, "Starting server");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    // Graceful shutdown: on SIGTERM/SIGINT, cancel all tasks then drain HTTP
    let server_shutdown = shutdown.clone();
    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            info!("Shutdown signal received, draining...");
            server_shutdown.cancel();
        })
        .await
        .expect("Server error");

    // Wait for background tasks to finish (up to 30 seconds)
    info!("Waiting for background tasks to finish (up to 30s)...");
    let drain_timeout = tokio::time::Duration::from_secs(30);
    let _ = tokio::time::timeout(drain_timeout, async {
        let _ = listener_handle.await;
        let _ = stale_handle.await;
    })
    .await;

    info!("Shutdown complete");
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Failed to install SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {},
            _ = sigterm.recv() => {},
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.expect("Failed to listen for Ctrl+C");
    }
}
