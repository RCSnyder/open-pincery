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
2. Inside a mission, execution is open-ended and materially capable. The agent can decompose a problem, spawn sub-pincers, call tools for hours or days, write and execute code, provision infrastructure, spin up auxiliary services, rotate its own secrets within scope, and produce arbitrarily large artifacts — anything the granted capabilities and remaining budget permit.
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

The operator sets the delivery policy (chat, email, morning digest, synchronous callback). The substrate never silently times out a mission waiting on a human response — that would be a failure of the substrate, not the agent.

## Durable Bets

These are the stable bets that should survive individual iteration changes.

### 1. The event log is the product

The log is not debug exhaust. It is a first-class artifact: typed, queryable, replayable, exportable.

### 2. Memory is the substrate, not the reasoner

The differentiating primitive is the coordinated memory controller — parametric (model weights), working (context), external (event log + projections + vector + graph). Reasoners are pluggable; memory is the thing we own. The memory controller interface is backend-agnostic by design:

- **v7:** Postgres + pgvector for semantic recall.
- **v10-ish:** graph substrate for relational queries over cross-mission history. Target: **CozoDB embedded** (Rust, single-binary, Datalog, vector-capable) — chosen to preserve the one-Rust-binary-plus-one-database sovereign-host story. SurrealDB, IndraDB, Memgraph, Neo4j were considered and rejected on sovereignty, language, or deployment grounds.

### 3. Capability-scoped credentials beat ambient authority

Agents do not inherit broad env-var authority. They receive narrow, auditable capabilities that can be delegated, rotated, and revoked. An agent granted `provision:database` can spin up a Postgres instance; an agent granted `read:inbox` cannot. Within its granted capabilities the agent acts with full initiative. Outside them it must escalate to request a capability grant — it cannot silently expand its own authority. Blast radius matters more when one person is the whole company.

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

No vendor has privileged architectural access. The reasoner is an abstraction across three axes: **provider** (who serves inference), **model** (what weights), and **data-governance class** (sovereign / enterprise-bounded / vendor-consumer). Each mission type's acceptance contract declares a minimum governance class; the substrate enforces it.

### 11. A single pincer should be able to build the rest

The substrate must be primitive-rich enough that, given capabilities and a budget, one pincer can author mission types, instantiate sub-pincers, scope their authority, and hand missions to them — all inside the invariants. This is what separates Open Pincery from the 2026 competitive set:

- **Task runners** (Devin, Cursor Background Agents, ChatGPT Agent, Zapier Agents, Lindy, Claude Cowork / Dispatch) ship no substrate. Each run starts near-zero or at session-scoped memory; tasks do not compose into _"my business is running."_ Dispatch is worth naming separately: it runs on the operator's own machine with local memory — closer to sovereign than most — but the unit of work is still a task, not a mission with an acceptance contract and a replayable trail.
- **Framework runtimes** (LangGraph Platform, Bedrock AgentCore, Cloudflare Agents) ship primitives and expect the operator to design the substrate. A solo founder with time to design a substrate would not need the product.

Open Pincery ships opinions strong enough to bootstrap a company and primitives generic enough to survive stronger models. The test: _can a solo operator say "create a pincer, and have it build the thing that runs my inbox, and have that thing spin up its sub-pincers for the pieces it needs" — and get a company, not a pile of tasks?_ If the answer is no, the substrate is too thin. If the primitives needed to make the answer yes are also 2026-model-specific opinions, the substrate is too thick. The point is the narrow ridge between those two failures.

### 12. The substrate encodes invariants, not opinions

Legal, security, economic, and engineering invariants belong in the primitives:

- An accountable human exists and is named (legal).
- The event log is complete and replayable (engineering).
- Capabilities mediate authority (security).
- Missions have budgets (economic).
- Agents cannot silently rewrite their own charter, grant themselves capabilities, raise their own budget, or fake completion (behavioral invariants).

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

| Tripwire                                    | Signal                                                                                                                                     | Required response                                                                                                                                                                                        |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Mission catalog discipline breaks           | Founder performs the same manual task three times without either formalizing a mission or recording a decision not to.                     | Within one week: formalize a mission type + contract, or write an ADR-style note deferring it explicitly.                                                                                                |
| Reasoner sovereignty erodes                 | A mission type ships whose acceptance contract cannot be satisfied without a specific vendor's proprietary endpoint.                       | Either define an equivalent path in the enterprise-bounded or sovereign class, or mark the mission as deferred.                                                                                          |
| Bootstrap ladder stalls at Stage 0          | 90-day founder benchmark completes and Stage 2 work has not started within 30 days.                                                        | Explicit decision: commit to Stage 2 with timeline, or publicly record the project as remaining at Stage 0 with reasons.                                                                                 |
| Year-two external cohort fails to form      | Six months after 90-day benchmark completes, fewer than three non-founder operators are running Tier 1 in a measurable way.                | Either redirect effort to reduce onboarding friction, or reassess the "professional software for any like-founder" claim.                                                                                |
| Market slots us as task runner              | More than 30% of outbound conversations classify OP as a Devin- or Dispatch-class task runner before the substrate-runtime framing lands.  | Rewrite public-facing one-liner and sequence of first examples; the three-part category claim is not optional marketing.                                                                                 |
| Competitor adds persistent memory credibly  | A framework or hosted runtime ships event-sourced, replayable, operator-owned memory that is exportable and queryable outside the runtime. | Evaluate whether OP's differentiation has moved; update peer comparison and reassess first wedge. The next differentiator is _memory the operator can subpoena and replay independently of the runtime._ |
| Hyperscaler enterprise terms drift          | Bedrock, Azure AI, or Vertex changes no-training / no-retention posture adversely.                                                         | Re-evaluate enterprise-bounded class recommendations; accelerate self-hosted open-weight path.                                                                                                           |
| Open-weight frontier closes the quality gap | A sovereign-class reasoner becomes adequate for Tier 1 missions currently requiring enterprise-bounded.                                    | Accelerate Stage 4 work; revise governance-class defaults on affected missions.                                                                                                                          |

Review this document quarterly or immediately when any tripwire fires.

## Companion Docs

- `docs/input/v6_pre_iterate/strategic-answers-2026-04.md` — D1–D10 opinionated answers with citations; provenance only.
- `docs/input/v6_pre_iterate/tripwires-2026-04.md` — extended tripwire context; superseded by the condensed table above but preserved for narrative.
- `docs/input/v6_pre_iterate/agent-taxonomy-2026-04.md` — Category 5 reasoning; superseded by the Category Claim section above.
- `docs/input/v6_pre_iterate/research-synthesis-2026-04.md` — academic + IS research grounding; still live as the rationale for memory-as-substrate, accountable-human, and the dual-taxonomy claim.
- `docs/input/v6_pre_iterate/first-principles-assessment.md` — earliest thinking record; superseded by this doc.

## v6 Disposition

When the v6 scaffolding cycle runs, this doc:

1. Becomes `docs/reference/north-star.md` (canonical, no date suffix).
2. Supersedes all five precursor docs in `docs/input/v6_pre_iterate/` as the single source of truth.
3. Feeds the v6 `design.md` directly on three points: memory controller interface, mission-record schema (including `accountable_human` as an invariant field), forward-compat hooks for pgvector and CozoDB.
4. Drives v6 ACs of the form _"north-star states X in ≤N sentences"_ — documentation-level, not code-level.

v6 ships no code. v6 is the ground floor v7–v12 build against.
