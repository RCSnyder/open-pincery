#!/bin/bash
# Probe: reproduce landlock+bwrap EPERM failure with strace to localize syscall.
set -u
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq >/tmp/apt.log 2>&1
apt-get install -y -qq strace </dev/null >>/tmp/apt.log 2>&1
which strace || { echo "APT LOG:"; cat /tmp/apt.log; exit 1; }

BIN=$(ls /cargo-target/debug/deps/sandbox_real_smoke-* 2>/dev/null | grep -v '\.d$' | head -1)
echo "BIN=$BIN"

# Pick exact test filter, run under strace
strace -f -e trace=mount,mount_setattr,fsmount,move_mount,unshare,clone,landlock_restrict_self,landlock_add_rule,landlock_create_ruleset,prctl \
  -o /tmp/strace.out \
  "$BIN" real_sandbox_runs_trivial_true --exact --nocapture --test-threads=1 \
  >/tmp/test.out 2>&1 || true

echo "=== test stdout/err tail ==="
tail -30 /tmp/test.out
echo "=== all EPERM-returning syscalls ==="
grep EPERM /tmp/strace.out | tail -30
echo "=== all mount calls ==="
grep -E "^[0-9]+\s+mount\(|^[0-9]+\s+mount_setattr" /tmp/strace.out | tail -30
echo "=== landlock calls ==="
grep -E "landlock_" /tmp/strace.out | tail -20
