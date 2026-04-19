# Open Pincery — Technical Stack

## Language: Rust

### Why Rust

- **CAS lifecycle correctness** — The coarse database lifecycle (`agents.status`: `asleep → awake → maintenance → asleep`) is coordinated with a finer-grained Rust enum that matches the TLA+ runtime states. Race conditions caught at compile time.
- **Event sourcing** — Serde + strong types for the event log means malformed events are a compile error, not a runtime surprise.
- **Long-running platform** — No GC pauses, predictable memory, single binary deployment. This is a runtime that hosts many agents 24/7.
- **Async I/O** — Tokio is mature. The system is I/O bound (Postgres, LLM API calls), and Rust's async is excellent for that.
- **Compile-time checked SQL** — sqlx verifies queries against the actual database schema at compile time.

## Database: Postgres

### Architecture → Postgres Mapping

- **Append-only event log**: Regular table with `INSERT` only, indexed by agent + timestamp.
- **CAS lifecycle**: `UPDATE ... WHERE status = 'asleep' RETURNING *` — atomic compare-and-swap in one statement.
- **Projections (identity + work list)**: Plain TEXT columns holding free-form prose snapshots. Version history is additional rows keyed by version/timestamp rather than mutable documents.
- **Wake summaries**: Table with agent_id + wake_id + summary text. `ORDER BY created_at DESC LIMIT 20` for prompt assembly.
- **Inter-agent messaging**: `INSERT` into events + `LISTEN/NOTIFY` to wake the target agent's runtime. Native Postgres pub/sub.
- **Webhook dedup**: `INSERT ... ON CONFLICT DO NOTHING` on the SHA-256 hash column.
- **Event collapse**: Append-only compaction metadata or prompt-assembly projection. Raw events stay `INSERT`-only and summaries never delete history.
- **Timer scheduling**: `pg_cron` or a simple `timers` table polled by the runtime.
- **Stale wake recovery**: `UPDATE agents SET status = 'asleep' WHERE status = 'awake' AND wake_started_at < NOW() - INTERVAL '2 hours'`

### Why Postgres

- **`LISTEN/NOTIFY`** gives event-driven wake triggers for free. When a webhook, human message, or inter-agent message lands, the inserting process does `NOTIFY agent_<id>`, and the Rust runtime wakes the agent. No polling, no message broker, no SQS.
- **Atomic CAS** — `UPDATE ... WHERE status = 'asleep' RETURNING *` is the entire compare-and-swap. No DynamoDB conditional writes needed.
- **Extensions** — pg_cron for timers, pgvector if we ever need embeddings, JSONB for flexible projections. All under one roof.
- **Simplicity** — No DynamoDB, no S3, no Lambda, no AWS SDK. Just a Rust binary and a Postgres instance.

## Rust Crate Stack

- **axum**: Webhook ingress + HTTP API
- **tokio**: Async runtime
- **sqlx**: Compile-time checked Postgres queries
- **tokio-postgres**: LISTEN/NOTIFY for agent wake triggers
- **serde**: Event serialization
- **reqwest**: LLM API calls (OpenRouter/OpenAI compatible)
- **zerobox**: Per-tool sandbox execution and secret injection
