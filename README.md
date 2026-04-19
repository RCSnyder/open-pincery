# Open Pincery

**Open Pincery is an open-source multi-agent platform for durable, event-driven AI agents.** Each agent is a continuous entity with a stable identity, append-only event log, and wake/sleep lifecycle. Agents wake on messages, webhooks, or timers, work until done, and rest. Configure them by conversation. Orchestrate fleets via async messaging.

> **Status:** v3 runtime implemented. Full v1 + v2 feature set (CAS lifecycle, event log, LLM-powered wake/sleep, maintenance projections, HTTP API, PostgreSQL persistence, graceful shutdown, Docker Compose, rate limiting, HMAC webhook ingress, agent management) plus v3 operational polish: structured JSON logging (`LOG_FORMAT=json`), Prometheus `/metrics` endpoint (`METRICS_ADDR=...`), `/health` + `/ready` split with per-subsystem failure reporting, GitHub Actions CI (fmt + clippy + tests + cargo-deny), signed release workflow with CycloneDX SBOMs (cosign keyless), and five operator runbooks under `docs/runbooks/`.

## Why Another Agent Platform?

Most AI agent frameworks treat agents as ephemeral function calls — stateless, session-scoped, and gone between requests. Open Pincery inverts that. An agent is a **continuous entity**: the same identity, the same accumulated history, the same ongoing responsibilities, persisting across every interaction and every span of time between them.

The architecture draws on event sourcing, the actor model, and distributed systems engineering to give agents properties that matter for real work:

- **Durable identity** — agents maintain a prose self-description that evolves through conversation, not config files
- **Append-only event log** — the complete record of everything that happened, queryable and replayable
- **Wake/sleep lifecycle** — agents rest by default, wake on events, work until done, then rest again
- **Async inter-agent messaging** — agents communicate by message passing with no shared transcript
- **Shell as universal tool** — one programmable executor instead of dozens of individual tool definitions
- **Self-configuration** — reshape an agent's behavior by talking to it; the maintenance process captures changes durably

## Architecture at a Glance

```text
┌──────────────────────────────────────────────────────┐
│                    Open Pincery                      │
│                                                      │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐               │
│  │ Agent A │  │ Agent B │  │ Agent C │  ...          │
│  │         │  │         │  │         │               │
│  │ Identity│  │ Identity│  │ Identity│               │
│  │WorkList │  │WorkList │  │WorkList │               │
│  │EventLog │  │EventLog │  │EventLog │               │
│  └────┬────┘  └─────┬───┘  └──────┬──┘               │
│       │  async msg  │  async msg  │                  │
│       └─────────────┼─────────────┘                  │
│                     │                                │
│  ┌──────────────────┴──────────────────┐             │
│  │         Runtime Harness             │             │
│  │  Wake/Sleep · Maintenance · CAS     │             │
│  └──────────────────┬──────────────────┘             │
│                     │                                │
│  ┌──────────────────┴──────────────────┐             │
│  │     PostgreSQL (event store)        │             │
│  └─────────────────────────────────────┘             │
└──────────────────────────────────────────────────────┘
        ▲            ▲            ▲
        │            │            │
    Webhooks      Timers      Messages
```

Each agent has its own event stream, identity projection, and work list. The runtime harness orchestrates wake/sleep cycles using CAS (compare-and-swap) to ensure exactly one wake is active per agent at any time. PostgreSQL is the single source of truth — no message brokers, no cloud-specific services.

## Core Concepts

| Concept                       | Description                                                                                                                |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| **Continuous Agent**          | A persistent entity with durable identity, work list, and append-only event log. Not a session.                            |
| **Wake Cycle**                | A bounded active episode: wake on event → reason → use tools → sleep when done.                                            |
| **Between-Wakes Maintenance** | A single LLM call after each wake updates identity, work list, and produces a wake summary.                                |
| **Event Log**                 | Append-only record of everything: messages, tool calls, timer firings, wake boundaries. Source of truth.                   |
| **Projections**               | Identity and work list — free-form prose derived from the event log, maintained incrementally.                             |
| **Shell Tool**                | A programmable executor where agents write programs, not individual tool calls. Intermediate data stays out of the prompt. |
| **Semantic Stop**             | No kill switch. Say "stop" in a message. The agent reads it and stops. Say "carry on" and it resumes.                      |

## Stack

| Concern     | Choice                                          |
| ----------- | ----------------------------------------------- |
| Runtime     | Rust                                            |
| Database    | PostgreSQL                                      |
| HTTP/API    | axum                                            |
| Async       | tokio                                           |
| SQL         | sqlx (compile-time checked)                     |
| Sandbox     | zerobox (per-tool isolation)                    |
| Credentials | Vault/proxy model (LLM never sees real secrets) |

## Security Model

Six defense layers, from innermost to outermost:

