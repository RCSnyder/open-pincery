//! AC-36: the runtime's child-process executor is the ONLY place in
//! `src/runtime/` allowed to call `Command::new`.
//!
//! This guard prevents a future refactor from smuggling a second spawn
//! site in (bypassing env scrub, timeout, and pre-flight rejection).
//!
//! We count occurrences across `src/runtime/**/*.rs`. The only match
//! should be inside the sandbox module — either the legacy
//! `sandbox.rs` single-file layout or the Slice A2b.2 `sandbox/`
//! directory module (where `mod.rs` owns the spawn site and the
//! layer submodules `bwrap.rs`, `seccomp.rs`, `landlock.rs`,
//! `cgroup.rs`, `netns.rs` may also compose `Command` invocations).

use std::fs;
use std::path::Path;

fn collect_rs_files(root: &Path, out: &mut Vec<std::path::PathBuf>) {
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            out.push(path);
        }
    }
}

#[test]
fn only_sandbox_may_call_command_new() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("runtime");
    let mut files = Vec::new();
    collect_rs_files(&root, &mut files);
    assert!(
        !files.is_empty(),
        "expected to find runtime .rs files under {}",
        root.display()
    );

    let mut offenders: Vec<(String, usize, String)> = Vec::new();
    for f in &files {
        let contents = fs::read_to_string(f).expect("read");
        for (i, line) in contents.lines().enumerate() {
            // Skip comments and doc-strings — we only care about real calls.
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("*") {
                continue;
            }
            if line.contains("Command::new") {
                // Allow the legacy single-file module `sandbox.rs`
                // and every file inside the `sandbox/` directory
                // module. Anything else in `src/runtime/` is a
                // banned second spawn site.
                let in_sandbox_module = f
                    .components()
                    .any(|c| c.as_os_str() == "sandbox" || c.as_os_str() == "sandbox.rs");
                if !in_sandbox_module {
                    offenders.push((f.to_string_lossy().into_owned(), i + 1, line.to_string()));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "Command::new is only allowed in src/runtime/sandbox/ (or legacy sandbox.rs); \
         found outside:\n{}",
        offenders
            .iter()
            .map(|(f, l, c)| format!("  {}:{}: {}", f, l, c.trim()))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
