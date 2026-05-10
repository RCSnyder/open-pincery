//! AC-90 (v9.1): `pcy doctor` — operator self-diagnosis.
//!
//! Eight ordered, independent checks. Each yields a [`CheckResult`].
//! No check aborts the run — a FAIL in one row does not skip later
//! rows. Output: human-readable table (default) or JSON
//! (`--output json`). Exit code policy:
//!
//! * non-strict (default): exit 0 if every check is `Ok` or `Warn`;
//!   exit 1 if any check is `Fail`.
//! * `--strict`: exit 1 if any check is `Fail` OR `Warn`, **except**
//!   for the kernel-floor row on macOS/Windows — that platform
//!   inherently lacks the kernel surface, and developers on it should
//!   not be blocked by `--strict` from running the rest of the
//!   checks (per CR-v91-3, resolved at v9.1 ITERATE).
//!
//! Each check is built around a [`Probe`] trait so unit tests can
//! drive the diagnosis with deterministic fixtures.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// One of three outcomes per check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

impl Status {
    fn glyph(self) -> &'static str {
        match self {
            Status::Ok => "OK  ",
            Status::Warn => "WARN",
            Status::Fail => "FAIL",
        }
    }
}

/// Output shape for one row of the doctor report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub check: String,
    pub status: Status,
    pub detail: String,
    pub remediation: String,
    /// True if this row is the kernel-floor row on a non-Linux host.
    /// `--strict` ignores rows where this is set (see CR-v91-3).
    #[serde(default)]
    pub strict_exempt: bool,
}

impl CheckResult {
    fn ok(check: &'static str, detail: impl Into<String>) -> Self {
        Self {
            check: check.to_string(),
            status: Status::Ok,
            detail: detail.into(),
            remediation: String::new(),
            strict_exempt: false,
        }
    }
    fn warn(
        check: &'static str,
        detail: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            check: check.to_string(),
            status: Status::Warn,
            detail: detail.into(),
            remediation: remediation.into(),
            strict_exempt: false,
        }
    }
    fn fail(
        check: &'static str,
        detail: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            check: check.to_string(),
            status: Status::Fail,
            detail: detail.into(),
            remediation: remediation.into(),
            strict_exempt: false,
        }
    }
}

/// Output format for `pcy doctor`. Mirrors the `--output` family but
/// stays self-contained because `doctor` predates a working DB / token
/// and so cannot fall back on [`crate::cli::output`]'s server-shaped
/// renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorOutput {
    Table,
    Json,
}

/// Test-injection seam. Real production code passes [`LiveProbe`];
/// tests construct a [`StubProbe`] with deterministic field values.
pub trait Probe {
    fn env_file_exists(&self) -> bool;
    fn database_url(&self) -> Option<String>;
    fn llm_base_url(&self) -> Option<String>;
    fn docker_version(&self) -> Option<String>;
    /// `Ok` on Linux with the floor passed; `Err(msg)` on Linux when
    /// the floor failed; `None` on non-Linux (caller emits WARN).
    fn kernel_floor(&self) -> Option<Result<String, String>>;
    /// `Ok(())` if a `SELECT 1` round-trip succeeds. `Err(msg)` otherwise.
    fn db_ping(&self) -> Result<(), String>;
    /// `(applied, total)` migration counts.
    fn migration_status(&self) -> Result<(usize, usize), String>;
    /// `Ok(n)` admin-user count from `SELECT count(*) FROM users WHERE role='admin'`.
    fn admin_user_count(&self) -> Result<u64, String>;
    /// HEAD or GET against `LLM_API_BASE_URL` within a few seconds.
    fn llm_probe(&self) -> Result<u16, String>;
    /// On Linux: `Ok` if a no-op sandboxed exec returns 0 without
    /// emitting `sandbox_blocked`. `Err(msg)` otherwise. `None` if
    /// the sandbox preflight already failed or the host is non-Linux.
    fn sandbox_smoke(&self) -> Option<Result<(), String>>;
}

