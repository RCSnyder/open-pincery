# Sandbox Architecture Audit — Principal Engineer Review

**Status:** DRAFT (decision required before implementation)
**Reviewer:** Distinguished principal security engineer perspective
**Date:** 2026
**Scope:** `src/runtime/sandbox/{bwrap,landlock,seccomp,cgroup}.rs`
**Trigger:** PR #4 sandbox CI failing with `bwrap: Failed to make / slave: Operation not permitted` on Ubuntu 24.04 (kernel 6.8) and WSL2 (kernel 6.6) when `SandboxProfile { landlock: true, seccomp: true }`.

> **Update 2026-05-01 (RECONCILE):** Finding **§3.1 (seccomp denylist)** is **RESOLVED by AC-77** — commit `5982ab3` on branch `v6-01_implementation` shipped a default-deny allowlist (≈58 entries sourced empirically + manually-justified additions), `clone` argument-filter blocking `CLONE_NEWUSER | CLONE_NEWNS`, `ESCAPE_PRIMITIVES` negative control, `ALLOWLIST_SIZE_FLOOR/CEILING` install-time guards, and `mismatch_action=KillProcess` (SIGSYS exit 159) in `Enforce` mode / `Log` in `Audit` mode. The `denied_syscalls()` symbol no longer exists; `allowed_syscalls()` + `clone_arg_rules()` are the new primitives in `src/runtime/sandbox/seccomp.rs`. SIGSYS terminations emit a `sandbox_syscall_denied` event via `src/observability/seccomp_audit.rs`.

---

## TL;DR

The current sandbox installs Landlock on the **parent process** via `Command::pre_exec`, then `execve`s `bwrap`. This is **architecturally inverted** and is the root cause of the CI breakage. Per kernel.org documentation:

