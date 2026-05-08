use axum::{
    extract::{Extension, Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::{scoped_agent, AppState, AuthUser};
use crate::error::AppError;
use crate::models::event::{self, Event};

#[derive(Deserialize, IntoParams)]
pub struct EventQuery {
    pub limit: Option<i64>,
    pub since: Option<Uuid>,
}

#[derive(Serialize, ToSchema)]
pub struct EventsResponse {
    pub events: Vec<Event>,
    pub total: i64,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/agents/{id}/events", get(get_events))
}

/// Stream recent events for an agent. Optional `since` filters to
/// events strictly after the given event UUID; optional `limit` caps
/// results at up to 1000.
#[utoipa::path(
    get,
    path = "/api/agents/{id}/events",
    tag = "events",
    params(
        ("id" = Uuid, Path, description = "Agent ID"),
        EventQuery,
    ),
    responses(
        (status = 200, description = "Events for agent", body = EventsResponse),
        (status = 404, description = "Agent not found"),
    ),
    security(("bearerAuth" = [])),
)]
pub async fn get_events(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
    Extension(auth): Extension<AuthUser>,
    Query(params): Query<EventQuery>,
) -> Result<Json<EventsResponse>, AppError> {
    scoped_agent(&state, &auth, agent_id).await?;

    let limit = params.limit.unwrap_or(100).min(1000);
    let events = if let Some(since_id) = params.since {
        event::events_since_id(&state.pool, agent_id, since_id, limit).await?
    } else {
        event::recent_events(&state.pool, agent_id, limit).await?
    };
    let total = events.len() as i64;
    Ok(Json(EventsResponse { events, total }))
}
