//! AC-22: Dockerfile declares a non-root runtime user.
//!
//! This is a static guard test that runs without Docker available. The full
//! end-to-end proof (running container exec'd returns UID 10001) is covered by
//! `tests/docker_nonroot.sh` which is gated on a working Docker daemon.

#[test]
fn ac_22_dockerfile_declares_nonroot_user() {
    let dockerfile = std::fs::read_to_string("Dockerfile").expect("Dockerfile exists at repo root");

    // Runtime stage must create a pcy user with UID 10001.
    assert!(
        dockerfile.contains("--uid 10001"),
        "Dockerfile must create user with UID 10001 (AC-22 T-24)"
    );
    assert!(
        dockerfile.contains("--gid 10001"),
        "Dockerfile must create group with GID 10001 (AC-22 T-24)"
    );
    assert!(
        dockerfile.contains("useradd --system --uid 10001 --gid pcy"),
        "Dockerfile must use a system useradd for `pcy` (AC-22 T-24)"
    );

    // The final USER directive must be pcy.
    let last_user = dockerfile
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with("USER "))
        .expect("Dockerfile must contain a USER directive");
    assert_eq!(
        last_user.trim(),
        "USER pcy",
        "Final USER directive must be `USER pcy` (AC-22 T-24)"
    );

    // Copies in the runtime stage must chown to pcy so files aren't owned by root.
    // Count COPY --from=builder lines and ensure each has --chown=pcy:pcy.
    let runtime_copy_lines: Vec<&str> = dockerfile
        .lines()
        .filter(|l| l.trim_start().starts_with("COPY --from=builder"))
        .collect();
    assert!(
        !runtime_copy_lines.is_empty(),
        "Expected COPY --from=builder lines in runtime stage"
    );
    for copy in &runtime_copy_lines {
        assert!(
            copy.contains("--chown=pcy:pcy"),
            "Runtime COPY must use --chown=pcy:pcy to avoid root-owned files (AC-22 T-24): {copy}"
        );
    }

    // HEALTHCHECK must still bind to port 8080 (unprivileged, works as non-root).
    assert!(
        dockerfile.contains("http://localhost:8080/health"),
        "HEALTHCHECK against /health on 8080 must remain (AC-22 T-24)"
    );
}
