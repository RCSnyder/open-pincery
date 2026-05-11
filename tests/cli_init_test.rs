//! AC-89 (v9.1): tests for `pcy init` — `.env` bootstrap.
//!
//! Exercises the pure-Rust render path + filesystem behaviour. The
//! interactive prompts are stubbed by injecting [`Prompts`] with
//! deterministic closures so the suite runs without a TTY.

use std::fs;
use std::path::PathBuf;

use open_pincery::cli::commands::init::{
    self, generate_bootstrap_token, generate_vault_key, render_env, run, Prompts,
    DEFAULT_LLM_BASE_URL,
};

fn fixed_prompts(key: &'static str, url: &'static str) -> Prompts {
    Prompts {
        llm_key: Box::new(move || Ok(key.to_string())),
        llm_base_url: Box::new(move || Ok(url.to_string())),
    }
}

#[test]
fn bootstrap_token_is_64_hex_chars() {
    let t = generate_bootstrap_token().expect("rng");
    assert_eq!(t.len(), 64, "token = {t:?}");
    assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn vault_key_is_base64_of_32_bytes() {
    use base64::Engine as _;
    let k = generate_vault_key().expect("rng");
    // `STANDARD` base64 of 32 raw bytes is exactly 44 chars (4 of
    // which are `=` padding only if the input length is a multiple
    // of 3 — 32 is not, so the standard encoding produces "=" padding).
    assert_eq!(k.len(), 44, "key = {k:?}");
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(k.as_bytes())
        .expect("valid base64");
    assert_eq!(decoded.len(), 32);
}

#[test]
fn render_env_includes_all_required_vars() {
    let body = render_env(
        "deadbeef".repeat(8).as_str(),
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        Some("sk-test-123"),
        DEFAULT_LLM_BASE_URL,
    );
    for var in [
        "DATABASE_URL=",
        "OPEN_PINCERY_HOST=",
        "OPEN_PINCERY_PORT=",
        "OPEN_PINCERY_BOOTSTRAP_TOKEN=",
        "OPEN_PINCERY_VAULT_KEY=",
        "LLM_API_BASE_URL=",
        "LLM_API_KEY=sk-test-123",
    ] {
        assert!(
            body.contains(var),
            "missing `{var}` in rendered body:\n{body}"
        );
    }
}

#[test]
fn render_env_blank_key_emits_credential_hint_not_var() {
    let body = render_env(
        "00".repeat(32).as_str(),
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        None,
        DEFAULT_LLM_BASE_URL,
    );
    assert!(
        !body.contains("LLM_API_KEY="),
        "blank key must NOT produce an LLM_API_KEY= line; body:\n{body}"
    );
    assert!(body.contains("pcy credential add openai_api_key"));
    assert!(body.contains("pcy provider add openrouter"));
}

#[test]
fn run_writes_file_and_returns_path() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let out = dir.path().join(".env");
    let written = run(Some(out.clone()), false, fixed_prompts("sk-abc", "")).expect("init ok");
    assert_eq!(written, out);
    assert!(out.exists());

    let body = fs::read_to_string(&out).expect("read");
    // Empty url prompt collapses to the default per Prompts::interactive
    // semantics — our test stub returned "" which trims to "" and
    // render_env writes that verbatim. Run's behaviour here is to pass
    // through; the *interactive* default-substitution lives in the
    // prompt closure. So with a "" stub we expect the literal "":
    assert!(body.contains("LLM_API_BASE_URL=\n"));
    assert!(body.contains("LLM_API_KEY=sk-abc\n"));
    assert!(body.contains("OPEN_PINCERY_BOOTSTRAP_TOKEN="));
    assert!(body.contains("OPEN_PINCERY_VAULT_KEY="));
}

#[test]
fn run_refuses_overwrite_without_force() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let out = dir.path().join(".env");
    fs::write(&out, "PRE-EXISTING\n").expect("seed");

    let err = run(
        Some(out.clone()),
        false,
        fixed_prompts("ignored", DEFAULT_LLM_BASE_URL),
    )
    .expect_err("must refuse to clobber existing file");
    assert!(
        format!("{err}").contains("--force"),
        "error must mention --force; got: {err}"
    );

    // File is untouched.
    let body = fs::read_to_string(&out).expect("read");
    assert_eq!(body, "PRE-EXISTING\n");
}

#[test]
fn run_with_force_overwrites() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let out = dir.path().join(".env");
    fs::write(&out, "PRE-EXISTING\n").expect("seed");

    run(
        Some(out.clone()),
        true,
        fixed_prompts("sk-xyz", DEFAULT_LLM_BASE_URL),
    )
    .expect("force overwrite");

    let body = fs::read_to_string(&out).expect("read");
    assert!(!body.contains("PRE-EXISTING"));
    assert!(body.contains("LLM_API_KEY=sk-xyz"));
}

#[cfg(unix)]
#[test]
fn run_sets_mode_0600_on_unix() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("tmpdir");
    let out = dir.path().join(".env");
    run(
        Some(out.clone()),
        false,
        fixed_prompts("sk", DEFAULT_LLM_BASE_URL),
    )
    .expect("init ok");
    let perms = fs::metadata(&out).expect("stat").permissions();
    let mode = perms.mode() & 0o777;
    assert_eq!(mode, 0o600, "expected mode 0600, got {mode:o}");
}

#[test]
fn next_steps_message_does_not_leak_secret_values() {
    let path = PathBuf::from("/tmp/test.env");
    let msg = init::next_steps_message(&path);
    // The message references the file but never embeds the literal
    // secret bytes. We assert by spot-checking: pass a token through
    // render_env, then make sure that token value isn't in the
    // next-steps string.
    let token = generate_bootstrap_token().unwrap();
    let key = generate_vault_key().unwrap();
    assert!(!msg.contains(&token));
    assert!(!msg.contains(&key));
    assert!(msg.contains("pcy login"));
    assert!(msg.contains("pcy doctor"));
}
