# Open Pincery Dev Setup — macOS

This runbook takes a macOS contributor from a clean clone to a green
`cargo test` run against Open Pincery v9, including the Linux-only
sandbox suite (AC-53 Zerobox, AC-71 secret proxy, AC-72 egress
allowlist).

v9's sandbox depends on kernel primitives (bubblewrap, slirp4netns,
landlock LSM, cgroup v2) that do not exist on Darwin. Rather than
maintain a parallel macOS sandbox, Open Pincery ships a pinned
Ubuntu 24.04 "devshell" Docker image — see AC-75 in
[`scaffolding/scope.md`](../../scaffolding/scope.md).

## Prerequisites

- macOS 13+ (Apple Silicon or Intel)
- [Docker Desktop 24+](https://www.docker.com/products/docker-desktop/) with the default linux/amd64 runtime enabled (Apple Silicon users: keep _Use Rosetta for x86_64/amd64 emulation_ enabled under **Settings → General**)
- Git 2.40+
- A PostgreSQL client if you want to inspect the DB from the host (optional — `devshell` ships one)

You do **not** need Rust, sqlx-cli, or any sandbox binary on the host —
they all live inside the devshell image.

## First-time setup

```bash
# 1. Clone the repo and enter it.
git clone https://github.com/RCSnyder/open-pincery.git
cd open-pincery

# 2. Pre-pull the devshell image so the first cargo test is fast.
docker pull ghcr.io/open-pincery/devshell:v9

# 3. Smoke-test the wrapper.
./scripts/devshell.sh --version-check
```

If step 3 prints your Docker version and `devshell image:
ghcr.io/open-pincery/devshell:v9`, the wrapper is wired correctly.

## Running the test suite

All `cargo` commands route through the devshell:

```bash
./scripts/devshell.sh cargo test
./scripts/devshell.sh cargo test --test devshell_parity_test
./scripts/devshell.sh cargo clippy --all-targets -- -D warnings
```

Interactive shell for ad-hoc work:

```bash
./scripts/devshell.sh
# inside the container:
cargo test
sqlx migrate run
exit
```

Build artifacts land in `./target/devshell/` on the host (separate from
a native `./target` so you can keep a host Rust toolchain around for
editor integration without cache collisions).

## Editor integration

VS Code + rust-analyzer works best with the
[Dev Containers](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)
extension pointed at `Dockerfile.devshell`. Alternatively, install
Rust 1.88 on the host with `rustup` for fast autocomplete and use the
devshell only for `cargo test`.

## Troubleshooting

| Symptom                                                                     | Fix                                                                                          |
| --------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `docker: Error response from daemon: could not select device driver`        | Restart Docker Desktop and re-run.                                                           |
| `Error response from daemon: privileged mode is incompatible with rootless` | Docker Desktop rootless mode must stay **off** for AC-75 — the sandbox needs `--privileged`. |
| `cargo test` hangs at "Updating crates.io index"                            | The devshell uses host networking (`--network host`); check macOS firewall / VPN rules.      |
| Apple Silicon: very slow builds                                             | Ensure Rosetta emulation is enabled; `docker info` should report `Architecture: x86_64`.     |

## Next steps

- Read [`docs/SECURITY.md`](../SECURITY.md) (ships in Slice A1 / AC-54) before touching sandbox code.
- Review AC-53 through AC-75 in [`scaffolding/scope.md`](../../scaffolding/scope.md) to understand which slice you are contributing to.
