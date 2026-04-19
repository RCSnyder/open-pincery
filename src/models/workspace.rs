use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub deployment_mode: String,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Workspace {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub slug: String,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
}

pub async fn create_organization(
    pool: &PgPool,
    name: &str,
    slug: &str,
    created_by: Uuid,
) -> Result<Organization, AppError> {
    let org = sqlx::query_as::<_, Organization>(
        "INSERT INTO organizations (name, slug, created_by)
         VALUES ($1, $2, $3)
         RETURNING *",
    )
    .bind(name)
    .bind(slug)
    .bind(created_by)
    .fetch_one(pool)
    .await?;
    Ok(org)
}

pub async fn create_workspace(
    pool: &PgPool,
    org_id: Uuid,
    name: &str,
    slug: &str,
    created_by: Uuid,
) -> Result<Workspace, AppError> {
    let ws = sqlx::query_as::<_, Workspace>(
        "INSERT INTO workspaces (organization_id, name, slug, created_by)
         VALUES ($1, $2, $3, $4)
         RETURNING *",
    )
    .bind(org_id)
    .bind(name)
    .bind(slug)
    .bind(created_by)
    .fetch_one(pool)
    .await?;
    Ok(ws)
}

pub async fn add_org_membership(
    pool: &PgPool,
    org_id: Uuid,
    user_id: Uuid,
    role: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO organization_memberships (organization_id, user_id, role)
         VALUES ($1, $2, $3)",
    )
    .bind(org_id)
    .bind(user_id)
    .bind(role)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn add_workspace_membership(
    pool: &PgPool,
    ws_id: Uuid,
    user_id: Uuid,
    role: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
         VALUES ($1, $2, $3)",
    )
    .bind(ws_id)
    .bind(user_id)
    .bind(role)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_workspace_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<Workspace>, AppError> {
    let ws = sqlx::query_as::<_, Workspace>(
        "SELECT w.* FROM workspaces w
         JOIN workspace_memberships wm ON wm.workspace_id = w.id
         WHERE wm.user_id = $1 AND wm.status = 'active'
         LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(ws)
}
