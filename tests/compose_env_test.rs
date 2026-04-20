//! AC-28 / AC-32: docker-compose.yml contract.
//!
//! Static guards that run without Docker:
//!   - No hardcoded secret literal (`changeme`, `change-me`, etc.) for the
//!     bootstrap token.
//!   - Every runtime-relevant env var is forwarded via ${VAR} interpolation.
//!   - Required secrets use `:?` fail-fast; optional vars use `:-default`.
//!   - Default port binding is loopback-only (127.0.0.1).
//!
//! The live `docker compose config` check is gated behind `COMPOSE_AVAILABLE=1`
//! so CI legs without Docker still exercise the static guards.

use std::process::Command;

const COMPOSE: &str = "docker-compose.yml";

fn compose_contents() -> String {
    std::fs::read_to_string(COMPOSE).expect("docker-compose.yml must exist at repo root")
}

#[test]
fn ac_28_no_hardcoded_bootstrap_token_literal() {
    let yaml = compose_contents();
    for bad in ["changeme", "change-me", "change_me", "CHANGEME"] {
        assert!(
            !yaml.contains(bad),
            "docker-compose.yml must not contain literal `{bad}` (AC-28/AC-32)"
        );
    }
    // The bootstrap token must be sourced via ${VAR:?...} from the operator env.
    assert!(
        yaml.contains("OPEN_PINCERY_BOOTSTRAP_TOKEN: ${OPEN_PINCERY_BOOTSTRAP_TOKEN:?"),
        "docker-compose.yml must forward OPEN_PINCERY_BOOTSTRAP_TOKEN via ${{VAR:?...}} (AC-28)"
    );
}

#[test]
fn ac_28_required_secrets_fail_fast() {
    let yaml = compose_contents();
    for required in [
        "OPEN_PINCERY_BOOTSTRAP_TOKEN",
        "LLM_API_BASE_URL",
        "LLM_API_KEY",
    ] {
        let needle = format!("{required}: ${{{required}:?");
        assert!(
            yaml.contains(&needle),
            "{required} must use ${{VAR:?message}} fail-fast interpolation (AC-28)"
        );
    }
}

#[test]
fn ac_28_optional_vars_forwarded_with_defaults() {
    let yaml = compose_contents();
    // Every optional var the binary reads must reach the container via
    // ${VAR:-default} interpolation so operators can override without editing
    // the compose file.
    for (var, _default_hint) in [
        ("OPEN_PINCERY_HOST", "0.0.0.0"),
        ("OPEN_PINCERY_PORT", "8080"),
        ("LLM_MODEL", "anthropic"),
        ("LLM_MAINTENANCE_MODEL", "anthropic"),
        ("LLM_PRICE_INPUT_PER_MTOK", "3.0"),
        ("LLM_PRICE_OUTPUT_PER_MTOK", "15.0"),
        ("LLM_MAINTENANCE_PRICE_INPUT_PER_MTOK", "3.0"),
        ("LLM_MAINTENANCE_PRICE_OUTPUT_PER_MTOK", "15.0"),
        ("LOG_FORMAT", ""),
        ("METRICS_ADDR", ""),
        ("RUST_LOG", "open_pincery"),
        ("MAX_PROMPT_CHARS", "100000"),
        ("ITERATION_CAP", "50"),
        ("STALE_WAKE_HOURS", "2"),
        ("WAKE_SUMMARY_LIMIT", "20"),
        ("EVENT_WINDOW_LIMIT", "200"),
    ] {
        let needle = format!("${{{var}:-");
        assert!(
            yaml.contains(&needle),
            "{var} must be forwarded via ${{VAR:-default}} interpolation (AC-28)"
        );
    }
}

