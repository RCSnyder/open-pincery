//! AC-40 (v7): `pcy credential` subcommands.
//!
//! Security contract:
//!
//! * There is NO `--value` clap argument. Credential values are only
//!   ever accepted interactively via [`rpassword::prompt_password`]
//!   (hidden terminal input) or from stdin when the operator passes
//!   `--stdin`. This keeps values out of shell history and `ps aux`.
//! * Responses from the server are always `CredentialSummary` shapes
//!   (name + created_at + created_by). Ciphertext, nonces, and raw
//!   values never touch the CLI.
//! * `list` prints a terse table; `revoke` requires confirmation
//!   unless `--yes` is passed.

use std::io::{self, Read};

use crate::api_client::ApiClient;
use crate::cli::config::{load, save};
use crate::error::AppError;

/// Resolve the workspace_id the credential commands should target.
///
/// Preference order:
///   1. Cached `workspace_id` in the CLI config (set on bootstrap/login).
///   2. Fresh `GET /api/me` call (and cache the result).
async fn resolve_workspace_id(client: &ApiClient) -> Result<String, AppError> {
    let mut cfg = load()?;
    if let Some(ws) = cfg.workspace_id.as_ref() {
        return Ok(ws.clone());
    }
    let resp = client.me().await?;
    let ws = resp["workspace_id"]
        .as_str()
        .ok_or_else(|| AppError::Internal("/api/me response missing workspace_id".into()))?
        .to_string();
    cfg.workspace_id = Some(ws.clone());
    // Best-effort cache; ignore write errors so a read-only HOME
    // doesn't break the command.
    let _ = save(&cfg);
    Ok(ws)
}

/// Read a credential value from the appropriate source.
///
/// When `use_stdin` is true we read the entire stdin to EOF and trim a
/// single trailing newline (`\n` or `\r\n`) so `printf 'secret' | pcy ...`
/// and `echo secret | pcy ...` both work. Otherwise we invoke
/// [`rpassword::prompt_password`] which suppresses terminal echo.
fn read_value(name: &str, use_stdin: bool) -> Result<String, AppError> {
    if use_stdin {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| AppError::Internal(format!("reading stdin: {e}")))?;
        // Trim exactly one trailing newline (CRLF or LF).
        if buf.ends_with('\n') {
            buf.pop();
            if buf.ends_with('\r') {
                buf.pop();
            }
        }
        Ok(buf)
    } else {
        rpassword::prompt_password(format!("Value for credential '{name}' (hidden): "))
            .map_err(|e| AppError::Internal(format!("reading password: {e}")))
    }
}

pub async fn add(client: &ApiClient, name: String, use_stdin: bool) -> Result<(), AppError> {
    let ws_id = resolve_workspace_id(client).await?;
    let value = read_value(&name, use_stdin)?;
    let resp = client.create_credential(&ws_id, &name, &value).await?;
    // Print the server's projection (name + created_at + created_by only).
    println!("{resp}");
    Ok(())
}

pub async fn list(client: &ApiClient) -> Result<(), AppError> {
    let ws_id = resolve_workspace_id(client).await?;
    let resp = client.list_credentials(&ws_id).await?;
    let arr = resp.as_array().cloned().unwrap_or_default();
    if arr.is_empty() {
        println!("(no active credentials)");
        return Ok(());
    }
    println!("NAME\tCREATED_AT\tCREATED_BY");
    for row in arr {
        let n = row["name"].as_str().unwrap_or("");
        let c = row["created_at"].as_str().unwrap_or("");
        let b = row["created_by"].as_str().unwrap_or("");
        println!("{n}\t{c}\t{b}");
    }
    Ok(())
}

pub async fn revoke(client: &ApiClient, name: String, yes: bool) -> Result<(), AppError> {
    let ws_id = resolve_workspace_id(client).await?;
    if !yes {
        eprintln!(
            "Revoke credential '{name}' in workspace {ws_id}? This cannot be undone. \
             Pass --yes to confirm."
        );
        return Err(AppError::BadRequest(
            "revoke requires --yes confirmation".into(),
        ));
    }
    client.revoke_credential(&ws_id, &name).await?;
    println!("revoked: {name}");
    Ok(())
}
