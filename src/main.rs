use open_pincery::{api, background, config, db, runtime};

use std::sync::Arc;
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

    // Start background tasks
    let bg_pool = pool.clone();
    let bg_config = config.clone();
    let bg_llm = llm.clone();
    tokio::spawn(async move {
        background::listener::start_listener(bg_pool, bg_config, bg_llm).await;
    });

    let stale_pool = pool.clone();
    let stale_config = config.clone();
    tokio::spawn(async move {
        background::stale::start_stale_recovery(stale_pool, stale_config).await;
    });

    // Build API
    let state = api::AppState {
        pool,
        config: (*config).clone(),
    };

    let app = api::router(state);

    // Health endpoint
    let app = app.route(
        "/health",
        axum::routing::get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
    );

    let addr = format!("{}:{}", config.host, config.port);
    info!(addr = %addr, "Starting server");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    axum::serve(listener, app)
        .await
        .expect("Server error");
}
