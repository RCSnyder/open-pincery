//! Integration tests for AC-90 `pcy doctor`. Uses the in-process
//! [`Probe`] trait so the tests are deterministic and don't require a
//! live database, Docker, network, or Linux kernel.

use open_pincery::cli::commands::doctor::{
    diagnose, exit_code, render_json, render_table, CheckResult, Probe, Status,
};

/// Fully-configurable stub probe — every field corresponds to one
/// production probe method.
#[derive(Default)]
struct StubProbe {
    env_file_exists: bool,
    database_url: Option<String>,
    llm_base_url: Option<String>,
    docker_version: Option<String>,
    kernel_floor: Option<Result<String, String>>,
    db_ping: Option<Result<(), String>>,
    migration_status: Option<Result<(usize, usize), String>>,
    admin_user_count: Option<Result<u64, String>>,
    llm_probe: Option<Result<u16, String>>,
    sandbox_smoke: Option<Result<(), String>>,
}

impl Probe for StubProbe {
    fn env_file_exists(&self) -> bool {
        self.env_file_exists
    }
    fn database_url(&self) -> Option<String> {
        self.database_url.clone()
    }
    fn llm_base_url(&self) -> Option<String> {
        self.llm_base_url.clone()
    }
    fn docker_version(&self) -> Option<String> {
        self.docker_version.clone()
    }
    fn kernel_floor(&self) -> Option<Result<String, String>> {
        self.kernel_floor.clone()
    }
    fn db_ping(&self) -> Result<(), String> {
        self.db_ping
            .clone()
            .unwrap_or_else(|| Err("not configured".to_string()))
    }
    fn migration_status(&self) -> Result<(usize, usize), String> {
        self.migration_status
            .clone()
            .unwrap_or_else(|| Err("not configured".to_string()))
    }
    fn admin_user_count(&self) -> Result<u64, String> {
        self.admin_user_count
            .clone()
            .unwrap_or_else(|| Err("not configured".to_string()))
    }
    fn llm_probe(&self) -> Result<u16, String> {
        self.llm_probe
            .clone()
            .unwrap_or_else(|| Err("not configured".to_string()))
    }
    fn sandbox_smoke(&self) -> Option<Result<(), String>> {
        self.sandbox_smoke.clone()
    }
}

fn happy_probe() -> StubProbe {
    StubProbe {
        env_file_exists: true,
        database_url: Some("postgres://localhost/x".into()),
        llm_base_url: Some("https://example.com".into()),
        docker_version: Some("27.0.0".into()),
        kernel_floor: Some(Ok("landlock ABI 6, all checks passed".into())),
        db_ping: Some(Ok(())),
        migration_status: Some(Ok((30, 30))),
        admin_user_count: Some(Ok(1)),
        llm_probe: Some(Ok(200)),
        sandbox_smoke: Some(Ok(())),
    }
}

#[test]
fn diagnose_emits_eight_checks_in_fixed_order() {
    let rows = diagnose(&happy_probe());
    assert_eq!(rows.len(), 8, "AC-90 specifies 8 ordered checks");
    let names: Vec<&str> = rows.iter().map(|r| r.check.as_str()).collect();
    assert_eq!(
        names,
        vec![
            ".env file",
            "docker",
            "kernel floor",
            "database",
            "migrations",
            "bootstrap",
            "llm",
            "sandbox smoke",
        ]
    );
}

#[test]
fn happy_path_is_all_ok_and_exit_zero() {
    let rows = diagnose(&happy_probe());
    assert!(rows.iter().all(|r| r.status == Status::Ok));
    assert_eq!(exit_code(&rows, false), 0);
    assert_eq!(exit_code(&rows, true), 0);
}

#[test]
fn fail_in_db_yields_exit_one_even_without_strict() {
    let mut p = happy_probe();
    p.db_ping = Some(Err("connection refused".into()));
    let rows = diagnose(&p);
    let db = rows.iter().find(|r| r.check == "database").unwrap();
    assert_eq!(db.status, Status::Fail);
    assert!(db.detail.contains("connection refused"));
    assert!(!db.remediation.is_empty());
    assert_eq!(exit_code(&rows, false), 1);
}

#[test]
fn non_linux_kernel_floor_is_warn_but_strict_exempt() {
    let mut p = happy_probe();
    p.kernel_floor = None; // simulates macOS / Windows
    p.sandbox_smoke = None;
    let rows = diagnose(&p);
    let kf = rows.iter().find(|r| r.check == "kernel floor").unwrap();
    assert_eq!(kf.status, Status::Warn);
    assert!(
        kf.strict_exempt,
        "non-Linux kernel-floor WARN is strict-exempt per CR-v91-3"
    );
    // Non-strict and strict both 0 because the only WARNs are exempt.
    assert_eq!(exit_code(&rows, false), 0);
    assert_eq!(exit_code(&rows, true), 0);
}

#[test]
fn non_exempt_warn_under_strict_is_exit_one() {
    let mut p = happy_probe();
    p.docker_version = None; // docker WARN — not strict-exempt
    let rows = diagnose(&p);
    let d = rows.iter().find(|r| r.check == "docker").unwrap();
    assert_eq!(d.status, Status::Warn);
    assert!(!d.strict_exempt);
    assert_eq!(exit_code(&rows, false), 0);
    assert_eq!(exit_code(&rows, true), 1);
}

#[test]
fn json_output_is_valid_json_array_with_expected_keys() {
    let rows = diagnose(&happy_probe());
    let s = render_json(&rows);
    let parsed: serde_json::Value = serde_json::from_str(&s).expect("valid JSON");
    let arr = parsed.as_array().expect("top-level array");
    assert_eq!(arr.len(), 8);
    for row in arr {
        assert!(row.get("check").is_some());
        assert!(row.get("status").is_some());
        assert!(row.get("detail").is_some());
        assert!(row.get("remediation").is_some());
    }
}

#[test]
fn table_renders_every_row() {
    let rows = diagnose(&happy_probe());
    let s = render_table(&rows);
    for r in &rows {
        assert!(
            s.contains(&r.check),
            "table should contain row '{}'",
            r.check
        );
    }
    assert!(s.starts_with("STATUS"));
}

#[test]
fn partial_migrations_are_fail() {
    let mut p = happy_probe();
    p.migration_status = Some(Ok((10, 30)));
    let rows = diagnose(&p);
    let m = rows.iter().find(|r| r.check == "migrations").unwrap();
    assert_eq!(m.status, Status::Fail);
    assert_eq!(exit_code(&rows, false), 1);
}

#[test]
fn missing_admin_user_is_fail_with_login_hint() {
    let mut p = happy_probe();
    p.admin_user_count = Some(Ok(0));
    let rows = diagnose(&p);
    let b = rows.iter().find(|r| r.check == "bootstrap").unwrap();
    assert_eq!(b.status, Status::Fail);
    assert!(
        b.remediation.contains("pcy login"),
        "remediation must steer to pcy login"
    );
}

#[test]
fn check_result_serde_roundtrip_preserves_strict_exempt() {
    let row = CheckResult {
        check: "kernel floor".to_string(),
        status: Status::Warn,
        detail: "native sandbox unavailable on this OS".into(),
        remediation: "use the Linux devshell".into(),
        strict_exempt: true,
    };
    let s = serde_json::to_string(&row).unwrap();
    let back: CheckResult = serde_json::from_str(&s).unwrap();
    assert!(back.strict_exempt);
    assert_eq!(back.status, Status::Warn);
}
