//! AC-78 (v9): audit-chain verification HTTP surface.
//!
//! Endpoints (mounted under the authenticated `/api` subtree):
//!
//! - `POST /api/audit/chain/verify` — verify all agents in the
//!   caller's workspace (admin-only).
//! - `POST /api/audit/chain/verify/agents/{id}` — verify a single
//!   agent (admin-only; agent must be in the caller's workspace).
//!
//! Both handlers:
//!   1. Gate on [`credential::is_workspace_admin`] for the caller's
//!      workspace; non-admin -> 403.
//!   2. Run the same `audit_chain::verify_workspace` /
//!      `audit_chain::verify_and_emit` path used by the `pcy audit
//!      verify` CLI and by the startup gate.
//!   3. Return a JSON payload with one entry per agent describing
//!      `verified` or `broken`. The wire shape mirrors what the CLI
//!      pretty-prints so the two surfaces stay byte-for-byte
//!      compatible.
//!
//! Cross-workspace access is forbidden by tenancy: the path-less
//! workspace endpoint always uses `auth.workspace_id`; the per-agent
//! endpoint 404s if the agent belongs to another workspace.

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use super::{scoped_agent, AppState, AuthUser};
use crate::background::audit_chain::{self, ChainStatus};
use crate::error::AppError;
use crate::models::credential;

/// Wire shape for one agent's chain status.
#[derive(Serialize, ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AgentChainStatusResponse {
    Verified {
        agent_id: Uuid,
        events_in_chain: u64,
        last_entry_hash: String,
    },
    Broken {
        agent_id: Uuid,
        first_divergent_event_id: Uuid,
        expected_hash: String,
        actual_hash: String,
        events_walked: u64,
    },
}

impl AgentChainStatusResponse {
    fn from_pair(agent_id: Uuid, status: ChainStatus) -> Self {
        match status {
            ChainStatus::Verified {
                events_in_chain,
                last_entry_hash,
            } => Self::Verified {
                agent_id,
                events_in_chain,
                last_entry_hash,
            },
            ChainStatus::Broken {
                first_divergent_event_id,
                expected_hash,
                actual_hash,
                events_walked,
            } => Self::Broken {
                agent_id,
                first_divergent_event_id,
                expected_hash,
                actual_hash,
                events_walked,
            },
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct AuditChainVerifyResponse {
    pub agents: Vec<AgentChainStatusResponse>,
    /// `true` iff every agent in the response is `verified`. The CLI
    /// uses this to choose a process exit code without re-walking the
    /// list.
    pub all_verified: bool,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/audit/chain/verify", post(verify_workspace_handler))
        .route(
            "/audit/chain/verify/agents/{id}",
            post(verify_agent_handler),
        )
}

async fn require_admin(state: &AppState, auth: &AuthUser) -> Result<(), AppError> {
    if !credential::is_workspace_admin(&state.pool, auth.workspace_id, auth.user_id).await? {
        return Err(AppError::Forbidden("workspace admin role required".into()));
    }
    Ok(())
}

/// Verify every agent in the caller's workspace. Admin-only.
#[utoipa::path(
    post,
    path = "/api/audit/chain/verify",
    tag = "audit",
    responses(
        (status = 200, description = "Chain status per agent", body = AuditChainVerifyResponse),
        (status = 403, description = "Caller is not a workspace admin"),
    ),
    security(("bearerAuth" = [])),
)]
pub async fn verify_workspace_handler(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> Result<(StatusCode, Json<AuditChainVerifyResponse>), AppError> {
    require_admin(&state, &auth).await?;

    let results = audit_chain::verify_workspace(&state.pool, auth.workspace_id).await?;
    let mut all_verified = true;
    let agents = results
        .into_iter()
        .map(|r| {
            if matches!(r.status, ChainStatus::Broken { .. }) {
                all_verified = false;
            }
            // verify_workspace already calls verify_and_emit per agent,
            // so the audit_chain_verified / _broken events are written
            // before this map runs. We only project the result onto
            // the wire shape here.
            AgentChainStatusResponse::from_pair(r.agent_id, r.status)
        })
        .collect::<Vec<_>>();

    Ok((
        StatusCode::OK,
        Json(AuditChainVerifyResponse {
            agents,
            all_verified,
        }),
    ))
}

/// Verify one agent in the caller's workspace. Admin-only.
#[utoipa::path(
    post,
    path = "/api/audit/chain/verify/agents/{id}",
    tag = "audit",
    params(("id" = Uuid, Path, description = "Agent ID")),
    responses(
        (status = 200, description = "Chain status for this agent", body = AuditChainVerifyResponse),
        (status = 403, description = "Caller is not a workspace admin"),
        (status = 404, description = "Agent not found in caller's workspace"),
    ),
    security(("bearerAuth" = [])),
)]
pub async fn verify_agent_handler(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(agent_id): Path<Uuid>,
) -> Result<(StatusCode, Json<AuditChainVerifyResponse>), AppError> {
    require_admin(&state, &auth).await?;
    let agent = scoped_agent(&state, &auth, agent_id).await?;

    let status = audit_chain::verify_and_emit(&state.pool, agent.id).await?;
    let all_verified = matches!(status, ChainStatus::Verified { .. });
    let agents = vec![AgentChainStatusResponse::from_pair(agent.id, status)];

    Ok((
        StatusCode::OK,
        Json(AuditChainVerifyResponse {
            agents,
            all_verified,
        }),
    ))
}
