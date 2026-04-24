//! AC-84 startup-preflight integration checks.
//!
//! These tests exercise the process-entry preflight behavior wired in
//! `src/main.rs`: unmet floor requirements must fail closed with exit
//! code 4 before normal server bootstrap.

#![cfg(target_os = "linux")]

use std::process::Command;

fn server_bin() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_open-pincery") {
        return std::path::PathBuf::from(path);
    }
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_open_pincery") {
        return std::path::PathBuf::from(path);
    }
    panic!("cargo did not provide CARGO_BIN_EXE_open-pincery/open_pincery");
}

#[test]
fn relaxed_without_allow_unsafe_exits_4_and_logs_event() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let output = Command::new(server_bin())
        .current_dir(tmp.path())
        .env_clear()
        .env("LOG_FORMAT", "json")
        .env("OPEN_PINCERY_SANDBOX_FLOOR", "relaxed")
        .output()
        .expect("spawn server binary");

    assert_eq!(
        output.status.code(),
        Some(4),
        "expected preflight exit code 4, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("sandbox_kernel_floor_unmet")
            || stderr.contains("sandbox_kernel_floor_unmet"),
        "missing sandbox_kernel_floor_unmet event in output: stdout={stdout} stderr={stderr}"
    );
}
