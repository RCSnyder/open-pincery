//! AC-40 (v7): minimal "who am I" endpoint so the CLI can discover the
//! session's workspace_id without a bootstrap response. Returns the
//! fields already materialised by `auth_middleware`.
//!
//! AC-44 (v8): annotated with `#[utoipa::path]` and `ToSchema`-deriving
//! response DTO so the endpoint appears in `/openapi.json`.

use axum::{extract::Extension, routing::get, Json, Router};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use super::{AppState, AuthUser};

/// Response body for `GET /api/me`.
#[derive(Serialize, ToSchema)]
pub struct MeResponse {
    /// The authenticated user's UUID.
    #[schema(example = "018f2b71-9a4a-7c8b-9c3b-4d5e6f7a8b9c")]
    pub user_id: Uuid,
    /// The workspace resolved for the session.
    #[schema(example = "018f2b71-9a4a-7c8b-9c3b-4d5e6f7a8b9c")]
    pub workspace_id: Uuid,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/me", get(me_handler))
}

/// Return the authenticated user's id and the workspace the session
/// resolves to.
#[utoipa::path(
    get,
    path = "/api/me",
    tag = "me",
    responses(
        (status = 200, description = "Session identity", body = MeResponse),
        (status = 401, description = "Missing or invalid session token"),
    ),
    security(("bearerAuth" = [])),
)]
pub async fn me_handler(Extension(auth): Extension<AuthUser>) -> Json<MeResponse> {
    Json(MeResponse {
        user_id: auth.user_id,
        workspace_id: auth.workspace_id,
    })
}
