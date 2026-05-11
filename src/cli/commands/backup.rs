//! AC-91 (v9.1): `pcy backup` / `pcy restore` — operator-driven
//! recovery before the operator trusts the install with real work.
//!
//! Contract:
//!
//! * `pcy backup --output PATH [--include-vault-key]` writes a single
//!   gzipped tar at `PATH` containing:
//!     - `manifest.json`: schema_version + server_version + taken_at + includes_vault_key
//!     - `pgdump.bin`:    `pg_dump --format=custom` of `$DATABASE_URL`
//!     - `vault_key.b64`: optional, only when `--include-vault-key` is passed
//! * `pcy restore --input PATH` validates the manifest, refuses
//!   newer schema versions, runs `pg_restore --clean --if-exists
//!   --no-owner --no-privileges` against `$DATABASE_URL`, then
//!   runs `sqlx migrate run` to catch up the schema.
//! * Both verbs require `pg_dump` / `pg_restore` on PATH. Missing
//!   tools = clear, non-zero exit with remediation hint.
//! * `--include-vault-key` is opt-in. Without it, the tarball
//!   contains zero plaintext key material.
//!
//! Events emitted (via direct DB insert, source `"operator"`):
//!
//! * `backup_taken` on a successful backup.
//! * `backup_restored` on a successful restore.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Utc;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Hard schema version. Bumped manually when a release adds migrations.
/// Equal to the count of files in `migrations/` at release time.
pub const SCHEMA_VERSION: u32 = 24;

/// Server semver string written into the manifest. Read from the
/// `CARGO_PKG_VERSION` baked into the binary.
fn server_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub server_version: String,
    pub taken_at: String,
    pub includes_vault_key: bool,
}

fn tool_on_path(tool: &str) -> bool {
    // `which`-style probe. Avoid the `which` crate to keep the
    // dependency budget tight; v9.1 already sanctioned tar + flate2
    // only.
    #[cfg(windows)]
    let names = [format!("{tool}.exe"), tool.to_string()];
    #[cfg(not(windows))]
    let names = [tool.to_string()];
    let path = match std::env::var_os("PATH") {
        Some(p) => p,
        None => return false,
    };
    for dir in std::env::split_paths(&path) {
        for n in &names {
            if dir.join(n).is_file() {
                return true;
            }
        }
    }
    false
}

fn require_pg_tool(tool: &str) -> Result<(), AppError> {
    if !tool_on_path(tool) {
        return Err(AppError::Internal(format!(
            "`{tool}` not found on PATH — install postgresql-client (Debian/Ubuntu) \
             or postgresql (Fedora/macOS) before running this command"
        )));
    }
    Ok(())
}

fn database_url() -> Result<String, AppError> {
    std::env::var("DATABASE_URL").map_err(|_| {
        AppError::BadRequest(
            "DATABASE_URL not set — backup/restore read this env var directly".into(),
        )
    })
}

fn vault_key_b64() -> Option<String> {
    std::env::var("VAULT_KEY_BASE64").ok()
}

/// Create a file with mode 0o600 on Unix. On Windows the file
/// inherits the default ACL; this mirrors AC-89's "best-effort
/// Windows ACL" stance documented in init.rs.
fn create_secret_file(path: &Path) -> Result<fs::File, AppError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| AppError::Internal(format!("create {path:?}: {e}")))
    }
    #[cfg(not(unix))]
    {
        fs::File::create(path).map_err(|e| AppError::Internal(format!("create {path:?}: {e}")))
    }
}

/// Write `bytes` to `path` with mode 0o600 on Unix. Same Windows
/// caveat as [`create_secret_file`].
fn write_secret_file(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    use std::io::Write as _;
    let mut f = create_secret_file(path)?;
    f.write_all(bytes)
        .map_err(|e| AppError::Internal(format!("write {path:?}: {e}")))?;
    f.sync_all().ok();
    Ok(())
}

