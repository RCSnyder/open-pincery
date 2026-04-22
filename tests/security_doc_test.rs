//! AC-54 — Security Threat Model.
//!
//! Open Pincery v9 elevates sandboxing, credential handling, and
//! multi-tenant enforcement from aspirational to enforced.  AC-54
//! requires a published threat model so operators and contributors
//! share an explicit view of what the platform defends against, what
//! it explicitly does not, and how to report an issue.
//!
//! Spec (scope.md § AC-54): `docs/SECURITY.md` must exist, be linked
//! from `README.md`, and contain four sections with minimum content:
//!
//! 1. Adversary capabilities
//! 2. In-scope attacks — at minimum: prompt-injection exfil, tool-
//!    sandbox escape, credential leak via event log, session hijack,
//!    webhook replay
//! 3. Out-of-scope — at minimum: compromised host, compromised
//!    Postgres, insider with DB credentials
//! 4. Disclosure — a working email OR PGP fingerprint OR GitHub
//!    Security Advisories link
//!
//! This test enforces the structural contract by regex so the
//! document cannot silently drift out of compliance.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

#[test]
fn security_md_exists_and_is_linked_from_readme() {
    let security = repo_root().join("docs").join("SECURITY.md");
    assert!(security.exists(), "AC-54: docs/SECURITY.md must exist");

    let readme = read(&repo_root().join("README.md"));
    // Accept either a bare path reference or a markdown link.
    assert!(
        readme.contains("docs/SECURITY.md") || readme.contains("docs\\SECURITY.md"),
        "AC-54: README.md must link to docs/SECURITY.md"
    );
}

#[test]
fn security_md_has_the_four_required_headings() {
    let body = read(&repo_root().join("docs").join("SECURITY.md"));

    for heading in [
        "## Adversary Capabilities",
        "## In-Scope Attacks",
        "## Out-of-Scope",
        "## Disclosure",
    ] {
        assert!(
            body.contains(heading),
            "AC-54: docs/SECURITY.md must contain `{heading}` heading"
        );
    }
}

#[test]
fn in_scope_attacks_enumerate_the_five_required_threats() {
    let body = read(&repo_root().join("docs").join("SECURITY.md"));

    // Five required in-scope attack classes.  Matches are case-
    // insensitive and substring-based so the prose can embellish
    // around the canonical term.
    let lower = body.to_lowercase();
    for needle in [
        "prompt-injection",
        "sandbox escape",
        "credential leak",
        "session hijack",
        "webhook replay",
    ] {
        assert!(
            lower.contains(needle),
            "AC-54: In-Scope Attacks must cover `{needle}`"
        );
    }
}

#[test]
fn out_of_scope_enumerates_the_three_required_exclusions() {
    let body = read(&repo_root().join("docs").join("SECURITY.md"));
    let lower = body.to_lowercase();
    for needle in ["compromised host", "compromised postgres", "insider"] {
        assert!(
            lower.contains(needle),
            "AC-54: Out-of-Scope must cover `{needle}`"
        );
    }
}

#[test]
fn disclosure_section_provides_at_least_one_contact_channel() {
    let body = read(&repo_root().join("docs").join("SECURITY.md"));

    // Accept any one of: RFC-5322-ish email, PGP fingerprint hint,
    // or a GitHub Security Advisories link.
    let has_email = regex_like_email(&body);
    let has_pgp = body.to_lowercase().contains("pgp") || body.to_lowercase().contains("gpg");
    let has_gha = body.contains("github.com") && body.to_lowercase().contains("security");

    assert!(
        has_email || has_pgp || has_gha,
        "AC-54: Disclosure section must include an email, PGP fingerprint, or GitHub Security Advisories link"
    );
}

/// Minimal email detector — enough to satisfy AC-54's "a working email
/// address" without pulling in the `regex` crate from test code.
fn regex_like_email(body: &str) -> bool {
    for token in body.split_whitespace() {
        let stripped = token.trim_matches(|c: char| {
            !c.is_ascii_alphanumeric() && c != '@' && c != '.' && c != '-' && c != '_' && c != '+'
        });
        if let Some((local, domain)) = stripped.split_once('@') {
            if !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
            {
                return true;
            }
        }
    }
    false
}
