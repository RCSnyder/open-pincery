//! AC-48 (v8) — `pcy context` noun.
//!
//! Pure on-disk operations against the config store. No HTTP. This is
//! the first noun in the v8 tree because it exercises [`crate::cli::config`]
//! and [`crate::cli::migrate`] directly, giving slice 2d a minimal
//! end-to-end proof before the HTTP-bound nouns (credential/agent/...)
//! land.
//!
//! Verbs:
//! - `list` — one row per named context; `*` marks the active one.
//! - `current` — print the active context name (empty → no-op + exit 0).
//! - `use <name>` — switch the active context. Errors if name missing.
//! - `set <name> --url / --token / --workspace` — upsert fields on a
//!   named context; creates it if absent. `--token` is intentionally
//!   optional and read from stdin when passed without a value would
//!   leak into history; stdin reading is deferred to slice 2e once
//!   the root `Cli` has the shared flag surface.
//! - `delete <name>` — remove a context. Refuses to delete the active
//!   one (use `use <other>` first).
//!
//! Every verb takes a `&Path` config file rather than calling
//! `config::config_path()` itself so tests can exercise hermetic
//! tempdirs without touching the process-global `PCY_CONFIG_PATH`
//! env var. The clap dispatcher resolves the path once and threads
//! it through.

use std::path::Path;

use clap::Subcommand;
use serde::{Deserialize, Serialize};

use crate::cli::config::{self, CliConfig, ContextConfig};
use crate::cli::output::{self, OutputFormat, TableRow};
use crate::error::AppError;

/// Verbs under `pcy context`. Per readiness.md AC-48: list, current,
/// use, set, delete. No `show` verb — `list --output json | jq '.<n>'`
/// and `list --output jsonpath='{.items[?(@.name=="x")]}'` cover it.
#[derive(Subcommand, Debug)]
pub enum ContextCommands {
    /// List every named context. `*` marks the active one.
    List,
    /// Print the active context name.
    Current,
    /// Switch the active context.
    Use {
        /// Context name (must already exist).
        name: String,
    },
    /// Create or update a named context.
    Set {
        /// Context name (created if absent).
        name: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        /// Workspace id to pin for this context.
        #[arg(long = "workspace")]
        workspace_id: Option<String>,
    },
    /// Remove a named context.
    Delete { name: String },
}

/// One row of `pcy context list`, suitable for JSON / YAML / table
/// rendering. `active` is true for exactly one row (the one selected
/// by `current-context`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRow {
    pub name: String,
    pub url: Option<String>,
    pub workspace_id: Option<String>,
    pub active: bool,
}

impl TableRow for ContextRow {
    fn headers() -> &'static [&'static str] {
        &["ACTIVE", "NAME", "URL", "WORKSPACE"]
    }

    fn row(&self) -> Vec<String> {
        vec![
            if self.active { "*".into() } else { "".into() },
            self.name.clone(),
            self.url.clone().unwrap_or_default(),
            self.workspace_id.clone().unwrap_or_default(),
        ]
    }
}

/// Dispatch a parsed [`ContextCommands`] against the config at `path`,
/// rendering output per `fmt`. Stdout bytes are returned as a `String`
/// so the caller (and tests) can assert on them without a subprocess.
pub fn run(cmd: ContextCommands, path: &Path, fmt: &OutputFormat) -> Result<String, AppError> {
    match cmd {
        ContextCommands::List => list(path, fmt),
        ContextCommands::Current => current(path),
        ContextCommands::Use { name } => use_(path, &name),
        ContextCommands::Set {
            name,
            url,
            token,
            workspace_id,
        } => set(path, &name, url, token, workspace_id),
        ContextCommands::Delete { name } => delete(path, &name),
    }
}

/// List all contexts, marking the active one. Sorted by name because
/// the underlying store is a `BTreeMap` — stable output for tests.
pub fn list(path: &Path, fmt: &OutputFormat) -> Result<String, AppError> {
    let cfg = config::load_from_path(path)?;
    let active = cfg.current_context.as_deref();
    let rows: Vec<ContextRow> = cfg
        .contexts
        .iter()
        .map(|(name, ctx)| ContextRow {
            name: name.clone(),
            url: ctx.url.clone(),
            workspace_id: ctx.workspace_id.clone(),
            active: Some(name.as_str()) == active,
        })
        .collect();
    output::render(&rows, fmt)
}

