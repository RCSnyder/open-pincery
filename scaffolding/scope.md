# Scope — Open Pincery v1

## Problem

There is no open-source platform that treats AI agents as continuous, durable entities with event-sourced memory, CAS-protected lifecycle, and wake/sleep cycles. Existing frameworks treat agents as ephemeral function calls that vanish between requests. Open Pincery implements the Continuous Agent Architecture: agents persist indefinitely with stable identity, append-only event logs, and self-configuration through conversation.

## Smallest Useful Version

A single Rust binary + PostgreSQL that implements the core agent runtime for `self_host_individual` mode. A user can: bootstrap a local admin, create an agent via HTTP API, send it a message, watch the agent wake via CAS, reason with an LLM, execute shell commands, update its own identity and work list through maintenance, and go back to sleep — with every action recorded in an append-only event log. The agent is a continuous entity that remembers across wakes.

This is the foundation that every other feature (webhooks, multi-tenancy, approval workflows, credential vaults, MCP, dashboard) builds on top of. Without this working end-to-end, nothing else matters.

## Acceptance Criteria

- **AC-1** (CAS Lifecycle): An agent transitions through Resting → WakeAcquiring → Awake → WakeEnding → Maintenance → Resting using `UPDATE ... WHERE status = $expected RETURNING *` in PostgreSQL. Concurrent wake attempts on the same agent are rejected; only one wake is active at any time. Verified by a test that attempts two simultaneous wake acquisitions and confirms exactly one succeeds.

- **AC-2** (Event Log): All agent activity (messages received, tool calls, tool results, wake start, wake end, plans, messages sent) is appended to an immutable event log in PostgreSQL. Events are never updated or deleted. Event ordering is preserved by timestamp. Verified by sending a message, letting the agent wake and act, then querying the event log and confirming the complete sequence.

- **AC-3** (Prompt Assembly): The system assembles a bounded prompt from: (1) constitution/system prompt from the active prompt template, (2) current UTC time, (3) up to 20 most recent wake summaries (each ≤500 chars), (4) current identity projection, (5) current work list projection, (6) most recent 200 events converted to chat messages. Character-based trim drops oldest messages first. Verified by creating an agent with known projections and events, assembling the prompt, and confirming all components are present and correctly ordered.

- **AC-4** (Wake Loop): The agent reasons via an OpenAI-compatible chat completions API. When the LLM returns tool_calls, the runtime dispatches them (shell, plan, sleep) and feeds results back. When the LLM returns text, the agent implicitly sleeps. The wake loop respects an iteration cap (default 50); exceeding it terminates the wake with `iteration_cap` reason. Verified by an integration test where an agent receives a message, makes at least one tool call, and completes a wake cycle end-to-end.

- **AC-5** (Maintenance Cycle): After each wake ends, a single LLM call receives the previous identity, work list, wake transcript, and termination reason, and returns updated identity (prose), updated work list (prose), and a wake summary (≤500 chars). These are written as new versioned rows in PostgreSQL, never updating in place. Verified by checking that after a wake, new projection rows and a wake summary exist with correct content.

- **AC-6** (HTTP API): The runtime exposes REST endpoints via axum: `POST /api/agents` (create), `GET /api/agents` (list), `GET /api/agents/:id` (detail with current projections), `POST /api/agents/:id/messages` (send message), `GET /api/agents/:id/events` (event log). All endpoints return JSON. Verified by exercising each endpoint and confirming correct status codes and response shapes.

- **AC-7** (Wake Triggers): When a message is inserted into the event log, the runtime issues `NOTIFY agent_<id>`. A background listener receives notifications and triggers wake acquisition. Verified by sending a message to a resting agent and confirming it wakes within 5 seconds without polling.

- **AC-8** (Stale Wake Recovery): A background job runs periodically and detects agents whose `wake_started_at` is older than 2 hours while still in `awake` or `maintenance` status. It force-releases them to `asleep` and records a `stale_wake_recovery` event. Verified by setting an agent to `awake` with a stale timestamp and confirming recovery occurs.

- **AC-9** (Drain Check): After maintenance completes, the system checks for `message_received` events newer than the wake's high-water mark. If found, it immediately acquires a new wake via CAS (maintenance → awake) without returning to rest. If none, it transitions to asleep. Verified by sending a message during an active wake and confirming the drain check triggers a follow-up wake.

- **AC-10** (Local Admin Bootstrap): On first run with an empty database, the system runs migrations, creates a bootstrap local_admin user, a default organization, and a default workspace. The bootstrap is gated by an install-time token passed via environment variable. Verified by starting with an empty database and confirming the bootstrap completes with correct rows.