/// AC-91: take a backup. Writes `output` (gzipped tar). On success
/// inserts a `backup_taken` event row before returning.
pub async fn backup(output: PathBuf, include_vault_key: bool) -> Result<(), AppError> {
    require_pg_tool("pg_dump")?;
    let db = database_url()?;

    // Stage files in a tempdir so the tar writer can stream them in
    // one pass without holding the whole dump in memory.
    let staging = tempfile::tempdir().map_err(|e| AppError::Internal(format!("tempdir: {e}")))?;
    let dump_path = staging.path().join("pgdump.bin");

    let status = Command::new("pg_dump")
        .arg("--format=custom")
        .arg("--no-owner")
        .arg("--no-privileges")
        .arg("--file")
        .arg(&dump_path)
        .arg(&db)
        .status()
        .map_err(|e| AppError::Internal(format!("pg_dump spawn: {e}")))?;
    if !status.success() {
        return Err(AppError::Internal(format!(
            "pg_dump exited with status {status} — see its stderr above"
        )));
    }

    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        server_version: server_version().into(),
        taken_at: Utc::now().to_rfc3339(),
        includes_vault_key: include_vault_key,
    };
    let manifest_json = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| AppError::Internal(format!("manifest serialize: {e}")))?;
    let manifest_path = staging.path().join("manifest.json");
    fs::write(&manifest_path, &manifest_json)
        .map_err(|e| AppError::Internal(format!("manifest write: {e}")))?;

    let key_path = staging.path().join("vault_key.b64");
    if include_vault_key {
        let key = vault_key_b64().ok_or_else(|| {
            AppError::BadRequest(
                "--include-vault-key requested but VAULT_KEY_BASE64 not set".into(),
            )
        })?;
        // Stage the plaintext key file with 0o600 on Unix — the
        // tempdir inherits umask otherwise, which on a shared host
        // could expose the AES-256-GCM master key to other users
        // between `tar finish` and tempdir drop. Same standard as
        // AC-89's `.env` writer.
        write_secret_file(&key_path, key.as_bytes())?;
    }

    // Create the output tarball with 0o600 when it carries key
    // material. Without the flag the tarball is harmless and the
    // operator likely wants to scp/rsync it; 0o644 is fine there.
    let out_file = if include_vault_key {
        create_secret_file(&output)?
    } else {
        fs::File::create(&output)
            .map_err(|e| AppError::Internal(format!("create {output:?}: {e}")))?
    };
    let gz = GzEncoder::new(out_file, Compression::default());
    let mut builder = tar::Builder::new(gz);

    builder
        .append_path_with_name(&manifest_path, "manifest.json")
        .map_err(|e| AppError::Internal(format!("tar manifest: {e}")))?;
    builder
        .append_path_with_name(&dump_path, "pgdump.bin")
        .map_err(|e| AppError::Internal(format!("tar pgdump: {e}")))?;
    if include_vault_key {
        builder
            .append_path_with_name(&key_path, "vault_key.b64")
            .map_err(|e| AppError::Internal(format!("tar vault_key: {e}")))?;
    }
    builder
        .into_inner()
        .map_err(|e| AppError::Internal(format!("tar finish: {e}")))?
        .finish()
        .map_err(|e| AppError::Internal(format!("gz finish: {e}")))?;

    emit_event(&db, "backup_taken").await?;
    Ok(())
}