> Threads sandboxed with filesystem restrictions cannot modify filesystem topology, whether via `mount(2)` or `pivot_root(2)`.
> — [Landlock kernel docs, Current Limitations](https://docs.kernel.org/userspace-api/landlock.html)

> Every new thread resulting from `clone(2)` inherits Landlock domain restrictions from its parent.
> — [Landlock kernel docs, Inheritance](https://docs.kernel.org/userspace-api/landlock.html)

Combined: bwrap inherits the parent's Landlock domain, then EPERMs on its very first `mount(NULL, "/", MS_SLAVE | MS_REC, NULL)` call because Landlock V1 unconditionally denies `mount(2)` to any sandboxed thread. **Removing `/` from the allowlist did not fix it because the issue is `mount(2)` denial, not path-resolution denial.**

The fix is a small **`pincery-init` exec wrapper** that runs _inside_ the bwrap sandbox, applies Landlock + seccomp + capability drop _after_ bwrap completes mount setup, then `execve`s the user command. This is the canonical pattern used by `flatpak`, `firejail`, and the official rust-landlock `sandboxer.rs` example. This document also identifies **four additional defects** in the current architecture that must be addressed for an industry-leading agentic-OS posture.

---

## 1. What we have today

### 1.1 Defense layers (claimed in code comments)

| #   | Layer                                             | Mechanism                                                            | Status                                     |
| --- | ------------------------------------------------- | -------------------------------------------------------------------- | ------------------------------------------ |
| 1   | Mount/PID/IPC/UTS/cgroup namespace isolation      | `bwrap --unshare-{user,pid,ipc,uts,cgroup-try,net}`                  | ✅ implemented                             |
| 2   | Resource quota                                    | cgroup v2 `pincery-<uuid>` (memory/pids/cpu) attached post-spawn     | ✅ implemented                             |
| 3   | Syscall filtering                                 | seccomp-bpf **default-deny allowlist** (~58 syscalls + `clone` arg-filter) injected via `--seccomp <fd>` | ✅ RESOLVED by AC-77 (was denylist anti-pattern; see §3.1)
| 4   | Path-based MAC                                    | Landlock V1 PathBeneath rules installed in parent `pre_exec`         | ❌ broken — see §2                         |
| 5   | UID/GID/capability drop                           | not implemented                                                      | ❌ missing                                 |
| 6   | L7 egress allowlist (slirp4netns + envoy/HAProxy) | not implemented                                                      | ❌ missing                                 |

### 1.2 Default profile (`src/runtime/sandbox/mod.rs`)

```rust
SandboxProfile { seccomp: true, landlock: true, deny_net: true,
                 env_allowlist: ["PATH"], timeout: 30s }
```

### 1.3 Process topology (current — broken)

```
pincery-server (uid=N, no caps)
  └── tokio::spawn -> Command::new("bwrap") with pre_exec(install_landlock)
        ├── pre_exec runs in CHILD, post-fork, pre-execve:
        │     prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)
        │     landlock_restrict_self(ruleset_fd, 0)
        │     // child now in Landlock domain D1
        ├── execve("bwrap", argv)
        └── bwrap (in Landlock domain D1, NO_NEW_PRIVS=1)
              └── unshare(CLONE_NEWUSER | CLONE_NEWNS | ...)
                    └── mount(NULL, "/", MS_SLAVE | MS_REC, NULL)  ⇐ EPERM
```

### 1.4 Process topology (proposed — see §4)

```
pincery-server
  └── Command::new("bwrap")  (no pre_exec)
        └── execve("bwrap", argv) including --ro-bind /proc/self/exe /pincery-init
              └── bwrap performs full mount setup, pivot_root, etc.
                    └── execve("/pincery-init", original_cmd)
                          └── pincery-init: prctl(NO_NEW_PRIVS); landlock_restrict_self();
                                            seccomp_load(); capset(empty); setresuid(nobody);
                                            execve(original_cmd)
                                └── user command (in domain D1, no caps, nobody, seccomp+landlock)
```

---

## 2. Root cause — definitive, primary-source backed

### 2.1 Kernel constraint (Linux 5.13–6.x, all Landlock ABI versions to date)

From [`docs.kernel.org/userspace-api/landlock.html`](https://docs.kernel.org/userspace-api/landlock.html), §"Current limitations":

> **Filesystem topology modification.** Threads sandboxed with filesystem restrictions cannot modify filesystem topology, whether via `mount(2)` or `pivot_root(2)`. However, `chroot(2)` calls are not denied.

This is **not configurable** by the userspace ruleset. Adding or removing PathBeneath rules has zero effect on this gate. The check happens in `security/landlock/fs.c::hook_sb_mount` and is purely "is this thread in any Landlock domain? → EPERM".

### 2.2 Inheritance constraint

From the same document, §"Inheritance":

> Every new thread resulting from `clone(2)` inherits Landlock domain restrictions from its parent. […] When a thread sandboxes itself, we have the guarantee that the related security policy will stay enforced on all this thread's descendants.

Therefore: **a Landlock domain installed before `execve("bwrap")` is unconditionally inherited by bwrap and all of bwrap's `clone()`d setup children.**

### 2.3 Bwrap's mount calls (`bubblewrap.c`, current main)

Bwrap's setup sequence (verified by reading [`containers/bubblewrap/bubblewrap.c`](https://github.com/containers/bubblewrap/blob/main/bubblewrap.c)) makes at minimum the following `mount(2)` calls in the sandboxed child:

1. `mount(NULL, "/", NULL, MS_SILENT | MS_SLAVE | MS_REC, NULL)` — propagation flip. **First mount call. EPERMs immediately.**
2. `mount("tmpfs", "/newroot", "tmpfs", MS_NODEV | MS_NOSUID, NULL)` — root tmpfs.
3. Per `--ro-bind` arg: `mount(src, dest, NULL, MS_BIND | MS_REC, NULL)` then `mount(NULL, dest, NULL, MS_REMOUNT | MS_BIND | MS_RDONLY, NULL)`.
4. `mount("proc", "...", "proc", MS_NOSUID | MS_NOEXEC | MS_NODEV, NULL)`.
5. `pivot_root(".", ".")` — also blocked by Landlock.

Each of these is essential to constructing the sandbox. Landlock V1 must be installed **after** all of them complete.

### 2.4 Why the previous `/` allowlist "fix" appeared to work, then didn't

Commit `85b0bd7` added `/` to `ROOTFS_RX_PATHS`. CI initially passed with the cached binary built before commit `94886d8` (which introduced landlock at all). When the binary was rebuilt with the `/` change, all 4 real_smoke tests failed identically — because **the EPERM is from `mount(2)` denial, not from path resolution**, so the allowlist contents are irrelevant. Reverted.

### 2.5 Confirmation

From the `landlock_restrict_self(2)` man page:

> In order to enforce a ruleset, either the caller must have `CAP_SYS_ADMIN` in its user namespace, or the thread must already have the `no_new_privs` bit set.

We set `NO_NEW_PRIVS`. That is necessary for `landlock_restrict_self` to succeed. It is _not_ the cause of the bwrap mount EPERM (NO_NEW_PRIVS does not gate `mount(2)` directly — it gates suid escalation and `execve` setid behavior).

---

## 3. Findings beyond the immediate bug

A principal-engineer review surfaces five further issues. Each is rated by severity and mapped to a primary source.

### 3.1 [CRITICAL] Seccomp is a **denylist**, not an allowlist

> **✅ RESOLVED in AC-77 (2026-05-01, commits `5982ab3`..`8770751`).** `denied_syscalls()` removed. New `allowed_syscalls()` returns ~70 entries (41 empirical from `tests/fixtures/seccomp/observed_syscalls.txt` + 28 manually-justified in `additions.txt` covering dash/coreutils, glibc-2.39 modern syscalls, and pincery-init Rust residuals between `apply_seccomp` and `execvp`); `clone_arg_rules()` masks `CLONE_NEWUSER | CLONE_NEWNS`; `clone3` namespace lockout is delegated to AC-86 (`bwrap --disable-userns` + `--cap-drop ALL` + UID drop) per readiness `T-AC77-4`. `mismatch_action=KillProcess` (SIGSYS exit 159) in `Enforce`, `Log` in `Audit`. `ESCAPE_PRIMITIVES` (19 syscalls incl. `bpf`, `mount`, `umount2`, `pivot_root`, `*_module`, `kexec_*`, `reboot`, `ptrace`, `io_uring_*`, `perf_event_open`, `name_to_handle_at`, `open_by_handle_at`, `fanotify_init`, `fanotify_mark`) is asserted absent at install time. `ALLOWLIST_SIZE_FLOOR=40 / CEILING=120` guards reject install when the allowlist drifts. SIGSYS termination emits a `sandbox_syscall_denied` event (AUDIT_SECCOMP correlation deferred to G2c.2). The historical analysis below is retained for record.

**Code (historical):** `src/runtime/sandbox/seccomp.rs::denied_syscalls()` blocks 11 syscalls; `mismatch_action=Allow`.

**Why it's a critical defect:**

- The syscall surface on x86_64 is ~360 syscalls. Denying 11 leaves ~349 attack-surface entries.
- Every kernel CVE in the last 5 years (e.g., CVE-2022-0185 fsconfig, CVE-2022-2588 cls_route, CVE-2023-0386 overlayfs, CVE-2024-1086 nf_tables) was exploitable via syscalls **not** in our denylist.
- The code itself acknowledges this: `// FIXME: switch to allowlist in next sub-slice`.

**Authoritative guidance:** Docker's default seccomp profile and systemd's `SystemCallFilter=` both use **allowlists**. Chrome's sandbox uses an allowlist. Bottlerocket, gVisor, Kata, Firecracker all use allowlists. Denylist seccomp is universally regarded as security theater.

**Reference:** [`seccomp(2)` man page](https://man7.org/linux/man-pages/man2/seccomp.2.html) §NOTES: "It is recommended to apply seccomp filters in conjunction with no*new_privs, and only to syscalls that an application is \_known* to need."

### 3.2 [HIGH] `RulesetStatus::PartiallyEnforced` is silently accepted

**Code:** `src/runtime/sandbox/landlock.rs` accepts `FullyEnforced | PartiallyEnforced`, errors only on `NotEnforced`.

**Why it matters:** From the rust-landlock crate docs ([`docs.rs/landlock`](https://docs.rs/landlock/latest/landlock/) §"Test strategy"):

> Developers should test their sandboxed applications with a kernel that supports all requested Landlock features and check that `restrict_self()` returns a status matching `Ok(RestrictionStatus { ruleset: RulesetStatus::FullyEnforced, no_new_privs: true })`.

`PartiallyEnforced` means the running kernel does not support some access right we asked for (e.g., `LANDLOCK_ACCESS_FS_REFER` on ABI < 2, `TRUNCATE` on ABI < 3, `IOCTL_DEV` on ABI < 5). Silently degrading is **fail-open** for the missing axis. For an "industry leading agentic OS" we need either:

- (a) `HardRequirement` mode: refuse to start if ABI < 5; surface it as a deployment requirement.
- (b) Tier the requirement: `FullyEnforced` for prod, `PartiallyEnforced` only with explicit operator opt-in + structured warning.

### 3.3 [HIGH] No capability drop, no UID/GID change

**Code:** `bwrap.rs::build_bwrap_args` does not pass `--uid`, `--gid`, or `--cap-drop`.

**Why it matters:** Inside the bwrap user-namespace, the sandboxed process starts as `uid=real_uid` (typically the pincery service account) with **all 39 capabilities effective in the new userns**. While userns capabilities don't translate to host capabilities, several attack paths exist:

- `CAP_NET_RAW` in the new userns + `slirp4netns` egress would allow ARP/ICMP spoofing inside the sandbox network if egress ever materializes.
- `CAP_SYS_ADMIN` in the new userns enables further nested namespace creation, which can be a kernel attack surface (CVE-2022-0185, CVE-2023-32233).
- `CAP_SYS_PTRACE` in-userns plus a shared PID namespace would be exploitable; we already isolate PID, but the cap is still present.

**Industry pattern:** Run as `nobody:nogroup` (uid=65534) inside the sandbox with the empty capability set. Bwrap supports this directly: `--uid 65534 --gid 65534 --cap-drop ALL`.

### 3.4 [MEDIUM] No kernel ABI floor enforcement

**Code:** No check that the running kernel supports Landlock ABI ≥ N.

**Why it matters:** ABI 1 (Linux 5.13) cannot restrict file linking/renaming, truncation, or IOCTLs. ABI 4 (5.19) is needed for network port filtering. ABI 6 (6.7) is needed for abstract UNIX socket scoping (relevant for D-Bus escape) and signal scoping. An "industry leading" posture demands ABI ≥ 6 in prod, with an explicit floor in `preferences.md` or `scope.md`.

### 3.5 [MEDIUM] Single Landlock domain, no per-tool further restriction

The architecture installs one ruleset before exec. The kernel allows up to 16 stacked rulesets per thread. A defense-in-depth design would let an "outer" tool (e.g., the LLM agent shell) install a permissive ruleset, then have each invoked subtool further constrain itself with a tighter ruleset before its own work. This composes well with a future per-tool capability declaration (`tool.yaml: { fs: { read: ["/workspace"], write: ["/workspace/output"] } }`).

### 3.6 [LOW] No audit-log integration

ABI ≥ 7 supports `LANDLOCK_RESTRICT_SELF_LOG_NEW_EXEC_ON` which causes the kernel to emit `AUDIT_LANDLOCK_*` records when the sandboxed process attempts a denied operation. We currently set `flags=0`. Wiring kernel audit → our event log gives forensic visibility into "what did the agent try to do" — invaluable for an agentic OS.

---

## 4. Proposed architecture

### 4.1 Design principles

1. **Restrictions installed by the _innermost_ trustworthy code.** Bwrap builds the namespace; a tiny pincery-init wrapper inside the sandbox locks it down; the user command runs under those locks.
2. **Allowlist, not denylist, for syscalls.** Generated from a profile per agent/tool class.
3. **Fully enforced or refuse to start.** No silent degradation.
4. **Explicit kernel ABI floor.** Documented and asserted at server boot, not per-call.
5. **Composition-friendly.** Per-tool ruleset stacking is enabled, not blocked.

### 4.2 Concrete process model

```
pincery-server
  ├── At startup: assert_landlock_abi(min=6); assert_seccomp_bpf();
  │              fail to start if kernel doesn't meet floor.
  └── per RealSandbox::run():
        Command::new(bwrap_path)
            .args(build_bwrap_args(profile))
            // No pre_exec landlock install.
            // Bwrap argv includes:
            //   --ro-bind /usr/lib/pincery/pincery-init /sandbox/init
            //   --uid 65534 --gid 65534 --cap-drop ALL
            //   --seccomp <denylist_fd>      // wide net for bwrap setup itself
            //   /sandbox/init <original_argv...>
            .spawn()
        ├── bwrap performs: unshare, uid_map, mount, pivot_root, drop caps
        └── execve("/sandbox/init", original_argv)
              ├── pincery-init reads its embedded policy:
              │     - landlock ruleset (FS rx + rw paths from profile)
              │     - seccomp allowlist BPF (per profile_class)
              ├── prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)
              ├── landlock_restrict_self(ruleset_fd,
              │       LANDLOCK_RESTRICT_SELF_LOG_NEW_EXEC_ON | TSYNC)
              ├── prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, &allowlist)
              ├── verify RestrictionStatus::FullyEnforced; exit(2) if not
              └── execve(original_argv[0], original_argv)
```

### 4.3 `pincery-init` design

A separate binary (`src/bin/pincery-init.rs`) compiled as `staticc` (musl + `panic=abort`) so it can be `--ro-bind`ed into any sandbox without dragging in glibc dependencies. Surface area:

- **Input:** policy passed via `--policy-fd <N>` (a memfd with a JSON-serialized `SandboxInitPolicy`, via `serde_json`) — this avoids env-variable smuggling and keeps the policy out of `/proc/self/environ`.
- **Behavior:** apply prctl + landlock + seccomp + (optional) `setuid`/`setgid`/`setgroups([])` if not already done, verify enforcement, `execvp(args)`.
- **Failure mode:** any error → write structured JSON to fd 3 (sync pipe owned by the parent) → `_exit(125)` (distinguishable from user-command exit codes).

### 4.4 Seccomp allowlist generation

Replace `denied_syscalls()` with profile-driven allowlist. Recommended starting set (modeled on Docker's default + agentic-OS additions):

- `read, write, openat, close, fstat, newfstatat, lseek, mmap, mprotect, munmap, brk, rt_sigaction, rt_sigprocmask, rt_sigreturn, ioctl, pread64, pwrite64, readv, writev, access, faccessat2, pipe2, dup, dup3, getpid, getppid, getuid, geteuid, getgid, getegid, getpgrp, getpgid, getsid, gettid, gettimeofday, clock_gettime, clock_nanosleep, nanosleep, sched_yield, futex, getrandom, exit, exit_group, wait4, waitid, kill, tgkill, fcntl, fadvise64, fchdir, getcwd, getdents64, lstat, readlink, readlinkat, statx, statfs, prlimit64, rseq, set_robust_list, set_tid_address, sigaltstack, prctl(subset), arch_prctl, execve, execveat, clone(restricted_flags), clone3(restricted_flags), epoll_create1, epoll_ctl, epoll_pwait, eventfd2, signalfd4, poll, ppoll, select, pselect6`.

Network syscalls allowed only when `profile.deny_net == false`: `socket, bind, listen, accept4, connect, getsockname, getpeername, sendto, recvfrom, sendmsg, recvmsg, sendmmsg, recvmmsg, shutdown, getsockopt, setsockopt`.

Hard-deny via `SECCOMP_RET_KILL_PROCESS` (not `KILL_THREAD`, not `ERRNO`):

- `clone`/`clone3` with `CLONE_NEWUSER|CLONE_NEWNS` (user-arg filtering, BPF cmpand on flags).
- `bpf, perf_event_open, kexec_*, finit_module, init_module, delete_module, mount, umount2, pivot_root, swapon, swapoff, reboot, settimeofday, adjtimex, clock_settime, ptrace, process_vm_{readv,writev}, kcmp, syslog, acct, quotactl, lookup_dcookie, sysfs, _sysctl, nfsservctl, vhangup, modify_ldt, vm86old, vm86, create_module, get_kernel_syms, query_module, uselib, ustat, set_mempolicy, mbind, get_mempolicy, migrate_pages, move_pages, set_thread_area, get_thread_area, io_setup, io_destroy, io_submit, io_cancel, io_getevents, io_uring_*, name_to_handle_at, open_by_handle_at, fanotify_init, fanotify_mark`.

`SECCOMP_RET_KILL_PROCESS` is preferred over `RET_ERRNO(EPERM)` because it removes any opportunity for the program to handle the failure and pivot to a different exploit attempt.

### 4.5 Kernel ABI floor

```rust
// src/runtime/sandbox/preflight.rs (new)
pub fn assert_kernel_floor() -> Result<(), SandboxBootError> {
    let abi = ABI::new_current();      // landlock crate
    if (abi as u32) < 6 {
        return Err(SandboxBootError::LandlockTooOld { found: abi as u32, required: 6 });
    }
    // Also probe: seccomp-bpf availability, cgroup v2, user_namespaces=1, bwrap binary present + version >= 0.8.
    Ok(())
}
```

Called once at server startup, fail-fast. `preferences.md` and `DELIVERY.md` then explicitly list "Linux ≥ 6.7" as a runtime requirement.

### 4.6 Landlock ruleset construction

```rust
// Set CompatLevel::HardRequirement for production.
let abi = ABI::V6;  // matches floor
let mut ruleset = Ruleset::default()
    .set_compatibility(CompatLevel::HardRequirement)
    .handle_access(AccessFs::from_all(abi))?
    .handle_access(AccessNet::from_all(abi))?       // bind/connect TCP scoping
    .scope(Scope::AbstractUnixSocket | Scope::Signal)?  // ABI 6
    .create()?;

for path in &profile.fs_rx { ruleset.add_rules(path_beneath_rules([path], AccessFs::from_read(abi)))?; }
for path in &profile.fs_rw { ruleset.add_rules(path_beneath_rules([path], AccessFs::from_all(abi)))?; }

let status = ruleset.restrict_self()?;
if status.ruleset != RulesetStatus::FullyEnforced || !status.no_new_privs {
    return Err(SandboxInitError::NotFullyEnforced(status));
}
```

### 4.7 Layer 5 — capability drop & UID change

Achieved entirely through bwrap flags, no extra code:

```
--uid 65534 --gid 65534 --cap-drop ALL --unshare-user
```

plus pincery-init double-checks: `setresuid(65534, 65534, 65534); setresgid(65534, 65534, 65534); setgroups(0, NULL); capset(empty)` — defense in depth.

### 4.8 Layer 6 — egress (deferred to a later slice; design noted)

When network is enabled, attach the sandbox to a `slirp4netns` instance that itself has a fixed L7 allowlist (HAProxy or Envoy in front). This keeps the kernel's net stack out of the sandbox while allowing controlled HTTPS egress to a small set of agent-relevant endpoints. **Not in scope for this audit's initial fix.**

---

## 5. Threat model & mitigation matrix

| Threat                                        | Today                                                  | Proposed                                                          | Notes             |
| --------------------------------------------- | ------------------------------------------------------ | ----------------------------------------------------------------- | ----------------- |
| Container/sandbox escape via `mount` syscall  | denylist blocks (Allow on miss is concerning)          | seccomp KILL_PROCESS on mount                                     | hardened          |
| Kernel exploit via untracked syscall          | exposed (denylist)                                     | minimized (allowlist)                                             | the biggest delta |
| Filesystem path escape (`../`, symlinks)      | landlock prevents, but only PartiallyEnforced accepted | landlock FullyEnforced or refuse                                  | hardened          |
| Privilege escalation via setuid binary        | NO_NEW_PRIVS set                                       | NO_NEW_PRIVS set + uid=nobody + cap-drop                          | hardened          |
| User-namespace nesting (CVE-2023-32233 class) | possible (CAP_SYS_ADMIN in userns)                     | seccomp blocks `clone(CLONE_NEWUSER)`                             | hardened          |
| Abstract UNIX socket → host D-Bus             | not blocked                                            | `LANDLOCK_SCOPE_ABSTRACT_UNIX_SOCKET` (ABI 6)                     | hardened          |
| Signal injection to host PID                  | partial (PID ns)                                       | + `LANDLOCK_SCOPE_SIGNAL`                                         | hardened          |
| Resource exhaustion                           | cgroup memory/pids/cpu                                 | unchanged                                                         | already good      |
| ptrace of host processes                      | seccomp blocks ptrace                                  | unchanged                                                         | already good      |
| BPF program load                              | seccomp blocks bpf                                     | unchanged                                                         | already good      |
| io_uring (CVE-2024-0582 class)                | NOT blocked today                                      | seccomp KILL*PROCESS on io_uring*\*                               | new               |
| Network egress to arbitrary host              | bwrap --unshare-net (full deny)                        | slirp4netns + L7 allowlist when needed                            | future slice      |
| Audit/forensics ("what did the agent try?")   | none                                                   | LANDLOCK_RESTRICT_SELF_LOG_NEW_EXEC_ON → kernel audit → event log | new               |

---

## 6. Migration plan

1. **`pincery-init` binary** — new `src/bin/pincery-init.rs`, musl static build target, no_std-ish minimal deps (`libc`, `landlock`, `seccompiler`).
2. **`SandboxInitPolicy` IPC** — serde_json-serialized policy on memfd, fd passed via bwrap into the sandbox.
3. **`build_bwrap_args` rewrite** — add `--uid 65534 --gid 65534 --cap-drop ALL`, `--ro-bind <pincery-init-path> /sandbox/init`, `--ro-bind <policy-memfd> /sandbox/policy`. Replace user command with `["/sandbox/init", "--policy-fd", "3", "--", original...]`.
4. **Remove `pre_exec` landlock install** — delete the failing path; landlock now installed inside `pincery-init`.
5. **Seccomp rewrite** — replace `denied_syscalls()` with `allowed_syscalls()`. Add per-profile-class generation. Default mismatch action: `KillProcess`.
6. **Kernel floor preflight** — `assert_kernel_floor()` called from `pincery-server` startup.
7. **Test matrix** — keep `OPEN_PINCERY_SKIP_REAL_BWRAP` gate; add a new `tests/sandbox_escape_suite.rs` with the 12-payload escape test (AC-76) covering: mount, pivot_root, ptrace, BPF load, io_uring, fanotify, /proc/sysrq-trigger, abstract UNIX socket, signal cross-PID-ns, kernel module load, fs path escape via symlink, capability use of CAP_NET_RAW.
8. **Documentation** — update `preferences.md` to declare Linux ≥ 6.7 floor; update `DELIVERY.md` "known limitations" with the kernel requirement; add `docs/security/threat-model.md` with the matrix from §5.

Each step is independently committable with green CI; the whole migration is roughly one BUILD slice per numbered item.

---

## 7. References (primary sources)

- [Landlock kernel docs (kernel.org)](https://docs.kernel.org/userspace-api/landlock.html) — definitive on `mount(2)` denial, inheritance, ABI versions, scoping.
- [`landlock_restrict_self(2)` man page](https://man7.org/linux/man-pages/man2/landlock_restrict_self.2.html) — NO_NEW_PRIVS requirement, error semantics.
- [`landlock(7)` man page](https://man7.org/linux/man-pages/man7/landlock.7.html) — overview, design intent.
- [rust-landlock crate docs](https://docs.rs/landlock/latest/landlock/) — `CompatLevel::HardRequirement`, test strategy expecting `FullyEnforced`.
- [`landlock-lsm/rust-landlock/examples/sandboxer.rs`](https://github.com/landlock-lsm/rust-landlock/blob/master/examples/sandboxer.rs) — canonical "small wrapper installs landlock then execs" pattern.
- [`containers/bubblewrap/bubblewrap.c`](https://github.com/containers/bubblewrap/blob/main/bubblewrap.c) — exact mount sequence; flag semantics for `--cap-drop`, `--uid`, `--seccomp`.
- [`seccomp(2)` man page](https://man7.org/linux/man-pages/man2/seccomp.2.html) — allowlist guidance, RET_KILL_PROCESS semantics.
- [`user_namespaces(7)`](https://man7.org/linux/man-pages/man7/user_namespaces.7.html) — capability scoping inside userns.
- Recent kernel CVEs informing the allowlist/denylist decision: CVE-2022-0185 (fsconfig), CVE-2022-2588 (cls_route), CVE-2023-0386 (overlayfs), CVE-2023-32233 (nf_tables UAF in user-ns), CVE-2024-1086 (nf_tables), CVE-2024-0582 (io_uring).

---

## 8. Decision required

Before I implement, please confirm:

1. **Adopt the `pincery-init` exec-wrapper architecture?** (Y/N)
2. **Set kernel floor at Linux ≥ 6.7 (Landlock ABI 6)?** Acceptable for production targets given current LTS is 6.6 and 6.12 LTS lands soon. (Y / lower-floor-N)
3. **Switch seccomp to allowlist with `KILL_PROCESS` mismatch?** (Y/N)
4. **Default `--uid 65534 --gid 65534 --cap-drop ALL` in the sandbox?** Confirm no current tool/agent expects to run as the host UID. (Y/N)
5. **Slip Phase G** by ~one BUILD cycle to land this rework first? Alternative: ship Phase G with `landlock=false` default and revisit, accepting reduced security posture in the interim. (rework-first / ship-degraded)

On `Y` to all five, I'll generate `scaffolding/scope.md` v8 (or an iteration entry) and execute the migration plan in §6.
