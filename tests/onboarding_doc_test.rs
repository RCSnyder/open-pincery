//! AC-92 (v9.1): `docs/onboarding.md` is the canonical one-page
//! first-run gate. These tests enforce:
//!
//! * The seven required sections exist, in order.
//! * The doc stays under the 250-line tripwire.
//! * Every `pcy <verb>` invocation shown in a fenced code block names
//!   a real top-level clap verb (no aspirational commands).
//! * Forward-references to v9.1 ACs that have not yet shipped are
//!   limited to prose (no copy-paste examples of unimplemented
//!   verbs).

use std::collections::HashSet;
use std::path::PathBuf;

fn doc_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/onboarding.md")
}

fn read_doc() -> String {
    std::fs::read_to_string(doc_path()).expect("docs/onboarding.md exists")
}

/// The clap-registered top-level verbs in `src/cli/mod.rs` as of
/// v9.1. Build-time grep would be more robust but pulls in a heavy
/// dependency; we lock this list deliberately so adding/removing a
/// verb forces a docs review.
fn real_clap_verbs() -> HashSet<&'static str> {
    [
        "login",
        "agent",
        "message",
        "events",
        "budget",
        "demo",
        "status",
        "credential",
        "context",
        "whoami",
        "completion",
        "audit",
        "init",
        "doctor",
    ]
    .into_iter()
    .collect()
}

#[test]
fn onboarding_doc_exists() {
    assert!(doc_path().exists(), "AC-92 requires docs/onboarding.md");
}

#[test]
fn onboarding_doc_under_line_tripwire() {
    let doc = read_doc();
    let lines = doc.lines().count();
    assert!(
        lines <= 250,
        "AC-92 tripwire: docs/onboarding.md must be <= 250 lines (was {lines})"
    );
}

#[test]
fn onboarding_has_seven_sections_in_order() {
    let doc = read_doc();
    let want = [
        "## 1. Prerequisites",
        "## 2. Five commands",
        "## 3. Doctor check",
        "## 4. Add your first credential",
        "## 5. Send your first message",
        "## 6. Backup before trust",
        "## 7. Where next",
    ];
    let mut cursor = 0usize;
    for heading in &want {
        let idx = doc[cursor..]
            .find(heading)
            .unwrap_or_else(|| panic!("missing section heading: '{heading}'"));
        cursor += idx + heading.len();
    }
}

/// Extract every fenced code block ``` ... ``` (any language) and
/// scan for `pcy <verb>` invocations.
fn fenced_pcy_verbs(doc: &str) -> Vec<String> {
    let mut verbs = Vec::new();
    let mut in_fence = false;
    for line in doc.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence {
            continue;
        }
        // Find "pcy " occurrences; the next whitespace-delimited
        // token is the verb. Skip comments (#) lines wholly.
        let no_comment = line.split('#').next().unwrap_or("");
        for (i, _) in no_comment.match_indices("pcy ") {
            let rest = &no_comment[i + 4..];
            if let Some(verb) = rest.split_whitespace().next() {
                let cleaned: String = verb
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                    .collect();
                if !cleaned.is_empty() {
                    verbs.push(cleaned);
                }
            }
        }
    }
    verbs
}

#[test]
fn every_fenced_pcy_command_maps_to_a_real_clap_verb() {
    let doc = read_doc();
    let real = real_clap_verbs();
    let verbs = fenced_pcy_verbs(&doc);
    assert!(
        !verbs.is_empty(),
        "expected at least one `pcy <verb>` in fenced examples"
    );
    for v in &verbs {
        assert!(
            real.contains(v.as_str()),
            "docs/onboarding.md shows `pcy {v}` in a code block but no such top-level clap verb exists. \
             Either implement the verb or drop the example from a fenced block (prose-only references are fine)."
        );
    }
}

#[test]
fn unimplemented_verbs_appear_only_in_prose() {
    // AC-91 (backup/restore) and AC-93 (provider) are v9.1 work that
    // lands AFTER this doc — they may be referenced in prose but
    // never as runnable examples.
    let doc = read_doc();
    let fenced = fenced_pcy_verbs(&doc);
    for blocked in &["backup", "restore", "provider"] {
        assert!(
            !fenced.iter().any(|v| v == blocked),
            "`pcy {blocked}` must not appear in a fenced code block until its AC ships"
        );
    }
}

#[test]
fn onboarding_doc_advertises_doctor_strict_and_json_flags() {
    let doc = read_doc();
    assert!(
        doc.contains("--output json"),
        "operators need the JSON output documented"
    );
    assert!(
        doc.contains("--strict"),
        "the strict flag must be documented alongside CR-v91-3 carve-out"
    );
}
