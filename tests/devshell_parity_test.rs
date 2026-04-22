//! AC-75 — Cross-Platform Developer Environment.
//!
//! v9's Linux-only kernel primitives (landlock, bubblewrap, slirp4netns,
//! cgroup v2) make native Mac/Windows development impossible.  AC-75 ships
//! a pinned Docker "devshell" image and wrapper scripts so contributors on
//! any host can run the identical sandbox test suite inside a
//! reproducible Ubuntu 24.04 environment.
//!
//! This file enforces two levels of verification:
//!
//! 1. **Structural** (always runs, cross-platform):
//!    - `Dockerfile.devshell` exists and pins Ubuntu 24.04 + the sandbox
//!      toolchain required by AC-53 / AC-71 / AC-72.
//!    - `scripts/devshell.sh` + `scripts/devshell.ps1` exist and invoke
//!      the pinned image with the documented flags.
//!    - `docs/runbooks/dev_setup_macos.md` + `dev_setup_windows.md` exist
//!      and reference `devshell`.
//!    - `README.md` contains a Development section pointing at the
//!      devshell wrappers.
//!
//! 2. **Runtime parity** (opt-in, Linux hosts only):
//!    - Gated on `OPEN_PINCERY_DEVSHELL_PARITY=1` so the outer test only
//!      runs when CI explicitly requests it.  The outer run shells into
//!      the devshell container and re-executes the inner sandbox suite;
//!      both paths must agree.  Until A2a lands the inner suite does not
//!      yet exist, so this block is wired but skipped by default.

use std::path::{Path, PathBuf};

/// Repo root, resolved once from `CARGO_MANIFEST_DIR` so the test is
/// independent of the working directory at invocation time.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

#[test]
fn dockerfile_devshell_is_pinned_and_installs_sandbox_toolchain() {
    let path = repo_root().join("Dockerfile.devshell");
    assert!(
        path.exists(),
        "AC-75: Dockerfile.devshell must exist at repo root"
    );
    let body = read(&path);

    // Pinned base image so every contributor gets the same kernel-header
    // set and sandbox binaries.
    assert!(
        body.contains("ubuntu:24.04"),
        "AC-75: Dockerfile.devshell must pin ubuntu:24.04 (found:\n{body})"
    );

    // Sandbox userland required by AC-53 (Zerobox) + AC-72 (egress
    // allowlist).  We check for the package names rather than the
    // binaries so the test does not require the image to be built.
    for pkg in ["bubblewrap", "slirp4netns", "uidmap", "libseccomp-dev"] {
        assert!(
            body.contains(pkg),
            "AC-75: Dockerfile.devshell must install {pkg} (found:\n{body})"
        );
    }

    // Rust toolchain + sqlx-cli so `devshell cargo test` and migrations
    // work without extra host setup.
    assert!(
        body.contains("rustup") || body.contains("cargo"),
        "AC-75: Dockerfile.devshell must provision the Rust toolchain"
    );
    assert!(
        body.contains("sqlx-cli"),
        "AC-75: Dockerfile.devshell must install sqlx-cli"
    );
}

#[test]
fn devshell_sh_launches_pinned_image_with_required_flags() {
    let path = repo_root().join("scripts").join("devshell.sh");
    assert!(path.exists(), "AC-75: scripts/devshell.sh must exist");
    let body = read(&path);

    assert!(
        body.starts_with("#!/"),
        "AC-75: scripts/devshell.sh must start with a shebang"
    );
    assert!(
        body.contains("ghcr.io/open-pincery/devshell:v9"),
        "AC-75: scripts/devshell.sh must reference the pinned image tag"
    );
    // The sandbox binaries need cgroup v2 + privileged exec.  Without
    // these flags the inner sandbox tests would behave differently from
    // a Linux host.
    for flag in ["--privileged", "--cgroupns=host"] {
        assert!(
            body.contains(flag),
            "AC-75: scripts/devshell.sh must pass {flag}"
        );
    }
}

#[test]
fn devshell_ps1_mirrors_bash_wrapper() {
    let path = repo_root().join("scripts").join("devshell.ps1");
    assert!(
        path.exists(),
        "AC-75: scripts/devshell.ps1 must exist (Windows contributor path)"
    );
    let body = read(&path);

    assert!(
        body.contains("ghcr.io/open-pincery/devshell:v9"),
        "AC-75: scripts/devshell.ps1 must reference the pinned image tag"
    );
    for flag in ["--privileged", "--cgroupns=host"] {
        assert!(
            body.contains(flag),
            "AC-75: scripts/devshell.ps1 must pass {flag}"
        );
    }
}

#[test]
fn runbooks_for_mac_and_windows_exist() {
    let root = repo_root().join("docs").join("runbooks");
    for name in ["dev_setup_macos.md", "dev_setup_windows.md"] {
        let path = root.join(name);
        assert!(path.exists(), "AC-75: docs/runbooks/{name} must exist");
        let body = read(&path);
        assert!(
            body.to_lowercase().contains("devshell"),
            "AC-75: {name} must reference the devshell wrapper"
        );
        assert!(
            body.contains("cargo test"),
            "AC-75: {name} must walk contributor from clone to `cargo test`"
        );
    }
}

#[test]
fn readme_documents_devshell_workflow() {
    let path = repo_root().join("README.md");
    let body = read(&path);

    assert!(
        body.contains("## Development"),
        "AC-75: README.md must contain a `## Development` section"
    );
    assert!(
        body.contains("scripts/devshell"),
        "AC-75: README.md Development section must reference scripts/devshell"
    );
}

/// Runtime parity — outer Linux host shells into the devshell and
/// re-runs the inner sandbox suite.  Gated so the default `cargo test`
/// does not require Docker.  The inner suite (`tests/sandbox_escape_test.rs`)
/// lands in Slice A2a; until then this block is a soft placeholder that
/// only asserts the wrapper is invocable.
#[test]
fn devshell_parity_outer_to_inner() {
    if std::env::var("OPEN_PINCERY_DEVSHELL_PARITY")
        .ok()
        .as_deref()
        != Some("1")
    {
        eprintln!(
            "AC-75 parity check skipped: set OPEN_PINCERY_DEVSHELL_PARITY=1 on a Linux host with Docker to enable."
        );
        return;
    }

    #[cfg(not(target_os = "linux"))]
    {
        panic!("AC-75 parity check requires a Linux host; run this test inside CI's Linux runner.");
    }

    #[cfg(target_os = "linux")]
    {
        let script = repo_root().join("scripts").join("devshell.sh");
        let output = std::process::Command::new("bash")
            .arg(&script)
            .arg("--version-check")
            .output()
            .expect("AC-75: devshell.sh must be executable on a Linux host with Docker");
        assert!(
            output.status.success(),
            "AC-75: devshell.sh --version-check failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}
