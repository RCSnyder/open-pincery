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
├── docker-compose.yml          # Postgres for dev
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
│   └── 20260418000013_create_auth_audit.sql
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
  Response: 201 { id, name, status, created_at }

GET /api/agents
  Headers: Authorization: Bearer <session_token>
  Response: 200 [ { id, name, status, created_at } ]

GET /api/agents/:id
  Headers: Authorization: Bearer <session_token>
  Response: 200 { id, name, status, identity, work_list, wake_id, created_at }

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
