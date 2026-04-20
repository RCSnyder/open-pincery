use axum::{
    extract::{ConnectInfo, Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
    Router,
};
use governor::{clock::DefaultClock, state::keyed::DashMapStateStore, Quota, RateLimiter};
use sqlx::PgPool;
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};
use uuid::Uuid;

pub mod agents;
pub mod bootstrap;
pub mod events;
pub mod health;
pub mod messages;
pub mod webhooks;

use crate::{
    error::AppError,
    models::{agent, user},
};

type KeyedRateLimiter = RateLimiter<IpAddr, DashMapStateStore<IpAddr>, DefaultClock>;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: crate::config::Config,
    pub unauth_limiter: Arc<KeyedRateLimiter>,
    pub auth_limiter: Arc<KeyedRateLimiter>,
    /// AC-19: per-background-task liveness flags. Each task sets its own
    /// flag to `true` when its main loop is serving, and back to `false`
    /// when it exits (clean shutdown or error). `/ready` requires BOTH
    /// to be `true`.
    pub listener_alive: Arc<AtomicBool>,
    pub stale_alive: Arc<AtomicBool>,
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

fn extract_client_ip(req: &Request) -> IpAddr {
    // Use connected peer address only — do not trust X-Forwarded-For
    // as self-hosted deployments typically run without a reverse proxy
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip())
        .unwrap_or(IpAddr::from([127, 0, 0, 1]))
}

fn rate_limit_response() -> Response {
    metrics::counter!(crate::observability::metrics::RATE_LIMIT_REJECTED).increment(1);
    let mut resp = Response::new(axum::body::Body::from("Too Many Requests"));
    *resp.status_mut() = StatusCode::TOO_MANY_REQUESTS;
    resp.headers_mut()
        .insert("retry-after", "60".parse().unwrap());
    resp
}

pub async fn unauth_rate_limit(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    let ip = extract_client_ip(&req);
    state
        .unauth_limiter
        .check_key(&ip)
        .map_err(|_| rate_limit_response())?;
    Ok(next.run(req).await)
}

pub async fn auth_rate_limit(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    let ip = extract_client_ip(&req);
    state
        .auth_limiter
        .check_key(&ip)
        .map_err(|_| rate_limit_response())?;
    Ok(next.run(req).await)
}

impl AppState {
    pub fn new(pool: PgPool, config: crate::config::Config) -> Self {
        let unauth_limiter = Arc::new(RateLimiter::keyed(Quota::per_minute(
            NonZeroU32::new(10).unwrap(),
        )));
        let auth_limiter = Arc::new(RateLimiter::keyed(Quota::per_minute(
            NonZeroU32::new(60).unwrap(),
        )));
        Self {
            pool,
            config,
            unauth_limiter,
            auth_limiter,
            listener_alive: Arc::new(AtomicBool::new(false)),
            stale_alive: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub(crate) async fn scoped_agent(
    state: &AppState,
    auth: &AuthUser,
    agent_id: Uuid,
) -> Result<agent::Agent, AppError> {
    let found = agent::get_agent(&state.pool, agent_id)
        .await?
        .ok_or(AppError::NotFound("Agent not found".into()))?;

    if found.workspace_id != auth.workspace_id {
        return Err(AppError::Forbidden("Forbidden".into()));
    }

    Ok(found)
}

pub fn router(state: AppState) -> Router {
    let authed = Router::new()
        .merge(agents::router())
        .merge(messages::router())
        .merge(events::router())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_rate_limit,
        ));

    let unauthed = Router::new()
        .merge(bootstrap::router())
        .nest("/api", webhooks::router())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            unauth_rate_limit,
        ));

    // Serve static UI files, falling back to index.html for SPA routing
    let static_files =
        ServeDir::new("static").not_found_service(ServeFile::new("static/index.html"));

    Router::new()
        .route("/health", axum::routing::get(health::health))
        .route("/ready", axum::routing::get(health::ready))
        .merge(unauthed)
        .nest("/api", authed)
        .fallback_service(static_files)
        .with_state(state)
}