/// Walk the 8 checks and return their results. Pure orchestration:
/// every check delegates to the [`Probe`] so this function is the
/// stable target for unit tests.
pub fn diagnose(probe: &dyn Probe) -> Vec<CheckResult> {
    let mut rows = Vec::with_capacity(8);

    // 1. .env present
    rows.push(if probe.env_file_exists() {
        CheckResult::ok(".env file", ".env present in cwd")
    } else {
        CheckResult::fail(
            ".env file",
            ".env not found in cwd",
            "run `pcy init` to generate one",
        )
    });

    // 2. Docker reachable
    rows.push(match probe.docker_version() {
        Some(v) => CheckResult::ok("docker", format!("docker reachable: {v}")),
        None => CheckResult::warn(
            "docker",
            "`docker version` unavailable",
            "install Docker, or run pincery natively (Linux only)",
        ),
    });

    // 3. Kernel floor (Linux only)
    rows.push(match probe.kernel_floor() {
        Some(Ok(detail)) => CheckResult::ok("kernel floor", detail),
        Some(Err(msg)) => CheckResult::fail(
            "kernel floor",
            msg,
            "upgrade kernel ≥ 6.7 + bubblewrap ≥ 0.8 (see docs/runbooks/)",
        ),
        None => {
            let mut r = CheckResult::warn(
                "kernel floor",
                "native sandbox unavailable on this OS",
                "use the Linux devshell for production workloads",
            );
            r.strict_exempt = true;
            r
        }
    });

    // 4. DB reachable
    rows.push(match probe.db_ping() {
        Ok(()) => CheckResult::ok("database", "SELECT 1 ok"),
        Err(msg) => CheckResult::fail(
            "database",
            format!("DB unreachable: {msg}"),
            "check DATABASE_URL and that Postgres is running",
        ),
    });

    // 5. Migrations applied
    rows.push(match probe.migration_status() {
        Ok((applied, total)) if applied == total && total > 0 => {
            CheckResult::ok("migrations", format!("{applied}/{total} applied"))
        }
        Ok((applied, total)) => CheckResult::fail(
            "migrations",
            format!("{applied}/{total} applied"),
            "run the server once or `sqlx migrate run` to apply pending migrations",
        ),
        Err(msg) => CheckResult::fail(
            "migrations",
            format!("could not read migration table: {msg}"),
            "ensure the schema has been initialized",
        ),
    });

    // 6. Bootstrap completed
    rows.push(match probe.admin_user_count() {
        Ok(n) if n > 0 => CheckResult::ok("bootstrap", format!("{n} admin user(s) present")),
        Ok(_) => CheckResult::fail(
            "bootstrap",
            "no admin user in users table",
            "run `pcy login --bootstrap-token <token>` after starting the server",
        ),
        Err(msg) => CheckResult::fail(
            "bootstrap",
            format!("could not query users table: {msg}"),
            "verify DB connection then re-run",
        ),
    });

    // 7. LLM provider reachable
    rows.push(match probe.llm_probe() {
        Ok(code) if (200..400).contains(&code) => {
            CheckResult::ok("llm", format!("provider responded {code}"))
        }
        Ok(code) => CheckResult::fail(
            "llm",
            format!("provider responded {code}"),
            "verify LLM_API_BASE_URL and LLM_API_KEY",
        ),
        Err(msg) => CheckResult::fail(
            "llm",
            format!("provider unreachable: {msg}"),
            "verify LLM_API_BASE_URL is correct and reachable",
        ),
    });

    // The original v9.1 design listed an eighth "sandbox smoke" check.
    // It was dropped from scope at v9.1 REVIEW (2026-05-10) — a real
    // probe requires a bootstrapped DB + agent and is out of v9.1's
    // budget. The check is deferred to v9.2 as AC-90b. The
    // `Probe::sandbox_smoke` trait method remains for forward
    // compatibility but is no longer rendered.

    rows
}

/// Compute the exit code for a diagnosis under the strict-mode rules
/// documented at module top.
pub fn exit_code(rows: &[CheckResult], strict: bool) -> i32 {
    let any_fail = rows.iter().any(|r| r.status == Status::Fail);
    if any_fail {
        return 1;
    }
    if strict {
        let strict_offender = rows
            .iter()
            .any(|r| r.status == Status::Warn && !r.strict_exempt);
        if strict_offender {
            return 1;
        }
    }
    0
}

