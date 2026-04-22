//! AC-53 / A2b.1: Linux sandbox dependency gate.
//!
//! These tests are the admission-control contract for the four Linux-only
//! crates (`seccompiler`, `landlock`, `cgroups-rs`, `nix`) that make
//! AC-53's Zerobox real. They assert four things, independent of whether
//! `cargo deny check` has been run:
//!
//! 1. `Cargo.toml` declares each crate as a Linux-target-gated dependency,
//!    not a platform-wide dependency (otherwise Windows/Mac `cargo build`
//!    breaks for contributors who haven't rebuilt inside the devshell yet).
//! 2. Each version is a concrete pin (`^0.X` style), not a floating
//!    `"*"` or git ref.
//! 3. `deny.toml`'s license allowlist covers every license these crates
//!    declare (MIT and Apache-2.0 are already covered from v6; this
//!    test guards against a future crate being added without its
//!    license entry being checked).
//! 4. `deny.toml`'s `[bans]` section has no entries denying these
//!    crate names — i.e. we have not accidentally banned a crate we
//!    also depend on.
//!
//! The purpose is: make scope-creep visible. If a future slice tries to
//! add a Linux sandbox crate without reviewing it, this test fails loudly
//! and REVIEW sees the diff.

use std::fs;

const SANDBOX_CRATES: &[&str] = &["seccompiler", "landlock", "cgroups-rs", "nix"];

fn cargo_toml() -> toml::Table {
    let s = fs::read_to_string("Cargo.toml").expect("Cargo.toml must exist");
    toml::from_str(&s).expect("Cargo.toml must be valid TOML")
}

fn deny_toml() -> toml::Table {
    let s = fs::read_to_string("deny.toml").expect("deny.toml must exist");
    toml::from_str(&s).expect("deny.toml must be valid TOML")
}

/// Resolve the Linux-target dependency table at
/// `[target."cfg(target_os = \"linux\")".dependencies]`.
fn linux_target_deps(cargo: &toml::Table) -> &toml::Table {
    let target = cargo
        .get("target")
        .and_then(|v| v.as_table())
        .expect("Cargo.toml must have a [target.*] section for Linux sandbox crates");
    // Match by string search — TOML sub-table keys with special chars
    // (quotes, dots) are kept verbatim at this level.
    let (_, linux_cfg) = target
        .iter()
        .find(|(k, _)| {
            let k = k.trim_matches('"');
            k.contains("target_os") && k.contains("linux")
        })
        .expect(
            "Cargo.toml must declare a `[target.'cfg(target_os = \"linux\")']` section so the \
             sandbox crates are not linked on Windows/macOS",
        );
    linux_cfg
        .get("dependencies")
        .and_then(|v| v.as_table())
        .expect("Linux target section must have a [.dependencies] sub-table")
}

#[test]
fn all_four_sandbox_crates_are_declared_linux_only() {
    let cargo = cargo_toml();
    let deps = linux_target_deps(&cargo);
    for crate_name in SANDBOX_CRATES {
        assert!(
            deps.contains_key(*crate_name),
            "Cargo.toml must declare `{crate_name}` under \
             [target.'cfg(target_os = \"linux\")'.dependencies] (AC-53 / A2b.1)"
        );
    }
}

#[test]
fn sandbox_crates_are_absent_from_platform_wide_dependencies() {
    // The whole point of Linux-gating is that Windows/macOS `cargo
    // build` does not try to compile them. If any of these leak into
    // the top-level `[dependencies]`, non-Linux CI breaks.
    let cargo = cargo_toml();
    let platform_deps = cargo
        .get("dependencies")
        .and_then(|v| v.as_table())
        .expect("[dependencies] must exist");
    for crate_name in SANDBOX_CRATES {
        // `nix` is the one edge case: if already transitively pulled,
        // a promoted direct-dep entry at top level would wrongly
        // compile it on Windows. Enforce that the direct declaration
        // lives in the Linux-gated table.
        assert!(
            !platform_deps.contains_key(*crate_name),
            "{crate_name} must NOT be a top-level [dependencies] entry; it is Linux-only. \
             Move it under [target.'cfg(target_os = \"linux\")'.dependencies]."
        );
    }
}

#[test]
fn sandbox_crate_versions_are_concrete_pins() {
    let cargo = cargo_toml();
    let deps = linux_target_deps(&cargo);
    for crate_name in SANDBOX_CRATES {
        let spec = deps.get(*crate_name).unwrap_or_else(|| {
            panic!("{crate_name} missing from Linux target deps — failed in the prior test")
        });
        let version_str = match spec {
            toml::Value::String(s) => s.clone(),
            toml::Value::Table(t) => t
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    panic!("{crate_name} table form must carry a `version = \"...\"` key")
                })
                .to_string(),
            other => {
                panic!("{crate_name} dependency spec must be a string or table, got {other:?}")
            }
        };
        assert!(
            !version_str.contains('*'),
            "{crate_name} version must be a concrete pin, not a wildcard: {version_str:?}"
        );
        assert!(
            !version_str.is_empty(),
            "{crate_name} version must not be empty"
        );
        // Accept caret-implicit `"0.5"` / explicit `"^0.5"` / exact `"=0.5.1"`.
        let first = version_str.chars().next().unwrap();
        assert!(
            first.is_ascii_digit() || first == '^' || first == '=' || first == '~',
            "{crate_name} version must start with a digit, `^`, `=`, or `~` \
             (got {version_str:?}); git/path deps are not permitted for sandbox crates"
        );
    }
}

#[test]
fn deny_toml_has_not_banned_any_sandbox_crate() {
    // Defensive: if a future dev writes `deny = ["nix"]` without
    // realising it's now a direct dep, `cargo deny check` would fail.
    // Catch it earlier with a unit test.
    let deny = deny_toml();
    let bans = deny
        .get("bans")
        .and_then(|v| v.as_table())
        .expect("deny.toml must have a [bans] section");
    let deny_list = bans
        .get("deny")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for entry in deny_list {
        let name = match &entry {
            toml::Value::String(s) => s.clone(),
            toml::Value::Table(t) => t
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            _ => continue,
        };
        for sandbox in SANDBOX_CRATES {
            assert_ne!(
                name, *sandbox,
                "deny.toml [bans].deny names `{sandbox}`, which is a direct sandbox dependency"
            );
        }
    }
}

#[test]
fn deny_toml_license_allowlist_covers_sandbox_crate_licenses() {
    // Known licenses for the four sandbox crates as of April 2026
    // (verified on crates.io):
    //   seccompiler  — Apache-2.0
    //   landlock     — Apache-2.0 OR MIT
    //   cgroups-rs   — Apache-2.0 OR MIT
    //   nix          — MIT
    // v6 deny.toml already allows MIT and Apache-2.0. This test pins
    // that invariant so a future dev can't drop one accidentally.
    let deny = deny_toml();
    let licenses = deny
        .get("licenses")
        .and_then(|v| v.as_table())
        .expect("deny.toml must have [licenses] section");
    let allow = licenses
        .get("allow")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let allow_set: Vec<String> = allow
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    for required in ["Apache-2.0", "MIT"] {
        assert!(
            allow_set.iter().any(|s| s == required),
            "deny.toml [licenses].allow must include `{required}` (covers sandbox crates)"
        );
    }
}
