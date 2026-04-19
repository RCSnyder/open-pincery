# Open Pincery — Incorporating Best Practices from Agentic Pipeline Research

Source: [Sun & Staron, "Agentic Pipelines in Embedded Software Engineering: Emerging Practices and Challenges" (2026)](https://arxiv.org/pdf/2601.10220)

The paper identifies 11 practices and 14 challenges across three themes from a systematic study of agentic AI adoption in industrial embedded software engineering. Below, each practice is mapped to Open Pincery's architecture with concrete implementation guidance.

---

## Theme 1: Orchestrated AI Workflow

### Practice 1: AI-Friendly Artifacts (AICP)

**Paper finding:** Teams create "AI-Friendly Code Protocol" documents — structured descriptions of repository layout, conventions, and constraints that agents consume before acting. Examples: `agent.md`, `CLAUDE.md`, `.cursor/rules`, `copilot-instructions.md`.

**Open Pincery implementation:**

Each agent's **identity projection** already serves this role (the "prose" document the agent maintains about itself). Extend this to a first-class `agent_context` artifact:

```sql
CREATE TABLE agent_context (
    agent_id   UUID REFERENCES agents(id),
    key        TEXT NOT NULL,        -- 'identity', 'conventions', 'constraints', 'repo_map'
    content    TEXT NOT NULL,
    version    INT NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (agent_id, key)
);
```

- **Identity** — who the agent is, its purpose, personality (already in the architecture)
- **Conventions** — coding style, naming patterns, architectural constraints
- **Constraints** — what the agent must NOT do, safety boundaries
- **Repo map** — structured description of the codebase the agent operates on

These artifacts are versioned and included in **prompt assembly** (the two-tier memory system). The agent can update them via the maintenance cycle.

### Practice 2: Compiler-in-the-Loop Feedback

**Paper finding:** Agents that receive compiler/linter feedback in their agentic loop produce significantly better code. The pattern: generate → compile → feed errors back → regenerate.

**Open Pincery implementation:**

This maps directly to the **wake loop** with tool execution. The architecture already supports the pattern; implementing Practice 2 means adding built-in verification tools such as:

```text
Wake Loop Iteration:
  1. LLM generates code (tool call: write_file)
  2. LLM calls verify tool (tool call: compile/lint/test)
  3. Tool result contains errors → injected as mid-wake event
  4. LLM sees errors in next iteration → fixes
  5. Repeat until clean or iteration cap hit
```

In the TLA+ state machine this pattern already fits naturally — `ToolExecuting → ToolResultProcessing → Awake` loops without any new lifecycle states. The remaining implementation work is to provide **built-in verification tools** that agents can call:

| Tool              | Purpose                                       |
| ----------------- | --------------------------------------------- |
| `compile`         | Run language-specific compiler, return errors |
| `lint`            | Run linter, return warnings/errors            |
| `test`            | Run test suite, return failures               |
| `typecheck`       | Run type checker (tsc, mypy, etc.)            |
| `validate_schema` | Validate output against a JSON schema         |

These tools run inside Zerobox sandboxes with read-only filesystem access + the specific build directory.

### Practice 3: Prompt Management as Engineering Discipline

**Paper finding:** Prompts should be version-controlled, reviewed, tested, and treated as first-class engineering artifacts — not inline strings.

**Open Pincery implementation:**

Store prompts in Postgres as immutable, versioned templates:

```sql
CREATE TABLE prompt_templates (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name          TEXT NOT NULL,         -- 'wake_system_prompt', 'maintenance_prompt', 'tool_dispatch'
    version       INT NOT NULL,
    template      TEXT NOT NULL,         -- Handlebars/Tera template with {{variables}}
    is_active     BOOLEAN NOT NULL DEFAULT FALSE,
    created_by    UUID REFERENCES users(id),
    created_at    TIMESTAMPTZ DEFAULT NOW(),
    change_reason TEXT,
    UNIQUE (name, version)
);

CREATE UNIQUE INDEX prompt_templates_one_active_per_name
    ON prompt_templates (name)
    WHERE is_active = TRUE;
```

The table itself is the version history. Each new prompt revision is a new immutable row, which keeps the schema aligned with the TLA's `ORDER BY version DESC` lookup and avoids a second history table drifting out of sync.

**Benefits:**

- Prompts are reviewable, diffable, and auditable
- Roll forward or back by activating a different immutable version row
- LLM-call provenance can record the exact `name + version` pair used at runtime
- Prompt injection defenses can be standardized in templates (delimiters, canary tokens)

### Practice 4: Tool Standardization through MCP

**Paper finding:** Model Context Protocol (MCP) is emerging as the standard for tool integration. Teams that adopt MCP get consistent tool discovery, schema validation, and composability.

**Open Pincery implementation:**

Support MCP as a first-class tool provider:

```text
Agent's available tools:
  ├── Built-in tools (shell, file read/write, HTTP)
  ├── Custom tools (defined per-agent in agent_context)
  └── MCP servers (discovered via MCP protocol)
```

The runtime acts as an MCP client. When an agent's configuration references an MCP server, the runtime:

1. Connects to the MCP server
2. Discovers available tools via `tools/list`
3. Exposes them to the LLM in the wake prompt
4. Routes tool calls through the MCP protocol
5. Each MCP tool call still runs through the security pipeline (prompt injection scan on tool output, credential isolation)

This aligns with Zerobox and OneCLI — MCP tool calls go through the credential vault, and any local execution happens inside a sandbox.

Configured MCP endpoints belong in a workspace-scoped registry. If Open Pincery later ships a platform-global catalog of curated connectors, that catalog should live in separate runtime-owned metadata rather than sharing the tenant-visible registry namespace.

---

## Theme 2: Responsible AI Governance

### Practice 5: Human-in-the-Loop Supervision

**Paper finding:** Critical operations require human approval. Teams implement approval gates for: deployments, database modifications, security-sensitive operations, and irreversible actions.

**Open Pincery implementation:**

The architecture already has **"explicit sleep"** (agent decides to sleep and wait). Extend this to a **human approval gate**:

```sql
CREATE TABLE approval_requests (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id          UUID NOT NULL REFERENCES agents(id),
    wake_id           UUID NOT NULL,
    tool_name         TEXT NOT NULL,
    tool_input        JSONB NOT NULL,
    category          TEXT NOT NULL,
    status            TEXT NOT NULL DEFAULT 'pending'
                      CHECK (status IN ('pending', 'approved', 'rejected', 'expired')),
    requested_at      TIMESTAMPTZ DEFAULT NOW(),
    expires_at        TIMESTAMPTZ,
    resolved_at       TIMESTAMPTZ,
    resolved_by       UUID REFERENCES users(id),
    resolution_reason TEXT
);
```

The approval row is the canonical machine-readable request. Human-readable action descriptions belong in derived UI views, not in the authoritative row shape.

**Flow:**

1. Agent's tool call matches a "requires approval" pattern (configured per-agent)
2. Runtime creates an approval request and transitions agent to `AwaitingApproval` state
3. Human reviews via API/dashboard and approves or rejects
4. Approval creates a wake event → agent resumes with the approved tool call
5. Rejection creates a wake event → agent sees rejection reason and adjusts

**Configurable approval patterns:**

- `git push` / `git commit` — code deployment
- `git push` to protected branches / PR merge / release — code promotion
- `DROP TABLE` / `DELETE FROM` — destructive database operations
- `curl` to new domains not in allowlist — network access expansion
- `sudo` / privilege escalation
- Any tool call costing more than $X (LLM token budget)

### Practice 6: Traceability and Auditability

**Paper finding:** Every AI-generated change must be traceable to the prompt, model, and context that produced it. Teams need to know: what was the input? What model generated this? What was the full context?

**Open Pincery implementation:**

This is inherent in the event-sourced architecture. Every event in the log already captures:

- Timestamp
- Agent ID
- Event type (tool call, LLM response, human message, webhook, etc.)
- Full content

Enhance with explicit provenance:

```sql
CREATE TABLE llm_calls (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id        UUID NOT NULL REFERENCES agents(id),
    wake_id         UUID NOT NULL,
    call_type       TEXT NOT NULL,       -- wake_reasoning, maintenance, prompt_injection_scan
    model           TEXT NOT NULL,       -- 'gpt-4o', 'claude-sonnet-4-20250514', etc.
    prompt_hash     TEXT NOT NULL,       -- SHA-256 of the full assembled prompt
    prompt_template TEXT,                -- which template version was used
    prompt_tokens   INT,
    completion_tokens INT,
    total_tokens    INT,
    cost_usd        NUMERIC(10, 6),
    latency_ms      INT,
    response_hash   TEXT NOT NULL,       -- SHA-256 of the full response
    finish_reason   TEXT,
    temperature     FLOAT,
    created_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE llm_call_prompts (
    llm_call_id   UUID PRIMARY KEY REFERENCES llm_calls(id),
    system_prompt TEXT NOT NULL,
    messages_json JSONB NOT NULL,
    tools_json    JSONB,
    response_text TEXT NOT NULL
);
```

Combined with the event log, this gives full traceability: for any agent action, you can reconstruct the exact prompt, model, and context that produced it.

### Practice 7: Cost and Resource Governance

**Paper finding:** Agentic pipelines can be expensive. Teams need cost visibility, budget caps, and the ability to kill runaway agents.

**Open Pincery implementation:**

The architecture already has `IterationCapHit` and `ContextCapHit` states. Extend with explicit budget tracking:

```sql
-- Per-agent budget
ALTER TABLE agents ADD COLUMN budget_limit_usd NUMERIC(12, 6) NOT NULL DEFAULT 10.000000;
ALTER TABLE agents ADD COLUMN budget_used_usd  NUMERIC(12, 6) NOT NULL DEFAULT 0.000000 CHECK (budget_used_usd >= 0);
```

These columns are part of the canonical `agents` schema because the wake loop reads and mutates them directly; they are not just reporting fields.

**Budget enforcement in the wake loop:**

1. After each LLM call, compute cost from token counts × model pricing
2. Increment `budget_used_usd`
3. If `budget_used_usd >= budget_limit_usd`, terminate wake with `BudgetExhausted` event
4. Dashboard shows cost per agent, per wake, per day

**Kill switch:**

```sql
-- Administrative suspension is a control-plane flag, not a new lifecycle state
UPDATE agents
SET is_enabled = FALSE,
    disabled_reason = 'manual kill',
    disabled_at = NOW()
WHERE id = $1;
-- Workers refuse new wake acquisition while is_enabled = FALSE.
-- If the agent is already awake, let the runtime stop new tool dispatch,
-- record the suspension, and release back through maintenance -> asleep.
```

### Practice 8: Responsible Output Handling

**Paper finding:** AI-generated code should be clearly marked as AI-generated. Teams use commit metadata, comments, or labels to distinguish human vs AI work.

**Open Pincery implementation:**

Every event in the log already has a `source` field. For agents that produce code:

- Git commits include `Co-authored-by: Open Pincery Agent <agent-id@open-pincery>` trailer
- File modifications include metadata: `// Generated by agent <name> in wake <wake_id>`
- The event log explicitly records whether content was human-authored or AI-generated

This is important for downstream compliance. The audit trail makes it trivially provable which changes were AI-generated.

---

## Theme 3: Sustainable AI Adoption

### Practice 9: Incremental Adoption / Gradual Trust

**Paper finding:** Successful teams don't give agents full autonomy on day one. They start with read-only access, then add write access, then add deployment access as trust is established.

**Open Pincery implementation:**

Agent capability levels, stored as configuration:

```sql
CREATE TABLE agent_capabilities (
    agent_id    UUID REFERENCES agents(id),
    capability  TEXT NOT NULL,        -- 'read_files', 'write_files', 'execute_shell',
                                     -- 'network_access', 'deploy', 'create_agents'
    granted     BOOLEAN DEFAULT FALSE,
    granted_at  TIMESTAMPTZ,
    granted_by  UUID REFERENCES users(id),
    PRIMARY KEY (agent_id, capability)
);
```

**Capability tiers:**

| Tier       | Capabilities                             | Use Case                            |
| ---------- | ---------------------------------------- | ----------------------------------- |
| Observer   | Read files, read events                  | Monitoring, analysis                |
| Worker     | + Write files, execute shell (sandboxed) | Code generation, data processing    |
| Operator   | + Network access (scoped domains)        | API integration, deployment         |
| Autonomous | + Create agents, inter-agent messaging   | Self-organizing multi-agent systems |

New agents start at Observer. Promotion requires explicit human action. Zerobox sandbox profiles are derived from the capability tier.

### Practice 10: Feedback Loops and Continuous Improvement

**Paper finding:** Teams that systematically collect feedback on agent outputs and use it to improve prompts/tools see compounding quality gains.

**Open Pincery implementation:**

The **maintenance cycle** already exists — the single LLM call where the agent reflects on the completed wake and updates its projections. Extend this to include structured self-assessment:

```json
{
  "wake_id": "...",
  "tasks_attempted": 3,
  "tasks_completed": 2,
  "tasks_failed": 1,
  "failure_reasons": ["test suite failed after 3 retries"],
  "tools_used": ["write_file", "compile", "test"],
  "tokens_consumed": 15234,
  "self_rating": 0.7,
  "improvement_notes": "Need to run tests before committing next time"
}
```

This structured feedback is stored in the event log and can be:

- Aggregated in dashboards for human review
- Fed into future prompts ("In your last wake, you noted: ...")
- Used to adjust capability tiers (repeated failures → lower trust)

### Practice 11: Knowledge Sharing Across Agents

**Paper finding:** Information learned by one agent should be available to others. Teams build shared knowledge bases and cross-agent learning mechanisms.

**Open Pincery implementation:**

Inter-agent messaging is already in the architecture. Add a **shared knowledge store**:

```sql
CREATE TABLE shared_knowledge (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id   UUID NOT NULL REFERENCES workspaces(id),
    namespace      TEXT NOT NULL,        -- 'codebase', 'api_patterns', 'failure_modes'
    key            TEXT NOT NULL,
    content        TEXT NOT NULL,
    contributed_by UUID REFERENCES agents(id),
    confidence     FLOAT DEFAULT 1.0,
    created_at     TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(workspace_id, namespace, key)
);
```

During maintenance, agents can publish discoveries. During prompt assembly, agents can query relevant shared knowledge. This is the "prose projection" concept extended to collective intelligence. The table is workspace-scoped by default so it matches the shared-table + RLS tenant model in the security architecture. Cross-workspace sharing, if desired later, should be an explicit publication flow rather than a global namespace.

---

## Challenges and Mitigations

The paper identifies 14 challenges. Key ones relevant to Open Pincery:

| Challenge                  | Paper Description                                         | Open Pincery Mitigation                                              |
| -------------------------- | --------------------------------------------------------- | -------------------------------------------------------------------- |
| **Hallucination**          | Agents generate plausible but incorrect code/actions      | Compiler-in-the-loop verification, test execution, output rails      |
| **Context window limits**  | Important context falls out of the window                 | Event collapse (backpressure), two-tier memory, wake summaries       |
| **Non-determinism**        | Same input produces different outputs across runs         | LLM call logging with full prompts (reproducibility via replay)      |
| **Debugging difficulty**   | Hard to understand why an agent did something             | Full event log, LLM call traceability, prompt reconstruction         |
| **Security risks**         | Prompt injection, credential leaks, unauthorized actions  | 6-layer security architecture (see security-architecture.md)         |
| **Cost unpredictability**  | Token costs can spike unexpectedly                        | Budget caps, per-wake cost tracking, model-specific pricing tables   |
| **Tool reliability**       | External tools/APIs can fail or return unexpected results | Retry with backoff, tool output validation, fallback strategies      |
| **Integration complexity** | Connecting to diverse tools and services                  | MCP standardization, typed tool schemas, Zerobox-sandboxed execution |

---

## Implementation Priority

### Already Specified In The Architecture

- ✅ Event sourcing (traceability)
- ✅ CAS lifecycle (state machine correctness)
- ✅ Identity/work list projections (AI-friendly artifacts)
- ✅ Maintenance cycle (feedback loops)
- ✅ Inter-agent messaging (knowledge sharing)
- ✅ Wake summaries (context management)
- ✅ Event collapse (backpressure)
- ✅ Iteration/context caps (resource governance)

These items are specified in the architecture and source-of-truth docs. They are not all implemented in code yet.

### Phase 1 — Quick Wins

1. Prompt template versioning (Practice 3)
2. LLM call logging with provenance (Practice 6)
3. Agent capability tiers (Practice 9)
4. Budget tracking (Practice 7)

### Phase 2 — Tool Ecosystem

1. Built-in verification tools: compile, lint, test (Practice 2)
2. MCP client integration (Practice 4)
3. Human approval gates (Practice 5)

### Phase 3 — Intelligence

1. Structured self-assessment in maintenance (Practice 10)
2. Shared knowledge store (Practice 11)
3. AI-generated content labeling (Practice 8)
