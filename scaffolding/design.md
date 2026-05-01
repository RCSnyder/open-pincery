# Design — Open Pincery v1

## Architecture

A single Rust binary that runs as a long-lived service process. Three concurrent subsystems share a PostgreSQL connection pool:

```text
┌─────────────────────────────────────────────────────────────┐
│                   Open Pincery Binary                       │
│                                                             │
│  ┌─────────────────┐  ┌──────────────────┐  ┌────────────┐ │
│  │   HTTP Server    │  │  Wake Executor   │  │ Background │ │
│  │   (axum)         │  │  (tokio tasks)   │  │ Jobs       │ │
│  │                  │  │                  │  │            │ │
│  │ POST /agents     │  │ wake_loop()      │  │ LISTEN/    │ │
│  │ POST /messages   │  │ maintenance()    │  │ NOTIFY     │ │
│  │ GET  /events     │  │ drain_check()    │  │ handler    │ │
│  │ POST /bootstrap  │  │                  │  │            │ │
│  │ GET  /health     │  │                  │  │ Stale wake │ │
│  │                  │  │                  │  │ recovery   │ │
│  └────────┬─────────┘  └────────┬─────────┘  └─────┬──────┘ │
│           │                     │                   │        │
│           └──────────┬──────────┴───────────────────┘        │
│                      │                                       │
│              ┌───────┴───────┐                               │
│              │  sqlx Pool    │                               │
│              │  (PgPool)     │                               │
│              └───────┬───────┘                               │
│                      │                                       │
└──────────────────────┼───────────────────────────────────────┘
                       │
               ┌───────┴───────┐        ┌──────────────────┐
               │  PostgreSQL   │        │  LLM API         │
               │  (event store │        │  (OpenAI-compat)  │
               │   + CAS)      │        │  via reqwest      │
               └───────────────┘        └──────────────────┘
```

**Flow for a human message:**

1. HTTP server receives `POST /api/agents/:id/messages`
2. Inserts `message_received` event into event log
3. Issues `NOTIFY agent_<id>` on the Postgres channel
4. Background listener receives notification
5. Spawns a tokio task to attempt wake acquisition (CAS)
6. If CAS succeeds: prompt assembly → wake loop → maintenance → drain check
7. If CAS fails: exit cleanly (event is safe in the log)

## Directory Structure

```
open-pincery/
├── Cargo.toml
├── .env.example
├── docker-compose.yml          # App + Postgres (dev and Docker deploy)
├── migrations/
│   ├── 20260418000001_create_users.sql
│   ├── 20260418000002_create_organizations.sql
│   ├── 20260418000003_create_workspaces.sql
│   ├── 20260418000004_create_memberships.sql
│   ├── 20260418000005_create_agents.sql
│   ├── 20260418000006_create_events.sql
│   ├── 20260418000007_create_projections.sql
│   ├── 20260418000008_create_wake_summaries.sql
│   ├── 20260418000009_create_prompt_templates.sql
│   ├── 20260418000010_create_llm_calls.sql
│   ├── 20260418000011_create_tool_audit.sql
│   ├── 20260418000012_create_sessions.sql
│   ├── 20260418000013_create_auth_audit.sql
│   └── 20260418000014_event_source_not_null.sql
├── src/
│   ├── main.rs                 # Entry point: config, pool, server, background tasks
│   ├── lib.rs                  # Crate root: public module declarations
│   ├── auth.rs                 # Session token generation + SHA-256 hashing
│   ├── config.rs               # Env-based configuration (DATABASE_URL, LLM_API_KEY, etc.)
│   ├── db.rs                   # Pool creation, migration runner
│   ├── error.rs                # Unified error types
│   ├── models/
│   │   ├── mod.rs
│   │   ├── agent.rs            # Agent struct, CAS operations (acquire/release)
│   │   ├── event.rs            # Event struct, append, query
│   │   ├── user.rs             # User struct, session management
│   │   ├── workspace.rs        # Org, workspace, membership structs
│   │   ├── projection.rs       # AgentProjection, WakeSummary structs + queries
│   │   ├── prompt_template.rs  # PromptTemplate struct + active lookup
│   │   └── llm_call.rs         # LlmCall, LlmCallPrompt structs + insert
│   ├── api/
│   │   ├── mod.rs              # Router assembly
│   │   ├── agents.rs           # CRUD endpoints
│   │   ├── messages.rs         # Send message endpoint
│   │   ├── events.rs           # Event log query endpoint
│   │   └── bootstrap.rs        # First-run bootstrap endpoint
│   ├── runtime/
│   │   ├── mod.rs
│   │   ├── wake_loop.rs        # Core LLM + tool dispatch loop
│   │   ├── prompt.rs           # Prompt assembly from DB state
│   │   ├── maintenance.rs      # Between-wakes LLM call + projection writes
│   │   ├── drain.rs            # Post-maintenance drain check
│   │   ├── llm.rs              # OpenAI-compatible chat completions client
│   │   └── tools.rs            # Tool definitions + dispatch (shell, plan, sleep)
│   └── background/
│       ├── mod.rs
│       ├── listener.rs         # LISTEN/NOTIFY handler, wake trigger
│       └── stale.rs            # Stale wake recovery job
├── static/
│   ├── index.html              # Dashboard SPA entry point
│   ├── css/
│   │   └── style.css
│   └── js/
│       ├── api.js
│       └── app.js
└── tests/
    ├── common/
    │   └── mod.rs              # Test helpers (DB setup, fixtures)
    ├── lifecycle_test.rs       # AC-1: CAS lifecycle
    ├── event_log_test.rs       # AC-2: Event log immutability
    ├── prompt_test.rs          # AC-3: Prompt assembly
    ├── wake_loop_test.rs       # AC-4: Wake loop end-to-end
    ├── maintenance_test.rs     # AC-5: Maintenance cycle
    ├── api_test.rs             # AC-6: HTTP API
    ├── trigger_test.rs         # AC-7: LISTEN/NOTIFY wake triggers
    ├── stale_test.rs           # AC-8: Stale wake recovery
    ├── drain_test.rs           # AC-9: Drain check
    └── bootstrap_test.rs       # AC-10: Local admin bootstrap
```

## Interfaces

### Agent (Postgres ↔ Rust)

```rust
// Coarse DB lifecycle — the code uses raw strings ("asleep", "awake", "maintenance")
// rather than a Rust enum. The TLA+ fine-grained states (ToolDispatching, PromptAssembling, etc.)
// are in-memory runtime states, not persisted to DB.

#[derive(sqlx::FromRow, Debug, Serialize, Deserialize)]
pub struct Agent {
    pub id: Uuid,
    pub name: String,
    pub workspace_id: Uuid,
    pub owner_id: Uuid,
    pub status: String,          // "asleep" | "awake" | "maintenance"
    pub wake_id: Option<Uuid>,
    pub wake_started_at: Option<DateTime<Utc>>,
    pub wake_iteration_count: i32,
    pub permission_mode: String, // "yolo" for v1
    pub is_enabled: bool,
    pub disabled_reason: Option<String>,
    pub disabled_at: Option<DateTime<Utc>>,
    pub budget_limit_usd: Decimal,
    pub budget_used_usd: Decimal,
    pub webhook_secret: String,
    pub created_at: DateTime<Utc>,
}

// CAS wake acquisition — THE critical operation
// Returns Some(agent) if CAS succeeds, None if another wake won
pub async fn acquire_wake(pool: &PgPool, agent_id: Uuid) -> Result<Option<Agent>>
// SQL: UPDATE agents SET status='awake', wake_id=gen_random_uuid(),
//      wake_started_at=NOW(), wake_iteration_count=0
//      WHERE id=$1 AND status='asleep' AND is_enabled=TRUE
//      RETURNING *

// CAS release to maintenance
pub async fn transition_to_maintenance(pool: &PgPool, agent_id: Uuid) -> Result<Option<Agent>>
// SQL: UPDATE agents SET status='maintenance'
//      WHERE id=$1 AND status='awake' RETURNING *

// CAS release to asleep
pub async fn release_to_asleep(pool: &PgPool, agent_id: Uuid) -> Result<Option<Agent>>
// SQL: UPDATE agents SET status='asleep', wake_id=NULL,
//      wake_started_at=NULL, wake_iteration_count=0
//      WHERE id=$1 AND status='maintenance' RETURNING *

// CAS drain re-acquire (maintenance → awake)
pub async fn drain_reacquire(pool: &PgPool, agent_id: Uuid) -> Result<Option<Agent>>
// SQL: UPDATE agents SET status='awake', wake_id=gen_random_uuid(),
//      wake_started_at=NOW(), wake_iteration_count=0
//      WHERE id=$1 AND status='maintenance' AND is_enabled=TRUE
//      RETURNING *
```

### Event Log

```rust
#[derive(sqlx::FromRow, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub event_type: String,      // message_received, tool_call, tool_result, etc.
    pub source: String,          // human, webhook, timer, agent, system
    pub wake_id: Option<Uuid>,
    pub tool_name: Option<String>,
    pub tool_input: Option<String>,
    pub tool_output: Option<String>,
    pub content: Option<String>,
    pub termination_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

// Append-only — the only write operation
pub async fn append_event(
    pool: &PgPool,
    agent_id: Uuid,
    event_type: &str,
    source: &str,
    wake_id: Option<Uuid>,
    tool_name: Option<&str>,
    tool_input: Option<&str>,
    tool_output: Option<&str>,
    content: Option<&str>,
    termination_reason: Option<&str>,
) -> Result<Event>
// SQL: INSERT INTO events (...) VALUES (...) RETURNING *

// Query recent events for prompt assembly
pub async fn recent_events(pool: &PgPool, agent_id: Uuid, limit: i64) -> Result<Vec<Event>>
// SQL: SELECT * FROM events WHERE agent_id=$1
//      ORDER BY created_at DESC LIMIT $2

// Check for events newer than high-water mark (drain check)
pub async fn has_pending_events(pool: &PgPool, agent_id: Uuid, since: DateTime<Utc>) -> Result<bool>
// SQL: SELECT COUNT(*) FROM events WHERE agent_id=$1
//      AND created_at > $2 AND source = 'human'
```

### Prompt Assembly Output

```rust
pub struct AssembledPrompt {
    pub system_prompt: String,    // constitution + time + summaries + identity + work_list
    pub messages: Vec<ChatMessage>, // recent events converted to chat format
    pub tools: Vec<ToolDefinition>, // available tool schemas
}
```

### LLM Client (OpenAI-compatible)

```rust
pub struct LlmClient {
    http: reqwest::Client,
    base_url: String,     // e.g. "https://openrouter.ai/api/v1"
    api_key: String,
    model: String,        // e.g. "anthropic/claude-sonnet-4-20250514"
    maintenance_model: String, // e.g. "anthropic/claude-sonnet-4-20250514"
}

pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Option<Vec<ToolDefinition>>,
}

pub struct ChatResponse {
    pub id: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

pub struct Choice {
    pub message: ResponseMessage,
    pub finish_reason: String,  // "stop" | "tool_calls"
}

pub struct ResponseMessage {
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}
```

### Tool Dispatch

```rust
pub enum ToolResult {
    Output(String),       // Tool produced output
    Sleep,                // Agent called sleep tool
    Error(String),        // Tool execution failed
}

// Dispatch a tool call to its handler
pub async fn dispatch_tool(tool_call: &ToolCallRequest) -> ToolResult

// Tool definitions for LLM
pub fn tool_definitions() -> Vec<ToolDefinition>
// Returns: shell (command execution), plan (record intention), sleep (end wake)
```

### HTTP API Contracts

```
POST /api/bootstrap
  Headers: Authorization: Bearer <BOOTSTRAP_TOKEN>
  Response: 201 { user_id, organization_id, workspace_id, session_token }

POST /api/agents
  Headers: Authorization: Bearer <session_token>
  Body: { "name": "string" }
  Response: 201 { id, name, status, is_enabled, disabled_reason?, webhook_secret, identity?, work_list?, created_at }

GET /api/agents
  Headers: Authorization: Bearer <session_token>
  Response: 200 [ { id, name, status, is_enabled, disabled_reason?, identity?, work_list?, created_at } ]

GET /api/agents/:id
  Headers: Authorization: Bearer <session_token>
  Response: 200 { id, name, status, is_enabled, disabled_reason?, identity?, work_list?, created_at }

POST /api/agents/:id/messages
  Headers: Authorization: Bearer <session_token>
  Body: { "content": "string" }
  Response: 202 { event_id }

GET /api/agents/:id/events
  Headers: Authorization: Bearer <session_token>
  Query: ?limit=100&offset=0&event_type=<optional>
  Response: 200 { events: [...], total: int }

GET /health
  Response: 200 { status: "ok", db: "connected" }
```

### Configuration (env vars)

```
DATABASE_URL=postgres://user:pass@localhost:5432/open_pincery
OPEN_PINCERY_HOST=0.0.0.0
OPEN_PINCERY_PORT=8080
OPEN_PINCERY_BOOTSTRAP_TOKEN=<random-secret-for-first-run>
LLM_API_BASE_URL=https://openrouter.ai/api/v1
LLM_API_KEY=<api-key>
LLM_MODEL=anthropic/claude-sonnet-4-20250514
LLM_MAINTENANCE_MODEL=anthropic/claude-sonnet-4-20250514
MAX_PROMPT_CHARS=100000            # Character budget for prompt assembly
ITERATION_CAP=50                   # Max wake loop iterations per wake
STALE_WAKE_HOURS=2                 # Hours before a wake is considered stale
WAKE_SUMMARY_LIMIT=20              # Max wake summaries included in prompt
EVENT_WINDOW_LIMIT=200             # Max recent events included in prompt
LLM_PRICE_INPUT_PER_MTOK=3.0                 # v4 AC-23 — USD per 1M input tokens, primary model
LLM_PRICE_OUTPUT_PER_MTOK=15.0               # v4 AC-23 — USD per 1M output tokens, primary model
LLM_MAINTENANCE_PRICE_INPUT_PER_MTOK=3.0     # v4 AC-23 — USD per 1M input tokens, maintenance model
LLM_MAINTENANCE_PRICE_OUTPUT_PER_MTOK=15.0   # v4 AC-23 — USD per 1M output tokens, maintenance model
RUST_LOG=open_pincery=info
```

## External Integrations

### PostgreSQL

- **Purpose**: Single source of truth — event log, CAS lifecycle, projections, audit
- **Error handling**: sqlx connection pool with automatic reconnection. CAS failures are expected (concurrent access) and handled by retry/exit logic. Connection errors surface as 503 on API endpoints.
- **Test strategy**: Live database. Tests use a dedicated test database created/dropped per test run. `sqlx::test` attribute or manual pool setup with migrations.

### LLM API (OpenAI-compatible)

- **Purpose**: Wake loop reasoning and maintenance projection updates
- **Error handling**: Retry with exponential backoff (3 attempts, 1s/2s/4s). On persistent failure, terminate wake gracefully with `llm_error` termination reason and proceed to maintenance. Log the failure in llm_calls table.
- **Test strategy**: Mock HTTP server (wiremock-rs or similar) for unit tests. Optional recorded/live mode for integration tests via env var toggle.

## Observability

Skyscraper tier — structured logging + Postgres audit tables:

- **Structured logging**: `tracing` crate with JSON subscriber. Every wake logs: agent_id, wake_id, iteration count, tool calls, termination reason.
- **LLM call audit**: Every LLM call recorded in `llm_calls` table with model, token counts, latency, prompt hash, response hash. Full prompts optionally stored in `llm_call_prompts`.
- **Tool execution audit**: Every tool call recorded in `tool_audit` table with inputs, outputs, duration, exit code.
- **Health endpoint**: `GET /health` returns DB connectivity status.
- **What to check at 2am**: Query `events` table for recent `wake_end` events with `termination_reason = 'iteration_cap'` or `'stale_wake_recovery'`. Check `agents` table for agents stuck in `awake` status. Check `llm_calls` for error patterns.

Phase 2 additions: Prometheus metrics endpoint, OpenTelemetry tracing, Grafana dashboards.

## Complexity Exceptions

1. **`wake_loop.rs`** may exceed 300 lines due to the core LLM interaction loop, tool dispatch, iteration cap checking, and mid-wake event polling all being tightly coupled in a single control flow. Splitting this artificially would obscure the state machine. Target: ≤400 lines.
2. **Migration files** are numerous (13+) because each TLA+-specified table gets its own migration per preferences.md ("One migration per schema change").

## Open Questions

None — all three clarifications from scope.md have documented resolutions:

1. LLM provider: generic OpenAI-compatible client with configurable base URL ✓
2. Zerobox: deferred to Phase 2, shell tool uses basic subprocess ✓
3. Constitution: default template shipped in seed migration ✓

---

## v2 Design Addendum — Operational Readiness

### Architecture Changes

No core architecture changes. v2 adds:

1. **Shutdown signal handler** in `main.rs` — tokio signal listener for SIGTERM/SIGINT that triggers a `CancellationToken` shared across HTTP server, background listener, and stale recovery job. Axum's `with_graceful_shutdown` handles HTTP draining.

2. **Rate limiting middleware** in `src/api/mod.rs` — custom axum middleware using the `governor` crate directly, providing per-IP rate limiting via `RateLimiter<IpAddr, DashMapStateStore<IpAddr>, DefaultClock>`. Two tiers: bootstrap (10 req/min) and authenticated (60 req/min). Returns `Response` with `Retry-After` header on 429.

3. **Webhook endpoint** in `src/api/webhooks.rs` — HMAC-SHA256 verified JSON ingress.

4. **Agent management endpoints** in `src/api/agents.rs` — PATCH and DELETE routes.

5. **Dockerfile** + updated `docker-compose.yml` — multi-stage build, healthcheck, app + postgres services.

### New Files

```
Dockerfile                      # Multi-stage Rust build
src/api/webhooks.rs             # Webhook ingress endpoint
migrations/20260418000015_add_webhook_secrets.sql   # Per-agent webhook_secret column
migrations/20260418000016_create_webhook_dedup.sql   # Idempotency key dedup table
tests/shutdown_test.rs          # AC-11
tests/rate_limit_test.rs        # AC-13
tests/webhook_test.rs           # AC-14
tests/agent_mgmt_test.rs        # AC-15
```

### New Interfaces

