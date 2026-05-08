#!/usr/bin/env bash
# Open Pincery CLI installer.
#
# Detects OS + architecture, downloads the matching signed `pcy` binary from
# a GitHub release, verifies the cosign signature (if cosign is installed)
# and the SHA-256 checksum (always), then installs into $PREFIX/bin.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/RCSnyder/open-pincery/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/RCSnyder/open-pincery/main/install.sh | bash -s -- --version v1.0.2
#
# Environment:
#   PCY_VERSION   — release tag to install (default: latest)
#   PCY_PREFIX    — install prefix (default: $HOME/.local)
#   PCY_REPO      — override source repo (default: RCSnyder/open-pincery)

set -euo pipefail

PCY_REPO="${PCY_REPO:-RCSnyder/open-pincery}"
PCY_VERSION="${PCY_VERSION:-}"
PCY_PREFIX="${PCY_PREFIX:-$HOME/.local}"
VERIFY_COSIGN="auto" # auto | require | skip

usage() {
  cat <<EOF
Open Pincery CLI installer

Usage: $0 [options]

Options:
  --version <tag>      Install a specific release tag (default: latest)
  --prefix <dir>       Install prefix; binary goes to <prefix>/bin (default: \$HOME/.local)
  --require-cosign     Fail the install if cosign is missing (default: verify if present)
  --skip-cosign        Skip cosign verification entirely (sha256 still enforced)
  -h, --help           Show this help

Examples:
  $0
  $0 --version v1.0.2 --prefix /usr/local
  curl -fsSL https://raw.githubusercontent.com/${PCY_REPO}/main/install.sh | bash
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version) PCY_VERSION="$2"; shift 2 ;;
    --prefix) PCY_PREFIX="$2"; shift 2 ;;
    --require-cosign) VERIFY_COSIGN="require"; shift ;;
    --skip-cosign) VERIFY_COSIGN="skip"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage; exit 2 ;;
  esac
done

err() { echo "error: $*" >&2; exit 1; }
info() { echo "==> $*"; }

# ---- Detect platform -------------------------------------------------------
UNAME_S="$(uname -s)"
UNAME_M="$(uname -m)"

case "$UNAME_S" in
  Linux)   OS="linux" ;;
  Darwin)  OS="macos" ;;
  MINGW*|MSYS*|CYGWIN*) OS="windows" ;;
  *) err "unsupported OS: $UNAME_S (supported: Linux, macOS, Windows via Git Bash)" ;;
esac

case "$UNAME_M" in
  x86_64|amd64) ARCH="x86_64" ;;
  arm64|aarch64) ARCH="aarch64" ;;
  *) err "unsupported architecture: $UNAME_M" ;;
esac

# Windows releases are x86_64 only today
if [ "$OS" = "windows" ] && [ "$ARCH" != "x86_64" ]; then
  err "windows-$ARCH is not published; only windows-x86_64 is available"
fi

# Linux-aarch64 is CLI-only (server is x86_64 only), which is what we want here.
SUFFIX="${OS}-${ARCH}"
EXT=""
if [ "$OS" = "windows" ]; then EXT=".exe"; fi

info "Detected platform: ${SUFFIX}"

# ---- Resolve version -------------------------------------------------------
need() { command -v "$1" >/dev/null 2>&1 || err "required tool not found: $1"; }

need curl

if [ -z "$PCY_VERSION" ]; then
  info "Resolving latest release from github.com/${PCY_REPO}"
  PCY_VERSION="$(curl -fsSL "https://api.github.com/repos/${PCY_REPO}/releases/latest" \
    | grep -E '"tag_name"\s*:' | head -n1 | sed -E 's/.*"([^"]+)".*/\1/')"
  [ -n "$PCY_VERSION" ] || err "could not resolve latest release tag"
fi
info "Installing pcy ${PCY_VERSION}"

# ---- Download artifacts ----------------------------------------------------
ASSET_BASE="https://github.com/${PCY_REPO}/releases/download/${PCY_VERSION}"
BIN_NAME="pcy-${PCY_VERSION}-${SUFFIX}${EXT}"
SHA_NAME="${BIN_NAME}.sha256"
SIG_NAME="${BIN_NAME}.sig"
CRT_NAME="${BIN_NAME}.pem"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

fetch() {
  local url="$1" out="$2"
  curl -fsSL --retry 3 --retry-delay 1 -o "$out" "$url" \
    || err "download failed: $url"
}

info "Downloading ${BIN_NAME}"
fetch "${ASSET_BASE}/${BIN_NAME}" "${TMP}/${BIN_NAME}"
fetch "${ASSET_BASE}/${SHA_NAME}" "${TMP}/${SHA_NAME}"

# ---- Verify sha256 ---------------------------------------------------------
info "Verifying SHA-256 checksum"
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$TMP" && sha256sum -c "${SHA_NAME}") >/dev/null \
    || err "sha256 mismatch for ${BIN_NAME}"
elif command -v shasum >/dev/null 2>&1; then
  (cd "$TMP" && shasum -a 256 -c "${SHA_NAME}") >/dev/null \
    || err "sha256 mismatch for ${BIN_NAME}"
else
  err "no sha256 tool available (install sha256sum or shasum)"
fi

# ---- Verify cosign (keyless, GitHub OIDC) ----------------------------------
if [ "$VERIFY_COSIGN" = "skip" ]; then
  info "Skipping cosign verification (--skip-cosign)"
else
  if command -v cosign >/dev/null 2>&1; then
    info "Verifying cosign signature"
    fetch "${ASSET_BASE}/${SIG_NAME}" "${TMP}/${SIG_NAME}"
    fetch "${ASSET_BASE}/${CRT_NAME}" "${TMP}/${CRT_NAME}"
    # shellcheck disable=SC2016
    IDENTITY_REGEX='https://github\.com/'"${PCY_REPO//\//\\/}"'/\.github/workflows/release\.yml@refs/tags/v.*'
    COSIGN_EXPERIMENTAL=1 cosign verify-blob \
      --certificate-identity-regexp "${IDENTITY_REGEX}" \
      --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
      --signature "${TMP}/${SIG_NAME}" \
      --certificate "${TMP}/${CRT_NAME}" \
      "${TMP}/${BIN_NAME}" \
      || err "cosign verification failed"
  elif [ "$VERIFY_COSIGN" = "require" ]; then
    err "cosign not installed and --require-cosign was passed. Install cosign: https://docs.sigstore.dev/cosign/installation/"
  else
    echo "warning: cosign not installed — skipping signature verification." >&2
    echo "         install cosign for supply-chain verification:" >&2
    echo "           https://docs.sigstore.dev/cosign/installation/" >&2
  fi
fi

# ---- Install ---------------------------------------------------------------
DEST_DIR="${PCY_PREFIX}/bin"
DEST="${DEST_DIR}/pcy${EXT}"

mkdir -p "$DEST_DIR"
install -m 0755 "${TMP}/${BIN_NAME}" "$DEST" 2>/dev/null \
  || { cp "${TMP}/${BIN_NAME}" "$DEST" && chmod 0755 "$DEST"; }

info "Installed: $DEST"

# ---- PATH hint -------------------------------------------------------------
case ":${PATH:-}:" in
  *":${DEST_DIR}:"*) ;;
  *)
    echo
    echo "Note: ${DEST_DIR} is not on your PATH. Add it with:"
    echo "  echo 'export PATH=\"${DEST_DIR}:\$PATH\"' >> ~/.bashrc   # or ~/.zshrc"
    ;;
esac

echo
"$DEST" --help >/dev/null 2>&1 || true
info "Done. Try: pcy --help"
