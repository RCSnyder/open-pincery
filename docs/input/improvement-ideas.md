# Improvement Ideas — Input Doc

> **Status:** Brainstorming. This is an input doc, not a scope or design. Ideas here are candidates for future EXPAND phases, not commitments.
>
> **Sources mixed in:**
>
> 1. Operator questions accumulated during first real use of the v4 runtime.
> 2. An industry essay arguing that today's "production agents" suffer from governance debt, excessive agency, and ephemeral sessions — and that the fix has to happen at the platform layer, not the prompt layer.
> 3. Cloudflare's Agents SDK + "Project Think" announcement, which proposes a concrete set of primitives: per-agent Durable Objects, fibers (durable execution), sub-agents, a tree-structured session API, sandboxed code execution, an execution ladder (workspace → isolate → npm → browser → sandbox), and self-authored extensions.
>
> **Purpose:** before any of these become scope, decide which are _aligned with the continuous-agent architecture_, which are _tangential_, and which _explicitly contradict_ the current design. Keep the conflicts visible instead of silently absorbing them.

---

## Part 1 — Open operator questions

Raw questions from first real use of the stack. Each one is either a doc gap, a test gap, or a real design gap.

### Q1. "Does this thing even work?"

- First-run confidence is low. The v4 runtime ships, tests pass, but the operator has no smoke-test path that proves _"I can talk to an agent and it actually responds intelligently"_ in under 60 seconds after `docker compose up`.
- Gap class: **validation / UX**, not architecture.
- Candidate fix: a `pcy demo` command that bootstraps, creates a canned agent with a known prompt, sends a message, polls events, and prints the agent's reply. Also: a UI "send a test message" button on first load.

### Q2. "How does the UI work?"

- The UI exists (`static/index.html`, `static/js/`) and is served from `/`, but there's no operator-facing walkthrough. Login panel, agent list, message view — the mental model isn't written down.
- Gap class: **documentation**, possibly also UX polish.
- Candidate fix: a short `docs/ui.md` with screenshots + state diagram of the UI's login/agent/message flow.

### Q3. "Can agents talk to other agents?"