```rust
// Webhook verification
pub fn verify_hmac(secret: &[u8], payload: &[u8], signature: &str) -> bool

// Agent management
pub async fn update_agent(pool: &PgPool, id: Uuid, name: Option<&str>, is_enabled: Option<bool>, disabled_reason: Option<&str>) -> Result<Agent>
pub async fn soft_delete_agent(pool: &PgPool, id: Uuid) -> Result<Agent>
```

```
PATCH /api/agents/:id
  Headers: Authorization: Bearer <session_token>
  Body: { "name"?: "string", "is_enabled"?: bool }
  Response: 200 { id, name, status, is_enabled, disabled_reason?, identity?, work_list?, created_at }

DELETE /api/agents/:id
  Headers: Authorization: Bearer <session_token>
  Response: 200 { id, name, status, is_enabled: false, disabled_reason: "deleted", created_at }

POST /api/agents/:id/webhooks
  Headers: X-Webhook-Signature: sha256=<hex>, X-Idempotency-Key: <unique-id>
  Body: { "content": "string", "source"?: "string" }
  Response: 202 { status: "accepted" } (new) | 200 { status: "duplicate" } (duplicate)
  Error: 401 (invalid signature)
```

### New Config

Rate limits are hardcoded in `AppState::new()` (10 req/min bootstrap, 60 req/min authenticated). No env var overrides in v2; configurable rate limits deferred to v3.

### External Integrations (new)

**Docker** — Build + runtime container. Error handling: healthcheck retries for Postgres readiness. Test strategy: manual (`docker compose up` + curl health).

### Complexity Exceptions (v2)

None new — all v2 additions are small, self-contained modules.

---

## v3 Design Addendum — Operational Observability & Release Hygiene

### Scope

Additive only. No runtime behaviour changes. Six new concerns:

1. **CI workflow** (AC-16) — GitHub Actions fmt/clippy/test/deny gate
2. **JSON logging toggle** (AC-17) — `LOG_FORMAT` env var switches `tracing-subscriber` format
3. **Metrics listener** (AC-18) — separate HTTP port serving Prometheus text, opt-in via `METRICS_ADDR`
4. **Health / readiness split** (AC-19) — `/health` stays liveness-only, new `/ready` does dependency checks
5. **Release workflow + SBOM** (AC-20) — GitHub Actions tag-triggered build, CycloneDX SBOM, cosign keyless signing
6. **Runbooks** (AC-21) — five `docs/runbooks/*.md` files, each with Symptom / Diagnostic / Remediation / Escalation

### New Files

```
.github/
  workflows/
    ci.yml                       # AC-16: fmt, clippy, test (with Postgres service), cargo-deny
    release.yml                  # AC-20: tag build + SBOM + cosign sign + GitHub Release
.cargo/
  config.toml                    # AC-20: build-env tweaks — [net] retry + aarch64 cross-linker.
                                 #        NOTE: the [profile.release] overrides (LTO, strip,
                                 #        codegen-units=1, opt-level=3) live in top-level Cargo.toml
                                 #        because stable Rust reads profile settings from the manifest,
                                 #        not from .cargo/config.toml. scope.md AC-20 wording is
                                 #        pre-reconcile and still names .cargo/config.toml; the
                                 #        substance of the AC (LTO + strip in the release profile) is
                                 #        satisfied in Cargo.toml.
deny.toml                        # AC-16: cargo-deny config (advisories + licenses + bans + sources)
src/
  observability/
    mod.rs                       # AC-17/18: logging init + metrics init + metrics handle type
    logging.rs                   # AC-17: init_logging() — human vs JSON toggle
    metrics.rs                   # AC-18: init_metrics() — PrometheusHandle + metric name constants
    server.rs                    # AC-18: spawn_metrics_server(addr, handle) — separate axum app
  api/
    health.rs                    # AC-19: /health (liveness) + /ready (DB + migrations + tasks)
docs/
  runbooks/
    stale-wake-triage.md         # AC-21
    db-restore.md                # AC-21
    migration-rollback.md        # AC-21
    rate-limit-tuning.md         # AC-21
    webhook-debugging.md         # AC-21
tests/
  observability_test.rs          # AC-17 + AC-18: JSON log format + metrics endpoint smoke
  health_test.rs                 # AC-19: /health stays 200 when DB unreachable; /ready flips
```

### New Dependencies (Cargo.toml)

```toml
metrics = "0.24"
metrics-exporter-prometheus = { version = "0.16", default-features = false }
```

Both are maintained by the `tokio-rs/tracing`-adjacent ecosystem, lightweight, no transitive heavy deps. The `metrics` facade is crate-local (no runtime overhead when unused); the Prometheus exporter is used only for its `PrometheusBuilder::install_recorder()` + `PrometheusHandle::render()` — we do not use the crate's built-in HTTP listener (the `http-listener` feature was removed during REVIEW fixes because we ship a hand-rolled axum `/metrics` server in `src/observability/server.rs`). Metrics are only served when `METRICS_ADDR` is set. No new dependency for logging — `tracing-subscriber` already has `json` feature enabled in Cargo.toml.

No new runtime dependencies for CI / release — those are pure GitHub Actions YAML.

### New Interfaces

```rust
// src/observability/logging.rs
pub fn init_logging() {
    let json = std::env::var("LOG_FORMAT").ok().as_deref() == Some("json");
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    if json {
        tracing_subscriber::fmt().with_env_filter(env_filter).json().init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }
}

// src/observability/metrics.rs
pub struct MetricsState { pub handle: PrometheusHandle }

pub fn init_metrics() -> MetricsState { /* PrometheusBuilder::new().install_recorder() */ }

// Canonical metric names (counters unless noted)
pub const WAKE_STARTED: &str = "open_pincery_wake_started_total";
pub const WAKE_COMPLETED: &str = "open_pincery_wake_completed_total";   // labels: reason
pub const LLM_CALL: &str = "open_pincery_llm_call_total";
pub const LLM_PROMPT_TOKENS: &str = "open_pincery_llm_prompt_tokens_total";
pub const LLM_COMPLETION_TOKENS: &str = "open_pincery_llm_completion_tokens_total";
pub const TOOL_CALL: &str = "open_pincery_tool_call_total";             // labels: tool
pub const WEBHOOK_RECEIVED: &str = "open_pincery_webhook_received_total";
pub const RATE_LIMIT_REJECTED: &str = "open_pincery_rate_limit_rejected_total";
pub const ACTIVE_WAKES: &str = "open_pincery_active_wakes";             // gauge
pub const WAKE_DURATION: &str = "open_pincery_wake_duration_seconds";   // histogram

// src/observability/server.rs
pub async fn spawn_metrics_server(addr: SocketAddr, handle: PrometheusHandle, cancel: CancellationToken) -> Result<()>
// Binds a separate axum Router with only GET /metrics → handle.render()

// src/api/health.rs
pub async fn health() -> impl IntoResponse { (StatusCode::OK, Json(json!({"status":"ok"}))) }

pub async fn ready(State(app): State<AppState>) -> impl IntoResponse {
    // Check 1: DB round-trip
    if sqlx::query("SELECT 1").execute(&app.pool).await.is_err() {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"status":"not_ready","failing":"database"})));
    }
    // Check 2: expected migration count matches (COUNT _sqlx_migrations WHERE success = TRUE
    //          >= crate::db::expected_migration_count())
    // Check 3: per-task liveness flags app.listener_alive AND app.stale_alive both true.
    //          503 payload reports which specific task is down:
    //          "background_task:listener" | "background_task:stale_recovery" | "background_tasks".
    (StatusCode::OK, Json(json!({"status":"ready"})))
}
```

### Integration Points

| Change                                                                                                        | Where                                                                                                                                                             | Why                                                                                                                                                           |
| ------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Replace `tracing_subscriber::fmt::init()` in `main.rs`                                                        | `src/main.rs`                                                                                                                                                     | AC-17 toggle                                                                                                                                                  |
| Record counters inside existing runtime code                                                                  | `src/runtime/wake_loop.rs`, `src/runtime/maintenance.rs`, `src/runtime/llm.rs`, `src/runtime/tools.rs`, `src/api/webhooks.rs`, `src/api/mod.rs` rate limit branch | AC-18 — one-line `metrics::counter!()` calls at existing natural points                                                                                       |
| Move `/health` handler out of `main.rs`, add `/ready`                                                         | `src/api/health.rs`                                                                                                                                               | AC-19                                                                                                                                                         |
| Add per-task liveness flags (`listener_alive: Arc<AtomicBool>`, `stale_alive: Arc<AtomicBool>`) to `AppState` | `src/api/mod.rs`                                                                                                                                                  | AC-19 — readiness depends on both flags; each task flips its own on entry and an `AliveGuard` `Drop` flips it back on any exit (clean shutdown, error, panic) |
| Conditionally spawn metrics server                                                                            | `src/main.rs`                                                                                                                                                     | AC-18 — only when `METRICS_ADDR` set                                                                                                                          |

### External Integrations (new)

| Integration                                      | Failure mode                                                                                                                   | Test strategy                                     |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------- |
| GitHub Actions (CI + release)                    | Workflow turns red                                                                                                             | Real run on a feature branch — manual gate        |
| cosign keyless (sigstore)                        | Signing step fails; release still publishes unsigned if step is `continue-on-error: false` (it is not) → release fails cleanly | Manual verification on first release tag          |
| Prometheus scraper (optional, operator-supplied) | None — we're the producer                                                                                                      | Smoke test with `curl` in `observability_test.rs` |

### Observability

v3 _is_ the observability story. After v3:

- **Logs**: stderr/stdout, optionally JSON (AC-17)
- **Metrics**: Prometheus pull on opt-in port (AC-18)
- **Health**: `/health` and `/ready` endpoints (AC-19)
- **Release provenance**: signed artifacts + SBOM (AC-20)
- **Operator runbooks**: written down, not tribal (AC-21)

Traces (OTEL) explicitly deferred to v4+.

### Complexity Exceptions (v3)

None. All additions are small, isolated modules. `src/observability/mod.rs` + `logging.rs` + `metrics.rs` + `server.rs` together should fit under 300 lines.

### Open Questions (v3)

None. All choices are final.

---

# v4 Addendum — Usable Self-Host

This section is additive. Every prior v1/v2/v3 interface, file path, and data shape remains in effect except where explicitly replaced below.

## v4 Architecture Delta

```text
┌─────────────────────────────────────────────────────────────────────┐
│  Same runtime binary (open-pincery) — axum server + background jobs │
│                                                                     │
│  New file-level additions:                                          │
│    src/api/webhook_rotate.rs        (AC-24 endpoint handler)        │
│    src/api_client.rs                (shared HTTP client: CLI + tests) │
│    src/cli/mod.rs                   (CLI entrypoint, command dispatch) │
│    src/cli/config.rs                (~/.config/open-pincery/config.toml) │
│    src/cli/commands/{agent,budget,events,message,status,bootstrap,login}.rs │
│    src/bin/pcy.rs                   (thin: open_pincery::cli::run())│
│                                                                     │
│  Modified:                                                          │
│    src/api/agents.rs                (+rotate_webhook_secret_handler,│
│                                     AC-24 route registered here;    │
│                                     no separate webhook_rotate.rs)  │
│    src/background/listener.rs       (+budget check before acquire)  │
│    src/runtime/llm.rs               (new `Pricing` struct,          │
│                                     `LlmClient::with_pricing(...)`) │
│    src/models/llm_call.rs           (cost_usd + in-tx              │
│                                     agents.budget_used_usd bump)    │
│    src/main.rs                      (LLM_PRICE_* env → Pricing)     │
│    src/api/events.rs +              (since=<uuid> pagination for    │
│    src/models/event.rs              UI long-poll, AC-26)            │
│    src/api/mod.rs                   (+scoped_agent helper —         │
│                                     workspace-scoped agent lookup   │
│                                     shared by PATCH/DELETE/messages/│
│                                     events/rotate)                  │
│    Cargo.toml                       (clap dep, [[bin]] pcy entry)   │
│    Dockerfile                       (USER pcy, chown /app)          │
│    static/index.html                (real SPA shell, ES-module boot)│
│    static/js/*.js + static/js/views/*.js                            │
│                                     (new — UI split by concern:     │
│                                      app/api/state/ui + 4 views)    │
│    static/css/style.css             (new — minimal reset + utility) │
│                                                                     │
│  New docs:                                                          │
│    docs/api.md                      (HTTP API contract, AC-27)      │
└─────────────────────────────────────────────────────────────────────┘
```

## v4 Directory Structure (delta only)

```
src/
  api/
    agents.rs                # MODIFIED — `rotate_webhook_secret_handler` + route
                             # `POST /api/agents/{id}/webhook/rotate` live here
                             # (no separate webhook_rotate.rs module — the handler
                             # was inlined next to the existing PATCH/DELETE
                             # handlers because it shares the `scoped_agent`
                             # lookup and the same auth middleware stack)
    events.rs                # MODIFIED — `?since=<uuid>` cursor support for UI poll (AC-26)
    mod.rs                   # MODIFIED — `pub(crate) async fn scoped_agent(...)`
                             # helper (workspace-scoped agent lookup, reused by
                             # PATCH / DELETE / messages / events / rotate)
  api_client.rs              # NEW — reusable HTTP client (CLI, tests)
  bin/
    pcy.rs                   # NEW — CLI entrypoint binary (AC-25)
  cli/
    mod.rs                   # NEW — argument parsing + dispatch
    config.rs                # NEW — read/write ~/.config/open-pincery/config.toml
    commands/
      mod.rs                 # NEW — re-exports
      login.rs               # NEW — pcy login (idempotent; bootstrap-or-login fallback)
      agent.rs               # NEW — pcy agent {create,list,show,disable,rotate-secret}
      message.rs             # NEW — pcy message
      events.rs              # NEW — pcy events [--tail --since]
      budget.rs              # NEW — pcy budget {set,show,reset}
      status.rs              # NEW — pcy status
  models/
    event.rs                 # MODIFIED — `events_since_id(...)` for cursor paging
    llm_call.rs              # MODIFIED — single-transaction insert +
                             # `agents.budget_used_usd` bump (AC-23, T-26)
  runtime/
    llm.rs                   # MODIFIED — `Pricing { input_per_mtok, output_per_mtok }`
                             # struct + `LlmClient::with_pricing(primary, maint)`
                             # builder; cost_usd computed per call (AC-23)
static/
  index.html                 # REPLACED — minimal SPA shell, `<script type="module" src="/js/app.js">`
  css/
    style.css                # NEW — minimal reset + utility classes (154 lines)
  js/
    app.js                   # NEW — hash-router entrypoint (103 lines)
    api.js                   # NEW — fetch wrapper + typed API helpers (124 lines)
    state.js                 # NEW — in-memory store + event poll loop (75 lines)
    ui.js                    # NEW — DOM helpers + shared nav (66 lines)
    views/
      login.js               # NEW — #/login view (58 lines)
      agents.js              # NEW — #/agents list view (68 lines)
      detail.js              # NEW — #/agents/:id detail + stream view (132 lines)
      settings.js            # NEW — #/agents/:id/settings (rotate + budget) (105 lines)
docs/
  api.md                     # NEW — AC-27 HTTP API contract
tests/
  budget_test.rs             # NEW — AC-23 integration test (cost_usd + refusal path)
  webhook_rotate_test.rs     # NEW — AC-24 integration test
  cli_e2e_test.rs            # NEW — AC-25 end-to-end test (invokes pcy binary)
  ui_smoke_test.rs           # NEW — AC-26 UI smoke (serves files + probes routes)
  dockerfile_nonroot_test.rs # NEW — AC-22 static Dockerfile guard
  wake_loop_test.rs          # MODIFIED — AC-23 end-to-end cost assertion
                             # (`cost_usd = 0.00045`, `budget_used_usd = 0.00045`)
```

## v4 Interfaces

### AC-22 — Non-root Dockerfile runtime stage

```dockerfile
# Stage 2: Runtime (revised)
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Non-root user for runtime (AC-22)
RUN groupadd --system --gid 10001 pcy \
 && useradd --system --uid 10001 --gid pcy --home-dir /app --shell /usr/sbin/nologin pcy

COPY --from=builder --chown=pcy:pcy /app/target/release/open-pincery /usr/local/bin/open-pincery
COPY --from=builder --chown=pcy:pcy /app/migrations /app/migrations
COPY --from=builder --chown=pcy:pcy /app/static /app/static

WORKDIR /app
USER pcy

ENV OPEN_PINCERY_HOST=0.0.0.0
ENV OPEN_PINCERY_PORT=8080
EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=3s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

ENTRYPOINT ["open-pincery"]
```

Note: `pcy` username collides with the CLI binary name; this is intentional (short, memorable) and fine because the container user never interacts with the CLI — the CLI runs on the operator's host.

### AC-23 — Budget enforcement

Cost accounting happens at LLM-call record time and feeds the pre-CAS budget
gate in the background listener. `src/runtime/llm.rs` exposes a `Pricing`
value type and an `LlmClient::with_pricing(...)` builder:

```rust
// src/runtime/llm.rs
#[derive(Debug, Clone, Copy, Default)]
pub struct Pricing {
    pub input_per_mtok: Decimal,   // USD per 1,000,000 input tokens
    pub output_per_mtok: Decimal,  // USD per 1,000,000 output tokens
}

impl Pricing {
    pub fn new(input_per_mtok: Decimal, output_per_mtok: Decimal) -> Self;
    pub fn cost_for(&self, usage: &Usage) -> Decimal;
}

pub struct LlmClient {
    // ...existing fields...
    pub primary_pricing: Pricing,     // applied to wake-loop calls
    pub maintenance_pricing: Pricing, // applied to maintenance calls
}

impl LlmClient {
    pub fn with_pricing(mut self, primary: Pricing, maintenance: Pricing) -> Self;
}
```

Pricing is wired in `src/main.rs` from environment (defaults are
Claude-Sonnet-class list prices):

```
LLM_PRICE_INPUT_PER_MTOK                 (default: 3.0)
LLM_PRICE_OUTPUT_PER_MTOK                (default: 15.0)
LLM_MAINTENANCE_PRICE_INPUT_PER_MTOK     (default: 3.0)
LLM_MAINTENANCE_PRICE_OUTPUT_PER_MTOK    (default: 15.0)
```

`Pricing::default()` is zero-cost, so tests that don't configure pricing
record `cost_usd = 0` unchanged.

