use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::AppError;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CliConfig {
    pub url: Option<String>,
    pub token: Option<String>,
}

fn config_path() -> Result<PathBuf, AppError> {
    if let Ok(explicit) = std::env::var("PCY_CONFIG_PATH") {
        return Ok(PathBuf::from(explicit));
    }
    let base = dirs::config_dir()
        .ok_or_else(|| AppError::Internal("unable to resolve config dir".into()))?;
    Ok(base.join("open-pincery").join("config.toml"))
}

pub fn load() -> Result<CliConfig, AppError> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(CliConfig::default());
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| AppError::Internal(format!("failed reading config: {e}")))?;
    toml::from_str(&text).map_err(|e| AppError::Internal(format!("invalid config.toml: {e}")))
}

pub fn save(cfg: &CliConfig) -> Result<(), AppError> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Internal(format!("failed creating config dir: {e}")))?;
    }
    let text = toml::to_string(cfg)
        .map_err(|e| AppError::Internal(format!("failed serializing config: {e}")))?;
    std::fs::write(&path, text)
        .map_err(|e| AppError::Internal(format!("failed writing config: {e}")))?;
    Ok(())
}
