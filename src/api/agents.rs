use axum::{
    extract::{Extension, Path, State},
    routing::{get, post},
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{scoped_agent, AppState, AuthUser};
use crate::error::AppError;
use crate::models::{agent, event, projection};

#[derive(Deserialize, ToSchema)]
pub struct CreateAgent {
    pub name: String,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateAgent {
    pub name: Option<String>,
    pub is_enabled: Option<bool>,
    #[schema(value_type = String, format = "decimal")]
    pub budget_limit_usd: Option<Decimal>,
}

#[derive(Serialize, ToSchema)]
pub struct AgentResponse {
    pub id: Uuid,
    pub name: String,
    pub status: String,
    pub is_enabled: bool,
    pub disabled_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_secret: Option<String>,
    pub identity: Option<String>,
    pub work_list: Option<String>,
    #[schema(value_type = String, format = "decimal")]
    pub budget_limit_usd: Decimal,
    #[schema(value_type = String, format = "decimal")]
    pub budget_used_usd: Decimal,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct RotateWebhookSecretResponse {
    pub webhook_secret: String,
}

impl AgentResponse {
    fn from_agent(
        a: agent::Agent,
        proj: Option<projection::AgentProjection>,
        include_secret: bool,
    ) -> Self {
        Self {
            id: a.id,
            name: a.name,
            status: a.status,
            is_enabled: a.is_enabled,
            disabled_reason: a.disabled_reason,
            webhook_secret: if include_secret {
                Some(a.webhook_secret)
            } else {
                None
            },
            identity: proj.as_ref().map(|p| p.identity.clone()),
            work_list: proj.as_ref().map(|p| p.work_list.clone()),
            budget_limit_usd: a.budget_limit_usd,
            budget_used_usd: a.budget_used_usd,
            created_at: a.created_at,
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/agents", post(create_agent).get(list_agents))
        .route(
            "/agents/{id}",
            get(get_agent_handler)
                .patch(update_agent_handler)
                .delete(delete_agent_handler),
        )
        .route(
            "/agents/{id}/webhook/rotate",
            post(rotate_webhook_secret_handler),
        )
}

/// Create a new agent within the caller's workspace.
#[utoipa::path(
    post,
    path = "/api/agents",
    tag = "agents",
    request_body = CreateAgent,
    responses(
        (status = 201, description = "Agent created", body = AgentResponse),
        (status = 401, description = "Missing or invalid session"),
    ),
    security(("bearerAuth" = [])),
)]
pub async fn create_agent(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateAgent>,
) -> Result<(axum::http::StatusCode, Json<AgentResponse>), AppError> {
    let a = agent::create_agent(&state.pool, &body.name, auth.workspace_id, auth.user_id).await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(AgentResponse::from_agent(a, None, true)),
    ))
}

/// List all agents in the caller's workspace.
#[utoipa::path(
    get,
    path = "/api/agents",
    tag = "agents",
    responses(
        (status = 200, description = "Agents in workspace", body = [AgentResponse]),
        (status = 401, description = "Missing or invalid session"),
    ),
    security(("bearerAuth" = [])),
)]
pub async fn list_agents(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Vec<AgentResponse>>, AppError> {
    let agents = agent::list_agents(&state.pool, auth.workspace_id).await?;
    Ok(Json(
        agents
            .into_iter()
            .map(|a| AgentResponse::from_agent(a, None, false))
            .collect(),
    ))
}

/// Fetch a single agent by ID.
#[utoipa::path(
    get,
    path = "/api/agents/{id}",
    tag = "agents",
    params(("id" = Uuid, Path, description = "Agent ID")),
    responses(
        (status = 200, description = "Agent", body = AgentResponse),
        (status = 404, description = "Agent not found"),
    ),
    security(("bearerAuth" = [])),
)]
pub async fn get_agent_handler(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<AgentResponse>, AppError> {
    let a = scoped_agent(&state, &auth, id).await?;
    let proj = projection::latest_projection(&state.pool, id).await?;
    Ok(Json(AgentResponse::from_agent(a, proj, false)))
}

/// Update mutable fields on an agent.
#[utoipa::path(
    patch,
    path = "/api/agents/{id}",
    tag = "agents",
    params(("id" = Uuid, Path, description = "Agent ID")),
    request_body = UpdateAgent,
    responses(
        (status = 200, description = "Agent updated", body = AgentResponse),
        (status = 404, description = "Agent not found"),
    ),
    security(("bearerAuth" = [])),
)]
pub async fn update_agent_handler(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateAgent>,
) -> Result<Json<AgentResponse>, AppError> {
    scoped_agent(&state, &auth, id).await?;

    let disabled_reason = match body.is_enabled {
        Some(false) => Some("disabled_by_user"),
        _ => None,
    };
    let a = agent::update_agent(
        &state.pool,
        id,
        body.name.as_deref(),
        body.is_enabled,
        disabled_reason,
        body.budget_limit_usd,
    )
    .await?;
    Ok(Json(AgentResponse::from_agent(a, None, false)))
}

/// Soft-delete an agent.
#[utoipa::path(
    delete,
    path = "/api/agents/{id}",
    tag = "agents",
    params(("id" = Uuid, Path, description = "Agent ID")),
    responses(
        (status = 200, description = "Agent soft-deleted", body = AgentResponse),
        (status = 404, description = "Agent not found"),
    ),
    security(("bearerAuth" = [])),
)]
pub async fn delete_agent_handler(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<AgentResponse>, AppError> {
    scoped_agent(&state, &auth, id).await?;

    let a = agent::soft_delete_agent(&state.pool, id).await?;
    Ok(Json(AgentResponse::from_agent(a, None, false)))
}

/// Rotate an agent's webhook HMAC secret. Returns the new secret
/// exactly once; it cannot be retrieved afterwards.
#[utoipa::path(
    post,
    path = "/api/agents/{id}/webhook/rotate",
    tag = "agents",
    params(("id" = Uuid, Path, description = "Agent ID")),
    responses(
        (status = 200, description = "New webhook secret", body = RotateWebhookSecretResponse),
        (status = 404, description = "Agent not found"),
    ),
    security(("bearerAuth" = [])),
)]
pub async fn rotate_webhook_secret_handler(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<RotateWebhookSecretResponse>, AppError> {
    scoped_agent(&state, &auth, id).await?;

    let new_secret = crate::auth::generate_webhook_secret();
    let mut tx = state.pool.begin().await?;
    let _rotated = agent::rotate_webhook_secret_tx(&mut tx, id, &new_secret).await?;

    event::append_event_tx(
        &mut tx,
        id,
        "webhook_secret_rotated",
        "api",
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await?;
    tx.commit().await?;

    Ok(Json(RotateWebhookSecretResponse {
        webhook_secret: new_secret,
    }))
}
