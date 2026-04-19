use axum::{
    extract::{ConnectInfo, Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
    Router,
};
use governor::{
    clock::DefaultClock,
    state::keyed::DashMapStateStore,
    Quota, RateLimiter,
};
use sqlx::PgPool;
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};
use uuid::Uuid;

pub mod agents;
pub mod bootstrap;
pub mod events;
pub mod messages;
pub mod webhooks;

use crate::models::user;

type KeyedRateLimiter = RateLimiter<IpAddr, DashMapStateStore<IpAddr>, DefaultClock>;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: crate::config::Config,
    pub unauth_limiter: Arc<KeyedRateLimiter>,
    pub auth_limiter: Arc<KeyedRateLimiter>,
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
    // Check X-Forwarded-For header first (for reverse proxies)
    if let Some(forwarded) = req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first_ip) = forwarded.split(',').next() {
            if let Ok(ip) = first_ip.trim().parse::<IpAddr>() {
                return ip;
            }
        }
    }
    // Fall back to connected peer address
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip())
        .unwrap_or(IpAddr::from([127, 0, 0, 1]))
}

pub async fn unauth_rate_limit(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let ip = extract_client_ip(&req);
    state
        .unauth_limiter
        .check_key(&ip)
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    Ok(next.run(req).await)
}

pub async fn auth_rate_limit(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let ip = extract_client_ip(&req);
    state
        .auth_limiter
        .check_key(&ip)
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    Ok(next.run(req).await)
}

impl AppState {
    pub fn new(pool: PgPool, config: crate::config::Config) -> Self {
        let unauth_limiter = Arc::new(RateLimiter::keyed(
            Quota::per_minute(NonZeroU32::new(10).unwrap()),
        ));
        let auth_limiter = Arc::new(RateLimiter::keyed(
            Quota::per_minute(NonZeroU32::new(60).unwrap()),
        ));
        Self {
            pool,
            config,
            unauth_limiter,
            auth_limiter,
        }
    }
}

pub fn router(state: AppState) -> Router {
    let authed = Router::new()
        .merge(agents::router())
        .merge(messages::router())
        .merge(events::router())
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_rate_limit));

    let unauthed = Router::new()
        .merge(bootstrap::router())
        .nest("/api", webhooks::router())
        .layer(axum::middleware::from_fn_with_state(state.clone(), unauth_rate_limit));

    // Serve static UI files, falling back to index.html for SPA routing
    let static_files = ServeDir::new("static")
        .not_found_service(ServeFile::new("static/index.html"));

    Router::new()
        .route("/health", axum::routing::get(health_check))
        .merge(unauthed)
        .nest("/api", authed)
        .fallback_service(static_files)
        .with_state(state)
}

async fn health_check(
    State(state): State<AppState>,
) -> axum::Json<serde_json::Value> {
    let db_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();
    axum::Json(serde_json::json!({
        "status": if db_ok { "ok" } else { "degraded" },
        "db": if db_ok { "connected" } else { "disconnected" }
    }))
}
