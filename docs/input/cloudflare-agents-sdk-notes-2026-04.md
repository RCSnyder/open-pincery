# Cloudflare Agents SDK — Technical Notes

**Source:** [developers.cloudflare.com/agents](https://developers.cloudflare.com/agents/) (Apr 2026, same quarter as the internal-stack blog post already covered in [cloudflare-ai-infra-notes-2026-04.md](cloudflare-ai-infra-notes-2026-04.md)). Live docs; last updated 2026-04-20.

**Why this doc exists:** The earlier Cloudflare blog described the internal _platform_ that wraps agents (proxy Worker, MCP Portal, Backstage, reviewer CI). This doc covers the _runtime primitive_ Cloudflare ships for third parties — the Agents SDK. An agent is a single TypeScript class sitting on a Durable Object with its own SQL database, WebSocket connections, and scheduler. The same class handles inbound email, HTTP, WebSocket messages, state changes, scheduled wakes, tool calls, MCP exposure, browser control, and voice streams. This is the most concrete public articulation of what an "agent runtime" actually contains, and it maps cleanly onto Open Pincery's pincer layer.

This doc is inspiration for the north-star and, when scope warrants, for the substrate design. It is not a directive to adopt Cloudflare; we are not running on Workers.

---

## Headline Claims

1. **An agent is one class per agent instance, backed by a Durable Object with its own SQL database and its own key-value state.** State survives restarts, deploys, and hibernation. The "reconstruct state at the start of every request" pattern goes away. Memory isn't attached to the agent — it _is_ the agent.
2. **The same class handles every event shape the agent can receive.** Inbound email, HTTP, WebSocket messages, state-change callbacks, scheduled wakes, tool calls — all land as methods on the class. No external dispatcher glue.
3. **State syncs to connected clients in real time.** Any `setState` update pushes to subscribed clients over WebSocket; a React hook (`useAgent`) consumes it. Agent state and UI state are the same state.
4. **An agent can wake itself.** Scheduled tasks run on a delay, at a specific time, or on cron. No user, no request, no external scheduler. The agent is a persistent actor, not a request handler.
5. **Typed RPC over WebSocket.** Methods marked `@callable()` become typed RPC the client calls directly. No REST layer, no JSON schema generation, no hand-rolled message-type enum.
6. **Any LLM provider is a configuration choice, not an architectural commitment.** Workers AI, OpenAI, Anthropic, Gemini, or anything else, swapped per call. Long-running reasoning models that take minutes to respond are declared supported out of the box.
7. **Workflows are the durable-execution primitive.** When the agent needs guaranteed multi-step execution with automatic retries that survives crashes and deploys, it delegates to a Workflow. The agent is the session; the Workflow is the durable step graph; the sandbox is the per-step execution surface. Three distinct primitives.

## The Capability Surface, Summarized

The SDK's top-level capability list from the index page:

| Capability               | Primitive                                    | What it actually is                                                                 |
| ------------------------ | -------------------------------------------- | ----------------------------------------------------------------------------------- |
| Remember everything      | Per-agent SQL + KV state on Durable Object   | Agent-scoped database; survives restarts; syncs to clients live                     |
| AI chat                  | `AIChatAgent` + `useAgentChat` React hook    | Streaming chat with automatic message persistence + resumable streams               |
| Any model                | Provider-agnostic `streamText`               | Workers AI, OpenAI, Anthropic, Gemini, etc. Long-running reasoning models supported |
| Realtime transport       | WebSockets + SSE                             | Agent holds a live connection; both push and pull                                   |
| Tools                    | Server-side + client-side + human-in-loop    | Tools run in the agent, in the browser, or pause for human approval                 |
| MCP                      | `McpAgent`                                   | Agent's own tools are exposed as an MCP server to other agents / LLMs               |
| Scheduling               | `schedule()` on the agent                    | Delay, specific time, or cron. Agent wakes itself                                   |
| Browser                  | Browser Rendering (Chrome DevTools Protocol) | Scrape, screenshot, debug, interact with pages                                      |
| Voice                    | STT / TTS over WebSocket                     | Realtime voice agents with persisted conversation                                   |
| Orchestration            | Workflows                                    | Multi-step, durable, retried, survives restart                                      |
| Coordination             | Multi-agent via Durable Object addressing    | Agents call other agents                                                            |
| Event reactivity         | Inbound email, HTTP, WS, state change        | All land as methods on the same class                                               |
| Realtime state → clients | `setState` push                              | UI is a live mirror of agent state                                                  |

## Technical Claims with Arguments

### The Durable Object shape is the bet

- **Each agent instance is an addressable, long-lived actor.** Not a request handler. A pincer in Open Pincery is closer to this than to a serverless function: it has identity, it has state, it runs in the background, it is woken by events. The Durable Object model describes what a pincer runtime _wants_ to look like even if OP never runs on Workers.
- **One SQL database per agent, not one shared warehouse.** This is the inverse of a typical monolithic app: instead of one Postgres + many records keyed by tenant, each agent owns a small database. At scale ("tens of millions of instances") this is how state stays cheap and scoped. For OP, this is a live architectural question: does a pincer get a scoped slice of the substrate's Postgres (schemas, per-pincer tables), or a per-pincer SQLite file, or stay in the shared event log with row-level capability filtering?
- **State survives everything the runtime does to it.** Restart, deploy, hibernation. The agent doesn't hydrate state on entry; the state is just there. This removes a whole class of startup bugs and makes long-running agents tractable. Bet #2's memory-as-substrate claim lines up with this exactly.

### One class, every event shape

- **`onEmail`, `onRequest`, `onMessage`, `onStateUpdate`, scheduled callbacks — all methods on the same class.** This matters because it kills the "router file + handler file + background worker + cron file" decomposition. The entire agent's behavior is readable in one place.
- **For OP, this is a design principle for pincers, not a directive to adopt the SDK.** A pincer's wake callback, its tool-result handler, its scheduled check-in, and its inbound-webhook reaction should all be declared in one artifact. If they're spread across four files the operator can't audit the pincer, and the capability model can't enforce its envelope cleanly.

### Realtime state is an architectural primitive

- **`setState` pushes to subscribed clients.** No explicit WebSocket send; no message-type enum. The client subscribes to the agent via `useAgent`, and the hook receives `onStateUpdate` whenever state changes.
- **Implication for OP:** the substrate's projection layer (event log → projections) _is_ the state Cloudflare is pushing. If a pincer's observable state is a projection, then the UI subscription is just a query + change feed against that projection. OP's eventual operator UI (console, inbox, etc.) can live on exactly this pattern — no separate push-notification layer, no WebSocket hand-rolling. This is consistent with where Bet #2 already points.

### Scheduling is part of the runtime, not an external system

- **`this.schedule(...)` runs on delay, at absolute time, or on cron.** The callback is a method on the agent's class. No Cloud Scheduler, no Temporal, no external worker.
- **OP already intends this** (wake loop + event-triggered activation); the Cloudflare shape is validation that the cheap path (scheduler is part of the substrate, not a microservice) is the correct one. The substrate's wake loop is the pincer equivalent of agent scheduling.

### @callable RPC collapses the client-server contract

- **Methods marked `@callable()` become typed RPC over WebSocket.** The client calls `agent.stub.increment()` and the method runs on the agent. No OpenAPI, no tRPC glue, no REST handler, no serialization boilerplate.
- **Implication for OP:** the operator console's calls into the substrate can follow the same shape. A capability-scoped operator method call to a pincer ("approve pending escalation", "terminate mission", "request explanation") should be a typed call into a method on the pincer class, auth checked at the substrate, not a hand-rolled REST endpoint. This is a UX design note more than a substrate note — Rust + Axum gives us the building blocks; we need to resist hand-rolling per-endpoint glue.

### Workflows are a separate primitive from the agent

- **Workflows guarantee executions. Agents hold sessions.** When an agent needs a multi-step, durable, retried operation that must complete even if the agent is evicted from memory, it starts a Workflow. The Workflow persists per-step state; the agent consumes the Workflow's result.
- **OP's mapping:** a mission is the workflow; a pincer is the agent; the event log + projections are the workflow's durable state. This is already the shape. The Cloudflare doc is a cleaner vocabulary for describing it — _session_ (pincer) vs _workflow_ (mission) vs _sandbox_ (per-step execution surface, from the earlier blog). Adopt this vocabulary in design docs.

### Tools, approvals, MCP

- **Server-side tools run in the agent; client-side tools run in the browser.** This is a genuine design axis — some tools need the browser's context (the user's clipboard, a web extension, the user's own session with a third-party app) and must run client-side; the agent orchestrates but does not execute them directly.
- **Human-in-the-loop is a first-class tool shape.** A tool can request approval; the agent pauses; the operator approves or rejects; the agent resumes. The message stream through `AIChatAgent` handles the pause/resume.
- **The agent's own tools are exposable as an MCP server.** Via `McpAgent`. This is the "be both an MCP client and an MCP server" claim. Open Pincery has the same claim in Bet #9 — `MCP outward to tools, MCP / A2A inward to peers`.

### Browser, voice, email — expensive capabilities as declared surfaces

- **Browser tools** use Chrome DevTools Protocol for scrape/screenshot/debug/interact. This is a genuine capability, not a playwright wrapper the user has to maintain. For OP it is another concrete argument for "real-browser authority is a capability-model test case" (from the GenericAgent notes): the substrate needs a browser capability _as a named thing with declared scope_, not as "the pincer just has a browser."
- **Voice agents** over WebSocket with STT/TTS and persisted conversation. Not OP-relevant in the short term — the founder isn't shipping voice pincers. Filed for watch.
- **Inbound email** is a supported event shape. An agent can have an email address and react to messages. This is directly relevant to OP's "CRM responder" and "inbound intake" mission ideas: an email to a designated address is a pincer wake event, not a separate ingestion pipeline.

### Coordination through addressed agents

- **Multi-agent is "agents call other agents via Durable Object addressing."** There is no central orchestrator primitive in the SDK. A coordinator agent is just an agent whose tools happen to be stubs for other agents.
- **For OP:** coordination is a catalog-level convention per Bet #12. This validates the decision not to put coordination primitives in the substrate — Cloudflare, at their scale and polish, also didn't.

---

## Implications for Open Pincery

These are my reading, not Cloudflare's claims.

1. **The Durable Object shape names a primitive OP is already building but hasn't formalized: the pincer actor.** A pincer is long-lived, addressable, stateful, woken by events, restarted without state loss. "Pincer actor" is the right mental model, not "pincer process" or "pincer job." Make this vocabulary explicit in the design doc once ANALYZE starts cataloging the runtime. Not a new bet — a clarifying frame under Bet #11.

2. **Session / Workflow / Sandbox is a clean three-way decomposition; OP should adopt the vocabulary.** Cloudflare separates:
   - **Session** (the agent's live context, `this.state`, open WebSockets, pending tool calls)
   - **Workflow** (the durable multi-step graph with retries)
   - **Sandbox** (the ephemeral execution surface per step)
     OP already has all three — pincer working memory, mission event log, and the Bet #11a sandbox — but they are not yet named as a triad. Naming them makes design discussions cheaper and makes it easier to see when one is being overloaded into another.

3. **Per-agent SQL is a live architectural question for Bet #2.** OP currently assumes one shared Postgres with scoped queries. Cloudflare demonstrates that per-agent databases scale. The question is not "should OP switch" — that's not a v7 decision — but "does the memory-controller interface leave room for per-pincer state to migrate to its own store if mission load or isolation demands it?" The controller should be the only thing that cares, and the answer should be yes. Flag this as a design constraint for Bet #2 rather than a new bet.

4. **`@callable` typed RPC is the right shape for operator-to-pincer control plane.** OP's operator console will want to approve escalations, terminate missions, inject overrides, and request explanations. Building this as a REST surface with hand-rolled handlers per verb is the standard way and the wrong way. The right shape is: each pincer (and the substrate itself) exposes a small set of capability-gated methods that the operator console calls directly with type-safe RPC. For Rust + Axum, this likely means an `rpc!` macro or tower-rpc-style crate; for TypeScript-ish operator UIs, tRPC. Design note for v8, not v7.

5. **State-sync-to-client is the right primitive for the operator UI.** The operator console shouldn't poll. It subscribes to mission projections and pincer state. Every state change on the pincer is a delta the UI receives. This is an implementation choice for the eventual console; flag it so the console doesn't accidentally get built as a polling app.

6. **Inbound email is a pincer wake event, full stop.** Any "CRM responder", "intake triage", "support first-pass" pincer in OP's future catalog should be designed on the assumption that an incoming email to a designated address is a capability-scoped wake event with the email in the payload. Not a separate IMAP poller. Not a separate ingestion pipeline. The substrate should accept inbound email as an event source before the first such mission ships.

7. **Human-in-the-loop is a tool shape, not a mission shape.** Cloudflare models approval as a _tool_ that happens to request human confirmation. OP currently thinks of approval at the mission level ("Tier 2 missions require escalation"). Both are valid; they compose. A mission's _acceptance contract_ gates the mission; a _tool_'s approval requirement gates individual actions within a running mission. Bet #5 (acceptance contracts) and Bet #3 (capabilities) together already allow this, but name the two layers — mission-level approval vs. action-level approval — explicitly so design discussions don't conflate them.

8. **"Long-running reasoning models that take minutes to respond work out of the box" is a substrate requirement, not a nice-to-have.** Any proxy in OP that times out at the default HTTP idle limit will silently break the reasoner-abstraction claim when a frontier reasoning model takes four minutes on a hard problem. The reasoner proxy must support long-duration streaming requests as a named requirement, not as a later patch.

9. **Multi-agent coordination via addressed pincers is the right composition model.** Bet #12 already says "coordination is a catalog convention, not a substrate primitive." Cloudflare arrived at the same answer at vastly larger scale. This is a validation, not a new claim. Cite it when Bet #12 is questioned.

10. **Browser, voice, and email are named capability _shapes_, not free-form tools.** They need capability-model treatment individually, not as an undifferentiated "tool access" grant:
    - `browser: {domains: [...], session_scope: ...}`
    - `email_inbox: {address: ..., intents: ["triage", "respond"]}`
    - `voice_session: {max_duration: ..., storage: ...}`
      Each has enough distinct authority surface to deserve its own capability type under Bet #3. Flag as an elaboration of Bet #3, not a new bet.

## What to Discard (or defer)

- **Workers / Durable Objects / Workflows as infrastructure.** OP is a Rust substrate on an operator-hosted machine. We do not run on Workers. We borrow vocabulary and design patterns, not runtime. Anyone reading this doc should treat every mention of Durable Objects as "an actor with a database and a mailbox" — that's the portable idea.
- **Voice agents as a near-term capability.** File for watch; do not scope.
- **`AIChatAgent` + `useAgentChat` as a concrete library to port.** The chat-agent pattern is worth borrowing; Cloudflare's specific library is not. OP's operator console will likely have a small chat surface; build it natively when needed.
- **MCP Portal as a distinct layer inside the substrate.** OP _is_ the portal at solo scale (already noted in [cloudflare-ai-infra-notes-2026-04.md](cloudflare-ai-infra-notes-2026-04.md)). The Agents-SDK-level `McpAgent` primitive is the right shape for _a pincer_ to expose its tools, not for the substrate to aggregate external MCP servers. Keep the two layers distinct in design docs.
- **Durable Objects' single-instance guarantee as a substrate requirement.** Cloudflare relies on "one Durable Object instance per id, globally." A self-hosted Rust substrate on one operator machine does not need global singletons; a per-pincer mutex or actor mailbox is enough. Do not over-architect to match the Cloudflare guarantee.