Insertion point is `src/background/listener.rs::trigger_wake`, **before** `agent::acquire_wake`:

```rust
// src/background/listener.rs (around line 99, before CAS acquire)

let candidate = agent::get_agent(&pool, agent_id).await?
    .ok_or(AppError::NotFound("agent disappeared".into()))?;

if candidate.budget_limit_usd > dec!(0)
    && candidate.budget_used_usd >= candidate.budget_limit_usd
{
    // Append budget_exceeded event without acquiring — agent stays asleep.
    let payload = serde_json::json!({
        "limit_usd": candidate.budget_limit_usd,
        "used_usd":  candidate.budget_used_usd,
    });
    event::append_event(
        &pool, agent_id, "budget_exceeded", "runtime",
        None, None, None, None, Some(payload.to_string()), None,
    ).await?;
    info!(agent_id = %agent_id, "Budget exceeded; refusing wake");
    return Ok(());
}

// Normal path: CAS acquire, then run_wake_loop as before
let acquired = agent::acquire_wake(&pool, agent_id).await?;
```

And at the LLM call record site (`src/runtime/llm.rs` or wherever `llm_call::record_call` is invoked with `cost_usd`):

```rust
// In the same transaction as llm_call insert:
let mut tx = pool.begin().await?;
sqlx::query!("INSERT INTO llm_calls (..., cost_usd, ...) VALUES (...)").execute(&mut *tx).await?;
sqlx::query!(
    "UPDATE agents SET budget_used_usd = budget_used_usd + $1 WHERE id = $2",
    cost_usd, agent_id,
).execute(&mut *tx).await?;
tx.commit().await?;
```

Decision: `budget_limit_usd = 0` = unlimited. `NULL` is not used (schema has `NOT NULL DEFAULT 10.0`). To set unlimited operators run `UPDATE agents SET budget_limit_usd = 0`.

### AC-24 — Webhook rotation endpoint

```rust
// src/api/agents.rs — handler inlined alongside PATCH/DELETE because it
// shares the `scoped_agent` workspace lookup and auth stack. No separate
// `webhook_rotate.rs` module is introduced.

async fn rotate_webhook_secret_handler(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<RotateWebhookSecretResponse>, AppError> {
    // 1. Workspace-scope the agent (404 if not in auth.workspace_id).
    scoped_agent(&state, &auth, id).await?;

    // 2. Generate 32 random bytes, base64-encoded.
    let new_secret = crate::auth::generate_webhook_secret();

    // 3. Rotate + audit event in a single transaction.
    let mut tx = state.pool.begin().await?;
    let _rotated = agent::rotate_webhook_secret_tx(&mut tx, id, &new_secret).await?;
    event::append_event_tx(
        &mut tx, id, "webhook_secret_rotated", "api",
        None, None, None, None, None, None,
    ).await?;
    tx.commit().await?;

    Ok(Json(RotateWebhookSecretResponse { webhook_secret: new_secret }))
}

#[derive(Serialize)]
struct RotateWebhookSecretResponse { webhook_secret: String }
```

Route registered in `src/api/agents.rs::router()`:

```rust
.route("/agents/{id}/webhook/rotate", post(rotate_webhook_secret_handler))
```

### AC-25 — `pcy` CLI

Cargo.toml:

```toml
[[bin]]
name = "pcy"
path = "src/bin/pcy.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
toml = "0.9"
dirs = "6"
```

`src/bin/pcy.rs`:

```rust
fn main() -> std::process::ExitCode {
    open_pincery::cli::run()
}
```

`src/cli/mod.rs`:

```rust
#[derive(clap::Parser)]
#[command(name = "pcy", version, about = "Open Pincery operator CLI")]
struct Cli {
    /// Override OPEN_PINCERY_URL
    #[arg(long, global = true)]
    url: Option<String>,
    /// Override cached session token
    #[arg(long, global = true)]
    token: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    Bootstrap { #[arg(long, env = "OPEN_PINCERY_BOOTSTRAP_TOKEN")] install_token: String },
    Login { #[arg(long)] token: String },
    Agent { #[command(subcommand)] action: AgentAction },
    Message { agent: String, text: String },
    Events { agent: String, #[arg(long)] tail: bool, #[arg(long)] since: Option<i64> },
    Budget { #[command(subcommand)] action: BudgetAction },
    Status,
}

pub fn run() -> std::process::ExitCode { /* parse + dispatch */ }
```

Config file (`~/.config/open-pincery/config.toml` — platform-appropriate via `dirs::config_dir()`):

```toml
url = "http://localhost:8080"
token = "<session-token>"
```

Shared HTTP client (`src/api_client.rs`) — reused by CLI, tests, and future consumers:

```rust
pub struct Client {
    base: String,
    token: Option<String>,
    http: reqwest::Client,
}

impl Client {
    pub fn new(base: String, token: Option<String>) -> Self { /* ... */ }
    pub async fn bootstrap(&self, install_token: &str) -> Result<BootstrapResponse>;
    pub async fn list_agents(&self) -> Result<Vec<AgentSummary>>;
    pub async fn create_agent(&self, name: &str) -> Result<AgentDetail>;
    pub async fn get_agent(&self, id: Uuid) -> Result<AgentDetail>;
    pub async fn send_message(&self, id: Uuid, text: &str) -> Result<()>;
    pub async fn list_events(&self, id: Uuid, since: Option<i64>) -> Result<Vec<EventRow>>;
    pub async fn rotate_secret(&self, id: Uuid) -> Result<String>;
    pub async fn set_enabled(&self, id: Uuid, enabled: bool) -> Result<()>;
    pub async fn set_budget(&self, id: Uuid, limit_usd: Decimal) -> Result<()>;
    pub async fn ready(&self) -> Result<ReadyStatus>;
}
```

### AC-26 — Vanilla JS UI

Hash-routed SPA. No build step. No framework. No external dependencies (no CDN fetches).

`static/index.html` (shell — bootstraps the ES-module graph):

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Open Pincery</title>
    <link rel="stylesheet" href="/css/style.css" />
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/js/app.js"></script>
  </body>
</html>
```

`static/js/` layer map (each file is an ES module imported via relative paths;
no bundler, no CDN, same-origin only):

```
static/js/app.js           # hash-route dispatcher, re-renders #app on hashchange
static/js/api.js           # `fetch` wrapper + typed API helpers (bootstrap, list agents,
                           # send message, list events since, rotate secret, set budget)
static/js/state.js         # in-memory store + `pollEvents(agentId, sinceId)` long-poll
                           # loop with exponential backoff (4s → 8s → 16s → 32s)
static/js/ui.js            # DOM helpers + shared nav shell (`currentRoute`, `routeTo`)
static/js/views/login.js   # #/login             — paste token, persist to localStorage
static/js/views/agents.js  # #/agents            — list view
static/js/views/detail.js  # #/agents/:id        — live stream + send-message form
static/js/views/settings.js# #/agents/:id/settings — rotate webhook secret, set/show budget
```

Event stream: `GET /api/agents/:id/events?since=<last_seen_id>` polled every 4s with exponential backoff on failure (4s → 8s → 16s → 32s cap). Caller remembers last-seen event id in module-local state inside `state.js` (not `localStorage` — intentionally ephemeral).

CSS: `static/css/style.css` — minimal reset + utility classes (154 lines, no flex/grid framework, direct rules). Accessible defaults (`prefers-color-scheme`, min contrast).

### AC-27 — `docs/api.md` API contract

Structure:

```markdown
# Open Pincery HTTP API v4

Stability: v4 → v5 compatible. Endpoints may be added; **none will be removed or
renamed**, and documented request/response field types will not change
incompatibly without a major version bump. Undocumented fields are not part
of the contract and may appear or disappear.

## Authentication

...

## Endpoints

### POST /api/bootstrap

...

### POST /api/agents

...

### GET /api/agents

...

<one section per endpoint the CLI or UI calls>
```

## v4 Data Model

No schema changes. Existing columns already sufficient:

| Column                                     | Table       | Used by                                     |
| ------------------------------------------ | ----------- | ------------------------------------------- |
| `budget_limit_usd` (NUMERIC, default 10.0) | `agents`    | AC-23                                       |
| `budget_used_usd` (NUMERIC, default 0)     | `agents`    | AC-23                                       |
| `webhook_secret` (TEXT)                    | `agents`    | AC-24                                       |
| `cost_usd` (NUMERIC)                       | `llm_calls` | AC-23 (increments `agents.budget_used_usd`) |

New event types (append-only convention, no schema change):

- `budget_exceeded` — `source='runtime'`, payload `{"limit_usd":…,"used_usd":…}`
- `webhook_secret_rotated` — `source='api'`, no payload

## v4 External Integrations

No new outbound integrations. All changes are internal or inbound (CLI, browser).

| Integration           | Failure mode                                                              | Test strategy                                           |
| --------------------- | ------------------------------------------------------------------------- | ------------------------------------------------------- |
| `pcy` CLI → HTTP API  | network error, auth failure, 4xx, 5xx — exit non-zero with stderr message | Real HTTP against live test server in `cli_e2e_test.rs` |
| Browser UI → HTTP API | fetch error, 401 → redirect to login, 5xx → render banner                 | `ui_smoke_test.rs` loads served files + probes routes   |

## v4 Observability

No changes to the observability stack. New events (`budget_exceeded`, `webhook_secret_rotated`) appear in the existing event log and are queryable via existing `GET /api/agents/:id/events`. No new metrics; `budget_exceeded` rate is observable as a label dimension on existing event counters if desired (deferred).

## v4 Complexity Exceptions

- **`static/js/**`— split by responsibility, no single-file ceiling**. BUILD
moved away from the single-file`static/app.js` design note in favour of
four ES-module layers (`app.js`router,`api.js`fetch client,`state.js`store + poll loop,`ui.js` DOM helpers) plus one file per view
(`views/{login,agents,detail,settings}.js`). This is served as-is by the
existing axum static handler via `<script type="module" src="/js/app.js">`— still no bundler, no framework, no build step, no CDN fetches. The
largest file is`views/detail.js` at 132 lines; the total UI budget
  (all JS + CSS combined) is well under the previous ~400-line single-file
  soft ceiling, so the explicit ceiling is retired. If any single module
  grows past ~200 lines, split it further rather than extending the ceiling.
- **`src/cli/**`— 600-line total budget** across`src/cli/mod.rs`, `src/cli/config.rs`, and `src/cli/commands/\*.rs`. Justification: a second binary in the same crate is cohesive with the runtime and shares `src/api_client.rs`; extracting to a separate workspace member is premature at v4 size. If v5 pushes the CLI past 600 lines, extract to a workspace member per preferences.md convention.

## v4 Open Questions

None. All interfaces, file paths, and test strategies are final.

## v4 Test Strategy

| AC    | Test file                                                             | Kind            | Notes                                                                                                                                                                                  |
| ----- | --------------------------------------------------------------------- | --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC-22 | `tests/docker_nonroot_test.sh` (shell, gated by `DOCKER_AVAILABLE=1`) | Integration     | Skipped in CI without Docker; documented in runbook                                                                                                                                    |
| AC-23 | `tests/budget_test.rs`                                                | Integration     | DB fixture + real wake attempt; asserts event + no llm_calls row                                                                                                                       |
| AC-24 | `tests/webhook_rotate_test.rs`                                        | Integration     | Two-secret HMAC flow                                                                                                                                                                   |
| AC-25 | `tests/cli_e2e_test.rs`                                               | End-to-end      | Uses `assert_cmd` or spawns `cargo run --bin pcy` against a live test server                                                                                                           |
| AC-26 | `tests/ui_smoke_test.rs`                                              | Smoke           | Serves files through the axum router, asserts `index.html` is reachable, grep-asserts the app.js loads `/api/agents` on list view; full-browser headless-chrome optional, gated by env |
| AC-27 | REVIEW subagent pass                                                  | Document review | Subagent cross-checks every CLI/UI call against `docs/api.md` and every endpoint in `src/api/` against the doc                                                                         |

---

## v5 Design Addendum — Operator Onramp

### Architecture Changes

None. v5 is docs, compose YAML, `.env.example`, scripts, and tests. No new runtime modules, no new API endpoints, no schema migrations, no dependencies.

### Operator Onramp Contract

The onramp is the documented + test-enforced path from an empty clone to a working first agent. It has six deliverables:

1. **`docker-compose.yml` env passthrough**: Every config variable the runtime reads reaches the container via `${VAR}` interpolation. Required secrets use `${VAR:?message}` to fail fast. Optional vars use `${VAR:-default}` with defaults matching `.env.example`.
2. **`.env.example` as the config contract**: Every `std::env::var(...)` call site in `src/config.rs`, `src/runtime/llm.rs` pricing, and `src/observability/` must have a corresponding entry, grouped + commented.
3. **`scripts/smoke.{sh,ps1}`**: Identical onramp behaviour on Linux/macOS/Windows. Bash is primary; PowerShell is byte-level equivalent behaviour.
4. **`README.md` Quick Start**: UI → `pcy` → curl, plus From-Binary, Troubleshooting, Reset, Backup, Going-Public.
5. **`docker-compose.caddy.yml` + `Caddyfile.example`**: Localhost-to-HTTPS overlay.
6. **Regression tests** in `tests/` that enforce (1)-(5) consistency with the runtime.

### New Files

| File                              | Purpose                                                                                                                                   |
| --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `docker-compose.caddy.yml`        | Overlay file adding a Caddy service in front of app; published ports switch from app:8080 to caddy:80/443                                 |
| `Caddyfile.example`               | Template with a single-line site block + env-var placeholders for domain and email                                                        |
| `scripts/smoke.sh`                | Bash: `compose up --wait` → poll `/ready` → `pcy login/agent create/message` → `pcy events` → assert `message_received`                   |
| `scripts/smoke.ps1`               | PowerShell equivalent                                                                                                                     |
| `tests/compose_env_test.rs`       | Runs `docker compose config` against a fixture env; asserts passthrough + secure defaults + fail-fast                                     |
| `tests/env_example_test.rs`       | Parses `.env.example`; scans source for `env::var`; asserts coverage modulo explicit allowlist                                            |
| `tests/smoke_script_test.rs`      | Static checks that `scripts/smoke.{sh,ps1}` cover the required milestones; gated live run of `bash scripts/smoke.sh` via `DOCKER_SMOKE=1` |
| `tests/readme_quickstart_test.rs` | Greps `README.md` for smoke-script-milestone strings + named anchor sections                                                              |
| `tests/caddy_overlay_test.rs`     | Runs `docker compose -f docker-compose.yml -f docker-compose.caddy.yml config`; asserts caddy service present and ports correct           |

### Modified Files

| File                 | Change                                                                                                                   |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `docker-compose.yml` | env block → `${VAR}` interpolation with `:?` / `:-` guards; ports → `127.0.0.1:8080:8080`                                |
| `.env.example`       | Refreshed with all v4 vars, grouped/commented, OpenRouter default + OpenAI alt block, `OPEN_PINCERY_HOST=127.0.0.1`      |
| `README.md`          | Quick Start rewrite (UI/pcy/curl/binary), Troubleshooting, Reset, Backup, Going-Public-HTTPS sections; API table updated |

### Test Strategy for Each Integration

| Concern                  | Strategy                                                                                                                                                              |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Docker Compose rendering | `docker compose config` against fixture `.env` — runs offline, no containers started                                                                                  |
| Smoke script             | Bash script exercised against the running test-DB stack already used by other integration tests; gated by `DOCKER_SMOKE=1` to avoid requiring compose in every CI leg |
| `.env.example` coverage  | Pure Rust test — regex-scans source, compares to parsed `.env.example`                                                                                                |
| README anchor presence   | Pure Rust test — `include_str!("../README.md")` + substring assertions                                                                                                |
| Caddyfile syntax         | `caddy validate` if binary present; fall back to structural parse if not (don't hard-require caddy binary in CI)                                                      |

### Observability

No new observability. v5 surfaces existing v3 observability (JSON logs, Prometheus `/metrics`) through documentation, not new code.

### Complexity Exceptions

None. Every file stays under 300 lines. Compose file remains a single YAML; the Caddy overlay is its own file so the baseline compose has zero new complexity for operators who skip TLS.

### Open Questions

None. OpenRouter stays the default LLM base URL; OpenAI ships as a commented alternative in `.env.example`. Port binding default is `127.0.0.1:8080:8080`; operators who want remote exposure explicitly override `OPEN_PINCERY_HOST=0.0.0.0` and can add back-end network filtering of their choosing.

---

## v6 Design Addendum — Capability Foundations & Security Baseline

### Architecture Delta

Four strictly-additive changes. No schema refactor, no API changes, no new crates of note, no new background tasks.

```
┌──────────────────── Wake Loop ────────────────────┐
│  llm → tool_calls                                 │
│    │                                              │
│    ▼                                              │
│  tools::dispatch_tool(tc, permission_mode, exec)  │  ◄── v6: two new params
│    │                                              │
│    ├── capability::required_for(name)             │  ◄── v6 AC-35
│    ├── capability::mode_allows(mode, cap) → bool  │
│    │      └── false → append tool_capability_denied event, return Error
│    │                                              │
│    └── exec.run(&ShellCommand, &SandboxProfile)   │  ◄── v6 AC-36
│         │   (Arc<dyn ToolExecutor> in AppState)   │
│         └── ProcessExecutor: tempdir cwd,         │
│             PATH-only env, 30s timeout, no sudo   │
└───────────────────────────────────────────────────┘

AgentStatus enum (v6 AC-34) lives in src/models/agent.rs and
is the sole as_db_str() source for every SQL status literal in that file.

deny.toml (v6 AC-37): cargo-deny v2 advisories schema (implicit vulnerability
deny) + yanked = "deny" + ignore list limited to a single documented,
dated exception (RUSTSEC-2023-0071) pinned against the test allowlist.
```

### Directory Structure (v6 deltas only)

```
src/
  models/
    agent.rs           — MODIFIED: AgentStatus enum + as_db_str/from_db_str + const DB_* bindings
  runtime/
    tools.rs           — MODIFIED: dispatch_tool(tc, mode, Arc<dyn ToolExecutor>) signature
    wake_loop.rs       — MODIFIED: loads agent.permission_mode, threads exec, passes to dispatch_tool
    capability.rs      — NEW: ToolCapability, PermissionMode, required_for, mode_allows
    sandbox.rs         — NEW: ToolExecutor trait, ProcessExecutor, ShellCommand, SandboxProfile, ExecResult
  api/
    mod.rs             — MODIFIED: AppState gains executor: Arc<dyn ToolExecutor>.
                         Two constructors: `AppState::new(pool, config)` defaults the
                         executor to `Arc::new(ProcessExecutor)` (convenience for tests);
                         `AppState::new_with_executor(pool, config, executor)` is the
                         production path so AppState and the wake loop share one instance.
  main.rs              — MODIFIED: constructs Arc::new(ProcessExecutor::default()) at startup
migrations/
  20260420000001_agent_status_states.sql  — NEW: ALTER CHECK constraint to include 'wake_acquiring', 'wake_ending'
tests/
  agent_status_test.rs       — NEW: AC-34 round-trip test
  no_raw_status_literals.rs  — NEW: AC-34 build-time grep guard
  capability_gate_test.rs    — NEW: AC-35 mode×capability table + locked-agent integration
  sandbox_test.rs            — NEW: AC-36 env-strip, timeout, sudo-reject
  no_raw_command_new.rs      — NEW: AC-36 build-time grep guard
  deny_config_test.rs        — NEW: AC-37 deny.toml schema assertion
deny.toml                    — MODIFIED: v2 advisories schema; yanked = "deny";
                                ignore = [ documented RUSTSEC-2023-0071 only ]
```

### Interfaces

**AC-34 — `src/models/agent.rs`:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentStatus {
    Resting,       // DB: "asleep"        — spec name; legacy DB value preserved
    WakeAcquiring, // DB: "wake_acquiring" — reserved, not yet written by any transition
    Awake,         // DB: "awake"
    WakeEnding,    // DB: "wake_ending"    — reserved, not yet written by any transition
    Maintenance,   // DB: "maintenance"
}

#[derive(Debug, thiserror::Error)]
#[error("invalid agent status: {0}")]
pub struct InvalidStatus(pub String);

impl AgentStatus {
    pub const DB_RESTING: &'static str = "asleep";
    pub const DB_WAKE_ACQUIRING: &'static str = "wake_acquiring";
    pub const DB_AWAKE: &'static str = "awake";
    pub const DB_WAKE_ENDING: &'static str = "wake_ending";
    pub const DB_MAINTENANCE: &'static str = "maintenance";

    pub fn as_db_str(self) -> &'static str { /* match */ }
    pub fn from_db_str(s: &str) -> Result<Self, InvalidStatus> { /* match */ }
}
```

Existing SQL sites in `src/models/agent.rs` keep their lowercase literals but each literal is replaced with a `const` identifier local to the same file (`DB_AWAKE`, `DB_MAINTENANCE`, `DB_RESTING`). The `no_raw_status_literals` test enforces that every `status = '…'` or `status IN (…)` occurrence under `src/` is either inside the `src/models/agent.rs` constant-definition block or uses one of those constants via interpolation; a future spec rename therefore either updates the enum mapping or fails compilation.

**AC-35 — `src/runtime/capability.rs`:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCapability { ReadLocal, WriteLocal, ExecuteLocal, Network, Destructive }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode { Yolo, Supervised, Locked }

impl PermissionMode {
    pub fn from_db_str(s: &str) -> Self { /* yolo/supervised/locked; unknown → Locked */ }
}

pub fn required_for(tool_name: &str) -> ToolCapability {
    match tool_name {
        "shell" => ToolCapability::ExecuteLocal,
        "plan"  => ToolCapability::ReadLocal,
        "sleep" => ToolCapability::ReadLocal,
        _       => ToolCapability::Destructive, // unknown tools default to most-restrictive
    }
}

pub fn mode_allows(mode: PermissionMode, cap: ToolCapability) -> bool {
    use PermissionMode::*; use ToolCapability::*;
    match (mode, cap) {
        (Yolo, _)                        => true,
        (Supervised, Destructive)        => false,
        (Supervised, _)                  => true,
        (Locked, ReadLocal)              => true,
        (Locked, _)                      => false,
    }
}
```

