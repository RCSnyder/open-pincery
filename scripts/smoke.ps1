$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $Root

function Fail([string]$Message, [string]$Anchor) {
  Write-Error $Message
  Write-Host "See README.md troubleshooting: $Anchor"
  exit 1
}

function Require-Cmd([string]$Name) {
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    Fail "Missing required command: $Name" "#from-signed-release-binary"
  }
}

function Read-EnvValue([string]$Key, [string]$Path) {
  $line = Get-Content $Path | Where-Object { $_ -match "^$Key=" } | Select-Object -Last 1
  if (-not $line) { return "" }
  return ($line -split "=", 2)[1].Trim('"')
}

function Get-TargetRoot {
  if ($env:CARGO_TARGET_DIR) {
    return $env:CARGO_TARGET_DIR
  }

  return (Join-Path $Root "target")
}

function Get-PcyCommand {
  $cmd = Get-Command pcy -ErrorAction SilentlyContinue
  if ($cmd) { return "pcy" }

  $targetRoot = Get-TargetRoot

  $release = Join-Path $targetRoot "release\pcy.exe"
  if (Test-Path $release) { return $release }

  $debug = Join-Path $targetRoot "debug\pcy.exe"
  if (Test-Path $debug) { return $debug }

  Fail "pcy binary not found. Build it with 'cargo build --release --bin pcy'." "#from-signed-release-binary"
  return ""
}

Require-Cmd docker
Require-Cmd curl.exe

if (-not (Test-Path ".env")) {
  Copy-Item ".env.example" ".env"
  Write-Host "Created .env from .env.example"
}

$BootstrapToken = Read-EnvValue "OPEN_PINCERY_BOOTSTRAP_TOKEN" ".env"
if ([string]::IsNullOrWhiteSpace($BootstrapToken) -or $BootstrapToken -eq "change-me-to-a-random-secret") {
  Fail "OPEN_PINCERY_BOOTSTRAP_TOKEN must be set to a non-placeholder value in .env." "#bootstrap-401"
}

$BaseUrl = if ($env:OPEN_PINCERY_URL) { $env:OPEN_PINCERY_URL } else { "http://localhost:8080" }
$Pcy = Get-PcyCommand

Write-Host "Starting stack..."
& docker compose up -d --wait
if ($LASTEXITCODE -ne 0) {
  Fail "docker compose up failed" "#compose-up-failed"
}

Write-Host "Waiting for /ready..."
$ready = $false
for ($i = 0; $i -lt 30; $i++) {
  try {
    & curl.exe -fsS "$BaseUrl/ready" | Out-Null
    if ($LASTEXITCODE -eq 0) {
      $ready = $true
      break
    }
  } catch {
    # ignore and retry
  }
  Start-Sleep -Seconds 2
}
if (-not $ready) {
  Fail "Service did not reach /ready within 60s." "#silent-wake"
}

Write-Host "Logging in..."
& $Pcy --url $BaseUrl login --bootstrap-token $BootstrapToken *> "$env:TEMP\pcy-login.log"
if ($LASTEXITCODE -ne 0) {
  Fail "pcy login failed. Check $env:TEMP\pcy-login.log" "#bootstrap-401"
}

$AgentName = "smoke-$(Get-Date -UFormat %s)"
Write-Host "Creating agent: $AgentName"
$AgentJson = & $Pcy --url $BaseUrl agent create $AgentName 2> "$env:TEMP\pcy-agent-create.err"
if ($LASTEXITCODE -ne 0) {
  Fail "pcy agent create failed. Check $env:TEMP\pcy-agent-create.err" "#bootstrap-401"
}

$AgentId = [regex]::Match(($AgentJson -join "`n"), "[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}").Value
if ([string]::IsNullOrWhiteSpace($AgentId)) {
  Fail "Could not parse agent id from pcy output." "#silent-wake"
}

Write-Host "Sending message..."
& $Pcy --url $BaseUrl message $AgentId "smoke ping" *> "$env:TEMP\pcy-message.log"
if ($LASTEXITCODE -ne 0) {
  Fail "pcy message failed. Check $env:TEMP\pcy-message.log" "#bootstrap-401"
}

Write-Host "Polling events for message_received..."
$found = $false
for ($i = 0; $i -lt 20; $i++) {
  $EventsOut = & $Pcy --url $BaseUrl events $AgentId 2> "$env:TEMP\pcy-events.err"
  if (($EventsOut -join "`n") -match "message_received") {
    $found = $true
    break
  }
  Start-Sleep -Seconds 2
}
if (-not $found) {
  Fail "Did not observe message_received event for $AgentId." "#silent-wake"
}

Write-Host "Smoke OK: bootstrap, agent create, message send, and event observation succeeded."
