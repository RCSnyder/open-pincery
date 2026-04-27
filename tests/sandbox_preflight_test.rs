//! AC-84 startup-preflight integration checks.
//!
//! These tests exercise the process-entry preflight behavior wired in
//! `src/main.rs`: unmet floor requirements must fail closed with exit
//! code 4 before normal server bootstrap.

#![cfg(target_os = "linux")]

use std::process::Command;

fn server_bin() -> std::path::PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_open-pincery") {
        return std::path::PathBuf::from(path);
    }
    if let Some(path) = option_env!("CARGO_BIN_EXE_open_pincery") {
        return std::path::PathBuf::from(path);
    }
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_open-pincery") {
        return std::path::PathBuf::from(path);
    }
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_open_pincery") {
        return std::path::PathBuf::from(path);
    }
    panic!("cargo did not provide CARGO_BIN_EXE_open-pincery/open_pincery");
}

fn output_text(output: &std::process::Output) -> String {
    format!(
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn ac84_positive_evidence_enabled() -> bool {
    std::env::var("OPEN_PINCERY_RUN_AC84_POSITIVE")
        .map(|v| v.trim() == "1")
        .unwrap_or(false)
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
        "expected preflight exit code 4, {}",
        output_text(&output)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("sandbox_kernel_floor_unmet")
            || stderr.contains("sandbox_kernel_floor_unmet"),
        "missing sandbox_kernel_floor_unmet event in output: stdout={stdout} stderr={stderr}"
    );
}

#[test]
fn relaxed_without_allow_unsafe_text_logging_still_names_event() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let output = Command::new(server_bin())
        .current_dir(tmp.path())
        .env_clear()
        .env("OPEN_PINCERY_SANDBOX_FLOOR", "relaxed")
        .output()
        .expect("spawn server binary");

    assert_eq!(
        output.status.code(),
        Some(4),
        "expected preflight exit code 4, {}",
        output_text(&output)
    );

    let combined = output_text(&output);
    assert!(
        combined.contains("sandbox_kernel_floor_unmet"),
        "missing sandbox_kernel_floor_unmet event in text logs: {combined}"
    );
}

#[test]
fn strict_compliant_floor_logs_ok_before_config_bootstrap() {
    if !ac84_positive_evidence_enabled() {
        eprintln!(
            "skipping AC-84 positive strict preflight evidence; set \
             OPEN_PINCERY_RUN_AC84_POSITIVE=1 in the privileged Linux sandbox-smoke job"
        );
        return;
    }

    let tmp = tempfile::tempdir().expect("temp dir");
    let output = Command::new(server_bin())
        .current_dir(tmp.path())
        .env_clear()
        .env("LOG_FORMAT", "json")
        .output()
        .expect("spawn server binary");

    assert_ne!(
        output.status.code(),
        Some(4),
        "strict compliant preflight should not fail with code 4, {}",
        output_text(&output)
    );

    let combined = output_text(&output);
    assert!(
        combined.contains("sandbox_kernel_floor_ok"),
        "missing sandbox_kernel_floor_ok before config bootstrap: {combined}"
    );
}

#[test]
fn relaxed_with_allow_unsafe_logs_warning_before_config_bootstrap() {
    if !ac84_positive_evidence_enabled() {
        eprintln!(
            "skipping AC-84 positive relaxed preflight evidence; set \
             OPEN_PINCERY_RUN_AC84_POSITIVE=1 in the privileged Linux sandbox-smoke job"
        );
        return;
    }

    let tmp = tempfile::tempdir().expect("temp dir");
    let output = Command::new(server_bin())
        .current_dir(tmp.path())
        .env_clear()
        .env("LOG_FORMAT", "json")
        .env("OPEN_PINCERY_SANDBOX_FLOOR", "relaxed")
        .env("OPEN_PINCERY_ALLOW_UNSAFE", "true")
        .output()
        .expect("spawn server binary");

    assert_ne!(
        output.status.code(),
        Some(4),
        "relaxed+allow preflight should not fail with code 4, {}",
        output_text(&output)
    );

    let combined = output_text(&output);
    assert!(
        combined.contains("sandbox_floor_relaxed"),
        "missing sandbox_floor_relaxed before config bootstrap: {combined}"
    );
}
