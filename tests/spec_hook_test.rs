//! AC-81 — commit-msg-spec-ref hook + devshell installer tests.
//!
//! Drives the hook end-to-end with synthetic (commit-message,
//! staged-diff) fixtures using temporary git repos. Validates the
//! devshell installer copies the hook idempotently and never
//! overwrites a user-customized hook.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn hook_source() -> PathBuf {
    workspace_root().join(".github/hooks/commit-msg-spec-ref")
}

fn devshell_script() -> PathBuf {
    workspace_root().join("scripts/devshell.sh")
}

fn coverage_doc() -> PathBuf {
    workspace_root().join("scaffolding/spec_coverage.md")
}

fn bash_path() -> String {
    if let Ok(custom) = std::env::var("OPEN_PINCERY_TEST_BASH") {
        return custom;
    }
    // On Windows the default `bash.exe` on PATH is WSL, which cannot
    // see Windows-style paths emitted by env!("CARGO_MANIFEST_DIR").
    // Prefer Git for Windows bash, which understands them.
    if cfg!(windows) {
        for candidate in [
            r"C:\Program Files\Git\bin\bash.exe",
            r"C:\Program Files\Git\usr\bin\bash.exe",
            r"C:\Program Files (x86)\Git\bin\bash.exe",
        ] {
            if std::path::Path::new(candidate).exists() {
                return candidate.to_string();
            }
        }
    }
    "bash".to_string()
}

/// Materialize a temp dir (uniquely named under target/tmp) and return
/// it. Caller is responsible for removing it on drop via TempDir.
fn make_temp_dir(label: &str) -> PathBuf {
    let base = workspace_root().join("target").join("tmp").join(format!(
        "spec-hook-{}-{}",
        label,
        std::process::id()
    ));
    let mut n = 0u32;
    loop {
        let p = base.with_extension(format!("{n}"));
        if !p.exists() {
            fs::create_dir_all(&p).expect("create temp dir");
            return p;
        }
        n += 1;
    }
}

/// Initialize a fresh git repo at `dir` with an initial commit so
/// `git diff --cached --name-only` returns sensible results.
fn git_init_repo(dir: &Path) {
    let run = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("git invocable");
        assert!(status.success(), "git {:?} failed", args);
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.email", "test@example.com"]);
    run(&["config", "user.name", "Test"]);
    fs::write(dir.join(".gitignore"), "").unwrap();
    run(&["add", ".gitignore"]);
    run(&["commit", "-q", "-m", "init"]);
}

/// Stage one or more files (creating empty content if missing) under
/// `dir` so `git diff --cached --name-only` lists them.
fn stage_files(dir: &Path, paths: &[&str]) {
    for rel in paths {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        if !p.exists() {
            fs::write(&p, "// test fixture\n").unwrap();
        } else {
            // Append a byte so the file is "modified" relative to HEAD.
            let mut content = fs::read_to_string(&p).unwrap_or_default();
            content.push_str("// touch\n");
            fs::write(&p, content).unwrap();
        }
        let status = Command::new("git")
            .args(["add", rel])
            .current_dir(dir)
            .status()
            .expect("git invocable");
        assert!(status.success(), "git add {rel} failed");
    }
}

/// Make `scaffolding/spec_coverage.md` available inside the temp repo
/// so the hook can read it via `git rev-parse --show-toplevel`.
fn copy_coverage_doc(dir: &Path) {
    let dest = dir.join("scaffolding/spec_coverage.md");
    fs::create_dir_all(dest.parent().unwrap()).unwrap();
    fs::copy(coverage_doc(), &dest).unwrap();
}

/// Write a commit message file with `body` and return its path.
fn write_msg(dir: &Path, body: &str) -> PathBuf {
    let p = dir.join("COMMIT_EDITMSG.test");
    fs::write(&p, body).unwrap();
    p
}

/// Run the hook against `msg_file` from inside `repo_dir`. Returns
/// (exit_status_success, stderr_string).
fn run_hook(repo_dir: &Path, msg_file: &Path) -> (bool, String) {
    let output = Command::new(bash_path())
        .arg(hook_source())
        .arg(msg_file)
        .current_dir(repo_dir)
        .output()
        .expect("bash invocable");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), stderr)
}

// ---------- Tests ----------

#[test]
fn rejects_runtime_change_without_trailer() {
    let dir = make_temp_dir("reject_runtime_no_trailer");
    git_init_repo(&dir);
    copy_coverage_doc(&dir);
    stage_files(&dir, &["src/runtime/widget.rs"]);
    let msg = write_msg(&dir, "feat(runtime): add a widget\n\nNo trailer.\n");
    let (ok, stderr) = run_hook(&dir, &msg);
    assert!(
        !ok,
        "hook MUST reject runtime edit with no canonical_action trailer; stderr was: {stderr}"
    );
    assert!(
        stderr.contains("REJECTED"),
        "rejection message expected; got: {stderr}"
    );
}

