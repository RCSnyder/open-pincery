//! AC-37 (v6): zero-advisory floor for `cargo deny`.
//!
//! Asserts the invariants of `deny.toml`'s `[advisories]` section as a static
//! test so that accidental drift (e.g. a developer reinstating an ignore entry
//! or loosening `yanked` to `"warn"`) fails fast, independent of CI running
//! `cargo deny check`.
//!
//! The spirit of the AC: any new RUSTSEC advisory must fail the build unless a
//! human explicitly removes it from the tree — AND any exception must carry a
//! dated, justified reason. We encode that here as:
//! - `version = 2` (v2 implicitly denies all vulnerabilities)
//! - `yanked = "deny"` (yanked crates also fail)
//! - `ignore` list contains ONLY the `ALLOWED_ADVISORIES` set below, and every
//!   entry must be a table with both `id` and `reason`.

use std::fs;

fn advisories_table() -> toml::Table {
    let contents = fs::read_to_string("deny.toml").expect("deny.toml must exist at repo root");
    let root: toml::Table = toml::from_str(&contents).expect("deny.toml must be valid TOML");
    root.get("advisories")
        .and_then(|v| v.as_table())
        .cloned()
        .expect("deny.toml must have an [advisories] table")
}

#[test]
fn advisories_uses_version_2() {
    let adv = advisories_table();
    let version = adv
        .get("version")
        .and_then(|v| v.as_integer())
        .expect("[advisories] must declare version");
    assert_eq!(
        version, 2,
        "deny.toml [advisories] must be version 2 (v2 implicitly denies vulnerabilities)"
    );
}

#[test]
fn advisories_denies_yanked() {
    let adv = advisories_table();
    let yanked = adv
        .get("yanked")
        .and_then(|v| v.as_str())
        .expect("[advisories] must set yanked");
    assert_eq!(
        yanked, "deny",
        "v6 AC-37 requires yanked = \"deny\"; found {yanked:?}"
    );
}

#[test]
fn advisories_ignore_list_only_contains_documented_exceptions() {
    // The only permitted exceptions. Each entry:
    //   - must be present in deny.toml's ignore list
    //   - must carry a non-empty `reason`
    // Any advisory in deny.toml not in this set fails the test. Any
    // advisory in this set not in deny.toml also fails the test. To
    // add a new exception you must update BOTH this list and deny.toml
    // in the same PR — a STOP-and-raise event.
    const ALLOWED_ADVISORIES: &[&str] = &["RUSTSEC-2024-0370"];

    let adv = advisories_table();
    let ignore = adv
        .get("ignore")
        .and_then(|v| v.as_array())
        .expect("[advisories] must declare an ignore array");

    let mut seen_ids: Vec<String> = Vec::new();
    for entry in ignore {
        let table = entry
            .as_table()
            .expect("every ignore entry must be a table with `id` and `reason`");
        let id = table
            .get("id")
            .and_then(|v| v.as_str())
            .expect("every ignore entry must have an `id`")
            .to_string();
        let reason = table
            .get("reason")
            .and_then(|v| v.as_str())
            .expect("every ignore entry must have a `reason`");
        assert!(
            !reason.trim().is_empty(),
            "ignore entry {id} has an empty reason; AC-37 requires a dated justification"
        );
        seen_ids.push(id);
    }

    for allowed in ALLOWED_ADVISORIES {
        assert!(
            seen_ids.iter().any(|s| s == allowed),
            "expected documented exception {allowed} is missing from deny.toml"
        );
    }
    for seen in &seen_ids {
        assert!(
            ALLOWED_ADVISORIES.contains(&seen.as_str()),
            "deny.toml ignores {seen} but the test's ALLOWED_ADVISORIES does not. \
             Adding a new exception is a STOP-and-raise event: update the test AND deny.toml together."
        );
    }
}
