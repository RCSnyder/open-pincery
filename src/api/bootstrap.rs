use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use super::AppState;
use crate::error::AppError;
use crate::models::{user, workspace};

/// Response from POST /api/bootstrap.
#[derive(Serialize, ToSchema)]
pub struct BootstrapResponse {
    pub user_id: Uuid,
    pub organization_id: Uuid,
    pub workspace_id: Uuid,
    pub session_token: String,
}

/// Response from POST /api/login.
#[derive(Serialize, ToSchema)]
pub struct LoginResponse {
    pub user_id: Uuid,
    pub session_token: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/bootstrap", post(bootstrap))
        .route("/api/login", post(login))
}

/// Initialise the system. Creates the local admin user, default
/// organization, default workspace, and returns a session token.
/// Requires the `OPEN_PINCERY_BOOTSTRAP_TOKEN` as a bearer header.
/// Returns 409 Conflict if the system has already been bootstrapped.
#[utoipa::path(
    post,
    path = "/api/bootstrap",
    tag = "auth",
    responses(
        (status = 201, description = "System bootstrapped", body = BootstrapResponse),
        (status = 401, description = "Missing or invalid bootstrap token"),
        (status = 409, description = "System already bootstrapped"),
    ),
)]
pub async fn bootstrap(
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
        return Err(AppError::Conflict(
            "System already bootstrapped. Use 'pcy login --bootstrap-token <token>' or POST /api/login to get a new session token.".into(),
        ));
    }

    let admin = user::create_local_admin(&state.pool, "admin@localhost", "Admin").await?;
    let org = workspace::create_organization(&state.pool, "default", "default", admin.id).await?;
    let ws = workspace::create_workspace(
        &state.pool,
        org.id,
        "default",
        "Default Workspace",
        admin.id,
    )
    .await?;
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

/// Obtain a new session token for the existing local admin, using
/// the bootstrap token. AC-45 (v8) makes `pcy login` idempotent by
/// layering this on top of the bootstrap call.
#[utoipa::path(
    post,
    path = "/api/login",
    tag = "auth",
    responses(
        (status = 200, description = "Session token issued", body = LoginResponse),
        (status = 400, description = "System not yet bootstrapped"),
        (status = 401, description = "Missing or invalid bootstrap token"),
    ),
)]
pub async fn login(
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

    let admin = user::find_local_admin(&state.pool)
        .await?
        .ok_or(AppError::BadRequest(
            "System not bootstrapped yet. Run 'pcy login --bootstrap-token <token>' first.".into(),
        ))?;

    let raw_token = crate::auth::generate_token();
    let token_hash = crate::auth::hash_token(&raw_token);
    user::create_session(&state.pool, admin.id, &token_hash, "local_admin").await?;

    Ok((
        StatusCode::OK,
        Json(LoginResponse {
            user_id: admin.id,
            session_token: raw_token,
        }),
    ))
}
