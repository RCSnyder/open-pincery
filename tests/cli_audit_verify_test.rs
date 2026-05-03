//! AC-78 G3c/T-AC78-6: end-to-end CLI exit-code coverage for
//! `pcy audit verify`.
//!
//! These tests spin up the in-process API server, drive it through the
//! `pcy` binary, and assert exit-code semantics that backed the
//! readiness `T-AC78-6` truth: clean chain -> exit 0; tampered chain
//! -> `EXIT_CODE_CHAIN_BROKEN = 2`.

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
        stale_wake_hours: 2,
        wake_summary_limit: 20,
        event_window_limit: 200,
        vault_key_b64: common::TEST_VAULT_KEY_B64.into(),
        sandbox: open_pincery::config::ResolvedSandboxMode::default(),
    }
}

async fn run_pcy(
    bin: String,
    cfg_path: std::path::PathBuf,
    args: Vec<String>,
) -> std::process::Output {
    tokio::task::spawn_blocking(move || {
        std::process::Command::new(bin)
            .env("PCY_CONFIG_PATH", cfg_path)
            .args(args)
            .output()
            .expect("pcy command executed")
    })
    .await
    .expect("spawn_blocking run_pcy")
}

async fn boot_server_and_login(
    pool: sqlx::PgPool,
) -> (
    String,
    std::path::PathBuf,
    tempfile::TempDir,
    String,
    String,
    tokio::task::JoinHandle<()>,
) {
    let state = AppState::new(pool, test_config());
    state.listener_alive.store(true, Ordering::Relaxed);
    state.stale_alive.store(true, Ordering::Relaxed);
    let app = api::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");
    let server = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await;
    });

    // Wait for /health to come up.
    let probe = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(400))
        .build()
        .unwrap();
    for _ in 0..40 {
        if let Ok(resp) = probe.get(format!("{base_url}/health")).send().await {
            if resp.status().is_success() {
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    let tmp = tempfile::tempdir().unwrap();
    let cfg_path = tmp.path().join("config.toml");
    let pcy_bin = std::env::var("CARGO_BIN_EXE_pcy").expect("pcy binary path set by cargo");

    // login (writes session into config file at cfg_path)
    let out = run_pcy(
        pcy_bin.clone(),
        cfg_path.clone(),
        vec![
            "--url".into(),
            base_url.clone(),
            "login".into(),
            "--bootstrap-token".into(),
            "test-token".into(),
        ],
    )
    .await;
    assert!(
        out.status.success(),
        "login failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    (pcy_bin, cfg_path, tmp, base_url, "ignored".into(), server)
}

#[tokio::test]
async fn pcy_audit_verify_exits_zero_on_clean_chain() {
    let pool = common::test_pool().await;
    let (pcy_bin, cfg_path, _tmp, _base_url, _ignored, server) =
        boot_server_and_login(pool.clone()).await;

    // Create one agent + send a message so the chain has > 1 entry.
    let out = run_pcy(
        pcy_bin.clone(),
        cfg_path.clone(),
        vec!["agent".into(), "create".into(), "audit-clean".into()],
    )
    .await;
    assert!(out.status.success(), "agent create failed");
    let create_json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let agent_id = create_json["id"].as_str().unwrap().to_string();
    let out = run_pcy(
        pcy_bin.clone(),
        cfg_path.clone(),
        vec!["message".into(), agent_id, "hi".into()],
    )
    .await;
    assert!(out.status.success(), "message failed");

    let out = run_pcy(pcy_bin, cfg_path, vec!["audit".into(), "verify".into()]).await;
    assert_eq!(
        out.status.code(),
        Some(0),
        "clean chain must exit 0 (T-AC78-6); stderr=\n{}\nstdout=\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["all_verified"], true);

    server.abort();
}

#[tokio::test]
async fn pcy_audit_verify_exits_nonzero_on_break() {
    let pool = common::test_pool().await;
    let (pcy_bin, cfg_path, _tmp, _base_url, _ignored, server) =
        boot_server_and_login(pool.clone()).await;

    // Create agent + 2 messages.
    let out = run_pcy(
        pcy_bin.clone(),
        cfg_path.clone(),
        vec!["agent".into(), "create".into(), "audit-tamper".into()],
    )
    .await;
    assert!(out.status.success());
    let create_json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let agent_id_str = create_json["id"].as_str().unwrap().to_string();
    let agent_uuid: uuid::Uuid = agent_id_str.parse().unwrap();
    for msg in &["one", "two"] {
        let out = run_pcy(
            pcy_bin.clone(),
            cfg_path.clone(),
            vec!["message".into(), agent_id_str.clone(), (*msg).into()],
        )
        .await;
        assert!(out.status.success());
    }

    // Direct UPDATE bypasses BEFORE INSERT trigger -> stale prev_hash
    // on the second event.
    let target: (uuid::Uuid,) = sqlx::query_as(
        "SELECT id FROM events WHERE agent_id = $1 ORDER BY created_at ASC, id ASC OFFSET 1 LIMIT 1",
    )
    .bind(agent_uuid)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE events SET content = 'evil' WHERE id = $1")
        .bind(target.0)
        .execute(&pool)
        .await
        .unwrap();

    let out = run_pcy(pcy_bin, cfg_path, vec!["audit".into(), "verify".into()]).await;
    // EXIT_CODE_CHAIN_BROKEN = 2 in src/cli/commands/audit.rs.
    assert_eq!(
        out.status.code(),
        Some(2),
        "tampered chain must exit 2 (T-AC78-6); stderr=\n{}\nstdout=\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["all_verified"], false);

    server.abort();
}
