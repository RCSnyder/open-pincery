//! AC-18: Prometheus metrics via the `metrics` facade.
//!
//! Metric name constants are defined here so that call sites stay consistent
//! and so that renames are mechanical. Canonical names use the
//! `open_pincery_` prefix — Prometheus will combine that with the
//! `_total` suffix added for counters.
//!
//! If no recorder is installed (the default when `METRICS_ADDR` is unset),
//! the `metrics::counter!()` macros are no-ops. So sprinkling calls through
//! the runtime costs nothing when metrics are disabled.

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

// ---- Metric names ----------------------------------------------------------

pub const WAKE_STARTED: &str = "open_pincery_wake_started_total";
pub const WAKE_COMPLETED: &str = "open_pincery_wake_completed_total";
/// Number of wakes currently executing. Gauge — used by dashboards to alert
/// when the system is stuck (e.g. always-high active count while completed
/// counter stays flat).
pub const ACTIVE_WAKES: &str = "open_pincery_active_wakes";
/// End-to-end wake duration (seconds). Histogram — supports p50/p95/p99
/// dashboards across all termination reasons.
pub const WAKE_DURATION: &str = "open_pincery_wake_duration_seconds";
pub const LLM_CALL: &str = "open_pincery_llm_call_total";
pub const LLM_PROMPT_TOKENS: &str = "open_pincery_llm_prompt_tokens_total";
pub const LLM_COMPLETION_TOKENS: &str = "open_pincery_llm_completion_tokens_total";
pub const TOOL_CALL: &str = "open_pincery_tool_call_total";
pub const WEBHOOK_RECEIVED: &str = "open_pincery_webhook_received_total";
pub const RATE_LIMIT_REJECTED: &str = "open_pincery_rate_limit_rejected_total";

/// Install a Prometheus recorder and return a handle that renders the
/// current snapshot on demand.
pub fn install_recorder() -> PrometheusHandle {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("metrics recorder already installed")
}