## Stack

| Concern | Choice | Source |
|---------|--------|--------|
| Runtime | Rust | preferences.md |
| Database | PostgreSQL | preferences.md |
| HTTP/API | axum | preferences.md |
| Async | tokio | preferences.md |
| SQL | sqlx (compile-time checked) | preferences.md |
| Serialization | serde + serde_json | preferences.md |
| HTTP client | reqwest | preferences.md (LLM API calls) |
| Logging | tracing + tracing-subscriber | Standard Rust ecosystem |

## Deployment Target

`self_host_individual` — a single Rust binary + PostgreSQL instance running on the user's machine or server. No cloud services, no containers required (though Docker Compose will be provided for convenience).

## Data Model

### Core tables (from TLA+ spec)

- `users` — authenticated humans (local_admin for bootstrap)
- `organizations` — tenant containers
- `workspaces` — agent grouping within orgs
- `organization_memberships` — user ↔ org roles
- `workspace_memberships` — user ↔ workspace roles
- `agents` — the continuous entities (status, wake_id, wake_started_at, iteration_count, owner_id, workspace_id, permission_mode, is_enabled, budget columns)
- `events` — append-only event log (agent_id, event_type, source, wake_id, tool_name, tool_input, tool_output, content, created_at)
- `agent_projections` — versioned identity + work list snapshots
- `wake_summaries` — compressed long-term memory per wake
- `prompt_templates` — versioned, immutable prompt templates with one-active-per-name constraint
- `llm_calls` — full LLM call provenance (model, tokens, cost, latency, prompt_hash, response_hash)
- `llm_call_prompts` — optional full prompt storage for reconstruction
- `tool_audit` — detailed tool execution records
- `user_sessions` — hashed session tokens
- `auth_audit` — authentication events

### Persistence

PostgreSQL is the single source of truth. All state is derived from the event log plus CAS-protected status columns. Append-only for events; versioned rows for projections.

## Estimated Cost

$0 — runs on localhost with a local PostgreSQL instance. No cloud services required. LLM API costs are usage-dependent and paid directly to the model provider (OpenRouter, OpenAI, etc.).

## Quality Tier

**Skyscraper** — TLA+ formal specification exists, event-sourced architecture, CAS concurrency control, multi-user system, security-critical (agents execute code), append-only audit trail. This demands: full test coverage (unit + integration + e2e), CI/CD, formal spec compliance, LLM observability, comprehensive audit schema, SBOM, and operational documentation.

## Clarifications Needed

1. **LLM provider default**: The spec says "OpenRouter/OpenAI compatible." Using a generic OpenAI-compatible client with base URL + API key configured via environment. Assuming this is correct.
2. **Zerobox availability**: Zerobox is listed as the sandbox but is a relatively new project. For v1, shell tool execution will use basic subprocess isolation (no filesystem/network restrictions). Zerobox integration is Phase 2 per security-architecture.md's own phasing.
3. **Constitution content**: The TLA+ spec describes what the constitution contains but no literal text. v1 will ship a default constitution template that covers the documented requirements.

## Deferred

- **Webhook ingress** (HMAC verification, dedup, normalization) — Phase 2
- **Inter-agent messaging** (send_message, cross-log recording, NOTIFY target) — Phase 2  
- **Tool permission system** (yolo/supervised/locked modes, approval workflow) — v1 uses yolo mode only; approval gates are Phase 2
- **Credential vault** (OneCLI integration, proxy injection, Zerobox secrets) — Phase 2
- **Process sandboxing** (Zerobox per-tool isolation) — Phase 2
- **Budget tracking** (per-agent USD limits, per-LLM-call cost tracking) — v1 tracks tokens but does not enforce budget caps
- **Event collapse** (backpressure for burst events) — Phase 2
- **Prompt injection defense** (scanning, canary tokens, rail pipeline) — Phase 2
- **MCP server support** (discovery, registry, agent-built servers) — Phase 2
- **Multi-tenancy** (org/workspace RBAC enforcement, policy sets, RLS) — v1 creates the schema but does not enforce RLS
- **Dashboard / UI** — Phase 2 (API-first for v1)
- **Greywall host sandbox** — Phase 2
- **Enterprise auth** (Entra OIDC, generic OIDC, SCIM) — Phase 2
- **SaaS features** (billing, subscriptions, abuse prevention) — Phase 2+
- **Compile/lint/test/typecheck verification tools** — Phase 2
- **Context character cap enforcement** — Phase 2 (iteration cap is v1)
- **Docker Compose** — included for convenience but not the primary deployment method
