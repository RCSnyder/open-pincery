//! AC-93 (v9.1): LLM providers REST API.
//!
//! Endpoints (mounted under the authenticated `/api` subtree):
//!
//! - `POST   /workspaces/{id}/providers`              create (admin-only)
//! - `GET    /workspaces/{id}/providers`              list (admin-only)
//! - `POST   /workspaces/{id}/providers/{name}/default`  set default (admin-only)
//! - `DELETE /workspaces/{id}/providers/{name}`       delete (admin-only)
//!
//! Mirrors the credential admin gate: path workspace must match the
//! session workspace and the caller must be a workspace admin.

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    routing::{delete, post},
    Json, Router,
};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use super::{AppState, AuthUser};
use crate::error::AppError;
use crate::models::credential;
use crate::models::llm_provider::{self, ProviderRow};

#[derive(Deserialize, ToSchema)]
pub struct CreateProviderBody {
    pub name: String,
    pub base_url: String,
    pub credential_name: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/workspaces/{id}/providers",
            post(create_handler).get(list_handler),
        )
        .route(
            "/workspaces/{id}/providers/{name}/default",
            post(set_default_handler),
        )
        .route("/workspaces/{id}/providers/{name}", delete(delete_handler))
}

/// Common pre-flight (admin gate). Mirrors `credentials::require_workspace_admin`.
async fn require_admin(
    state: &AppState,
    auth: &AuthUser,
    ws_id: Uuid,
    intent: &str,
) -> Result<(), AppError> {
    if ws_id != auth.workspace_id {
        return Err(AppError::NotFound("workspace not found".into()));
    }
    if !credential::is_workspace_admin(&state.pool, ws_id, auth.user_id).await? {
        let _ = credential::append_audit(
            &state.pool,
            ws_id,
            auth.user_id,
            "provider_forbidden",
            intent,
        )
        .await;
        return Err(AppError::Forbidden("workspace admin role required".into()));
    }
    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/workspaces/{id}/providers",
    tag = "providers",
    params(("id" = Uuid, Path, description = "Workspace ID")),
    request_body = CreateProviderBody,
    responses(
        (status = 201, description = "Provider created", body = ProviderRow),
        (status = 400, description = "Validation error or missing credential"),
        (status = 403, description = "Caller is not a workspace admin"),
    ),
    security(("bearerAuth" = [])),
)]
pub async fn create_handler(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(ws_id): Path<Uuid>,
    Json(body): Json<CreateProviderBody>,
) -> Result<(StatusCode, Json<ProviderRow>), AppError> {
    require_admin(&state, &auth, ws_id, &body.name).await?;
    let row = llm_provider::create(
        &state.pool,
        ws_id,
        &body.name,
        &body.base_url,
        &body.credential_name,
    )
    .await?;
    let _ = credential::append_audit(
        &state.pool,
        ws_id,
        auth.user_id,
        "provider_added",
        &body.name,
    )
    .await;
    Ok((StatusCode::CREATED, Json(row)))
}

#[utoipa::path(
    get,
    path = "/api/workspaces/{id}/providers",
    tag = "providers",
    params(("id" = Uuid, Path, description = "Workspace ID")),
    responses(
        (status = 200, description = "Provider list", body = [ProviderRow]),
        (status = 403, description = "Caller is not a workspace admin"),
    ),
    security(("bearerAuth" = [])),
)]
pub async fn list_handler(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(ws_id): Path<Uuid>,
) -> Result<Json<Vec<ProviderRow>>, AppError> {
    require_admin(&state, &auth, ws_id, "list").await?;
    Ok(Json(llm_provider::list(&state.pool, ws_id).await?))
}

#[utoipa::path(
    post,
    path = "/api/workspaces/{id}/providers/{name}/default",
    tag = "providers",
    params(
        ("id" = Uuid, Path, description = "Workspace ID"),
        ("name" = String, Path, description = "Provider name"),
    ),
    responses(
        (status = 204, description = "Default updated"),
        (status = 404, description = "Provider not found"),
        (status = 403, description = "Caller is not a workspace admin"),
    ),
    security(("bearerAuth" = [])),
)]
pub async fn set_default_handler(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((ws_id, name)): Path<(Uuid, String)>,
) -> Result<StatusCode, AppError> {
    llm_provider::validate_name(&name)?;
    require_admin(&state, &auth, ws_id, &name).await?;
    llm_provider::set_default(&state.pool, ws_id, &name).await?;
    let _ = credential::append_audit(
        &state.pool,
        ws_id,
        auth.user_id,
        "provider_default_set",
        &name,
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/workspaces/{id}/providers/{name}",
    tag = "providers",
    params(
        ("id" = Uuid, Path, description = "Workspace ID"),
        ("name" = String, Path, description = "Provider name"),
    ),
    responses(
        (status = 204, description = "Provider deleted"),
        (status = 400, description = "Cannot remove default while siblings exist"),
        (status = 404, description = "Provider not found"),
        (status = 403, description = "Caller is not a workspace admin"),
    ),
    security(("bearerAuth" = [])),
)]
pub async fn delete_handler(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((ws_id, name)): Path<(Uuid, String)>,
) -> Result<StatusCode, AppError> {
    llm_provider::validate_name(&name)?;
    require_admin(&state, &auth, ws_id, &name).await?;
    llm_provider::delete(&state.pool, ws_id, &name).await?;
    let _ =
        credential::append_audit(&state.pool, ws_id, auth.user_id, "provider_removed", &name).await;
    Ok(StatusCode::NO_CONTENT)
}
