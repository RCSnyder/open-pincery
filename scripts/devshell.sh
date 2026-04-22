#!/usr/bin/env bash
# AC-75 — Cross-platform developer shell for Open Pincery v9.
#
# v9's sandbox (AC-53 Zerobox), secret injection proxy (AC-71), and
# egress allowlist (AC-72) depend on Linux kernel primitives that do not
# exist on macOS or Windows hosts.  This wrapper launches the pinned
# devshell image so every contributor runs the same toolchain against
# the same kernel surface.
#
# Usage:
#   ./scripts/devshell.sh                     # interactive shell
#   ./scripts/devshell.sh cargo test          # one-off command
#   ./scripts/devshell.sh --version-check     # smoke test
#
# Requires Docker 24+ on the host.  On Linux the native toolchain works
# without this wrapper; run the script only for reproducibility or for
# the sandbox suite.

set -euo pipefail

IMAGE="${OPEN_PINCERY_DEVSHELL_IMAGE:-ghcr.io/open-pincery/devshell:v9}"

if ! command -v docker >/dev/null 2>&1; then
  echo "error: docker not found on PATH. Install Docker 24+ and retry." >&2
  exit 127
fi

# Lightweight smoke-test path used by tests/devshell_parity_test.rs so
# the parity suite can verify the wrapper is invocable without pulling
# and starting the full image.
if [[ "${1:-}" == "--version-check" ]]; then
  docker --version
  echo "devshell image: ${IMAGE}"
  exit 0
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# --privileged + --cgroupns=host are required so the inner sandbox can
# create user namespaces, mount tmpfs, and bind cgroup v2 controllers.
# The bind mount exposes the repo read-write so `cargo test`, sqlx
# migrations, and local edits all flow through to the host.
exec docker run --rm -it \
  --privileged \
  --cgroupns=host \
  --network host \
  -v "${REPO_ROOT}:/work" \
  -w /work \
  -e CARGO_TARGET_DIR=/work/target/devshell \
  "${IMAGE}" \
  "$@"
