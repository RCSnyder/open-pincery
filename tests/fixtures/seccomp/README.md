# AC-77 Slice G2a — Empirical seccomp allowlist corpus

This directory is the fixture-of-record for the AC-77 default-deny
syscall allowlist. Per readiness `T-AC77-2` and `R-AC77-2`, the
allowlist in `src/runtime/sandbox/seccomp.rs::allowed_syscalls()` is
sourced from this corpus, not from documentation or example
allowlists.

## Files

- `observed_syscalls.txt` — sorted, deduplicated union of syscall
  names captured by `scripts/capture_seccomp_corpus.sh` against the
  AC-76 happy-path command set. One name per line.
- `additions.txt` — syscalls **not** present in `observed_syscalls.txt`
  that the allowlist must include anyway. Each entry has a one-line
  rationale. These come from two sources:
  1. `pincery-init`'s residual syscalls between
     `apply_seccomp` and `execvp` (verify-no-new-privs prctl,
     verify-fully-enforced /proc read, then `execve`).
  2. Runtime primitives that strace's summary mode failed to record
     for the smallest happy-path commands (notably `exit_group` is
     not always reported when the traced program is itself the
     summarized one) but that are mandatory for any v9 workload.

## Capture provenance

| field        | value                               |
| ------------ | ----------------------------------- |
| host         | Docker `ubuntu:24.04`               |
| kernel       | `6.6.87.2-microsoft-standard-WSL2`  |
| glibc        | `Ubuntu GLIBC 2.39-0ubuntu8.7`      |
| target arch  | `x86_64`                            |
| capture date | 2026-04-30                          |
| script       | `scripts/capture_seccomp_corpus.sh` |
| commands     | 12 happy-path inputs (see script)   |

## Update procedure

When AC-66 (Tool Catalog Expansion) lands or any new built-in tool
extends the syscall surface, re-run:

```bash
./scripts/devshell.sh ./scripts/capture_seccomp_corpus.sh \
  > tests/fixtures/seccomp/observed_syscalls.txt
```

then update `allowed_syscalls()` in `src/runtime/sandbox/seccomp.rs`
to include the new entries (and bump the `additions.txt` rationale
if any of them are no longer additions).

The drift guard
`src/runtime/sandbox/seccomp.rs::tests::allowlist_covers_observed_corpus`
(unit test, reads this corpus via `include_str!`) catches drift
between this fixture and the source-of-truth `allowed_syscalls()`.
It runs on every build with no env-var gating.
