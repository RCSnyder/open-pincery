# DELIVERY.md — Open Pincery v1

## What Was Built

A multi-agent platform runtime implementing the Open Pincery architecture: event-sourced agents with CAS lifecycle management, LLM-powered wake/sleep cycles, maintenance projections, and an HTTP API. Single-binary Rust server backed by PostgreSQL.

## How to Use It

1. `docker compose up -d` — start PostgreSQL
2. `cp .env.example .env` — configure (set `LLM_API_KEY`, `OPEN_PINCERY_BOOTSTRAP_TOKEN`)
3. `cargo build --release && source .env && ./target/release/open-pincery`
4. `POST /api/bootstrap` with bearer token → get session token
5. `POST /api/agents` → create agents
6. `POST /api/agents/:id/messages` → send messages (triggers wake cycle)
7. `GET /api/agents/:id/events` → view event log

## What It Does

- **Agent lifecycle**: Agents transition `asleep → awake → maintenance → asleep` via atomic CAS operations
- **Wake loop**: On message, agent wakes, calls LLM iteratively with tools (shell, plan, sleep), records all events
- **Maintenance**: After each wake, LLM updates agent identity, work list, and summary
- **Drain check**: If new messages arrive during wake, agent re-wakes instead of sleeping
- **Stale recovery**: Background job detects agents stuck awake and force-releases them
- **Event log**: Append-only, ordered, complete history of every agent action
- **Projections**: Versioned, immutable snapshots of agent state after each wake

## Known Limitations

- **No sandboxing**: Shell tool runs with host privileges (Phase 2: Zerobox container isolation)
- **No webhooks/timers**: Only message-triggered wakes (Phase 2)
- **No inter-agent messaging**: Single-agent operation only (Phase 2)
- **No rate limiting**: API and LLM calls have no rate controls
- **No graceful shutdown**: Background tasks may be interrupted on SIGTERM
- **Single workspace**: Multi-tenancy schema exists but not enforced in API authorization
- **No UI**: API-only interface
- **cargo-audit**: Dependency vulnerability scan could not be run (tool installation timeout)

## Infrastructure

- **Runtime**: Single Rust binary (~15MB release)
- **Database**: PostgreSQL 16 (13 migration files)
- **External**: One OpenAI-compatible LLM API
- **Cost**: PostgreSQL hosting + LLM API usage. No other infrastructure costs.
