//! AC-93 (v9.1): `llm_providers` table access.
//!
//! Providers are workspace-scoped, named, and point at a base URL +
//! an existing vault credential. At most one row per workspace is the
//! default — enforced by a partial unique index in the migration.
//!
//! This module is the only place that knows the SQL shape of the
//! table; both the REST API ([`crate::api::providers`]) and the wake
//! loop's provider resolver ([`resolve_default`]) call into it.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

/// Hard limits enforced before round-tripping to the DB. The DB also
/// enforces these via CHECK constraints — application-layer rejection
/// is purely for friendly error messages.
const NAME_MIN: usize = 1;
const NAME_MAX: usize = 64;
const URL_MAX: usize = 2048;

#[derive(Debug, Clone, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct ProviderRow {
    pub name: String,
    pub base_url: String,
    pub credential_name: String,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
}

pub fn validate_name(name: &str) -> Result<(), AppError> {
    let len = name.len();
    if !(NAME_MIN..=NAME_MAX).contains(&len) {
        return Err(AppError::BadRequest(format!(
            "provider name must be {NAME_MIN}..={NAME_MAX} bytes"
        )));
    }
    let ok = name
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if !ok {
        return Err(AppError::BadRequest(
            "provider name may only contain [a-z0-9_]".into(),
        ));
    }
    Ok(())
}

pub fn validate_base_url(url: &str) -> Result<(), AppError> {
    if url.is_empty() || url.len() > URL_MAX {
        return Err(AppError::BadRequest(format!(
            "base_url must be 1..={URL_MAX} bytes"
        )));
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(AppError::BadRequest(
            "base_url must start with http:// or https://".into(),
        ));
    }
    Ok(())
}

/// Returns `true` iff a non-revoked credential with `name` exists in
/// `workspace_id`. Used by [`create`] to refuse providers that point
/// at a credential the operator has not yet added.
pub async fn credential_exists(
    pool: &PgPool,
    workspace_id: Uuid,
    name: &str,
) -> Result<bool, AppError> {
    let row: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM credentials \
         WHERE workspace_id = $1 AND name = $2 AND revoked_at IS NULL",
    )
    .bind(workspace_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::Internal(format!("credential_exists: {e}")))?;
    Ok(row.0 > 0)
}

pub async fn create(
    pool: &PgPool,
    workspace_id: Uuid,
    name: &str,
    base_url: &str,
    credential_name: &str,
) -> Result<ProviderRow, AppError> {
    validate_name(name)?;
    validate_name(credential_name)?;
    validate_base_url(base_url)?;

    if !credential_exists(pool, workspace_id, credential_name).await? {
        return Err(AppError::BadRequest(format!(
            "credential '{credential_name}' not found in this workspace — \
             run `pcy credential add {credential_name}` first"
        )));
    }

    // If this is the workspace's first provider, mark it default.
    let count: (i64,) =
        sqlx::query_as("SELECT count(*) FROM llm_providers WHERE workspace_id = $1")
            .bind(workspace_id)
            .fetch_one(pool)
            .await
            .map_err(|e| AppError::Internal(format!("provider count: {e}")))?;
    let make_default = count.0 == 0;

    let row: ProviderRow = sqlx::query_as(
        "INSERT INTO llm_providers (workspace_id, name, base_url, credential_name, is_default) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING name, base_url, credential_name, is_default, created_at",
    )
    .bind(workspace_id)
    .bind(name)
    .bind(base_url)
    .bind(credential_name)
    .bind(make_default)
    .fetch_one(pool)
    .await
    .map_err(|e| match e.as_database_error() {
        Some(db) if db.is_unique_violation() => AppError::BadRequest(format!(
            "provider '{name}' already exists in this workspace"
        )),
        _ => AppError::Internal(format!("provider insert: {e}")),
    })?;
    Ok(row)
}

pub async fn list(pool: &PgPool, workspace_id: Uuid) -> Result<Vec<ProviderRow>, AppError> {
    sqlx::query_as(
        "SELECT name, base_url, credential_name, is_default, created_at \
         FROM llm_providers WHERE workspace_id = $1 ORDER BY name ASC",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Internal(format!("provider list: {e}")))
}

/// Set `name` as the default provider and clear `is_default` on every
/// other row in the same workspace. Atomic via transaction.
pub async fn set_default(pool: &PgPool, workspace_id: Uuid, name: &str) -> Result<(), AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(format!("begin set_default: {e}")))?;
    sqlx::query("UPDATE llm_providers SET is_default = FALSE WHERE workspace_id = $1")
        .bind(workspace_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("clear default: {e}")))?;
    let res = sqlx::query(
        "UPDATE llm_providers SET is_default = TRUE WHERE workspace_id = $1 AND name = $2",
    )
    .bind(workspace_id)
    .bind(name)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::Internal(format!("set default: {e}")))?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("provider '{name}' not found")));
    }
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(format!("commit set_default: {e}")))?;
    Ok(())
}

/// Delete the named provider. Refuses if it is currently the default
/// AND there is no other provider that could take over (the operator
/// must `pcy provider use <other>` first).
pub async fn delete(pool: &PgPool, workspace_id: Uuid, name: &str) -> Result<(), AppError> {
    let row: Option<(bool,)> = sqlx::query_as(
        "SELECT is_default FROM llm_providers WHERE workspace_id = $1 AND name = $2",
    )
    .bind(workspace_id)
    .bind(name)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Internal(format!("provider lookup: {e}")))?;
    let Some((is_default,)) = row else {
        return Err(AppError::NotFound(format!("provider '{name}' not found")));
    };
    if is_default {
        // Count siblings.
        let sib: (i64,) = sqlx::query_as(
            "SELECT count(*) FROM llm_providers WHERE workspace_id = $1 AND name <> $2",
        )
        .bind(workspace_id)
        .bind(name)
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::Internal(format!("provider siblings: {e}")))?;
        if sib.0 > 0 {
            return Err(AppError::BadRequest(
                "refuse to remove the default provider while others exist — \
                 run `pcy provider use <other>` first"
                    .into(),
            ));
        }
    }
    let res = sqlx::query("DELETE FROM llm_providers WHERE workspace_id = $1 AND name = $2")
        .bind(workspace_id)
        .bind(name)
        .execute(pool)
        .await
        .map_err(|e| AppError::Internal(format!("provider delete: {e}")))?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("provider '{name}' not found")));
    }
    Ok(())
}

/// Resolve the workspace's default provider for use by the wake loop.
/// Returns `Ok(None)` when no provider row exists — callers should
/// fall back to the env-var path and emit `llm_provider_env_fallback`.
pub async fn resolve_default(
    pool: &PgPool,
    workspace_id: Uuid,
) -> Result<Option<(String, String)>, AppError> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT base_url, credential_name FROM llm_providers \
         WHERE workspace_id = $1 AND is_default = TRUE LIMIT 1",
    )
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Internal(format!("resolve_default: {e}")))?;
    Ok(row)
}
