# Thin wrapper around the pcy CLI that ships inside the app container.
# Runs `pcy` in the running compose stack so you don't need a local
# Rust/MSVC toolchain on Windows. Session config persists in the
# `pcyconfig` named volume.
#
# Usage: .\pcy.ps1 <subcommand> [args...]
#   .\pcy.ps1 bootstrap --bootstrap-token $env:OPEN_PINCERY_BOOTSTRAP_TOKEN
#   .\pcy.ps1 agent create scout
#   .\pcy.ps1 message <agent> "hello"
#   .\pcy.ps1 events <agent> --tail

$ErrorActionPreference = 'Stop'

# Allocate a TTY only when running interactively; docker exec -T when piped.
$ttyFlags = @()
if (-not [Console]::IsInputRedirected -and -not [Console]::IsOutputRedirected) {
    # leave empty — default docker exec allocates tty
} else {
    $ttyFlags = @('-T')
}

& docker compose exec @ttyFlags app pcy @args
exit $LASTEXITCODE