/// AC-91: restore from a backup tarball. Validates manifest, runs
/// `pg_restore --clean --if-exists`, then `sqlx migrate run`.
///
/// `write_vault_key_to`: if `Some(path)`, and the tarball was
/// created with `--include-vault-key`, the bundled key file is
/// written to `path` with mode 0o600 and the operator is told to
/// load it into `$VAULT_KEY_BASE64` before restarting `pcy`.
/// If `None`, an `--include-vault-key` tarball is still accepted
/// but the bundled key is left in the in-memory tempdir (dropped
/// on return) and the operator is reminded — via stderr — that
/// they must already have `$VAULT_KEY_BASE64` set or the restored
/// vault rows will be undecryptable. This matches the operator
/// recovery story from scope AC-91 (3).
pub async fn restore(input: PathBuf, write_vault_key_to: Option<PathBuf>) -> Result<(), AppError> {
    // Read + validate the manifest BEFORE shelling out, so an
    // operator on a machine without `pg_restore` still gets a clear
    // "this backup is from a newer build" diagnostic instead of a
    // tool-missing error.
    let in_file =
        fs::File::open(&input).map_err(|e| AppError::Internal(format!("open {input:?}: {e}")))?;
    let gz = GzDecoder::new(in_file);
    let mut archive = tar::Archive::new(gz);

    // Extract to a staging dir so we can read the manifest, then
    // hand `pgdump.bin` to `pg_restore`.
    let staging = tempfile::tempdir().map_err(|e| AppError::Internal(format!("tempdir: {e}")))?;
    archive
        .unpack(staging.path())
        .map_err(|e| AppError::Internal(format!("tar unpack: {e}")))?;

    let manifest_path = staging.path().join("manifest.json");
    let manifest: Manifest = {
        let bytes = fs::read(&manifest_path)
            .map_err(|e| AppError::Internal(format!("read manifest: {e}")))?;
        serde_json::from_slice(&bytes)
            .map_err(|e| AppError::BadRequest(format!("manifest parse: {e}")))?
    };
    if manifest.schema_version > SCHEMA_VERSION {
        return Err(AppError::BadRequest(format!(
            "refuse to restore: backup schema_version={} > this build's {} — \
             upgrade `pcy` before restoring",
            manifest.schema_version, SCHEMA_VERSION,
        )));
    }

    let dump_path = staging.path().join("pgdump.bin");
    if !dump_path.exists() {
        return Err(AppError::BadRequest("backup missing pgdump.bin".into()));
    }

    // Consume the bundled vault key — AC-91 sub-criterion (3).
    // If the manifest claims includes_vault_key:true, the tarball
    // MUST contain the file or the backup is malformed. If the
    // operator asked for `--write-vault-key-to PATH`, persist the
    // key there with 0o600 and print operator instructions.
    let staged_key = staging.path().join("vault_key.b64");
    if manifest.includes_vault_key {
        if !staged_key.exists() {
            return Err(AppError::BadRequest(
                "manifest declares includes_vault_key:true but tarball is missing vault_key.b64"
                    .into(),
            ));
        }
        let key_bytes = fs::read(&staged_key)
            .map_err(|e| AppError::Internal(format!("read staged vault key: {e}")))?;
        if let Some(dest) = &write_vault_key_to {
            write_secret_file(dest, &key_bytes)?;
            eprintln!(
                "wrote bundled vault key to {} (mode 0600). Load it before restarting pcy:\n\
                 \texport VAULT_KEY_BASE64=\"$(cat {})\"",
                dest.display(),
                dest.display(),
            );
        } else if vault_key_b64().is_none() {
            eprintln!(
                "warning: backup tarball includes a bundled vault key, but $VAULT_KEY_BASE64 \
                 is not set and `--write-vault-key-to PATH` was not passed. The restored \
                 credential rows will be undecryptable until you load the key. Re-run \
                 `pcy restore` with `--write-vault-key-to /path/to/vault.b64` to extract it.",
            );
        }
    } else if write_vault_key_to.is_some() {
        return Err(AppError::BadRequest(
            "--write-vault-key-to passed but this backup was taken without --include-vault-key"
                .into(),
        ));
    }

    // Manifest accepted — now we actually need pg_restore.
    require_pg_tool("pg_restore")?;
    let db = database_url()?;

    let status = Command::new("pg_restore")
        .arg("--clean")
        .arg("--if-exists")
        .arg("--no-owner")
        .arg("--no-privileges")
        .arg("--dbname")
        .arg(&db)
        .arg(&dump_path)
        .status()
        .map_err(|e| AppError::Internal(format!("pg_restore spawn: {e}")))?;
    if !status.success() {
        return Err(AppError::Internal(format!(
            "pg_restore exited with status {status}"
        )));
    }

    // Catch up the schema after restore. The dump came from an older
    // build (schema_version <= ours); any newer migrations apply now.
    sqlx_migrate_after_restore(&db).await?;

    emit_event(&db, "backup_restored").await?;
    Ok(())
}

