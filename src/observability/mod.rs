//! Observability: logging format toggle (AC-17) and Prometheus metrics (AC-18).
//!
//! Logging runs unconditionally (JSON toggled by `LOG_FORMAT=json`).
//! Metrics recording requires calling `metrics::install_recorder()`; the
//! `/metrics` HTTP server is optional via `METRICS_ADDR` env var.

pub mod landlock_audit;
#[cfg(target_os = "linux")]
pub mod landlock_audit_netlink;
pub mod logging;
pub mod metrics;
pub mod server;