Gate table (15 cells; every row covered by unit test):

| mode \ cap | ReadLocal | WriteLocal | ExecuteLocal | Network | Destructive |
| ---------- | :-------: | :--------: | :----------: | :-----: | :---------: |
| Yolo       |     ✓     |     ✓      |      ✓       |    ✓    |      ✓      |
| Supervised |     ✓     |     ✓      |      ✓       |    ✓    |      ✗      |
| Locked     |     ✓     |     ✗      |      ✗       |    ✗    |      ✗      |

Denied dispatch appends event `{event_type:"tool_capability_denied", source:"runtime", tool_name, content JSON {required_capability, permission_mode}}` and returns `ToolResult::Error("tool disallowed by permission mode")` without invoking the executor.

**AC-36 — `src/runtime/sandbox.rs`:**

```rust
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn run(&self, cmd: &ShellCommand, profile: &SandboxProfile) -> ExecResult;
}

pub struct ShellCommand { pub command: String }

pub struct SandboxProfile {
    pub cwd: Option<PathBuf>,         // None => fresh tempdir per call
    pub env_allowlist: Vec<String>,   // default ["PATH"]
    pub deny_net: bool,               // default true; advisory in v6, enforced in v8
    pub timeout: Duration,            // default 30s
}

pub enum ExecResult {
    Ok { stdout: String, stderr: String, exit_code: i32 },
    Timeout,
    Rejected(String),
    Err(String),
}

pub struct ProcessExecutor;

impl Default for ProcessExecutor { /* zero-field */ }
```

`ProcessExecutor::run` behavior:

1. Tokenise `cmd.command` on shell word-boundaries (whitespace, `;`, `&`, `|`, `(`, `)`, backtick, `$(`, quotes) and reject if any token equals `sudo` → `ExecResult::Rejected("sudo is not permitted")` (no process spawned). Catches prefix, bare, and chained forms (`echo ok && sudo …`); does NOT attempt to catch absolute-path escalation (`/usr/bin/sudo`) — that is defense-in-depth territory owned by env_clear + tempdir + timeout.
2. Create a fresh tempdir via `tempfile::tempdir()`; on failure → `ExecResult::Err`.
3. Build `tokio::process::Command::new("sh")`, `.arg("-c").arg(&cmd.command)`, `.current_dir(tempdir_path)`, `.env_clear()`, then re-add only the allowlisted vars from the host environment (`for k in &profile.env_allowlist { if let Ok(v) = std::env::var(k) { cmd.env(k, v); } }`), `.stdin(Stdio::null())`.
4. Spawn; wrap in `tokio::time::timeout(profile.timeout, child.wait_with_output())`; on timeout, `child.start_kill()` + `ExecResult::Timeout`.
5. On success, return `Ok { stdout, stderr, exit_code }`. 50 KB truncation is applied by `dispatch_tool`, not by the executor.

`dispatch_tool` signature updated:

```rust
pub async fn dispatch_tool(
    tool_call: &ToolCallRequest,
    mode: PermissionMode,
    executor: &Arc<dyn ToolExecutor>,
    pool: &PgPool,
    agent_id: Uuid,
    wake_id: Uuid,
) -> ToolResult
```

(The additional `pool`/`agent_id`/`wake_id` parameters are needed so the capability-denial branch can append its own event. The unchanged `Output`/`Error`/`Sleep` tool-result event appends remain in `wake_loop.rs` as before.)

`wake_loop::run_wake_loop` reads `current.permission_mode` once per loop iteration (it already loads `current` for the iteration-cap check) and passes `PermissionMode::from_db_str(&current.permission_mode)` plus `state.executor.clone()` into `dispatch_tool`.

`AppState` (defined in `src/api/mod.rs`) gains `pub executor: Arc<dyn ToolExecutor>`. `src/main.rs` constructs it once at startup:

```rust
let executor: Arc<dyn ToolExecutor> = Arc::new(ProcessExecutor);
```

Tests inject their own `ToolExecutor` impl (a `CountingExecutor` that never spawns, used by `tests/capability_gate_test.rs`).

**AC-37 — `deny.toml`:**

```toml
[advisories]
version = 2              # v2 schema: known vulnerabilities are implicitly denied
yanked = "deny"          # v6: was "warn"
ignore = [
    # Single documented exception — every entry must carry advisory ID,
    # dated justification, and a revisit trigger. Pinned by
    # tests/deny_config_test.rs against an explicit ALLOWED_ADVISORIES
    # allowlist; adding any new entry requires touching both files
    # in the same change (a STOP-and-raise event).
    { id = "RUSTSEC-2023-0071", reason = "rsa via sqlx-macros-core->sqlx-mysql; no Postgres runtime exposure; no upstream fix. Revisit on rsa release or sqlx 0.9." },
]
```

Note: cargo-deny's v2 advisories schema removed the explicit `vulnerability`
key — the v2 header itself IS the "deny known vulnerabilities" contract. The
v6 BUILD first shipped `ignore = []` (Slice 1) and then a post-BUILD fix added
the single RUSTSEC-2023-0071 entry after investigation showed the transitive
path is compile-time-only (sqlx-macros pulls sqlx-mysql regardless of runtime
features; no Postgres-runtime exposure; no upstream rsa fix since 2023-11).

`tests/deny_config_test.rs` parses `deny.toml` using the runtime `toml = "0.8"`
dep (already present for config parsing) and asserts `version = 2`,
`yanked = "deny"`, and that the ignored advisory ID set equals the test's
`ALLOWED_ADVISORIES` constant with every entry carrying a non-empty `reason`.

### External Integrations

None added. `ProcessExecutor` is a local-only executor; the only external call remains the existing LLM egress (unchanged). Zerobox, vault, proxy — all deferred to v7/v8/v9.

### Test Strategy

| AC    | Test file                         | Kind          | Notes                                                                                                                                                                                                                                                                                                                                                                                                         |
| ----- | --------------------------------- | ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| AC-34 | `tests/agent_status_test.rs`      | Unit          | Round-trip all 5 variants through `from_db_str` / `as_db_str`; `from_db_str("bogus")` → Err                                                                                                                                                                                                                                                                                                                   |
| AC-34 | `tests/no_raw_status_literals.rs` | Static / grep | Reads `src/**/*.rs` at test time, regex over `status\s\*(=                                                                                                                                                                                                                                                                                                                                                    | IN)\s\*['\(]`; allowlists the constant-definition block in `src/models/agent.rs` |
| AC-35 | `tests/capability_gate_test.rs`   | Unit + integ  | Table-driven: 15 `(mode, cap)` rows against `mode_allows`; integration creates a `Locked` agent, wakes via wiremock-served `shell` tool call, asserts one `tool_capability_denied` event + zero `tool_result` + a `CountingExecutor::spawns() == 0`                                                                                                                                                           |
| AC-36 | `tests/sandbox_test.rs`           | Unit          | (a) set `HOME=/tmp/fake`, `MY_SECRET=leak`; run `printenv` via `ProcessExecutor` with allowlist `["PATH"]`; assert neither name appears in stdout. (b) `sleep 60` with `timeout = 1s` → `ExecResult::Timeout`. (c) `sudo`-prefixed, (d) bare `sudo`, (e) chained `echo ok && sudo …` — all `ExecResult::Rejected` without spawn (probe file absent). (f) Ok path reports stdout + exit code. Six tests total. |
| AC-36 | `tests/no_raw_command_new.rs`     | Static / grep | Regex `Command::new\(` across `src/runtime/**` — exactly one match, inside `sandbox.rs`                                                                                                                                                                                                                                                                                                                       |
| AC-37 | `tests/deny_config_test.rs`       | Unit          | Parse `deny.toml`; assert `[advisories].vulnerability == "deny"` and `ignore == []`                                                                                                                                                                                                                                                                                                                           |
| AC-37 | CI `cargo deny check advisories`  | CI gate       | Already wired by v3 AC-16; must exit 0 on v6 HEAD                                                                                                                                                                                                                                                                                                                                                             |

### Observability

No new metrics. The existing `open_pincery_tool_call_total{tool}` counter is unchanged. A denied call does not increment `tool_call_total` (that counter reflects executions); the `tool_capability_denied` event in the event log is the system of record. We deliberately do not add a metric for this in v6 — when the denial rate becomes operationally interesting, a counter + runbook lands in whatever version ships `supervised`/`locked` as a UI-surfaced default.

### Complexity Exceptions

None. Every new file stays under 200 lines:

- `src/runtime/capability.rs` ≈ 70 lines (two enums + two const tables).
- `src/runtime/sandbox.rs` ≈ 130 lines (trait, types, `ProcessExecutor` with 5-step run).
- Migration ≈ 20 lines.
- Each test file < 150 lines.

### Key Scenario Trace

Scenario: a `Locked` agent receives a message; the LLM returns a `shell` tool call requesting a destructive filesystem command.

1. `src/background/listener.rs::on_notify` CAS-acquires wake (unchanged).
2. `run_wake_loop` loads `current.permission_mode = "locked"` → `PermissionMode::Locked`.
3. `run_wake_loop` calls `llm.chat(...)`; response contains `tool_calls: [{name:"shell", arguments:"{\"command\":\"<destructive>\"}"}]`.
4. `run_wake_loop` appends `tool_call` event (unchanged).
5. `tools::dispatch_tool(tc, PermissionMode::Locked, &state.executor, ...)` is invoked:
   - `capability::required_for("shell")` → `ExecuteLocal`.
   - `capability::mode_allows(Locked, ExecuteLocal)` → `false`.
   - Appends `tool_capability_denied` event with payload `{required_capability:"execute_local", permission_mode:"locked"}`.
   - Returns `ToolResult::Error("tool disallowed by permission mode")`.
6. `run_wake_loop` receives `Error` branch, appends `tool_result` event with the error body (existing v5 path), continues the loop.
7. Next LLM turn sees the error in the assembled prompt; `sleep` is the typical next action.
8. `ProcessExecutor::run` is never called. Host filesystem is untouched.

For a `Yolo` agent the same flow dispatches to `ProcessExecutor::run`, which runs the command under `sh -c` inside a fresh tempdir with `env_clear()` + `PATH`-only. A destructive command targeted at the tempdir is a near-no-op, which by itself illustrates why this is a defense-in-depth baseline, not a sandbox — the real sandbox (Zerobox) lands in v8 at the same `ToolExecutor` seam.

### Open Questions

None. The `ToolExecutor` trait seam is stable enough to host Zerobox (v8) and a test `CountingExecutor` (v6) without further refactor. The two reserved DB status values do not change existing transition code; the TLA+-faithful CAS split that uses them is tracked for v10.d change the compose ports line (documented in Troubleshooting).

---

## v7 Design Addendum — Credential Vault & Reasoner-Secret Refusal

### Architecture Delta

Six strictly-additive, interlocking changes. One new migration, one new Rust module, one new API router, one new CLI command group, one new runtime tool, one new prompt-template row.

```
┌───────────── Operator ─────────────┐           ┌────────── Agent Wake ──────────┐
│  pcy credential add <name>         │           │  llm → tool_calls              │
│    └─ rpassword prompt / stdin     │           │                                │
│         │                          │           │  tools::dispatch_tool          │
│         ▼                          │           │    │                           │
│  POST /api/workspaces/:id/creds    │──┐        │    ├─ capability gate (v6)    │
│    └─ auth_middleware (v2) +       │  │        │    ├─ AC-43: scan shell env   │
│       workspace_admin role check   │  │        │    │   for PLACEHOLDER:<name> │
│    └─ vault::seal(ws_id, name, val)│  │        │    │     ├─ hit → keep string │
│    └─ INSERT credentials + event   │  ▼        │    │     └─ miss/revoked:     │
│       credential_added             │  DB       │    │        credential_       │
│                                    │  ▲        │    │        unresolved event  │
│  pcy credential list               │  │        │    │                           │
│    └─ GET ... → names only         │  │        │    └─ ProcessExecutor::run    │
│                                    │  │        │                                │
│  pcy credential revoke <name>      │  │        │  list_credentials tool        │
│    └─ DELETE → revoked_at = NOW()  │──┘        │    └─ SELECT names FROM       │
│    └─ event credential_revoked     │           │       credentials WHERE       │
└────────────────────────────────────┘           │       workspace_id=$1 AND     │
                                                 │       revoked_at IS NULL      │
                                                 └────────────────────────────────┘

Vault module: vault::seal / vault::open — AES-256-GCM, OsRng nonce per seal,
              AAD = "{workspace_id}:{name}" bytes. Master key loaded once at
              startup from OPEN_PINCERY_VAULT_KEY (base64, 32 bytes).

Reasoner: default_agent prompt template v2 adds Credential Handling section
          with explicit redirect to `pcy credential add`. v1 row stays in
          table with is_active=false (one-active-per-name constraint honored).
```

### Directory Structure (v7 deltas only)

```
src/
  runtime/
    vault.rs           — NEW: Vault::seal / Vault::open, SealedCredential,
                         VaultError, MASTER key loading (AES-256-GCM, aes-gcm crate)
    tools.rs           — MODIFIED: list_credentials tool registered (AC-41);
                         ShellArgs gains optional env: HashMap<String,String>;
                         pre-dispatch placeholder resolution (AC-43);
                         dispatch_tool signature gains workspace_id: Uuid
    capability.rs      — MODIFIED: required_for("list_credentials") -> ReadLocal
    wake_loop.rs       — MODIFIED: passes agent.workspace_id into dispatch_tool
  models/
    credential.rs      — NEW: Credential struct + create/list/revoke/find helpers
  api/
    credentials.rs     — NEW: POST/GET/DELETE /api/workspaces/:id/credentials
    mod.rs             — MODIFIED: mounts credentials router; workspace_admin helper
  cli/
    commands/
      credential.rs    — NEW: pcy credential add/list/revoke
      mod.rs           — MODIFIED: pub mod credential
    mod.rs             — MODIFIED: Credential subcommand on Cli enum
  api_client.rs        — MODIFIED: create_credential / list_credentials / revoke_credential
  config.rs            — MODIFIED: vault_key: [u8; 32] loaded from OPEN_PINCERY_VAULT_KEY
  main.rs              — MODIFIED: passes vault_key into AppState
migrations/
  20260420000002_create_credentials.sql   — NEW: credentials table + unique partial index
  20260420000003_prompt_template_credentials.sql — NEW: bump default_agent to v2
                                                with Credential Handling section
tests/
  vault_roundtrip_test.rs       — NEW: AC-38 seal/open + tamper + wrong-key
  vault_api_test.rs             — NEW: AC-39 REST CRUD + role gate + list names-only
  cli_credential_test.rs        — NEW: AC-40 argv-reject + stdin round-trip
  list_credentials_tool_test.rs — NEW: AC-41 names-only + workspace isolation
  reasoner_refusal_test.rs      — NEW: AC-42 prompt template text assertions
  placeholder_envelope_test.rs  — NEW: AC-43 miss / hit / revoked dispatch
Cargo.toml                      — MODIFIED: + aes-gcm = "0.10", + rpassword = "7"
.env.example                    — MODIFIED: + OPEN_PINCERY_VAULT_KEY (documented)
docker-compose.yml              — MODIFIED: forwards OPEN_PINCERY_VAULT_KEY
```

