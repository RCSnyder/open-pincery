# Open Pincery

**Open Pincery is an open-source multi-agent platform for durable, event-driven AI agents.** Each agent is a continuous entity with a stable identity, append-only event log, and wake/sleep lifecycle. Agents wake on messages, webhooks, or timers, work until done, and rest. Configure them by conversation. Orchestrate fleets via async messaging.

> **Status:** **v9.0 shipped** — security-and-correctness push merged to `main` 2026-05-08. Closes seven P0 acceptance criteria: AC-76 (12-payload sandbox-escape suite running live every CI), AC-77 (default-deny seccomp allowlist + SIGSYS event), AC-78 (per-agent SHA-256 event-log hash chain with `BEFORE INSERT` trigger + `pcy audit verify` + startup gate), AC-79 (prompt-injection floor: untrusted-content delimiters, per-wake canary, JSON-Schema tool-call gate, per-wake tool-call rate limit), AC-80 (single-use capability nonces, TTL-bounded, workspace-scoped, capability-shape-bound), AC-81 (TLA+ spec-coverage manifest + commit-msg hook), AC-82 (fine-grained ten-state wake lifecycle with CAS-only DB transitions and canonical-JSON `lifecycle_transition` events). v8.0 agentic-harness CLI polish, v7 credential vault, v5 operator onramp, v4 self-host hardening, and the v1–v3 feature set all carried forward. See [`DELIVERY.md`](DELIVERY.md) for the full delivery narrative and [`scaffolding/log.md`](scaffolding/log.md) for the per-AC BUILD/REVIEW/RECONCILE/VERIFY trail.
>
> **Next:** v9.1 dependency-hygiene + deferred items (capability-nonce sweeper, AC-77 audit-mode capture-and-iterate loop, AC-78 test-constant bumps). Strategic direction in [`docs/input/north-star-2026-04.md`](docs/input/north-star-2026-04.md): memory substrate (pgvector), codebase steward as the first Tier 1 mission, reasoner abstraction, and the groundwork for a sovereign substrate that can run a one-person company.

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

Five defense layers, each backed by shipped code on `main` and a closed acceptance criterion. The table reflects the runtime topology actually deployed by `docker compose up -d --wait`; aspirational design vocabulary from earlier scoping rounds is retained only inside `<!-- historical -->` markers below.

| Layer | Mechanism | Status | AC |
| --- | --- | --- | --- |
| Process sandbox | bubblewrap + seccomp default-deny allowlist + landlock ABI ≥ 6 + `pincery-init` exec wrapper + UID/capability drop | Shipped | AC-53 / AC-77 / AC-83 / AC-85 / AC-86 |
| Audit log | SHA-256 per-agent hash chain w/ Postgres `BEFORE INSERT` trigger + `pcy audit verify` + startup verify gate (exit 5) | Shipped | AC-78 |
| Capability gate | Single-use TTL nonces, workspace-scoped, capability-shape-bound | Shipped | AC-80 |
| Prompt-injection floor | Untrusted-content delimiter wrapping + per-wake canary + JSON-Schema tool-call gate + per-wake rate limit | Shipped | AC-79 |
| Credential vault | AES-256-GCM at rest + `PLACEHOLDER:` substitution via secret-proxy (plaintext never enters the reasoner process) | Shipped | AC-38 / AC-40 / AC-43 / AC-71 / AC-74 |

See [docs/SECURITY.md](docs/SECURITY.md) for the v9 threat model — adversary capabilities, in-scope vs out-of-scope attacks, and the vulnerability disclosure process.

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
- [docs/input/security-architecture.md](docs/input/security-architecture.md) — original five-layer security model writeup
- [docs/input/best-practices.md](docs/input/best-practices.md) — practices mapped to academic research

## Development

Open Pincery v9 introduces a Linux-only agent sandbox (AC-53 process
sandbox, AC-71 secret-injection proxy, AC-72 per-agent egress
allowlist) that relies on kernel primitives unavailable on macOS or
Windows hosts (bubblewrap, slirp4netns, landlock LSM, cgroup v2).
AC-75 ships a pinned Ubuntu 24.04 Docker "devshell" so every
contributor runs the identical toolchain against the identical kernel
surface.

