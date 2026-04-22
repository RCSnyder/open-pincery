//! AC-46 / AC-48 integration: the `pcy context` noun end-to-end.
//!
//! These exercise the public [`cli::nouns::context`] verb functions
//! via the explicit-path API so tests stay hermetic (no process-wide
//! `PCY_CONFIG_PATH` races under parallel execution). Slice 2d-ii
//! will add subprocess-based byte-identical-stdout shim tests for
//! credential/agent/etc.; for now we prove the verb surface works.

use open_pincery::cli::config::{self, CliConfig, ContextConfig};
use open_pincery::cli::nouns::context::{self, ContextCommands, ContextRow};
use open_pincery::cli::output::OutputFormat;

fn write_two_contexts(path: &std::path::Path) {
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

/// AC-48: `pcy context list` over a two-context fixture renders both
/// rows, marks exactly one active, and the JSON variant is a valid
/// array with correct shape.
#[test]
fn context_list_renders_all_contexts_with_active_marker() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.toml");
    write_two_contexts(&path);

    // Table output — active marker present on exactly one row.
    let table = context::list(&path, &OutputFormat::Table).unwrap();
    assert!(table.contains("default"), "{table}");
    assert!(table.contains("prod"), "{table}");
    let active_lines: Vec<&str> = table
        .lines()
        .filter(|l| l.contains('*') && !l.contains("ACTIVE"))
        .collect();
    assert_eq!(active_lines.len(), 1, "exactly one active row: {table}");

    // JSON output — parseable, two items, exactly one active.
    let json = context::list(&path, &OutputFormat::Json).unwrap();
    let rows: Vec<ContextRow> = serde_json::from_str(&json).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows.iter().filter(|r| r.active).count(), 1);

    // Name output — one name per line, both present.
    let names = context::list(&path, &OutputFormat::Name).unwrap();
    let name_lines: Vec<&str> = names.lines().collect();
    assert!(name_lines.contains(&"default"), "{names}");
    assert!(name_lines.contains(&"prod"), "{names}");
}

/// AC-48: `pcy context use <name>` flips current-context and the
/// legacy-mirror url/token now reflect the newly-active context on
/// the next load. Guards against leaking the previous context's
/// credentials into HTTP calls after a switch.
#[test]
fn context_use_switches_active_and_updates_legacy_mirror() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.toml");
    write_two_contexts(&path);

    // Sanity: before switch, legacy mirror points at default.
    let before = config::load_from_path(&path).unwrap();
    assert_eq!(before.url.as_deref(), Some("http://localhost:8080"));

    // Dispatch through the verb enum path — same wire the clap
    // subcommand uses at runtime.
    let msg = context::run(
        ContextCommands::Use {
            name: "prod".into(),
        },
        &path,
        &OutputFormat::Table,
    )
    .unwrap();
    assert!(msg.contains("prod"), "{msg}");

    let after = config::load_from_path(&path).unwrap();
    assert_eq!(after.current_context.as_deref(), Some("prod"));
    assert_eq!(after.url.as_deref(), Some("https://prod"));
    assert_eq!(after.token.as_deref(), Some("t2"));
    assert_eq!(after.workspace_id.as_deref(), Some("w-prod"));
    // Both contexts still intact.
    assert_eq!(after.contexts.len(), 2);
}

/// AC-48: `pcy context set <name> --url X` creates a new context on
/// a fresh install and promotes it to active (no follow-up `use`
/// required). A subsequent set on an existing context updates fields
/// without flipping active.
#[test]
fn context_set_creates_and_updates_correctly() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.toml");

    // Fresh install: no file on disk. `set` should create + promote.
    context::run(
        ContextCommands::Set {
            name: "dev".into(),
            url: Some("http://dev".into()),
            token: Some("t".into()),
            workspace_id: None,
        },
        &path,
        &OutputFormat::Table,
    )
    .unwrap();

    let after_create = config::load_from_path(&path).unwrap();
    assert_eq!(after_create.current_context.as_deref(), Some("dev"));
    assert_eq!(
        after_create.contexts.get("dev").unwrap().url.as_deref(),
        Some("http://dev")
    );

    // Second context — should NOT auto-promote when an active one exists.
    context::run(
        ContextCommands::Set {
            name: "prod".into(),
            url: Some("https://prod".into()),
            token: None,
            workspace_id: None,
        },
        &path,
        &OutputFormat::Table,
    )
    .unwrap();

    let after_second = config::load_from_path(&path).unwrap();
    assert_eq!(after_second.current_context.as_deref(), Some("dev"));
    assert_eq!(after_second.contexts.len(), 2);
}

/// AC-48: `pcy context delete <active>` refuses with a BadRequest;
/// delete of an inactive context succeeds and leaves active intact.
#[test]
fn context_delete_refuses_active_and_removes_inactive() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.toml");
    write_two_contexts(&path);

    // Delete active -> error, no mutation.
    let err = context::run(
        ContextCommands::Delete {
            name: "default".into(),
        },
        &path,
        &OutputFormat::Table,
    )
    .unwrap_err();
    let err_text = format!("{err}");
    assert!(
        err_text.contains("cannot delete active"),
        "unexpected error: {err_text}"
    );
    let unchanged = config::load_from_path(&path).unwrap();
    assert_eq!(unchanged.contexts.len(), 2);

    // Delete inactive -> success.
    context::run(
        ContextCommands::Delete {
            name: "prod".into(),
        },
        &path,
        &OutputFormat::Table,
    )
    .unwrap();
    let after = config::load_from_path(&path).unwrap();
    assert_eq!(after.contexts.len(), 1);
    assert!(after.contexts.contains_key("default"));
    assert_eq!(after.current_context.as_deref(), Some("default"));
}

/// AC-48: `pcy context current` prints the active name. Empty string
/// + success (not an error) when nothing is configured — matches
///   `kubectl config current-context` contract so shell scripts can
///   test with `[ -n "$(pcy context current)" ]`.
#[test]
fn context_current_matches_active_or_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.toml");

    // Empty config: empty string.
    assert_eq!(context::current(&path).unwrap(), "");

    // Populated: the name.
    write_two_contexts(&path);
    assert_eq!(context::current(&path).unwrap(), "default");

    // After switch: updated.
    context::use_(&path, "prod").unwrap();
    assert_eq!(context::current(&path).unwrap(), "prod");
}
