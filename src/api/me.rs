//! AC-40 (v7): minimal "who am I" endpoint so the CLI can discover the
//! session's workspace_id without a bootstrap response. Returns the
//! fields already materialised by `auth_middleware`.

use axum::{extract::Extension, routing::get, Json, Router};
use serde::Serialize;
use uuid::Uuid;

use super::{AppState, AuthUser};

#[derive(Serialize)]
struct MeResponse {
    user_id: Uuid,
    workspace_id: Uuid,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/me", get(me_handler))
}

async fn me_handler(Extension(auth): Extension<AuthUser>) -> Json<MeResponse> {
    Json(MeResponse {
        user_id: auth.user_id,
        workspace_id: auth.workspace_id,
    })
}
