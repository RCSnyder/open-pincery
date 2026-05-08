use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{scoped_agent, AppState, AuthUser};
use crate::error::AppError;
use crate::models::event;

#[derive(Deserialize, ToSchema)]
pub struct SendMessage {
    pub content: String,
}

#[derive(Serialize, ToSchema)]
pub struct MessageResponse {
    pub event_id: Uuid,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/agents/{id}/messages", post(send_message))
}

/// Append a `message_received` event to the agent's log and wake the
/// runtime via `pg_notify`. Returns 202 Accepted with the new event's
/// UUID — processing is asynchronous.
#[utoipa::path(
    post,
    path = "/api/agents/{id}/messages",
    tag = "messages",
    params(("id" = Uuid, Path, description = "Agent ID")),
    request_body = SendMessage,
    responses(
        (status = 202, description = "Message accepted", body = MessageResponse),
        (status = 404, description = "Agent not found"),
    ),
    security(("bearerAuth" = [])),
)]
pub async fn send_message(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<SendMessage>,
) -> Result<(StatusCode, Json<MessageResponse>), AppError> {
    scoped_agent(&state, &auth, agent_id).await?;

    let ev = event::append_event(
        &state.pool,
        agent_id,
        "message_received",
        "human",
        None,
        None,
        None,
        None,
        Some(&body.content),
        None,
    )
    .await?;

    // Issue NOTIFY for the agent on the shared channel
    sqlx::query("SELECT pg_notify('agent_wake', $1::text)")
        .bind(agent_id.to_string())
        .execute(&state.pool)
        .await
        .map_err(AppError::Database)?;

    Ok((
        StatusCode::ACCEPTED,
        Json(MessageResponse { event_id: ev.id }),
    ))
}
