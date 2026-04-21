//! AC-48 integration: named CLI contexts end-to-end on disk.
//!
//! These exercise the public [`config::load_from_path`] /
//! [`config::save_to_path`] surface (explicit-path API) to avoid the
//! process-wide `PCY_CONFIG_PATH` env var — tests run in parallel and
//! a shared env var would race.

use open_pincery::cli::config::{self, CliConfig, ContextConfig};

/// AC-48: a v4 flat config on disk migrates to the v8 shape on first
/// load, writes a `.pre-v8` backup, and leaves subsequent loads as
/// no-ops (idempotency).
#[test]
fn v4_flat_config_migrates_with_backup_and_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.toml");
    let original = "url = \"http://legacy\"\ntoken = \"legacy-token\"\nworkspace_id = \"018f\"\n";
    std::fs::write(&path, original).unwrap();

    // First load -> migrates.
    let cfg = config::load_from_path(&path).unwrap();
    assert_eq!(cfg.current_context.as_deref(), Some("default"));
    let ctx = cfg
        .contexts
        .get("default")
        .expect("default context populated");
    assert_eq!(ctx.url.as_deref(), Some("http://legacy"));
    assert_eq!(ctx.token.as_deref(), Some("legacy-token"));
    assert_eq!(ctx.workspace_id.as_deref(), Some("018f"));

    // Backup captured the pre-migration bytes verbatim.
    let backup = tmp.path().join("config.toml.pre-v8");
    assert!(backup.exists(), "backup missing");
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);

    // On-disk file is in v8 shape.
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(
        after.contains("[contexts.default]"),
        "v8 shape missing: {after}"
    );
    assert!(
        after.contains("current-context = \"default\""),
        "current-context missing: {after}"
    );

    // Second load is a no-op: file + backup unchanged, contexts stable.
    let second = config::load_from_path(&path).unwrap();
    assert_eq!(second.contexts.len(), 1);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), after);
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);
}

/// AC-48: a file already holding two named contexts round-trips
/// through load/save without losing either one, and `current-context`
/// is durable across reloads.
#[test]
fn two_context_file_switching_current_context_persists() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.toml");

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
            url: Some("https://pincery.example.com".into()),
            token: Some("t2".into()),
            workspace_id: Some("w-prod".into()),
        },
    );
    cfg.current_context = Some("default".into());
    config::save_to_path(&cfg, &path).unwrap();

    // Flip current-context to prod and re-save.
    let mut reloaded = config::load_from_path(&path).unwrap();
    assert_eq!(reloaded.current_context.as_deref(), Some("default"));
    assert_eq!(reloaded.url.as_deref(), Some("http://localhost:8080"));
    reloaded.current_context = Some("prod".into());
    // Clear legacy fields so hydration picks them up from the new
    // active context instead of leaking the previous context's values.
    reloaded.url = None;
    reloaded.token = None;
    reloaded.workspace_id = None;
    config::save_to_path(&reloaded, &path).unwrap();

    // Next load reflects the switch; both contexts still intact.
    let after = config::load_from_path(&path).unwrap();
    assert_eq!(after.current_context.as_deref(), Some("prod"));
    assert_eq!(after.contexts.len(), 2);
    assert_eq!(after.url.as_deref(), Some("https://pincery.example.com"));
    assert_eq!(after.workspace_id.as_deref(), Some("w-prod"));
    assert!(after.contexts.contains_key("default"));
}

/// AC-48: legacy-field writes (the v1–v7 surface) keep working and
/// persist into the v8 active-context entry. Guards slices 2d–2e
/// against accidentally breaking `bootstrap` / `login` / `demo` /
/// `credential` before they move into `nouns/`.
#[test]
fn legacy_field_write_persists_into_active_context() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.toml");

    // Fresh install: no file. Simulate the v1-v7 call pattern.
    let mut cfg = config::load_from_path(&path).unwrap();
    assert!(cfg.contexts.is_empty());
    cfg.url = Some("http://new".into());
    cfg.token = Some("new-token".into());
    cfg.workspace_id = Some("018g".into());
    config::save_to_path(&cfg, &path).unwrap();

    let reloaded = config::load_from_path(&path).unwrap();
    assert_eq!(reloaded.current_context.as_deref(), Some("default"));
    let ctx = reloaded.contexts.get("default").unwrap();
    assert_eq!(ctx.url.as_deref(), Some("http://new"));
    assert_eq!(ctx.token.as_deref(), Some("new-token"));
    assert_eq!(ctx.workspace_id.as_deref(), Some("018g"));
    // Hydrated back into legacy view.
    assert_eq!(reloaded.url.as_deref(), Some("http://new"));
}

/// AC-48: save is atomic — a tempfile under the config dir is used
/// and cleaned up, and a pre-existing config is replaced, not
/// appended to or truncated-then-written.
#[test]
fn save_is_atomic_and_leaves_no_tempfile() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.toml");
    std::fs::write(&path, "stale = true\n").unwrap();

    let mut cfg = CliConfig::default();
    cfg.contexts.insert(
        "default".into(),
        ContextConfig {
            url: Some("http://x".into()),
            token: None,
            workspace_id: None,
        },
    );
    cfg.current_context = Some("default".into());
    config::save_to_path(&cfg, &path).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("stale"), "old content leaked: {text}");
    assert!(text.contains("[contexts.default]"));

    // No sibling tempfile.
    let entries: Vec<_> = std::fs::read_dir(tmp.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name())
        .collect();
    assert!(
        !entries
            .iter()
            .any(|n| n.to_string_lossy().ends_with(".tmp")),
        "tempfile not cleaned up: {entries:?}"
    );
}
