use crate::api_client::ApiClient;
use crate::cli::config::{load, save};
use crate::error::AppError;

pub async fn run(client: &ApiClient, bootstrap_token: String) -> Result<(), AppError> {
    let resp = client.bootstrap(&bootstrap_token).await?;
    let token = resp["session_token"]
        .as_str()
        .ok_or_else(|| AppError::Internal("bootstrap response missing session_token".into()))?
        .to_string();

    let workspace_id = resp["workspace_id"].as_str().map(|s| s.to_string());

    let mut cfg = load()?;
    cfg.url = Some(client.base_url.clone());
    cfg.token = Some(token.clone());
    if workspace_id.is_some() {
        cfg.workspace_id = workspace_id.clone();
    }
    save(&cfg)?;

    let mut out = serde_json::json!({"session_token": token});
    if let Some(ws) = workspace_id {
        out["workspace_id"] = serde_json::Value::String(ws);
    }
    println!("{out}");
    Ok(())
}
