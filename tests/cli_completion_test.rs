//! AC-51 (v8): `pcy completion <shell>` emits a non-empty, shell-appropriate
//! completion script for each of bash, zsh, fish, and powershell.

use std::process::Command;

fn pcy_bin() -> String {
    std::env::var("CARGO_BIN_EXE_pcy").expect("pcy binary path set by cargo")
}

fn completion_for(shell: &str) -> String {
    let out = Command::new(pcy_bin())
        .args(["completion", shell])
        // No network / no config required — completion is pure stdout
        // from `clap_complete::generate`.
        .env("PCY_NO_TTY", "1")
        .output()
        .expect("spawn pcy");
    assert!(
        out.status.success(),
        "pcy completion {shell} failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("completion script is utf8")
}

#[test]
fn bash_completion_contains_signature_markers() {
    let script = completion_for("bash");
    assert!(!script.is_empty(), "bash completion is empty");
    // clap_complete always emits a `_pcy()` shell function.
    assert!(
        script.contains("_pcy()"),
        "bash completion missing _pcy() function: {script}"
    );
}

#[test]
fn zsh_completion_contains_signature_markers() {
    let script = completion_for("zsh");
    assert!(!script.is_empty(), "zsh completion is empty");
    assert!(
        script.contains("#compdef pcy"),
        "zsh completion missing #compdef directive: {script}"
    );
}

#[test]
fn fish_completion_contains_signature_markers() {
    let script = completion_for("fish");
    assert!(!script.is_empty(), "fish completion is empty");
    assert!(
        script.contains("complete -c pcy"),
        "fish completion missing complete -c pcy: {script}"
    );
}

#[test]
fn powershell_completion_contains_signature_markers() {
    let script = completion_for("powershell");
    assert!(!script.is_empty(), "powershell completion is empty");
    assert!(
        script.contains("Register-ArgumentCompleter"),
        "powershell completion missing Register-ArgumentCompleter: {script}"
    );
}

#[test]
fn completion_rejects_unknown_shell() {
    let out = Command::new(pcy_bin())
        .args(["completion", "tcsh"])
        .output()
        .expect("spawn pcy");
    assert!(!out.status.success(), "unknown shell should be rejected");
    // clap's usage-error exit code is 2.
    assert_eq!(
        out.status.code().unwrap_or(-1),
        2,
        "expected clap usage-error exit 2"
    );
}
