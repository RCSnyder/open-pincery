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
        stale_wake_hours: 2,
        wake_summary_limit: 20,
        event_window_limit: 200,
    }
}

#[tokio::test]
async fn test_pcy_cli_e2e_core_flow() {
    let pool = common::test_pool().await;

    let state = AppState::new(pool.clone(), test_config());
    state.listener_alive.store(true, Ordering::Relaxed);
    state.stale_alive.store(true, Ordering::Relaxed);

    let app = api::router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    let server = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await;
    });

    // Wait for server socket/router to be ready before starting CLI calls.
    let probe = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(400))
        .build()
        .unwrap();
    let mut ready = false;
    for _ in 0..40 {
        if let Ok(resp) = probe.get(format!("{}/health", base_url)).send().await {
            if resp.status().is_success() {
                ready = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(ready, "in-process API server never became reachable");

    let tmp = tempfile::tempdir().unwrap();
    let cfg_path = tmp.path().join("config.toml");
    let pcy_bin = std::env::var("CARGO_BIN_EXE_pcy").expect("pcy binary path set by cargo");

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

    fn assert_ok(step: &str, out: &std::process::Output) {
        assert!(
            out.status.success(),
            "step `{}` failed\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}",
            step,
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // Child-process preflight: ensure the pcy binary itself can reach the server.
    let out = run_pcy(
        pcy_bin.clone(),
        cfg_path.clone(),
        vec!["--url".into(), base_url.clone(), "status".into()],
    )
    .await;
    assert_ok("preflight status", &out);

    // bootstrap
    let out = run_pcy(
        pcy_bin.clone(),
        cfg_path.clone(),
        vec![
            "--url".into(),
            base_url.clone(),
            "bootstrap".into(),
            "--bootstrap-token".into(),
            "test-token".into(),
        ],
    )
    .await;
    assert_ok("bootstrap", &out);

    // create agent
    let out = run_pcy(
        pcy_bin.clone(),
        cfg_path.clone(),
        vec!["agent".into(), "create".into(), "cli-agent".into()],
    )
    .await;
    assert_ok("agent create", &out);
    let create_json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let agent_id = create_json["id"].as_str().unwrap().to_string();

    // list agents
    let out = run_pcy(
        pcy_bin.clone(),
        cfg_path.clone(),
        vec!["agent".into(), "list".into()],
    )
    .await;
    assert_ok("agent list", &out);

    // send a message
    let out = run_pcy(
        pcy_bin.clone(),
        cfg_path.clone(),
        vec!["message".into(), agent_id.clone(), "hello from cli".into()],
    )
    .await;
    assert_ok("message", &out);

    // fetch events
    let out = run_pcy(
        pcy_bin.clone(),
        cfg_path.clone(),
        vec!["events".into(), agent_id.clone()],
    )
    .await;
    assert_ok("events", &out);
    let events_json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let first_event_id = events_json["events"][0]["id"].as_str().unwrap().to_string();

    let out = run_pcy(
        pcy_bin.clone(),
        cfg_path.clone(),
        vec!["message".into(), agent_id.clone(), "hello again".into()],
    )
    .await;
    assert_ok("message second", &out);

    let out = run_pcy(
        pcy_bin.clone(),
        cfg_path.clone(),
        vec!["message".into(), agent_id.clone(), "hello third".into()],
    )
    .await;
    assert_ok("message third", &out);

    let out = run_pcy(
        pcy_bin.clone(),
        cfg_path.clone(),
        vec![
            "events".into(),
            agent_id.clone(),
            "--since".into(),
            first_event_id,
        ],
    )
    .await;
    assert_ok("events since", &out);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(lines.len(), 2);
    let second_event: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let third_event: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(second_event["content"], "hello again");
    assert_eq!(third_event["content"], "hello third");

    // rotate secret
    let out = run_pcy(
        pcy_bin.clone(),
        cfg_path.clone(),
        vec!["agent".into(), "rotate-secret".into(), agent_id.clone()],
    )
    .await;
    assert_ok("agent rotate-secret", &out);

    // budget set/show/reset
    let out = run_pcy(
        pcy_bin.clone(),
        cfg_path.clone(),
        vec![
            "budget".into(),
            "set".into(),
            agent_id.clone(),
            "12.5".into(),
        ],
    )
    .await;
    assert_ok("budget set", &out);

    let out = run_pcy(
        pcy_bin.clone(),
        cfg_path.clone(),
        vec!["budget".into(), "show".into(), agent_id.clone()],
    )
    .await;
    assert_ok("budget show", &out);

    let out = run_pcy(
        pcy_bin.clone(),
        cfg_path.clone(),
        vec!["budget".into(), "reset".into(), agent_id.clone()],
    )
    .await;
    assert_ok("budget reset", &out);

    // status should return success when /ready is 200.
    let out = run_pcy(
        pcy_bin,
        cfg_path,
        vec!["--url".into(), base_url, "status".into()],
    )
    .await;
    assert_ok("status", &out);

    server.abort();
}
