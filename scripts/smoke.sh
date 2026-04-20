#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

fail() {
  echo "ERROR: $1" >&2
  echo "See README.md troubleshooting: $2" >&2
  exit 1
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "Missing required command: $1" "#from-signed-release-binary"
  fi
}

read_env_value() {
  local key="$1"
  local file="$2"
  awk -F= -v k="$key" '$1==k {v=$0; sub(/^[^=]*=/, "", v); gsub(/^"|"$/, "", v); print v}' "$file" | tail -n1
}

pick_pcy_cmd() {
  if command -v pcy >/dev/null 2>&1; then
    echo "pcy"
    return
  fi
  if [[ -x "$ROOT_DIR/target/release/pcy" ]]; then
    echo "$ROOT_DIR/target/release/pcy"
    return
  fi
  if [[ -x "$ROOT_DIR/target/debug/pcy" ]]; then
    echo "$ROOT_DIR/target/debug/pcy"
    return
  fi
  fail "pcy binary not found. Build it with 'cargo build --release --bin pcy'." "#from-signed-release-binary"
}

require_cmd docker
require_cmd curl
require_cmd awk
require_cmd grep

if [[ ! -f .env ]]; then
  cp .env.example .env
  echo "Created .env from .env.example"
fi

BOOTSTRAP_TOKEN="$(read_env_value OPEN_PINCERY_BOOTSTRAP_TOKEN .env)"
if [[ -z "$BOOTSTRAP_TOKEN" || "$BOOTSTRAP_TOKEN" == "change-me-to-a-random-secret" ]]; then
  fail "OPEN_PINCERY_BOOTSTRAP_TOKEN must be set to a non-placeholder value in .env." "#bootstrap-401"
fi

BASE_URL="${OPEN_PINCERY_URL:-http://localhost:8080}"
PCY_CMD="$(pick_pcy_cmd)"

echo "Starting stack..."
docker compose up -d --wait || fail "docker compose up failed" "#compose-up-failed"

echo "Waiting for /ready..."
ready_ok=0
for _ in $(seq 1 30); do
  if curl -fsS "$BASE_URL/ready" >/dev/null 2>&1; then
    ready_ok=1
    break
  fi
  sleep 2
done
if [[ "$ready_ok" -ne 1 ]]; then
  fail "Service did not reach /ready within 60s." "#silent-wake"
fi

echo "Bootstrapping session..."
"$PCY_CMD" --url "$BASE_URL" bootstrap --bootstrap-token "$BOOTSTRAP_TOKEN" >/tmp/pcy-bootstrap.out 2>/tmp/pcy-bootstrap.err \
  || fail "pcy bootstrap failed: $(cat /tmp/pcy-bootstrap.err)" "#bootstrap-401"

agent_name="smoke-$(date +%s)"
echo "Creating agent: $agent_name"
agent_json="$($PCY_CMD --url "$BASE_URL" agent create "$agent_name" 2>/tmp/pcy-agent-create.err)" \
  || fail "pcy agent create failed: $(cat /tmp/pcy-agent-create.err)" "#bootstrap-401"

agent_id="$(echo "$agent_json" | grep -Eo '[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}' | head -n1 || true)"
if [[ -z "$agent_id" ]]; then
  fail "Could not parse agent id from pcy output." "#silent-wake"
fi

echo "Sending message..."
"$PCY_CMD" --url "$BASE_URL" message "$agent_id" "smoke ping" >/tmp/pcy-message.out 2>/tmp/pcy-message.err \
  || fail "pcy message failed: $(cat /tmp/pcy-message.err)" "#bootstrap-401"

echo "Polling events for message_received..."
found=0
for _ in $(seq 1 20); do
  events_out="$($PCY_CMD --url "$BASE_URL" events "$agent_id" 2>/tmp/pcy-events.err || true)"
  if echo "$events_out" | grep -q 'message_received'; then
    found=1
    break
  fi
  sleep 2
done

if [[ "$found" -ne 1 ]]; then
  fail "Did not observe message_received event for $agent_id." "#silent-wake"
fi

echo "Smoke OK: bootstrap, agent create, message send, and event observation succeeded."
