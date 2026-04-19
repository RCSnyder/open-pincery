//! AC-19: Health and readiness endpoints.
//!
//! `/health` is pure liveness — 200 whenever the process is running.
//! `/ready` verifies the app can serve traffic: DB reachable + all expected
//! migrations applied + every background task alive. Container orchestrators
//! should route traffic based on readiness, and restart based on liveness.

use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};
use std::sync::atomic::Ordering;

use crate::api::AppState;

/// Liveness: always 200 if the process is up and the HTTP handler runs.
pub async fn health() -> (StatusCode, Json<Value>) {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

/// Readiness: 200 only when DB is reachable AND migrations applied AND
/// every background task is alive. Returns 503 with `failing` field
/// naming the failed check otherwise.
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

    // Check 2: all expected migrations applied (success = true).
    let expected = crate::db::expected_migration_count() as i64;
    let applied: Result<i64, _> =
        sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE success = TRUE")
            .fetch_one(&state.pool)
            .await;
    match applied {
        Ok(n) if n >= expected => {}
        Ok(n) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "status": "not_ready",
                    "failing": "migrations",
                    "expected": expected,
                    "applied": n
                })),
            );
        }
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "status": "not_ready", "failing": "migrations" })),
            );
        }
    }

    // Check 3: every background task alive (AND, not OR).
    let listener = state.listener_alive.load(Ordering::Relaxed);
    let stale = state.stale_alive.load(Ordering::Relaxed);
    if !(listener && stale) {
        let failing = if !listener && !stale {
            "background_tasks"
        } else if !listener {
            "background_task:listener"
        } else {
            "background_task:stale_recovery"
        };
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "not_ready", "failing": failing })),
        );
    }

    (StatusCode::OK, Json(json!({ "status": "ready" })))
}
