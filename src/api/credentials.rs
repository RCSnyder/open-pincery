//! AC-39 (v7): credentials REST API.
//!
//! Endpoints (mounted under the authenticated `/api` subtree):
//!
//! - `POST   /workspaces/{id}/credentials`       create (admin-only)
//! - `GET    /workspaces/{id}/credentials`       list  (admin-only, names only)
//! - `DELETE /workspaces/{id}/credentials/{name}` revoke (admin-only)
//!
//! All three handlers:
//!   1. Verify the path `workspace_id` matches the session's workspace
//!      (single-workspace deployments; cross-workspace is 404).
//!   2. Gate on [`credential::is_workspace_admin`]; non-admin → 403 +
//!      `credential_forbidden` audit row.
//!   3. Emit a `credential_added` / `credential_revoked` audit row on
//!      success.
//!
//! Wire projections never carry `ciphertext`, `nonce`, or `value`.

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    routing::{delete, post},
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use super::{AppState, AuthUser};
use crate::error::AppError;
use crate::models::credential::{self, CredentialSummary};

#[derive(Deserialize)]
struct CreateCredentialBody {
    name: String,
    value: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/workspaces/{id}/credentials",
            post(create_handler).get(list_handler),
        )
        .route(
            "/workspaces/{id}/credentials/{name}",
            delete(revoke_handler),
        )
}

/// Common pre-flight: path workspace matches session workspace AND the
/// caller is a workspace admin. Returns Unit on success; 404 if the
/// workspace isn't the caller's, 403 + audit if they are not admin.
async fn require_workspace_admin(
    state: &AppState,
    auth: &AuthUser,
    path_ws_id: Uuid,
    intent: &str,
) -> Result<(), AppError> {
    if path_ws_id != auth.workspace_id {
        // Do not leak existence of other workspaces.
        return Err(AppError::NotFound("workspace not found".into()));
    }
    if !credential::is_workspace_admin(&state.pool, path_ws_id, auth.user_id).await? {
        let _ = credential::append_audit(
            &state.pool,
            path_ws_id,
            auth.user_id,
            "credential_forbidden",
            intent,
        )
        .await;
        return Err(AppError::Forbidden("workspace admin role required".into()));
    }
    Ok(())
}

async fn create_handler(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(ws_id): Path<Uuid>,
    Json(body): Json<CreateCredentialBody>,
) -> Result<(StatusCode, Json<CredentialSummary>), AppError> {
    credential::validate_name(&body.name)?;
    let value_bytes = body.value.as_bytes();
    credential::validate_value_bytes(value_bytes)?;

    require_workspace_admin(&state, &auth, ws_id, &body.name).await?;

    // Seal — the vault is the only place value plaintext lives beyond
    // this function. `body.value` drops at the end of this function
    // and is never logged or serialized.
    let sealed = state
        .vault
        .seal(ws_id, &body.name, value_bytes)
        .map_err(|e| AppError::Internal(format!("seal failed: {e}")))?;

    let row = credential::create(
        &state.pool,
        ws_id,
        &body.name,
        &sealed.ciphertext,
        &sealed.nonce,
        auth.user_id,
    )
    .await?;

    credential::append_audit(
        &state.pool,
        ws_id,
        auth.user_id,
        "credential_added",
        &body.name,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(CredentialSummary {
            name: row.name,
            created_at: row.created_at,
            created_by: row.created_by,
        }),
    ))
}

async fn list_handler(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(ws_id): Path<Uuid>,
) -> Result<Json<Vec<CredentialSummary>>, AppError> {
    require_workspace_admin(&state, &auth, ws_id, "list").await?;
    let rows = credential::list_active(&state.pool, ws_id).await?;
    Ok(Json(rows))
}

async fn revoke_handler(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((ws_id, name)): Path<(Uuid, String)>,
) -> Result<StatusCode, AppError> {
    credential::validate_name(&name)?;
    require_workspace_admin(&state, &auth, ws_id, &name).await?;

    let revoked = credential::revoke(&state.pool, ws_id, &name).await?;
    if !revoked {
        return Err(AppError::NotFound(format!("credential '{name}' not found")));
    }

    credential::append_audit(
        &state.pool,
        ws_id,
        auth.user_id,
        "credential_revoked",
        &name,
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
