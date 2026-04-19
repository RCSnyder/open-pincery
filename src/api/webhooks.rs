use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header::HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::api::AppState;
use crate::models::{agent, event};

type HmacSha256 = Hmac<Sha256>;

pub fn router() -> Router<AppState> {
    Router::new().route("/agents/{id}/webhooks", post(receive_webhook))
}

fn verify_hmac(secret: &[u8], payload: &[u8], signature: &str) -> bool {
    let hex_sig = signature.strip_prefix("sha256=").unwrap_or(signature);
    let sig_bytes = match hex::decode(hex_sig) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let mut mac = match HmacSha256::new_from_slice(secret) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(payload);
    mac.verify_slice(&sig_bytes).is_ok()
}

#[derive(serde::Deserialize)]
struct WebhookPayload {
    content: String,
    source: Option<String>,
}

async fn receive_webhook(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, StatusCode> {
    // Look up agent
    let a = agent::get_agent(&state.pool, agent_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if !a.is_enabled {
        return Err(StatusCode::FORBIDDEN);
    }

    // Verify HMAC signature
    let signature = headers
        .get("x-webhook-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if !verify_hmac(a.webhook_secret.as_bytes(), &body, signature) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Check idempotency key
    let idempotency_key = headers
        .get("x-idempotency-key")
        .and_then(|v| v.to_str().ok());

    if let Some(key) = idempotency_key {
        // Try insert; if it already exists, this is a duplicate
        let inserted = sqlx::query_scalar::<_, bool>(
            "INSERT INTO webhook_dedup (idempotency_key, agent_id)
             VALUES ($1, $2)
             ON CONFLICT (idempotency_key, agent_id) DO NOTHING
             RETURNING TRUE"
        )
        .bind(key)
        .bind(agent_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if inserted.is_none() {
            // Duplicate — return 200 OK without creating a new event
            return Ok((StatusCode::OK, axum::Json(serde_json::json!({"status": "duplicate"}))));
        }
    }

    // Parse payload
    let payload: WebhookPayload = serde_json::from_slice(&body)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let source = payload.source.as_deref().unwrap_or("webhook");

    // Append event
    event::append_event(
        &state.pool,
        agent_id,
        "webhook_received",
        source,
        None, None, None, None,
        Some(&payload.content),
        None,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Trigger wake via NOTIFY
    sqlx::query("SELECT pg_notify('agent_wake', $1::text)")
        .bind(agent_id.to_string())
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::ACCEPTED, axum::Json(serde_json::json!({"status": "accepted"}))))
}
