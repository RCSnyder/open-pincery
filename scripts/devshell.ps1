# AC-75 — Cross-platform developer shell for Open Pincery v9 (Windows).
#
# Mirrors scripts/devshell.sh for PowerShell / Windows contributors.  v9
# introduces Linux-only sandbox primitives (AC-53 / AC-71 / AC-72) that
# cannot run natively on Windows; this wrapper launches the pinned
# Ubuntu 24.04 devshell image instead.
#
# Usage:
#   .\scripts\devshell.ps1                   # interactive shell
#   .\scripts\devshell.ps1 cargo test        # one-off command
#   .\scripts\devshell.ps1 --version-check   # smoke test
#
# Requires Docker Desktop 24+ with WSL2 backend on the host.

[CmdletBinding()]
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Args
)

$ErrorActionPreference = "Stop"

$Image = if ($env:OPEN_PINCERY_DEVSHELL_IMAGE) {
    $env:OPEN_PINCERY_DEVSHELL_IMAGE
} else {
    "ghcr.io/open-pincery/devshell:v9"
}

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    Write-Error "docker not found on PATH. Install Docker Desktop 24+ and retry."
    exit 127
}

if ($Args.Count -ge 1 -and $Args[0] -eq "--version-check") {
    docker --version
    Write-Host "devshell image: $Image"
    exit 0
}

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

# --privileged + --cgroupns=host are required so the inner sandbox can
# create user namespaces, mount tmpfs, and bind cgroup v2 controllers.
# Docker Desktop's WSL2 VM provides the Linux kernel surface the sandbox
# tests depend on.
$DockerArgs = @(
    "run", "--rm", "-it",
    "--privileged",
    "--cgroupns=host",
    "--network", "host",
    "-v", "${RepoRoot}:/work",
    "-w", "/work",
    "-e", "CARGO_TARGET_DIR=/work/target/devshell",
    $Image
) + $Args

& docker @DockerArgs
exit $LASTEXITCODE
