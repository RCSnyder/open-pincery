//! AC-48 (v8): v4→v8 config migration.
//!
//! A v4 config is a flat TOML with top-level `url` / `token` /
//! `workspace_id` and no `contexts` section. v8 stores the same data
//! under a named context, so migration is a pure reshape:
//!
//! 1. Back up the original file to `<path>.pre-v8` (skipped on
//!    overwrite — the first backup wins so repeated runs don't
//!    clobber user history).
//! 2. Move the legacy fields into `contexts["default"]` in memory,
//!    clear the legacy fields, set `current_context = Some("default")`.
//! 3. Atomic-save the migrated shape to `<path>`.
//! 4. [`CliConfig::hydrate_legacy_from_active`] (called by the
//!    loader) re-populates the legacy top-level view so every v1–v7
//!    call site keeps working.
//!
//! Migration is idempotent: once `contexts` is non-empty the loader
//! short-circuits the check ([`CliConfig::is_v4_flat`]) so a second
//! run is a no-op.

use std::path::Path;

use crate::cli::config::{atomic_write, CliConfig, ContextConfig};
use crate::error::AppError;

/// Convert a v4 flat config into v8 shape in place, writing a
/// `<path>.pre-v8` backup iff the original file exists and no backup
/// has been written yet.
pub fn migrate_v4_to_v8(cfg: &mut CliConfig, path: &Path) -> Result<(), AppError> {
    if !cfg.is_v4_flat() {
        return Ok(());
    }

    if path.exists() {
        let backup = path.with_extension(backup_extension(path));
        if !backup.exists() {
            let bytes = std::fs::read(path).map_err(|e| {
                AppError::Internal(format!("failed reading config for backup: {e}"))
            })?;
            atomic_write(&backup, &bytes)?;
        }
    }

    let ctx = ContextConfig {
        url: cfg.url.take(),
        token: cfg.token.take(),
        workspace_id: cfg.workspace_id.take(),
    };
    cfg.contexts.insert("default".to_string(), ctx);
    cfg.current_context = Some("default".to_string());

    if path.exists() || !cfg.contexts.is_empty() {
        let text = toml::to_string(cfg)
            .map_err(|e| AppError::Internal(format!("failed serializing config: {e}")))?;
        atomic_write(path, text.as_bytes())?;
    }

    Ok(())
}

/// Compute the backup file extension. `config.toml` → `toml.pre-v8`
/// so the backup lands at `config.toml.pre-v8`. For extensionless
/// paths we fall back to `"pre-v8"`.
fn backup_extension(path: &Path) -> String {
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) => format!("{ext}.pre-v8"),
        None => "pre-v8".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::config::{load_from_path, CliConfig};

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn migrates_flat_file_into_default_context() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        write(
            &path,
            "url = \"http://x\"\ntoken = \"t\"\nworkspace_id = \"w\"\n",
        );

        let cfg = load_from_path(&path).unwrap();
        assert_eq!(cfg.current_context.as_deref(), Some("default"));
        let ctx = cfg.contexts.get("default").unwrap();
        assert_eq!(ctx.url.as_deref(), Some("http://x"));
        assert_eq!(ctx.token.as_deref(), Some("t"));
        assert_eq!(ctx.workspace_id.as_deref(), Some("w"));
        // Legacy fields re-hydrated from the active context.
        assert_eq!(cfg.url.as_deref(), Some("http://x"));
    }

    #[test]
    fn writes_backup_to_pre_v8_sibling() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        let original = "url = \"http://x\"\ntoken = \"t\"\n";
        write(&path, original);

        load_from_path(&path).unwrap();

        let backup = tmp.path().join("config.toml.pre-v8");
        assert!(backup.exists(), "backup not created");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);
    }

    #[test]
    fn migration_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        write(&path, "url = \"http://x\"\ntoken = \"t\"\n");

        let first = load_from_path(&path).unwrap();
        let after_first = std::fs::read_to_string(&path).unwrap();

        // Second load must not re-run migration: file unchanged, backup
        // unchanged, contexts still exactly one entry.
        let second = load_from_path(&path).unwrap();
        let after_second = std::fs::read_to_string(&path).unwrap();

        assert_eq!(after_first, after_second);
        assert_eq!(first.contexts.len(), second.contexts.len());
        assert_eq!(second.contexts.len(), 1);
    }

    #[test]
    fn backup_is_written_only_once() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        write(&path, "url = \"http://x\"\n");

        // First load migrates + writes backup.
        load_from_path(&path).unwrap();
        let backup = tmp.path().join("config.toml.pre-v8");
        let first_backup = std::fs::read_to_string(&backup).unwrap();

        // Simulate a later legacy-shaped overwrite (should not occur in
        // practice, but guards against clobbering the first backup).
        write(&path, "url = \"http://other\"\n");
        load_from_path(&path).unwrap();
        let second_backup = std::fs::read_to_string(&backup).unwrap();

        assert_eq!(
            first_backup, second_backup,
            "backup must be preserved on subsequent migrations"
        );
    }

    #[test]
    fn v8_shaped_file_is_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        write(
            &path,
            r#"current-context = "prod"

[contexts.prod]
url = "https://prod"
token = "t"
"#,
        );
        let before = std::fs::read_to_string(&path).unwrap();

        let cfg = load_from_path(&path).unwrap();
        assert_eq!(cfg.current_context.as_deref(), Some("prod"));
        assert!(!tmp.path().join("config.toml.pre-v8").exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    }

    #[test]
    fn empty_config_is_a_noop() {
        let mut cfg = CliConfig::default();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        // Path does not exist, cfg is empty → no migration, no backup.
        migrate_v4_to_v8(&mut cfg, &path).unwrap();
        assert!(cfg.contexts.is_empty());
        assert!(cfg.current_context.is_none());
        assert!(!path.exists());
    }
}