<!-- historical -->
> **Terminology (historical).** Earlier design rounds named the Linux
> sandbox architecture _Zerobox_; the implementation that shipped is
> the bubblewrap + seccomp + landlock + `pincery-init` stack described
> in the Security Model table above. The unrelated Rust crate
> [`zeroize`](https://docs.rs/zeroize) is used under AC-74 for memory
> hygiene inside the secret-handling path — different concern, similar
> name, no relationship.
<!-- /historical -->

### On Linux

Native toolchain is supported directly:

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

### On macOS or Windows

Use the devshell wrapper to run the same commands inside the pinned
container. See the full walkthroughs in
[docs/runbooks/dev_setup_macos.md](docs/runbooks/dev_setup_macos.md) and
[docs/runbooks/dev_setup_windows.md](docs/runbooks/dev_setup_windows.md).

```bash
# macOS / Linux
./scripts/devshell.sh cargo test

# Windows (PowerShell)
.\scripts\devshell.ps1 cargo test
```

The wrapper invokes
`docker run --privileged --cgroupns=host ghcr.io/open-pincery/devshell:v9`
with the repo bind-mounted at `/work`, so edits on the host flow through
to the container immediately. Build artifacts are written to
`target/devshell/` to avoid colliding with any host Rust toolchain used
for editor integration.

If your shell inherited an old target directory, pin native Rust
artifacts to this checkout before you build:

```powershell
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'target\local-check'
cargo test
```

The devshell wrappers also support a separate host-side cache directory.
On Windows, this keeps the container-backed cargo cache under the repo
checkout while leaving native host artifacts separate:

```powershell
$env:OPEN_PINCERY_DEVSHELL_HOST_TARGET_DIR = Join-Path (Get-Location) 'target\devshell'
.\scripts\devshell.ps1 cargo test
```

You can still replace either value with another absolute path if you
intentionally want build artifacts on a different drive.

CI runs `tests/devshell_parity_test.rs` to confirm the wrapper,
`Dockerfile.devshell`, and runbooks stay in sync.

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

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.

---

## Quick Start

### Prerequisites

- Docker 24+
- Rust toolchain (for building the `pcy` CLI)
- An LLM API key (OpenRouter default, OpenAI-compatible also supported)

### Build the pcy CLI

```bash
cargo build --release --bin pcy
```

Then add it to your PATH:

```bash
# Linux / macOS / Git Bash
RELEASE_DIR="${CARGO_TARGET_DIR:-$PWD/target}/release"
export PATH="$RELEASE_DIR:$PATH"

# Windows PowerShell
$releaseDir = if ($env:CARGO_TARGET_DIR) { Join-Path $env:CARGO_TARGET_DIR 'release' } else { Join-Path $PWD 'target\release' }
$env:PATH = "$releaseDir;$env:PATH"
```

Or copy the binary to a directory already on your PATH:

```bash
# Linux / macOS
sudo cp "${CARGO_TARGET_DIR:-target}/release/pcy" /usr/local/bin/

# Windows PowerShell (admin)
$releaseDir = if ($env:CARGO_TARGET_DIR) { Join-Path $env:CARGO_TARGET_DIR 'release' } else { Join-Path $PWD 'target\release' }
Copy-Item (Join-Path $releaseDir 'pcy.exe') C:\Windows\System32\
```

### Web UI (fastest path)

1. Prepare env:

```bash
cp .env.example .env
```

Set non-placeholder values in `.env` for:

- `OPEN_PINCERY_BOOTSTRAP_TOKEN`
- `LLM_API_KEY`

1. Start stack and wait for health:

```bash
docker compose up -d --wait
curl -fsS http://localhost:8080/ready
```

1. Open the UI:

- `http://localhost:8080`

1. Bootstrap once (copy the returned session token into the UI login panel):

```bash
pcy --url http://localhost:8080 bootstrap --bootstrap-token "$OPEN_PINCERY_BOOTSTRAP_TOKEN"
```

### One-command end-to-end smoke test

If you just want to confirm the stack works, run:

```bash
pcy --url http://localhost:8080 demo --bootstrap-token "$OPEN_PINCERY_BOOTSTRAP_TOKEN"
```

This bootstraps (or logs in if already bootstrapped), creates a throwaway agent,
sends it a message, waits up to 60s for a real LLM reply, and prints it. If this
succeeds, your database, runtime, and LLM provider are all wired up correctly.

### pcy CLI path

Use this if you prefer terminal-first operations.

If `OPEN_PINCERY_URL=http://localhost:8080` is exported, the shortest command
forms are:

```bash
pcy login --bootstrap-token "$OPEN_PINCERY_BOOTSTRAP_TOKEN"
pcy agent create "my-agent"
pcy message AGENT_ID "hello from cli"
pcy events AGENT_ID
```

Equivalent explicit URL form:

```bash
# one-time bootstrap
pcy --url http://localhost:8080 bootstrap --bootstrap-token "$OPEN_PINCERY_BOOTSTRAP_TOKEN"

# create an agent
pcy --url http://localhost:8080 agent create "my-agent"

# send a message
pcy --url http://localhost:8080 message AGENT_ID "hello from cli"

# read events
pcy --url http://localhost:8080 events AGENT_ID
```

You can run the full scripted flow with:

```bash
bash scripts/smoke.sh
```

On Windows PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\smoke.ps1
```

Both smoke scripts check `CARGO_TARGET_DIR` before falling back to the
repo-local `target/` directory, so they keep working if you relocate the
native build cache to another drive.

### curl/HTTP appendix

```bash
curl -X POST http://localhost:8080/api/bootstrap \
  -H "Authorization: Bearer YOUR_BOOTSTRAP_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"email": "admin@example.com", "name": "Admin"}'

curl -X POST http://localhost:8080/api/agents \
  -H "Authorization: Bearer SESSION_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "my-agent"}'

curl -X POST http://localhost:8080/api/agents/AGENT_ID/messages \
  -H "Authorization: Bearer SESSION_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"content": "Hello, what can you do?"}'

curl http://localhost:8080/api/agents/AGENT_ID/events \
  -H "Authorization: Bearer SESSION_TOKEN"
```

### From Signed Release Binary

If you do not want to build from source, use release artifacts from GitHub Releases (AC-20).

```bash
# Example verification flow (replace filenames with your release assets)
cosign verify-blob open-pincery-linux-x86_64 \
  --signature open-pincery-linux-x86_64.sig \
  --certificate open-pincery-linux-x86_64.pem \
  --certificate-identity-regexp "https://github.com/RCSnyder/open-pincery/.*" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com"
```

### Troubleshooting

Anchor index:

- [bootstrap-401](#bootstrap-401)
- [rate-limit-429](#rate-limit-429)
- [silent-wake](#silent-wake)
- [compose-up-failed](#compose-up-failed)
- [already-bootstrapped](#already-bootstrapped)
- [lost-session-token](#lost-session-token)
- [log-format-json](#log-format-json)
- [metrics-scrape](#metrics-scrape)
- [backup-one-liner](#backup-one-liner)

#### bootstrap-401

- Confirm `.env` has the same `OPEN_PINCERY_BOOTSTRAP_TOKEN` you pass to `pcy login --bootstrap-token`.
- Verify the compose stack loaded your env: `docker compose config | grep OPEN_PINCERY_BOOTSTRAP_TOKEN`.

#### rate-limit-429

- Unauthenticated routes are limited to 10 req/min and authenticated routes to 60 req/min.
- Back off and retry after the `Retry-After` header.

#### silent-wake

- Check logs: `docker compose logs -f app`.
- Confirm `LLM_API_KEY` is valid and `LLM_API_BASE_URL` points to your provider.

#### compose-up-failed

- Check `docker compose logs -f app` and `docker compose logs -f db` for startup errors.
- Confirm required `.env` values are set (`OPEN_PINCERY_BOOTSTRAP_TOKEN`, `LLM_API_KEY`, `LLM_API_BASE_URL`).
- If this is a stale local state issue, run reset (`docker compose down -v`) and retry.

#### already-bootstrapped

- `/api/bootstrap` is one-time initialization. To get a new session token, use `pcy login --bootstrap-token <token>` or `POST /api/login`.
- If you need a full clean reset, run `docker compose down -v` and re-bootstrap.

#### lost-session-token

- Run `pcy login --bootstrap-token "$OPEN_PINCERY_BOOTSTRAP_TOKEN"` to get a fresh session token.
- The bootstrap token in `.env` is your recovery credential — keep it safe.

#### log-format-json

- Set `LOG_FORMAT=json` in `.env`, restart with `docker compose up -d`, then stream logs:

```bash
docker compose logs -f app
```

#### metrics-scrape

- Set `METRICS_ADDR=127.0.0.1:9090` in `.env` and restart.
- Scrape metrics:

```bash
curl -fsS http://127.0.0.1:9090/metrics | head
```

#### backup-one-liner

```bash
docker compose exec db pg_dump -U open_pincery open_pincery > backup.sql
```

See [`docs/runbooks/db-restore.md`](docs/runbooks/db-restore.md) for restore steps.

### Reset (wipe local state)

<a id="reset"></a>

```bash
docker compose down -v
```

This removes the Postgres volume and all local data.

### Going public with HTTPS

Default compose bindings are loopback-only (`127.0.0.1`) for safety.
To expose Open Pincery over HTTPS with Caddy:

1. Edit `Caddyfile.example` with your real domain and email.
1. Start with overlay:

```bash
docker compose -f docker-compose.yml -f docker-compose.caddy.yml up -d
```

1. Confirm Caddy is serving 80/443 and reverse-proxying to `app:8080`.

### Observability (optional)

- **Structured logs**: set `LOG_FORMAT=json` for JSON lines suitable for log pipelines.
- **Prometheus metrics**: set `METRICS_ADDR=127.0.0.1:9090` to expose `/metrics` on a dedicated listener.
- **Operator runbooks**: see [`docs/runbooks/`](docs/runbooks/) for stale wake, DB restore, rollback, rate-limit tuning, and webhook debugging.

### API

| Method | Path                             | Description                            |
| ------ | -------------------------------- | -------------------------------------- |
| GET    | `/health`                        | Liveness (always 200 while serving)    |
| GET    | `/ready`                         | Readiness (DB + migrations + bg tasks) |
| GET    | `/metrics`                       | Prometheus scrape (opt-in, own port)   |
| POST   | `/api/bootstrap`                 | One-time admin setup                   |
| POST   | `/api/login`                     | New session via bootstrap token        |
| POST   | `/api/agents`                    | Create agent (returns webhook_secret)  |
| GET    | `/api/agents`                    | List agents                            |
| GET    | `/api/agents/:id`                | Agent detail with projections          |
| PATCH  | `/api/agents/:id`                | Update agent name/enabled status       |
| DELETE | `/api/agents/:id`                | Soft-delete (disable with reason)      |
| POST   | `/api/agents/:id/messages`       | Send message (triggers wake)           |
| GET    | `/api/agents/:id/events`         | Event log                              |
| POST   | `/api/agents/:id/webhook/rotate` | Rotate per-agent webhook secret        |
| POST   | `/api/agents/:id/webhooks`       | Webhook ingress (HMAC-SHA256)          |

Compatibility note: older docs may refer to `/api/agents/:id/rotate-webhook-secret`.
The shipped v4/v5 route is `/api/agents/:id/webhook/rotate`.

All `/api/*` routes (except bootstrap) require `Authorization: Bearer <session_token>`.
Webhook routes require `X-Signature` header with HMAC-SHA256 of the body.

Rate limits: 10 requests/minute (unauthenticated), 60 requests/minute (authenticated). Exceeding the limit returns `429 Too Many Requests` with a `Retry-After` header.

### Tests

```bash
cargo test -- --test-threads=1
```
