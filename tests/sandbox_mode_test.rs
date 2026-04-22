//! AC-73 (Slice A2a): Sandbox mode configuration flag.
//!
//! These tests exercise `SandboxMode::resolve()` — the pure env-value
//! validator that `Config::from_env()` delegates to. The function is
//! kept pure so tests can pass explicit values without racing on
//! process-wide `std::env::set_var` (which is unsafe under parallel
//! cargo test execution).
//!
//! Runtime proof (event emission, periodic stderr warnings while
//! `disabled`) is deferred to Slice A2b where the sandbox module
//! restructure + event log wiring lands together.

use open_pincery::config::{ResolvedSandboxMode, SandboxMode, SandboxModeError};

#[test]
fn default_mode_is_enforce() {
    // No env var set → enforce. No allow_unsafe needed.
    let resolved = ResolvedSandboxMode::resolve(None, None).expect("default must resolve");
    assert_eq!(resolved.mode, SandboxMode::Enforce);
    assert!(!resolved.allow_unsafe);
}

#[test]
fn explicit_enforce_parses() {
    let resolved = ResolvedSandboxMode::resolve(Some("enforce"), None).unwrap();
    assert_eq!(resolved.mode, SandboxMode::Enforce);
}

#[test]
fn audit_mode_parses_without_allow_unsafe() {
    let resolved = ResolvedSandboxMode::resolve(Some("audit"), None).unwrap();
    assert_eq!(resolved.mode, SandboxMode::Audit);
    assert!(!resolved.allow_unsafe);
}

#[test]
fn disabled_without_allow_unsafe_is_rejected() {
    // AC-73: `disabled` is a footgun. Requires paired ALLOW_UNSAFE=true.
    let err = ResolvedSandboxMode::resolve(Some("disabled"), None).unwrap_err();
    assert!(
        matches!(err, SandboxModeError::DisabledRequiresAllowUnsafe),
        "expected DisabledRequiresAllowUnsafe, got {err:?}"
    );
}

#[test]
fn disabled_with_allow_unsafe_false_is_rejected() {
    // Explicit `false` is not acceptance. Only literal `true`.
    let err = ResolvedSandboxMode::resolve(Some("disabled"), Some("false")).unwrap_err();
    assert!(matches!(err, SandboxModeError::DisabledRequiresAllowUnsafe));
}

#[test]
fn disabled_with_allow_unsafe_true_is_accepted() {
    let resolved = ResolvedSandboxMode::resolve(Some("disabled"), Some("true")).unwrap();
    assert_eq!(resolved.mode, SandboxMode::Disabled);
    assert!(resolved.allow_unsafe);
}

#[test]
fn unknown_mode_value_is_rejected() {
    let err = ResolvedSandboxMode::resolve(Some("off"), None).unwrap_err();
    assert!(matches!(err, SandboxModeError::Invalid(_)));
    let err = ResolvedSandboxMode::resolve(Some(""), None).unwrap_err();
    assert!(matches!(err, SandboxModeError::Invalid(_)));
}

#[test]
fn mode_parse_is_case_insensitive() {
    for s in ["Enforce", "ENFORCE", "enforce"] {
        assert_eq!(SandboxMode::parse(s).unwrap(), SandboxMode::Enforce);
    }
    for s in ["Audit", "AUDIT"] {
        assert_eq!(SandboxMode::parse(s).unwrap(), SandboxMode::Audit);
    }
    for s in ["Disabled", "DISABLED"] {
        assert_eq!(SandboxMode::parse(s).unwrap(), SandboxMode::Disabled);
    }
}

#[test]
fn sandbox_mode_display_round_trips() {
    for m in [
        SandboxMode::Enforce,
        SandboxMode::Audit,
        SandboxMode::Disabled,
    ] {
        let s = m.to_string();
        let parsed = SandboxMode::parse(&s).unwrap();
        assert_eq!(parsed, m, "{s} round-trip failed");
    }
}

#[test]
fn allow_unsafe_is_ignored_when_mode_is_enforce() {
    // Declaring ALLOW_UNSAFE=true without disabled is allowed but
    // records allow_unsafe=true on the resolved struct so the startup
    // warning in A2b can surface it.
    let resolved = ResolvedSandboxMode::resolve(Some("enforce"), Some("true")).unwrap();
    assert_eq!(resolved.mode, SandboxMode::Enforce);
    assert!(resolved.allow_unsafe);
}

#[test]
fn env_example_documents_sandbox_mode() {
    // AC-73: operators must be able to discover the flag from
    // .env.example. The line may be commented since `enforce` is the
    // default, but the key must be present with its valid value set.
    let env_example =
        std::fs::read_to_string(".env.example").expect(".env.example must exist at repo root");
    assert!(
        env_example.contains("OPEN_PINCERY_SANDBOX_MODE"),
        ".env.example must document OPEN_PINCERY_SANDBOX_MODE (AC-73)"
    );
    assert!(
        env_example.contains("OPEN_PINCERY_ALLOW_UNSAFE"),
        ".env.example must document OPEN_PINCERY_ALLOW_UNSAFE (AC-73)"
    );
    // The comment block near the flag should list all three valid values.
    for val in ["enforce", "audit", "disabled"] {
        assert!(
            env_example.contains(val),
            ".env.example must mention `{val}` as a valid mode (AC-73)"
        );
    }
}
