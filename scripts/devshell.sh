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

# AC-81 — install the canonical_action commit-msg hook idempotently.
# Copies .github/hooks/commit-msg-spec-ref to .git/hooks/commit-msg if
# and only if no commit-msg hook is present, or the present hook is
# byte-identical to the unmodified .sample. Never overwrites a user's
# customized hook. Skipped silently when not in a git working tree.
install_commit_msg_hook() {
  local repo_root="${1:-}"
  local source_hook="${repo_root}/.github/hooks/commit-msg-spec-ref"
  local git_dir
  git_dir="$(git -C "$repo_root" rev-parse --git-dir 2>/dev/null || true)"
  if [[ -z "$git_dir" || ! -f "$source_hook" ]]; then
    return 0
  fi
  # Resolve to absolute path (git --git-dir may be relative).
  case "$git_dir" in
    /*) ;;
    *) git_dir="${repo_root}/${git_dir}" ;;
  esac
  local target="${git_dir}/hooks/commit-msg"
  local sample="${git_dir}/hooks/commit-msg.sample"
  mkdir -p "$(dirname "$target")"
  if [[ -e "$target" ]]; then
    if [[ -f "$sample" ]] && cmp -s "$target" "$sample"; then
      :  # unmodified sample — replace it
    elif cmp -s "$target" "$source_hook"; then
      return 0  # already installed and current
    else
      return 0  # user-customized, do not touch
    fi
  fi
  cp "$source_hook" "$target"
  chmod +x "$target"
}

if [[ -z "${OPEN_PINCERY_DEVSHELL_SKIP_HOOK_INSTALL:-}" ]]; then
  _devshell_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  install_commit_msg_hook "$_devshell_repo_root" || true
  unset _devshell_repo_root
fi

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
TARGET_CACHE_HOST="${OPEN_PINCERY_DEVSHELL_HOST_TARGET_DIR:-$REPO_ROOT/target/devshell}"
TARGET_CACHE_CONTAINER="/cargo-target"

mkdir -p "$TARGET_CACHE_HOST"

# Windows git-bash / MSYS2 rewrites unix-style args (`/work`, `-w /work`)
# into Windows paths before handing them to docker.exe, which then sees
# `C:/Program Files/Git/work` and fails. Disable that translation for the
# duration of this `docker run` invocation so in-container paths pass
# through verbatim. No-op on Linux/macOS.
export MSYS_NO_PATHCONV=1
export MSYS2_ARG_CONV_EXCL='*'

# Only attach a TTY when stdout is one; non-interactive callers
# (CI, `./scripts/devshell.sh cargo test`) must not get `-it`.
DOCKER_TTY_FLAGS=(-i)
if [[ -t 1 ]]; then
  DOCKER_TTY_FLAGS+=(-t)
fi

# --privileged + --cgroupns=host are required so the inner sandbox can
# create user namespaces, mount tmpfs, and bind cgroup v2 controllers.
# The bind mount exposes the repo read-write so `cargo test`, sqlx
# migrations, and local edits all flow through to the host.
exec docker run --rm "${DOCKER_TTY_FLAGS[@]}" \
  --privileged \
  --cgroupns=host \
  --network host \
  -v "${REPO_ROOT}:/work" \
  -v "${TARGET_CACHE_HOST}:${TARGET_CACHE_CONTAINER}" \
  -w /work \
  -e CARGO_TARGET_DIR="${TARGET_CACHE_CONTAINER}" \
  "${IMAGE}" \
  "$@"
