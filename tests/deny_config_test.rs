//! AC-37 (v6): zero-advisory floor for `cargo deny`.
//!
//! Asserts the invariants of `deny.toml`'s `[advisories]` section as a static
//! test so that accidental drift (e.g. a developer reinstating an ignore entry
//! or loosening `yanked` to `"warn"`) fails fast, independent of CI running
//! `cargo deny check`.
//!
//! The spirit of the AC: any new RUSTSEC advisory must fail the build unless a
//! human explicitly removes it from the tree. We encode that here as:
//! - `version = 2` (v2 implicitly denies all vulnerabilities)
//! - `yanked = "deny"` (yanked crates also fail)
//! - `ignore = []` (no advisory may be silently suppressed)

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
fn advisories_ignore_list_is_empty() {
    let adv = advisories_table();
    let ignore = adv
        .get("ignore")
        .and_then(|v| v.as_array())
        .expect("[advisories] must declare ignore = []");
    assert!(
        ignore.is_empty(),
        "v6 AC-37 requires an empty ignore list; found {} entries: {:?}. \
         Reinstating an ignore entry is a STOP-and-raise event.",
        ignore.len(),
        ignore
    );
}
