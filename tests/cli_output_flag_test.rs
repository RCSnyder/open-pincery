//! AC-47 integration: `--output` flag end-to-end.
//!
//! Spawns the real `pcy` binary via `CARGO_BIN_EXE_pcy` with a
//! hermetic `PCY_CONFIG_PATH` tempfile so the `context list` verb
//! has deterministic rows to render. Each variant of `--output`
//! (`json`, `yaml`, `name`, `jsonpath=<expr>`) is exercised against
//! the same fixture; the pipe-default (no `--output`) is also
//! covered so we lock in the contract that non-TTY stdout defaults
//! to JSON.
//!
//! `NO_COLOR` is injected so the `table` default — if ever printed
//! by accident — wouldn't leak ANSI into the captured stdout.

use std::path::Path;
use std::process::Command;

fn pcy_bin() -> String {
    std::env::var("CARGO_BIN_EXE_pcy").expect("pcy binary path set by cargo")
}

fn write_fixture(path: &Path) {
    // A hand-rolled v8 fixture with two contexts so jsonpath over
    // `[*].name` has something to filter across.
    let toml = r#"current-context = "default"

[contexts.default]
url = "http://localhost:8080"
token = "t1"

[contexts.prod]
url = "https://prod"
token = "t2"
workspace_id = "w-prod"
"#;
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, toml).unwrap();
}

fn run_pcy(args: &[&str], cfg_path: &Path) -> (String, String, i32) {
    let out = Command::new(pcy_bin())
        .env("PCY_CONFIG_PATH", cfg_path)
        .env("NO_COLOR", "1")
        // Force the non-TTY branch so default_for_tty() picks `Json`
        // regardless of where the test runs (terminal vs CI).
        .env("PCY_NO_TTY", "1")
        .args(args)
        .output()
        .expect("spawn pcy");
    let stdout = String::from_utf8(out.stdout).unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    let code = out.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

/// AC-47: `--output json` produces a parseable JSON array over the
/// context-list fixture. Global flag placement (`pcy --output json
/// context list`) and trailing-flag placement
/// (`pcy context list --output json`) must both work because clap's
/// `global = true` forwards the flag either way.
#[test]
fn output_json_parses_at_root_or_leaf() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    write_fixture(&cfg);

    for args in [
        &["--output", "json", "context", "list"][..],
        &["context", "list", "--output", "json"][..],
    ] {
        let (stdout, stderr, code) = run_pcy(args, &cfg);
        assert_eq!(code, 0, "args={args:?} stderr={stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
            .unwrap_or_else(|e| panic!("json parse failed for args={args:?}: {e}\n{stdout}"));
        let arr = parsed.as_array().expect("top-level array");
        assert_eq!(arr.len(), 2, "expected two contexts, got {arr:?}");
    }
}

/// AC-47: `--output yaml` produces a parseable YAML document.
#[test]
fn output_yaml_is_valid_yaml() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    write_fixture(&cfg);

    let (stdout, stderr, code) = run_pcy(&["context", "list", "--output", "yaml"], &cfg);
    assert_eq!(code, 0, "stderr={stderr}");
    let parsed: serde_yaml::Value = serde_yaml::from_str(&stdout)
        .unwrap_or_else(|e| panic!("yaml parse failed: {e}\n{stdout}"));
    let seq = parsed.as_sequence().expect("top-level sequence");
    assert_eq!(seq.len(), 2);
}

/// AC-47: `--output name` emits one name per line, no other columns.
#[test]
fn output_name_emits_one_name_per_line() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    write_fixture(&cfg);

    let (stdout, stderr, code) = run_pcy(&["context", "list", "--output", "name"], &cfg);
    assert_eq!(code, 0, "stderr={stderr}");
    let names: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert!(names.contains(&"default"), "{names:?}");
    assert!(names.contains(&"prod"), "{names:?}");
    assert_eq!(names.len(), 2);
    // No extra columns leaked — every line is exactly the name,
    // no tabs, no spaces, no `*` active marker.
    for line in &names {
        assert!(!line.contains('\t'), "tab leaked: {line:?}");
        assert!(!line.contains('*'), "active marker leaked: {line:?}");
    }
}

/// AC-47: `--output jsonpath=<expr>` filters via jsonpath-rust's
/// kubectl-compatible subset. Uses a bracket expression to pluck the
/// names array.
#[test]
fn output_jsonpath_filters_over_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    write_fixture(&cfg);

    let (stdout, stderr, code) =
        run_pcy(&["context", "list", "--output", "jsonpath=$[*].name"], &cfg);
    assert_eq!(code, 0, "stderr={stderr}");
    // render_jsonpath emits one unquoted string per line for string
    // matches (kubectl-compatible output shape).
    let names: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert!(names.contains(&"default"), "{names:?}");
    assert!(names.contains(&"prod"), "{names:?}");
}

/// AC-47: pipe default (no `--output`, non-TTY) is `json`. Proven by
/// the stdout parsing as JSON without any `--output` flag passed.
#[test]
fn pipe_default_is_json_when_no_output_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    write_fixture(&cfg);

    let (stdout, stderr, code) = run_pcy(&["context", "list"], &cfg);
    assert_eq!(code, 0, "stderr={stderr}");
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("pipe default is not JSON: {e}\n{stdout}"));
    assert!(parsed.is_array());
}

/// AC-47: an unknown `--output` value fails at clap-parse time (exit 2)
/// rather than silently falling through to a default. Locks the
/// error-path contract so typos don't produce empty stdout.
#[test]
fn unknown_output_format_is_a_clap_parse_error() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    write_fixture(&cfg);

    let (stdout, stderr, code) = run_pcy(&["context", "list", "--output", "xml"], &cfg);
    assert_ne!(code, 0, "stdout={stdout} stderr={stderr}");
    // clap's usage-error exit code is 2.
    assert_eq!(code, 2, "expected clap usage-error exit 2, got {code}");
    assert!(stdout.is_empty(), "no stdout on parse error: {stdout}");
}
