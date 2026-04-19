use axum::{
    extract::{Extension, Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AppState, AuthUser};
use crate::error::AppError;
use crate::models::agent;

#[derive(Deserialize)]
struct CreateAgent {
    name: String,
}

#[derive(Serialize)]
struct AgentResponse {
    id: Uuid,
    name: String,
    status: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl From<agent::Agent> for AgentResponse {
    fn from(a: agent::Agent) -> Self {
        Self {
            id: a.id,
            name: a.name,
            status: a.status,
            created_at: a.created_at,
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/agents", post(create_agent).get(list_agents))
        .route("/agents/{id}", get(get_agent_handler))
}

async fn create_agent(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateAgent>,
) -> Result<(axum::http::StatusCode, Json<AgentResponse>), AppError> {
    let a = agent::create_agent(&state.pool, &body.name, auth.workspace_id, auth.user_id).await?;
    Ok((axum::http::StatusCode::CREATED, Json(a.into())))
}

async fn list_agents(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Vec<AgentResponse>>, AppError> {
    let agents = agent::list_agents(&state.pool, auth.workspace_id).await?;
    Ok(Json(agents.into_iter().map(|a| a.into()).collect()))
}

async fn get_agent_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<AgentResponse>, AppError> {
    let a = agent::get_agent(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound("Agent not found".into()))?;
    Ok(Json(a.into()))
}
