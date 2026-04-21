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
      bootstrap.rs           # NEW — pcy bootstrap
      login.rs               # NEW — pcy login --token ...
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
| `scripts/smoke.sh`                | Bash: `compose up --wait` → poll `/ready` → `pcy bootstrap/login/agent create/message` → `pcy events` → assert `message_received`         |
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

deny.toml (v6 AC-37): vulnerability = "deny", ignore = [].
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
    mod.rs             — MODIFIED: AppState gains executor: Arc<dyn ToolExecutor>
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
deny.toml                    — MODIFIED: vulnerability = "deny", ignore = []
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

| mode \ cap  | ReadLocal | WriteLocal | ExecuteLocal | Network | Destructive |
| ----------- | :-------: | :--------: | :----------: | :-----: | :---------: |
| Yolo        |     ✓     |     ✓      |      ✓       |    ✓    |      ✓      |
| Supervised  |     ✓     |     ✓      |      ✓       |    ✓    |      ✗      |
| Locked      |     ✓     |     ✗      |      ✗       |    ✗    |      ✗      |

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
1. If `cmd.command.trim_start().starts_with("sudo")` (word-boundary match) → `ExecResult::Rejected("sudo is not permitted")` (no process spawned).
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
version = 2
vulnerability = "deny"   # v6: was implicit/high-critical only
unmaintained = "warn"
yanked = "deny"          # v6: was "warn"
ignore = []              # v6: empty, was allowed to grow
```

`tests/deny_config_test.rs` parses `deny.toml` with a small `toml` dev-dep (add `toml = "0.8"` under `[dev-dependencies]` if not already present) and asserts the four fields above.

### External Integrations

None added. `ProcessExecutor` is a local-only executor; the only external call remains the existing LLM egress (unchanged). Zerobox, vault, proxy — all deferred to v7/v8/v9.

### Test Strategy

| AC    | Test file                         | Kind           | Notes                                                                                                                                       |
| ----- | --------------------------------- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| AC-34 | `tests/agent_status_test.rs`      | Unit           | Round-trip all 5 variants through `from_db_str` / `as_db_str`; `from_db_str("bogus")` → Err                                                 |
| AC-34 | `tests/no_raw_status_literals.rs` | Static / grep  | Reads `src/**/*.rs` at test time, regex over `status\s*(=|IN)\s*['\(]`; allowlists the constant-definition block in `src/models/agent.rs`    |
| AC-35 | `tests/capability_gate_test.rs`   | Unit + integ   | Table-driven: 15 `(mode, cap)` rows against `mode_allows`; integration creates a `Locked` agent, wakes via wiremock-served `shell` tool call, asserts one `tool_capability_denied` event + zero `tool_result` + a `CountingExecutor::spawns() == 0` |
| AC-36 | `tests/sandbox_test.rs`           | Unit           | (a) set `HOME=/tmp/fake`, `MY_SECRET=leak`; run `printenv` via `ProcessExecutor` with allowlist `["PATH"]`; assert neither name appears in stdout. (b) `sleep 60` with `timeout = 1s` → `ExecResult::Timeout`. (c) command beginning with `sudo` → `ExecResult::Rejected` without spawn |
| AC-36 | `tests/no_raw_command_new.rs`     | Static / grep  | Regex `Command::new\(` across `src/runtime/**` — exactly one match, inside `sandbox.rs`                                                     |
| AC-37 | `tests/deny_config_test.rs`       | Unit           | Parse `deny.toml`; assert `[advisories].vulnerability == "deny"` and `ignore == []`                                                         |
| AC-37 | CI `cargo deny check advisories`  | CI gate        | Already wired by v3 AC-16; must exit 0 on v6 HEAD                                                                                           |

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
