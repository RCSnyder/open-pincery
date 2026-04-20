//! AC-33: Caddy TLS overlay exists and is structurally valid.

use std::process::Command;

#[test]
fn ac_33_overlay_files_exist_and_contain_reverse_proxy_contract() {
    let overlay = std::fs::read_to_string("docker-compose.caddy.yml")
        .expect("docker-compose.caddy.yml must exist");
    let caddyfile =
        std::fs::read_to_string("Caddyfile.example").expect("Caddyfile.example must exist");

    assert!(
        overlay.contains("caddy:"),
        "overlay must define caddy service"
    );
    assert!(overlay.contains("80:80"), "overlay must publish port 80");
    assert!(overlay.contains("443:443"), "overlay must publish port 443");
    assert!(
        overlay.contains("Caddyfile") || overlay.contains("Caddyfile.example"),
        "overlay must mount a Caddy config"
    );

    assert!(
        caddyfile.contains("reverse_proxy app:8080"),
        "Caddyfile.example must proxy to app:8080"
    );
    assert!(
        caddyfile.contains("example.your-domain.com"),
        "Caddyfile.example must include an operator-editable host"
    );
}

#[test]
fn ac_33_compose_overlay_renders_when_enabled() {
    if std::env::var("COMPOSE_AVAILABLE").ok().as_deref() != Some("1") {
        eprintln!("SKIP: set COMPOSE_AVAILABLE=1 to run docker compose overlay render check");
        return;
    }

    let out = Command::new("docker")
        .args([
            "compose",
            "-f",
            "docker-compose.yml",
            "-f",
            "docker-compose.caddy.yml",
            "config",
        ])
        .env("OPEN_PINCERY_BOOTSTRAP_TOKEN", "fixture-token-12345")
        .env("LLM_API_BASE_URL", "https://example.invalid/v1")
        .env("LLM_API_KEY", "fixture-key")
        .output()
        .expect("docker compose must be invokable");

    assert!(
        out.status.success(),
        "docker compose overlay config must succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let rendered = String::from_utf8_lossy(&out.stdout);
    assert!(
        rendered.contains("caddy:"),
        "rendered overlay must include caddy service"
    );
}

#[test]
fn ac_33_caddyfile_validates_when_caddy_is_available() {
    let has_caddy = Command::new("caddy")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !has_caddy {
        eprintln!("SKIP: caddy binary not found; structural checks covered by other assertions");
        return;
    }

    let out = Command::new("caddy")
        .args(["validate", "--config", "Caddyfile.example"])
        .output()
        .expect("caddy must be invokable");

    assert!(
        out.status.success(),
        "caddy validate failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
