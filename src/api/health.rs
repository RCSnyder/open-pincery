//! AC-19: Health and readiness endpoints.
//!
//! `/health` is pure liveness — 200 whenever the process is running.
//! `/ready` verifies the app can serve traffic: DB reachable + background
//! tasks alive. Container orchestrators should route traffic based on
//! readiness, and restart based on liveness.

use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};
use std::sync::atomic::Ordering;

use crate::api::AppState;

/// Liveness: always 200 if the process is up and the HTTP handler runs.
pub async fn health() -> (StatusCode, Json<Value>) {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

/// Readiness: 200 only when DB is reachable AND background tasks alive.
/// Returns 503 with `failing` field naming the failed check otherwise.
pub async fn ready(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    // Check 1: DB round-trip.
    if sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_err()
    {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "not_ready", "failing": "database" })),
        );
    }

    // Check 2: background tasks alive (listener + stale recovery).
    if !state.background_alive.load(Ordering::Relaxed) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "not_ready", "failing": "background_tasks" })),
        );
    }

    (StatusCode::OK, Json(json!({ "status": "ready" })))
}
