//! AC-48 (v8): named CLI contexts.
//!
//! On-disk TOML schema (v8):
//!
//! ```toml
//! current-context = "prod"
//!
//! [contexts.default]
//! url = "http://localhost:8080"
//! token = "…"
//! workspace_id = "018f…"
//!
//! [contexts.prod]
//! url = "https://pincery.example.com"
//! token = "…"
//! ```
//!
//! [`CliConfig`] also carries **legacy** top-level `url`/`token`/
//! `workspace_id` fields. They mirror the active context on load and
//! are written back into the active context on save. This keeps every
//! v1–v7 call site (`cfg.url = Some(…)`) working while the noun-verb
//! CLI restructure is split across slices 2c–2e.
//!
//! Migration from the v4 flat shape (top-level `url`/`token` only, no
//! `contexts` section) happens in [`crate::cli::migrate`].

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::AppError;

/// Per-context settings. Exactly one is "active" at a time, selected
/// by [`CliConfig::current_context`] or the `--context` flag.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CliConfig {
    /// Named contexts. Empty in a fresh install; populated by `pcy
    /// context set` (slice 2d) and by v4→v8 auto-migration.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub contexts: BTreeMap<String, ContextConfig>,

    /// Name of the currently active context. Set to `"default"` by
    /// migration; flipped by `pcy context use <name>`.
    #[serde(
        default,
        rename = "current-context",
        skip_serializing_if = "Option::is_none"
    )]
    pub current_context: Option<String>,

    // --- Legacy v1–v7 surface (kept until slice 2d finishes) ---------
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// AC-40 (v7): cached workspace_id from the most recent bootstrap
    /// or `pcy` session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

impl CliConfig {
    /// Project the active context into the legacy top-level fields.
    /// Non-destructive: a field already set (e.g. by a prior call) is
    /// not overwritten, so env/flag precedence above this layer still
    /// wins.
    pub fn hydrate_legacy_from_active(&mut self) {
        let Some(name) = self.current_context.clone() else {
            return;
        };
        let Some(ctx) = self.contexts.get(&name) else {
            return;
        };
        if self.url.is_none() {
            self.url = ctx.url.clone();
        }
        if self.token.is_none() {
            self.token = ctx.token.clone();
        }
        if self.workspace_id.is_none() {
            self.workspace_id = ctx.workspace_id.clone();
        }
    }

    /// Fold legacy top-level fields into the active context's entry,
    /// creating it (and the `"default"` context) if absent. Called
    /// before every save so `cfg.token = Some(t); save(&cfg)` persists
    /// in the v8 shape.
    pub fn sync_active_from_legacy(&mut self) {
        if self.current_context.is_none() {
            if self.url.is_some() || self.token.is_some() || self.workspace_id.is_some() {
                self.current_context = Some("default".to_string());
            } else {
                return;
            }
        }
        let name = self
            .current_context
            .clone()
            .expect("current_context set above");
        let entry = self.contexts.entry(name).or_default();
        if let Some(u) = &self.url {
            entry.url = Some(u.clone());
        }
        if let Some(t) = &self.token {
            entry.token = Some(t.clone());
        }
        if let Some(w) = &self.workspace_id {
            entry.workspace_id = Some(w.clone());
        }
    }

    /// True when the on-disk shape is the v4 flat form: legacy fields
    /// present, no `contexts` section and no `current-context`.
    pub fn is_v4_flat(&self) -> bool {
        self.contexts.is_empty()
            && self.current_context.is_none()
            && (self.url.is_some() || self.token.is_some() || self.workspace_id.is_some())
    }
}

/// Resolve the config file path. Precedence:
/// 1. `PCY_CONFIG_PATH` env var (tests + power users)
/// 2. `dirs::config_dir()/open-pincery/config.toml`
pub fn config_path() -> Result<PathBuf, AppError> {
    if let Ok(explicit) = std::env::var("PCY_CONFIG_PATH") {
        return Ok(PathBuf::from(explicit));
    }
    let base = dirs::config_dir()
        .ok_or_else(|| AppError::Internal("unable to resolve config dir".into()))?;
    Ok(base.join("open-pincery").join("config.toml"))
}

/// Load the CLI config from the default path, applying v4→v8
/// migration as a side effect if needed, then hydrating legacy
/// fields from the active context.
pub fn load() -> Result<CliConfig, AppError> {
    let path = config_path()?;
    load_from_path(&path)
}

/// Load from an explicit path. Exposed for tests.
pub fn load_from_path(path: &Path) -> Result<CliConfig, AppError> {
    let mut cfg = if path.exists() {
        let text = std::fs::read_to_string(path)
            .map_err(|e| AppError::Internal(format!("failed reading config: {e}")))?;
        toml::from_str::<CliConfig>(&text)
            .map_err(|e| AppError::Internal(format!("invalid config.toml: {e}")))?
    } else {
        CliConfig::default()
    };

    if cfg.is_v4_flat() {
        crate::cli::migrate::migrate_v4_to_v8(&mut cfg, path)?;
    }

    cfg.hydrate_legacy_from_active();
    Ok(cfg)
}

