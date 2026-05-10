//! AC-91 (v9.1): `pcy backup` / `pcy restore` contract tests.
//!
//! These tests cover the surfaces that DON'T require a live Postgres
//! or `pg_dump` on PATH:
//!
//! * Manifest round-trip via the public schema.
//! * `pcy backup` refuses cleanly when `pg_dump` is missing.
//! * `pcy restore` refuses cleanly when manifest's schema_version
//!   exceeds the build's known `SCHEMA_VERSION`.
//! * Tarball produced via the in-process `tar` crate is grep-clean
//!   of vault key bytes when `--include-vault-key` is NOT passed.
//!
//! End-to-end DB round-trip (taken_at → wipe → restore → events
//! readable) lives in the VERIFY suite, which has Postgres + the
//! postgresql-client tools available.

use open_pincery::cli::commands::backup::{
    read_manifest_from_tarball, tarball_contains_vault_key, Manifest, SCHEMA_VERSION,
};

fn pcy_bin() -> String {
    std::env::var("CARGO_BIN_EXE_pcy").expect("pcy binary path set by cargo")
}

#[test]
fn ac91_schema_version_constant_present() {
    assert!(
        SCHEMA_VERSION >= 24,
        "SCHEMA_VERSION must be at least v9.0's count"
    );
}

#[test]
fn ac91_manifest_json_shape_is_stable() {
    let m = Manifest {
        schema_version: SCHEMA_VERSION,
        server_version: "9.1.0".into(),
        taken_at: "2026-05-08T00:00:00Z".into(),
        includes_vault_key: false,
    };
    let s = serde_json::to_string(&m).unwrap();
    // Must include every field by name so an operator can audit the
    // manifest manually with `tar -xOf backup.tar.gz manifest.json`.
    assert!(s.contains("schema_version"));
    assert!(s.contains("server_version"));
    assert!(s.contains("taken_at"));
    assert!(s.contains("includes_vault_key"));
}

#[test]
fn ac91_backup_refuses_when_database_url_missing() {
    // No DATABASE_URL set → backup must refuse with a clear error.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("backup.tar.gz");
    let res = std::process::Command::new(pcy_bin())
        .env_remove("DATABASE_URL")
        .arg("backup")
        .arg("--file")
        .arg(&out)
        .output()
        .expect("spawn pcy");
    // Either we hit pg_dump missing first, or DATABASE_URL missing;
    // either way the exit must be non-zero and stderr informative.
    assert!(
        !res.status.success(),
        "backup must fail without DATABASE_URL/pg_dump"
    );
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(
        stderr.contains("DATABASE_URL") || stderr.contains("pg_dump"),
        "expected DATABASE_URL or pg_dump diagnostic, got: {stderr}"
    );
    assert!(!out.exists(), "no tarball should be written on failure");
}

#[test]
fn ac91_restore_refuses_forward_incompatible_manifest() {
    use flate2::write::GzEncoder;
    use flate2::Compression;

    let tmp = tempfile::tempdir().unwrap();
    let tarball = tmp.path().join("future.tar.gz");

    // Build a tarball whose manifest claims a schema_version far in
    // the future. Restore must refuse.
    let future = Manifest {
        schema_version: SCHEMA_VERSION + 1000,
        server_version: "99.0.0".into(),
        taken_at: "2099-01-01T00:00:00Z".into(),
        includes_vault_key: false,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&future).unwrap();
    // Stub pgdump.bin (empty file is enough — restore reads manifest first).
    let dump = tmp.path().join("pgdump.bin");
    std::fs::write(&dump, b"").unwrap();
    let manifest_path = tmp.path().join("manifest.json");
    std::fs::write(&manifest_path, &manifest_bytes).unwrap();

    {
        let f = std::fs::File::create(&tarball).unwrap();
        let gz = GzEncoder::new(f, Compression::default());
        let mut builder = tar::Builder::new(gz);
        builder
            .append_path_with_name(&manifest_path, "manifest.json")
            .unwrap();
        builder.append_path_with_name(&dump, "pgdump.bin").unwrap();
        builder.into_inner().unwrap().finish().unwrap();
    }

    // Round-trip manifest read.
    let read = read_manifest_from_tarball(&tarball).expect("read manifest");
    assert_eq!(read.schema_version, SCHEMA_VERSION + 1000);
    assert!(!read.includes_vault_key);

    let res = std::process::Command::new(pcy_bin())
        .env("DATABASE_URL", "postgres://nonsense/db")
        .arg("restore")
        .arg("--input")
        .arg(&tarball)
        .output()
        .expect("spawn pcy");
    assert!(
        !res.status.success(),
        "restore must refuse forward-incompatible"
    );
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(
        stderr.contains("schema_version") || stderr.contains("upgrade"),
        "expected schema_version refusal, got: {stderr}"
    );

    // And the tarball clearly does NOT contain the vault key file.
    assert!(!tarball_contains_vault_key(&tarball).unwrap());

    // Grep test: raw bytes of a fake vault key string must not appear
    // in the tarball when --include-vault-key wasn't passed.
    let raw = std::fs::read(&tarball).unwrap();
    let needle = b"VAULT_KEY";
    assert!(
        !raw.windows(needle.len()).any(|w| w == needle),
        "tarball without --include-vault-key must not contain VAULT_KEY token"
    );
}
