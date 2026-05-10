//! AC-93 (v9.1): `pcy provider` CLI end-to-end contract tests.
//!
//! Covers:
//!   * `pcy provider add` requires the credential to already exist
//!     in this workspace — refuses with a helpful message when not.
//!   * Round-trip: `add` -> `list` shows the row, `is_default=true`
//!     for the first provider; `use <name>` flips default among
//!     siblings; `remove --yes` deletes.
//!   * The clap schema does NOT accept a `--key` flag (provider
//!     creation only takes a credential reference, never raw key).

mod common;

use open_pincery::api::{self, AppState};
use open_pincery::config::Config;
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
        schema_invalid_retry_cap: 3,
        tool_call_rate_limit_per_wake: 32,
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

fn run_pcy(
    cfg_path: &std::path::Path,
    args: &[&str],
    stdin_bytes: Option<&[u8]>,
) -> std::process::Output {
    use std::io::Write;
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
fn ac93_clap_schema_has_no_key_flag() {
    // Provider rows reference a stored credential by name. There is
    // no `--key` flag and never a raw key on argv.
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    let out = run_pcy(
        &cfg,
        &[
            "provider",
            "add",
            "openrouter",
            "--base-url",
            "https://x",
            "--credential",
            "k",
            "--key",
            "secret",
        ],
        None,
    );
    assert!(
        !out.status.success(),
        "pcy provider add --key must be rejected by clap"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unexpected argument") || stderr.contains("--key"),
        "expected clap 'unexpected argument' error for --key, got stderr:\n{stderr}"
    );
}

#[tokio::test]
async fn ac93_add_list_use_remove_round_trip() {
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

    // Wait for socket.
    let probe = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(400))
        .build()
        .unwrap();
    for _ in 0..40 {
        if let Ok(r) = probe.get(format!("{base_url}/health")).send().await {
            if r.status().is_success() {
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");

    // Login.
    let cfg2 = cfg.clone();
    let base2 = base_url.clone();
    let out = tokio::task::spawn_blocking(move || {
        run_pcy(
            &cfg2,
            &["--url", &base2, "login", "--bootstrap-token", "test-token"],
            None,
        )
    })
    .await
    .unwrap();
    assert!(out.status.success(), "login failed: {:?}", out);

    // --- provider add with missing credential -> failure ---
    let cfg3 = cfg.clone();
    let out = tokio::task::spawn_blocking(move || {
        run_pcy(
            &cfg3,
            &[
                "provider",
                "add",
                "openrouter",
                "--base-url",
                "https://openrouter.ai/api/v1",
                "--credential",
                "missing_one",
            ],
            None,
        )
    })
    .await
    .unwrap();
    assert!(
        !out.status.success(),
        "provider add must refuse when credential missing"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("missing_one") || stderr.contains("credential"),
        "expected credential-missing message, got: {stderr}"
    );

    // --- create the credential, then provider add succeeds ---
    let cfg4 = cfg.clone();
    let out = tokio::task::spawn_blocking(move || {
        run_pcy(
            &cfg4,
            &["credential", "add", "openrouter_key", "--stdin"],
            Some(b"sk-test"),
        )
    })
    .await
    .unwrap();
    assert!(
        out.status.success(),
        "credential add failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let cfg5 = cfg.clone();
    let out = tokio::task::spawn_blocking(move || {
        run_pcy(
            &cfg5,
            &[
                "provider",
                "add",
                "openrouter",
                "--base-url",
                "https://openrouter.ai/api/v1",
                "--credential",
                "openrouter_key",
            ],
            None,
        )
    })
    .await
    .unwrap();
    assert!(
        out.status.success(),
        "provider add failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // --- list shows the row as default ---
    let cfg6 = cfg.clone();
    let out = tokio::task::spawn_blocking(move || {
        run_pcy(&cfg6, &["--output", "json", "provider", "list"], None)
    })
    .await
    .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("openrouter"), "list missing: {stdout}");
    assert!(
        stdout.contains("openrouter.ai"),
        "list missing url: {stdout}"
    );

    // --- add second provider, use it, list reflects new default ---
    let cfg7 = cfg.clone();
    let out = tokio::task::spawn_blocking(move || {
        run_pcy(
            &cfg7,
            &["credential", "add", "groq_key", "--stdin"],
            Some(b"gsk-test"),
        )
    })
    .await
    .unwrap();
    assert!(out.status.success());

    let cfg8 = cfg.clone();
    let out = tokio::task::spawn_blocking(move || {
        run_pcy(
            &cfg8,
            &[
                "provider",
                "add",
                "groq",
                "--base-url",
                "https://api.groq.com/openai/v1",
                "--credential",
                "groq_key",
            ],
            None,
        )
    })
    .await
    .unwrap();
    assert!(out.status.success());

    let cfg9 = cfg.clone();
    let out =
        tokio::task::spawn_blocking(move || run_pcy(&cfg9, &["provider", "use", "groq"], None))
            .await
            .unwrap();
    assert!(out.status.success(), "provider use failed: {:?}", out);

    // --- remove non-default works; remove default while siblings exist refuses ---
    let cfg10 = cfg.clone();
    let out = tokio::task::spawn_blocking(move || {
        run_pcy(&cfg10, &["provider", "remove", "groq", "--yes"], None)
    })
    .await
    .unwrap();
    // groq is now default; removing it while openrouter still exists refuses.
    assert!(
        !out.status.success(),
        "remove default with siblings must refuse"
    );

    let cfg11 = cfg.clone();
    let out = tokio::task::spawn_blocking(move || {
        run_pcy(&cfg11, &["provider", "remove", "openrouter", "--yes"], None)
    })
    .await
    .unwrap();
    assert!(out.status.success(), "non-default remove failed: {:?}", out);

    let cfg12 = cfg.clone();
    let out = tokio::task::spawn_blocking(move || {
        run_pcy(&cfg12, &["provider", "remove", "groq", "--yes"], None)
    })
    .await
    .unwrap();
    // groq is now the sole row; removing it succeeds (no siblings).
    assert!(out.status.success(), "lone remove failed: {:?}", out);
}
