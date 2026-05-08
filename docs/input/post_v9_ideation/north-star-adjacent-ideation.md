# North-Star-Adjacent Ideation

> **Status**: Strategic ideation, May 7 2026. Not a spec. Not a plan. A capture of three sessions of thinking-out-loud about where open-pincery sits in the agent-runtime landscape and what its strongest next move is. Treat as input to a future positioning decision, not as conclusions.

> **Revision**: 2026-05-07 evening. Reframed from "durable runtime for LLM-native actors" to "single-operator agent OS" after the user surfaced (a) an explicit positioning preference for OS-grade deploy/maintain/govern/secure properties over actor-theoretic correctness, and (b) PR #4 (`v6-01_implementation`, 238 commits, AC-34..AC-88), which provides the evidence that the work-already-shipped-on-branch is OS-shaped, not actor-shaped. The actor-lineage section (§4) is preserved as historical context but is no longer the load-bearing positioning.

> **Provenance**: This document was produced by an LLM-driven discovery session with R. Cooper Snyder (open-pincery author). Grounding sources: open-pincery README (verified), PR #4 description (verified), gbrain README (verified), gstack README (verified, via subagent), survey READMEs of ~18 agent-runtime projects (verified, via subagent), open-pincery source on `main` branch / v1.0.1 (verified, via subagent — but `main` is v5; PR #4 is v6→v9 and is _not_ yet sampled at the source-code level). All claims about open-pincery internals at v6+ are PR-description-derived and should be re-grounded against the `v6-01_implementation` branch before publication.

---

## 1. The framing in one sentence

> **Open-pincery is the open-source, self-hostable, single-operator agent OS — the substrate that lets one human deploy, supervise, secure, audit, and govern a fleet of durable LLM-native processes (pincers) on their own Postgres-backed kernel.**

Five load-bearing words: **single-operator**, **agent OS**, **Postgres-backed kernel**, **durable LLM-native processes**, **deploy/supervise/secure/audit/govern**.

No other project in the May 2026 landscape positions itself as an _OS_ for the operator-of-one. Cloudflare Durable Objects is a cloud. Temporal is a cluster. DBOS is a library. CrewAI/AutoGen/LangGraph are frameworks. **An OS — kernel surface, capability primitives, audit journal, supervised processes, secret store, sandboxed execution — for a single human running their own agent fleet — is a vacant category.**

### Why "OS" and not "runtime"

A runtime hosts code. An OS does that _plus_ govern, secure, audit, persist, and survive operator absence. Look at what's actually being built (PR #4 / v9):

