//! AC-40 (v7): `pcy credential` CLI end-to-end contract tests.
//!
//! Covers:
//!   * The clap schema exposes `add`, `list`, and `revoke` but does NOT
//!     accept a `--value` flag (secrets must come from the prompt or
//!     stdin, never argv).
//!   * `pcy credential add NAME --stdin` round-trips: the value is
//!     read from stdin, sealed server-side, and the server response
//!     does not echo the plaintext.
//!   * `pcy credential list` prints the stored name.
//!   * `pcy credential revoke NAME --yes` revokes; a second revoke
//!     returns a non-zero exit with a "not found" style error.
//!   * A static scan of `src/` confirms exactly one
//!     `rpassword::prompt_password(` call site — the single interactive
//!     prompt in the credential command — so future commands can't
//!     accidentally introduce a second prompt without tripping this
//!     guardrail.

mod common;

use open_pincery::api::{self, AppState};
use open_pincery::config::Config;
use std::io::Write;
use std::sync::atomic::Ordering;

fn test_config() -> Config {
    Config {
        database_url: String::new(),
        host: "127.0.0.1".into(),
        port: 0,
        bootstrap_token: "test-token".into(),
        llm_api_base_url: "http://localhost:9999".into(),
        llm_api_key: "fake".into(),
        llm_model: "test".into(),
        llm_maintenance_model: "test".into(),
        max_prompt_chars: 100000,
        iteration_cap: 50,
        stale_wake_hours: 2,
        wake_summary_limit: 20,
        event_window_limit: 200,
        vault_key_b64: common::TEST_VAULT_KEY_B64.into(),
        sandbox: open_pincery::config::ResolvedSandboxMode::default(),
    }
}

fn pcy_bin() -> String {
    std::env::var("CARGO_BIN_EXE_pcy").expect("pcy binary path set by cargo")
}

fn run_pcy_with_stdin(
    cfg_path: &std::path::Path,
    args: &[&str],
    stdin_bytes: Option<&[u8]>,
) -> std::process::Output {
    let mut cmd = std::process::Command::new(pcy_bin());
    cmd.env("PCY_CONFIG_PATH", cfg_path).args(args);
    if stdin_bytes.is_some() {
        cmd.stdin(std::process::Stdio::piped());
    }
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().expect("spawn pcy");
    if let Some(bytes) = stdin_bytes {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(bytes).expect("write stdin");
        }
    }
    child.wait_with_output().expect("pcy wait")
}

#[test]
fn ac40_clap_schema_rejects_value_flag() {
    // This is the core security contract: there must be no way to
    // pass a secret value on the command line. We invoke the binary
    // itself so the assertion is on the shipped schema, not just the
    // in-crate `Cli` struct (which is private).
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");

    let out = run_pcy_with_stdin(&cfg, &["credential", "add", "foo", "--value", "bar"], None);
    assert!(
        !out.status.success(),
        "pcy credential add --value must be rejected by clap; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unexpected argument") || stderr.contains("--value"),
        "expected clap 'unexpected argument' error for --value, got stderr:\n{stderr}"
    );
}

#[test]
fn ac40_exactly_one_rpassword_prompt_in_src() {
    // The credential CLI is the sole place a user ever types a
    // credential value. If a second prompt site appears, this test
    // flags it so reviewers can confirm the new site is also safe
    // (e.g. zeroizes, does not log, etc.).
    let mut count = 0;
    let mut sites: Vec<String> = Vec::new();
    let src_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in walkdir::WalkDir::new(&src_root)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(path).unwrap();
        for (lineno, line) in text.lines().enumerate() {
            if line.contains("rpassword::prompt_password(") {
                count += 1;
                sites.push(format!(
                    "{}:{}: {}",
                    path.display(),
                    lineno + 1,
                    line.trim()
                ));
            }
        }
    }
    assert_eq!(
        count,
        1,
        "expected exactly one rpassword::prompt_password call site in src/, found {count}:\n{}",
        sites.join("\n")
    );
}

#[tokio::test]
async fn ac40_add_list_revoke_round_trip() {
    let pool = common::test_pool().await;
    let state = AppState::new(pool.clone(), test_config());
    state.listener_alive.store(true, Ordering::Relaxed);
    state.stale_alive.store(true, Ordering::Relaxed);
    let app = api::router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");
    let _server = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await;
    });

    // Wait for the socket to be ready.
    let probe = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(400))
        .build()
        .unwrap();
    let mut ready = false;
    for _ in 0..40 {
        if let Ok(r) = probe.get(format!("{base_url}/health")).send().await {
            if r.status().is_success() {
                ready = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(ready, "server did not become reachable");

    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");

    // Log in (blocks until the pcy binary writes cfg + session token).
    let out = tokio::task::spawn_blocking({
        let cfg = cfg.clone();
        let base = base_url.clone();
        move || {
            run_pcy_with_stdin(
                &cfg,
                &["--url", &base, "login", "--bootstrap-token", "test-token"],
                None,
            )
        }
    })
    .await
    .unwrap();
    assert!(
        out.status.success(),
        "login failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    // --- Add via stdin ---
    let secret = b"sk_test_stdin_super_secret";
    let cfg2 = cfg.clone();
    let out = tokio::task::spawn_blocking(move || {
        run_pcy_with_stdin(
            &cfg2,
            &["credential", "add", "stripe_live", "--stdin"],
            Some(secret),
        )
    })
    .await
    .unwrap();
    assert!(
        out.status.success(),
        "credential add failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Secret must never appear in stdout / stderr (CLI prints the
    // server's sanitised summary).
    assert!(
        !stdout.contains("sk_test_stdin_super_secret"),
        "secret leaked into stdout: {stdout}"
    );
    assert!(stdout.contains("stripe_live"));

    // --- List ---
    let cfg3 = cfg.clone();
    let out = tokio::task::spawn_blocking(move || {
        run_pcy_with_stdin(&cfg3, &["credential", "list"], None)
    })
    .await
    .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("stripe_live"),
        "list missing name: {stdout}"
    );
    assert!(
        !stdout.contains("sk_test_stdin_super_secret"),
        "secret leaked into list output"
    );

    // --- Revoke without --yes must refuse ---
    let cfg4 = cfg.clone();
    let out = tokio::task::spawn_blocking(move || {
        run_pcy_with_stdin(&cfg4, &["credential", "revoke", "stripe_live"], None)
    })
    .await
    .unwrap();
    assert!(!out.status.success(), "revoke without --yes must fail");

    // --- Revoke with --yes ---
    let cfg5 = cfg.clone();
    let out = tokio::task::spawn_blocking(move || {
        run_pcy_with_stdin(
            &cfg5,
            &["credential", "revoke", "stripe_live", "--yes"],
            None,
        )
    })
    .await
    .unwrap();
    assert!(
        out.status.success(),
        "revoke --yes failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    // --- Second revoke -> not found ---
    let cfg6 = cfg.clone();
    let out = tokio::task::spawn_blocking(move || {
        run_pcy_with_stdin(
            &cfg6,
            &["credential", "revoke", "stripe_live", "--yes"],
            None,
        )
    })
    .await
    .unwrap();
    assert!(!out.status.success(), "second revoke should fail");

    // --- After revoke, list is empty ---
    let cfg7 = cfg.clone();
    let out = tokio::task::spawn_blocking(move || {
        run_pcy_with_stdin(&cfg7, &["credential", "list"], None)
    })
    .await
    .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("stripe_live"),
        "revoked credential still listed: {stdout}"
    );
}
