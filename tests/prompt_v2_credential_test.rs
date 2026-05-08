//! AC-42 (v7): hardened `wake_system_prompt` v2.
//!
//! This test asserts on the migration FILE (not the DB state) because
//! `tests/common/mod.rs` truncates `prompt_templates` and re-seeds a
//! simplified template for other tests. The migration file is the
//! source of truth that ships in production; if any of the required
//! substrings vanish during a refactor, this test catches it.
//!
//! Required substrings, per readiness.md AC-42:
//!   * `pcy credential add` — operator command for interactive storage.
//!   * `POST /api/workspaces/{workspace_id}/credentials` — automation path.
//!   * `PLACEHOLDER:` — the in-command substitution token.
//!   * `REFUSE` — the refusal verb the agent must echo.
//!   * `list_credentials` — the discovery tool.

use std::fs;
use std::path::Path;

fn migration_text() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations")
        .join("20260420000003_prompt_template_credentials.sql");
    fs::read_to_string(&path).expect("migration file present")
}

#[test]
fn ac42_migration_deactivates_v1_and_inserts_v2_active() {
    let sql = migration_text();
    // v1 must be marked inactive.
    assert!(
        sql.contains("UPDATE prompt_templates")
            && sql.contains("is_active = FALSE")
            && sql.contains("version = 1"),
        "migration must deactivate the v1 wake_system_prompt row"
    );
    // v2 must be inserted active.
    assert!(
        sql.contains("INSERT INTO prompt_templates")
            && sql.contains("'wake_system_prompt'")
            && sql.contains("    2,")
            && sql.contains("TRUE,"),
        "migration must insert version 2 with is_active = TRUE"
    );
}

#[test]
fn ac42_prompt_v2_contains_credential_handoff_substrings() {
    let sql = migration_text();
    let required = [
        "pcy credential add",
        "POST /api/workspaces/{workspace_id}/credentials",
        "PLACEHOLDER:",
        "REFUSE",
        "list_credentials",
    ];
    for needle in required {
        assert!(
            sql.contains(needle),
            "migration must reference `{needle}` so the agent has the exact handoff"
        );
    }
}

#[test]
fn ac42_prompt_v2_forbids_echoing_credentials() {
    let sql = migration_text();
    // The refusal pattern explicitly instructs the agent NOT to echo.
    assert!(
        sql.contains("Do NOT acknowledge the credential value")
            || sql.contains("do not acknowledge the credential value")
            || sql.contains("not acknowledge the credential"),
        "migration must explicitly forbid echoing credential values"
    );
    assert!(
        sql.contains("Do NOT repeat it back") || sql.contains("not repeat it back"),
        "migration must explicitly forbid repeating credentials back"
    );
}
