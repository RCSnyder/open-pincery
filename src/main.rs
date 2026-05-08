use open_pincery::{api, background, config, db, runtime};

use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Parse a `Decimal` env var with a default. Panics with a clear message on
/// bad input rather than silently zeroing a price.
fn price_from_env(key: &str, default: &str) -> rust_decimal::Decimal {
    let raw = std::env::var(key).unwrap_or_else(|_| default.to_string());
    raw.parse::<rust_decimal::Decimal>()
        .unwrap_or_else(|e| panic!("Invalid {key}={raw}: {e}"))
}

#[tokio::main]
async fn main() {
    // Load .env if present
    let _ = dotenvy::dotenv();

    // Init tracing (human-readable by default; LOG_FORMAT=json for structured output)
    open_pincery::observability::logging::init_logging();

    #[cfg(target_os = "linux")]
    if let Err(code) = runtime::sandbox::preflight::enforce_kernel_floor_at_startup() {
        std::process::exit(code);
    }

    let config = config::Config::from_env().expect("Failed to load configuration");

    #[cfg(target_os = "linux")]
    if let Err(code) = runtime::sandbox::preflight::enforce_memory_cap_at_startup(
        config.sandbox.mode,
        config.sandbox.allow_unsafe,
    ) {
        std::process::exit(code);
    }

    let pool = db::create_pool(&config.database_url)
        .await
        .expect("Failed to create database pool");

    db::run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    info!("Migrations complete");

    // AC-78 G3d: refuse to boot the listener if any agent's audit
    // chain is broken. Override is `OPEN_PINCERY_AUDIT_CHAIN_FLOOR=relaxed`
    // + `OPEN_PINCERY_ALLOW_UNSAFE=true` (same shape as the
    // sandbox-floor pattern). Reads env directly here rather than
    // threading two new fields through `Config` — these are
    // emergency-only toggles.
    {
        let relaxed = std::env::var("OPEN_PINCERY_AUDIT_CHAIN_FLOOR")
            .map(|v| v.trim().eq_ignore_ascii_case("relaxed"))
            .unwrap_or(false);
        let allow_unsafe = std::env::var("OPEN_PINCERY_ALLOW_UNSAFE")
            .map(|v| v.trim().eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if let Err(code) =
            open_pincery::background::audit_chain::enforce_audit_chain_floor_at_startup(
                &pool,
                relaxed,
                allow_unsafe,
            )
            .await
        {
            std::process::exit(code);
        }
    }

    // AC-23: pricing used to compute `cost_usd` for every recorded LLM call.
    // Defaults chosen for a reasonable Claude-Sonnet-class model; operators
    // override per-model via env.
    let primary_pricing = runtime::llm::Pricing::new(
        price_from_env("LLM_PRICE_INPUT_PER_MTOK", "3.0"),
        price_from_env("LLM_PRICE_OUTPUT_PER_MTOK", "15.0"),
    );
    let maintenance_pricing = runtime::llm::Pricing::new(
        price_from_env("LLM_MAINTENANCE_PRICE_INPUT_PER_MTOK", "3.0"),
        price_from_env("LLM_MAINTENANCE_PRICE_OUTPUT_PER_MTOK", "15.0"),
    );

    let llm = Arc::new(
        runtime::llm::LlmClient::new(
            config.llm_api_base_url.clone(),
            config.llm_api_key.clone(),
            config.llm_model.clone(),
            config.llm_maintenance_model.clone(),
        )
        .with_pricing(primary_pricing, maintenance_pricing),
    );

    let config = Arc::new(config);
    let shutdown = CancellationToken::new();

    // AC-36 / AC-53 (Slice A2b.3): single sandboxed executor shared by
    // every wake loop. This is the ONLY place in the binary that mints
    // a `ToolExecutor`; everything else receives `Arc<dyn ToolExecutor>`
    // via AppState. The factory chooses `RealSandbox` (bwrap-wrapped)
    // on Linux when `sandbox.mode` is `enforce` or `audit`, and falls
    // back to `ProcessExecutor` on non-Linux hosts or when the mode is
    // `disabled` (paired with `ALLOW_UNSAFE=true`, enforced at
    // Config::from_env time per AC-73).
    let executor = runtime::sandbox::build_executor(&config.sandbox);

    // Build API (holds the per-task alive flags used by /ready).
    // Share the single executor instance with the wake loop via AppState.
    let state = api::AppState::new_with_executor(pool.clone(), (*config).clone(), executor.clone());

    // AC-18: optional Prometheus metrics server.
    // If METRICS_ADDR is set (e.g. "127.0.0.1:9090"), install a recorder and
    // spawn the /metrics endpoint on that address. Otherwise, the metrics
    // macros sprinkled through the runtime are no-ops.
    let metrics_handle = if let Ok(addr_str) = std::env::var("METRICS_ADDR") {
        match addr_str.parse::<std::net::SocketAddr>() {
            Ok(addr) => {
                let handle = open_pincery::observability::metrics::install_recorder();
                match open_pincery::observability::server::spawn_metrics_server(
                    addr,
                    handle,
                    shutdown.clone(),
                )
                .await
                {
                    Ok((jh, bound)) => {
                        info!(addr = %bound, "Metrics endpoint enabled");
                        Some(jh)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to bind metrics server — continuing without /metrics");
                        None
                    }
                }
            }
            Err(e) => {
                tracing::warn!(addr = %addr_str, error = %e, "Invalid METRICS_ADDR — metrics disabled");
                None
            }
        }
    } else {
        None
    };

    // Start background tasks
    let bg_pool = pool.clone();
    let bg_config = config.clone();
    let bg_llm = llm.clone();
    let bg_executor = executor.clone();
    let bg_vault = state.vault.clone();
    let bg_shutdown = shutdown.clone();
    let bg_alive = state.listener_alive.clone();
    let listener_handle = tokio::spawn(async move {
        background::listener::start_listener(
            bg_pool,
            bg_config,
            bg_llm,
            bg_executor,
            bg_vault,
            bg_shutdown,
            bg_alive,
        )
        .await;
    });

    let stale_pool = pool.clone();
    let stale_config = config.clone();
    let stale_shutdown = shutdown.clone();
    let stale_alive = state.stale_alive.clone();
    let stale_handle = tokio::spawn(async move {
        background::stale::start_stale_recovery(
            stale_pool,
            stale_config,
            stale_shutdown,
            stale_alive,
        )
        .await;
    });

    let app = api::router(state);

    let addr = format!("{}:{}", config.host, config.port);
    info!(addr = %addr, "Starting server");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    // Graceful shutdown: on SIGTERM/SIGINT, cancel all tasks then drain HTTP
    let server_shutdown = shutdown.clone();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
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
        if let Some(jh) = metrics_handle {
            let _ = jh.await;
        }
    })
    .await;

    info!("Shutdown complete");
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
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
