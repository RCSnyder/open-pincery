//! AC-94 (v9.1 honesty pass) — README "Security Model" section reflects shipped
//! topology, DELIVERY.md heading bumped to v9.0, and the aspirational design
//! vocabulary `OneCLI` / `Greywall` / `Zerobox` does not appear anywhere in the
//! two documents outside `<!-- historical -->` HTML-comment blocks.
//!
//! Cross-reference scope.md "AC-94" and readiness.md "AC-94".

use std::fs;

const README_PATH: &str = "README.md";
const DELIVERY_PATH: &str = "DELIVERY.md";
const SCOPE_PATH: &str = "scaffolding/scope.md";

/// Strip everything between full-line `<!-- historical -->` and full-line
/// `<!-- /historical -->` fences (inclusive). A "full-line" fence is a line
/// whose trimmed content equals exactly the marker text — this prevents
/// in-body prose that *mentions* the marker (e.g. inside backticks) from
/// accidentally opening or closing a historical block.
fn strip_historical_blocks(s: &str) -> String {
    let open = "<!-- historical -->";
    let close = "<!-- /historical -->";
    let mut out = String::with_capacity(s.len());
    let mut inside = false;
    for line in s.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']).trim();
        if !inside && trimmed == open {
            inside = true;
            continue;
        }
        if inside && trimmed == close {
            inside = false;
            continue;
        }
        if !inside {
            out.push_str(line);
        }
    }
    assert!(
        !inside,
        "unterminated <!-- historical --> block (no full-line <!-- /historical --> close found)"
    );
    out
}

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("could not read {path}: {e}"))
}

#[test]
fn readme_contains_five_row_security_table() {
    let readme = read(README_PATH);
    let live = strip_historical_blocks(&readme);

    // Section header
    assert!(
        live.contains("## Security Model"),
        "README must keep a `## Security Model` section"
    );

    // Five rows referenced by mechanism keyword + AC anchor.
    let row_specs: &[(&str, &[&str])] = &[
        (
            "Process sandbox",
            &["AC-53", "AC-77", "AC-83", "AC-85", "AC-86"],
        ),
        ("Audit log", &["AC-78"]),
        ("Capability gate", &["AC-80"]),
        ("Prompt-injection floor", &["AC-79"]),
        // The vault row may list a subset; AC-40 + AC-71 are the load-bearing
        // anchors and must both appear.
        ("Credential vault", &["AC-40", "AC-71"]),
    ];

    for (mechanism, ac_anchors) in row_specs {
        let row = live
            .lines()
            .find(|line| line.contains(mechanism) && line.contains('|'))
            .unwrap_or_else(|| panic!("README Security Model table missing row for `{mechanism}`"));
        for ac in *ac_anchors {
            assert!(
                row.contains(ac),
                "README Security Model row `{mechanism}` must reference `{ac}`; got: {row}"
            );
        }
        assert!(
            row.contains("Shipped"),
            "README Security Model row `{mechanism}` must mark Status as `Shipped`; got: {row}"
        );
    }
}

#[test]
fn no_aspirational_vocabulary_outside_historical_markers() {
    for path in [README_PATH, DELIVERY_PATH] {
        let body = read(path);
        let live = strip_historical_blocks(&body);
        for forbidden in ["OneCLI", "Greywall", "Zerobox"] {
            assert!(
                !live.contains(forbidden),
                "{path} contains forbidden aspirational vocabulary `{forbidden}` \
                 outside <!-- historical --> markers; either remove it or wrap \
                 the surrounding context in <!-- historical --> ... <!-- /historical -->"
            );
        }
    }
}

#[test]
fn delivery_heading_is_v9_1() {
    let delivery = read(DELIVERY_PATH);
    let first_line = delivery
        .lines()
        .next()
        .expect("DELIVERY.md must not be empty");
    assert_eq!(
        first_line.trim(),
        "# DELIVERY.md — Open Pincery v9.1",
        "DELIVERY.md top heading must be exactly `# DELIVERY.md — Open Pincery v9.1`"
    );

    // The v9.1 summary lead must appear before the carried-forward
    // `## v9.0 Summary` and the `## What Was Built` section.
    let v91_idx = delivery
        .find("## v9.1 Summary")
        .expect("DELIVERY.md must contain a `## v9.1 Summary` lead");
    let v90_idx = delivery
        .find("## v9.0 Summary")
        .expect("DELIVERY.md must retain the `## v9.0 Summary` lead");
    let what_idx = delivery
        .find("## What Was Built")
        .expect("DELIVERY.md must retain `## What Was Built`");
    assert!(
        v91_idx < v90_idx,
        "`## v9.1 Summary` must precede `## v9.0 Summary`"
    );
    assert!(
        v90_idx < what_idx,
        "`## v9.0 Summary` must precede `## What Was Built`"
    );
}

#[test]
fn security_table_acs_are_shipped_per_scope() {
    // Every AC referenced in the README five-row table must correspond to a
    // shipped (closed) AC per scope.md. We use a coarse grep: the AC token
    // appears somewhere in scope.md. This is a cross-document lint, not a
    // semantic check — its job is to catch typos and removed ACs, not to
    // re-validate the truthfulness of the scope.md status itself.
    let readme = read(README_PATH);
    let live = strip_historical_blocks(&readme);
    let scope = read(SCOPE_PATH);

    let table_acs = [
        "AC-53", "AC-77", "AC-83", "AC-85", "AC-86", // process sandbox row
        "AC-78", // audit log
        "AC-80", // capability gate
        "AC-79", // prompt-injection floor
        "AC-38", "AC-40", "AC-43", "AC-71", "AC-74", // vault row (subset)
    ];

    for ac in table_acs {
        // sanity: the AC actually appears in the live README
        if !live.contains(ac) {
            // Vault row lists a subset; only enforce the load-bearing ones.
            if matches!(ac, "AC-38" | "AC-43" | "AC-74") {
                continue;
            }
            panic!("README Security Model table is missing reference to `{ac}`");
        }
        assert!(
            scope.contains(ac),
            "README Security Model table references `{ac}` but scope.md does not mention it"
        );
    }
}