#[test]
fn accepts_runtime_change_with_valid_trailer() {
    let dir = make_temp_dir("accept_runtime_valid");
    git_init_repo(&dir);
    copy_coverage_doc(&dir);
    stage_files(&dir, &["src/runtime/widget.rs"]);
    let msg = write_msg(
        &dir,
        "feat(runtime): real change\n\nBody.\n\ncanonical_action=AuthorizeExecution\n",
    );
    let (ok, stderr) = run_hook(&dir, &msg);
    assert!(
        ok,
        "hook MUST accept runtime edit with valid canonical_action trailer; stderr was: {stderr}"
    );
}

#[test]
fn accepts_docs_only_commit() {
    let dir = make_temp_dir("accept_docs_only");
    git_init_repo(&dir);
    copy_coverage_doc(&dir);
    // Stage a docs-only edit — must not require a trailer.
    stage_files(&dir, &["docs/notes.md"]);
    let msg = write_msg(&dir, "docs: tweak\n\nNo trailer needed.\n");
    let (ok, stderr) = run_hook(&dir, &msg);
    assert!(
        ok,
        "hook MUST accept docs-only commit without trailer; stderr: {stderr}"
    );
}

#[test]
fn rejects_unknown_canonical_action() {
    let dir = make_temp_dir("reject_unknown_action");
    git_init_repo(&dir);
    copy_coverage_doc(&dir);
    stage_files(&dir, &["src/api/handler.rs"]);
    let msg = write_msg(&dir, "feat(api): edit\n\ncanonical_action=NotARealAction\n");
    let (ok, stderr) = run_hook(&dir, &msg);
    assert!(
        !ok,
        "hook MUST reject unknown canonical_action; stderr: {stderr}"
    );
    assert!(
        stderr.contains("NotARealAction") || stderr.contains("REJECTED"),
        "rejection should name the offending action; got: {stderr}"
    );
}

#[test]
fn devshell_installs_hook_idempotently() {
    let dir = make_temp_dir("devshell_install");
    git_init_repo(&dir);
    // Mirror the source layout the installer expects.
    let mirror_hook = dir.join(".github/hooks/commit-msg-spec-ref");
    fs::create_dir_all(mirror_hook.parent().unwrap()).unwrap();
    fs::copy(hook_source(), &mirror_hook).unwrap();
    let mirror_script = dir.join("scripts/devshell.sh");
    fs::create_dir_all(mirror_script.parent().unwrap()).unwrap();
    fs::copy(devshell_script(), &mirror_script).unwrap();

    // Source the script to invoke the installer without launching docker.
    // The function `install_commit_msg_hook` is defined when the script
    // is sourced.
    let installer = format!(
        ". \"{}\" --version-check >/dev/null 2>&1 || true; \
         install_commit_msg_hook \"{}\"",
        mirror_script.to_string_lossy().replace('\\', "/"),
        dir.to_string_lossy().replace('\\', "/")
    );

    let run_install = || -> bool {
        let out = Command::new(bash_path())
            .arg("-c")
            .arg(&installer)
            .current_dir(&dir)
            .output()
            .expect("bash invocable");
        out.status.success()
    };

    // Run 1 — installs.
    assert!(run_install(), "first install run should succeed");
    let installed = dir.join(".git/hooks/commit-msg");
    assert!(installed.exists(), "hook should have been installed");
    let installed_bytes_1 = fs::read(&installed).unwrap();
    let source_bytes = fs::read(&mirror_hook).unwrap();
    assert_eq!(
        installed_bytes_1, source_bytes,
        "installed hook must match source byte-for-byte"
    );

    // Run 2 — no-op (idempotent).
    assert!(run_install(), "second install run should succeed");
    let installed_bytes_2 = fs::read(&installed).unwrap();
    assert_eq!(
        installed_bytes_1, installed_bytes_2,
        "second run must not modify an already-installed hook"
    );

    // Customize the hook and re-run — the installer must NOT overwrite.
    let custom = b"#!/usr/bin/env bash\nexit 0  # user override\n";
    fs::write(&installed, custom).unwrap();
    assert!(run_install(), "third install run should succeed");
    let after = fs::read(&installed).unwrap();
    assert_eq!(
        after, custom,
        "installer must NEVER overwrite a user-customized commit-msg hook"
    );
}