/// Print the active context name. Empty string + success when no
/// context is set — matches `kubectl config current-context` UX
/// where scripts can test with `[ -n "$(pcy context current)" ]`.
pub fn current(path: &Path) -> Result<String, AppError> {
    let cfg = config::load_from_path(path)?;
    Ok(cfg.current_context.unwrap_or_default())
}

/// Switch the active context. Returns `NotFound` if the named context
/// does not exist — exit code 1 via `AppError::NotFound` mapping.
pub fn use_(path: &Path, name: &str) -> Result<String, AppError> {
    let mut cfg = config::load_from_path(path)?;
    if !cfg.contexts.contains_key(name) {
        return Err(AppError::NotFound(format!("context '{name}' not found")));
    }
    cfg.current_context = Some(name.to_string());
    // Drop hydrated legacy fields so sync-on-save writes the
    // newly-active context's values back into the top-level mirror
    // on the next load rather than leaking the previous context's.
    cfg.url = None;
    cfg.token = None;
    cfg.workspace_id = None;
    config::save_to_path(&cfg, path)?;
    Ok(format!("Switched to context '{name}'"))
}

/// Create or update a named context. At least one of `url` / `token`
/// / `workspace_id` must be provided — a no-op call is a user error
/// (exits 1) rather than silently writing an empty entry.
pub fn set(
    path: &Path,
    name: &str,
    url: Option<String>,
    token: Option<String>,
    workspace_id: Option<String>,
) -> Result<String, AppError> {
    if url.is_none() && token.is_none() && workspace_id.is_none() {
        return Err(AppError::BadRequest(
            "pass at least one of --url, --token, --workspace".into(),
        ));
    }
    let mut cfg = config::load_from_path(path)?;
    let entry = cfg
        .contexts
        .entry(name.to_string())
        .or_insert_with(ContextConfig::default);
    if let Some(u) = url {
        entry.url = Some(u);
    }
    if let Some(t) = token {
        entry.token = Some(t);
    }
    if let Some(w) = workspace_id {
        entry.workspace_id = Some(w);
    }
    // A `set` on a fresh install (no current-context) promotes the
    // new entry to active. Keeps `pcy context set prod --url X` usable
    // end-to-end without a follow-up `pcy context use prod`.
    if cfg.current_context.is_none() {
        cfg.current_context = Some(name.to_string());
    }
    // Clear legacy mirror so sync-on-save doesn't re-write the old
    // active context's values on top of the update.
    strip_legacy_mirror(&mut cfg);
    config::save_to_path(&cfg, path)?;
    Ok(format!("Updated context '{name}'"))
}

/// Delete a named context. Refuses to delete the active one so the
/// CLI never lands in a state where `current-context` points at a
/// missing entry.
pub fn delete(path: &Path, name: &str) -> Result<String, AppError> {
    let mut cfg = config::load_from_path(path)?;
    if cfg.current_context.as_deref() == Some(name) {
        return Err(AppError::BadRequest(format!(
            "cannot delete active context '{name}'; use another first"
        )));
    }
    if cfg.contexts.remove(name).is_none() {
        return Err(AppError::NotFound(format!("context '{name}' not found")));
    }
    strip_legacy_mirror(&mut cfg);
    config::save_to_path(&cfg, path)?;
    Ok(format!("Deleted context '{name}'"))
}

