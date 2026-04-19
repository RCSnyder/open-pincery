use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::Serialize;
use uuid::Uuid;

use super::AppState;
use crate::error::AppError;
use crate::models::{user, workspace};

#[derive(Serialize)]
struct BootstrapResponse {
    user_id: Uuid,
    organization_id: Uuid,
    workspace_id: Uuid,
    session_token: String,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/bootstrap", post(bootstrap))
}

async fn bootstrap(
    State(state): State<AppState>,
    req: axum::extract::Request,
) -> Result<impl IntoResponse, AppError> {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized("Missing bearer token".into()))?;

    if token != state.config.bootstrap_token {
        return Err(AppError::Unauthorized("Invalid bootstrap token".into()));
    }

    // Check if already bootstrapped
    let existing = user::find_local_admin(&state.pool).await?;
    if existing.is_some() {
        return Err(AppError::Conflict("System already bootstrapped".into()));
    }

    let admin = user::create_local_admin(&state.pool, "admin@localhost", "Admin").await?;
    let org = workspace::create_organization(&state.pool, "default", "default", admin.id).await?;
    let ws = workspace::create_workspace(&state.pool, org.id, "default", "Default Workspace", admin.id).await?;
    workspace::add_org_membership(&state.pool, org.id, admin.id, "owner").await?;
    workspace::add_workspace_membership(&state.pool, ws.id, admin.id, "owner").await?;

    let raw_token = crate::auth::generate_token();
    let token_hash = crate::auth::hash_token(&raw_token);
    user::create_session(&state.pool, admin.id, &token_hash, "local_admin").await?;

    Ok((
        StatusCode::CREATED,
        Json(BootstrapResponse {
            user_id: admin.id,
            organization_id: org.id,
            workspace_id: ws.id,
            session_token: raw_token,
        }),
    ))
}