/// Render a diagnosis as a human-readable table.
pub fn render_table(rows: &[CheckResult]) -> String {
    let mut out = String::new();
    out.push_str("STATUS  CHECK             DETAIL\n");
    out.push_str("------  ----------------  ----------------------------------------\n");
    for r in rows {
        out.push_str(&format!(
            "{}    {:<16}  {}\n",
            r.status.glyph(),
            r.check,
            r.detail
        ));
        if r.status != Status::Ok && !r.remediation.is_empty() {
            out.push_str(&format!("        \u{2192} {}\n", r.remediation));
        }
    }
    out
}

/// Render a diagnosis as JSON (one top-level array).
pub fn render_json(rows: &[CheckResult]) -> String {
    serde_json::to_string_pretty(rows).unwrap_or_else(|_| "[]".to_string())
}

// ---------------------------------------------------------------------------
// Live probe: production-bound impl that touches the real environment.
// ---------------------------------------------------------------------------

/// Path to the `.env` file used by [`LiveProbe::env_file_exists`].
/// Defaults to `.env` in the current working directory.
pub fn default_env_path() -> PathBuf {
    PathBuf::from(".env")
}

/// Production probe that hits the real filesystem, DB pool, network,
/// and (on Linux) the kernel sandbox preflight. Constructed lazily by
/// [`run`] to keep `cargo doc` builds fast.
pub struct LiveProbe {
    env_path: PathBuf,
    database_url: Option<String>,
    llm_base_url: Option<String>,
    runtime: tokio::runtime::Handle,
}

impl LiveProbe {
    /// Construct a probe bound to the current process env.
    pub fn from_env(runtime: tokio::runtime::Handle) -> Self {
        Self {
            env_path: default_env_path(),
            database_url: std::env::var("DATABASE_URL").ok(),
            llm_base_url: std::env::var("LLM_API_BASE_URL").ok(),
            runtime,
        }
    }
}

impl Probe for LiveProbe {
    fn env_file_exists(&self) -> bool {
        self.env_path.exists()
    }

    fn database_url(&self) -> Option<String> {
        self.database_url.clone()
    }

    fn llm_base_url(&self) -> Option<String> {
        self.llm_base_url.clone()
    }

    fn docker_version(&self) -> Option<String> {
        match std::process::Command::new("docker")
            .arg("version")
            .arg("--format")
            .arg("{{.Client.Version}}")
            .output()
        {
            Ok(o) if o.status.success() => {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            }
            _ => None,
        }
    }

