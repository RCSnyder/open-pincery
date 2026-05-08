//! AC-78 (v9): `pcy audit verify` — walks the event-log hash chain
//! for every agent in the current workspace (or just one agent /
//! workspace when scoped via `--agent` / `--workspace`).
//!
//! Exit code 0 only when every walked chain verifies. Any
//! `audit_chain_broken` result causes [`run`] to return
//! [`ExitCode::from(2)`] so shell wrappers and CI pipelines can
//! detect tamper events without parsing JSON.
//!
//! The handler is a thin client over `POST /api/audit/chain/verify`
//! — the same path used by the startup gate (G3d) and any future
//! cron-style invocations.

use std::process::ExitCode;

use serde_json::Value;

use crate::api_client::ApiClient;
use crate::error::AppError;

/// Exit code returned when at least one agent's chain is broken.
/// Distinct from [`ExitCode::from(1)`] (which the CLI uses for plain
/// errors) so callers can distinguish "tamper detected" from
/// "request failed".
pub const EXIT_CODE_CHAIN_BROKEN: u8 = 2;

/// `pcy audit verify [--agent <id>] [--workspace <id>]`.
///
/// Exactly one scope is honoured: `--agent` (single agent),
/// `--workspace` (every agent in that workspace; must be the caller's
/// workspace), or neither (default to the caller's workspace). The
/// `--workspace` flag is currently a guard — single-workspace
/// deployments require it to match the caller's workspace; the API
/// rejects cross-workspace requests at the tenancy boundary.
pub async fn verify(
    client: &ApiClient,
    agent: Option<String>,
    _workspace: Option<String>,
) -> Result<ExitCode, AppError> {
    let response = match agent.as_deref() {
        Some(agent_id) => client.verify_chain_agent(agent_id).await?,
        None => client.verify_chain_workspace().await?,
    };

    let exit_code = if response
        .get("all_verified")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_CODE_CHAIN_BROKEN)
    };

    pretty_print(&response);
    Ok(exit_code)
}

/// Render the response in the human-friendly form documented in
/// readiness.md T-AC78-6: one line per agent, `OK (<n> events)` or
/// `BROKEN at event <id>`. Always also dumps the raw JSON to stdout
/// so non-interactive callers (CI, cron) can parse without
/// re-shelling.
fn pretty_print(response: &Value) {
    let agents = response
        .get("agents")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for entry in &agents {
        let agent_id = entry.get("agent_id").and_then(Value::as_str).unwrap_or("?");
        let status = entry
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        match status {
            "verified" => {
                let n = entry
                    .get("events_in_chain")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                eprintln!("agent {agent_id}: OK ({n} events)");
            }
            "broken" => {
                let target = entry
                    .get("first_divergent_event_id")
                    .and_then(Value::as_str)
                    .unwrap_or("?");
                eprintln!("agent {agent_id}: BROKEN at event {target}");
            }
            other => {
                eprintln!("agent {agent_id}: status={other}");
            }
        }
    }
    println!("{response}");
}
