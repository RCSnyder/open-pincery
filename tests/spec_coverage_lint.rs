//! AC-81 — spec coverage lint.
//!
//! Mechanically validates that `scaffolding/spec_coverage.md` accurately
//! maps every v9 acceptance criterion (AC-53..AC-88) to canonical TLA+
//! actions that actually exist in the `Next` disjunction body of
//! `docs/input/OpenPinceryCanonical.tla`.
//!
//! This test is the single source of mechanical truth for AC-81. If
//! `spec_coverage.md` cites an action name that does not appear in
//! `Next`, this test fails. If a v9 AC row is missing or duplicated,
//! this test fails.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

const REQUIRED_AC_RANGE: std::ops::RangeInclusive<u32> = 53..=88;
const NO_ACTION_MARKER: &str = "—";

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_text(rel: &str) -> String {
    let p = workspace_root().join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {}", p.display(), e))
}

/// Parse the rows of the spec_coverage.md table. Returns
/// AC-id -> Vec<canonical_action_token> (or empty vec if `—`).
///
/// Each row is split on unescaped `|`, then the canonical action cell
/// (column index 1) is split on `|` to recover individual action
/// tokens for multi-action ACs.
fn parse_spec_coverage_rows() -> BTreeMap<String, Vec<String>> {
    let text = read_text("scaffolding/spec_coverage.md");
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if !line.starts_with('|') {
            continue;
        }
        if line.contains("Canonical Action") || line.contains("---") {
            continue;
        }
        let cells = split_unescaped_pipes(line);
        if cells.len() < 3 {
            continue;
        }
        let ac = cells[0].trim();
        if !ac.starts_with("AC-") {
            continue;
        }
        let actions_cell = cells[1].trim();
        let actions: Vec<String> = if actions_cell == NO_ACTION_MARKER {
            Vec::new()
        } else {
            actions_cell
                .split('|')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        };
        out.insert(ac.to_string(), actions);
    }
    out
}

/// Split a markdown table row on `|` characters that are not preceded
/// by a backslash. Drops the empty leading and trailing cells produced
/// by the row's outer `|` delimiters.
fn split_unescaped_pipes(line: &str) -> Vec<String> {
    let mut cells: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut prev = '\0';
    for ch in line.chars() {
        if ch == '|' && prev != '\\' {
            cells.push(std::mem::take(&mut current));
        } else if ch == '|' && prev == '\\' {
            // Replace the backslash we already pushed with the literal `|`.
            current.pop();
            current.push('|');
        } else {
            current.push(ch);
        }
        prev = ch;
    }
    cells.push(current);
    // Drop empty leading + trailing cells caused by outer `|`.
    if cells.first().map(|c| c.trim().is_empty()).unwrap_or(false) {
        cells.remove(0);
    }
    if cells.last().map(|c| c.trim().is_empty()).unwrap_or(false) {
        cells.pop();
    }
    cells
}

/// Extract the set of canonical action identifiers that appear inside
/// the `Next ==` disjunction body of the canonical TLA+ spec.
fn parse_canonical_next_actions() -> BTreeSet<String> {
    let text = read_text("docs/input/OpenPinceryCanonical.tla");
    let mut in_next = false;
    let mut body = String::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if !in_next {
            if trimmed.starts_with("Next ==") {
                in_next = true;
            }
            continue;
        }
        // End of Next: first non-blank line that does NOT start with
        // a disjunction continuation `\/`.
        if trimmed.is_empty() {
            // Blank lines inside the block are fine.
            continue;
        }
        if !trimmed.starts_with("\\/") {
            break;
        }
        body.push_str(trimmed);
        body.push(' ');
    }
    assert!(
        !body.is_empty(),
        "failed to locate Next == disjunction body"
    );

    // Tokenize identifiers (Pascal/Camel-case words). Strip the `\/`
    // operator and split on whitespace; keep tokens that look like
    // canonical action names: leading uppercase ASCII letter followed
    // by alphanumerics.
    let mut actions: BTreeSet<String> = BTreeSet::new();
    for raw_tok in body.split_whitespace() {
        let tok = raw_tok.trim_matches(|c: char| !c.is_alphanumeric());
        if tok.is_empty() {
            continue;
        }
        let first = tok.chars().next().unwrap();
        if !first.is_ascii_uppercase() {
            continue;
        }
        if !tok.chars().all(|c| c.is_ascii_alphanumeric()) {
            continue;
        }
        actions.insert(tok.to_string());
    }
    actions
}

#[test]
fn table_well_formed() {
    let rows = parse_spec_coverage_rows();
    assert!(
        !rows.is_empty(),
        "spec_coverage.md produced zero parseable rows"
    );
    for ac in rows.keys() {
        assert!(ac.starts_with("AC-"), "row id {ac} does not start with AC-");
        let n: u32 = ac
            .trim_start_matches("AC-")
            .parse()
            .unwrap_or_else(|_| panic!("row id {ac} does not parse as AC-<number>"));
        assert!(n >= 1, "AC number {n} is not positive");
    }
}

#[test]
fn all_acs_present_with_canonical_actions() {
    let rows = parse_spec_coverage_rows();
    let mut missing: Vec<String> = Vec::new();
    for n in REQUIRED_AC_RANGE {
        let key = format!("AC-{n}");
        if !rows.contains_key(&key) {
            missing.push(key);
        }
    }
    assert!(
        missing.is_empty(),
        "spec_coverage.md is missing required AC rows: {missing:?}"
    );
}

#[test]
fn every_cited_action_is_in_canonical_next() {
    let rows = parse_spec_coverage_rows();
    let canonical = parse_canonical_next_actions();
    let mut violations: Vec<String> = Vec::new();
    for (ac, actions) in &rows {
        for a in actions {
            if !canonical.contains(a) {
                violations.push(format!(
                    "{ac} cites canonical_action '{a}' which is NOT in Next disjunction"
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "spec_coverage.md cites unknown canonical actions:\n  {}",
        violations.join("\n  ")
    );
}

#[test]
fn no_duplicate_ac_rows() {
    let text = read_text("scaffolding/spec_coverage.md");
    let mut seen: BTreeMap<String, u32> = BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if !line.starts_with('|') {
            continue;
        }
        if line.contains("Canonical Action") || line.contains("---") {
            continue;
        }
        let cells = split_unescaped_pipes(line);
        if cells.is_empty() {
            continue;
        }
        let ac = cells[0].trim().to_string();
        if !ac.starts_with("AC-") {
            continue;
        }
        *seen.entry(ac).or_insert(0) += 1;
    }
    let dups: Vec<&String> = seen
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(k, _)| k)
        .collect();
    assert!(
        dups.is_empty(),
        "spec_coverage.md has duplicate AC rows: {dups:?}"
    );
}

#[test]
fn no_empty_action_cells() {
    // Every row's canonical_action cell must be either `—` (no
    // canonical action) or a non-empty pipe-separated list. An empty
    // cell would mean the row is undecided and silently un-linted.
    let text = read_text("scaffolding/spec_coverage.md");
    let mut empties: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if !line.starts_with('|') {
            continue;
        }
        if line.contains("Canonical Action") || line.contains("---") {
            continue;
        }
        let cells = split_unescaped_pipes(line);
        if cells.len() < 2 {
            continue;
        }
        let ac = cells[0].trim();
        if !ac.starts_with("AC-") {
            continue;
        }
        let action_cell = cells[1].trim();
        if action_cell.is_empty() {
            empties.push(ac.to_string());
        }
    }
    assert!(
        empties.is_empty(),
        "spec_coverage.md has empty canonical_action cells for: {empties:?}"
    );
}