- Today: yes, but only via `shell` + `curl` to the HTTP API. No first-class `send_message(agent, content)` tool.
- Gap class: **runtime capability gap** (and a security gap — the agent would need a session token in its environment, which we don't currently provision).
- See Part 2 for the design tension this creates.

### Q4. "How do the permissions work?"

- Today: one local admin, one workspace, one session token class. All agents share the same LLM API key. There is no per-agent capability boundary at the runtime level.
- Gap class: **real architecture gap**. The essay in Part 3 argues this is the single biggest thing that separates "demo" from "enterprise-ready." Cloudflare's answer is capability-per-binding on Dynamic Workers.
- See Part 3, "Agent identity."

### Q5. "Should rate limits respect the native LLM call?"

- Today: rate limits are IP-based, tracking HTTP requests to the Open Pincery API. They are **not** aware of downstream LLM cost, tokens, or provider rate limits.
- Real consequence: a single authenticated client can trigger 60 wakes/minute, each of which may burn thousands of tokens. The operator-facing budget cap (`LLM_PRICE_*_PER_MTOK`) catches this in dollars _after the fact_, not in requests _before the fact_.
- Candidate fix directions:
  - **LLM-call rate limiter** (tokens or calls per agent per minute), enforced in `runtime::llm` before the HTTP call.
  - **Provider-aware backoff** — respect `Retry-After` / 429 from the LLM provider and feed it back into the wake scheduler.
  - **Budget-driven throttle** — when an agent is ≥ X% of its budget, slow down wakes rather than hard-refusing at 100%.

### Q6. "Should messages go through a queue?"

- Today: messages append directly to the event log, then `pg_notify` wakes the runtime. There is no durable queue _between_ the producer and the wake loop — the event log **is** the queue.
- The question in the operator's head is really two questions:
  1. **Backpressure**: if 10k messages arrive in a burst, do we drop, queue, or wake 10k times? (Today: we CAS-collapse them into a single wake per agent. That's actually good.)
  2. **Fanout / scaling**: if there are 100k agents, can one runtime process handle the LISTEN/NOTIFY stream? (Today: unclear, not load-tested.)
- Gap class: **load-testing and docs gap**, not necessarily an architecture change. The CAS-collapse design already does the right thing semantically.

### Q7. "Can it build and host new stuff within the same server?"

- Today: no. The server is a fixed Rust binary. Agents cannot create or deploy new HTTP endpoints, new UI pages, or new background services.
- Project Think's answer: Dynamic Workers + self-authored extensions. Agents write TypeScript at runtime, declare permissions, and the platform runs the code in a sandboxed isolate.
- Gap class: **major architectural direction decision.** See Part 4.

### Q8. "Actor-model scaling for agent messages?"

- Today's model is already actor-ish: each agent has a durable identity, a private event log, and a CAS lifecycle ensuring exactly-one-wake. What we _don't_ have is a sharding/placement story — every wake runs in the same process against one Postgres.
- Natural question: at what agent count or message rate does this break, and what's the next step? (Multiple runtime replicas sharing the same DB, with wake-claim via SQL? Per-shard runtime? Durable-Object-style placement?)

---

## Part 2 — Agent-to-agent messaging: tension with the architecture

This deserves its own section because it exposes a real design tension, not just a missing tool.

### What the current architecture says

- Agents communicate by **message passing with no shared transcript** (`README.md`).
- The **shell tool is the universal tool** — the design principle is that we should avoid adding tools; agents should write programs.
- Agents are continuous, so "send a message" is a durable intent, not a function return.

### What the operator wants

- `send_message("agent-b", "please summarize the Q1 report")` as a one-liner, without having to teach the agent to curl the API, discover agent IDs, or manage tokens.

### Where the tension is

A first-class `send_message` tool conflicts with "shell is universal." But the shell path has real problems:

1. The agent needs a session token. Where does it come from? (Inherited from the creator? Issued per-agent? That's the identity question from Q4.)
2. The agent needs to know agent IDs or have a discovery call. (`list_agents` via shell works but is awkward.)
3. Authorization is ambient — the agent can message _any_ agent in the workspace. There's no capability narrowing.

### Possible resolutions (not mutually exclusive)

- **A. First-class tool, narrow.** Add `send_message(agent_name, content)` that uses the creator-agent's scoped token and respects a per-agent "may message" allowlist. Aligns with "managed workforce" framing from the essay.
- **B. Platform-mediated shell environment.** Keep shell-as-universal, but the runtime injects a _per-wake_ scoped token and a `pcy` binary into the agent's shell environment. The agent writes `pcy message agent-b "..."` as a program. Preserves the shell philosophy, adds ambient auth, still needs a capability story.
- **C. Named mailboxes + subscriptions.** Agents don't address each other directly — they write to named channels, and other agents subscribe. Matches actor-model intuition and the "no shared transcript" rule, but is a real conceptual addition.

Decision deferred — this is a scope question, not a patch.

---

## Part 3 — Ideas adopted from the governance-debt essay

The essay makes four bets. Mapped to Open Pincery:

### Bet 1: Agents need identities, not shared credentials

**Essay claim:** Agents today borrow service accounts or human OAuth tokens. Policy lives in prompts. In production, prompt-level policy is not policy.

**Open Pincery today:**

- Per-agent webhook secret exists (AC-24 rotation endpoint). Good start.
- But: all agents share one LLM API key, one Postgres role, one shell host, one network. No platform-level per-agent identity.

**Ideas worth considering:**

- Per-agent LLM API keys (or proxy tokens via a credential vault — matches the stated OneCLI direction).
- Per-agent Postgres role with row-level policy so an agent can only read its own events at the _database_ level, not just the application level.
- Per-agent network egress policy (deny-by-default, allow-list). Natural once we adopt a sandbox (zerobox is already in the security model).
- Per-agent session tokens for the agent itself when it talks to the HTTP API (solves half of Part 2).

**Alignment:** Strongly aligned with the existing six-layer security model. This is not a new direction — it's finishing what the security-architecture doc already outlines.

### Bet 2: Agents need universal context, not scraped windows

**Essay claim:** Teams burn hours on custom serialization, bespoke session stores, hand-rolled memory. Context should be a platform primitive.

**Open Pincery today:**

- Event log + projections (identity, work list) + wake summaries is already a universal-context story at the persistence layer.
- But: context is _per-agent_. There is no cross-agent context, no integration with external business systems (CRM, ticketing, etc.), and no semantic search over the event log.

**Ideas worth considering:**

- FTS over events (Postgres `tsvector` column, or FTS5-equivalent). Low cost, high value.
- A "context provider" interface — pluggable adapters that inject information from external systems into a wake prompt (read-only, capability-scoped).
- Cross-agent context blocks (shared projections that multiple agents can read but only one can write).

**Alignment:** Event-sourcing foundation makes this easy. Additions, not contradictions.

### Bet 3: Agents need to survive your laptop closing

**Essay claim:** A session that survives a dropped WebSocket is table stakes. A _mission_ that survives a quarter is the actual bar.

**Open Pincery today:**

- Continuous agents — the whole point of the architecture. An agent's identity, work list, and event log persist across restarts, redeploys, and token rotations by design.
- CAS lifecycle + append-only log = "work done Tuesday is auditable Friday."
- This is the single bet where Open Pincery is _already ahead_ of most of the market.

**Ideas worth considering:**

- **Durable execution within a single wake.** Today if the process dies mid-tool-call, the tool call is lost. Project Think's "fibers" solve exactly this — checkpoint at each tool boundary so crash-during-wake resumes cleanly.
- **Human-in-the-loop approval primitives.** An agent should be able to pause, emit a structured "I need approval to do X" event, and resume when the human approves via the API or UI. Today this is doable ad hoc but not first-class.
- **Explicit mission objects.** Today missions are implicit in the work list prose. A named, stable `mission_id` that threads across wakes and agents would make "mission that survives a quarter" auditable.

**Alignment:** This is Open Pincery's strongest axis. The improvements here are sharpening an existing strength, not changing direction.

### Bet 4: Agents need platforms, not plumbing

**Essay claim:** Brilliant engineers are draining bandwidth into stack problems (memory, eval, observability, retries) that don't differentiate their product.

**Open Pincery today:**

- This _is_ the mission. The value proposition of a continuous-agent platform is precisely that the operator doesn't rebuild memory, wake scheduling, event logging, or credential vaults.
- Open-source + self-host-first is the same "start local, graduate to managed" story the essay endorses.

**Alignment:** No new idea — this is the existing vision, restated. Useful as marketing language and as a litmus test: _every new feature should remove stack work from the operator, not add it._

---

## Part 4 — Ideas adopted from Cloudflare Agents / Project Think

Cloudflare's post is a concrete set of primitives. Mapped to Open Pincery:

### 4.1 Per-agent Durable Objects → per-agent runtime shards (maybe)

**Cloudflare:** Each agent is a Durable Object — its own SQLite, its own hibernating compute, automatic placement and routing. Zero cost when idle.

**Open Pincery:** Per-agent isolation exists at the _data_ level (agent_id-scoped queries) but not at the _compute_ level (one runtime process serves all agents). A single slow wake can starve others.

**Worth adopting:**

- **Per-agent concurrency isolation**, even without sharding. E.g., a per-agent tokio task or a bounded queue per agent, so one runaway wake doesn't block the fleet.
- **Hibernation accounting** — we already hibernate (asleep state), but we don't _bill_ or _measure_ idle. Adding per-agent compute-time metrics makes the "zero cost when idle" story legible.

**Not adopting (yet):**

- Full Durable-Object-style global placement. That's a Cloudflare-platform feature, not something we ship in a single Rust binary. But the _shape_ of the API (agent name → singleton actor with its own state) is already ours.

### 4.2 Fibers → durable execution within a wake

**Cloudflare:** A fiber is a durable function invocation. Registered in SQLite before execution. Checkpointable via `stash()`. Recoverable via `onFiberRecovered` on restart.

**Open Pincery:** A wake is durable at its boundaries (the log records WAKE_STARTED and WAKE_FINISHED) but not _within_. If the runtime dies between tool call #3 and tool call #4 in a wake, we lose the in-flight state.

**Worth adopting:**

- **Wake checkpoints.** After every tool call, persist the model's message history + tool results as a checkpoint event. On restart, if a wake is in STARTED with no FINISHED, replay from the last checkpoint instead of starting over.
- **Durable tool-call IDs.** Every tool call gets a persisted ID before execution, so retry-after-crash is idempotent.

**Alignment:** This is a clear, finite improvement to the existing wake loop. Low design risk, high operational value. Strong candidate for a near-term iteration.

### 4.3 Sub-agents / Facets → delegated work

**Cloudflare:** An agent can spawn typed sub-agents with their own isolated SQLite. Parent calls sub-agent via typed RPC. No shared storage.

**Open Pincery:** Agents are flat today. "Delegation" is done by message-passing between peer agents.

**Worth considering:**

- A parent/child relationship in the agent schema (`parent_agent_id nullable`), with an API to create a child as part of a wake.
- Lifecycle rules: child inherits a capability subset from the parent, child's events are queryable from the parent's view, disabling a parent disables children.

**Caveats:**

- This is a real conceptual addition. Flat peer-to-peer agents are conceptually simpler. Adding hierarchy is worth it _if_ we find real use cases we can't express flatly.
- Could be delivered as "agent templates" + a dedicated `spawn_subagent` tool rather than a whole new schema.

**Alignment:** Tangential to current design, not contradictory. Defer until a use case demands it.

### 4.4 Session API → tree-structured conversation, forking, compaction, FTS

**Cloudflare:** Messages stored as a tree (`parent_id`), enabling forks, non-destructive compaction, and FTS5 search.

**Open Pincery:** Events are a flat append-only log per agent. "Summarization" happens via wake summaries but without an explicit tree or fork primitive.

**Worth adopting:**

- **FTS over events** (already listed under Bet 2). Cheap, standard Postgres.
- **Non-destructive compaction of event context in prompts** — instead of truncating old events out of the window, summarize them into a "compact" event that references the originals. Preserve auditability while keeping the prompt small.
- **Forking** is probably overkill for now. Forking a continuous agent's timeline has unclear semantics (two agents with the same identity? One "what-if" agent? That's close to sub-agents.).

**Alignment:** FTS + compaction are straightforward additions. Forking is a feature in search of a user story.

### 4.5 Codemode → LLM writes programs, not tool calls

**Cloudflare:** Instead of N round-trips through the model for N tool calls, the model writes a single program that calls all the tools locally and returns the result. They report a 99.9% token reduction for their MCP example.

**Open Pincery:** `README.md` explicitly states the shell tool is "a programmable executor where agents write programs, not individual tool calls." This is _already our philosophy._

**Worth making explicit:**

- Document that the shell tool _is_ the codemode pattern.
- Provide a small DSL or standard library inside the shell environment (helpers for: curl a local API, parse JSON, read/write a scratch file, etc.) so the "programs" agents write are short and conventional.
- Consider a sandboxed JS/TS executor in addition to shell for agents that prefer typed code. Not a replacement — an additional tier.

**Alignment:** Vindicates the existing design. The improvement is to lean into it more explicitly.

### 4.6 Execution ladder → workspace → isolate → npm → browser → sandbox

**Cloudflare's tiers:**

- Tier 0: Workspace (durable filesystem).
- Tier 1: Dynamic Worker (JS sandbox, no network).
- Tier 2: + npm packages.
- Tier 3: + headless browser.
- Tier 4: + full OS sandbox (git, compilers, test runners).

**Open Pincery today:**

- Tier 0 analog: the event log + projections is a "workspace" but not a filesystem the agent can `ls`.
- Tier 4 analog: zerobox (planned, per security architecture).
- Tiers 1–3: not present.

**Worth considering:**

- **A per-agent scratch filesystem** (`/workspace/agent-<id>/`), persisted, checkpointed, searchable. Makes the "workspace" mental model concrete. This is a near-term, finite feature.
- **A sandboxed scripting tier between shell and full OS** — e.g., a wasmtime-based JS or Lua runtime with no network, for cheap, safe code execution without spinning up a container.
- **Headless browser as an agent tool** — deferred, but worth naming as a future tier.

**Alignment:** The ladder idea is compatible with the existing security model. Tier 0 (workspace) is the most valuable near-term addition.

### 4.7 Self-authored extensions

**Cloudflare:** Agents write their own tools at runtime. Declare permissions (network, workspace). The platform bundles, sandboxes, and registers them.

**Open Pincery:** No analog. And — critically — the current architecture forbids this without a sandbox story (see Bet 1 on identity).

**Worth adopting (conditional on sandbox):**

- Once zerobox is integrated, agents could write shell scripts or programs into their workspace and register them as named tools for future wakes.
- The permission declaration model (explicit allow-list at creation time) should be copied wholesale. "The agent declares what it needs" is the right pattern.

**Alignment:** Genuine new direction. Requires the identity/sandbox work first. Defer until Bet 1 lands.

---

## Part 5 — Synthesis: what to do with all this

### Already aligned; sharpen and document

1. **Continuous agents + durable missions** (Bet 3) — the existing vision. Add named `mission_id`s, document "survives a quarter" explicitly.
2. **Shell as codemode** (4.5) — document the philosophy, add a small standard library inside the shell env.
3. **Per-agent isolation at the data layer** — exists; add hibernation / idle metrics so the story is measurable.

### Finite, near-term iterations (good candidates for the next scope)

4. **LLM-aware rate limiting and provider backoff** (Q5). Small, well-scoped, addresses a real operator pain.
5. **Wake-level durable execution / checkpoints** (4.2 fibers). Clear, finite, bounded addition to the wake loop.
6. **First-class agent-to-agent messaging with scoped tokens** (Part 2, resolution A or B). Solves Q3 + partially Q4.
7. **FTS over events + non-destructive compaction** (4.4, Bet 2). Standard Postgres, pays back immediately.
8. **Per-agent scratch workspace / filesystem** (4.6 tier 0). Makes the "workspace" model concrete.
9. **`pcy demo` smoke-test path + UI walkthrough doc** (Q1, Q2). Low code cost, huge onboarding win.

### Real architectural decisions (deserve their own EXPAND cycles)

10. **Per-agent identity and capability enforcement** (Bet 1, Q4). This is the single biggest gap between "demo" and "enterprise-ready." Pick a target — per-agent LLM keys? Per-agent Postgres roles? Both? — before scoping.
11. **Sandboxed execution tier between shell and full OS** (4.6 tiers 1–3). Unblocks self-authored extensions later.
12. **Human-in-the-loop approval primitives** (Bet 3). Should agents be able to pause on an `APPROVAL_REQUESTED` event?
13. **Multi-runtime scaling story** (Q8). At what agent count does the single-process runtime break? What replaces it?

### Tangential / defer

14. **Sub-agents / hierarchy** (4.3). Conceptually interesting; no concrete user story yet.
15. **Conversation forking** (4.4). Semantics unclear for continuous agents.
16. **Self-hosting new stuff within the server** (Q7). Requires 10 + 11 first.
17. **Self-authored extensions** (4.7). Requires 10 + 11 first.

---

## Part 6 — Non-goals (so we remember what we rejected)

- **Becoming a Cloudflare clone.** The Durable Object placement / Worker isolate model is a platform feature we can't replicate in a single Rust binary. We can borrow the _API shape_ (agent name → singleton with state + scheduling) because we already have it. We shouldn't chase the _deployment model_.
- **Adopting a specific vendor SDK surface** (Cloudflare, LangChain, etc.) as our external API. Open Pincery's API contract (AC-27) is deliberately minimal. New primitives should reinforce that, not dilute it.
- **Turning the shell tool into a tool registry.** We explicitly chose "one programmable executor" over "dozens of tools." Adding `send_message` as a first-class tool is a deliberate, limited exception because of the ambient-auth problem, not a policy change.

---

## Part 7 — Next step

Before any of this becomes scope:

1. Pick **one** item from the "finite, near-term iterations" list (section 5) as the next iteration candidate.
2. Run `/iterate` against `scope.md` with that one item as the new requirement set.
3. Let the pipeline do the work — don't mix-and-match ideas across cycles.

If we want to tackle something from "real architectural decisions," that's a full EXPAND cycle on its own, not an iteration.