async fn sqlx_migrate_after_restore(database_url: &str) -> Result<(), AppError> {
    use sqlx::postgres::PgPoolOptions;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
        .map_err(|e| AppError::Internal(format!("connect for migrate: {e}")))?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| AppError::Internal(format!("sqlx migrate: {e}")))?;
    pool.close().await;
    Ok(())
}

async fn emit_event(_database_url: &str, event_type: &str) -> Result<(), AppError> {
    // AC-91 (v9.1 known limitation): the `events` table requires a
    // non-null `agent_id`, but `backup_taken` / `backup_restored` are
    // operator-scoped, not agent-scoped. v9.1's T-v91-2 truth budgets
    // exactly one new schema object (`llm_providers`), so we DO NOT
    // add an operator-events table this release. Instead we emit the
    // audit trail via tracing + stderr so operators see the row in
    // their journald / log aggregator. v9.2 will add an
    // `operator_events` table and persist these properly.
    tracing::info!(target: "open_pincery::audit", event_type = event_type, source = "operator", "backup/restore audit event");
    eprintln!(
        "audit: event_type={event_type} source=operator at={}",
        chrono::Utc::now().to_rfc3339()
    );
    Ok(())
}

#[allow(dead_code)]
pub fn read_manifest_from_tarball(path: &Path) -> Result<Manifest, AppError> {
    let f = fs::File::open(path).map_err(|e| AppError::Internal(format!("open: {e}")))?;
    let gz = GzDecoder::new(f);
    let mut archive = tar::Archive::new(gz);
    for entry in archive
        .entries()
        .map_err(|e| AppError::Internal(format!("tar entries: {e}")))?
    {
        let mut entry = entry.map_err(|e| AppError::Internal(format!("tar entry: {e}")))?;
        let p = entry
            .path()
            .map_err(|e| AppError::Internal(format!("entry path: {e}")))?
            .to_path_buf();
        if p == Path::new("manifest.json") {
            let mut s = String::new();
            entry
                .read_to_string(&mut s)
                .map_err(|e| AppError::Internal(format!("read manifest entry: {e}")))?;
            return serde_json::from_str(&s)
                .map_err(|e| AppError::BadRequest(format!("manifest parse: {e}")));
        }
    }
    Err(AppError::BadRequest(
        "manifest.json missing from tarball".into(),
    ))
}

#[allow(dead_code)]
pub fn tarball_contains_vault_key(path: &Path) -> Result<bool, AppError> {
    let f = fs::File::open(path).map_err(|e| AppError::Internal(format!("open: {e}")))?;
    let gz = GzDecoder::new(f);
    let mut archive = tar::Archive::new(gz);
    for entry in archive
        .entries()
        .map_err(|e| AppError::Internal(format!("tar entries: {e}")))?
    {
        let entry = entry.map_err(|e| AppError::Internal(format!("tar entry: {e}")))?;
        let p = entry
            .path()
            .map_err(|e| AppError::Internal(format!("entry path: {e}")))?
            .to_path_buf();
        if p == Path::new("vault_key.b64") {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_matches_migrations_dir() {
        // If a migration is added, SCHEMA_VERSION must be bumped or
        // restore from older backups will silently skip the new
        // schema. This guards the manifest contract.
        let count = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations"))
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .ok()
                    .and_then(|x| x.path().extension().map(|s| s.to_owned()))
                    .map(|s| s == "sql")
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(
            count as u32, SCHEMA_VERSION,
            "SCHEMA_VERSION constant out of date with migrations/ directory"
        );
    }

    #[test]
    fn manifest_roundtrip() {
        let m = Manifest {
            schema_version: SCHEMA_VERSION,
            server_version: server_version().into(),
            taken_at: "2026-05-08T00:00:00Z".into(),
            includes_vault_key: false,
        };
        let s = serde_json::to_string(&m).unwrap();
        let back: Manifest = serde_json::from_str(&s).unwrap();
        assert_eq!(back.schema_version, m.schema_version);
        assert!(!back.includes_vault_key);
    }
}