/// Zero the legacy top-level url/token/workspace fields so
/// `sync_active_from_legacy` (called by `save_to_path`) doesn't
/// re-stamp stale hydrated values into the active context.
fn strip_legacy_mirror(cfg: &mut CliConfig) {
    cfg.url = None;
    cfg.token = None;
    cfg.workspace_id = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_two(path: &Path) {
        let mut cfg = CliConfig::default();
        cfg.contexts.insert(
            "default".into(),
            ContextConfig {
                url: Some("http://localhost:8080".into()),
                token: Some("t1".into()),
                workspace_id: None,
            },
        );
        cfg.contexts.insert(
            "prod".into(),
            ContextConfig {
                url: Some("https://prod".into()),
                token: Some("t2".into()),
                workspace_id: Some("w-prod".into()),
            },
        );
        cfg.current_context = Some("default".into());
        config::save_to_path(&cfg, path).unwrap();
    }

    #[test]
    fn list_marks_active_context_with_star() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        seed_two(&path);

        let out = list(&path, &OutputFormat::Table).unwrap();
        // Both contexts present.
        assert!(out.contains("default"), "{out}");
        assert!(out.contains("prod"), "{out}");
        // Active marker on the `default` row only.
        let default_line = out.lines().find(|l| l.contains("default")).unwrap();
        assert!(default_line.contains('*'), "{default_line}");
        let prod_line = out.lines().find(|l| l.contains("prod")).unwrap();
        assert!(!prod_line.contains('*'), "{prod_line}");
    }

    #[test]
    fn list_json_round_trips_structurally() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        seed_two(&path);

        let out = list(&path, &OutputFormat::Json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let arr = parsed.as_array().expect("json array");
        assert_eq!(arr.len(), 2);
        let active: Vec<_> = arr
            .iter()
            .filter(|r| r.get("active").and_then(|v| v.as_bool()) == Some(true))
            .collect();
        assert_eq!(active.len(), 1);
        assert_eq!(
            active[0].get("name").and_then(|v| v.as_str()),
            Some("default")
        );
    }

    #[test]
    fn current_returns_active_name() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        seed_two(&path);
        assert_eq!(current(&path).unwrap(), "default");
    }

    #[test]
    fn current_on_empty_config_is_empty_string() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        assert_eq!(current(&path).unwrap(), "");
    }

    #[test]
    fn use_switches_active_context() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        seed_two(&path);

        let msg = use_(&path, "prod").unwrap();
        assert!(msg.contains("prod"));

        let reloaded = config::load_from_path(&path).unwrap();
        assert_eq!(reloaded.current_context.as_deref(), Some("prod"));
        // Legacy mirror reflects the new active context's url.
        assert_eq!(reloaded.url.as_deref(), Some("https://prod"));
    }

    #[test]
    fn use_unknown_context_returns_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        seed_two(&path);
        let err = use_(&path, "ghost").unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)), "{err:?}");
    }

    #[test]
    fn set_creates_new_context_and_promotes_on_fresh_install() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        set(
            &path,
            "dev",
            Some("http://dev".into()),
            Some("dev-token".into()),
            None,
        )
        .unwrap();
        let reloaded = config::load_from_path(&path).unwrap();
        assert_eq!(reloaded.current_context.as_deref(), Some("dev"));
        let dev = reloaded.contexts.get("dev").unwrap();
        assert_eq!(dev.url.as_deref(), Some("http://dev"));
        assert_eq!(dev.token.as_deref(), Some("dev-token"));
    }

    #[test]
    fn set_updates_existing_without_flipping_active() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        seed_two(&path);
        set(&path, "prod", Some("https://prod-2".into()), None, None).unwrap();
        let reloaded = config::load_from_path(&path).unwrap();
        // Active stays `default` — `set` only promotes on fresh install.
        assert_eq!(reloaded.current_context.as_deref(), Some("default"));
        assert_eq!(
            reloaded.contexts.get("prod").unwrap().url.as_deref(),
            Some("https://prod-2")
        );
    }

    #[test]
    fn set_with_no_fields_is_a_user_error() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        let err = set(&path, "ghost", None, None, None).unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)), "{err:?}");
    }

    #[test]
    fn delete_refuses_active_context() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        seed_two(&path);
        let err = delete(&path, "default").unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)), "{err:?}");
        let reloaded = config::load_from_path(&path).unwrap();
        assert!(reloaded.contexts.contains_key("default"));
    }

    #[test]
    fn delete_removes_inactive_context() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        seed_two(&path);
        let msg = delete(&path, "prod").unwrap();
        assert!(msg.contains("prod"));
        let reloaded = config::load_from_path(&path).unwrap();
        assert!(!reloaded.contexts.contains_key("prod"));
        assert!(reloaded.contexts.contains_key("default"));
    }

    #[test]
    fn delete_unknown_returns_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        seed_two(&path);
        let err = delete(&path, "ghost").unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)), "{err:?}");
    }
}
