#!/usr/bin/env bash
# AC-77 Slice G2a — empirical seccomp allowlist corpus capture.
#
# Runs each AC-76 happy-path command under `strace -fc` and emits
# one syscall name per line to stdout (sorted, deduplicated). The
# capture is intentionally NOT run inside bwrap: the seccomp filter
# governs syscalls invoked by the user process (and its children)
# *after* bwrap's parent-side namespace setup. The userspace syscall
# set for these workloads is invariant under bwrap modulo a few
# kernel-side namespace-translation details that do not change the
# syscall *number* observed by seccomp-bpf.
#
# Usage:
#   ./scripts/capture_seccomp_corpus.sh > tests/fixtures/seccomp/observed_syscalls.txt
#
# Run inside the devshell container (or any Linux host with strace):
#   ./scripts/devshell.sh ./scripts/capture_seccomp_corpus.sh \
#       > tests/fixtures/seccomp/observed_syscalls.txt
#
# Each command is one of the 11-shape happy-path inputs the AC-76
# 12-payload escape suite drives through `RealSandbox::run`. The
# corpus is the *union* across all of them.
#
# The output is consumed by:
#   - human review (`tests/fixtures/seccomp/observed_syscalls.txt`)
#   - the AC-77 source-of-truth in `src/runtime/sandbox/seccomp.rs`
#     (`allowed_syscalls()`)
#   - the regen-on-new-tool diff-fail test
#     (`tests/seccomp_allowlist_test.rs::allowlist_matches_observed_corpus`,
#     gated on `OPEN_PINCERY_RUN_AC77_REGEN=1`).

set -euo pipefail

if ! command -v strace >/dev/null 2>&1; then
  echo "error: strace not found on PATH (install strace package)" >&2
  exit 127
fi

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

# Each entry is one command line passed to `sh -c`. The set covers:
#   - core POSIX shell exec (`echo`, `:`, `true`)
#   - text utilities used by AC-76 payload pipelines (`cat`, `head`,
#     `seq`, `tr`, `wc`)
#   - process / shell builtins through dash (`id`, `command`)
#   - pipelines that exercise `pipe2` / `dup2` / `wait4`
#   - timeout-wrapped commands (the AC-76 fork-bomb / pid-exhaustion
#     harness shape)
#
# We deliberately exclude the destructive escape payloads themselves
# (mount, unshare -U, /etc/shadow read) — those are negative-control
# fixtures, not happy-path workloads. Including them would inflate
# the allowlist with their failure-path syscalls.
COMMANDS=(
  "echo hello"
  "true"
  ":"
  "/bin/sh -c 'echo nested'"
  "cat /etc/hostname"
  "head -c 64 /dev/urandom | wc -c"
  "seq 1 5"
  "id -u"
  "command -v echo"
  "echo a | tr a-z A-Z | wc -c"
  "timeout 1s sh -c 'echo timed'"
  "dd if=/dev/zero of=/dev/null bs=1024 count=1 status=none"
)

i=0
for cmd in "${COMMANDS[@]}"; do
  log="$WORKDIR/strace.$i.log"
  # `-f` follows forks (covers pipelines and `sh -c` children).
  # `-c` emits a summary table (one row per syscall name).
  # `-o` writes the table to file; we ignore the actual stdout/stderr
  # of the workload.
  if ! strace -f -c -o "$log" /bin/sh -c "$cmd" >/dev/null 2>&1; then
    echo "warn: command failed (still capturing syscalls): $cmd" >&2
  fi
  i=$((i + 1))
done

# strace -c emits a summary table whose "syscall" column is the last
# whitespace-delimited token on lines that contain a percentage in the
# first numeric column. The simplest robust extraction is: pick lines
# whose first non-space token parses as a number (the % time column),
# then take the trailing whitespace-delimited token. The header lines
# ("% time", "------"), totals row, and any error lines are skipped.
{
  for log in "$WORKDIR"/strace.*.log; do
    awk '
      # Skip header, separators, totals.
      /^[[:space:]]*-+/ { next }
      /^[[:space:]]*%[[:space:]]*time/ { next }
      /^[[:space:]]*total[[:space:]]/ { next }
      # Lines whose first column starts with a digit or is "0.00" are
      # data rows. The last whitespace-delimited token is the syscall
      # name.
      /^[[:space:]]*[0-9]/ {
        n = $NF
        if (n == "total") next
        if (n ~ /^[a-z_][a-z0-9_]*$/) print n
      }
    ' "$log"
  done
} | sort -u
