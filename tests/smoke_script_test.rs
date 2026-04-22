//! AC-30: smoke scripts exist and bash path can execute end-to-end when
//! DOCKER_SMOKE=1 is set in the environment.

use std::process::Command;

#[test]
fn ac_30_smoke_scripts_exist_and_cover_required_steps() {
    let sh = std::fs::read_to_string("scripts/smoke.sh").expect("scripts/smoke.sh must exist");
    let ps1 = std::fs::read_to_string("scripts/smoke.ps1").expect("scripts/smoke.ps1 must exist");

    for needle in [
        "CARGO_TARGET_DIR",
        "docker compose up -d --wait",
        "/ready",
        "login --bootstrap-token",
        "agent create",
        "message",
        "events",
        "message_received",
    ] {
        assert!(
            sh.contains(needle),
            "scripts/smoke.sh missing required step: {needle}"
        );
        assert!(
            ps1.contains(needle),
            "scripts/smoke.ps1 missing required step: {needle}"
        );
    }
}

#[test]
fn ac_30_bash_smoke_executes_when_enabled() {
    if std::env::var("DOCKER_SMOKE").ok().as_deref() != Some("1") {
        eprintln!("SKIP: set DOCKER_SMOKE=1 to run scripts/smoke.sh end-to-end");
        return;
    }

    let out = Command::new("bash")
        .arg("scripts/smoke.sh")
        .output()
        .expect("bash must be available to run scripts/smoke.sh");

    assert!(
        out.status.success(),
        "scripts/smoke.sh failed\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Smoke OK"),
        "smoke success marker missing from output"
    );
}
