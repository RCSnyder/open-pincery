//! AC-18: tiny HTTP server for the Prometheus `/metrics` endpoint.
//!
//! Runs on a separate opt-in port so it is never exposed as part of the
//! main API surface. Enabled by setting the `METRICS_ADDR` env var
//! (e.g. `METRICS_ADDR=127.0.0.1:9090`).

use axum::{extract::State, routing::get, Router};
use metrics_exporter_prometheus::PrometheusHandle;
use std::net::SocketAddr;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

async fn render_metrics(State(handle): State<PrometheusHandle>) -> String {
    handle.render()
}

/// Spawn the metrics HTTP server. Returns a `JoinHandle` and the bound
/// address (useful for tests that bind `127.0.0.1:0`).
pub async fn spawn_metrics_server(
    addr: SocketAddr,
    handle: PrometheusHandle,
    shutdown: CancellationToken,
) -> std::io::Result<(JoinHandle<()>, SocketAddr)> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;
    info!(addr = %bound, "Metrics server listening");

    let app = Router::new()
        .route("/metrics", get(render_metrics))
        .with_state(handle);

    let join = tokio::spawn(async move {
        let serve = axum::serve(listener, app).with_graceful_shutdown(async move {
            shutdown.cancelled().await;
        });
        if let Err(e) = serve.await {
            warn!(error = %e, "Metrics server error");
        }
    });

    Ok((join, bound))
}
