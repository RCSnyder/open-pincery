use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
    Router,
};
use sqlx::PgPool;
use uuid::Uuid;

pub mod agents;
pub mod bootstrap;
pub mod events;
pub mod messages;

use crate::models::user;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: crate::config::Config,
}

#[derive(Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub workspace_id: Uuid,
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token_hash = crate::auth::hash_token(token);

    let session = user::find_session_by_token_hash(&state.pool, &token_hash)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Check expiry
    if session.expires_at < chrono::Utc::now() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Find the user's workspace (first workspace they belong to)
    let workspace = crate::models::workspace::find_workspace_for_user(&state.pool, session.user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::FORBIDDEN)?;

    req.extensions_mut().insert(AuthUser {
        user_id: session.user_id,
        workspace_id: workspace.id,
    });

    Ok(next.run(req).await)
}

pub fn router(state: AppState) -> Router {
    let authed = Router::new()
        .merge(agents::router())
        .merge(messages::router())
        .merge(events::router())
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware));

    Router::new()
        .merge(bootstrap::router())
        .nest("/api", authed)
        .with_state(state)
}
