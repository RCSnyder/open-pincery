# Open Pincery

**Open Pincery is an open-source multi-agent platform for durable, event-driven AI agents.** Each agent is a continuous entity with a stable identity, append-only event log, and wake/sleep lifecycle. Agents wake on messages, webhooks, or timers, work until done, and rest. Configure them by conversation. Orchestrate fleets via async messaging.

> **Status:** Early-stage. The architecture is specified ([TLA+ spec](docs/input/OpenPinceryAgent.tla), [design docs](docs/input/)), the implementation stack is chosen, but the runtime is not yet buildable.

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
docs/
  input/              # Architecture specs, TLA+ model, design docs
  reference/          # Audit reports, adoption plans
preferences.md        # Stack + conventions for the build agent
scaffolding/          # Scope, design, readiness, experiment log
<project code>        # The runtime (not yet implemented)
```

## Getting Started

> **Note:** The runtime is not yet buildable. The architecture is fully specified and the implementation stack is chosen. See [docs/input/](docs/input/) for the complete design.

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
