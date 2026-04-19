use axum::{
    extract::{Extension, Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AppState, AuthUser};
use crate::error::AppError;
use crate::models::{agent, projection};

#[derive(Deserialize)]
struct CreateAgent {
    name: String,
}

#[derive(Serialize)]
struct AgentResponse {
    id: Uuid,
    name: String,
    status: String,
    identity: Option<String>,
    work_list: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl AgentResponse {
    fn from_agent(a: agent::Agent, proj: Option<projection::AgentProjection>) -> Self {
        Self {
            id: a.id,
            name: a.name,
            status: a.status,
            identity: proj.as_ref().map(|p| p.identity.clone()),
            work_list: proj.as_ref().map(|p| p.work_list.clone()),
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
    Ok((axum::http::StatusCode::CREATED, Json(AgentResponse::from_agent(a, None))))
}

async fn list_agents(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Vec<AgentResponse>>, AppError> {
    let agents = agent::list_agents(&state.pool, auth.workspace_id).await?;
    Ok(Json(agents.into_iter().map(|a| AgentResponse::from_agent(a, None)).collect()))
}

async fn get_agent_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<AgentResponse>, AppError> {
    let a = agent::get_agent(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound("Agent not found".into()))?;
    let proj = projection::latest_projection(&state.pool, id).await?;
    Ok(Json(AgentResponse::from_agent(a, proj)))
}
