//! AC-45 (v8): `pcy login` — idempotent authentication.
//!
//! Operators (and agents scripting against the API) should never have
//! to care whether a Pincery server has been bootstrapped yet. With a
//! bootstrap token in hand, `pcy login --bootstrap-token ...` always
//! succeeds on a reachable server: it attempts `POST /api/bootstrap`
//! first, and on the `409 Conflict` that a previously-bootstrapped
//! server returns, transparently falls back to `POST /api/login`.
//!
//! Both paths persist the returned `session_token` + `workspace_id`
//! into the CLI config and print one JSON line to stdout so the
//! output is parseable regardless of which code path fired.

use crate::api_client::ApiClient;
use crate::cli::config::{load, save};
use crate::error::AppError;

pub fn run(url: String, token: String) -> Result<(), AppError> {
    let mut cfg = load()?;
    cfg.url = Some(url);
    cfg.token = Some(token);
    save(&cfg)?;
    println!("{}", serde_json::json!({"status": "logged_in"}));
    Ok(())
}

/// Try bootstrap first; on `409 Conflict` fall back to `/api/login`.
/// Any other error (network, 401, 500) surfaces unchanged.
pub async fn run_with_bootstrap(
    client: &ApiClient,
    bootstrap_token: String,
) -> Result<(), AppError> {
    let (resp, already_bootstrapped) = match client.bootstrap(&bootstrap_token).await {
        Ok(v) => (v, false),
        Err(e) if is_already_bootstrapped(&e) => {
            // Fall back to login — the server is healthy, it's just
            // already initialized. This is the happy path for every
            // invocation after the first.
            let v = client.login(&bootstrap_token).await?;
            (v, true)
        }
        Err(e) => return Err(e),
    };

    let token = resp["session_token"]
        .as_str()
        .ok_or_else(|| AppError::Internal("login response missing session_token".into()))?
        .to_string();

    let mut cfg = load()?;
    cfg.url = Some(client.base_url.clone());
    cfg.token = Some(token.clone());
    if let Some(ws) = resp["workspace_id"].as_str() {
        cfg.workspace_id = Some(ws.to_string());
    }
    save(&cfg)?;

    let mut out = serde_json::json!({
        "session_token": token,
        "already_bootstrapped": already_bootstrapped,
    });
    if let Some(ws) = resp["workspace_id"].as_str() {
        out["workspace_id"] = serde_json::Value::String(ws.to_string());
    }
    println!("{out}");
    Ok(())
}

/// `ApiClient::send_json` flattens every non-success response into
/// `AppError::BadRequest("HTTP <code>: <body>")`. Detecting the
/// bootstrap-already-done case therefore means string-inspecting the
/// error. This is ugly but bounded — the only caller is idempotent
/// login, and the server's 409 message is stable (see `src/api/
/// bootstrap.rs`).
fn is_already_bootstrapped(err: &AppError) -> bool {
    match err {
        AppError::BadRequest(msg) => msg.contains("HTTP 409"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_409_conflict_string() {
        let e =
            AppError::BadRequest("HTTP 409: {\"error\":\"System already bootstrapped.\"}".into());
        assert!(is_already_bootstrapped(&e));
    }

    #[test]
    fn does_not_detect_other_errors() {
        assert!(!is_already_bootstrapped(&AppError::BadRequest(
            "HTTP 401: unauthorized".into()
        )));
        assert!(!is_already_bootstrapped(&AppError::Unauthorized(
            "bad token".into()
        )));
        assert!(!is_already_bootstrapped(&AppError::Internal(
            "something exploded".into()
        )));
    }
}
