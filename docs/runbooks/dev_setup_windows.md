# Open Pincery Dev Setup — Windows

This runbook takes a Windows contributor from a clean clone to a green
`cargo test` run against Open Pincery v9, including the Linux-only
sandbox suite (AC-53 Zerobox, AC-71 secret proxy, AC-72 egress
allowlist).

v9's sandbox depends on kernel primitives (bubblewrap, slirp4netns,
landlock LSM, cgroup v2) that do not exist on native Windows. Open
Pincery ships a pinned Ubuntu 24.04 "devshell" Docker image instead —
see AC-75 in [`scaffolding/scope.md`](../../scaffolding/scope.md).

## Prerequisites

- Windows 10 22H2+ or Windows 11 (x64)
- [Docker Desktop 23+ for Windows](https://www.docker.com/products/docker-desktop/) with the **WSL2 backend** enabled (Settings → General → _Use the WSL 2 based engine_). Verified working on Docker Desktop 23.0.5.
- [Git for Windows](https://git-scm.com/download/win) 2.40+
- PowerShell 7+ (Windows PowerShell 5.1 also works but PS7 is recommended)

You do **not** need Rust, sqlx-cli, or any sandbox binary on the host —
they all live inside the devshell image.

## First-time setup

```powershell
# 1. Clone the repo and enter it.
git clone https://github.com/RCSnyder/open-pincery.git
Set-Location open-pincery

# 2. Allow the PowerShell wrapper to run in this shell session
#    (project scripts are not signed).
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass

# 3. Pre-pull the devshell image so the first cargo test is fast.
docker pull ghcr.io/open-pincery/devshell:v9

# 4. Smoke-test the wrapper.
.\scripts\devshell.ps1 --version-check
```

If step 4 prints your Docker version and `devshell image:
ghcr.io/open-pincery/devshell:v9`, the wrapper is wired correctly.

## Running the test suite

All `cargo` commands route through the devshell:

```powershell
.\scripts\devshell.ps1 cargo test
.\scripts\devshell.ps1 cargo test --test devshell_parity_test
.\scripts\devshell.ps1 cargo clippy --all-targets -- -D warnings
```

Interactive shell for ad-hoc work:

```powershell
.\scripts\devshell.ps1
# inside the container:
cargo test
sqlx migrate run
exit
```

Build artifacts land in `.\target\devshell\` on the host (separate from
a native `.\target` so you can keep a host Rust toolchain around for
editor integration without cache collisions).

## Keeping heavy artifacts off `C:`

Use Cargo's native target override for any host-side Rust build you run
outside the devshell, including rust-analyzer's background `cargo check`:

```powershell
$env:CARGO_TARGET_DIR = 'E:\open-pincery-target\native'
cargo test
```

The devshell wrapper has its own host-side override. Point it at a
directory on `E:\` and the wrapper bind-mounts that path into the
container as Cargo's target directory:

```powershell
$env:OPEN_PINCERY_DEVSHELL_HOST_TARGET_DIR = 'E:\open-pincery-target\devshell'
.\scripts\devshell.ps1 cargo test
```

That moves repo-controlled build artifacts off the system drive without
changing the checkout path.

### Persist the override for every shell

Per-shell `$env:` assignments vanish when the terminal closes and are
invisible to VS Code, rust-analyzer, Git Bash, and `cmd.exe`. To make
the relocation stick across every shell and editor, persist the
variables under `HKCU\Environment` and create the target directories
once:

```powershell
# Run in PowerShell; requires no admin rights.
setx CARGO_TARGET_DIR "E:\open-pincery-target\native"
setx OPEN_PINCERY_DEVSHELL_HOST_TARGET_DIR "E:\open-pincery-target\devshell"
New-Item -ItemType Directory -Force -Path `
  'E:\open-pincery-target\native', `
  'E:\open-pincery-target\devshell' | Out-Null
```

`setx` writes to `HKCU\Environment`, so **new** shells (PowerShell, cmd,
Git Bash, VS Code integrated terminals) inherit the values. Existing
shells and a running VS Code need to be restarted before they see the
change. Once the new shell is up, reclaim the old cache on `C:` with:

```powershell
Remove-Item -Recurse -Force .\target
```

Verify the values with:

```powershell
reg query "HKCU\Environment" /v CARGO_TARGET_DIR
reg query "HKCU\Environment" /v OPEN_PINCERY_DEVSHELL_HOST_TARGET_DIR
```

### WSL2 and Docker Desktop storage

Docker Desktop's WSL2 disk image is separate from the repo's build
artifacts. Move that in Docker Desktop under Settings -> Resources ->
Advanced -> Disk image location. For a general-purpose WSL distro, use
the normal `wsl --export` / `wsl --import` workflow; the repo itself
cannot relocate that VM storage.

## Editor integration

VS Code + rust-analyzer works best with the
[Dev Containers](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)
extension pointed at `Dockerfile.devshell`. Alternatively, install
Rust 1.88 on the host with `rustup-init.exe` for fast autocomplete and
use the devshell only for `cargo test`.

## Troubleshooting

| Symptom                                                                                                 | Fix                                                                                                                                                                   |
| ------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Set-ExecutionPolicy` blocks scripts                                                                    | Run the command from a **new** PowerShell window; the `-Scope Process` policy only applies to that session.                                                           |
| `docker: error during connect: open //./pipe/docker_engine: The system cannot find the file specified.` | Start Docker Desktop and wait for the whale icon to go steady.                                                                                                        |
| `privileged mode is incompatible with rootless`                                                         | Docker Desktop rootless mode must stay **off** for AC-75 — the sandbox needs `--privileged`.                                                                          |
| Line-ending errors on shell scripts                                                                     | Run `git config --global core.autocrlf input` before cloning, or `git checkout -- .` after cloning to rewrite line endings.                                           |
| Slow first build                                                                                        | The initial `cargo build` populates `target\devshell\`; subsequent runs are incremental.                                                                              |
| Git Bash: `docker: invalid reference format` or path like `C:/Program Files/Git/work`                   | MSYS is mangling the `-v` bind-mount path. Prefix `docker run` with `MSYS_NO_PATHCONV=1` and use `$(pwd -W)` for the host path. The PowerShell wrapper is unaffected. |
| AC-53 sandbox tests fail with `landlock: not supported`                                                 | WSL2 kernel is older than 5.13. Run `wsl --update` and restart Docker Desktop; confirm with `wsl cat /proc/version`.                                                  |

## Next steps

- Read [`docs/SECURITY.md`](../SECURITY.md) (ships in Slice A1 / AC-54) before touching sandbox code.
- Review AC-53 through AC-75 in [`scaffolding/scope.md`](../../scaffolding/scope.md) to understand which slice you are contributing to.
