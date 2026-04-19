use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub auth_provider: String,
    pub auth_subject: String,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub session_token_hash: String,
    pub auth_provider: String,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

pub async fn create_local_admin(
    pool: &PgPool,
    email: &str,
    display_name: &str,
) -> Result<User, AppError> {
    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (email, display_name, auth_provider, auth_subject)
         VALUES ($1, $2, 'local_admin', 'bootstrap')
         RETURNING *"
    )
    .bind(email)
    .bind(display_name)
    .fetch_one(pool)
    .await?;
    Ok(user)
}

pub async fn find_local_admin(pool: &PgPool) -> Result<Option<User>, AppError> {
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE auth_provider = 'local_admin' AND auth_subject = 'bootstrap' LIMIT 1"
    )
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

pub async fn create_session(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    auth_provider: &str,
) -> Result<UserSession, AppError> {
    let session = sqlx::query_as::<_, UserSession>(
        "INSERT INTO user_sessions (user_id, session_token_hash, auth_provider, expires_at)
         VALUES ($1, $2, $3, NOW() + INTERVAL '30 days')
         RETURNING id, user_id, session_token_hash, auth_provider, created_at, last_seen_at, expires_at, revoked_at"
    )
    .bind(user_id)
    .bind(token_hash)
    .bind(auth_provider)
    .fetch_one(pool)
    .await?;
    Ok(session)
}

pub async fn find_session_by_token_hash(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<UserSession>, AppError> {
    let session = sqlx::query_as::<_, UserSession>(
        "SELECT id, user_id, session_token_hash, auth_provider, created_at, last_seen_at, expires_at, revoked_at
         FROM user_sessions
         WHERE session_token_hash = $1
           AND revoked_at IS NULL
           AND expires_at > NOW()"
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    Ok(session)
}
