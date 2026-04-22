//! AC-53 / Slice A2b.3: build_executor factory selection.

use open_pincery::config::{ResolvedSandboxMode, SandboxMode};
use open_pincery::runtime::sandbox::{build_executor, executor_kind_for, ExecutorKind};

fn resolved(kind: SandboxMode, allow_unsafe: bool) -> ResolvedSandboxMode {
    ResolvedSandboxMode {
        mode: kind,
        allow_unsafe,
    }
}

#[test]
fn disabled_mode_always_returns_process_executor() {
    assert_eq!(
        executor_kind_for(&resolved(SandboxMode::Disabled, true)),
        ExecutorKind::Process,
    );
}

#[cfg(target_os = "linux")]
#[test]
fn enforce_mode_on_linux_returns_real_sandbox() {
    assert_eq!(
        executor_kind_for(&resolved(SandboxMode::Enforce, false)),
        ExecutorKind::Real,
    );
}

#[cfg(target_os = "linux")]
#[test]
fn audit_mode_on_linux_returns_real_sandbox() {
    assert_eq!(
        executor_kind_for(&resolved(SandboxMode::Audit, false)),
        ExecutorKind::Real,
    );
}

#[cfg(not(target_os = "linux"))]
#[test]
fn non_linux_always_returns_process_executor_even_in_enforce_mode() {
    assert_eq!(
        executor_kind_for(&resolved(SandboxMode::Enforce, false)),
        ExecutorKind::Process,
    );
    assert_eq!(
        executor_kind_for(&resolved(SandboxMode::Audit, false)),
        ExecutorKind::Process,
    );
}

#[test]
fn build_executor_returns_a_usable_arc_for_disabled_mode() {
    let sandbox = resolved(SandboxMode::Disabled, true);
    let _executor = build_executor(&sandbox);
}