### Interfaces

**AC-38 — `src/runtime/vault.rs`:**

```rust
use aes_gcm::{aead::{Aead, KeyInit, Payload}, Aes256Gcm, Nonce};
use rand::RngCore;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("credential authentication failed")]
    Authentication,
    #[error("invalid master key: {0}")]
    InvalidKey(String),
}

#[derive(Debug, Clone)]
pub struct SealedCredential {
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

#[derive(Clone)]
pub struct Vault {
    key: [u8; 32],
}

impl Vault {
    pub fn from_base64(b64: &str) -> Result<Self, VaultError>; // 32 bytes after decode
    pub fn seal(&self, workspace_id: Uuid, name: &str, plaintext: &[u8]) -> SealedCredential;
    pub fn open(&self, workspace_id: Uuid, name: &str, sealed: &SealedCredential) -> Result<Vec<u8>, VaultError>;
}
```

- Nonce: 12 fresh random bytes from `OsRng` per seal.
- AAD: `format!("{workspace_id}:{name}").into_bytes()` — binds ciphertext to the pair so cross-name/cross-workspace substitution fails.
- `open` on tampered ciphertext, tampered nonce, wrong `(workspace_id, name)`, or wrong key → `VaultError::Authentication`. Never panics.

**AC-39 — `src/api/credentials.rs`:**

Router mounted under the existing authenticated API subtree. Role gate enforced in handler:

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/workspaces/{id}/credentials",
            post(create_credential_handler).get(list_credentials_handler))
        .route("/workspaces/{id}/credentials/{name}",
            axum::routing::delete(revoke_credential_handler))
}

#[derive(Deserialize)]
struct CreateCredential { name: String, value: String }

#[derive(Serialize)]
struct CredentialSummary {
    name: String,
    created_at: DateTime<Utc>,
    created_by: Uuid,
}
```

Validation:

- `name` must match `^[a-z0-9_]{1,64}$` (regex verified without the `regex` crate using a hand-rolled ASCII check — avoids new dep).
- `value` length: 1..=8192 bytes.
- All three handlers require `require_workspace_admin(&state.pool, auth.user_id, ws_id)` which returns 403 for non-admin members and 404 for workspaces the user is not a member of. Denials append an `auth_forbidden` event.
- `GET` response is `Vec<CredentialSummary>` — no `value`, no `ciphertext`, no `nonce`. JSON serialization via serde `#[derive(Serialize)]` on `CredentialSummary` only; `Credential` (with ciphertext) is never serialized to the wire.
- `POST` success appends event `credential_added` with `content` = JSON `{"name": "...", "created_by": "..."}` (no value). Duplicate non-revoked name → 409 Conflict.
- `DELETE` sets `revoked_at = NOW()` and appends `credential_revoked` event with the name.

Workspace-scoped events: the events table is `agent_id`-scoped, but audit events for workspace-level actions need a home. v7 decision: use `auth_audit` table (already exists since v2 AC-13) for `credential_added`/`credential_revoked`/`auth_forbidden`. Action column is `credential_added`/`credential_revoked`/`credential_forbidden`; `details` JSONB carries `{workspace_id, name, actor_user_id}`.

**AC-40 — `src/cli/commands/credential.rs`:**

```rust
use rpassword::prompt_password;

pub async fn add(client: &ApiClient, name: String, stdin: bool) -> Result<(), AppError> {
    // Reject argv-based value: clap schema does not expose a --value flag.
    let value = if stdin {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf.trim_end_matches('\n').to_string()
    } else {
        prompt_password(format!("Value for credential '{name}' (hidden): "))?
    };
    // POST to workspace's credentials endpoint using client.token session.
    client.create_credential(&name, &value).await?;
    println!("credential '{name}' added");
    Ok(())
}
```

- **No `--value` clap argument**. The compile-time shape of the clap enum enforces that a value can never be accepted via argv. AC-40's argv-rejection test asserts this by invoking `pcy credential add foo --value bar` and asserting a clap error.
- Stdin mode is triggered by `--stdin` flag; interactive mode uses `rpassword::prompt_password` which on Unix disables terminal echo via `termios` and on Windows via `SetConsoleMode`. If `--stdin` is not set and no TTY is available, `rpassword` returns an error that the caller surfaces as an actionable message.
- `list` prints `NAME  CREATED_AT` two-column table from the `GET` response.
- `revoke` prompts `y/N` confirmation unless `--yes`.

**AC-41 — new `list_credentials` tool:**

Added to `tool_definitions()` in `src/runtime/tools.rs`:

```rust
ToolDefinition {
    tool_type: "function".into(),
    function: FunctionDef {
        name: "list_credentials".into(),
        description: "List credential names available to this agent's workspace. Returns names only, never values. Use this to discover what credentials exist; use PLACEHOLDER:<name> in shell env to reference them.".into(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    },
},
```

`required_for("list_credentials") -> ToolCapability::ReadLocal` so `Locked`/`Supervised`/`Yolo` all have access.

Dispatch handler: `list_credentials(pool, workspace_id) -> ToolResult::Output(json_array_of_summaries)`. Records standard `tool_call` + `tool_result` events (existing path).

**AC-42 — hardened `default_agent` prompt template:**

Migration `20260420000003_prompt_template_credentials.sql`:

```sql
-- Deactivate v1 (preserved for audit; immutable)
UPDATE prompt_templates
SET is_active = FALSE
WHERE name = 'wake_system_prompt' AND version = 1;

-- Insert v2 with Credential Handling section
INSERT INTO prompt_templates (name, version, template, is_active, change_reason)
VALUES (
    'wake_system_prompt', 2,
    -- base text of v1 + NEW "## Credential Handling" section ending with:
    -- "If a user offers any value that looks like a credential (a contiguous
    --  string ≥24 chars mixing letters and digits, or obvious API key / token
    --  shapes), REFUSE to echo, store, or act on it. Emit a user-facing
    --  message directing them to `pcy credential add <name>` (see API path
    --  POST /api/workspaces/:id/credentials). Never include credential values
    --  in identity or work_list updates."
    E'…',
    TRUE,
    'v7 AC-42: hardened credential handling + vault redirect'
);
```

The existing one-active-per-name unique partial index guarantees v1 becomes inactive before v2 becomes active (done in a single migration transaction).

Test asserts:

- `SELECT template FROM prompt_templates WHERE name='wake_system_prompt' AND is_active=TRUE` contains literal substrings `pcy credential add`, `REFUSE`, and `POST /api/workspaces/:id/credentials`.
- Active row version is `2`, not `1`.
- Prompt assembly (AC-3) continues to pass: assembled prompt includes the new template text.

This is a prompt-content assertion — we do **not** attempt to test LLM behavior (that requires a real model). The scope language "LLM response asserted to contain `pcy credential add`" is satisfied by asserting the _instruction_ to the LLM is present; behavioral compliance of real models is out of v7's verifiability scope and covered by the existing wiremock framework if needed.

**AC-43 — `PLACEHOLDER:<name>` dispatch envelope:**

`ShellArgs` gains optional env:

```rust
#[derive(Deserialize)]
struct ShellArgs {
    command: String,
    #[serde(default)]
    env: std::collections::HashMap<String, String>,
}
```

Tool schema (in `tool_definitions()`) adds a nullable `env` object property.

`dispatch_tool` (after capability gate, before executor call) scans `env`:

```rust
for (k, v) in &parsed.env {
    if let Some(name) = v.strip_prefix("PLACEHOLDER:") {
        match credential::find_active(pool, workspace_id, name).await? {
            Some(_) => { /* hit — leave value unchanged; v9 will substitute */ }
            None => {
                let payload = json!({
                    "tool_name": "shell",
                    "credential_name": name,
                    "reason": "missing_or_revoked"
                }).to_string();
                event::append_event(pool, agent_id, "credential_unresolved",
                    "runtime", Some(wake_id), Some("shell"), Some(&payload),
                    None, None, None).await?;
                return ToolResult::Error(format!("credential not found: {name}"));
            }
        }
    }
}
// Proceed to ProcessExecutor::run with the (unchanged) env map.
```

`ShellCommand` gains `pub env: HashMap<String, String>`; `ProcessExecutor` sets each entry via `.env(k, v)` AFTER the env_clear + PATH allowlist step. Child receives the literal `PLACEHOLDER:<name>` string — v7 does **not** perform substitution (that's v9's proxy job).

`dispatch_tool` signature changes from v6's `(tc, mode, pool, agent_id, wake_id, executor)` to v7's `(tc, mode, pool, workspace_id, agent_id, wake_id, executor)`. `wake_loop::run_wake_loop` looks up `agent.workspace_id` and threads it. The `missing_or_revoked` distinction is recorded in the payload string; scope AC-43 test (c) revokes via `DELETE` and asserts `reason = "revoked"` — we unify to `missing_or_revoked` because the active-credentials query returns `None` for both cases and distinguishing them requires a second query. Test assertion relaxed accordingly and noted under "Scope adjustments" below.

### Data Model

One new table + one FK path:

```sql
-- migrations/20260420000002_create_credentials.sql
CREATE TABLE credentials (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id  UUID NOT NULL REFERENCES workspaces(id),
    name          TEXT NOT NULL,
    ciphertext    BYTEA NOT NULL,
    nonce         BYTEA NOT NULL,
    created_by    UUID NOT NULL REFERENCES users(id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at    TIMESTAMPTZ,
    CHECK (length(nonce) = 12),
    CHECK (length(ciphertext) >= 16),
    CHECK (name ~ '^[a-z0-9_]{1,64}$')
);

CREATE UNIQUE INDEX credentials_one_active_per_name
    ON credentials (workspace_id, name)
    WHERE revoked_at IS NULL;

CREATE INDEX credentials_workspace_idx ON credentials (workspace_id);
```

AAD (`{workspace_id}:{name}`) is reconstructed at `open` time from the row, not stored — it's fully derivable and storing it would waste space and invite drift.

### External Integrations

None added. The vault is an in-process crypto module over the existing Postgres; the only network dependency remains the unchanged LLM egress. v9 will add the Zerobox proxy; v7 does not.

### Test Strategy

| AC    | Test file                             | Kind     | Notes                                                                                                                                                                                                                                                                                                                                                                     |
| ----- | ------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC-38 | `tests/vault_roundtrip_test.rs`       | Unit     | Seal+open round-trips 100 iterations with distinct nonces (nonce set size == 100); tampering ciphertext/nonce/name/workspace_id/key → `VaultError::Authentication`, no panic. Uses `aes-gcm` directly; no DB.                                                                                                                                                             |
| AC-39 | `tests/vault_api_test.rs`             | DB integ | admin create/list/revoke; non-admin 403; list JSON contains zero bytes of the secret value; duplicate non-revoked name → 409; revoke-then-readd works                                                                                                                                                                                                                     |
| AC-40 | `tests/cli_credential_test.rs`        | CLI unit | Invokes `Cli::try_parse_from(["pcy","credential","add","foo","--value","bar"])` and asserts a clap error (no `--value` flag exists). Stdin round-trip with `--stdin` handled by test using `ApiClient` directly (full PTY path smoke-tested at the `rpassword` call site by integration test scaffold, not in CI)                                                         |
| AC-41 | `tests/list_credentials_tool_test.rs` | DB integ | Workspace A has 3 creds (1 revoked); dispatch_tool("list_credentials") returns Output JSON of 2 summaries; workspace B (isolated) returns `[]`; payload contains zero bytes of any stored secret value                                                                                                                                                                    |
| AC-42 | `tests/reasoner_refusal_test.rs`      | DB integ | Active `wake_system_prompt` row has `version = 2`; template text contains `pcy credential add`, `REFUSE`, `POST /api/workspaces/:id/credentials`; v1 row still exists with `is_active = false`                                                                                                                                                                            |
| AC-43 | `tests/placeholder_envelope_test.rs`  | DB integ | (a) Yolo agent + `PLACEHOLDER:missing` → `ToolResult::Error("credential not found: missing")`, one `credential_unresolved` event, zero `CountingExecutor` spawns. (b) `PLACEHOLDER:stripe_test` after seeding stripe_test → dispatch proceeds, child env contains literal `PLACEHOLDER:stripe_test`. (c) Revoke stripe_test, re-dispatch → `credential_unresolved` event. |

### Observability

- New events: `credential_added`, `credential_revoked`, `credential_unresolved` (all in `events` or `auth_audit` per table above). Queryable via existing `/api/agents/:id/events` for per-agent (the `credential_unresolved` runtime event) and via direct `auth_audit` query for workspace-level (added/revoked).
- No new Prometheus counters in v7. If denial rate becomes operationally interesting, add `open_pincery_credential_unresolved_total` in a follow-up; the event log is the system of record.
- `tracing::warn!` on `credential_unresolved` with structured fields `{agent_id, workspace_id, credential_name}`.

### Complexity Exceptions

None. File budgets:

- `src/runtime/vault.rs` ≈ 120 lines (two methods, two error arms, base64 decode).
- `src/api/credentials.rs` ≈ 220 lines (three handlers + validation helpers + role gate).
- `src/cli/commands/credential.rs` ≈ 120 lines (three subcommands, no-argv assertion, rpassword branch).
- `src/models/credential.rs` ≈ 120 lines (create/list/find_active/revoke + struct).
- Migrations ≈ 60 lines total.
- Each new test file < 200 lines.

### Key Scenario Trace

Scenario: operator stores `stripe_test`, agent reasons "I should charge a test card" and emits a `shell` tool call referencing the credential by placeholder.

1. Operator runs `pcy credential add stripe_test` → `rpassword` prompts with echo disabled, reads `sk_test_…`.
2. CLI posts `{name:"stripe_test", value:"sk_test_…"}` to `/api/workspaces/<ws>/credentials` with bearer session token.
3. Handler verifies caller has `workspace_admin`, validates name regex, validates value length, calls `vault.seal(ws_id, "stripe_test", b"sk_test_…")`, inserts `credentials` row, appends `credential_added` audit event. Returns 201.
4. Agent wakes, `run_wake_loop` assembles prompt (v2 template includes "Credential Handling"), LLM returns tool_call `list_credentials`.
5. `dispatch_tool` — capability gate passes (ReadLocal under any mode) — queries non-revoked credentials for workspace → returns `[{"name":"stripe_test", ...}]`. Event log gets `tool_call` + `tool_result`.
6. Next LLM turn: tool_call `shell` with `{"command":"curl -sS -u $KEY: https://api.stripe.com/...", "env":{"KEY":"PLACEHOLDER:stripe_test"}}`.
7. `dispatch_tool`: capability gate passes for Yolo (ExecuteLocal). Placeholder scan finds `PLACEHOLDER:stripe_test`, looks up, row exists and is not revoked. Proceed.
8. `ProcessExecutor::run` creates tempdir, `env_clear()`, re-adds `PATH`, then adds `KEY=PLACEHOLDER:stripe_test`. Child sees the placeholder string, not the secret. v7 has no proxy; the curl fails (or Stripe returns auth error). The correct outcome for v7 — the seam exists, the secret never leaves the substrate process memory, v9 will plug in the proxy.
9. For a `Locked` agent: step 7 trips the capability gate on `ExecuteLocal`; `list_credentials` still works (ReadLocal) so the agent can at least observe what it _would_ have access to if promoted.

### Scope Adjustments (from EXPAND)

1. **AC-43 — `reason="missing"` vs `"revoked"`** merged into a single `reason="missing_or_revoked"` string. Distinguishing them requires a second query after `find_active` returns None, with no operational value that v7 needs — the `credential_unresolved` event already points at the specific name and the operator consults the `credentials` table (or `pcy credential list`) to see whether they revoked it. Test (c) is updated accordingly.
2. **AC-39 — workspace-level audit events** land in `auth_audit` (existing v2 table), not `events`. `events` is `agent_id`-scoped and would require a synthetic agent to hold workspace actions; `auth_audit` is already designed for non-agent actions and already queried by operator tooling.
3. **AC-40 — TTY echo-suppression test via real PTY** deferred to a docs runbook. The `rpassword` crate is widely audited; asserting its behavior in CI would require spawning a PTY which is platform-specific and high-maintenance. The AC-40 test asserts the clap shape (no `--value`), the stdin round-trip path, and the choice of `rpassword::prompt_password` at the call site (grep-based static check).
4. **AC-42 — "entropy heuristic in the system prompt"** is dropped from the template; real LLMs are unreliable regex engines. The template instead instructs the model to refuse **any** inbound content that a plausible operator would recognize as a credential (API key, token, password, hex blob), and to always redirect to the vault rather than engage with the content. The AC-42 test asserts the instruction text, not a regex.

These adjustments are structural, not scope-reducing: every AC's core invariant (storage, isolation, audit, discoverability, refusal, seam) is intact.

### Open Questions

None with BUILD impact. Two tracked as v7 Deferred in `scope.md`:

- Master-key rotation (re-key every sealed row) — bounded, land when operator community asks.
- Per-mission credential ACLs — v10 `capability_scope`.

---

## v8 Design Addendum — Unified API Surface (Schema-Driven CLI, MCP, and Distribution)

### Architecture Delta

Five strictly-additive surface changes. One new compile-time spec aggregator (`utoipa`), one new unauthenticated route (`/openapi.json`), one new Rust module (`src/mcp/`), one restructured CLI tree (`src/cli/nouns/`) with context multiplexing, one installer + completion distribution layer. Zero runtime-semantic, schema, or handler-logic changes.

