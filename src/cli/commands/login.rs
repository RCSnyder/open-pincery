use crate::api_client::ApiClient;
use crate::cli::config::{load, save};
use crate::error::AppError;

pub fn run(url: String, token: String) -> Result<(), AppError> {
    let mut cfg = load()?;
    cfg.url = Some(url);
    cfg.token = Some(token);
    save(&cfg)?;
    println!("logged in");
    Ok(())
}

pub async fn run_with_bootstrap(
    client: &ApiClient,
    bootstrap_token: String,
) -> Result<(), AppError> {
    let resp = client.login(&bootstrap_token).await?;
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

    println!("{}", serde_json::json!({"session_token": token}));
    Ok(())
}
