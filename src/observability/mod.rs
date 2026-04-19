//! Observability: logging format toggle (AC-17) and Prometheus metrics (AC-18).
//!
//! Logging runs unconditionally (JSON toggled by `LOG_FORMAT=json`).
//! Metrics recording/exporter are added in a later slice.

pub mod logging;