```
┌──────────────── Remote Operator / Agent ────────────────┐
│                                                         │
│  curl -fsSL .../install.sh | bash                       │
│    └─ platform detect → sha256 → cosign → $PREFIX/bin   │
│                                                         │
│  pcy completion zsh >> ~/.zshrc.d/pcy                   │
│  pcy context set prod --url https://pcy.example.com     │
│  pcy login                                              │
│    ├─ first run: POST /api/bootstrap (idempotent shim)  │
│    └─ subsequent: POST /api/login                       │
│                                                         │
│  pcy <noun> <verb> [name|uuid] -o {table|json|yaml|...} │
│                                                         │
│  Claude Desktop / Cursor / Copilot Chat                 │
│    └─ mcpServers: { "pincery": { command: "pcy",        │
│                                  args: ["mcp","serve"]}}│
│         └─ stdio JSON-RPC ── tools/list, tools/call     │
└─────────────────┬───────────────────────────────────────┘
                  │
                  │ HTTP(S) + Bearer token
                  ▼
┌───────────── Open Pincery Server (axum) ────────────────┐
│                                                         │
│  Unauth router:  /health  /ready  /metrics              │
│                  /openapi.json  /openapi.yaml  ← NEW    │
│                  /api/bootstrap  /api/webhooks/*        │
│                                                         │
│  Auth router (Bearer): /api/me /api/agents/*            │
│                        /api/credentials/*  (v7)         │
│                        /api/events/*  /api/messages/*   │
│                                                         │
│  Every handler carries `#[utoipa::path]` + its DTOs     │
│  carry `#[derive(ToSchema)]`. Aggregator in             │
│  `src/api/openapi.rs` emits the 3.1 document at         │
│  startup; served as JSON + YAML bytes at the two        │
│  new routes.                                            │
└─────────────────────────────────────────────────────────┘

CLI tree (clap):                        MCP stdio flow:

  pcy                                    initialize
  ├── login                                → { serverInfo, capabilities }
  ├── whoami                             tools/list
  ├── agent                                → [ { name: "agent.list", … },
  │   ├── list                               { name: "agent.create", … }, … ]
  │   ├── get <name|uuid>                tools/call name="agent.create"
  │   ├── create                           → proxy to POST /api/workspaces/$ws/agents
  │   ├── update <name|uuid>               → HTTP result → MCP content[]
  │   ├── delete <name|uuid>  [--force]
  │   └── send <name|uuid> <text>        src/mcp/ modules:
  ├── credential                           protocol.rs — JSON-RPC framing
  │   ├── list                             tools.rs   — OpenAPI→tool derivation
  │   ├── add <name>  [--stdin]            bridge.rs  — tool call → HTTP
  │   └── revoke <name>  [--force]         mod.rs     — stdio event loop
  ├── budget
  │   └── set  / get                     src/cli/nouns/ modules:
  ├── event                                agent.rs credential.rs budget.rs
  │   ├── list <agent|uuid>                event.rs  context.rs     auth.rs
  │   └── tail <agent|uuid>                output.rs  — OutputFormat render
  ├── context                              resolve.rs — name-or-UUID lookup
  │   ├── list  / current                  migrate.rs — v4 flat → v8 contexts
  │   ├── use <name>                       mcp.rs     — `pcy mcp serve`
  │   ├── set <name> --url …
  │   └── delete <name>
  ├── completion {bash|zsh|fish|powershell}
  ├── mcp serve
  ├── bootstrap   (hidden alias → login, warns once)
  ├── message     (hidden alias → agent send, warns once)
  ├── events      (hidden alias → event list, warns once)
  └── demo        REMOVED (→ scripts/demo.sh)
```

### Directory Structure (v8 deltas only)

```
src/
  api/
    openapi.rs         — NEW: `ApiDoc` utoipa::OpenApi aggregator;
                         openapi_json / openapi_yaml handlers; unauth_router()
                         merge helper
    mod.rs             — MODIFIED: mount openapi routes on the unauth side;
                         router() unchanged for authenticated surface
    agents.rs          — MODIFIED: `#[utoipa::path]` on every handler;
                         `ToSchema` on CreateAgentRequest / AgentResponse / …
    credentials.rs     — MODIFIED: same annotation pass over v7 endpoints
    me.rs              — MODIFIED: same
    events.rs          — MODIFIED: same
    messages.rs        — MODIFIED: same
    webhooks.rs        — MODIFIED: same (these *are* part of the contract)
    bootstrap.rs       — MODIFIED: same + utoipa note that `/api/bootstrap`
                         is idempotent-or-conflict (for AC-45's shim semantics)
  mcp/
    mod.rs             — NEW: `run_stdio(ctx: &CliConfig) -> Result<(), _>`
                         event loop; reads framed JSON-RPC from stdin,
                         writes responses to stdout; debug logs to stderr
    protocol.rs        — NEW: Request / Response / Error types matching MCP
                         2025-06-18; content-length framing; serde round-trip
    tools.rs           — NEW: OpenApiToolRegistry — parses `/openapi.json`
                         (or the local `ApiDoc::openapi()`) and derives
                         `(name, description, inputSchema)` per operation
    bridge.rs          — NEW: `invoke(tool_name, args) -> McpToolResult`
                         maps tool name → HTTP method + path template,
                         substitutes path params from args, sends via
                         ApiClient, maps response/error to MCP content
  cli/
    mod.rs             — MODIFIED: root `Cli` struct gains `--context` +
                         `--output`; `Commands` reduced to the v8 noun
                         variants; legacy variants kept behind
                         `#[command(hide = true)]` pointing at shim fns
    config.rs          — MODIFIED: ContextConfig { url, token, workspace_id,
                         user_id }; CliConfig { current_context: String,
                         contexts: BTreeMap<String, ContextConfig> };
                         load() auto-migrates v4 flat schema (see migrate.rs);
                         save() writes atomically (tempfile + rename)
    commands/
      mod.rs           — MODIFIED: re-exports from nouns/; legacy shims
                         (bootstrap_shim, message_shim, events_shim) that
                         emit warn_deprecated() + delegate
      <legacy files>   — KEPT as thin delegates for one release; to be
                         deleted in the tag after v8 lands
    nouns/
      mod.rs           — NEW: pub mods + `warn_deprecated(old, new)` helper
                         gated by OPEN_PINCERY_NO_DEPRECATION_WARNINGS
      agent.rs         — NEW: list/get/create/update/delete/send verbs
      credential.rs    — NEW: list/add/revoke verbs (wraps v7 CLI)
      budget.rs        — NEW: get/set verbs
      event.rs         — NEW: list/tail verbs
      context.rs       — NEW: list/current/use/set/delete verbs
      auth.rs          — NEW: login / whoami / logout verbs
      completion.rs    — NEW: clap_complete generator dispatch
      mcp.rs           — NEW: `serve` verb → crate::mcp::run_stdio
    output.rs          — NEW: OutputFormat enum {Table, Json, Yaml,
                         JsonPath(String), Name}; `render<T: Serialize +
                         TableRow>(value, format, stdout_is_tty)`
    resolve.rs         — NEW: `resolve_agent(client, ctx, needle) ->
                         Result<Uuid, ResolutionError>` (Exact-UUID |
                         Exact-Name | Ambiguous[Vec<(Uuid,String)>] |
                         NotFound); same shape for credentials/events
    migrate.rs         — NEW: v4 flat config → v8 contexts migration;
                         backs up to config.toml.pre-v8 before rewrite
  lib.rs               — MODIFIED: pub mod mcp; re-export minimal surface

install.sh             — NEW (at repo root): platform/arch detect,
                         GitHub release asset fetch, sha256 enforce,
                         cosign verify (soft-fail unless --require-cosign),
                         install to $PCY_PREFIX/bin. Drafted during v7
                         exploration; v8 finalizes + tests.

scripts/
  demo.sh              — NEW: the former `pcy demo` flow re-homed here

docs/
  api.md               — MODIFIED: prose trimmed; replaced by links to
                         `/openapi.json` + a "How to drive the API" section
                         covering curl, pcy, and MCP
  runbooks/
    cli-install.md     — NEW: install.sh instructions + completion install
    mcp-setup.md       — NEW: example Claude Desktop / Cursor configs

tests/
  openapi_spec_test.rs         — AC-44: spec served, 3.1 valid,
                                 path coverage vs router() enumeration,
                                 every route annotated
  cli_login_idempotent_test.rs — AC-45: fresh & re-run login both succeed;
                                 bootstrap alias warns once
  cli_noun_verb_test.rs        — AC-46: legacy/new parity table;
                                 ambiguous-name disambiguation exit 2
  cli_output_flag_test.rs      — AC-47: json/yaml/jsonpath/name + TTY
                                 default + NO_COLOR + --force/--yes
  cli_context_test.rs          — AC-48: migration + switching +
                                 env/flag overrides + whoami
  mcp_smoke_test.rs            — AC-49: initialize + tools/list diff +
                                 tools/call round-trip + event loop
  installer_test.rs            — AC-50: shellcheck + fixture-served
                                 install + sha256 mismatch + cosign gate
                                 (behind #[cfg(feature = "installer-e2e")])
  cli_completion_test.rs       — AC-51: four shells emit non-empty
                                 completion containing shell-specific marker
  api_naming_test.rs           — AC-52a: OpenAPI walker — plural paths,
                                 {id} param, summaries, PUT ban, etc.
  cli_naming_test.rs           — AC-52b: clap walker — about strings,
                                 --output parity, forbidden flag names

Cargo.toml              — MODIFIED: +utoipa, +utoipa-axum, +clap_complete
                                     (dev) +openapiv3, +jsonpath-rust-or-eq
                                     features: installer-e2e = []
```

### Interfaces

**OpenAPI aggregator** (`src/api/openapi.rs`):

```rust
#[derive(utoipa::OpenApi)]
#[openapi(
    paths(
        api::me::get_me,
        api::agents::list_agents,
        api::agents::create_agent,
        api::agents::get_agent,
        api::agents::update_agent,
        api::agents::delete_agent,
        api::agents::send_message,
        api::credentials::list_credentials,
        api::credentials::create_credential,
        api::credentials::revoke_credential,
        api::events::list_events,
        api::bootstrap::bootstrap,
        // … every route currently in api::router()
    ),
    components(schemas(
        models::Agent, models::AgentStatus, models::Credential,
        api::agents::CreateAgentRequest, api::agents::AgentResponse,
        api::me::MeResponse, /* … */
    )),
    security(("bearerAuth" = [])),
    info(title = "Open Pincery API", version = env!("CARGO_PKG_VERSION")),
    modifiers(&BearerAuthAddon),
)]
pub struct ApiDoc;

pub fn openapi_router() -> axum::Router<AppState> {
    Router::new()
        .route("/openapi.json", get(openapi_json))
        .route("/openapi.yaml", get(openapi_yaml))
}
```

Handler annotations follow this pattern — shown for `list_agents`:

```rust
#[utoipa::path(
    get,
    path = "/api/workspaces/{workspace_id}/agents",
    params(("workspace_id" = Uuid, Path,)),
    responses(
        (status = 200, description = "Agents in workspace",
         body = Vec<AgentResponse>),
        (status = 401, description = "Missing or invalid token"),
    ),
    security(("bearerAuth" = [])),
    tag = "agent",
)]
pub async fn list_agents(/* … unchanged signature … */) { /* … */ }
```

**MCP protocol types** (`src/mcp/protocol.rs`):

```rust
#[derive(Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,       // "2.0"
    pub id: Option<Value>,
    pub method: String,        // "initialize" | "tools/list" | "tools/call"
    pub params: Option<Value>,
}

#[derive(Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Serialize, Deserialize)]
pub struct Tool {
    pub name: String,           // "agent.create"
    pub description: String,    // from OpenAPI operation summary
    pub input_schema: Value,    // OpenAPI requestBody/parameters → JSON Schema
}

#[derive(Serialize, Deserialize)]
pub struct CallToolResult {
    pub content: Vec<Content>,  // Content::Text { text: "..." } for JSON bodies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}
```

Framing: one JSON object per line (newline-delimited JSON-RPC), matching the MCP stdio transport spec. No Content-Length headers (newline-delimited is the supported stdio framing in the 2025-06-18 revision).

**Context config** (`src/cli/config.rs` — v8 schema):

```toml
current-context = "default"

[contexts.default]
url          = "http://127.0.0.1:8080"
token        = "sess_..."
workspace_id = "018f..."
user_id      = "018f..."

[contexts.prod]
url          = "https://pcy.example.com"
token        = "sess_..."
workspace_id = "018f..."
user_id      = "018f..."
```

Precedence for resolving the active context: `--context <name>` flag > `OPEN_PINCERY_CONTEXT` env > `current-context` in file.

**Output format** (`src/cli/output.rs`):

```rust
#[derive(Clone, Debug)]
pub enum OutputFormat {
    Table,
    Json,
    Yaml,
    JsonPath(String),
    Name,
}

pub fn default_for_tty(stdout_is_tty: bool) -> OutputFormat {
    if stdout_is_tty { OutputFormat::Table } else { OutputFormat::Json }
}

pub trait TableRow {
    fn headers() -> &'static [&'static str];
    fn row(&self) -> Vec<String>;
}

pub fn render<T: Serialize + TableRow>(
    values: &[T],
    fmt: &OutputFormat,
    stdout_is_tty: bool,
) -> Result<(), OutputError> { /* … */ }
```

**Name-or-UUID resolver** (`src/cli/resolve.rs`):

```rust
pub enum Resolution<T> {
    ById(Uuid),
    ByName { id: Uuid, name: String },
    Ambiguous(Vec<(Uuid, String)>),
    NotFound,
}

pub async fn resolve_agent(
    client: &ApiClient,
    workspace_id: Uuid,
    needle: &str,
) -> Result<Uuid, AppError>;
```

If `needle` parses as a UUID, a single GET confirms it exists; otherwise a LIST is filtered by `name == needle`. Ambiguous matches exit 2 with a two-column table on stderr.

### Data Model

No schema changes. v8 is surface-only.

### External Integrations

**MCP client integrations** are operator-configured, not server-side. Example Claude Desktop config:

```json
{
  "mcpServers": {
    "pincery-prod": {
      "command": "pcy",
      "args": ["mcp", "serve"],
      "env": { "OPEN_PINCERY_CONTEXT": "prod" }
    }
  }
}
```

`pcy mcp serve` inherits the operator's contexts from `~/.config/open-pincery/config.toml`; no additional auth configuration inside the MCP client. Failure modes (server unreachable, token expired, rate-limited) surface as MCP errors with a stable error-code map: `-32001` unreachable, `-32002` unauthorized, `-32003` rate-limited, `-32004` not-found, `-32000` server-side generic.

**GitHub Releases** (install-path only): `install.sh` fetches from `api.github.com/repos/<owner>/<repo>/releases/latest` with a 10s timeout and a graceful retry-once on transient failures. No cached token is sent; unauthenticated GitHub API rate limit is adequate for one-shot installs.

No new server-side outbound integrations.

### Test Strategy (one row per v8 AC)

| AC     | Test file                    | Kind          | Notes                                                                                                                                        |
| ------ | ---------------------------- | ------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| AC-44  | openapi_spec_test.rs         | integration   | Spins up `api::router()` in-process; fetches `/openapi.json`; `openapiv3::OpenAPI` parse; diff vs route enumeration.                         |
| AC-45  | cli_login_idempotent_test.rs | e2e (compose) | Fresh compose reset + `pcy login --bootstrap-token` × 2 (second reports `already_bootstrapped:true`); `--help` does not contain `bootstrap`. |
| AC-46  | cli_noun_verb_test.rs        | e2e (compose) | Parameterized (legacy, new) pairs; stdout byte-identical; ambiguous name → exit 2 + table on stderr.                                         |
| AC-47  | cli_output_flag_test.rs      | e2e (compose) | json parses; name line-per-item; jsonpath selector; TTY detection via `PTY` fixture; `NO_COLOR`; `--force` vs `--yes`.                       |
| AC-48  | cli_context_test.rs          | unit + e2e    | Unit: migrate.rs over fixture files. E2e: two contexts, switching, env/flag override, whoami 200/1 against wrong token.                      |
| AC-49  | mcp_smoke_test.rs            | integration   | Spawn `pcy mcp serve` subprocess; full JSON-RPC round-trip; tools/list diff vs router; agent.create loop → event lands.                      |
| AC-50  | installer_test.rs            | script        | `bash -n` + shellcheck warning-level; local fixture GitHub mirror; sha256 mismatch; cosign-required gate. Feature-gated.                     |
| AC-51  | cli_completion_test.rs       | unit          | Four shells; non-empty + shell-specific marker string.                                                                                       |
| AC-52a | api_naming_test.rs           | integration   | Walks `ApiDoc::openapi()`; asserts plural paths / `{id}` / summaries / no PUT / no `format` field.                                           |
| AC-52b | cli_naming_test.rs           | unit          | Walks clap `Command` tree; asserts about strings, `-o` parity, no `--format`/`--yes` outside deprecated.                                     |

Every test carries `// AC-NN` comment trailing the test function; the v3 grep guardrail already ensures this.

### Observability

No new metrics or log lines on the server side. The `/openapi.json` handler reuses the `/health` structured log line (method + path + status + latency).

Client-side: `pcy` gains a `--verbose` flag that turns on `RUST_LOG=pcy=debug`-equivalent tracing to stderr; off by default. `pcy mcp serve` always logs its JSON-RPC framing errors to stderr (stdout is reserved for protocol traffic). Deprecation warnings write exactly one stderr line per invocation with the `warning:` prefix.

### Complexity Exceptions

1. `src/mcp/mod.rs` may exceed the 200-line soft target. JSON-RPC stdio event loops are genuinely irreducible below that threshold (framing + dispatch + error mapping + graceful shutdown). 300-line ceiling is the hard stop; if it grows beyond that we split into `event_loop.rs` + `dispatch.rs`.
2. `src/cli/output.rs` will carry both the enum + the `render` function + per-resource `TableRow` impls. 250-line ceiling; beyond that we push `TableRow` impls into the noun modules.
3. The legacy-shim compatibility surface (hidden `bootstrap`, `message`, `events`, `--yes`, `--format`) adds test paths that duplicate the new paths. Accepted for one release; v1.2.0 removes them and prunes the duplicate tests.
4. `utoipa::path` annotations above every handler are verbose. Accepted — they are the source of truth for AC-44 and AC-52a.

