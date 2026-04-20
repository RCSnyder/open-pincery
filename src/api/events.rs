use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AppState;
use crate::error::AppError;
use crate::models::event::{self, Event};

#[derive(Deserialize)]
struct EventQuery {
    limit: Option<i64>,
    since: Option<Uuid>,
}

#[derive(Serialize)]
struct EventsResponse {
    events: Vec<Event>,
    total: i64,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/agents/{id}/events", get(get_events))
}

async fn get_events(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
    Query(params): Query<EventQuery>,
) -> Result<Json<EventsResponse>, AppError> {
    let limit = params.limit.unwrap_or(100).min(1000);
    let events = if let Some(since_id) = params.since {
        event::events_since_id(&state.pool, agent_id, since_id, limit).await?
    } else {
        event::recent_events(&state.pool, agent_id, limit).await?
    };
    let total = events.len() as i64;
    Ok(Json(EventsResponse { events, total }))
}
