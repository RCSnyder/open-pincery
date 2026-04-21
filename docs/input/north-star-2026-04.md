# Open Pincery North Star

**Date:** 2026-04-20 (v6 synthesis)
**Status:** Canonical direction for the project. Supersedes prior `strategic-answers`, `tripwires`, `agent-taxonomy`, `research-synthesis`, and `first-principles-assessment` drafts in `docs/input/v6_pre_iterate/` as the single source of truth for product direction. Those precursor docs remain as provenance. This doc will move to `docs/reference/north-star.md` when the v6 scaffolding cycle reconciles.

---

## One-Paragraph Pitch

Open Pincery is a **sovereign substrate for an AI staff of one** — a single-binary Rust runtime that turns an event-sourced memory and a catalog of bounded mission types into a dependable, auditable workforce running on infrastructure you own. It is for the founder-CEO wearing every hat, or the one-CTO operator of a small company, who needs leverage without surrendering their data, their model choice, or their ability to sleep. Everything it does leaves a replayable trail, every mission has a named human who is accountable for the outcome, and no vendor has privileged architectural access.

## One-Line Positioning

> The only AI substrate I would actually let run my company overnight, on infrastructure I control, against models I choose.

## Thesis

A one-person company can run professionally only when its tools hold a professional line without constant supervision. Open Pincery is that line: a substrate where durable specialist agents run bounded mission types under acceptance contracts, with every action leaving a replayable trail and a named accountable human.

Professional-grade, not slop. Dogfooded by its founder before anyone else is asked to use it. Sovereign by architecture, not by preference.

## Buyer

The buyer is one person wearing many hats:

- The solo founder-CEO who is also CTO, support lead, ops lead, and accountant — the author of this project is the first instance.
- The single technical CTO of a small company responsible for most technical surfaces.

These are the same buyer at different career stages, sharing the same constraint: **no compliance owner to hand evidence to, no ops team to babysit running processes, no tolerance for confident-wrong AI output.** They need leverage, and they need to sleep.

This is not the median solo founder. The median solo founder pays Lindy or Zapier $50/mo, accepts vendor-consumer terms, and gets on with shipping. Open Pincery's buyer is the minority who have already been burned by vendor lock-in, a data-handling incident, a compliance customer requirement, or the dawning realization that their business depends on a substrate they do not own. If the external cohort refuses to pay for sovereignty, the buyer hypothesis was wrong — not the substrate.

This buyer is the year-one focus. The substrate is designed so that enterprise and SaaS are natural extensions — not rewrites — once the single-operator experience has been proven. Those extensions are explicitly deferred.

## The Professional Bar

The word "professional" carries most of the weight of this project, so it is made concrete. A mission is not professional until all six of these are true:

1. **Auditable** — every action is recorded in the event log with provenance.
2. **Accountable** — every mission instance has a named human who owns the outcome. Legal accountability never transfers to the software.
3. **Bounded** — the mission type declares an acceptance contract: what "done" means, what evidence is required, what the output must look like.
4. **Reproducible** — any material action can be replayed from the log.
5. **Cost-ceilinged** — every model call and tool invocation has a declared budget. Over-budget triggers escalation, not silent continuation.
6. **Rollback-capable or confirmation-gated** — every write has a defined way back, _or_ an explicit operator confirmation before the irreversible step. An exploratory mission that spent $140 of compute has no rollback; the budget itself is the receipt and the learning is the artifact. A mission about to send an external email, delete a production row, or post a public artifact stops and asks. The invariant is _"no silent irreversibility"_, not _"everything is undoable."_

The founder benchmark (below) measures the system against this checklist, not against vibes.

## Category Claim

Open Pincery occupies three classifications at once. All three are necessary; dropping any one confuses the positioning.