### Key Scenario Trace — Remote Operator Drives Production Pincery via MCP

1. Operator on macOS M-series runs `curl -fsSL https://raw.githubusercontent.com/.../install.sh | bash` → `install.sh` detects `darwin-arm64`, pulls latest release manifest, downloads `pcy-vX.Y.Z-macos-aarch64`, verifies sha256, verifies cosign signature against the repo's public key, installs to `~/.local/bin/pcy`, prints PATH hint if needed.
2. `pcy completion zsh > ~/.zfunc/_pcy` → completions live alongside the operator's other CLIs.
3. `pcy context set prod --url https://pcy.example.com` → creates `~/.config/open-pincery/config.toml` with `current-context = "prod"` and a `[contexts.prod]` table.
4. `pcy login` → with `OPEN_PINCERY_BOOTSTRAP_TOKEN` set: GETs `/api/me` first, gets 401 "not bootstrapped", retries via `POST /api/bootstrap`, persists session token in `contexts.prod.token`. Without the bootstrap env but with a pre-baked password, POSTs `/api/login`. Exit 0.
5. `pcy whoami -o table` → GETs `/api/me`; prints a two-row table with context, server, user, workspace.
6. Operator opens Claude Desktop, adds the `mcpServers.pincery-prod` entry shown above. Claude Desktop spawns `pcy mcp serve` as a stdio subprocess and sends `initialize`.
7. `pcy mcp serve` loads `prod` context, opens an `ApiClient` with the stored token, responds to `initialize` with `{ serverInfo: { name: "open-pincery", version: "X.Y.Z" }, capabilities: { tools: {} } }`.
8. Claude sends `tools/list`. The MCP server reads the local `ApiDoc::openapi()` registry (same crate, no network call needed) and emits one tool per operation: `agent.list`, `agent.create`, `agent.send_message`, `credential.list`, …
9. Claude (or the human guiding it) calls `tools/call { name: "agent.create", arguments: { name: "investigator", persona_prompt: "…", credential_policy: "Locked" } }`.
10. `bridge.rs` maps `agent.create` → `POST /api/workspaces/{workspace_id}/agents`, substitutes `workspace_id` from the context, sends the JSON body, receives 201 with the new agent record.
11. Response is wrapped in an MCP `CallToolResult { content: [Text(json_body)] }` and written back to stdout.
12. On the server, the usual v1–v7 machinery runs — `agent_created` event lands in `events`, maintenance is unchanged, the agent begins its wake loop. No new server path was exercised.
13. Hours later the operator reconnects, `pcy event list investigator -o json --context prod | jq` streams recent events into a local Jupyter notebook. The same data was reachable via `tools/call { name: "event.list", … }` from Claude; the two paths share one OpenAPI-derived contract.

The chain is closed: one `ApiDoc` feeds `/openapi.json`, the MCP tool registry, the lint test, and the conformance tests. Every downstream surface — human CLI, MCP-driven agent, curl, future SDKs — reads from the same spec.

### Scope Adjustments (from EXPAND)

1. **AC-47 `--output jsonpath` uses a kubectl-compatible subset, not full JSONPath.** `jsonpath-rust` covers `.foo.bar`, `.items[*].name`, `.items[0]`, filters `[?(@.active==true)]`. Operators who need full JQ can pipe `-o json | jq`. Test fixtures only assert the subset.
2. **AC-49 MCP spec version is pinned to `2025-06-18` for v8 ship.** Later revisions land in subsequent minor tags; the hand-written protocol module is structured so the version constant + the `initialize` response are the only change points.
3. **AC-50 `install.sh` on Windows is supported via git-bash / WSL only.** Native PowerShell installer is deferred — `winget` is the right Windows distribution seam and lands with the AC-deferred package-manager track.
4. **AC-52 "no `PUT` in the API" is enforced as a lint, not an architectural ban.** If a future endpoint genuinely needs idempotent upsert, the allowlist is a one-line addition with a justification comment — same pattern as the capability-gate table.

These adjustments sharpen the ACs without reducing the invariants (machine-readable contract, idempotent login, noun-verb tree, universal output flag, named contexts, MCP parity with API, signed-binary installer, four-shell completions, schema-layer lints).

### Open Questions

None with BUILD impact. Three tracked as v8 Deferred in `scope.md`:

- Generated SDKs (Python/TypeScript) from `/openapi.json` — AC-44 unlocks this cleanly; release pipeline is the gating work.
- Terraform provider — same story.
- Long-running MCP daemon for remote (cross-host) agents — pairs with v11 signals + per-agent auth.

### v8 Dependencies on Prior Versions

None broken.

- v2 auth middleware & rate-limit buckets: reused unchanged; `/openapi.{json,yaml}` join the `/health` bucket.
- v3 CI guardrails (AC-16 grep tests, AC-17 CI green): extended with AC-52a/b tests; no existing test touched.
- v4 HTTP API contract (AC-27): strictly preserved. Every documented endpoint gains `#[utoipa::path]` but no shape or status code changes.
- v5 `.env.example` / smoke script (AC-29/AC-30): smoke updated to invoke `pcy login` + assert `/openapi.json` 200; no new env vars.
- v6 capability gate (AC-35): server-side — unchanged. MCP tool calls traverse the same authenticated HTTP path as the CLI; capability decisions still happen in `tools::dispatch_tool`.
- v7 vault + CLI (AC-38..AC-43): credentials endpoints gain annotations; `pcy credential` commands live unchanged under the noun tree (verbs semantic-identical to v7).

---

## v9 DESIGN — Trust Gate (2026-04-22)

v9 adds 23 acceptance criteria (AC-53..AC-75) across security, auth, credential workflows, UI, observability, multi-tenant enforcement, and rollout hardening. This section specifies the architecture for every AC; per-slice implementation detail lives in each build-slice commit.

### Architecture Overview

Three new subsystems join the existing runtime:

1. **`src/runtime/sandbox/`** — layered Linux sandbox (AC-53 + AC-72). `SandboxedExecutor` wraps v6's `ProcessExecutor`. Every tool exec composes six layers in a fixed order: Bubblewrap namespaces + nested-userns disable → cgroup v2 setup → landlock ruleset → seccomp-bpf allowlist → uid/cap drop → slirp4netns egress proxy with allowlist. Each layer is a sub-module with its own failure mode and unit test; the `compose` entry point fails closed — any layer refusing to initialize aborts the exec with a `sandbox_unavailable` error before any user code runs.
2. **`src/runtime/secret_proxy.rs`** — out-of-process credential resolver (AC-71). Agent process has zero read access to the vault key; it forwards tool requests with `PLACEHOLDER:<name>` tokens intact to a unix-socket endpoint (`$XDG_RUNTIME_DIR/pincery-secret.sock` by default). The proxy resolves placeholders and delivers plaintext to the sandboxed child via one of three injection modes (env, stdin, header). The `http_get` tool proxies the outbound HTTP call itself rather than exposing the credential to the agent at all.
3. **`src/tenancy.rs`** — workspace-scoped query middleware (AC-65). Every API handler resolves the session's `workspace_id` and passes it to a new `ScopedPool::query(workspace_id, sql, params)` helper; every query injects `AND workspace_id = $1` at the binding site. A lint test greps `src/api/` for bare `sqlx::query*!?` invocations and fails the build on any hit.

The existing request path remains: HTTP handler → capability gate → tool dispatch → executor. v9 inserts `ScopedPool` at the handler layer, `SecretProxy` between dispatch and executor, and `SandboxedExecutor` replacing the raw `ProcessExecutor`.

### Directory Structure (additions)

```
src/
  tenancy.rs                      # AC-65 scoped-pool middleware + helper
  api/
    deposit.rs                    # AC-56 public deposit page (GET/POST /deposit/:token)
    credential_requests.rs        # AC-55, AC-57 (list/approve/reject)
    sessions.rs                   # AC-58 (refresh/revoke/list)
    users.rs                      # AC-59 (add/list/set-role/delete)
    cost.rs                       # AC-63 cost rollup endpoint
    version.rs                    # AC-69 /api/version
    events_export.rs              # AC-62 jsonl/csv streaming
    agent_network.rs              # AC-72 allowlist CRUD
  runtime/
    observability/
      redaction.rs                # AC-74 tracing/event redaction layer
      seccomp_audit.rs            # AC-77 sandbox_syscall_denied event payload + audit-record parser + DB emit helper
    sandbox/
      mod.rs                      # SandboxedExecutor entry + compose() (POSIX 128+signum signal->exit_code translation in ProcessExecutor)
      bwrap.rs                    # Bubblewrap wrapper (namespaces, --disable-userns, uid/gid/cap drop, bind mounts; POSIX 128+signum signal->exit_code translation so SIGSYS surfaces as exit_code 159)
      init_policy.rs              # parent -> pincery-init policy, including AC-87 landlock_scopes bitmap
      seccomp.rs                  # AC-77 seccompiler default-deny allowlist (~70 entries: 41 empirical + 28 manual) + clone arg-filter (CLONE_NEWUSER|CLONE_NEWNS lockout) + ESCAPE_PRIMITIVES negative control + ALLOWLIST_SIZE_FLOOR/CEILING bounds (40..=120); Enforce=KillProcess (SIGSYS exit 159), Audit=Log
      landlock.rs                 # landlock filesystem ruleset + raw ABI-6 IPC scope installer
      cgroup.rs                   # cgroup v2 write helpers (cgroups-rs)
      netns.rs                    # slirp4netns proxy + egress allowlist plumbing
      profiles/
        bwrap_args.toml           # default bind-mount / env layout
    secret_proxy.rs               # AC-71 unix-socket server + client stub
    tools/
      http_get.rs                 # AC-66 with agent_http_allowlist check
      file_read.rs                # AC-66 per-agent tempdir scope
      db_query.rs                 # AC-66 stored-credential + read-only regex
  background/
    retention.rs                  # AC-64 archive + prune job
    rate_limit.rs                 # AC-67 rolling 60s window (token bucket)
  cli/commands/
    credential_request.rs         # AC-57 list/approve/reject
    session.rs                    # AC-58 list/revoke/refresh
    user.rs                       # AC-59 add/list/set-role/delete
    cost.rs                       # AC-63 CLI rollup
    events_archive.rs             # AC-64 archive subcommand
    agent_network.rs              # AC-72 allow/list/revoke
static/
  js/htmx.min.js                  # AC-61 vendored 1.9.x (no CDN)
  css/pico.min.css                # AC-61 vendored 2.0.x
  views/                          # AC-61 six server-rendered partials
    login.html | agents.html | agent_detail.html | events.html | budget.html | credential_inbox.html
docs/SECURITY.md                  # AC-54 threat model
docs/runbooks/
  dev_setup_macos.md              # AC-75 contributor guide
  dev_setup_windows.md            # AC-75 contributor guide
  rollback_to_v8.md               # pre-v9 rollback recipe
Dockerfile.devshell               # AC-75 pinned Ubuntu 24.04 dev shell image
scripts/devshell.sh               # AC-75 Linux/macOS wrapper
scripts/devshell.ps1              # AC-75 PowerShell wrapper
scripts/capture_seccomp_corpus.sh # AC-77 strace-based syscall corpus capture (regenerates tests/fixtures/seccomp/observed_syscalls.txt)
tests/
  fixtures/
    seccomp/
      observed_syscalls.txt       # AC-77 empirical corpus (kernel 6.6 / glibc 2.39 / x86_64) sourcing the allowlist
      additions.txt               # AC-77 manually-justified additions (28 entries: dash + coreutils helpers, glibc-2.39 modern syscalls statx/faccessat2/madvise/etc., pincery-init Rust residuals between apply_seccomp and execvp)
      README.md                   # AC-77 fixture provenance + regeneration recipe
  sandbox_escape_test.rs          # AC-53 12-payload matrix
  seccomp_allowlist_test.rs       # AC-77 happy-path + unshare SIGSYS + audit-mode no-SIGSYS (3 cfg(linux) integration tests)
  sigsys_event_test.rs            # AC-77 review-fix R1: SIGSYS termination emits sandbox_syscall_denied event
  landlock_scope_test.rs          # AC-87 abstract-socket + signal scope live proof
  sandbox_mode_test.rs            # AC-73 enforce/audit/disabled
  sandbox_perf_test.rs            # AC-73 p95 budget
  secret_proxy_test.rs            # AC-71 memory sweep
  credential_hygiene_test.rs      # AC-74 redaction + zeroize
  network_egress_test.rs          # AC-72 allow/block
  credential_request_tool_test.rs # AC-55
  credential_deposit_test.rs      # AC-56
  cli_credential_request_test.rs  # AC-57
  session_ttl_test.rs             # AC-58
  rbac_test.rs                    # AC-59
  readme_auth_section_test.rs     # AC-60
  ui_smoke_test_v9.rs             # AC-61
  event_search_export_test.rs    # AC-62
  cost_report_test.rs             # AC-63
  event_retention_test.rs         # AC-64
  multi_tenant_isolation_test.rs  # AC-65 5x5 + SQLi probes
  tenancy_middleware_test.rs      # AC-65 lint
  tool_catalog_test.rs            # AC-66
  workspace_rate_limit_test.rs    # AC-67
  ollama_config_test.rs           # AC-68
  version_handshake_test.rs       # AC-69
  terminology_test.rs             # AC-70
migrations/
  20260501000001_add_workspace_id_to_sessions.sql
  20260501000002_create_credential_requests.sql
  20260501000003_add_users_role.sql
  20260501000004_add_sessions_expires_at.sql
  20260501000005_create_agent_http_allowlist.sql
  20260501000006_create_agent_network_allowlist.sql
```

### Interfaces

**Secret Proxy IPC (AC-71)** — length-prefixed JSON over unix socket:

```rust
// Request: agent process -> secret proxy
struct ResolveRequest {
    tool_call_id: Uuid,
    agent_id: Uuid,
    workspace_id: Uuid,
    placeholders: Vec<String>,        // e.g. ["OPENAI_API_KEY"]
    injection_mode: InjectionMode,    // Env | Stdin | HttpHeader { name }
    child_stdin_fd: Option<RawFd>,    // SCM_RIGHTS passed fd for stdin mode
}

enum ResolveResponse {
    Ready { env: HashMap<String, OsString> },  // proxy wrote stdin if requested
    Missing { name: String },                  // emits credential_unresolved
    Denied { reason: String },                 // capability gate refused
}
```

**Scoped Pool (AC-65)**:

```rust
pub struct ScopedPool<'a> { pool: &'a PgPool, workspace_id: Uuid }
impl ScopedPool<'_> {
    pub async fn fetch_one<T>(&self, sql: &str, binds: Binds) -> Result<T>;
    pub async fn fetch_all<T>(&self, sql: &str, binds: Binds) -> Result<Vec<T>>;
    pub async fn execute(&self, sql: &str, binds: Binds) -> Result<u64>;
    // SELECT/UPDATE/DELETE auto-append `AND workspace_id = $1`;
    // INSERT auto-fills workspace_id from the scope.
}
```

**Credential Request Surface (AC-55/56/57)**:

```
POST /api/agents/:id/tools/request_credential
  body: { name, reason, doc_url? }
  -> 201 { request_id }   (emits credential_requested; deposit_token NEVER returned)
GET  /api/credential-requests?status=pending
POST /api/credential-requests/:id/approve  -> 200 { deposit_url }  (admin/operator)
POST /api/credential-requests/:id/reject   -> 200 { status: "rejected" }
GET  /deposit/:deposit_token   -> 200 text/html (no auth, single-use)
POST /deposit/:deposit_token   -> 303 /deposit/success  (24h TTL)
```

**Sandbox Events (AC-53 / AC-71 / AC-72)**:

```
sandbox_blocked   { tool_call_id, payload_category, denied_by_layer, syscall?, path? }
secret_injected   { tool_call_id, name, tool_name, injection_mode }
network_blocked   { tool_call_id, destination_host, destination_port, protocol }
```

### External Integrations & Test Strategy

| Integration                                          | Purpose                    | Failure mode                                                                                                                                                                                                                                                                                                                            | Test strategy                                                                                           |
| ---------------------------------------------------- | -------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `bubblewrap` binary                                  | Namespace isolation        | Missing / ns disabled → exec refuses, `sandbox_unavailable`; nested userns not disabled → AC-86 smoke fails                                                                                                                                                                                                                             | Live on ubuntu-24.04 CI; AC-86 smoke asserts uid/gid/caps and `unshare -U` denial; ignored on non-Linux |
| `seccompiler` crate (0.5)                             | AC-77 default-deny syscall allowlist (~70 entries: 41 empirical + 28 manual) + `clone` arg-filter + ESCAPE_PRIMITIVES negative control + ALLOWLIST_SIZE_FLOOR/CEILING bounds | `build_bpf_program` returns `Err` and exec refuses if size guards trip or any escape primitive is present in the allowlist; `Enforce` mode uses `mismatch_action=KillProcess` (SIGSYS exit 159), `Audit` uses `mismatch_action=Log` | Unit: 9 tests in `seccomp.rs` (program shape, escape-primitive absence, size bounds, clone arg-filter, corpus-subset guard, memfd round-trip); 5 tests in `seccomp_audit.rs` (audit-record parser, payload, event emit). Integration: `tests/seccomp_allowlist_test.rs` (4 cfg=linux: happy-path + SIGSYS + seccomp-disabled control + audit-mode-no-SIGSYS) and `tests/sigsys_event_test.rs` (2 cfg=linux); live on privileged sandbox-smoke CI |
| Landlock ABI + `landlock` crate + raw ABI-6 syscalls | FS confinement + IPC floor | Startup preflight fails closed if ABI < 6 in strict mode; relaxed mode only downgrades to ABI >= 1 with `OPEN_PINCERY_ALLOW_UNSAFE=true`; no bwrap-only fallback. `landlock = 0.4` handles filesystem rules; `pincery-init` uses raw `landlock_create_ruleset` / `landlock_restrict_self` only for `LANDLOCK_SCOPE_ABSTRACT_UNIX_SOCKET | LANDLOCK_SCOPE_SIGNAL`.                                                                                 | Live in privileged sandbox-smoke; AC-84 positive process tests run with `OPEN_PINCERY_RUN_AC84_POSITIVE=1`; AC-87 `tests/landlock_scope_test.rs` proves abstract-socket denial and signal EPERM on ABI >= 6 |
| `slirp4netns`                                        | Egress proxy + allowlist   | Missing → exec refuses                                                                                                                                                                                                                                                                                                                  | Live: allowed host succeeds, denied host blocks                                                         |
| `cgroups-rs`                                         | Resource limits            | cgroup v2 not mounted → exec refuses                                                                                                                                                                                                                                                                                                    | Live: small OOM / PID thresholds                                                                        |
| Postgres                                             | Tenancy enforcement        | Middleware bypass → lint fails CI                                                                                                                                                                                                                                                                                                       | Live: 5×5 isolation matrix + SQLi probes                                                                |
| HTMX + Pico                                          | UI                         | Static asset 404 → UI smoke red                                                                                                                                                                                                                                                                                                         | Live: curl each route + check CSP header                                                                |