/// Save to the default path via atomic tempfile+rename write.
pub fn save(cfg: &CliConfig) -> Result<(), AppError> {
    let path = config_path()?;
    save_to_path(cfg, &path)
}

/// Save to an explicit path. The config is cloned so legacy-field
/// syncing does not mutate the caller's view.
pub fn save_to_path(cfg: &CliConfig, path: &Path) -> Result<(), AppError> {
    let mut cfg = cfg.clone();
    cfg.sync_active_from_legacy();

    let text = toml::to_string(&cfg)
        .map_err(|e| AppError::Internal(format!("failed serializing config: {e}")))?;
    atomic_write(path, text.as_bytes())
}

/// Tempfile + rename. Rename is atomic on both POSIX and Windows when
/// source and destination share a filesystem.
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Internal(format!("config path {path:?} has no parent")))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| AppError::Internal(format!("failed creating config dir: {e}")))?;
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("config.toml");
    let tmp = parent.join(format!(".{file_name}.tmp"));
    std::fs::write(&tmp, bytes)
        .map_err(|e| AppError::Internal(format!("failed writing temp config: {e}")))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| AppError::Internal(format!("failed renaming config into place: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_v4_flat_shape() {
        let cfg = CliConfig {
            url: Some("http://x".into()),
            token: Some("t".into()),
            ..Default::default()
        };
        assert!(cfg.is_v4_flat());
    }

    #[test]
    fn v8_shape_is_not_v4_flat() {
        let mut cfg = CliConfig::default();
        cfg.contexts
            .insert("default".into(), ContextConfig::default());
        cfg.current_context = Some("default".into());
        assert!(!cfg.is_v4_flat());
    }

    #[test]
    fn empty_shape_is_not_v4_flat() {
        let cfg = CliConfig::default();
        assert!(!cfg.is_v4_flat());
    }

    #[test]
    fn hydrate_copies_active_context_into_legacy() {
        let mut cfg = CliConfig::default();
        cfg.contexts.insert(
            "prod".into(),
            ContextConfig {
                url: Some("https://prod".into()),
                token: Some("t".into()),
                workspace_id: Some("w".into()),
            },
        );
        cfg.current_context = Some("prod".into());
        cfg.hydrate_legacy_from_active();
        assert_eq!(cfg.url.as_deref(), Some("https://prod"));
        assert_eq!(cfg.token.as_deref(), Some("t"));
        assert_eq!(cfg.workspace_id.as_deref(), Some("w"));
    }

    #[test]
    fn hydrate_does_not_overwrite_existing_legacy_values() {
        let mut cfg = CliConfig {
            url: Some("https://override".into()),
            ..Default::default()
        };
        cfg.contexts.insert(
            "prod".into(),
            ContextConfig {
                url: Some("https://prod".into()),
                ..Default::default()
            },
        );
        cfg.current_context = Some("prod".into());
        cfg.hydrate_legacy_from_active();
        assert_eq!(cfg.url.as_deref(), Some("https://override"));
    }

    #[test]
    fn sync_writes_legacy_back_into_active_context() {
        let mut cfg = CliConfig {
            url: Some("https://new".into()),
            token: Some("t2".into()),
            ..Default::default()
        };
        cfg.contexts.insert("prod".into(), ContextConfig::default());
        cfg.current_context = Some("prod".into());
        cfg.sync_active_from_legacy();
        let prod = cfg.contexts.get("prod").unwrap();
        assert_eq!(prod.url.as_deref(), Some("https://new"));
        assert_eq!(prod.token.as_deref(), Some("t2"));
    }

    #[test]
    fn sync_creates_default_context_when_none_active() {
        let mut cfg = CliConfig {
            url: Some("http://x".into()),
            token: Some("t".into()),
            ..Default::default()
        };
        cfg.sync_active_from_legacy();
        assert_eq!(cfg.current_context.as_deref(), Some("default"));
        assert_eq!(
            cfg.contexts.get("default").and_then(|c| c.url.as_deref()),
            Some("http://x")
        );
    }

    #[test]
    fn round_trip_write_then_read_preserves_v8_shape() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");

        let mut cfg = CliConfig::default();
        cfg.contexts.insert(
            "prod".into(),
            ContextConfig {
                url: Some("https://prod".into()),
                token: Some("t".into()),
                workspace_id: None,
            },
        );
        cfg.contexts.insert(
            "staging".into(),
            ContextConfig {
                url: Some("https://staging".into()),
                token: Some("t2".into()),
                workspace_id: None,
            },
        );
        cfg.current_context = Some("prod".into());

        save_to_path(&cfg, &path).unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("current-context = \"prod\""), "{text}");
        assert!(text.contains("[contexts.prod]"), "{text}");
        assert!(text.contains("[contexts.staging]"), "{text}");

        let reloaded = load_from_path(&path).unwrap();
        assert_eq!(reloaded.current_context.as_deref(), Some("prod"));
        assert_eq!(reloaded.contexts.len(), 2);
        assert_eq!(reloaded.url.as_deref(), Some("https://prod"));
    }

    #[test]
    fn atomic_write_does_not_leave_tempfile_behind() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        atomic_write(&path, b"hello").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
        let tmp_sibling = tmp.path().join(".config.toml.tmp");
        assert!(!tmp_sibling.exists());
    }
}
