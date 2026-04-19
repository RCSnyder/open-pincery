//! AC-18: Prometheus metrics endpoint.
//!
//! Verifies that installing the recorder and spawning the HTTP server
//! exposes a `/metrics` endpoint that reflects counter increments.

use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;

use open_pincery::observability::metrics as m;
use open_pincery::observability::server;

#[tokio::test]
async fn metrics_endpoint_renders_counters() {
    // Install recorder and bind an ephemeral port.
    let handle = m::install_recorder();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let shutdown = CancellationToken::new();

    let (join, bound) = server::spawn_metrics_server(addr, handle, shutdown.clone())
        .await
        .expect("metrics server should bind");

    // Increment a representative counter from the public name table.
    metrics::counter!(m::WAKE_STARTED).increment(1);
    metrics::counter!(m::WAKE_COMPLETED, "reason" => "completed").increment(1);
    metrics::counter!(m::RATE_LIMIT_REJECTED).increment(3);
    // Exercise the gauge + histogram so they appear in the scrape output.
    metrics::gauge!(m::ACTIVE_WAKES).increment(1.0);
    metrics::gauge!(m::ACTIVE_WAKES).decrement(1.0);
    metrics::histogram!(m::WAKE_DURATION).record(0.25);

    // Scrape /metrics.
    let url = format!("http://{bound}/metrics");
    let body = reqwest::get(&url)
        .await
        .expect("scrape should succeed")
        .text()
        .await
        .expect("body should decode");

    assert!(
        body.contains("open_pincery_wake_started_total"),
        "expected wake_started counter in output:\n{body}"
    );
    assert!(
        body.contains("open_pincery_wake_completed_total"),
        "expected wake_completed counter in output:\n{body}"
    );
    assert!(
        body.contains("open_pincery_rate_limit_rejected_total 3"),
        "expected rate_limit_rejected=3 in output:\n{body}"
    );
    // Label should be preserved on the completed counter.
    assert!(
        body.contains("reason=\"completed\""),
        "expected reason label on wake_completed:\n{body}"
    );
    // AC-18: active-wakes gauge + wake-duration histogram must render.
    assert!(
        body.contains("open_pincery_active_wakes"),
        "expected active_wakes gauge in output:\n{body}"
    );
    assert!(
        body.contains("open_pincery_wake_duration_seconds"),
        "expected wake_duration histogram in output:\n{body}"
    );

    shutdown.cancel();
    let _ = join.await;
}