- **Deployment shape: Continuous Agents (Category 5).** Resident, event-driven, mission-bounded, wake/sleep. Distinct from coding harnesses, dark factories, auto-research loops, and orchestration frameworks. The diagnostic question it passes: _"Is the agent still running while you're asleep?"_
- **IS archetype: Collaborative Agentic IS.** Humans and agents both hold high agency on shared objectives. Not assisting, not autonomous, not hybrid. Delegation flows in multiple directions; patterns are conventions above the substrate (see _Delegation Direction_ below).
- **Cognitive capabilities: whatever the mission demands, inside scope.** Agents introspect (know what they have and haven't done), self-monitor (detect when they are drifting from the contract), adaptively select strategies, and escalate when confidence drops below threshold. The constraint on agency is not cognitive — it is **capabilities + budget + acceptance contract**. Inside that scope the agent can do anything the operator's credentials permit: write code, provision infrastructure, spin up auxiliary services, call tools, spawn sub-agents, run for hours.

The peer set for competitive comparison is **runtimes** — Zapier Agents, Lindy, AWS Bedrock AgentCore, LangGraph Platform, Cloudflare Agents, Cursor Background Agents, ChatGPT Agent, Devin, Claude Cowork / Dispatch — not libraries (CrewAI, LangGraph, AutoGen). Comparing Open Pincery to a library is a category error. The sharper 2026 diagnostic: _"can you subpoena its memory and replay its decisions six months later on your own infrastructure?"_

## Bounded Missions, Unbounded Depth

Open Pincery is bounded at the mission-type level and unbounded at the execution-depth level.

1. The system has a curated catalog of mission types. Each mission type has an acceptance contract, a declared set of capabilities it may use, and a budget ceiling.
2. Inside a mission, execution is open-ended and materially capable. A pincer can decompose a problem, call tools for hours or days, write and execute code in a capability-scoped sandbox, provision infrastructure within its grant, and produce arbitrarily large artifacts — anything the granted capabilities and remaining budget permit. The pincer does not grant itself new capabilities, rotate its own secrets, or create sub-pincers; if it needs those things, it escalates.
3. The professional bar is enforced by the contract, the capability scope, and the budget ceiling — not by shallowness. An agent that cannot meet the contract, exhausts its budget, or needs authority it was not granted **escalates** to the accountable human. It does not fake completion, silently expand scope, or consume unbounded resources.

This is how professional firms operate. A law firm does not take any case; it takes specific case types with a scope of engagement and a fee schedule, and inside each type the depth scales to whatever the matter demands, while the output is held to a professional standard regardless of depth.

This is not a cap on intelligence or initiative. It is how intelligence and initiative are made auditable.

## Delegation Direction is a First-Class Concern (Not an Enum)

Missions flow in multiple directions. A mission can be assigned by the human and owned by the agent, shared with ownership transitioning by situation, or nudged by the agent suggesting work the human should do. These distinctions matter — they shape the acceptance contract and the trust surface — but they are **conventions, not substrate types**. The substrate does not enumerate delegation patterns; it supports bidirectional initiation and ownership records on the mission. Conventions emerge above the substrate and can evolve with models.

The one line the substrate _does_ draw: an agent directing the human as subordinate, with ownership legally transferred from human to agent, is out of scope for year one. Not because it is impossible to implement, but because EU AI Act legal accountability and operator trust have not been earned. Weak nudges, escalations, and bidirectional hand-offs are fully supported from day one.

## First Mission Catalog (Tier 1)

Tier 1 is the minimum set of **standing missions** whose absence prevents a one-person company from functioning. Built in order:

1. **Codebase steward** _(bidirectional)_ — PR review, dependency hygiene, release notes, changelog, security-sensitive-file flagging.
2. **Inbox triage** _(user-invoked)_ — surface what needs a human response, draft replies for the rest, escalate anything with a deadline or a dollar sign.
3. **Commitments tracker** _(bidirectional)_ — track what has been promised to whom by when; surface upcoming obligations, overdue items, decisions needed.
4. **Weekly digest** _(IS-invoked, weak)_ — Monday artifact of last week's work, this week's decisions needed, overdues, and pending escalations.
5. **Exploratory runner** _(user-invoked)_ — the first exploratory mission type. The operator gives a charter, a dollar budget, a wall-clock budget, and a capability scope; the agent returns a log, a summary, and a recommendation. This is the _"go experiment with this repo for $200"_ primitive.

Tiers 2 and 3 (pipeline follow-up, content ops, competitor watch, financial summary, contract review, research synthesis, customer-support auto-response) are explicitly deferred until Tier 1 is solid in daily founder use.

The **"shipping"** target for year one is one mission done to the Professional Bar (the codebase steward). The other three prove the substrate's reach; the first proves the bar.

## Two Mission Shapes as a Lens (Not a Discriminant)

It is useful to notice that missions come in two shapes:

- **Standing** — recurring operational work with a tight, specific contract. _Review every PR. Triage the inbox hourly. Digest the week._
- **Exploratory** — one-shot, time-and-budget-boxed investigation where the operator cannot specify the output in advance. The contract is structural (_what shape of answer, what budget, what capabilities_), not substantive. The canonical example: _"Here's $200. Go experiment with this repo. Capabilities: read, clone, compute. Tell me if it's worth our time."_

This distinction is a lens for thinking about acceptance contracts, not a type on the mission record. A mission carries a charter, a budget, a capability scope, and an acceptance signal; whether we call it standing or exploratory is a property of what the charter says, not a separate field. The substrate does not need to enumerate the two — operators and the catalog conventions do.

What matters is that the exploratory shape is honored at all. Without it, operators route open-ended curiosity through ad-hoc prompts outside the substrate, and the substrate loses its audit property. With it, even _"go look at this"_ leaves a replayable trail.

## Signals: How Missions Communicate

A mission never stalls silently. When the agent needs something from the accountable human — permission, a secret, a decision, a clarification, a second opinion, a budget increase, or acknowledgment that it is done — it emits a **signal**. Signals are the single generic primitive for agent↔human communication inside a mission.

A signal carries:

- A direction (agent → human, or human → agent).
- A semantic tag (free-form, operator-defined; not a substrate-level enum).
- A payload (what is being asked or provided).
- A response expectation (ack / decide / provide / none).

The substrate guarantees delivery, recording, and replay. It does not enumerate the kinds of signals a mission may emit. That is an intentional choice: stronger models will want to renegotiate charters, propose decompositions, request modalities, and collaborate with other agents in ways we cannot predict. A generic signal primitive absorbs that evolution; a typed enum of seven escalation categories fights it.

Year-one conventions — not substrate types, but common tags operators and the catalog are likely to settle on — include:

- _capability-grant-request_ — _"I need `github:write` to open this PR."_
- _secret-provision-request_ — _"Please add `STRIPE_API_KEY` scoped `read:invoice`."_
- _budget-increase-request_ — _"$180 of $200 spent, need $100 more to finish."_
- _ambiguity_ — _"Charter says 'worth our time' — revenue, learning, or strategic fit?"_
- _decision-point_ — _"This fork diverges our public API. Proceed?"_
- _confidence-below-threshold_ — _"40% confidence on this PR review. Please review manually."_
- _capability-absence_ — _"I need `provision:database`, which doesn't exist in the substrate."_

These are illustrative, not canonical. Operators can define new tags. The substrate does not care what the tag says; it cares that the signal is logged, deliverable, and replayable.

The operator sets the delivery policy (signal inbox, email, morning digest, synchronous callback). The substrate never silently times out a mission waiting on a human response — that would be a failure of the substrate, not the agent.

## Durable Bets

These are the stable bets that should survive individual iteration changes.

### 1. The event log is the product

The log is not debug exhaust. It is a first-class artifact: typed, queryable, replayable, exportable.

### 2. Memory is the substrate, not the reasoner

The differentiating primitive is the coordinated memory controller — parametric (model weights), working (context), external (event log + projections + vector + graph). Reasoners are pluggable; memory is the thing we own. The memory controller interface is backend-agnostic by design:

- **v7:** Postgres + pgvector for semantic recall.
- **v10-ish:** graph substrate for relational queries over cross-mission history. Target: **CozoDB embedded** (Rust, single-binary, Datalog, vector-capable) — chosen to preserve the one-Rust-binary-plus-one-database sovereign-host story. SurrealDB, IndraDB, Memgraph, Neo4j were considered and rejected on sovereignty, language, or deployment grounds.

The _event log_ is the ground truth; above it, memory is organized into five layers with distinct update cadences and recall patterns (framing borrowed from GenericAgent's L0–L4):

- **L0 — Rules**: the Professional Bar, capability-model invariants, acceptance-contract grammar. Immutable within a substrate version; always in scope.
- **L1 — Index**: small queryable index over projections so pincers can route without loading the event log.
- **L2 — Operator facts**: stable knowledge about the operator's world (vendors, conventions, preferences, owned systems). Grows slowly, persists across missions.
- **L3 — Pincer skill tree**: auto-crystallized execution paths scoped to a pincer or mission family. See Bet #6a.
- **L4 — Mission archives**: distilled records of completed missions for long-horizon recall.

The memory controller is the only component allowed to cross these layers. Reasoners query through it; they do not see raw SQL or raw event-log bytes. Context budget is a controller responsibility: the controller delivers a small, relevant context per call, not an unbounded scroll.

### 3. Capability-scoped credentials beat ambient authority

Agents do not inherit broad env-var authority. They receive narrow, auditable capabilities that can be delegated, rotated, and revoked. An agent granted `provision:database` can spin up a Postgres instance; an agent granted `read:inbox` cannot. Within its granted capabilities the agent acts with full initiative. Outside them it must escalate to request a capability grant — it cannot silently expand its own authority. Blast radius matters more when one person is the whole company.

Expensive capability surfaces — a browser session, an email inbox, a voice channel, a code-execution sandbox — each get a _distinct capability shape_, not an undifferentiated "tool access" grant. A `browser` capability declares its allowed domains and session scope; an `email_inbox` capability declares its address and intent set (`triage`, `respond`); a `voice_session` capability declares duration and storage. Granting "use my browser" wholesale is ambient authority by another name; the substrate refuses to model it that way.

Credentials themselves flow through a two-layer mechanism specified in the Open Pincery TLA model and the security architecture. The operator provisions secrets **out-of-band** through a dedicated **credential vault** (encrypted at rest, AES-256-GCM); the operator never pastes a secret into a chat surface, a charter field, or a signal response. The vault is the only safe channel, and the reasoner is system-prompted to refuse any attempt to receive a secret through conversation and to redirect the operator to the vault. At runtime, the substrate reads the credential's _name_ from the vault and hands a **placeholder** to the sandbox (`AWS_ACCESS_KEY_ID=ZEROBOX_SECRET_…`); the real value only exists inside the Zerobox proxy, which substitutes it into outbound HTTPS requests to pre-approved hosts. The process never sees raw credentials; exfiltration to an arbitrary host returns the placeholder string. `list_credentials` returns names only, never values.

This is the mechanism that makes the whole bet enforceable: without the vault + proxy-injection pattern, "capability-scoped" degrades into "we promised to be careful with env vars."

### 4. Missions are the unit of work

Prompts and wakes are runtime mechanics. Missions are the business object. A mission instance is a small, generic record:

- An identity and an accountable human.
- A charter (free-form intent).
- A capability scope (what it may touch).
- A budget (dollars and wall-clock).
- An acceptance signal (how we will know it is done or stuck).
- An event log.

That is it. Standing vs exploratory, delegation direction, governance class, mission-type name, and signal semantics all live _inside_ the charter and its conventions — not as substrate-level enums. This is deliberate: we do not know what shapes future models will want to negotiate, and a small set of generic fields absorbs that evolution better than a large set of specific types.

### 5. Every mission type has an acceptance contract

A mission type without a contract is not ready for the catalog. The contract is what separates professional output from slop and what allows honest escalation instead of faked completion. The contract itself is immutable within a mission instance: only the accountable human can change what "done" means mid-run. The agent can interpret the contract, pick strategies under it, and refine its own plan — but it cannot silently rewrite the contract.

### 5a. Every mission escalates rather than fakes completion

When an agent cannot meet its acceptance contract — whether for capability, data, authority, budget, or confidence reasons — it stops, records the unfinished state, and returns control to the accountable human with a structured explanation of what blocked it. Escalation is a first-class mission outcome, on par with success. A mission that completes only because it silently narrowed its own criteria is a failed mission, regardless of what it reports.

_Escalation is the name we give to the pattern of signals that return control to the accountable human when the contract cannot be met. The substrate guarantees signals; convention names this particular signal pattern "escalation."_

### 6. Mission catalog grows from real use

A mission joins the catalog because the operator hit the same manual work three times, not because it sounded good in a planning document. Speculative missions are not built.

### 6a. Pincers grow a skill tree; the canonical catalog grows more slowly

There are two layers of captured know-how, and collapsing them wastes most of the agent's learning.

- **Canonical mission catalog** (Bet #6 above) — Tier N mission _types_ with acceptance contracts, promoted by the operator after repeated real use. Lean. Curated. Operator-gated.
- **Pincer skill tree** (this bet) — auto-crystallized execution paths scoped to a pincer or a mission family, written to memory layer L3 on first successful completion of a novel sub-task. Pruned on failure or disuse.

When a pincer inside an exploratory mission figures out how to parse a specific vendor's export format, navigate a specific portal, or wire up a specific API, that execution path is valuable operator-scoped knowledge. Without L3, it is lost when the sandbox tears down and the next similar sub-task repeats the same struggle. With L3, the pincer recalls the crystallized skill and runs the path directly.

Auto-crystallization is _not_ self-granting authority. A skill compresses already-granted, already-executed work into a faster re-run. Every crystallized skill declares its capability dependencies; a pincer granted only `github:read` cannot invoke a skill that requires `github:write`. This preserves Bet #11's "agents cannot silently expand their own authority" invariant — the substrate enforces it at skill-invocation time.

The catalog discipline from Bet #6 still decides what gets _formalized_ as a Tier N mission type. Skills in L3 are not missions; they are the compressed experience missions draw on.

This is the Voyager pattern (Wang et al., NeurIPS 2023 — LLM agent in Minecraft with an automatic curriculum and a skill library of verified programs) wrapped in OP's capability model. The primitive is not novel; the composition with capability scoping, governance class, and the canonical-catalog gate is.

### 7. Tools are typed, bounded, and inspectable

A raw shell is not a finished tool surface. Tools are typed, sandboxed, capability-aware, and MCP-compatible at the protocol boundary.

### 8. Replay is a user feature

When an agent behaves unexpectedly, the winning system is the one that can explain and replay the behavior, not the one that hides it.

### 9. Protocol boundaries matter

Open Pincery owns its substrate, not every adjacent system.

- MCP outward to tools (year one).
- MCP / A2A inward to peers (year three, forward-compat constraint on year-one auth and mission-record shape).
- A pincer protocol inward to reasoners.
- Thin RPC or HTTP boundaries to ingress peers like OpenClaw.

### 10. The substrate is sovereign by default

No vendor has privileged architectural access. The reasoner is an abstraction across four axes: **provider** (who serves inference), **model** (what weights), **data-governance class** (sovereign / enterprise-bounded / vendor-consumer), and **role** (security-review / docs-draft / exploratory-planning / etc.). Each mission type's acceptance contract declares a minimum governance class; the substrate enforces it. Each reasoner call carries a role; catalog config maps `(role, governance_class) → (provider, model)` so models can be swapped without changing mission code. Cheap open-weight models handle cheap roles; frontier models handle risk-concentrated roles; the mapping is operator configuration, not hardcoded.

The reasoner proxy supports long-duration streaming requests as a named substrate requirement, not as a later patch. A frontier reasoning model that takes minutes to respond on a hard problem must complete cleanly through the proxy; any default HTTP idle timeout that silently breaks this assumption is a substrate bug.

### 11. A substrate rich enough to run a company, not just a task

The substrate must be primitive-rich enough that **an operator** can compose a roster of pincers — each with narrow authority, each running missions against acceptance contracts — that together run the business. This is what separates Open Pincery from the 2026 competitive set:

- **Task runners** (Devin, Cursor Background Agents, ChatGPT Agent, Zapier Agents, Lindy, Claude Cowork / Dispatch) ship no substrate. Each run starts near-zero or at session-scoped memory; tasks do not compose into _"my business is running."_ Dispatch is worth naming separately: it runs on the operator's own machine with local memory — closer to sovereign than most — but the unit of work is still a task, not a mission with an acceptance contract and a replayable trail.
- **Framework runtimes** (LangGraph Platform, Bedrock AgentCore, Cloudflare Agents) ship primitives and expect the operator to design the substrate. A solo founder with time to design a substrate would not need the product.

Open Pincery ships opinions strong enough to bootstrap a company and primitives generic enough to survive stronger models. The test: _can a solo operator, in an afternoon, declare a catalog of mission types, grant each a narrow capability scope and budget, point them at the operator's event streams (PRs, inbox, commitments, repos), and get a company — not a pile of tasks?_ If the answer is no, the substrate is too thin. If the primitives needed to make the answer yes are also 2026-model-specific opinions, the substrate is too thick. The point is the narrow ridge between those two failures.

Pincers do not compose pincers. The operator composes the roster; pincers run missions. Composition between pincers happens through the event log and operator-authored mission chains (mission A's completion event triggers mission B's creation), never through direct pincer-to-pincer messaging or pincer-authored pincer creation. This is an invariant, not a convention — it lives on the list in Bet #12, and the reasoning for holding it as the v7 default (with framing B as the likely v8/v9 relaxation) is recorded under Decisions Carried Into v7, D2.

### 11a. Execution environment is a first-class axis of authority

This bet is a direct corollary of #11: a pincer that runs a mission must be **granted** somewhere to run, not only _what it may call_ and _how much it may spend_. A pincer therefore receives three axes of authority from the operator (or from the catalog configuration for its mission type):

- **Capabilities** — what tools and credentials it may use (Bet #3).
- **Budget** — dollars and wall-clock ceiling (Bet #4).
- **Execution environment** — a sandbox with declared filesystem, network, and compute limits _distinct from the substrate host_.

Agent-authored code never executes on the substrate host. Every exploratory mission, every step that clones a repo and runs tests, every code-generation step runs inside the concrete sandbox primitives already committed to in the security architecture:

- **Zerobox** — Layer 1, per-tool/per-process sandbox. Rust SDK, ~10ms overhead, Bubblewrap+Seccomp on Linux / Seatbelt on macOS. Deny-by-default filesystem, network, and environment. Proxy-mediated secret injection (see Bet #3). The runtime constructs a Zerobox sandbox for every tool execution inside the wake loop.
- **Greywall** — Layer 4, host-level sandbox wrapping the Open Pincery binary itself. Defense in depth: if Zerobox is bypassed, Greywall constrains the whole runtime. Deployment-layer concern (the operator launches `greywall -- ./open-pincery serve`); no Rust code change required to gain the defense.

The sandbox is disposable per step or per session; state that must survive lives in the event log, not in the sandbox. This is what makes running Open Pincery overnight on the operator's own machine safe — without it, every code-executing mission is either a host compromise or a punt back to the operator.

The runtime shape this implies decomposes into three named primitives that should not be conflated in design discussions:

- **Pincer** — the long-lived, addressable, stateful actor. Holds working memory and open connections; survives restart; is woken by events (cron, inbound message, mission delegation, scheduled self-wake).
- **Mission** — the durable multi-step graph with retries, an acceptance contract, an accountable human, and an event log. Survives the death of any individual pincer.
- **Sandbox** — Zerobox-backed ephemeral, capability-scoped per-step execution surface. No persistent identity. Disposable.

A pincer holds the session; a mission holds the workflow; a sandbox holds the per-step execution. Three primitives, not one conflated runtime.

### 12. The substrate encodes invariants, not opinions

Legal, security, economic, and engineering invariants belong in the primitives:

- An accountable human exists and is named (legal).
- The event log is complete and replayable (engineering).
- Capabilities mediate authority (security).
- Credentials live in the vault and reach the runtime only as proxy-injected placeholders (security).
- Missions have budgets (economic).
- Agent-authored code runs in a Zerobox sandbox, not on the substrate host (security).
- Pincers do not message other pincers directly, and do not create pincers. Composition between pincers happens through the event log and operator-authored mission chains (architectural).
- Agents cannot silently rewrite their own charter, grant themselves capabilities, raise their own budget, rotate their own secrets, or fake completion (behavioral invariants).

Behavioral conventions — signal types, delegation patterns, mission categories, Tier N catalogs — live _above_ the substrate and evolve with models. When in doubt, ask _"would this still be true with a 10x smarter reasoner?"_ If yes, it may be an invariant. If no, it is a convention; keep it out of the substrate primitives and let it live in the catalog, the contract, or the operator's configuration.

## Boundaries and Complements

- **OpenClaw** is a complement. It owns channel ingress, sessions, routing, mobile/chat surfaces. Open Pincery integrates rather than rebuilds.
- **MCP** is the outward tool boundary. Open Pincery exposes and consumes MCP rather than inventing a closed protocol.
- **Postgres** is the year-one memory substrate. pgvector joins in v7. CozoDB joins when a Tier 1 mission's acceptance contract requires a query Postgres recursive CTEs cannot answer cleanly.

## Non-Goals

The current direction explicitly does not prioritize:

1. A React-heavy product UI.
2. Workflow-graph authoring.
3. A large first-party integrations catalog.
4. Broad enterprise governance features before Tier 1 is solid.
5. Marketplace mechanics before the substrate proves itself.
6. Speculative missions built for hypothetical buyers rather than real founder pain.
7. Generic chat-to-anything assistant behavior.
8. Any architecture that assumes user data flows through vendor-consumer endpoints.
9. Reasoner features available only via a specific vendor's proprietary extension with no equivalent hyperscaler-enterprise or self-hosted path.
10. Legal accountability transfer from human to software. Every mission has a named accountable human. The software is never the accountable party.
11. Self-modifying acceptance contracts. An agent interprets, plans, and adapts strategy inside its contract; it cannot rewrite the contract itself. Only the accountable human can change what "done" means.
12. Self-granting capabilities or raising its own budget. An agent can request a capability grant or budget increase via escalation; it cannot silently expand its own authority.
13. Strong IS-invoked delegation (agent directs the human as subordinate, ownership transferred) in year one. Weak IS-invoked (agent suggests work for the human) and escalation are supported from day one.
14. Speculative AGI-research claims (machine consciousness, self-referential awareness in the philosophical sense). The substrate is engineering, not frontier research.

## Year-One Focus

Year one is the substrate plus Tier 1, founder-dogfooded daily.

1. **Pre-conditions**: executor hardening, LLM timeout, typed events, typed errors.
2. **Substrate spine**: event chaining, capabilities, missions, acceptance contracts, replay, evidence export, accountable-human on the mission record. (Delegation patterns are labeled on the catalog, not enumerated on the record.)
3. **Memory controller interface**: backend-agnostic shape with Postgres as first backend; pgvector slot designed for v7.
4. **Reasoner abstraction**: provider / model / governance-class tri-axis. At least one frontier-hosted implementation and one self-hosted open-weight implementation proven end-to-end against a Tier 1 mission.
5. **Tier 1 missions in order**: codebase steward → inbox triage → commitments tracker → weekly digest. Each declares a minimum governance class (invariant, contract-level) and a delegation-pattern label (convention, catalog-level).
6. **MCP outward** adopted on a parallel low-cost track.
7. **Operate the founder's company on Open Pincery** for the benchmark period.

Pincer protocol extraction and OpenClaw integration are year-two unless they unblock a Tier 1 mission.

## The Benchmark That Ends Arguments

The project optimizes toward one artifact:

> The founder ran Open Pincery as the operating substrate of a one-person company for 90 continuous days. Here is the mission log, here are the escalations, here is what shipped, here is what the agents caught that a human would have missed, here is every budget boundary and capability refusal, and here is the replay trail behind every material action.

The benchmark runs each Tier 1 mission at or above the minimum governance class its contract declares. No Tier 1 mission runs against vendor-consumer endpoints during the benchmark.

**N=1 caveat.** The founder benchmark is necessary but not sufficient — it is N=1 with maximum selection bias. The year-two gate is stricter:

> At least three non-founder operators each running the Tier 1 catalog continuously for at least 30 days, with a public dashboard of per-acceptance-criterion pass rates.

Failing to stand up that external cohort within six months of the 90-day founder benchmark completing is a governance failure. If the software only works for the founder, it is not professional software.

## Bootstrap Provenance and the Sovereignty Ladder

Year-one development of Open Pincery runs on VS Code agent mode using frontier hosted models. This is a vendor-consumer reasoning surface used in the authoring loop of a project whose thesis is that vendor-consumer reasoning surfaces are unsuitable as architectural assumptions for running a company.

This is not a contradiction. It is the standard pattern: Linux was developed on Minix and proprietary Unix, Git on BitKeeper, Rust in OCaml. The replacement substrate is built with the best available tools; the transition off those tools is a named, tracked goal.

The ladder is a concrete, falsifiable staircase. The project can at any time state which stage it is at.

| Stage | Name                                             | Characterization                                                                                                                                                                                                                                                                     |
| ----- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 0     | Vendor-consumer authoring (current)              | Written with VS Code agent mode + frontier hosted models. No reasoner abstraction yet. No Tier 1 mission runs yet. Acknowledged, not celebrated.                                                                                                                                     |
| 1     | Substrate sovereign-capable                      | Substrate spine + reasoner abstraction exist. At least one Tier 1 mission runs end-to-end. That mission declares a minimum governance class and is verified against both a frontier-hosted reasoner and a self-hosted open-weight reasoner. Authoring still uses VS Code agent mode. |
| 2     | Codebase steward runs on Open Pincery's own repo | Codebase steward deployed against this repository: real PR reviews, release notes, dependency hygiene, security-sensitive-file flagging.                                                                                                                                             |
| 3     | Authoring loop migrates off vendor-consumer      | Primary development assistance shifts to Open Pincery's own agents at enterprise-bounded or sovereign class. VS Code agent mode is optional fallback.                                                                                                                                |
| 4     | Sovereign-default authoring                      | Authoring loop runs on self-hosted open-weight models by default. Open Pincery develops Open Pincery under its own stated sovereignty terms.                                                                                                                                         |

Stages 0–2 are year-one scope. Stages 3–4 are year-two or later. Staying at Stage 0 past the 90-day benchmark without an explicit decision to do so is a governance failure.

## Tripwires

The strategy is only as good as the operational guardrails that catch it drifting. A tripwire is a signal that an assumption has broken; each has an owner, a cadence, and a required response.

| Tripwire                                    | Signal                                                                                                                                         | Required response                                                                                                                                                                                        |
| ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| Mission catalog discipline breaks           | Founder performs the same manual task three times without either formalizing a mission or recording a decision not to.                         | Within one week: formalize a mission type + contract, or write an ADR-style note deferring it explicitly.                                                                                                |
| Reasoner sovereignty erodes                 | A mission type ships whose acceptance contract cannot be satisfied without a specific vendor's proprietary endpoint.                           | Either define an equivalent path in the enterprise-bounded or sovereign class, or mark the mission as deferred.                                                                                          |
| Bootstrap ladder stalls at Stage 0          | 90-day founder benchmark completes and Stage 2 work has not started within 30 days.                                                            | Explicit decision: commit to Stage 2 with timeline, or publicly record the project as remaining at Stage 0 with reasons.                                                                                 |
| Year-two external cohort fails to form      | Six months after 90-day benchmark completes, fewer than three non-founder operators are running Tier 1 in a measurable way.                    | Either redirect effort to reduce onboarding friction, or reassess the "professional software for any like-founder" claim.                                                                                |
| Market slots us as task runner              | More than 30% of outbound conversations classify OP as a Devin- or Dispatch-class task runner before the substrate-runtime framing lands.      | Rewrite public-facing one-liner and sequence of first examples; the three-part category claim is not optional marketing.                                                                                 |
| Competitor adds persistent memory credibly  | A framework or hosted runtime ships event-sourced, replayable, operator-owned memory that is exportable and queryable outside the runtime.     | Evaluate whether OP's differentiation has moved; update peer comparison and reassess first wedge. The next differentiator is _memory the operator can subpoena and replay independently of the runtime._ |
| Hyperscaler enterprise terms drift          | Bedrock, Azure AI, or Vertex changes no-training / no-retention posture adversely.                                                             | Re-evaluate enterprise-bounded class recommendations; accelerate self-hosted open-weight path.                                                                                                           |
| Open-weight frontier closes the quality gap | A sovereign-class reasoner becomes adequate for Tier 1 missions currently requiring enterprise-bounded.                                        | Accelerate Stage 4 work; revise governance-class defaults on affected missions.                                                                                                                          |
| Sandbox escape or host-execution incident   | A pincer executes code directly on the substrate host, or a sandboxed pincer reaches outside its declared filesystem/network/compute envelope. | Halt affected mission types, audit the event log for breach scope, harden the sandbox implementation before resuming. This is a Bet #11a invariant; treat a breach as a security incident, not a bug.    |     | Skill tree rots | A crystallized L3 skill causes mission failures more than twice, or the skill tree grows past a size where the memory controller's recall quality measurably drops. | Prune the offending skills; require the pincer to re-crystallize from fresh evidence. Skills are auto-promoted but not sacred. |
| Context budget drifts up                    | Sustained per-call context exceeds the memory controller's declared ceiling for the governance class across a rolling window.                  | Investigate whether skills are being over-loaded, projections are too wide, or the Insight Index is degraded. Stalled missions are often context-drowned missions.                                       |

Review this document quarterly or immediately when any tripwire fires.

## Technical Advice Absorbed

External technical sources reviewed during the v6 cycle; the highest-leverage takeaways are captured here so the bets above carry their weight. Full extractions live in the companion notes.

**From Stonebraker on DBOS ([stonebraker-dbos-notes-2026-04.md](stonebraker-dbos-notes-2026-04.md)):**

- The read-write inflection for agentic AI is coming and will demand ACID semantics across agents. Today's read-heavy Tier 1 missions get away with soft consistency; the catalog will drift read-write within two to three releases and the substrate's transactional guarantees will become load-bearing.
- Put state in the database, then engineer around it. Validates the Bet #2 memory-as-substrate choice from a senior operator who arrived independently at the same line of reasoning.
- Atomic multi-step missions ("the whole workflow either finishes or looks like it never happened") are the right target for the mission layer. Current substrate offers durable-step; atomic-mission is a known gap.
- Do not expose raw text-to-SQL over the event log to reasoners. Structured recall primitives over narrow views outperform free-form NL-to-SQL; LLMs score 0% on real warehouses and 35% even with the FROM clause supplied.
- Structured-data joins belong in SQL, not in LLM context. When multiple memory backends are in play (Postgres, pgvector, CozoDB), the memory controller does the join; the reasoner asks narrow questions.
- Do not adopt DBOS as a dependency. Watch it for primitive ideas (atomic-workflow semantics, `@step` ergonomics, Conductor operator affordances) worth porting into the Rust substrate.

**From Cloudflare's internal AI platform ([cloudflare-ai-infra-notes-2026-04.md](cloudflare-ai-infra-notes-2026-04.md)):**

- Tool-definition context cost is a real design constraint. Exposing N typed tools directly through MCP will hit the same ~7.5%-per-request ceiling Cloudflare documented at 34 tools. Code-Mode-style lazy tool discovery (a search tool and an execute tool, catalog loaded on demand) is the proven fix and belongs in Bet #9's MCP surface design.
- A repo's `AGENTS.md` is the direct analog of a mission acceptance contract: both exist because agents produce plausible-but-wrong output when local context is implicit. The codebase-steward Tier 1 mission should consume and maintain the repo's `AGENTS.md` as part of its acceptance contract.
- Config-as-code, compiled at deploy time, with local override wins. Tier 1 missions should be authored as structured files (YAML frontmatter + markdown charter), compiled into a validated catalog at build, with operator-local overrides layered on top. No hand-editing a canonical catalog at runtime.
- Open-weight on sovereign inference is 77% cheaper at a 7B-tokens/day workload (Cloudflare's single security agent). The Sovereignty Ladder's Stage 4 is as much a cost-survival feature as a data-governance feature; the solo founder running persistent agents at even a fraction of that volume cannot economically route everything through frontier endpoints.
- **Ephemeral sandboxes as a first-class primitive** (now Bet #11a, with Zerobox + Greywall as the concrete implementations). Cloudflare's Dynamic Workers + Sandbox SDK pattern is the missing axis of authority: agent-authored code runs disposably, not on the host. Without this, every code-executing mission is either a host compromise or a punt. This is now an invariant on #12.
- **Acceptance contracts as skills with stable rule IDs** (extension of Bet #5). Cloudflare's Engineering Codex ships standards as (a) machine-citable rules of the shape _"use X if doing Y"_ and (b) an agent skill with progressive disclosure — the same source consumed by human author, reviewing agent, and CI. Open Pincery should author Tier 1 acceptance contracts as skills under `.github/skills/contracts/<mission>/` with stable rule IDs citable in review output and escalations.
- **Role as a fourth axis of the reasoner abstraction** (folded into Bet #10). Cheap open-weight models handle cheap roles (docs review, summarization); frontier models handle risk-concentrated roles (security review, architectural changes). The mapping from `(role, governance_class)` to `(provider, model)` is operator config, not hardcoded.
- **Watch item, not yet a bet:** a substrate-level catalog of the _operator's owned systems_ (not just pincers and missions). At one-person scale this lives in the founder's head and the repo layout; at two-person scale it becomes load-bearing. Promote to a bet if v8–v9 organically grows one.
- **What OP does _not_ adopt from Cloudflare's reviewer coordinator:** the classify-and-fanout pattern as agent-to-agent delegation. Cloudflare's reviewer classifies an MR into a risk tier, then delegates to named specialist _agents_ (code quality, security, codex compliance, docs, performance, release impact). OP cannot model that as pincer-to-pincer delegation without violating Bet #12. Where the multi-pass shape is useful, OP runs the passes inside a single mission with multiple reasoner calls, each carrying its own role and governance class (Bet #10). The classification + multi-pass shape is still valuable; the multi-agent implementation is not.

**From GenericAgent ([genericagent-notes-2026-04.md](genericagent-notes-2026-04.md)):**

- **Auto-crystallized pincer skill trees** (now Bet #6a, distinct from the canonical catalog). Each successful novel sub-task crystallizes its execution path into memory layer L3 as a reusable skill. This is how pincers stay relentless in byte-mediated work — the struggle is recovered, not repeated. The canonical catalog (Bet #6, operator-gated, three-repetition rule) is unchanged; skill-tree growth is the layer beneath it.
- **Layered memory L0–L4** (folded into Bet #2). Rules, Index, Operator facts, Pincer skills, Mission archives — five layers with distinct cadences, all sitting above the event log as ground truth. The memory controller is the sole crossing point; context budget is its responsibility.
- **A `code_run` primitive is the natural op against Bet #11a's sandbox.** The sandbox is the container; `code_run` is the operation. A pincer granted `code_run` inside a capability-scoped sandbox can dynamically install packages, write scripts, and extend itself — which is what keeps missions from stalling on "no tool for this." Spec this as a first-class capability; do not conflate with "the sandbox exists."
- **Real-browser-session authority is a capability-model test case.** Granting "use my browser" wholesale is ambient authority. The capability grant must be domain-scoped and session-scoped; the pincer emits a signal when attempting to navigate outside its declared domain list. This is a concrete design case Bet #3 must handle.
- **Context-budget discipline is a memory-controller responsibility, not an agent virtue.** GenericAgent sustains <30K tokens by construction. OP should enforce a per-call context budget via the memory controller; pincers do not choose their own context size. This is a property the controller enforces on behalf of every Professional Bar item; stalled missions are often context-drowned missions.
- **Agent-loop line count as an informal tripwire.** If the wake loop grows much beyond a page, complexity is leaking out of skills and memory (where it belongs) into the loop (where it doesn't). Watch it.

**From the Cloudflare Agents SDK ([cloudflare-agents-sdk-notes-2026-04.md](cloudflare-agents-sdk-notes-2026-04.md)):**

- **Pincer-as-actor is the right runtime mental model** (now elaborated in Bet #11a). A pincer is a long-lived, addressable, stateful actor whose state survives restart — not a request handler, not a job, not a process. The Durable-Object shape Cloudflare ships externally is the cleanest articulation of this primitive in public.
- **Session / mission / sandbox is a clean three-way decomposition** (now in Bet #11a). Cloudflare separates session (live agent context), workflow (durable multi-step graph with retries), and sandbox (per-step execution surface). OP already has all three; naming them as a triad makes design discussions cheaper and prevents one being overloaded into another.
- **Long-running reasoning model support is a substrate requirement, not a patch** (now in Bet #10). Any proxy that times out at the default HTTP idle limit silently breaks the reasoner-abstraction claim the moment a frontier reasoning model takes four minutes on a hard problem.
- **Inbound email is a pincer wake event, full stop** (watch item). Any future CRM-responder, intake-triage, or support-first-pass mission should be designed assuming an incoming email to a designated address is a capability-scoped wake event with the email in the payload, not a separate IMAP poller. The substrate should accept inbound email as an event source before the first such mission ships.
- **Per-pincer SQL is a live design question for Bet #2.** Cloudflare demonstrates that per-agent databases scale. The memory controller interface should leave room for a pincer's state to migrate to its own per-pincer store if isolation or load demands it; the controller should be the only thing that cares.
- **What OP does _not_ adopt from the Agents SDK:** `@callable` RPC between agents, coordinator-as-agent delegating to specialist agents, addressed agent-to-agent messaging. These violate the Bet #12 invariant that pincers do not talk to pincers. OP-side coordination happens through the event log, not through addressed agent calls. Cloudflare's internal reviewer coordinator (one agent fanning out to specialist agents) is also out of scope for the same reason; where the pattern is useful (a code review that needs security, style, and performance passes), OP runs those passes inside a single mission with multiple reasoner calls at different roles and governance classes.

**From the agent-harness landscape and karpathy/autoresearch ([agent-harness-landscape-2026-04.md](agent-harness-landscape-2026-04.md)):**

- **OP's bets are a composition of public primitives, not novel inventions.** ReAct, Toolformer, Self-Refine, Self-Consistency, Constitutional AI, Reflexion, Voyager, DSPy/GEPA — every primitive OP relies on already exists in the literature. The novelty is the composition into a sovereign substrate with explicit authority bounds. State this honestly when defending the bets; it improves the argument, not weakens it.
- **Fixed-budget experiment loops are the evidence mechanism OP currently lacks.** Acceptance contracts produce pass/fail; benchmark runs produce _distributions_ over attempts. Distributions are what let an operator argue one stack is better than another. autoresearch's design — fixed wall-clock budget per attempt, one editable surface, ~100 attempts overnight, append-only results log — is a model worth borrowing for an eventual benchmark mission family.
- **Autonomous Overnight Benchmark is a future mission family worth naming now** (not yet a Tier 1 mission). A pincer whose subject is _other pincers_, running fixed-budget attempts against a locked fixture and producing comparable experiment logs, is the missing feedback loop between "we wrote a pincer" and "we promoted it to Tier N." Earliest plausible landing is v9, after Bet #11a sandboxes are real and at least two Tier 1 missions are stable. Without it, promotion stays operator intuition.
- **Two-clock authoring model: operator-owned contracts vs agent-owned skills.** autoresearch's `program.md` (human, weekly) / `train.py` (agent, hourly) decomposition validates the catalog convention OP is already converging on: acceptance contracts are operator-owned and slow-changing; crystallized skills are pincer-owned and fast-changing. Make the two clocks explicit when the catalog conventions are written.
- **Reflexion-style verbal self-critique inside the wake-summary loop is a cheap quality uplift.** One extra reasoner call per mission that conditions the next attempt on "what failed last time and why" is plausibly larger delta than another capability. Worth a prototype inside a Tier 1 mission before it becomes a substrate-level claim.
- **DSPy/GEPA-style prompt compilation refines how Bet #5 and Bet #6a compose.** Prompts can be _compiled_ from an acceptance contract's metric and dev set rather than hand-authored. The compiled prompt becomes a versioned artifact in memory (likely L2 or L3) and is subject to the same crystallization / pruning rules as any other skill. Not yet a substrate commitment; flag for Bet #5 design.

## Decisions Carried Into v7

Two design questions surfaced during the v6 cycle that could have become rabbit holes. Both are decided below on a reversibility-first reading of the mission — _sovereign, auditable, replayable substrate for a solo operator._ Either decision is revisitable; the choice is which direction is cheaper to revisit.

### D1. Operator surface is mission console + signal inbox + vault. No chat primitive.

**Decision.** The v7 operator UI does not ship a free-text "chat with a pincer" surface. The three load-bearing surfaces are:

- **Mission console** — launches missions against catalog entries, shows running missions, shows completed missions with their acceptance-contract results and event-log links.
- **Signal inbox** — pincer-to-operator escalations, operator responses to signals, delivery-policy configuration. Free-text communication happens here as a _signal payload_, not as a standalone primitive.
- **Vault** — the only UI surface that accepts secret material. Out-of-band, encrypted at rest, `list_credentials` returns names only (per Bet #3 and the TLA spec).

The Signals section's earlier mention of "chat" as a delivery policy option is retired; the delivery-policy set is `{signal inbox, email, morning digest, synchronous callback}`.

**Reversibility reasoning.** Asymmetric consequences.

- Adding a chat surface _later_ if operators genuinely miss it is a weekend of UI work, bounded in scope.
- Removing chat _after_ operators have pasted secrets, API keys, or PII into it is structurally impossible — those tokens are in the event log permanently, indexed and retrievable.
- The Bet #3 invariant ("secrets reach the runtime only as proxy-injected placeholders; the reasoner never sees raw secret material") becomes _mechanically_ enforceable if no chat text field exists. With a chat field, enforcement collapses onto model-behavior and system-prompt refusal, which is one regression away from a permanent leak.

**When to revisit.** If three or more Tier 1 operators independently request a conversational surface _and_ a mechanism exists to keep secrets out of the chat event stream at the substrate level (not the prompt level), reopen as a proposed bet. Not before.

### D2. Pincers do not create pincers. Operator-authored mission chains only (for v7).

**Decision.** For v7, Bet #12's invariant stands: pincers do not message other pincers directly and do not create pincers. Composition between pincers happens through the event log and operator-authored mission chains (mission A's completion event triggers mission B's creation, with A and B pre-wired by the operator in the catalog). Multi-role work inside a single mission runs as multiple reasoner calls at different roles and governance classes (Bet #10), not as pincer-to-pincer delegation.

**Reversibility reasoning.** The Q2 discussion surfaced three framings: hard invariant (A), catalog-mediated spawning (B), operator-approved runtime spawning (C), plus a hybrid direct-call-with-event-log-recording (D). CS theory (end-to-end arguments, CSP, capability security, event-sourcing, what TLA+ can actually verify) leans toward A or B; developer-ergonomics arguments lean toward B, C, or D. The deciding factor is asymmetric commitment cost:

- **Starting with A and relaxing to B later** is a small future bet: add a `MissionFanoutRequested` event type, add a `may_fan_out_to` field on catalog entries, add one state transition in the substrate. The existing event log accommodates it natively; existing missions keep running unchanged.
- **Starting with B and walking back to A** is impossible in practice: the substrate has committed surface area to concurrent pincer orchestration, the TLA spec models it, the event schema encodes it, and every mission authored against B breaks if it is removed.
- **D (direct `@call` between pincers with an event-log record)** is theoretically weakest — the interaction state lives in reasoner working memory mid-call, which is precisely what event-sourcing is supposed to make impossible — and is also the hardest to walk back once shipped.

A also aligns with the "prove one Tier 1 mission works end-to-end before shipping coordination infrastructure" discipline: coordination primitives should land _after_ a real mission has demonstrated the ergonomic need for them, not before.

**When to revisit (v8 or v9).** Reopen as a proposed bet if any of the following hold:

- A concrete Tier 1 mission demonstrably stalls or escalates more than three times because it could not fan out to a sibling pincer, _and_ the fan-out shape is stable enough to pre-declare in the catalog (favors B).
- An external operator runs the Tier 1 catalog and reports the same pattern independently.
- A sandbox-escape or authority-laundering incident disqualifies any variant of B / C / D permanently (locks in A).

Until one of those triggers fires, A is the default and the substrate does not ship spawn primitives.

## Companion Docs

- [stonebraker-dbos-notes-2026-04.md](stonebraker-dbos-notes-2026-04.md) — extracted technical claims from the Stonebraker interview; live input, maintained.
- [cloudflare-ai-infra-notes-2026-04.md](cloudflare-ai-infra-notes-2026-04.md) — extracted technical claims from the Cloudflare blog; live input, maintained.
- [cloudflare-agents-sdk-notes-2026-04.md](cloudflare-agents-sdk-notes-2026-04.md) — extracted technical claims from the Cloudflare Agents SDK docs; live input, maintained.
- [genericagent-notes-2026-04.md](genericagent-notes-2026-04.md) — extracted technical claims from the GenericAgent project; live input, maintained.
- [agent-harness-landscape-2026-04.md](agent-harness-landscape-2026-04.md) — peer-harness survey, theoretical-spine map, autoresearch design analysis, and the Deepresearcher / Autonomous Overnight Benchmark proposal; live input, maintained.
- `docs/input/v6_pre_iterate/strategic-answers-2026-04.md` — D1–D10 opinionated answers with citations; provenance only.
- `docs/input/v6_pre_iterate/tripwires-2026-04.md` — extended tripwire context; superseded by the condensed table above but preserved for narrative.
- `docs/input/v6_pre_iterate/agent-taxonomy-2026-04.md` — Category 5 reasoning; superseded by the Category Claim section above.
- `docs/input/v6_pre_iterate/research-synthesis-2026-04.md` — academic + IS research grounding; still live as the rationale for memory-as-substrate, accountable-human, and the dual-taxonomy claim.
- `docs/input/v6_pre_iterate/first-principles-assessment.md` — earliest thinking record; superseded by this doc.
- `docs/input/v6_pre_iterate/dbos_mike_stonebraker_transcript.txt` — raw transcript; provenance.
- `docs/input/v6_pre_iterate/cloudflare_ai_infra.txt` — raw blog text; provenance.

## v6 Disposition

When the v6 scaffolding cycle runs, this doc:

1. Becomes `docs/reference/north-star.md` (canonical, no date suffix).
2. Supersedes all five precursor docs in `docs/input/v6_pre_iterate/` as the single source of truth.
3. Feeds the v6 `design.md` directly on three points: memory controller interface, mission-record schema (including `accountable_human` as an invariant field), forward-compat hooks for pgvector and CozoDB.
4. Drives v6 ACs of the form _"north-star states X in ≤N sentences"_ — documentation-level, not code-level.

v6 ships no code. v6 is the ground floor v7–v12 build against.
