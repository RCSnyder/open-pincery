//! AC-39 (v7): `credentials` table access + workspace-admin helper.
//!
//! Sealed credentials are stored as `(ciphertext, nonce)` BYTEA pairs. The
//! `Credential` type holds them and is NEVER serialized to JSON — all
//! wire-facing responses go through [`CredentialSummary`] which omits
//! both. The vault module ([`crate::runtime::vault::Vault`]) is the only
//! code path that sees plaintext.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

/// Internal row — includes ciphertext and nonce. Not `Serialize`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Credential {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Wire-facing projection — names + metadata only. No value, no
/// ciphertext, no nonce. `#[derive(Serialize)]` is intentionally
/// restricted to this type.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CredentialSummary {
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub created_by: Uuid,
}

/// AC-39 name validation. Rejects anything that is not `^[a-z0-9_]{1,64}$`.
/// Hand-rolled to avoid pulling in the `regex` crate for one check; the
/// same pattern is also enforced by a DB CHECK constraint so this is a
/// fast path with defense-in-depth.
pub fn validate_name(name: &str) -> Result<(), AppError> {
    let len = name.len();
    if !(1..=64).contains(&len) {
        return Err(AppError::BadRequest(
            "credential name must be 1..=64 bytes".into(),
        ));
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(AppError::BadRequest(
            "credential name must match [a-z0-9_]+".into(),
        ));
    }
    Ok(())
}

/// AC-39 value length validation. Values are opaque bytes; only the size
/// matters here (content is encrypted).
pub fn validate_value_bytes(value: &[u8]) -> Result<(), AppError> {
    let len = value.len();
    if !(1..=8192).contains(&len) {
        return Err(AppError::BadRequest(
            "credential value must be 1..=8192 bytes".into(),
        ));
    }
    Ok(())
}

/// Insert a sealed credential. Returns [`AppError::Conflict`] if an
/// active (non-revoked) credential already exists with the same name
/// in this workspace (enforced by `credentials_one_active_per_name`).
pub async fn create(
    pool: &PgPool,
    workspace_id: Uuid,
    name: &str,
    ciphertext: &[u8],
    nonce: &[u8; 12],
    created_by: Uuid,
) -> Result<Credential, AppError> {
    let result = sqlx::query_as::<_, Credential>(
        "INSERT INTO credentials (workspace_id, name, ciphertext, nonce, created_by)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
    )
    .bind(workspace_id)
    .bind(name)
    .bind(ciphertext)
    .bind(&nonce[..])
    .bind(created_by)
    .fetch_one(pool)
    .await;

    match result {
        Ok(c) => Ok(c),
        Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("23505") => {
            Err(AppError::Conflict(format!(
                "credential '{name}' already exists in this workspace"
            )))
        }
        Err(e) => Err(AppError::Database(e)),
    }
}

/// List active (non-revoked) credentials for a workspace. Summary
/// projection only — ciphertext and nonce never leave the DB row.
pub async fn list_active(
    pool: &PgPool,
    workspace_id: Uuid,
) -> Result<Vec<CredentialSummary>, AppError> {
    let rows = sqlx::query_as::<_, CredentialSummary>(
        "SELECT name, created_at, created_by
         FROM credentials
         WHERE workspace_id = $1 AND revoked_at IS NULL
         ORDER BY name ASC",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Look up an active credential by name. AC-43 call site — returns
/// `None` for both "never existed" and "revoked"; the caller does not
/// distinguish (see design.md Scope Adjustments for rationale).
pub async fn find_active(
    pool: &PgPool,
    workspace_id: Uuid,
    name: &str,
) -> Result<Option<Credential>, AppError> {
    let row = sqlx::query_as::<_, Credential>(
        "SELECT * FROM credentials
         WHERE workspace_id = $1 AND name = $2 AND revoked_at IS NULL",
    )
    .bind(workspace_id)
    .bind(name)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Mark an active credential as revoked. Returns `true` if exactly one
/// row was updated, `false` otherwise (name not found or already
/// revoked — caller maps to 404).
pub async fn revoke(pool: &PgPool, workspace_id: Uuid, name: &str) -> Result<bool, AppError> {
    let res = sqlx::query(
        "UPDATE credentials
         SET revoked_at = NOW()
         WHERE workspace_id = $1 AND name = $2 AND revoked_at IS NULL",
    )
    .bind(workspace_id)
    .bind(name)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() == 1)
}

/// AC-39 role gate. Returns `true` if the user has an active
/// `workspace_owner` or `workspace_admin` membership on the workspace.
/// Unknown users or non-members return `false`; callers map that to
/// 403.
pub async fn is_workspace_admin(
    pool: &PgPool,
    workspace_id: Uuid,
    user_id: Uuid,
) -> Result<bool, AppError> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT role FROM workspace_memberships
         WHERE workspace_id = $1 AND user_id = $2 AND status = 'active'",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(match row {
        Some((role,)) => matches!(
            role.as_str(),
            "owner" | "workspace_owner" | "admin" | "workspace_admin"
        ),
        None => false,
    })
}

/// AC-39 audit. Writes a row to `auth_audit` with `details` JSONB
/// carrying `{name, actor_user_id}` plus the workspace_id column.
/// `event_type` is one of `credential_added`, `credential_revoked`,
/// `credential_forbidden`.
pub async fn append_audit(
    pool: &PgPool,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    event_type: &str,
    credential_name: &str,
) -> Result<(), AppError> {
    let details = serde_json::json!({
        "credential_name": credential_name,
        "actor_user_id": actor_user_id,
    });
    sqlx::query(
        "INSERT INTO auth_audit (user_id, auth_provider, event_type, workspace_id, details)
         VALUES ($1, 'credential_vault', $2, $3, $4)",
    )
    .bind(actor_user_id)
    .bind(event_type)
    .bind(workspace_id)
    .bind(details)
    .execute(pool)
    .await?;
    Ok(())
}
