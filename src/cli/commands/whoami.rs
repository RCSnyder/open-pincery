//! AC-48 (v8): `pcy whoami` — print current identity + context.
//!
//! Hits `GET /api/me` with the active context's session token and
//! prints `{context, user_id, workspace_id, url}` as one line of
//! JSON. Exit 0 iff the token authenticates; any HTTP error (401,
//! unreachable, etc.) surfaces as a non-zero exit via the standard
//! `AppError` path.
//!
//! This is the primary "am I logged in?" check for scripts and
//! agentic harnesses — everything it needs to decide whether to
//! proceed or re-login is on stdout.

use crate::api_client::ApiClient;
use crate::cli::config::CliConfig;
use crate::error::AppError;

pub async fn run(client: &ApiClient, cfg: &CliConfig) -> Result<(), AppError> {
    let resp = client.me().await?;
    let context = cfg
        .current_context
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let mut out = serde_json::json!({
        "context": context,
        "url": client.base_url.clone(),
    });
    if let Some(uid) = resp.get("user_id").and_then(|v| v.as_str()) {
        out["user_id"] = serde_json::Value::String(uid.to_string());
    }
    if let Some(ws) = resp.get("workspace_id").and_then(|v| v.as_str()) {
        out["workspace_id"] = serde_json::Value::String(ws.to_string());
    }
    println!("{out}");
    Ok(())
}
