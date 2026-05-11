//! AC-93 (v9.1): `pcy provider` subcommands.
//!
//! LLM provider management — a workspace can register one or more
//! provider rows pointing at OpenAI-compatible base URLs paired with
//! a stored credential. Exactly one provider per workspace may be
//! marked default; the wake loop reads `resolve_default` to pick the
//! base_url + credential at request time, falling back to env vars
//! (emitting `llm_provider_env_fallback`) when no default exists.

use serde::{Deserialize, Serialize};

use crate::api_client::ApiClient;
use crate::cli::config::{load, save};
use crate::cli::output::{self, OutputFormat, TableRow};
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRow {
    pub name: String,
    pub base_url: String,
    pub credential_name: String,
    pub is_default: bool,
    pub created_at: String,
}

impl TableRow for ProviderRow {
    fn headers() -> &'static [&'static str] {
        &["NAME", "BASE_URL", "CREDENTIAL", "DEFAULT", "CREATED_AT"]
    }
    fn row(&self) -> Vec<String> {
        vec![
            self.name.clone(),
            self.base_url.clone(),
            self.credential_name.clone(),
            if self.is_default {
                "yes".into()
            } else {
                "no".into()
            },
            self.created_at.clone(),
        ]
    }
}

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
    let _ = save(&cfg);
    Ok(ws)
}

pub async fn add(
    client: &ApiClient,
    name: String,
    base_url: String,
    credential_name: String,
) -> Result<(), AppError> {
    let ws_id = resolve_workspace_id(client).await?;
    let resp = client
        .create_provider(&ws_id, &name, &base_url, &credential_name)
        .await?;
    println!("{resp}");
    Ok(())
}

pub async fn list(client: &ApiClient, fmt: &OutputFormat) -> Result<(), AppError> {
    let ws_id = resolve_workspace_id(client).await?;
    let resp = client.list_providers(&ws_id).await?;
    let arr = resp.as_array().cloned().unwrap_or_default();
    let rows: Vec<ProviderRow> = arr
        .into_iter()
        .map(|row| ProviderRow {
            name: row["name"].as_str().unwrap_or("").to_string(),
            base_url: row["base_url"].as_str().unwrap_or("").to_string(),
            credential_name: row["credential_name"].as_str().unwrap_or("").to_string(),
            is_default: row["is_default"].as_bool().unwrap_or(false),
            created_at: row["created_at"].as_str().unwrap_or("").to_string(),
        })
        .collect();
    let rendered = output::render(&rows, fmt)?;
    print!("{rendered}");
    Ok(())
}

pub async fn use_default(client: &ApiClient, name: String) -> Result<(), AppError> {
    let ws_id = resolve_workspace_id(client).await?;
    client.set_default_provider(&ws_id, &name).await?;
    println!("{}", serde_json::json!({ "default": name }));
    Ok(())
}

pub async fn remove(client: &ApiClient, name: String, yes: bool) -> Result<(), AppError> {
    let ws_id = resolve_workspace_id(client).await?;
    if !yes {
        eprintln!("Remove provider '{name}' in workspace {ws_id}? Pass --yes to confirm.");
        return Err(AppError::BadRequest(
            "remove requires --yes confirmation".into(),
        ));
    }
    client.delete_provider(&ws_id, &name).await?;
    println!("{}", serde_json::json!({ "removed": name }));
    Ok(())
}