#[test]
fn ac_32_app_binds_loopback_only_by_default() {
    let yaml = compose_contents();
    // The app's host-side port mapping must be 127.0.0.1:8080:8080 — never
    // a bare "8080:8080" or "0.0.0.0:...".
    assert!(
        yaml.contains("\"127.0.0.1:8080:8080\""),
        "docker-compose.yml must bind the app port to 127.0.0.1 only (AC-32)"
    );
    assert!(
        !yaml.contains("- \"8080:8080\""),
        "docker-compose.yml must not expose app on 0.0.0.0 by default (AC-32)"
    );
}

#[test]
fn ac_32_db_binds_loopback_only_by_default() {
    let yaml = compose_contents();
    assert!(
        yaml.contains("\"127.0.0.1:5432:5432\""),
        "docker-compose.yml must bind the db port to 127.0.0.1 only (AC-32)"
    );
    assert!(
        !yaml.contains("- \"5432:5432\""),
        "docker-compose.yml must not expose db on 0.0.0.0 by default (AC-32)"
    );
}

#[test]
fn ac_28_compose_config_resolves_with_env_fixture() {
    if std::env::var("COMPOSE_AVAILABLE").ok().as_deref() != Some("1") {
        eprintln!("SKIP: set COMPOSE_AVAILABLE=1 to run live `docker compose config` check");
        return;
    }
    let out = Command::new("docker")
        .args(["compose", "config"])
        .env("OPEN_PINCERY_BOOTSTRAP_TOKEN", "fixture-token-12345")
        .env("LLM_API_BASE_URL", "https://example.invalid/v1")
        .env("LLM_API_KEY", "fixture-key")
        .env("OPEN_PINCERY_HOST", "0.0.0.0")
        .env("OPEN_PINCERY_PORT", "8080")
        .output()
        .expect("docker compose must be invokable");
    assert!(
        out.status.success(),
        "docker compose config must succeed with required vars set: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let rendered = String::from_utf8_lossy(&out.stdout);
    assert!(
        rendered.contains("fixture-token-12345"),
        "rendered compose must contain the operator-supplied bootstrap token"
    );
    assert!(
        rendered.contains("OPEN_PINCERY_HOST: 0.0.0.0"),
        "rendered compose must contain OPEN_PINCERY_HOST from interpolation"
    );
    assert!(
        rendered.contains("OPEN_PINCERY_PORT: \"8080\"")
            || rendered.contains("OPEN_PINCERY_PORT: '8080'"),
        "rendered compose must contain OPEN_PINCERY_PORT from interpolation"
    );
    assert!(
        !rendered.contains("changeme"),
        "rendered compose must not contain `changeme` literal"
    );
}

#[test]
fn ac_32_compose_config_fails_fast_without_required_secrets() {
    if std::env::var("COMPOSE_AVAILABLE").ok().as_deref() != Some("1") {
        eprintln!("SKIP: set COMPOSE_AVAILABLE=1 to run live fail-fast check");
        return;
    }

    // Compose auto-loads ./.env from CWD unless --env-file is supplied.
    // Force a truly scrubbed environment by passing an empty env-file.
    let mut empty_env = std::env::temp_dir();
    empty_env.push("open_pincery_empty_compose_env_test.env");
    std::fs::write(&empty_env, "\n").expect("must write empty compose env file");

    let out = Command::new("docker")
        .args([
            "compose",
            "--env-file",
            empty_env.to_str().expect("temp path must be utf-8"),
            "config",
        ])
        .env_remove("OPEN_PINCERY_BOOTSTRAP_TOKEN")
        .env_remove("LLM_API_KEY")
        .env_remove("LLM_API_BASE_URL")
        .output()
        .expect("docker compose must be invokable");

    let _ = std::fs::remove_file(&empty_env);

    assert!(
        !out.status.success(),
        "docker compose config must fail when required secrets are unset (AC-32)"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("OPEN_PINCERY_BOOTSTRAP_TOKEN")
            || stderr.contains("LLM_API_KEY")
            || stderr.contains("LLM_API_BASE_URL"),
        "fail-fast error must name the missing variable (AC-32): stderr={stderr}"
    );
}