1. **[Zerobox](https://github.com/afshinm/zerobox)** — deny-by-default per-tool process sandboxing with secret injection via proxy
2. **[OneCLI](https://github.com/onecli/onecli)** — credential vault where agents authenticate with proxy tokens; real credentials injected at the gateway
3. **Prompt injection defense** — delimiter enforcement, instruction hierarchy, canary tokens, rate limiting
4. **[Greywall](https://github.com/GreyhavenHQ/greywall)** — outer host-level sandbox wrapping the entire runtime
5. **Database security** — Postgres RLS, compile-time checked queries, append-only audit
6. **Webhook/API security** — HMAC-SHA256 verification, SHA-256 dedup, rate limiting, TLS

## Deployment Modes

Open Pincery is designed for four deployment modes. Self-hosting is first-class — the runtime functions without any proprietary control plane.

| Mode                     | Description                                  |
| ------------------------ | -------------------------------------------- |
| `self_host_individual`   | Single-user, local binary + Postgres         |
| `self_host_team`         | Team deployment with optional split topology |
| `saas_managed`           | Hosted service with GitHub OAuth             |
| `enterprise_self_hosted` | Entra/OIDC, SCIM, BYOK                       |

## How It Differs

| Property            | Open Pincery                              | Typical Agent Frameworks                    |
| ------------------- | ----------------------------------------- | ------------------------------------------- |
| Agent lifetime      | Continuous — persists across interactions | Ephemeral — created per request             |
| Memory              | Event-sourced log + prose projections     | Chat history, RAG, or vector DB bolt-on     |
| Concurrency control | CAS lifecycle, exactly-one-wake           | None or implicit                            |
| Configuration       | Conversation-driven, durable              | Code, YAML, config files                    |
| Tool model          | Programmable shell executor               | Fixed tool registry                         |
| Multi-agent         | Async message passing, no shared state    | Shared transcripts or orchestrator patterns |
| Infrastructure      | Rust + Postgres, no cloud lock-in         | Python + various cloud services             |

## Project Structure

```text
src/
  api/                # HTTP handlers (agents, events, messages, bootstrap)
  background/         # Listener (NOTIFY/LISTEN) and stale recovery
  runtime/            # Wake loop, maintenance, LLM client, tools, prompt assembly
  models/             # Database models (agents, events, projections, etc.)
  config.rs           # Environment configuration
  db.rs               # Pool creation + migrations
  error.rs            # Unified error type
  auth.rs             # Token hashing
migrations/           # PostgreSQL schema (16 migrations)
tests/                # Integration tests (25 tests across 14 files)
docker-compose.yml    # App + PostgreSQL (Docker deploy)
docs/
  input/              # Architecture specs, TLA+ model, design docs
  reference/          # Audit reports, adoption plans
scaffolding/          # Scope, design, readiness, experiment log
```

## Getting Started

To explore the architecture:

```bash
git clone https://github.com/RCSnyder/open-pincery.git
cd open-pincery
```

Start with:

- [docs/input/technical-stack.md](docs/input/technical-stack.md) — implementation stack and crate choices
- [docs/input/OpenPinceryAgent.tla](docs/input/OpenPinceryAgent.tla) — formal TLA+ specification of the agent state machine. Copy into [TLA+ Process Studio](https://tlaplus-process-studio.com/) for visualizing the state machine of the system.
- [docs/input/security-architecture.md](docs/input/security-architecture.md) — six-layer security model
- [docs/input/best-practices.md](docs/input/best-practices.md) — practices mapped to academic research

## References

The Continuous Agent Architecture that Open Pincery implements is described in detail in an upcoming publication by the original author, to be released under MIT license.

<!-- TODO: Add citation and link to the Continuous Agent Architecture paper
     (author of continuous_agent_architecture.pdf) once published under MIT. -->

### Academic Foundation

The platform's design practices are informed by emerging research in agentic software engineering:

> Agentic Pipelines in Embedded Software Engineering: Emerging Practices and Challenges.
> arXiv:2601.10220 \[cs.SE\]. [https://doi.org/10.48550/arXiv.2601.10220](https://doi.org/10.48550/arXiv.2601.10220)

### Additional Resources

- [Agentic Strategy Lab — Deliverables](https://agenticstrategylab.com/deliverables) — research and frameworks for agentic system design considerations. Only that page specifically.

## License

[MIT](LICENSE)

---

## Quick Start

### Prerequisites

- Rust 1.75+
- Docker (for PostgreSQL) or an existing PostgreSQL 16+ instance
- An OpenAI-compatible LLM API key (e.g., OpenRouter, OpenAI)

### 1. Start PostgreSQL

**Option A — Docker Compose (full stack):**

```bash
# Start both the app and PostgreSQL
cp .env.example .env
# Edit .env: set LLM_API_KEY and OPEN_PINCERY_BOOTSTRAP_TOKEN
docker compose up -d
```

The app starts on `http://localhost:8080` with migrations applied automatically. Skip to step 4.

**Option B — PostgreSQL only (development):**

```bash
docker compose up -d db
```

This starts Postgres on `localhost:5432` with user/password/database all set to `open_pincery`.

### 2. Configure Environment

```bash
cp .env.example .env
```

Edit `.env` and set at minimum:

- `LLM_API_KEY` — your LLM provider API key
- `OPEN_PINCERY_BOOTSTRAP_TOKEN` — a secret for the one-time admin setup

### 3. Build and Run

```bash
cargo build --release
```

**Linux/macOS:**

```bash
source .env && ./target/release/open-pincery
```

**Windows (PowerShell):**

```powershell
Get-Content .env | ForEach-Object { if ($_ -match '^\s*([^#][^=]+)=(.*)$') { [System.Environment]::SetEnvironmentVariable($matches[1], $matches[2]) } }
.\target\release\open-pincery.exe
```

The server starts on `http://localhost:8080`. You should see `Starting server` in the logs. Migrations run automatically on first start.

### 4. Bootstrap (One-Time)

Create the first admin user:

```bash
curl -X POST http://localhost:8080/api/bootstrap \
  -H "Authorization: Bearer YOUR_BOOTSTRAP_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"email": "admin@example.com", "name": "Admin"}'
```

This returns a `session_token`. Save it — all subsequent API calls require it.

### 5. Create an Agent and Send a Message

```bash
# Create an agent
curl -X POST http://localhost:8080/api/agents \
  -H "Authorization: Bearer SESSION_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "my-agent"}'

# Send a message (triggers a wake cycle)
curl -X POST http://localhost:8080/api/agents/AGENT_ID/messages \
  -H "Authorization: Bearer SESSION_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"content": "Hello, what can you do?"}'

# Check the event log
curl http://localhost:8080/api/agents/AGENT_ID/events \
  -H "Authorization: Bearer SESSION_TOKEN"
```

### 6. Verify Health

```bash
curl http://localhost:8080/health
# {"status":"ok"}

curl http://localhost:8080/ready
# 200 {"status":"ready"} when DB is reachable, migrations are applied, and
# both background tasks (LISTEN/NOTIFY + stale recovery) are alive.
# Otherwise 503 with {"status":"not_ready","failing":"database" |
#   "migrations" | "background_task:listener" |
#   "background_task:stale_recovery" | "background_tasks"}.
```

### 7. Observability (optional)

- **Structured logs**: set `LOG_FORMAT=json` to emit one JSON object per line
  (`timestamp`, `level`, `target`, `fields.message`, plus span context) for
  ingestion into Loki, Vector, Elastic, etc. Any other value produces
  human-readable output.
- **Prometheus metrics**: set `METRICS_ADDR=127.0.0.1:9090` to spawn a
  separate `/metrics` endpoint. Exposes counters for wakes, LLM calls,
  tokens consumed, tool calls, webhook deliveries, rate-limit rejections;
  a gauge for active wakes; and a histogram of wake durations. Leave unset
  to disable metrics entirely (zero cost).
- **Operator runbooks**: see [`docs/runbooks/`](docs/runbooks/) for
  diagnostic + remediation playbooks (stale wakes, DB restore, migration
  rollback, rate-limit tuning, webhook debugging).

### API

| Method | Path                       | Description                            |
| ------ | -------------------------- | -------------------------------------- |
| GET    | `/health`                  | Liveness (always 200 while serving)    |
| GET    | `/ready`                   | Readiness (DB + migrations + bg tasks) |
| GET    | `/metrics`                 | Prometheus scrape (opt-in, own port)   |
| POST   | `/api/bootstrap`           | One-time admin setup                   |
| POST   | `/api/agents`              | Create agent (returns webhook_secret)  |
| GET    | `/api/agents`              | List agents                            |
| GET    | `/api/agents/:id`          | Agent detail with projections          |
| PATCH  | `/api/agents/:id`          | Update agent name/enabled status       |
| DELETE | `/api/agents/:id`          | Soft-delete (disable with reason)      |
| POST   | `/api/agents/:id/messages` | Send message (triggers wake)           |
| GET    | `/api/agents/:id/events`   | Event log                              |
| POST   | `/api/agents/:id/webhooks` | Webhook ingress (HMAC-SHA256)          |

All `/api/*` routes (except bootstrap) require `Authorization: Bearer <session_token>`.
Webhook routes require `X-Signature` header with HMAC-SHA256 of the body.

Rate limits: 10 requests/minute (unauthenticated), 60 requests/minute (authenticated). Exceeding the limit returns `429 Too Many Requests` with a `Retry-After` header.

### Tests

```bash
cargo test -- --test-threads=1
```