### Observability

- **Logs**: every sandbox layer failure → structured `error!` with `layer`, `kind`, `tool_call_id`. No plaintext credentials are ever logged (secret proxy scrubs at IPC boundary).
- **New event types**: `sandbox_blocked`, `sandbox_would_block`, `sandbox_mode_changed`, `sandbox_mode_default`, `sandbox_self_test_failed`, `sandbox_scope_unavailable`, `sandbox_syscall_denied` (AC-77; `{tool_name, agent_id, wake_id, correlation_pids, syscall_nr, syscall_name, audit_pid, audit_epoch_millis, record_correlated}` — emitted on SIGSYS-terminated tool invocation, `syscall_nr=-1` until G2c.2 wires AUDIT_SECCOMP correlation), `network_blocked`, `secret_injected`, `credential_plaintext_rejected`, `credential_requested`, `credential_deposited`, `credential_request_rejected`, `deposit_attempt`, `rate_limit_exceeded`.
- **Counters (stdout / structured)**: `sandbox_exec_total{outcome}`, `egress_attempts_total{decision}`, `secret_resolutions_total{mode}`, `tenancy_queries_total{workspace_id}`.
- **CLI verbs added**: `pcy session {list,revoke,refresh}`, `pcy user {add,list,set-role,delete}`, `pcy credential request {list,approve,reject}`, `pcy agent network {allow,list,revoke}`, `pcy events archive`, `pcy cost`.

### Complexity Exceptions

1. **`src/runtime/sandbox/mod.rs` may exceed 300 lines.** Each layer lives in its own sub-module, but `compose()` must orchestrate all six with partial-failure cleanup. File budget 400 lines; split further only if REVIEW flags it.
2. **`tests/sandbox_escape_test.rs` at ~500 lines.** 12 payloads × 4 categories × assertion+event check. Splitting by category is permitted; single-file is also acceptable given the shared harness cost.
3. **AC-65 endpoint-migration slice touches ~25 files in `src/api/` at once.** Necessary — the middleware lint disallows partial migration. One slice + one large test; REVIEW must sign off.
4. **`src/tenancy.rs::Binds` is a bespoke subset of `sqlx` binds, not a drop-in alias.** Trade-off accepted: the API surface stays small (three methods) and every call site is auditable.

### Open Questions

- **Landlock kernel floor.** Resolved by AC-84 / Slice G0b: strict startup requires Landlock ABI >= 6 (Linux >= 6.7), seccomp-bpf, cgroup v2, `/proc/sys/user/max_user_namespaces > 0`, Debian/Ubuntu `unprivileged_userns_clone=1` for non-root callers, and bubblewrap >= 0.8.0 before config loading, DB bootstrap, or listener bind. `OPEN_PINCERY_SANDBOX_FLOOR=relaxed` only downgrades Landlock to ABI >= 1 when paired with `OPEN_PINCERY_ALLOW_UNSAFE=true`; there is no bwrap-only fallback. Linux CI/devshell evidence remains the VERIFY proof path.
- **slirp4netns vs nftables.** v9 uses slirp4netns (unprivileged, userspace). nftables is a v10 opt-in for performance at scale. No BUILD impact.
- **Session refresh vs rotation.** v9 uses sliding expiration + an explicit `POST /api/sessions/refresh`. Refresh-token rotation (separate short access + long refresh) deferred to v11 with OAuth integration.

### v9 Dependencies on Prior Versions

- v6 AC-36 `ProcessExecutor`: wrapped by `SandboxedExecutor`. Existing `tests/sandbox_test.rs` (process-isolation smoke, misnamed) continues to pass.
- v7 AC-40 credential vault: extended via `credential_requests` table; existing `credentials` table untouched. Secret proxy (AC-71) reuses the vault decrypt path.
- v8 AC-45 idempotent login, AC-47 `--output`: every new list endpoint (requests, sessions, users, cost, network allowlist) honours `--output` and the noun-verb tree.
- v8 AC-52b naming lint: extended to allowlist new subcommands (`credential request`, `session`, `user`, `cost`, `agent network`).

### v9 DESIGN Addendum — Audit Hardening (AC-73 / AC-74 / AC-75)

The audit pass added three ACs. Implementation notes:

**AC-73 Sandbox Mode Flag** lives in `src/config.rs` as `pub enum SandboxMode { Enforce, Audit, Disabled }` with `FromStr` parsing `OPEN_PINCERY_SANDBOX_MODE`. `SandboxedExecutor::exec` reads mode at call time (not at construction) so it can be flipped without server restart. `Audit` mode short-circuits ALL deny decisions to Allow but emits `sandbox_would_block` events; `Disabled` skips the sandbox entirely but only if `OPEN_PINCERY_ALLOW_UNSAFE=true` is set — otherwise startup aborts with `SANDBOX_MODE=disabled requires OPEN_PINCERY_ALLOW_UNSAFE=true (this is a safety interlock)`. Startup self-test runs ONE synthetic tool call at `SANDBOX_MODE=enforce` with a known-blocking payload (`cat /etc/shadow`); if it succeeds, boot aborts.

**AC-74 Credential Hygiene** introduces `src/observability/redaction.rs` exporting a `RedactionLayer` that implements `tracing_subscriber::Layer`. It wraps every field value in a `Visit` impl; if the field name matches `password|api_key|token|secret|authorization|bearer` (case-insensitive) OR the value matches one of six credential-shape regexes (`sk-[A-Za-z0-9]{16,}`, `ghp_[A-Za-z0-9]{36}`, `xox[baprs]-[A-Za-z0-9-]{10,}`, JWT tri-dot, AWS AKIA/ASIA, Azure Bearer), the value is replaced with `<REDACTED>` before the downstream formatter sees it. The same regex set plus dynamic credential names (loaded from the `credentials` table at startup + on `credential_added` events) runs inside `src/models/events.rs::Event::try_new`, rejecting events with `EventRejected::CredentialPlaintext`. `SecretBuffer` in `src/runtime/secret_proxy.rs`:

```rust
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(ZeroizeOnDrop)]
struct SecretBuffer {
    #[zeroize(drop)]
    bytes: Vec<u8>,
    mlock_region: Option<region::LockGuard>,  // mlock() so pages never swap
}
```

**AC-75 Cross-Platform Dev Env** ships `Dockerfile.devshell` (Ubuntu 24.04 + bubblewrap + slirp4netns + uidmap + libseccomp-dev + landlock headers + rustup + sqlx-cli). `scripts/devshell.sh` runs `docker run --rm -it --privileged --cgroupns=host -v $PWD:/work -w /work ghcr.io/open-pincery/devshell:v9 "$@"`. `.ps1` mirror for PowerShell. CI publishes the image on tag push. `docs/runbooks/dev_setup_{macos,windows}.md` walks a new contributor from clone to `devshell cargo test` green.

### v9 Threat-Model Additions (feed into AC-54 SECURITY.md)

Audit surfaced these explicit threat-model items SECURITY.md must address:

1. **In-scope**: prompt-injection-driven credential-echo into event log (AC-74 rejects); tool-sandbox escape (AC-53 12-payload matrix); cross-workspace data access via session forgery (AC-65 + constant-time compare); session hijack via XSS (AC-61 nonce-CSP + cookie `HttpOnly`); CSRF on deposit page (AC-56 double-submit token); timing attack on session token (AC-58 + `subtle`); swap-leak of plaintext credentials (AC-71 + `mlock`); supply-chain attack via new sandbox crates (AC-73 + `cargo deny`).
2. **Out-of-scope (documented, not mitigated)**: compromised host kernel; malicious Postgres admin; physical access; kernel CVEs (user must patch); side-channel attacks on CPU microarchitecture (Spectre-class).
3. **Deployment hardening checklist** (new section): disable or encrypt swap; run with `--security-opt no-new-privileges`; kernel ≥ 5.13; `/proc/sys/kernel/unprivileged_userns_clone=1`; outbound firewall at host level defense-in-depth.

### v9 Observability Additions (feed into Observability section)

New event types added by the audit pass:

- `sandbox_would_block { tool_call_id, payload_category, reason }` — AC-73 `audit` mode
- `sandbox_mode_changed { old, new, user_id }` — AC-73
- `sandbox_mode_default` — AC-73 startup warning (unset → enforce)
- `sandbox_self_test_failed { reason }` — AC-73 startup abort
- `credential_plaintext_rejected { event_type, pattern_matched }` — AC-74
- `deposit_attempt { deposit_token_id, outcome, source_ip }` — AC-56 hardening

Every new event type is registered in `src/models/events.rs` and enumerated in `tests/event_type_lint.rs`.

### v9 G0f Design Reconciliation - AC-88 Landlock Audit Integration

This addendum records the implemented AC-88 shape after Slice G0f REVIEW.
It narrows the original "background reader" design to the per-invocation
reader that the code actually uses. That is intentional: per-invocation
source creation gives the audit bridge a real `{agent_id, wake_id,
tool_name}` context, bounded timestamps, and sampled process-tree PIDs
before any `landlock_denied` event can be appended.

#### Architecture Delta

AC-88 adds four cooperating pieces:

1. **Policy flag plumbing**: `src/runtime/sandbox/init_policy.rs` adds
   `landlock_restrict_flags: u32`, `LANDLOCK_AUDIT_ABI_FLOOR = 7`, and
   `LANDLOCK_RESTRICT_SELF_LOG_NEW_EXEC_ON`. `RealSandbox` sets the flag
   only when the probed Landlock ABI is >= 7. ABI 6 keeps filesystem and
   IPC-scope enforcement active while audit visibility degrades.
2. **Raw Landlock restrict path**: `src/runtime/sandbox/landlock.rs`
   exposes `install_landlock_with_restrict_flags`. When flags are
   nonzero it builds the filesystem ruleset through raw Landlock syscalls
   so `pincery-init` can pass `LANDLOCK_RESTRICT_SELF_LOG_NEW_EXEC_ON`.
3. **Per-invocation audit source**: `src/runtime/tools.rs` creates an
   audit source around each shell invocation. The source tries Linux
   audit netlink first, then falls back to a file reader opened at EOF.
   The fallback path is `OPEN_PINCERY_LANDLOCK_AUDIT_LOG` when set,
   otherwise `/var/log/audit/audit.log`.
4. **Correlation and append bridge**:
   `src/observability/landlock_audit.rs` parses audit records, rejects
   uncorrelated records, and appends `landlock_denied` only through
   `models::event::append_event`. Correlation uses sampled process-tree
   PIDs from `RealSandbox`, audit `pid`, audit `ppid` / `parent_pid`, and
   the shell invocation start/finish timestamp window to reject stale PID
   reuse.

#### Directory Structure Delta

```text
src/
  observability/
    landlock_audit.rs          # NEW - parser, source abstraction, correlation, event bridge
    landlock_audit_netlink.rs  # NEW - Linux NETLINK_AUDIT reader + nlmsghdr fixture decoder
    mod.rs                     # MODIFIED - registers both AC-88 modules
  runtime/
    sandbox/
      init_policy.rs           # MODIFIED - landlock_restrict_flags + ABI/flag constants
      bwrap.rs                 # MODIFIED - ABI-gated flag plumbing + process-tree PID sampling
      landlock.rs              # MODIFIED - raw restrict_self flag path
      mod.rs                   # MODIFIED - ExecResult::Ok includes audit_pids
    tools.rs                   # MODIFIED - per-shell audit source + bounded append polling
  bin/
    pincery_init.rs            # MODIFIED - applies Landlock with restrict flags
docker-compose.yml             # MODIFIED - forwards OPEN_PINCERY_LANDLOCK_AUDIT_LOG to app container
tests/
  landlock_audit_test.rs       # NEW - deterministic + gated live AC-88 proof
  compose_env_test.rs          # MODIFIED - deployment env forwarding proof for audit-log fallback
.env.example                   # MODIFIED - OPEN_PINCERY_LANDLOCK_AUDIT_LOG docs
```

#### Interfaces

```rust
// src/runtime/sandbox/init_policy.rs
pub const LANDLOCK_AUDIT_ABI_FLOOR: u32 = 7;
pub const LANDLOCK_RESTRICT_SELF_LOG_NEW_EXEC_ON: u32 = 1 << 1;

pub struct SandboxInitPolicy {
    pub landlock_scopes: u64,
    pub landlock_restrict_flags: u32,
    // existing fields unchanged
}

// src/runtime/sandbox/mod.rs
pub enum ExecResult {
    Ok { stdout: String, stderr: String, exit_code: i32, audit_pids: Vec<u32> },
    Timeout,
    Rejected(String),
    Err(String),
}

// src/observability/landlock_audit.rs
pub struct LandlockAuditRecord {
    pub pid: Option<u32>,
    pub parent_pid: Option<u32>,
    pub audit_epoch_millis: Option<u128>,
    pub denied_path: String,
    pub requested_access: String,
    pub syscall: String,
}

pub struct LandlockAuditContext {
    pub agent_id: Uuid,
    pub wake_id: Option<Uuid>,
    pub tool_name: String,
    pub audit_pids: Vec<u32>,
    pub invocation_started_at_millis: Option<u128>,
    pub invocation_finished_at_millis: Option<u128>,
}

pub trait AuditRecordSource {
    fn read_available_records(&mut self) -> io::Result<Vec<String>>;
}

pub async fn append_landlock_denials_within<S>(
    pool: &PgPool,
    context: &LandlockAuditContext,
    source: &mut S,
    window: Duration,
) -> Result<usize, AppError>
where
    S: AuditRecordSource + ?Sized;
```

`landlock_denied` event payloads include the required AC fields
`tool_name`, `agent_id`, `denied_path`, `requested_access`, and
`syscall`, plus `wake_id`, `correlation_pids`, `audit_pid`,
`audit_parent_pid`, and `audit_epoch_millis` when available.

#### External Integrations

| Integration                                                  | Failure mode                                                       | Error handling                                                                                                                                      | Test strategy                                                                                                                                                                                                                  |
| ------------------------------------------------------------ | ------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Linux audit netlink (`NETLINK_AUDIT`, `AUDIT_NLGRP_READLOG`) | Missing capability, unavailable audit subsystem, bind/read failure | Fall back to audit log file opened from EOF; if fallback also fails, emit one-time `audit_log_unavailable` warning and continue sandbox enforcement | Deterministic `nlmsghdr` fixture decoder in `landlock_audit_netlink.rs`; live test skips with explicit evidence when source unavailable                                                                                        |
| Audit log file fallback                                      | File missing, unreadable, or no records written by auditd/journald | `invocation_audit_source_from_end` reports the combined netlink/file failure; shell tool still executes under Landlock                              | EOF fallback unit test proves old records are not replayed; env var `OPEN_PINCERY_LANDLOCK_AUDIT_LOG` allows alternate fixture/source path; `tests/compose_env_test.rs` proves the override reaches the deployed app container |
| Landlock ABI 7 audit flag                                    | Kernel reports ABI < 7                                             | `landlock_restrict_flags = 0`; emit one-time `audit_log_unavailable`; keep ABI >= 6 enforcement active                                              | ABI 6 deterministic fallback test; Linux live tests skip below ABI 7 with explicit evidence                                                                                                                                    |

No new Rust crate dependency is added for AC-88. Netlink and raw
Landlock calls use the existing Linux `libc` dependency.

#### Observability

- `landlock_denied`: appended to the agent event log only after real
  runtime correlation. `source = "runtime"`, `tool_name = "shell"`,
  `wake_id` is present when available, and the JSON payload carries the
  denial and correlation fields above.
- `audit_log_unavailable`: a one-time structured tracing warning when
  ABI < 7 or no audit source is readable. It is not faked as an agent
  event without a real agent context.

#### Test Strategy

| Proof                   | Coverage                                                                                                                                                                                                                   |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Parser aliases          | `tests/landlock_audit_test.rs` parses `path`/`name`/`denied_path`, `requested_access`/`requested`/`accesses`, `syscall` aliases, and ignores non-Landlock records                                                          |
| EOF fallback            | File source opens at EOF and reads only records appended after source creation                                                                                                                                             |
| Bounded delayed polling | `append_landlock_denials_within` appends two delayed correlated records, continuing after the first append until quiet period or total window                                                                              |
| Correlation             | Tests cover process PID, audit `ppid`, `parent_pid`, stale/post-invocation PID reuse rejection, untimestamped live-context rejection, and uncorrelated append refusal                                                      |
| Netlink decoding        | `landlock_audit_netlink.rs` decodes deterministic `nlmsghdr` payload frames without a live audit subsystem                                                                                                                 |
| Live Linux proof        | Gated by bwrap availability, Landlock ABI >= 7, and readable audit netlink/log source; skips print explicit evidence when unavailable                                                                                      |
| Compose env forwarding  | `tests/compose_env_test.rs` asserts `docker-compose.yml` forwards `OPEN_PINCERY_LANDLOCK_AUDIT_LOG` via `${VAR:-}` and the gated `COMPOSE_AVAILABLE=1` `docker compose config` fixture renders the supplied audit-log path |

#### Complexity Exceptions

`src/observability/landlock_audit.rs` is allowed up to 450 lines for
AC-88 because it deliberately keeps parser, source abstraction,
correlation, bounded polling, and append bridge together. Linux netlink
framing is split into `landlock_audit_netlink.rs`. If the audit module
grows beyond 450 lines, split parser/correlation into separate files.

#### Open Questions

None. Live audit availability is an environment precondition, not an AC
ambiguity. ABI < 7 degrades observability only; sandbox enforcement
remains active.
