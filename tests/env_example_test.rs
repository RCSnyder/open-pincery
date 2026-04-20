//! AC-29: `.env.example` is a current and complete config contract.
//!
//! Scans the source tree for every `std::env::var("KEY")` call site and
//! asserts each key is either:
//!   1. Present in `.env.example` (active or commented entry), or
//!   2. Listed in `INTERNAL_ONLY` below with a justification.
//!
//! Also verifies the secure defaults required by AC-32.

use std::collections::HashSet;

/// Env vars the runtime reads but deliberately does not surface in
/// `.env.example` — typically because they are test-only, CI-only, or
/// documented elsewhere. Every entry must carry a comment explaining why.
const INTERNAL_ONLY: &[&str] = &[
    // Test-only — set by the integration test harness, not by operators.
    "TEST_DATABASE_URL",
    // Set by Docker's automatic linking / user tooling, never by operators.
    "DOCKER_SMOKE",
    "COMPOSE_AVAILABLE",
    // HOME is stdlib/cli convenience, not a product config.
    "HOME",
];

fn scan_source_for_env_vars() -> HashSet<String> {
    let mut found = HashSet::new();
    let roots = ["src"];
    for root in roots {
        walk(std::path::Path::new(root), &mut found);
    }
    found
}

fn walk(dir: &std::path::Path, found: &mut HashSet<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, found);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                extract_env_var_keys(&contents, found);
            }
        }
    }
}

/// Extracts the string literal argument from every `env::var("...")` call.
fn extract_env_var_keys(src: &str, found: &mut HashSet<String>) {
    // Match `env::var("KEY")` — simple, matches both `std::env::var` and the
    // `use std::env` shorthand. We skip the non-literal forms (the single
    // generic helper in config.rs that takes a `&str` parameter).
    for (idx, _) in src.match_indices("env::var(\"") {
        let after = &src[idx + "env::var(\"".len()..];
        if let Some(end) = after.find('"') {
            let key = &after[..end];
            // Filter out obvious non-env patterns (shouldn't occur but defensive).
            if key
                .chars()
                .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
            {
                found.insert(key.to_string());
            }
        }
    }
}

fn env_example_keys() -> HashSet<String> {
    let contents = std::fs::read_to_string(".env.example").expect(".env.example must exist");
    let mut keys = HashSet::new();
    for line in contents.lines() {
        let line = line.trim_start();
        // Active entry:  KEY=value
        // Commented opt: # KEY=value
        let without_hash = line.strip_prefix('#').map(str::trim_start).unwrap_or(line);
        if let Some(eq) = without_hash.find('=') {
            let key = &without_hash[..eq];
            if key
                .chars()
                .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
                && !key.is_empty()
            {
                keys.insert(key.to_string());
            }
        }
    }
    keys
}

#[test]
fn ac_29_every_source_env_var_is_in_env_example_or_allowlisted() {
    let source_keys = scan_source_for_env_vars();
    let example_keys = env_example_keys();
    let allow: HashSet<String> = INTERNAL_ONLY.iter().map(|s| s.to_string()).collect();

    let mut missing: Vec<&String> = source_keys
        .iter()
        .filter(|k| !example_keys.contains(*k) && !allow.contains(*k))
        .collect();
    missing.sort();

    assert!(
        missing.is_empty(),
        "Every env var the runtime reads must appear in .env.example or in \
         tests/env_example_test.rs::INTERNAL_ONLY. Missing: {missing:?} (AC-29)"
    );
}

#[test]
fn ac_29_env_example_has_no_orphan_entries() {
    // Every key in .env.example must correspond to either a real runtime read
    // OR the documented CLI/pcy helpers listed below. This prevents
    // .env.example from drifting in the other direction (accumulating stale
    // entries that confuse operators).
    let source_keys = scan_source_for_env_vars();
    let example_keys = env_example_keys();
    // Allowlist: config-helper-read keys that the static scan can't see
    // because they're fetched via a variable key (e.g. config.rs `env_or`
    // helper takes `&str` at runtime, and main.rs `price_from_env` forwards
    // its first argument to `std::env::var(key)`). These are real reads —
    // just not literal env::var("KEY") call sites.
    let dynamic_but_known: &[&str] = &[
        "DATABASE_URL",
        "OPEN_PINCERY_HOST",
        "OPEN_PINCERY_PORT",
        "OPEN_PINCERY_BOOTSTRAP_TOKEN",
        "LLM_API_BASE_URL",
        "LLM_API_KEY",
        "LLM_MODEL",
        "LLM_MAINTENANCE_MODEL",
        "LLM_PRICE_INPUT_PER_MTOK",
        "LLM_PRICE_OUTPUT_PER_MTOK",
        "LLM_MAINTENANCE_PRICE_INPUT_PER_MTOK",
        "LLM_MAINTENANCE_PRICE_OUTPUT_PER_MTOK",
        "MAX_PROMPT_CHARS",
        "ITERATION_CAP",
        "STALE_WAKE_HOURS",
        "WAKE_SUMMARY_LIMIT",
        "EVENT_WINDOW_LIMIT",
        "RUST_LOG",
    ];
    let known: HashSet<String> = source_keys
        .iter()
        .cloned()
        .chain(dynamic_but_known.iter().map(|s| s.to_string()))
        .collect();
    let mut orphans: Vec<&String> = example_keys
        .iter()
        .filter(|k| !known.contains(*k))
        .collect();
    orphans.sort();
    assert!(
        orphans.is_empty(),
        ".env.example contains keys no source file reads: {orphans:?} (AC-29)"
    );
}

#[test]
fn ac_32_env_example_defaults_to_loopback_host() {
    let contents = std::fs::read_to_string(".env.example").expect(".env.example");
    assert!(
        contents.contains("OPEN_PINCERY_HOST=127.0.0.1"),
        ".env.example must default OPEN_PINCERY_HOST to 127.0.0.1 (AC-32)"
    );
    assert!(
        !contents.contains("OPEN_PINCERY_HOST=0.0.0.0"),
        ".env.example must not default to 0.0.0.0 (AC-32)"
    );
}

#[test]
fn ac_29_env_example_includes_openai_alternative() {
    let contents = std::fs::read_to_string(".env.example").expect(".env.example");
    assert!(
        contents.contains("api.openai.com"),
        ".env.example must include a commented OpenAI alternative block (AC-29)"
    );
}