- **Sandbox + seccomp + landlock** = process boundary (the kernel's job)
- **Capability nonces + capability gates** = capabilities system (POSIX caps / Capsicum / seL4 flavor)
- **Hash-chained audit log + startup gate + recovery runbook** = journaling / fsck (kernel responsibility)
- **AES-256-GCM credential vault** = keychain / secret store (OS service)
- **Prompt-injection defense + canary + jsonschema validation** = input-IDS at the AI boundary (kernel hardening)
- **Wake/sleep CAS + LISTEN/NOTIFY** = scheduler + IPC

The work is OS work. Has been all along. The actor-runtime framing was the wrong abstraction layer — actors are the _application model_, the OS is the layer below.

### What an "agent OS" promises an operator-of-one

The contract is: **deploy once, supervise lightly, trust the journal.** Specifically:

1. **Deploy**: a single command (`pcy bootstrap`) brings the OS up. Postgres, sandbox, vault, audit chain, all wired.
2. **Supervise**: lights-out tolerable. Pincers wake, fail, retry, report. Operator checks dashboard once a day.
3. **Maintain**: backup is `pg_dump`. Upgrade is one migration runner. Rollback is one transaction. No clusters.
4. **Provenance**: every event in the OS is hash-chained. Operator can prove what happened, when, by whom.
5. **Governance**: capabilities are explicit. A pincer cannot do what an operator hasn't granted. No ambient authority.
6. **Security**: every tool call is sandboxed. Every secret is vaulted. Every prompt is validated. Defense in depth.
7. **Continuity**: pincers persist across operator absence. Identity, work list, queue, schedule — all durable.

That's the OS contract. **It is what the existing 238-commit PR is delivering.** The framing finally matches the work.

---

## 2. The "anything is a pincer" thesis

A pincer is a continuous agent: durable identity, event log, wake/sleep, async messaging, runs on the open-pincery substrate. The user's intuition: anything that fits that shape **is** a pincer.

Examples:

- **brain-pincer** — wraps gbrain. Wakes on webhook, ingests, enriches, sleeps.
- **coding-pincer** — runs gstack-style methodology. Wakes on a task message, plans, ships, sleeps.
- **mailroom-pincer** — wakes on inbound email webhook, parses, dispatches to other pincers, replies.
- **lease-pincer** — wraps hopper. Wakes on demand, leases a GPU, exposes endpoint, watches budget, tears down, sleeps.
- **scheduler-pincer** — fires timers, dispatches recurring work to other pincers.
- **integrator-pincer** — adapter for external systems (Slack, Stripe, Linear). Translates webhooks → pincer messages.

The generalization is correct. It's also old: it is the actor model with an LLM inside. See §4.

---

## 3. The substrate / pincer dichotomy

Every feature question collapses to: **is this _the substrate_, or is it a _pincer_?**

| Goes in substrate                        | Goes in a pincer                          |
| ---------------------------------------- | ----------------------------------------- |
| Durable identity, event log, projections | Memory / knowledge graph                  |
| Wake/sleep CAS lifecycle                 | Methodology / planning                    |
| LISTEN/NOTIFY message dispatch           | Email / Slack / webhook adapters          |
| Multi-tenant isolation (RLS)             | GPU lease / external infra                |
| Tool-call replay semantics               | Domain skills (coding, support, research) |
| Approvals / suspend-on-event primitive   | Specific approval workflows               |
| Tracing / causal event IDs               | Cost reporting dashboards                 |
| Capability scoping / sandbox             | Specific tool implementations             |

**Rule**: things that _use_ the runtime are pincers. Things that _guarantee properties about_ the runtime are substrate.

This collapses the previous "primitives 1-12" list to maybe four real substrate primitives:

1. Tracing / causal event IDs
2. Approvals (suspend-wake-on-event)
3. Capability scoping
4. Tool-call replay semantics

Everything else is a first-party pincer that ships in `pincers/` or in a separate repo.

---

## 4. The historical lineage

The pincer thesis is not new vocabulary for a new idea. It is new vocabulary for a 50-year-old idea applied to a new computational unit (the LLM call).

| Year  | System                     | Author                   | Contribution to the lineage                                                   |
| ----- | -------------------------- | ------------------------ | ----------------------------------------------------------------------------- |
| 1973  | Actor model                | Hewitt                   | Private state + mailbox + behavior; async messages; identity persists         |
| 1973  | Unix processes + pipes     | Thompson/Ritchie/McIlroy | "Anything is a file"; small specialized programs composed                     |
| 1976  | Smalltalk                  | Kay                      | "The big idea is messaging"                                                   |
| 1978  | CSP                        | Hoare                    | Channel-based concurrency (the _other_ branch)                                |
| 1985  | Linda / tuple spaces       | Gelernter                | Coordination via shared writable space                                        |
| 1986  | Erlang/OTP                 | Armstrong                | Supervision trees, "let it crash", hot reload, nine-nines uptime              |
| ~1999 | TLA+                       | Lamport                  | Specifying concurrent systems before implementing                             |
| ~2006 | Event sourcing / CQRS      | Young / Fowler           | Append-only log of facts; state is a projection                               |
| 2009  | Akka                       | Lightbend                | Actor model on the JVM; Akka Persistence ≈ event-sourced actors               |
| 2010  | Microsoft Orleans          | MSR                      | "Virtual actors" / grains: auto-activate, auto-deactivate, identity-addressed |
| 2019  | Temporal                   | Fitzpatrick et al.       | Durable workflow execution with deterministic replay                          |
| 2020  | Cloudflare Durable Objects | Cloudflare               | Virtual actors in V8 isolates                                                 |
| 2024+ | DBOS Transact              | Stonebraker et al.       | Postgres-native durable execution                                             |
| 2026  | open-pincery               | RCSnyder                 | Event-sourced LLM-native actors on Postgres                                   |

**One-liner positioning**: _"DBOS's deployment model + Temporal's correctness model + an actor-with-LLM as the unit of computation."_

### What the lineage tells you to do

- **Don't conflate substrate with applications** (Smalltalk's lesson)
- **Don't make authoring hard** (Erlang's lesson — brilliant runtime, 20-year adoption gap)
- **Don't hide the formal model — but don't show it to users** (Lamport's lesson)
- **Don't fight the messaging primitive** (Kay's regret about C++)
- **Make the unit of computation obvious and small** (Unix's lesson)
- **Sell to engineers who care about correctness, not to people watching demos** (Temporal's positioning)

---

## 5. The competitive landscape (May 2026, surveyed)

```
                           Substrate / infra-shaped
                                    ▲
                                    │
                Temporal ───┐       │       ┌─── Restate
                            │       │       │
                Cloudflare ─┤       │       ├─── Inngest
                Durable Obj │       │       │
                            │       │       │
                            DBOS ───┼─── open-pincery
                            │       │       │
              ◄─────────────┼───────┼───────┼─────────────►
              No LLM        │       │       │       LLM-native
                            │       │       │
                LangGraph ──┤       │       ├─── MAF (Microsoft)
                (durable    │       │       │   OpenAI Agents SDK
                 mode)      │       │       │   AG2
                            │       │       │
                            │       │       │
                Letta ──────┘       │       └─── CrewAI, AutoGen,
                                    │           PydanticAI, smolagents,
                                    │           Goose, Eliza, Mastra,
                                    │           Agno
                                    ▼
                           Application / framework-shaped
```

**Open-pincery sits in the lower-right quadrant: LLM-native + infra-shaped.** That quadrant is _under-occupied_. The closest occupants in spirit:

- **Cloudflare Durable Objects + Workers AI + Agents SDK** — but Cloudflare-locked-in, no self-host.
- **DBOS + LLM bolt-ons** — possible future, not shipped as a product.

### Closest competitors (honest)

| Project           | Why pick them                                                                         | Why pick open-pincery                                                                                                                                    |
| ----------------- | ------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **DBOS Transact** | Same Postgres-native deployment. TS + Python first-class. Library-as-runtime. Mature. | Open-pincery has formal TLA+ spec, event-sourced actor model with continuous identity, LLM-native primitives. DBOS is workflow-shaped, not actor-shaped. |
| **Temporal**      | Battle-tested, 7 SDKs, deterministic replay is core.                                  | Temporal needs a 5-service cluster. Open-pincery is one Postgres. Temporal is workflow-shaped (no continuous agent identity).                            |
| **Restate**       | Rust core like you. Newer, multi-language SDKs.                                       | Restate has no LLM-native story; pincers are LLM-native by design. Open-pincery's actor identity is more durable than Restate's invocation-shaped model. |

---

## 6. Claim audit (what survived, what didn't)

### Survived adversarial audit

1. **Event sourcing + actor model with continuous identity is in the actor heritage.** Direct lineage from Orleans/Akka/Temporal. CrewAI/LangGraph/Letta lack identity-continuous actors. Architecturally sound.
2. **Durable execution for LLM agents is underserved.** First-mover advantage is real.
3. **Single-Postgres deployment is pragmatic.** DBOS proves there's appetite. Temporal/Restate are heavier.

### Should be retracted or qualified

1. **"Anything is a pincer" as user-facing vocabulary** — keep internally, drop from the README headline. People search "durable agent runtime," not "pincer." Lead with category, then introduce the term.
2. **"Rust-only is fine"** — never quite said it, but worth being explicit: a Python SDK that wraps the HTTP API is probably the single highest-leverage week of work in the project.
3. **"The Orleans/Temporal/Akka neighborhood, not LangChain/CrewAI"** — the boundary I drew was wrong. The actual closest neighbors are **DBOS, Restate, Inngest** — not Orleans. Benchmark against DBOS specifically.

### Subagent error to flag

- The audit claimed "no project has deterministic replay." That's wrong — deterministic replay is **the core feature of Temporal**. The genuine gap is _deterministic replay across the LLM call boundary_, not deterministic replay in general.

---

## 7. Cross-repo strategic map (open-pincery + gbrain + gstack)

```
┌─────────────────────────────────────────────────┐
│  CHOREOGRAPHY  ←  gstack (91k★)                 │
│  "How an agent should sequence work"            │
│  Single-user, session-scoped, methodology       │
├─────────────────────────────────────────────────┤
│  MEMORY        ←  gbrain (13.6k★)                │
│  "What an agent knows about your world"         │
│  Per-user knowledge graph, hybrid search, jobs  │
├─────────────────────────────────────────────────┤
│  SUBSTRATE    ←  open-pincery (0★)              │
│  "Where agents persist and run"                 │
│  Multi-agent, event-sourced, durable identity   │
└─────────────────────────────────────────────────┘
```

These projects do not compete. They occupy three layers. Garry Tan owns choreography + memory; they snap together via MCP. Open-pincery owns substrate; **nothing snaps onto it yet**. The integration story is the bottleneck, not feature breadth.

### Integration paths (concrete)

| Path                          | How                                                                                                      | Status                                                |
| ----------------------------- | -------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| **open-pincery hosts gbrain** | A pincer's shell tool wraps `gbrain query` / `gbrain put`. Brain-pincer wraps the whole gbrain instance. | Not shipped. ~half-day of work. **Highest leverage.** |
| **gstack hosts open-pincery** | gstack triggers a pincer via webhook (`POST /api/agents/:id/messages`), waits, surfaces result.          | Not shipped. Needs reverse-proxy or public endpoint.  |
| **gbrain hosts open-pincery** | gbrain minion `curl`s pincer HTTP API.                                                                   | Possible, low priority.                               |
| **MCP everywhere**            | Expose pincer surface as MCP tools; consume gbrain/external MCP tools as pincer tools.                   | Not shipped. **High leverage.**                       |

---

## 8. Three positioning options (pick one)

|                                                            | Pitch                                                                                                                                                                                                        | Defensibility                                                         | Risk                                                                                       |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| **A. Substrate the Tan stack runs on**                     | "gstack and gbrain assume the agent dies between sessions. Open-pincery is the persistence layer that lets a fleet of agents accumulate memory and respond to webhooks even when no human is at a terminal." | **High.** Real gap in the Tan stack. Rides on 91k+13.6k stars.        | Becomes "and also" rather than leader.                                                     |
| **B. The formally-correct alternative to OpenClaw/Hermes** | "Everyone else gets the easy parts wrong under load. We have TLA+, event sourcing, CAS."                                                                                                                     | **Medium.** True, but few users feel this pain until they've shipped. | Slow adoption. Sells to people who already had outages.                                    |
| **C. A standalone agent platform**                         | "Use open-pincery instead of OpenClaw/Hermes/CrewAI."                                                                                                                                                        | **Low for now.** Year behind on ecosystem.                            | Direct head-to-head with two more mature competitors. Deepens "lost in the sauce" feeling. |

**Recommendation**: A is the move. It plays to the real differentiator (durable identity, fleet, formal spec) rather than competing on surface features.

---

## 9. The genuine "next thing" the field needs (ranked)

> **Revision 2**: Operator explicitly downgraded LLM-replay-determinism. The new top of the list is _operability_ primitives — what makes the OS deployable, maintainable, and trustworthy in single-operator hands. Replay determinism is preserved as a footnote in §9.7.

### 1. **Single-command deploy + maintain + upgrade lifecycle** ← OS-shaped #1

The operator-of-one needs `pcy bootstrap`, `pcy upgrade`, `pcy backup`, `pcy restore`, `pcy doctor` to _just work_ without thinking about Postgres internals, migration ordering, or service supervision. AC-78's startup gate + recovery runbook is the seed of this; expand it to a full lifecycle. **Without this, every other primitive is unreachable.**

### 2. **Operator dashboard / observability surface**

A single-operator OS without a "what is my fleet doing right now" view is not an OS. Live pincers, recent events, sandbox denials, capability rejections, audit-chain integrity, cost/spend, queued work. AC-44 (OpenAPI 3.1) is the seed; need the human-facing surface on top. OpenTelemetry export is table stakes; a built-in `/admin` is the differentiator.

### 3. **Original LLM-call-boundary determinism for replay** (formerly #1, retained for completeness)

Temporal-style replay assumes deterministic user code. LLMs are non-deterministic. Open-pincery already captures `llm_calls` rows; what's missing is the prefer-cache code path. **Operator priority: deferred.** Strategically interesting (only project to ship this) but not what an operator-of-one is asking for. Pick up only after operability lands.

### 4. **MCP as the tool boundary**

MCP is winning. Goose, Mastra, smolagents, Eliza, Cloudflare Agents all support it. Exposing the pincer surface through MCP costs almost nothing and adds open-pincery to every MCP-aware client (Claude Code, gstack, gbrain) for free. **High leverage, low cost.**

### 5. **Cost-aware durable execution** ← genuinely empty space

No surveyed project treats LLM cost as a first-class budget primitive. `llm_calls.cost` is already populated (verified in source audit). Missing: per-pincer budget cap with deterministic shutdown when exceeded. Real product feature nobody else has. Natural home for the hopper/GPU-lease work — leases and pincers share the same budget primitive.

### 6. **Multi-pincer coordination as a first-class API**

NOTIFY/LISTEN already exists. Make pincer-to-pincer messaging a first-class API, not a shell-tool side effect. CrewAI/AG2 hand-roll their own brokers; you have Postgres. Low cost, real differentiator.

### 7. **Reference pincers (mailroom, brain, lease, scheduler)**

Until pincer-other-than-demo exists, "anything is a pincer" is unsubstantiated. Mailroom (~200 LOC) closes the demo loop. Brain-pincer (gbrain wrap) closes the depth loop. Lease-pincer (hopper-as-pincer) closes the GPU loop and dissolves the entire `remote-agent-assistent` repo into a directory.

### 8. **Python SDK for authoring**

Source audit revealed pincer behavior already lives in `agent_projections` TEXT fields — DB-resident, no recompile required. So a Python SDK is a 200-line ergonomics wrapper around HTTP, not a Temporal-style durable-execution SDK. Cheap. Required for adoption beyond Rust devs.

### Lower priority than they sound

- **A2A protocol** — over-hyped, no convergence, every project has its own version. Wait.
- **HITL / approvals** — substrate primitive (suspend-wake-on-event) is wake-on-event you already have; the _workflow_ is a pincer concern, not OS.
- **Eval frameworks** — application-layer; let downstream projects do it.

---

## 10. Sharp questions (must answer before public positioning)

1. **Replay semantics for the LLM call boundary** — what does v1.0.1 actually do? Records completion bytes in the event log and replays from log? Or re-calls the LLM on recovery and accepts divergence? Whichever, that's _the_ correctness paragraph in the README.

2. **Server-client boundary** — does v1.0.1 already let an external HTTP caller register a webhook and define a pincer's behavior remotely? Or must pincer behavior be compiled into the Rust binary? If the former, ship the Python SDK next. If the latter, refactor for the boundary first.

3. **DBOS comparison** — have you read DBOS Transact? They're 80% of your deployment model with 20% of your formal-correctness model. If you can't write the "what DBOS gives you, plus TLA+, plus event sourcing, plus LLM-first" sentence, DBOS will eat the niche before v2.

4. **MCP** — yes/no/timeline. One-week decision, probably the highest-leverage 1-week investment.

5. **The deterministic-LLM-replay angle** — are you pursuing it? If yes, that's the headline of the project. If no, don't claim "Orleans-grade correctness" because Orleans's correctness story includes replay.

---

## 11. Concrete next moves (in order)

> **Revision 2 (post PR-4 review)**: revised priorities reflect the operator-of-one OS reframe and the existence of 238 commits of v6→v9 work in PR #4 that are _not_ yet merged.

### Phase A — Land what's already built

1. **Decide AC-81 + AC-82 fate.** Either land them this week or explicitly defer to v9.1 and ship PR #4 now. Two ACs are gating an OS that already has bubblewrap sandbox, seccomp, capability nonces, hash-chain audit, credential vault, and 321 passing tests on real Postgres. The harness will keep finding ACs; an operator decides when v9 is done.
2. **Merge PR #4 to `main`** so v6→v9 is the public face of open-pincery. The current README/code drift (six security layers claimed; only HMAC on `main`) only resolves when this lands.
3. **README rewrite** to match v9. Lead with "single-operator agent OS" framing. Drop or correct the actor-runtime / six-layer claims that pre-date the merge.

### Phase B — Operator surface (the missing OS half)

4. **`pcy doctor`** — health check. Verifies Postgres reachable, sandbox functional, audit chain intact, vault decryptable, queue not stuck. Single command, exit codes for automation.
5. **`pcy backup` / `pcy restore`** — opinionated wrappers over `pg_dump` plus vault-key handling plus audit-chain verification on restore. Operator-of-one cannot tolerate "go read the Postgres docs."
6. **`pcy upgrade`** — migration runner with pre-flight, rollback-on-failure, post-flight audit-chain re-verify. AC-78's startup gate is the seed.
7. **Operator dashboard (`/admin`)** — live pincers, recent events, sandbox denials, capability rejections, cost spend, queued work, audit-chain status. AC-44 (OpenAPI) is the API; the HTML is what an operator-of-one actually opens.

### Phase C — Reach

8. **MCP surface.** Expose pincer tools as MCP. One week. Massive distribution.
9. **First reference pincer: mailroom.** ~200 LOC. Wakes on inbound webhook (email-as-input via SES/Postmark/etc.), routes to other pincers. Closes the demo loop.
10. **Python SDK** as ergonomics wrapper over HTTP. ~200 LOC. Cheaper than I previously claimed because pincer behavior is already DB-resident.
11. **Brain-pincer** wrapping gbrain. Closes the depth loop and slots open-pincery into the Tan stack as the persistent-substrate layer.
12. **Lease-pincer** wrapping hopper. Dissolves the `remote-agent-assistent` repo into `pincers/lease/`. ~100 LOC.

### Phase D — Differentiation

13. **Per-pincer cost budget primitive.** Data already captured (`llm_calls.cost`); add the cap-and-shutdown enforcement. First project to ship this.
14. **First-class pincer-to-pincer messaging API** (not via shell tool).
15. **OpenTelemetry export** (table stakes; enterprise eval).
16. **LLM-replay-from-cache** if and only if (a) phases A–C are done and (b) it still feels like a positioning win when re-evaluated.

### What's no longer in the list

- **Standalone GPU-lease subsystem.** Becomes lease-pincer. ~100 LOC. Done.
- **lights-out-swe as a custom harness.** Either retire in favor of gstack, or accept that it will keep producing more ACs than ship and budget for that explicitly.
- **Generic "agent framework" features** (multi-agent orchestration DSL, eval frameworks, RAG pipelines). Not OS work. If needed, they're pincers.

---

## 12. The PR #4 problem — named explicitly

PR #4 is 238 commits, draft for 2+ weeks, gated on AC-81 + AC-82. Verification at HEAD: 321 passed / 0 failed. CI: 6/6 green. Real Postgres. Real bubblewrap. Real capability nonces.

**This is the "lost in the sauce" feeling, made specific.** Not a strategy gap. Not a positioning gap. **A merge-discipline gap.**

Two readings:

- **Generous**: AC-81 (TLA+ spec coverage manifest mapping every AC to canonical actions/invariants + commit-msg hook) and AC-82 (fine-grained `AgentStatus` variants for `WakeAcquiring` / `PromptAssembling` / `ToolDispatching` / etc.) are real correctness work. Spec coverage prevents drift. Fine-grained statuses unlock observability. Land them, then ship.
- **Adversarial**: AC-81 and AC-82 are _meta-quality_ work. Neither is operator-visible. They satisfy the lights-out-swe pipeline's quality bar, not an operator's needs. An operator running open-pincery as their OS does not care if `WakeAcquiring` is a separate enum variant from `PromptAssembling`. They care that pincers wake.

**Recommendation: land or defer within one week.** If they land, they land. If they don't land in a week, defer to v9.1 with explicit `DELIVERY.md` notes and ship PR #4. **The harness will keep finding ACs. The operator is the one who says "v9 is done."**

This is also the test for whether the OS framing is working. A runtime author would labor over AC-82 because state-machine purity matters to a runtime. An OS author ships v9 because operators are waiting.

---

## 13. Meta-notes for future sessions

- **Vocabulary**: keep "pincer" internally; lead the README with "single-operator agent OS" or similar searchable category language. "Pincer" is the user-space process; the OS is open-pincery.
- **Don't add features that aren't substrate or kernel-grade.** Use the dichotomy in §3 and the OS contract in §1 as the gate. Anything else is a pincer.
- **Strategic positioning is downstream of architectural comprehension AND merge discipline.** Three sessions of strategic re-framing happened against `main` (v5). The actual project is on a draft branch (v9). That gap explains a lot of the felt-confusion.
- **Substrates feel incomplete by design** — they only complete when ecosystem builds on them. Stop trying to make the substrate feel complete; make it cheap and obvious to write the next pincer. **Reference pincers are the cheapest answer to "what does open-pincery do?" that doesn't require reading 4600 lines of TLA+.**
- **The harness producing more ACs than you can ship is not a discovery problem; it's a merge-discipline problem.** Phase A above is the answer.

---

## Appendix: Glossary

- **Pincer**: a continuous LLM-native agent — durable identity, event log, wake/sleep, async messaging.
- **Substrate**: open-pincery itself — the runtime that hosts pincers and guarantees properties about them.
- **Wake cycle**: bounded active episode of a pincer: wake → reason → tools → sleep.
- **CAS lifecycle**: compare-and-swap on Postgres `status` column to ensure a single wake per trigger.
- **Event log**: append-only Postgres table; the source of truth for a pincer.
- **Projection**: derived state (identity prose, work list) computed from the event log.
- **Tan stack**: gbrain (memory) + gstack (choreography); built by Garry Tan; snap together via MCP.
- **The dichotomy**: substrate vs pincer (§3); the gate for every feature decision.
