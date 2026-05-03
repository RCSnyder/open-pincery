//! AC-52b (v8): clap command-tree lint.
//!
//! Walks `Cli::command()` and enforces project-wide CLI conventions:
//!
//! 1. Every subcommand (at any depth) has a non-empty `about` or
//!    `long_about` string. No mystery commands.
//! 2. The legacy `--format` flag does not exist anywhere — only
//!    `--output` is allowed for format selection (AC-47).
//! 3. `--yes` exists only on the explicitly allowlisted destructive
//!    paths; its presence elsewhere is a smell that secrets or data
//!    could be silently mutated.
//! 4. `--output` is declared on the root as a global flag so every
//!    data leaf inherits it. Enforced by confirming the argument
//!    exists and is marked `global`.

use clap::CommandFactory;
use open_pincery::cli::Cli;

/// Commands that are allowed to expose `--yes` for destructive
/// confirmation. Paths are space-joined subcommand names rooted at
/// `pcy`, e.g. `"credential revoke"`.
const YES_ALLOWLIST: &[&str] = &["credential revoke"];

fn walk<'a>(
    cmd: &'a clap::Command,
    path: &mut Vec<&'a str>,
    out: &mut Vec<(String, &'a clap::Command)>,
) {
    path.push(cmd.get_name());
    out.push((path.join(" "), cmd));
    for sub in cmd.get_subcommands() {
        walk(sub, path, out);
    }
    path.pop();
}

fn all_commands() -> Vec<(String, clap::Command)> {
    let root = Cli::command();
    let mut path = Vec::new();
    let mut out = Vec::new();
    walk(&root, &mut path, &mut out);
    // Clone commands so the returned Vec owns them (the walker holds
    // borrows into `root`, which dies at end of scope).
    out.into_iter().map(|(p, c)| (p, c.clone())).collect()
}

#[test]
fn every_subcommand_has_about() {
    let mut missing = Vec::new();
    for (path, cmd) in all_commands() {
        // Skip the root — `#[command(name = "pcy")]` has no derive
        // about, and its purpose is self-evident from the binary name.
        if !path.contains(' ') {
            continue;
        }
        if cmd.get_about().is_none() && cmd.get_long_about().is_none() {
            missing.push(path);
        }
    }
    assert!(
        missing.is_empty(),
        "subcommands missing `about`/`long_about` (add a doc comment or #[command(about = \"…\")]): {missing:?}"
    );
}

#[test]
fn no_format_flag_anywhere() {
    let mut offenders = Vec::new();
    for (path, cmd) in all_commands() {
        for arg in cmd.get_arguments() {
            if arg.get_long() == Some("format") || arg.get_short() == Some('f') {
                // `-f` on its own is not banned; only `--format`. We
                // keep the short-flag check advisory by only flagging
                // when the long form is `format`.
                if arg.get_long() == Some("format") {
                    offenders.push(format!("{path} --format"));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "--format is banned (AC-47 mandates --output only): {offenders:?}"
    );
}

#[test]
fn yes_flag_only_on_allowlisted_paths() {
    let mut offenders = Vec::new();
    for (path, cmd) in all_commands() {
        for arg in cmd.get_arguments() {
            if arg.get_long() != Some("yes") {
                continue;
            }
            // Strip the leading "pcy " for allowlist comparison.
            let sub = path.strip_prefix("pcy ").unwrap_or(&path);
            if !YES_ALLOWLIST.contains(&sub) {
                offenders.push(path.clone());
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "--yes found outside the allowlist {YES_ALLOWLIST:?}: {offenders:?}"
    );
}

#[test]
fn output_is_global_on_root() {
    let root = Cli::command();
    let arg = root
        .get_arguments()
        .find(|a| a.get_long() == Some("output"))
        .expect("root `pcy` must declare --output");
    assert!(
        arg.is_global_set(),
        "--output must be global so every leaf command inherits it"
    );
}

#[test]
fn no_color_is_global_on_root() {
    let root = Cli::command();
    let arg = root
        .get_arguments()
        .find(|a| a.get_long() == Some("no-color"))
        .expect("root `pcy` must declare --no-color");
    assert!(arg.is_global_set(), "--no-color must be global (AC-47)");
}
