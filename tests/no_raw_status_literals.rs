//! AC-34 (v6) guard: no raw SQL-style status literals anywhere under `src/`.
//!
//! After the `AgentStatus` enum landed, every CAS SQL statement must
//! interpolate from `AgentStatus::DB_*` rather than embed a single-quoted
//! literal like `'asleep'` directly. The Rust string constants use double
//! quotes (`"asleep"`) so a single-quoted occurrence is a regression.
//!
//! This static test prevents accidental drift (e.g. a future PR copy-pasting
//! an old SQL fragment) without requiring a running database.

use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN: &[&str] = &[
    "'asleep'",
    "'awake'",
    "'maintenance'",
    "'wake_acquiring'",
    "'wake_ending'",
];

fn collect_rs_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_raw_status_literals_in_src() {
    let mut files = Vec::new();
    collect_rs_files(Path::new("src"), &mut files);
    assert!(!files.is_empty(), "src/ must contain at least one .rs file");

    let mut offenders: Vec<(PathBuf, &'static str, usize)> = Vec::new();
    for path in &files {
        let contents = fs::read_to_string(path).expect("must read source file");
        for (lineno, line) in contents.lines().enumerate() {
            for pat in FORBIDDEN {
                if line.contains(pat) {
                    offenders.push((path.clone(), pat, lineno + 1));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "v6 AC-34 requires zero raw SQL status literals in src/. \
         Use AgentStatus::DB_* constants instead. Offenders:\n{}",
        offenders
            .iter()
            .map(|(p, pat, l)| format!("  {}:{} contains {}", p.display(), l, pat))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