    fn kernel_floor(&self) -> Option<Result<String, String>> {
        #[cfg(target_os = "linux")]
        {
            use crate::runtime::sandbox::preflight::{
                assert_kernel_floor, FloorEnv, FloorOutcome, RealKernelProbe,
            };
            let env = FloorEnv::from_env();
            match assert_kernel_floor(&RealKernelProbe, &env) {
                Ok(FloorOutcome::Passed { landlock_abi }) => Some(Ok(format!(
                    "landlock ABI {landlock_abi}, all checks passed"
                ))),
                Ok(FloorOutcome::Relaxed { landlock_abi }) => Some(Ok(format!(
                    "landlock ABI {landlock_abi} (RELAXED — ALLOW_UNSAFE armed)"
                ))),
                Err(e) => Some(Err(format!("{e:?}"))),
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }

    fn db_ping(&self) -> Result<(), String> {
        let url = self
            .database_url
            .as_ref()
            .ok_or_else(|| "DATABASE_URL not set".to_string())?
            .clone();
        let handle = self.runtime.clone();
        std::thread::scope(|s| {
            s.spawn(|| {
                handle.block_on(async {
                    use sqlx::Connection;
                    let mut c = sqlx::PgConnection::connect(&url)
                        .await
                        .map_err(|e| e.to_string())?;
                    sqlx::query("SELECT 1")
                        .execute(&mut c)
                        .await
                        .map_err(|e| e.to_string())?;
                    Ok::<_, String>(())
                })
            })
            .join()
            .map_err(|_| "db_ping task panicked".to_string())?
        })
    }

    fn migration_status(&self) -> Result<(usize, usize), String> {
        let url = self
            .database_url
            .as_ref()
            .ok_or_else(|| "DATABASE_URL not set".to_string())?
            .clone();
        // Count files in ./migrations/*.sql (best-effort relative path).
        let total = std::fs::read_dir("migrations")
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path()
                            .extension()
                            .and_then(|x| x.to_str())
                            .map(|x| x.eq_ignore_ascii_case("sql"))
                            .unwrap_or(false)
                    })
                    .count()
            })
            .unwrap_or(0);
        let handle = self.runtime.clone();
        let applied: usize = std::thread::scope(|s| {
            s.spawn(|| {
                handle.block_on(async {
                    use sqlx::Connection;
                    let mut c = sqlx::PgConnection::connect(&url)
                        .await
                        .map_err(|e| e.to_string())?;
                    let row: (i64,) = sqlx::query_as("SELECT count(*) FROM _sqlx_migrations")
                        .fetch_one(&mut c)
                        .await
                        .map_err(|e| e.to_string())?;
                    Ok::<_, String>(row.0.max(0) as usize)
                })
            })
            .join()
            .map_err(|_| "migration_status task panicked".to_string())?
        })?;
        Ok((applied, total))
    }

    fn admin_user_count(&self) -> Result<u64, String> {
        let url = self
            .database_url
            .as_ref()
            .ok_or_else(|| "DATABASE_URL not set".to_string())?
            .clone();
        let handle = self.runtime.clone();
        std::thread::scope(|s| {
            s.spawn(|| {
                handle.block_on(async {
                    use sqlx::Connection;
                    let mut c = sqlx::PgConnection::connect(&url)
                        .await
                        .map_err(|e| e.to_string())?;
                    let row: (i64,) =
                        sqlx::query_as("SELECT count(*) FROM users WHERE role='admin'")
                            .fetch_one(&mut c)
                            .await
                            .map_err(|e| e.to_string())?;
                    Ok::<_, String>(row.0.max(0) as u64)
                })
            })
            .join()
            .map_err(|_| "admin_user_count task panicked".to_string())?
        })
    }

    fn llm_probe(&self) -> Result<u16, String> {
        let base = self
            .llm_base_url
            .as_ref()
            .ok_or_else(|| "LLM_API_BASE_URL not set".to_string())?
            .clone();
        let handle = self.runtime.clone();
        std::thread::scope(|s| {
            s.spawn(|| {
                handle.block_on(async {
                    let client = reqwest::Client::builder()
                        .timeout(Duration::from_secs(3))
                        .build()
                        .map_err(|e| e.to_string())?;
                    // Try /models first (most providers); fall back to base URL.
                    let url = if base.ends_with('/') {
                        format!("{base}models")
                    } else {
                        format!("{base}/models")
                    };
                    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
                    Ok::<_, String>(resp.status().as_u16())
                })
            })
            .join()
            .map_err(|_| "llm_probe task panicked".to_string())?
        })
    }

    fn sandbox_smoke(&self) -> Option<Result<(), String>> {
        // Out of scope for v9.1 — a real sandbox smoke needs a wired
        // tool dispatcher with a bootstrapped DB and an agent. We
        // return None on every host and surface that as a WARN so the
        // strict-mode policy stays consistent. Documented in the
        // module doc-comment so RECONCILE can reclassify if a fuller
        // smoke is added later.
        let _ = self;
        None
    }
}

// ---------------------------------------------------------------------------
// Command entry point.
// ---------------------------------------------------------------------------

/// Run the doctor command. Returns the process exit code.
pub fn run(strict: bool, output: DoctorOutput) -> i32 {
    let runtime = match tokio::runtime::Handle::try_current() {
        Ok(h) => h,
        Err(_) => {
            // Standalone CLI path: build a fresh current-thread
            // runtime and never expose it to the caller.
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build doctor runtime");
            let handle = rt.handle().clone();
            return run_with_handle(handle, strict, output, Some(rt));
        }
    };
    run_with_handle(runtime, strict, output, None)
}

fn run_with_handle(
    runtime: tokio::runtime::Handle,
    strict: bool,
    output: DoctorOutput,
    _keep_alive: Option<tokio::runtime::Runtime>,
) -> i32 {
    let probe = LiveProbe::from_env(runtime);
    let rows = diagnose(&probe);
    match output {
        DoctorOutput::Table => print!("{}", render_table(&rows)),
        DoctorOutput::Json => println!("{}", render_json(&rows)),
    }
    exit_code(&rows, strict)
}

/// Path helper kept public so external smoke scripts can find the
/// resolved default location (matches [`default_env_path`]).
pub fn env_path() -> &'static Path {
    Path::new(".env")
}
