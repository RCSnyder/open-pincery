//! AC-31: README quick-start contract stays aligned with shipped operator path.

#[test]
fn ac_31_readme_contains_required_quickstart_sections_and_steps() {
    let readme = std::fs::read_to_string("README.md").expect("README.md must exist");

    for heading in [
        "## Quick Start",
        "### Web UI (fastest path)",
        "### pcy CLI path",
        "### curl/HTTP appendix",
        "### From Signed Release Binary",
        "### Troubleshooting",
        "### Going public with HTTPS",
        "### Reset (wipe local state)",
    ] {
        assert!(
            readme.contains(heading),
            "README missing section: {heading}"
        );
    }

    // Milestone commands must match the smoke scripts.
    for step in [
        "docker compose up -d --wait",
        "curl -fsS http://localhost:8080/ready",
        "pcy login",
        "pcy agent create",
        "pcy message",
        "pcy events",
    ] {
        assert!(
            readme.contains(step),
            "README quick start missing smoke milestone command: {step}"
        );
    }

    // Troubleshooting anchors required by scope/readiness.
    for anchor in [
        "#bootstrap-401",
        "#rate-limit-429",
        "#silent-wake",
        "#already-bootstrapped",
        "#log-format-json",
        "#metrics-scrape",
        "#backup-one-liner",
    ] {
        assert!(
            readme.contains(anchor),
            "README missing troubleshooting anchor reference: {anchor}"
        );
    }

    // API table must include webhook secret rotation path.
    assert!(
        readme.contains("/api/agents/:id/webhook/rotate")
            || readme.contains("/api/agents/:id/rotate-webhook-secret"),
        "README API table must include webhook secret rotation endpoint"
    );
}
