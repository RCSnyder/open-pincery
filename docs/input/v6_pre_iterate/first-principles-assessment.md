# First-Principles Assessment — "Back to Basics for an AI Platform"

> **SUPERSESSION NOTICE (2026-04-20):** This document is preserved as a _thinking record_. The canonical strategic direction is now `docs/reference/north-star-2026-04.md`, with `docs/reference/strategic-answers-2026-04.md` and `docs/reference/tripwires-2026-04.md` as companions. Where this document names the first buyer as a mid-market platform engineer, the first mission as an isolated GitHub PR monitor, the first wedge as SOC 2 / NIST AI RMF compliance tooling, or the benchmark as a 30-day signed evidence bundle, those positions have been superseded. The current positions are: founder-first dogfooding, a Tier 1 mission catalog (codebase steward, inbox triage, commitments tracker, weekly digest), a sovereign-agentic-workforce wedge, and a 90-day founder-operated benchmark. Read this document for how the conclusions were reached; read the reference docs for what the conclusions currently are.

> **Status:** Input doc, not scope. Captures a critical-thinking pass against the frame:
> _"With AI, what do we actually need as fundamental services? Most stuff is junk now."_
>
> **Frame under test:**
>
> - Primitives: **agents, executors, credential management, data/files**.
> - Properties: **systems of record, business processes, proof something did something precisely**.
>
> This doc stress-tests that frame against the current Open Pincery codebase (post-v5, at tag `v1.0.0`) and against the other input docs. It is a pre-EXPAND artifact — candidate material for the next `/iterate` cycle, not a commitment.

---

## 1. The frame, stress-tested

The frame is tighter than the "improvement-ideas" brainstorm because it collapses Cloudflare's 7 primitives and the governance-debt essay's 4 bets into one sentence. The test for any feature becomes: _does it deliver one of those seven things, or is it chrome?_

**One omission worth naming: scheduling / time.** Agents that wake on their own (cron-like) or on external events (webhooks, other agents) are the difference between a batch tool and a continuous entity. Open Pincery already has `LISTEN/NOTIFY` + stale recovery for this, but it's worth naming explicitly.

**One primitive doing two jobs: "proof something did something precisely."**

- Job A — **complete provenance**: what model, what prompt, what input, what output. ✅ Have this via `llm_calls` + `events`.
- Job B — **tamper-evidence**: can a future auditor prove the event log wasn't edited? ❌ Not have this. Hash chain / signed entries is a real missing primitive, not chrome.

---

## 2. Primitives mapped to the current codebase

| Primitive                   | What exists                                                                                                                                                             | Quality                                                                                | Gap                                                                                                                                                                               |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Agents**                  | `agents` table, CAS lifecycle (`asleep→awake→maintenance→asleep`), per-agent webhook secret, owner, workspace                                                           | Strong. Crown jewel.                                                                   | Fine-grained TLA+ states (`Resting`/`WakeAcquiring`/etc.) are in-memory only — DB uses three raw strings. Carried as debt since v4.                                               |
| **Executors**               | `shell` tool. Philosophy: "one programmable executor, agents write programs."                                                                                           | Strong conceptually; weak in practice.                                                 | No sandbox (Zerobox deferred). No persisted scratch space. Shell runs with host privileges — the single largest v1 "known limitation" and still unaddressed at v5.                |
| **Credential management**   | Per-agent `webhook_secret` (inbound), global `LLM_API_KEY` (outbound), session tokens (human). OneCLI named in security-architecture.md as Phase 2.                     | **Weak.**                                                                              | All agents share one LLM key. No per-agent outbound credential scope. No proxy-injection vault. Essay's "Bet 1" (biggest demo-vs-production gap) is unresolved.                   |
| **Data / files**            | `events`, `agent_projections` (TEXT), `wake_summaries`. All structured as rows.                                                                                         | Strong for _prose memory_.                                                             | **No addressable filesystem.** Agent cannot `ls` its own workspace, cannot persist a scratch artifact across wakes, cannot version a file. This is improvement-ideas §4.6 Tier 0. |
| **System of record**        | `events` append-only, `llm_calls`, `tool_audit`, `auth_audit`, versioned `agent_projections`, versioned `prompt_templates`                                              | **Very strong.** Arguably best-in-class vs. competitors in `competitive-landscape.md`. | None at the functional level.                                                                                                                                                     |
| **Business process**        | Wake loop = durable business process at the _boundary_ level (wake_started / wake_finished are persisted). Drain check = continuation. Stale recovery = crash handling. | Medium.                                                                                | **Not durable within a wake.** If process dies between tool call #3 and #4, in-flight state is lost. Improvement-ideas §4.2 (fibers) is the fix.                                  |
| **Proof of precise action** | Event log + `llm_calls` (model, tokens, cost, latency, prompt_hash, response_hash) + `tool_audit`                                                                       | Medium-strong on _completeness_, zero on _tamper-evidence_.                            | No hash chain over events. No signed LLM-call attestations. Admin with DB access can edit history silently.                                                                       |

---

## 3. What might be "junk" under this frame

Not "delete" — **"stop investing"**. Candidates for future pruning if the frame holds:

| Feature                                                                          | Why it might be junk                                                                                                                                                                                     | Counterpoint                                                                                                                    |
| -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| **Maintenance cycle's free-form prose projections** (identity, work list)        | Dual-purpose LLM call after every wake. Writes free-form prose that is never schema-checked and rarely read back verbatim. Could be replaced by explicit **mission objects** + auto-generated summaries. | It _is_ the "continuous identity" claim in the README. Killing it kills the product thesis unless replaced by something better. |
| **Rate limiting per IP**                                                         | IP-based limiting on a self-hosted single-tenant runtime is chrome. Real risk is LLM spend, already tracked by `budget_used_usd`.                                                                        | Still useful on public endpoints. Keep the code, stop adding.                                                                   |
| **Webhook ingress as a first-class subsystem**                                   | Webhooks are "a message from outside." HMAC + idempotency dedup is ~200 lines supporting one narrow pattern.                                                                                             | Real use case (external systems pushing work). Could equivalently be `pcy message` from a cron job.                             |
| **Agent PATCH/DELETE**                                                           | Soft-delete and rename are CRUD chrome.                                                                                                                                                                  | Operators ask for it. Cheap.                                                                                                    |
| **Multi-tenant RBAC scaffolding** (workspaces, memberships, policy sets in TLA+) | Partially built, mostly not enforced. Only meaningful if SaaS is the target. Self-host audit explicitly says billing/RBAC can be inert.                                                                  | If committing to self-host-first, this is ~30% of schema that does nothing. A real choice.                                      |
| **`pcy` CLI**                                                                    | Duplicates the HTTP API in a second surface. Once UI is real, CLI value drops.                                                                                                                           | Scripting/CI. Still legit, just not a primitive.                                                                                |
| **`docs/api.md` stability contract**                                             | Paperwork, not a primitive. Useful for consumer confidence; not on the primitive list.                                                                                                                   | Necessary if others build on top.                                                                                               |
| **Prometheus metrics + CI + SBOM + signed releases** (v3)                        | Ops hygiene. Mandatory for "skyscraper" tier self-declaration, but not primitives.                                                                                                                       | Required for any credible release. Stop expanding, don't cut.                                                                   |

**Honest estimate:** ~25–35% of current code is "platform chrome around the primitives." Not unusual — most systems are ~50% chrome. Calling it out explicitly creates a pruning budget.

---

## 4. What's genuinely missing under this frame

Ranked by "does it make the primitive work or not":

| #      | Gap                                                                                         | Primitive it serves      | Severity        | Notes                                                                                                                                                            |
| ------ | ------------------------------------------------------------------------------------------- | ------------------------ | --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **G1** | **Durable execution within a wake** (checkpoints per tool call)                             | Business process         | **High**        | Single biggest correctness hole. Wake crash = lost work.                                                                                                         |
| **G2** | **Per-agent outbound credential scope** (proxy-injected, never seen by agent process)       | Credential management    | **High**        | Essay's Bet 1. Cannot honestly claim "production-grade agents" without this. OneCLI integration or equivalent.                                                   |
| **G3** | **Tamper-evident event log** (hash chain or signed entries)                                 | Proof                    | **High**        | "Proof something did something precisely" is currently "trust the DBA." Hash-chained events + signed LLM-call attestations make audit cryptographic, not social. |
| **G4** | **First-class agent workspace filesystem** (`/workspace/agent-<id>/` persisted, searchable) | Data / files             | **Medium-High** | Agents can `cat /tmp/foo` but nothing persists cleanly across wakes. Makes "agent as continuous entity" legible as files, not just DB rows.                      |
| **G5** | **Sandboxed executor** (Zerobox or equivalent)                                              | Executor                 | **High**        | Not new — "Phase 2/3" since v1. Under this frame, an executor that runs host-privileged IS NOT an executor primitive. It's a landmine.                           |
| **G6** | **Scoped agent-to-agent messaging with per-agent tokens**                                   | Credentials + agents     | Medium          | Q3 + Q4 from improvement-ideas. Impossible today without giving an agent a human session token.                                                                  |
| **G7** | **Explicit mission / task objects** (stable `mission_id` threading across wakes and agents) | Business process + proof | Medium          | Replaces prose work_list with something auditable.                                                                                                               |
| **G8** | **FTS + non-destructive compaction over events**                                            | System of record         | Low-Medium      | Makes event log _useful_, not just complete.                                                                                                                     |

---

## 5. Reframed iteration options (frames, not features)

### Option α — "Finish the primitive: credentials"

**G2 + G5** together. Per-agent credential scoping **and** sandboxed execution in one iteration — they're two halves of the same boundary: "what this agent can touch outside itself." Single highest-leverage move from "demo" to "platform." **Large iteration.** Requires design addendum + dependency choice (Zerobox vs. alternatives; OneCLI vs. homegrown).

### Option β — "Finish the primitive: proof"

**G1 + G3.** Durable business process mid-wake **and** cryptographically tamper-evident event log. The two "proof" gaps. **Medium-large iteration.** Fully additive to schema. No external dependencies.

### Option γ — "Finish the primitive: data"

**G4 + G8.** Addressable persistent filesystem per agent + searchable event log. Turns "workspace" metaphor concrete; makes memory retrieval real. **Medium iteration.**

### Option δ — "Narrowest bite from the original menu"

**G1 alone** (wake checkpoints). Keeps v6 small, ships in days. Safe.

### Anti-option ε — "Prune, don't build"

Spend a full iteration _removing_ code. Delete unused multi-tenant scaffolding if self-host-first is really the target. Consolidate the maintenance cycle. Ship the same product with less surface area. Cheapest long-term, hardest to justify short-term.

---

## 6. Critical counterpoints (arguing against the framing itself)

- **"Junk" is context-dependent.** The rate limiter is junk for self-host single-user, _essential_ for SaaS. The maintenance cycle is chrome if replaced, primitive if not. Cannot evaluate junk until a deployment mode is picked — project still lists all four (`self_host_individual`, `self_host_team`, `saas_managed`, `enterprise_self_hosted`).

- **Hash-chained audit (G3) is genuinely expensive to get right.** Merkle proofs, key custody, re-orgs on migration — a full iteration on its own and arguably out of scope unless an external auditor is a known customer. Don't pick it unless you mean it.

- **Sandboxing (G5) has been "next iteration" since v1.** A pattern: every iteration picks the one that didn't require a sandbox dependency choice. If you keep deferring it, the platform eventually ships without its declared security model. Credibility issue even if no one gets hurt.

- **"Fundamental services" framing has a trap.** Every AI platform author thinks _their_ list of 4–7 primitives is minimal. Cloudflare says 7 (Durable Objects, fibers, sub-agents, sessions, codemode, execution ladder, extensions). LangChain says different 7. This doc says 4 + 3 properties. The governance essay says 4 bets. No external oracle says who's right. The real test isn't "does this match first principles" — it's **"does this let operators do what they couldn't before?"**

- **The "everything else is junk" instinct is worth interrogating.** v1–v5 did work operators use. Prune too aggressively and you'll rediscover why each piece exists. Suggestion: tag features `primitive / enabler / product`, don't delete anything yet, and use the tags to _budget future investment_.

---

## 7. Decisions to make before the next `/iterate` confirmation

1. **Deployment mode commitment.** Self-host only? Self-host + SaaS? This alone moves 25–30% of current code between "primitive" and "junk."
2. **Primitive priority:** pick ONE of {credentials+executor, proof, data}. All three are real gaps. Only one per iteration done well.
3. **Prune-or-build?** Option ε is uncomfortable but may be the right call before adding more.

---

## 8. How this relates to existing input docs

- Extends `improvement-ideas.md` Part 5 ("what to do with all this") by adding a pruning axis the brainstorm deliberately omitted.
- Consistent with `security-architecture.md` Phase 2 priorities (OneCLI + Zerobox = G2 + G5 = Option α).
- Consistent with `enterprise-readiness.md` "Operational Readiness" and "Audit Export" gaps — G3 (tamper-evidence) directly addresses the SIEM/evidence-package requirement.
- In tension with `saas-readiness.md` and the TLA+ multi-tenant surface — if the deployment-mode decision lands on self-host-first, significant TLA+ machinery becomes candidate-for-prune (anti-option ε).
- Does not contradict `best-practices.md` — Practices 1–11 from the paper all map cleanly onto the primitive list (identity ↔ agents, compiler-in-the-loop ↔ executors, prompt management ↔ proof/system-of-record, approval gates ↔ business process).

---

## 9. Audit: what a distinguished systems engineer would grill this with

These are the questions a senior SRE / distributed-systems lead would raise during a design review. Most are currently unanswered in scaffolding or input docs. Some are design debt; some are docs debt; none are showstoppers, but leaving them all implicit makes the system feel more mature than it is.

### 9.1 Delivery and ordering semantics

- **What is the delivery guarantee on `NOTIFY`?** Postgres `LISTEN/NOTIFY` is **at-most-once** — if the listener is disconnected when a notify fires, the event is silently dropped. Today we rely on stale-wake recovery + polling-on-reconnect to paper over this. Is that documented anywhere? Is there a test that a message inserted _while the listener is down_ still causes a wake on reconnect? (Suspect: no.)
- **What happens during a migration that takes longer than the listener reconnect window?** Are all "missed" messages replayed, or just the most recent?
- **Ordering within a wake:** events are ordered by `created_at` timestamp. Clock skew between DB writes and Rust timestamps? What's the monotonicity guarantee? A `serial`/`bigserial` sequence alongside the timestamp would be cheap and strictly ordered.

### 9.2 Idempotency, retries, exactly-once

- Webhooks have HMAC + idempotency-key dedup. **Good.**
- LLM calls have no idempotency key. If the runtime crashes after the LLM responds but before `insert_llm_call` commits, the spend happened and the event didn't. The cost accounting is wrong by exactly one call. Rare but real.
- Tool calls: no durable `tool_call_id` pre-execution. Improvement-ideas §4.2 flagged this. Crash-after-side-effect = ghost work.
- **Recommendation:** adopt the invariant "every side-effecting action has a durable ID allocated _before_ the side effect." This is a systems-engineering hygiene rule, not a feature.

### 9.3 Schema evolution of the event log

- Events are append-only. Great. But the **shape** of event payloads is informal JSON in `tool_input` / `tool_output` / `content`. When payload shapes change, old events no longer parse the way the new code expects.
- There is no `event_version` column. There is no registry of event types + their schemas. Prompt assembly iterates historical events — what happens when the structure of a 6-month-old event becomes unparseable?
- **Recommendation:** add `event_version INT` + a typed registry. Decide up front: versioned parsers, or upcaster functions, or frozen schemas.

### 9.4 Replay and reconstruction

- Event sourcing's headline promise: projections can always be rebuilt from the event log. **Can they?** Today `agent_projections` rows are produced by an LLM call in the maintenance cycle — which is **non-deterministic**. Given the same events, you'd get a _different_ identity/work_list projection on replay. That's not event sourcing; it's event logging with a side-effecting reducer.
- This is fine, but it should be _acknowledged_. "Replay" in this system means "rebuild a deterministic subset (costs, counts, which wake owned which event) from the log," not "reproduce agent state bit-identical."
- G7 (mission objects) could be deterministic. Prose projections cannot be.

### 9.5 Time, clocks, and "precisely"

- What does "proof something did something _precisely_" actually mean when the model is non-deterministic (temperature > 0, provider-side version drift)?
- Today we capture `prompt_hash` + `response_hash`. That proves _what happened_. It does not prove _that the same prompt would produce the same response again_. If you need reproducibility, you need model-provider version pinning + seeded sampling + cached provider responses (a la VCR).
- **Recommendation:** add `model_fingerprint` (provider-reported model version, not just `gpt-4o`) to `llm_calls`. Accept that replay is for auditability, not reproduction.

### 9.6 Backpressure, load, capacity

- `background::listener` is single-process, serving all agents. What breaks first at: 100 agents, 1k agents, 10k agents, 100k agents?
- What's the target? The TLA+ and scope docs don't name one. Saying "we support many agents" with no ceiling is a lie by omission.
- `wake_loop` iteration cap is 50. Per-wake LLM budget is set. Per-IP HTTP rate limit is set. **Per-agent wake-rate is unbounded.** A runaway agent can wake 60 times/minute, burning budget until the dollar cap hits. Dollar-based throttling is after-the-fact.

### 9.7 Failure domains

- Single process, single Postgres. If Postgres dies, all agents die. Documented.
- If the Rust process dies mid-wake, the wake is "stale" after 2 hours. That's a 2-hour recovery window — acceptable for self-host, unacceptable for SaaS.
- No "another instance is already processing this agent" fencing beyond CAS on the `status` column. If two runtimes are pointed at the same DB (accidentally or via misconfigured HA), CAS prevents duplicate wakes but **not** duplicate listeners. Both processes will try to wake.
- **Recommendation:** document "single writer per DB" as a hard invariant, or introduce a lease/leader-election row before any HA story.

### 9.8 Observability gaps

- Prometheus metrics exist (v3). Great for counters/gauges. **No distributed tracing.** A wake fans out: HTTP receive → event insert → NOTIFY → listener → CAS → prompt assembly → LLM call → tool dispatch → LLM call → ... Today, tying a single user-perceived "request" to the full causal chain requires manual log grepping on `agent_id`/`wake_id`.
- OpenTelemetry tracing was explicitly deferred in v3. It's the single highest-value observability add for debugging wake pathologies. Revisit.
- `wake_id` is the natural trace root. It's already in every event. OTEL would just make the spans visualizable.

### 9.9 Migration safety

- Migrations are SQL files with sequential numeric prefixes. No rollback scripts. No pre-migration sanity checks. No `CHECK` or online-migration discipline (no "add column nullable, backfill, make not-null in follow-up migration" pattern documented).
- Runbook `docs/runbooks/migration-rollback.md` exists (v3 AC-21). Is it tested? Unclear.

### 9.10 "Event log as source of truth" — partially true

- Events are the source of truth for **what happened**.
- `agents.status`, `agents.budget_used_usd`, `agents.webhook_secret` are _mutable state_ that is **not derivable from the event log alone**. If the `agents` table were wiped and rebuilt from `events`, you would not get the same `budget_used_usd`.
- That's OK — not every field needs to be derived — but it means the system is a **hybrid** of event-sourced and CRUD, and current docs imply it's pure event-sourced. Be honest about the boundary.

---

## 10. Audit: what a senior executive would grill this with

These are blunt, not technical. A CEO / head of product / VP Engineering cares about _whether this ships, to whom, at what margin, and what happens when it breaks._

### 10.1 Who is the buyer?

- The README, scope docs, and input docs talk past each other: `self_host_individual`, `self_host_team`, `saas_managed`, `enterprise_self_hosted` are all "supported." **In practice only `self_host_individual` works end-to-end.** The other three are architecture without product.
- An executive would ask: **"Pick one. What's the wedge?"** Trying to be all four at once means none get done well.
- Candidate wedges, ranked by realism:
  1. **Solopreneur power user** — runs 3–10 personal agents for coding, research, ops. $0 MRR, but a credible community.
  2. **Small engineering team self-host** — 10–50 agents, shared workspace. The `self_host_team` mode. Blocked by G2 (credentials) and G6 (messaging).
  3. **SaaS managed** — everything the team needs, we run it. Blocked by billing, signup abuse, customer UI, ToS — none of which is built.
  4. **Enterprise self-host** — SCIM, BYOK, SIEM. Blocked by everything in `enterprise-readiness.md`.

### 10.2 Buy vs build

- **Why not LangGraph + Temporal + Vault + Postgres?** That combo gets you: workflow graphs (LangGraph), durable execution (Temporal), credential management (Vault), system of record (Postgres). You'd still write the agent loop and the prompt assembly, but you'd inherit a lot.
- Open Pincery's answer has to be: **"continuous agents are a different primitive than workflow graphs."** Is that thesis defensible? Yes — `competitive-landscape.md` makes the argument well. But an exec would demand a one-paragraph version of that argument they could put in a deck.
- **Risk:** Cloudflare, AWS Bedrock Agents, OpenAI Assistants, and Anthropic Skills are all shipping adjacent primitives with platform-scale distribution. The window for "continuous agent" as a standalone category is narrowing.

### 10.3 Unit economics

- At 10 agents, 3 wakes/day, 5 LLM calls/wake, $0.002/call — ~$9/month LLM spend per user. OK.
- At 100 agents, same multipliers — $90/month. Still OK for self-host.
- **SaaS math:** if we run it for the customer, the customer's LLM is our cost unless we pass through. If we pass through, every customer sees a BYOK setup friction. **Which is it?** No input doc answers this.
- Postgres for a single-tenant self-host is ~$0 (Docker). Postgres for multi-tenant SaaS is either per-tenant (expensive) or shared-schema (RLS nightmare). The TLA+ says shared with RLS; that's the hardest-to-operate choice and has not been stress-tested.

### 10.4 Liability and safety posture

- The shell tool runs host-privileged. The DELIVERY.md lists this under "known limitations" in prose. An exec reading this before a sales conversation would freeze.
- **"An agent wrote a rogue `git push --force` and deleted the customer's main branch"** is not a hypothetical; it's a weekly-in-the-field problem for every agent platform. What's our story? Today: "the operator configured approvals." That's not a product answer, that's a shrug.
- **Recommendation:** G5 (sandboxed executor) isn't a v6 feature. It's a **Do Not Sell Without It** feature for anything beyond solopreneur.

### 10.5 Compliance posture

- SOC 2: would likely pass Type I with the audit log + `auth_audit` + SBOM. Type II needs 6 months of operational evidence — which means: is the system running anywhere yet? No.
- GDPR: right-to-erasure on an append-only log requires crypto-shredding (encrypt each tenant's event payloads with a tenant-scoped key; delete the key to "erase"). Not implemented.
- EU AI Act (high-risk AI systems): agents executing code at scale may qualify. The Act requires risk management, data governance, transparency. We have traceability (good) but no risk management framework (gap).
- HIPAA: not targeted. Correctly out of scope for v1-v6.

### 10.6 Key-person risk

- This is a solopreneur project with a lights-out build harness. If the solopreneur is hit by a bus, what happens to the code? MIT license means anyone can fork — but fork _what_? The operational knowledge lives in the scaffolding/ dir, which is actually unusually good for a one-person project. Praiseworthy.
- Still, **one honest question:** what's the succession plan? If this grows to 100 self-hosters, someone needs to answer their issues. Is that you, forever?

### 10.7 Exit / moat

- MIT license + commoditizing LLM APIs = **no licensing moat**.
- **Moat candidates:**
  1. Operational excellence on hosted version (brand + ops).
  2. Ecosystem around `pcy` CLI + prompt templates + agent recipes.
  3. Tamper-evident audit (G3) as a differentiator for regulated industries.
  4. The event-sourced architecture itself as a _dataset_ — every customer's agent history is uniquely yours to reason over (privacy-preserved).
- None of these are built yet. All are buildable.

---

## 11. Audit: what an individual entrepreneur / power user would grill this with

These are the "hands on keyboard, 30 minutes to decide if this is worth my Saturday" questions. The product must answer _all_ of them with a yes for solopreneur wedge to work.

### 11.1 Time-to-first-agent-doing-real-work

- Today, per `scripts/smoke.sh`: ~60 seconds from `docker compose up` to a message in the event log. **Good.**
- Time to first _useful_ work? Unknown. `pcy demo` (commit `72559b1`) gets you a canned response, not a completed task. There's no recipe for "configure an agent to do X concretely."
- **Missing:** an `examples/` directory with 3-5 recipes: "coding assistant that watches a git repo," "research agent that writes daily digests to a file," "issue triager for my GitHub repo." Each recipe = a preset prompt template + a sample constitution + a known-working LLM model.

### 11.2 Can I trust it overnight?

- Solopreneur's recurring test: "I'll fire up an agent before bed, can I sleep?"
- Today: budget cap prevents runaway spend. **Good.**
- Today: no sandbox means the agent can `rm -rf` anything in its own container. **Acceptable if contained to Docker.** Actively dangerous if running natively via `cargo run`.
- Today: no alerting. If an agent got stuck in a loop, you find out in the morning. Prometheus counters exist; need a recipe for "how to page myself when my agent hits its budget cap."

### 11.3 Data portability

- Can I export? No documented `pcy export` command. Event log dump via `pg_dump` is possible — that's the runbook.
- Can I import? Move an agent between laptops? Fork an agent (copy its identity + events to a new `agent_id`)? Not today.
- **Recommendation:** a `pcy agent export <id>` → tarball of events + projections + prompt template refs. Tiny feature, huge trust win.

### 11.4 Integration with existing tooling

- Can Claude Code / Cursor / Codex drive `pcy`? Yes in theory (CLI is scriptable). In practice, there's no documented pattern for "I'm an AI in my editor, please delegate this to my Open Pincery agent for overnight work."
- The interesting ergonomic: **`pcy` as a tool surface for _other_ AIs.** This is already how Cursor/Claude Code interact with git, npm, cargo. Open Pincery is the "put this on the background queue" primitive.

### 11.5 Cost transparency

- `pcy budget show <agent>` exists. Shows dollars used/limit.
- **What's missing:** cost-per-wake breakdown, cost-per-task, cost-per-model, "this agent costs $X/day, here's why." Solopreneurs watch LLM spend like a hawk.

### 11.6 Social proof / network

- Single-user tool today. No shared prompt marketplace, no "other people's agents," no templates community.
- This may be fine for 2026 — the solopreneur wedge can be "power user for me + small audience." But it caps network effects.

---

## 12. CLI ergonomics for agentic development — this deserves its own section

The CLI is not chrome in an agentic system. It's the **interop surface between humans, other AIs, and the runtime.** Every other primitive (agent, executor, credential, data) is eventually driven through CLI in scripts, cron jobs, and AI delegation. Under-investing here means the system is only usable through the GUI, which defeats the "continuous agent" thesis.

### 12.1 What `pcy` has today

Per `docs/api.md` and `src/bin/pcy.rs`:

- `bootstrap`, `login`, `agent {create,list,show,disable,rotate-secret}`, `message`, `events`, `budget {show,set,reset}`, `status`, `demo`.

**This is a reasonable skeleton.** It covers CRUD + one operation. But it's the _minimum_ — there are ergonomic gaps that matter specifically for agentic workflows.

### 12.2 Gaps and ergonomic improvements (by principle)

#### Principle A — stdout is structured, stderr is human

- Every command should accept `--json` (or be JSON by default and add `--human`). Today this is inconsistent — `pcy events` streams human-readable lines that break JSON tooling.
- Exit codes must be semantic: 0 success, 1 user error, 2 auth failure, 3 rate-limited, 4 budget exceeded. Today most failures exit 1 with a prose message.
- **Why it matters for agents:** another AI calling `pcy` needs to parse the result. If every command returns JSON with a stable schema, the "other AI" experience is dramatically better.

#### Principle B — every operation is resumable / re-executable

- `pcy message` is fire-and-forget. There's no `pcy wait <agent> --for-reply --timeout 60s` pattern. So scripting a "send and check the response" is clunky.
- **Proposal:** `pcy chat <agent>` — a REPL that sends a message, tails events until the agent sleeps, prints the final message. Synchronous UX on an async system. Huge win for "agent as expert consultant."

#### Principle C — discoverability beats documentation

- `pcy <command> --help` must explain _everything_ about the command, including example invocations. Today help text is terse.
- `pcy explain <wake_id>` — human-readable trace of what happened in a wake. Who sent the trigger? What was the prompt? What tools got called? What did the model say?
- `pcy doctor` — diagnose the local setup. Postgres reachable? Migrations applied? LLM API key valid? Budget unexhausted? Listener running? Today `pcy status` does a piece of this.

#### Principle D — tail / follow / filter, always

- `pcy events --follow --agent <id> --type tool_call` — live filtered stream. Essential for debugging.
- `pcy agents --follow` — see agents transitioning state in real time.
- No streaming via HTTP long-poll today from the CLI — the UI does this, the CLI doesn't. Asymmetric.

#### Principle E — safety on destructive operations

- `pcy agent disable` should require `--yes` or interactive confirm. `pcy budget reset` too.
- `pcy agent delete` doesn't exist; `pcy agent disable` is soft-delete. That's actually correct — make the naming match (`disable`, not `delete`).

#### Principle F — completions and env hygiene

- `pcy completions bash|zsh|fish|powershell` — ship shell completions. One hour of work, daily delight.
- Respect `NO_COLOR`, `CI`, `$XDG_CONFIG_HOME`. Fail loudly if `OPEN_PINCERY_URL` points at a different protocol than expected.

#### Principle G — an agent can drive `pcy` without a human in the loop

This is the CLI-for-agents angle and it's _different_ from CLI-for-humans:

- Agents running `pcy` from inside their own shell tool need a **scoped token** that is _not_ the human operator's session token. Today: no such token exists. The shell tool has no authenticated path back into the runtime.
- Agents need a way to **send messages to other agents** — either as a CLI subcommand (`pcy message <other-agent> "..."`) run inside the sending agent's shell, OR as a first-class tool (Part 2 of `improvement-ideas.md`).
- This ties back to G2 (credential management) and G6 (agent-to-agent messaging). **The CLI is where these primitives become usable.**

#### Principle H — dry-run and "would-do"

- `pcy message --dry-run` — assemble the prompt, show what would be sent to the LLM, exit without making the call. Invaluable for debugging prompt assembly.
- `pcy agent create --dry-run` — show the SQL / API payload, don't commit.

### 12.3 CLI as a design forcing function

Here's a useful heuristic: **every primitive should have a clean `pcy` verb.** If a primitive doesn't, that primitive isn't really a primitive yet, it's a half-built capability.

| Primitive        | Clean CLI verb today?                  | Verdict                                                        |
| ---------------- | -------------------------------------- | -------------------------------------------------------------- |
| Agents           | `pcy agent {create,list,show,disable}` | ✅                                                             |
| Executors        | `pcy shell <agent> "<cmd>"`            | ❌ Missing. Only accessible via agent-initiated tool calls.    |
| Credentials      | `pcy secret {set,list,rotate}`         | ❌ Missing. Only `rotate-webhook-secret` exists.               |
| Data / files     | `pcy workspace {ls,cat,put,get}`       | ❌ Missing. No per-agent filesystem primitive.                 |
| System of record | `pcy events`, `pcy llm-calls`          | ⚠️ `pcy events` exists. `pcy llm-calls` doesn't.               |
| Business process | `pcy wake {list,show,explain}`         | ❌ No wake-level CLI at all. Wake is implicit.                 |
| Proof            | `pcy audit {export,verify}`            | ❌ Missing. No export verb. No verify (because no hash chain). |

**This table alone is a roadmap.** Each missing verb corresponds to a real gap in the platform.

---

## 13. Roadmap sketch (not a commitment)

Synthesizing sections 4, 5, 9, 10, 11, 12 into a plausible 4–6 iteration sequence. All of this is thinking, not promising. The guiding rule: **every iteration should deliver one primitive more completely, and every iteration should add one `pcy` verb.**

### Phase A — "Make the CLI the product surface" (1 iteration, small)

- `--json` everywhere, semantic exit codes, `pcy chat <agent>` REPL, `pcy explain <wake_id>`, `pcy doctor`, `pcy completions`.
- No new primitive — existing primitives become _usable_.
- **Enables:** AI-driven `pcy` delegation, scripting, agentic development by other AIs.
- Risk: trivial. Pure UX work.

### Phase B — "Finish the proof primitive" (1 iteration, medium)

- G1 — wake checkpoints + durable tool-call IDs.
- G3 — hash-chained event log (SHA-256 chain over `events`, next_prev_hash column). Signed LLM calls optional.
- Add `pcy audit export` and `pcy audit verify`.
- **Enables:** "proof something did something precisely" as a defensible claim.
- Risk: schema migration. Crypto choice (keys, rotation). Adds operational surface.

### Phase C — "Finish the credential + executor primitive" (1-2 iterations, large)

- G2 — per-agent credential vault. Whether OneCLI or a native Rust implementation (`src/vault/`) is a dependency decision.
- G5 — sandboxed executor (Zerobox on Linux/macOS, Windows story deferred or fall back to Docker).
- G6 — agent-scoped session tokens injected into the shell environment. Opens the door to first-class agent-to-agent messaging.
- Add `pcy secret {set,list,rotate}` and `pcy shell <agent> "<cmd>"`.
- **Enables:** honestly claiming "production-grade" for team-scale deploys.
- Risk: large. Dependency choice. Cross-platform sandbox coverage. Schema changes.

### Phase D — "Finish the data primitive" (1 iteration, medium)

- G4 — per-agent persisted workspace filesystem.
- G8 — FTS over events + non-destructive compaction.
- Add `pcy workspace {ls,cat,put,get}` and `pcy events --search <query>`.
- **Enables:** agents that maintain state as files, not just prose. Makes "continuous agent" concrete.

### Phase E — "Missions, not prose" (1 iteration, medium)

- G7 — mission objects. Replace or augment work_list prose with explicit, addressable, stateful mission rows.
- Add `pcy mission {create,list,show,assign}`.
- **Enables:** auditable business processes. Compliance-grade artifacts.
- This also clarifies whether the maintenance cycle's prose projections should be kept, shrunk, or retired.

### Phase F — "Prune and document" (1 iteration, small — or concurrent with above)

- Delete unused multi-tenant scaffolding IF the wedge commitment (§10.1) landed on self-host-first.
- Document event schema versioning (§9.3), single-writer invariant (§9.7), time/precision model (§9.5), replay semantics (§9.4).
- Add OTEL tracing wrapped around the existing metrics (§9.8).
- No new primitive. Just honest docs + less code.

### Critical ordering observations

- **Phase A before everything else.** Every subsequent phase benefits from a CLI that can actually drive development and audit. If Phase B ships without `pcy audit verify`, operators can't consume the new primitive.
- **Phase B and Phase C are independent.** Could run in parallel if multiple contributors existed. For solopreneur, serialize.
- **Phase C is the single largest risk.** It's also the single largest credibility move. Deferring it indefinitely is a strategic mistake.
- **Phase E is the "thesis defense."** Missions replace prose as the durable unit of work. This is where Open Pincery decides whether "continuous agent" means "a named process with memory" (already have) or "a bounded business actor with auditable objectives" (need to build).
- **Phase F is optional and should stay optional.** Pruning without a deployment-mode commitment is thrashing.

### What to decide next

Before picking Phase B / C / D as v6:

1. **Commit to a deployment mode.** Write it in `preferences.md`. Everything downstream becomes clearer.
2. **Commit to a wedge buyer.** Solopreneur, team, or enterprise. Not all three.
3. **Decide if Phase A happens first.** Strong recommendation: yes. Every other phase shows up in CLI, so CLI quality compounds.
4. **Decide whether to pick up OneCLI + Zerobox as dependencies, or build native equivalents.** This is a 3-year commitment in either direction.

None of these decisions are made in this doc. They are named, so they can be made intentionally rather than drifting into by default.

---

## 14. Product thesis — "conversation is the surface, pincers are the runtime"

> **Status:** Thinking, not scope. Captures the positioning shift that emerged from critiquing the buy-vs-build frame in §10.2.

### 14.1 The anti-thesis

LangGraph, n8n, Airflow-for-AI, Inngest AgentKit, Temporal-workflow-authoring-with-LLMs — these are **workflow authoring tools with LLMs glued in**. They reproduce the last 20 years of pipeline engineering with smarter boxes. You still draw the DAG. You still write nodes. You still wire edges. The LLM is a node type, not a paradigm shift. That is "magically agentic" — same plumbing, new font.

This platform explicitly rejects that direction.

### 14.2 The positive thesis

The product surface is **a conversation**, not a graph editor.

The operator states an outcome. The system:

1. **Decomposes** the goal into specialist sub-goals.
2. **Spawns pincers** (specialist subagents) to own each sub-goal.
3. Lets those pincers **talk to each other** when coordination is needed.
4. Lets pincers **build whatever tooling they need mid-flight** (scripts, workers, data pipelines) rather than picking from a fixed tool catalog.
5. **Captures everything in the event log** so the whole assembly is auditable after the fact.

No DAG. No node palette. No workflow-as-code. The agents _are_ the workflow, and they assemble themselves around the goal.

### 14.3 What this rhymes with (and what it does not)

- **Rhymes with:** Erlang/OTP actor systems, Unix pipes as an ad-hoc composition surface, Smalltalk-style "living system" environments, Manus / Devin / Claude Projects as the current commercial analogs in the "describe it and it does it" category.
- **Does not rhyme with:** LangGraph, n8n, Temporal-as-a-product, Zapier, Airflow, Make.com. These are all authoring surfaces for workflows someone else runs.

The mental model is **pincers in a pincery**: peer specialists with continuous identity and memory, addressable by name, coordinating through async messages, operating under a shared audit trail. Subagent spawning is lateral, not hierarchical (a pincer can delegate to a peer; it does not compose a tree of children unless a tree is what the goal demands).

### 14.4 Uncomfortable questions this frame creates

1. **What is a pincer allowed to do autonomously vs. ask for approval vs. refuse outright?**
   Auto-spawn + auto-build + shell tool = escalation-by-default. Without explicit capability scoping, any pincer can spawn an army or install anything. The product must answer this _before_ the "build whatever it needs" promise is marketing.

2. **How much of Temporal are you rebuilding inside the wake loop?**
   Durable execution (checkpoint per step, resume on crash, deterministic replay) is the enabling infrastructure for reliable auto-spawn. You either embed a mature engine or rebuild the semantics. See §14.7.

3. **How does the user specify the goal well enough to be done correctly?**
   The thesis trades authoring complexity for specification complexity. Bad prompts + autonomous execution = expensive wrong answers. The ergonomic burden shifts from engineers to articulation.

4. **Where is the budget?**
   Budget today is per-agent. A conversation that spawns 12 pincers needs a **mission-level budget cap** that scopes across the whole spawn tree. Call this **G9 — mission-scoped budgets and capability delegation chains**. New gap, only exists under this frame.

### 14.5 What this frame rules out (forever, not just for now)

- Visual workflow builders of any kind.
- "Agent templates" as the primary authoring surface. Templates can exist as bootstrapping prompts but cannot be how users _specify_ behavior.
- A first-party integrations library. Agents curl documented APIs, because documented APIs are the integration layer. No `StripeTool`, no `GitHubTool`.
- Deterministic prompt pipelines. The whole point is the pincers decide the pipeline on the fly.

### 14.6 Primitive table, re-emphasized under this frame

| Primitive    | Old emphasis ("multi-agent platform")  | New emphasis ("conversation is the surface")         |
| ------------ | -------------------------------------- | ---------------------------------------------------- |
| Agents       | Named, addressable long-lived entities | Specialist pincers that self-assemble around a goal  |
| Executors    | Tool for an agent to use               | Substrate a pincer builds its own tools from         |
| Credentials  | Scope of an agent's reach              | Scope of capability delegation across pincer chains  |
| Data / files | Agent's memory                         | Shared work surface between pincers                  |
| Record       | Audit log                              | Replay-able proof of a self-assembled execution      |
| Process      | Wake loop                              | Durable multi-pincer computation                     |
| Proof        | Tamper-evident history                 | Tamper-evident history **plus** deterministic replay |

### 14.7 Durable execution in Rust — the Temporal-primitive question

If durable execution is the enabling infrastructure for auto-spawn + resumable multi-pincer work, the project has to make a **build / adopt / embed** decision. Surveyed options as of 2026-04:

| Option                                               | What it is                                                                                                                                                                                                                                                                                                                 | Fit for Open Pincery                                                                                                                                                                                                                                                                              |
| ---------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Temporal**                                         | Go core + SDK for many languages including Rust (community). Production-grade, battle-tested.                                                                                                                                                                                                                              | Heavy. Brings a Go process, gRPC, its own persistence backend. Breaks the "single Rust binary + Postgres" deployment story.                                                                                                                                                                       |
| **Restate**                                          | Durable execution engine, Rust core, open source. Growing.                                                                                                                                                                                                                                                                 | Closer stack fit than Temporal. Still a separate process. Worth a serious look if Phase C/E lands.                                                                                                                                                                                                |
| **Flawless** ([flawless.dev](https://flawless.dev/)) | Rust durable-execution engine. Workflows are **regular Rust functions compiled to WebAssembly** executed in a deterministic sandbox. Side effects (HTTP, clocks, RNG) are logged; on restart, the workflow is replayed and side effects are served from the log until the current state is reached. Beta 3 as of Dec 2024. | **Very interesting architectural fit.** WASM-as-sandbox aligns with the shell-as-codemode philosophy and could double as part of the G5 (sandboxed executor) answer. Wake loops become deterministic Rust fns. Maturity risk: beta, small community, appears one-person as of last public update. |
| **Cadence / workflow-rs / misc**                     | Research-grade Rust workflow crates.                                                                                                                                                                                                                                                                                       | Mostly toy. Not production candidates.                                                                                                                                                                                                                                                            |
| **Build it in-app**                                  | Extend wake loop with checkpoint events per tool call; on restart, replay events to the last checkpoint.                                                                                                                                                                                                                   | Matches G1 in §4. Minimal dependencies. Gets **durable boundaries but not deterministic replay of in-process compute.** Sufficient for single-agent wakes, insufficient for multi-pincer orchestration.                                                                                           |

**Decision axes:**

- **Single-binary-ness** is a hard preference today. Anything that adds a separate daemon (Temporal, Restate, Flawless-as-server) breaks the "docker compose up and go" promise unless packaged as an optional service.
- **WASM as executor** is an interesting convergence. If the sandboxed executor (G5) ends up being WASM-based anyway (Wasmtime + WASI), then Flawless's approach of "workflow is a WASM module" composes beautifully. The pincer's durable logic and its sandboxed tools would share a runtime.
- **Maturity vs. fit.** Temporal is mature and wrong-shape. Flawless is right-shape and immature. Restate is a middle ground.
- **Project risk of depending on Flawless specifically.** Beta, one public maintainer, small community. If adopted, plan for the possibility of forking or maintaining it yourself. That is not automatically disqualifying — the whole stack is small enough that vendoring a beta dependency is survivable — but it should be a conscious choice.

**Tentative recommendation (not a commitment):**

- **Phase B (proof):** build G1 in-app. Get durable boundaries (checkpoint per tool call) done natively. This is the 80/20 solution and unblocks everything downstream.
- **Phase C or later (credentials + executor + multi-pincer):** re-evaluate Flawless or Restate specifically for _multi-pincer orchestration_. If Flawless is still maintained and has matured, it is the single best architectural match on paper: WASM-sandboxed, deterministic-replay, Rust-native, no separate daemon required (embeddable library). If it has stalled, Restate is the fallback. Temporal is the "we ran out of options" choice.
- **Under no circumstances:** expose Temporal-style workflow authoring as a user-facing surface. It directly contradicts §14.1.

### 14.8 Consequence for the primitive gap ranking

The G1–G8 table in §4 was ranked under the old frame. Under this product thesis, centrality changes:

| Gap          | Old severity | Centrality to product thesis                                                                                                   |
| ------------ | ------------ | ------------------------------------------------------------------------------------------------------------------------------ |
| **G1**       | High         | **Central.** Multi-pincer spawn without durable execution = unreliable.                                                        |
| **G6**       | Medium       | **Central.** Pincers that cannot talk are not pincers.                                                                         |
| **G7**       | Medium       | **Central.** "Mission" is the user-facing unit of work.                                                                        |
| **G9 (new)** | n/a          | **Central.** Mission-scoped budgets + capability delegation chains. Without this, auto-spawn is unsafe.                        |
| **G2**       | High         | Important. Enables safe delegation across spawns.                                                                              |
| **G5**       | High         | Important. Enables safe auto-build of tools.                                                                                   |
| **G4**       | Medium-High  | Useful. Pincer IPC via files.                                                                                                  |
| **G8**       | Low-Medium   | Nice to have.                                                                                                                  |
| **G3**       | High (old)   | **Demoted inside the pure multi-pincer thesis, but still strategically important for the later provability/compliance wedge.** |

The new top-five are **G1, G6, G7, G9, G2** — four of them explicitly about enabling safe, durable, auditable multi-pincer execution. That is the shape of the product thesis.

This reprioritization is local to the product-thesis pass in §14. The later strategy work re-elevates tamper-evidence when discussing the initial commercial wedge and the provability bet. Read §14.8 as "what matters most for the self-assembling multi-pincer runtime," not as the final year-one go-to-market ordering.

### 14.9 The ergonomic bar — "specialist-ready in 5 minutes"

The phrase "ergonomic for getting stuff done in specialist ways" deserves its own test. A reasonable bar:

- A user who wants a **specialist** (e.g. a coding pincer that watches a repo, or a research pincer that writes daily digests) should go from _empty install_ to _working specialist_ in under five minutes, through conversation, with zero YAML.
- The specialist, once alive, should be addressable by name (`pcy message research-pincer "..."`), should persist forever, and should be able to spawn helper pincers for bounded sub-tasks without a human in the loop.
- The audit trail for that specialist should answer the question "what did it do yesterday?" in one CLI invocation.

If any of these bars is not met, the thesis is aspirational, not delivered. Today: all three are partially met (1 via `pcy demo`, 2 via messages but not spawning, 3 via `pcy events` but noisily). **None are clean.**

### 14.10 What changes in the roadmap

Under this thesis, §13's phase order tightens:

- **Phase A (CLI ergonomics)** stays first. The CLI is how the user talks to the pincery and how pincers talk to each other.
- **Phase B** becomes **G1 + G7 + G9** — durable execution + mission objects + mission-scoped budgets. The "missions" phase leads, not trails.
- **Phase C** stays G2 + G5 + G6 — credentials, sandbox, messaging. The three halves of "pincers can safely delegate to each other."
- **Phase D** becomes lower priority — filesystem + FTS are enablers, not thesis-critical.
- **Phase E** goes away as a separate step; it was missions, which is now in Phase B.
- **Phase F** (prune + document) is unchanged.

The order is: **talk to the system cleanly → make missions durable → make pincers safely collaborative → make shared work concrete → prune and document.**

---

## 15. Codebase audit vs. the strategic bet — where Open Pincery can actually shine

This section is grounded in a direct audit of `src/` on the post-v5 codebase (commit `9efe337`, v1.0.0). It is the first time in this document that claims about the code are backed by line-level evidence rather than `design.md` or memory.

### 15.1 Quiet wins already in the code (undersold by the docs)

These are real, working, and better than the docs advertise. They are the substrate a future version stands on.

- **CAS state machine is clean and lock-free.** Four single-statement conditional `UPDATE`s (`acquire_wake`, `transition_to_maintenance`, `release_to_asleep`, `drain_reacquire`) do all the concurrency control. No Redis, no Mutex, no distributed lock manager. This is the hard part of the runtime and it is ~40 lines of SQL across `src/models/agent.rs`.
- **Cost accounting is transactionally coupled to LLM calls.** `INSERT llm_calls` and `UPDATE agents.budget_used_usd` share one `sqlx::Transaction` in `src/models/llm_call.rs`. No double-billing on crash, no lost usage on rollback. Billing correctness is done.
- **`/ready` is real.** It checks DB health, migration count, and the atomic liveness of _every_ background task. This is a 3-line curl away from being a Kubernetes-grade readiness probe today.
- **Integration tests boot the actual binary.** `tests/cli_e2e_test.rs` spawns the real `pcy` binary against an ephemeral axum server and a real Postgres. The test suite is genuinely end-to-end, not a mock theatre.
- **Webhook HMAC + dedup via `ON CONFLICT DO NOTHING`** is crisp and correct.

Strategic implication: the **durable-state substrate is stronger than the docs claim**. The weak parts are above it, not below it.

### 15.2 Oversold surfaces (docs > code)

These are places where `design.md`, `scope.md`, or marketing copy imply a thing exists and the code says otherwise. Honest naming matters for the roadmap.

| Claim in docs                                         | Code reality                                                                                                                                                                             |
| ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Append-only event log                                 | Append-only **by code discipline only**. No DB `REVOKE UPDATE`, no trigger. One migration already mutates `events` to backfill a column.                                                 |
| Structured observability                              | Structured logs + Prometheus counters. **Zero `#[instrument]` macros, zero OTEL deps, zero trace propagation across `tokio::spawn`.**                                                    |
| Tool audit / permission modes / sandbox profiles      | Migration `20260418000011_create_tool_audit.sql` has a full columnar schema. **Zero writes, zero reads, zero enforcement in Rust.** `agents.permission_mode` is set but never consulted. |
| Pincer multi-agent runtime                            | The word "pincer" does not appear in any code path. No spawn, no message-between-agents, no subagent plumbing.                                                                           |
| Per-agent NOTIFY channels (`design.md` §architecture) | One global channel `agent_wake` with the agent UUID as payload. Fine, but different from the design.                                                                                     |
| "Universal executor"                                  | One tool (`shell`) that runs `sh -c` raw on the host. Two other tools (`plan`, `sleep`) are a no-op and a terminator.                                                                    |

Strategic implication: the **gap between the story and the code is largest in exactly the areas that will matter most as models get better**: tool execution, observability, and multi-agent collaboration.

### 15.3 Foot-guns and silent correctness bugs found by the audit

Not roadmap-critical, but they shape any "polish before scaling" story.

- **No timeout on `reqwest::Client` in `LlmClient::new`.** A hung LLM provider hangs a wake task indefinitely; only stale-recovery (default 2h) frees it.
- **No timeout, no sandbox, no working-dir constraint on `shell` tool.** A prompted `curl http://169.254.169.254` works today on any cloud VM. A `:(){ :|:& };:` works today.
- **Unbounded `tokio::spawn` in the NOTIFY listener.** No semaphore. A flood across many agents = unbounded tasks.
- **`last_tool_call_id` is a single `Option<String>` in prompt assembly.** If the LLM returns two parallel tool calls in one turn, the second `tool_result` gets an empty `tool_call_id`. Works today because of serial dispatch, brittle tomorrow.
- **Iteration cap increments only on tool-call turns.** A model that returns text + `finish_reason != "stop"` with no tool_calls has an ambiguous loop-exit path.
- **`prompt_hash` is a `wrapping_mul(31)` rolling hash**, not cryptographic, despite the "audit trail" comment.
- **10-argument `append_event` function** called from 15 sites, dispatching on stringly-typed `event_type`. One typo = a silent new event type with no handler.
- **Error swallowing in stale recovery** (`let _ = event::append_event(...)`). Audit events can be silently lost exactly when recovery runs — the worst time.
- **On Windows, `sh -c` does not exist.** The shell tool is unix-only with no platform branch, despite the `pcy` CLI being cross-platform.

Strategic implication: **Phase B must fix the executor and the LLM-call timeout before touching anything more ambitious.** These are not roadmap items, they are pre-conditions.

### 15.4 Shape of the code in one line

`src/` is **~3–4k lines of Rust** across tight modules; nothing near the 300-line brake. The codebase is dramatically smaller than the scaffolding and docs imply. That is good news: **the system is still steerable**. The cost of a strategic pivot is measured in weeks, not quarters.

### 15.5 What changes as models get better — the honest answer

The common story is "better models = more autonomy, longer horizons, less scaffolding." That is only partly right. The more precise prediction:

- **Reasoning quality improves → the _prompt_ matters less.** The marginal value of clever prompt engineering, retrieval tricks, and hand-tuned scratchpads decays. What persists is the **environment the model acts in**: tools, filesystems, credentials, other agents, durable state.
- **Context windows grow → "put everything in the prompt" stops being a strategy.** Not because it fails, but because it becomes wasteful and slow. The winners will be systems that _chose_ what to remember, not systems that pushed everything in and hoped.
- **Tool use gets better → the tools themselves become the ceiling.** A model that can use tools perfectly is bounded by the tools it has. A world-class coding model with only `sh -c` on the host is a liability. A world-class coding model with a typed, sandboxed, auditable tool surface is a product.
- **Multi-step planning improves → process specification replaces imperative scripting.** The model decides the steps; you specify the invariants, budgets, and proofs. LangGraph-style node graphs are the wrong abstraction for this future. Missions, capabilities, and receipts are the right ones.
- **Agent-as-operator becomes normal → trust becomes the product.** When every competent knowledge worker has an agent doing real work on their behalf, the differentiator stops being "does it work" and starts being "can I prove what it did, to whom, with what authority, under what budget."

Strategic implication: **the parts of Open Pincery that will age well are the parts most aligned with durable state, proof, capability scoping, and auditability.** The parts that will age badly are anything that tries to be clever about _the model's reasoning_.

### 15.6 Where Open Pincery can genuinely shine — six bets

These are not features. They are positions. Each is defensible as models improve, not in spite of improvement.

**Bet 1 — The event log as a first-class, queryable, provable artifact.**

Not just append-only, but:

- Cryptographically chained (SHA-256 of (prev_hash, event_payload, signing_key)), so tampering is detectable by anyone holding one valid hash pointer.
- Versioned payloads (`event_schema_version`, typed enum in Rust, codegen'd to JSON Schema).
- Queryable with SQL AND semantic search (pgvector on event summaries) AND full-text.
- Exportable as a signed bundle: "here is everything agent X did between $T_1$ and $T_2$, provably."

Why this ages well: better models produce more actions per unit time; regulators, clients, and the agent's own future self all need to reconstruct what happened. Nobody else in the agent-framework space treats the log as the product. Most treat it as debug output.

**Bet 2 — Capability-scoped credentials as values, not config.**

A credential is an object with: `(scope, owner_agent, parent_cap, expires_at, max_uses, constraints, signed_chain)`. Agents mint child capabilities from their own, with strictly narrower scope. Every tool call consumes a capability. The log records which capability was used. Revocation is by hash.

Why this ages well: as agents collaborate, the question "who authorized this action" becomes the first question every auditor asks. Ambient credentials (a `.env` file the agent can read) will look archaic. Capability tokens with cryptographic provenance will look inevitable.

**Bet 3 — Missions as the unit of work, not prompts or wakes.**

A mission is a durable object: `(id, goal, budget, deadline, acceptance_predicate, parent_mission, spawned_by, status)`. A wake operates on a mission. A pincer inherits the parent mission's budget slice and acceptance obligation. Success is checkable — the `acceptance_predicate` is code (or declarative), not vibes.

Why this ages well: "chat with the system" is the surface; "the system achieved your mission, here is the proof" is the receipt. Better models make missions more ambitious; the primitive is unchanged.

**Bet 4 — A sandboxed, typed, native-to-Rust tool surface.**

Not just "less unsafe shell." A deliberate surface:

- `fs.read / fs.write / fs.search` scoped to a workspace directory, tracked in the event log, diffable.
- `http.fetch` with allowlist per capability.
- `shell.run` in a WASM or Firecracker or gVisor sandbox, CPU/memory/time bounded, network-isolated by default.
- `agent.spawn` and `agent.message` with capability-bearer semantics.
- `mission.accept` with signed attestation.

Each tool has a typed signature, an event schema, and an audit category. Adding a tool is adding a file, not editing ten places.

Why this ages well: models will get better at _using_ tools faster than humans get better at _specifying_ them. A well-designed tool surface is a moat. `sh -c` is not a tool surface.

**Bet 5 — Replayability as a core user experience.**

The event log is enough to replay a wake. Make this an end-user feature, not a debug one:

- `pcy replay <wake_id>` — rerun the wake against a fresh agent, same inputs, see what a different model/prompt/tool would have done.
- `pcy diff <wake_id_a> <wake_id_b>` — compare two wakes that took different paths.
- `pcy what-if <wake_id> --model=<X>` — replay with a different model, show the delta.

This is the killer feature for the "my agent did something weird" moment. No agent framework today does this well. Open Pincery's event log already has most of the data; it needs the tooling and the discipline (e.g., never skip logging an LLM-visible byte).

Why this ages well: as agents do more, the "why did it do that" question gets louder. A system that can answer it precisely is a system people will pay for.

**Bet 6 — The CLI as an agent-native surface, not a human one.**

`pcy` today is a human CLI with JSON output. Flip the polarity:

- Every command has a `--json` (machine) and a `--human` (pretty) mode; machine is default if stdin is not a TTY.
- Every command has a stable, versioned schema.
- Every command writes to the event log with an actor principal.
- New verbs: `pcy mission create`, `pcy capability mint`, `pcy replay`, `pcy log export`, `pcy prompt compare`.
- An agent running inside a pincer can invoke `pcy` with its own capability token and do anything a human operator can do.

Why this ages well: the emerging convention is "agents are first-class users of CLIs." A CLI designed from day one to be used by both humans and agents, with the same audit trail, is a genuinely novel surface.

### 15.7 Creative moonshots — things no current agent framework does

If the boring bets above are the floor, these are the ceiling. Each is speculative, each would be novel.

**Moonshot A — Proof-carrying actions.**

Every `tool_call` event carries not just the inputs and outputs, but a **witness**: a small piece of evidence (hash, signature, transcript excerpt, cryptographic proof) that the action had the effect claimed. `fs.write` carries the pre/post file hashes. `http.post` carries the response hash and timestamp from a trusted source. `payment.send` carries the on-chain receipt. The log is not a record of what the agent _said_ it did; it is a record of what it _provably_ did.

This is a bridge between the agentic-systems world and the verifiable-compute world. No agent framework does this. It would make Open Pincery the obvious substrate for agents doing regulated, financial, or legal work.

**Moonshot B — The pincery as a marketplace of typed pincers.**

A pincer is a typed agent: `(input_schema, output_schema, cost_estimate, latency_estimate, capability_requirements)`. An agent solving a mission shops for pincers the way a human shops for contractors: "I need a typed-output PDF-extractor pincer, budget $0.10, latency <30s, capability = read-only filesystem on /tmp." The pincery returns a ranked list; the agent picks one; the mission log records the trade.

Pincers can be first-party (bundled), local (user-defined), or remote (downloaded and sandboxed). The marketplace is the differentiator — not the agent loop.

This reframes Open Pincery from "an agent framework" to **"the operating system for specialist agents"**. That is a much bigger product.

**Moonshot C — Continuous self-compression as a first-class loop.**

Maintenance today writes a ≤500-char summary. Replace it with a real learning loop:

- After each wake, a second (cheaper) LLM proposes updates to a `lessons` table keyed by (agent_id, topic_hash).
- Lessons are retrievable at prompt assembly time by the current situation's topic hash.
- Lessons that never get retrieved get pruned. Lessons that get retrieved and followed get reinforced. Lessons that get retrieved and overridden get demoted.
- The agent's "personality" emerges as the equilibrium of its lesson table.

This is not RAG over documents. It is RAG over the agent's own past judgments, with a measurable utility signal. Every existing agent framework either does no learning or does fine-tuning. Nobody does structured, auditable, reversible lesson accumulation.

**Moonshot D — Missions as tradeable obligations.**

A mission is an object with a budget, a deadline, and an acceptance predicate. Objects of that shape are negotiable. Two agents (human or AI) can trade missions. One agent can forward a mission it cannot finish to another agent with better tools, transferring the residual budget and the obligation.

This is the agent-native analogue of a work-item marketplace. It only works if missions are durable, typed, and capability-bound — which is exactly what Bet 3 builds.

**Moonshot E — The honest agent — default-visible mistakes.**

An agent that, every wake, runs a secondary "red-team" pass over its own log and asks: "if someone audits me, what did I do in the last 24h that I cannot justify?" It surfaces the worst item to the owner proactively. Not hallucination detection — behavior self-critique.

This is a small feature with a huge trust footprint. Every other framework hides errors behind graceful fallbacks. Open Pincery could default to showing them.

### 15.8 What _not_ to build — sharpened by the audit

The audit reinforces the "junk candidates" section (§3), but with more confidence:

- **Anything that tries to make the model smarter with clever prompting.** Models are getting smarter on their own. Invest in the environment, not the prompt.
- **Anything that hard-codes a control-flow graph** (LangGraph, n8n-style nodes). Missions + acceptance predicates are the right abstraction; flowcharts are the wrong one.
- **A second opinionated UI surface.** The static-HTML dashboard is fine for ops. A full React SPA would be a distraction. The product surface is the CLI and the conversation; the browser is infra.
- **Generic plugin/extension systems.** Typed pincers with capability contracts are better than untyped plugins. Don't ship both.
- **LLM model abstraction layers.** One OpenAI-compatible HTTP client + per-provider adapters where needed is fine. LangChain-style `LLM` interfaces with 15 backends are not fine; they leak the worst of every provider.

### 15.9 The one-sentence strategic bet

> **Open Pincery wins by being the most _provable_ substrate for agentic work — not the smartest, not the fastest, not the most featureful.**

Every bet above is a specialization of that sentence. The audit confirms the substrate is real and salvageable. The gap between the story and the code is largest in the areas where the market is about to move. That is good news: Open Pincery has the rare combination of **working primitives + room to differentiate + a small enough codebase to pivot**.

### 15.10 Concrete next actions (if this framing holds)

Not a commitment — a menu, ordered by leverage per week of work.

1. **Cap and harden the shell tool.** 30-minute fix. Adds `timeout`, optional `cwd`, optional working-dir allowlist. Unblocks everything.
2. **Add `reqwest` timeout to `LlmClient`.** 5-minute fix. Kills the 2h hang bug.
3. **Strongly type `event_type` as a Rust enum.** One-day refactor. Eliminates the typo class of bugs. Sets up versioned event payloads.
4. **Introduce `Mission` as a first-class table and a first-class primitive in the wake loop.** One-week slice. Unlocks Bet 3, Bet 4, Moonshot D.
5. **Chain events with a rolling SHA-256 hash.** Two-day slice. Unlocks Bet 1 and every compliance conversation.
6. **Typed tool registry with event-schema + audit-category + capability-requirement per tool.** Two-week slice. Unlocks Bet 2, Bet 4, Moonshot A.
7. **Replay CLI (`pcy replay`, `pcy diff`).** One-week slice, mostly CLI + a prompt-assembler refactor. Unlocks Bet 5 and an enormous amount of goodwill.
8. **Pincer spawn + inter-agent messaging with capability-bearer semantics.** Two-week slice. Unlocks Bet 4's `agent.spawn`, Moonshot B, and the entire "conversation is the surface" thesis.

The first two are strict pre-conditions. The remaining six line up with Phase B → Phase C of the roadmap in §13, reweighted by §14. Items 3–5 are the "prove it" spine. Items 6–8 are the "make it shine" spine.

None of these require betting on a specific LLM generation. All of them get more valuable, not less, as models improve.

---

## 16. OpenClaw + what else this document is missing

### 16.1 OpenClaw reality check — it is a _gateway_, not an agent

The existing `docs/input/competitive-landscape.md` lists OpenClaw as _"The original open-source autonomous AI agent. Single agent, session-based."_ **That entry is stale and wrong.** A direct read of `docs.openclaw.ai` shows what OpenClaw actually is today:

> _OpenClaw is a self-hosted **gateway** that connects your favorite chat apps and channel surfaces — built-in channels plus bundled or external channel plugins such as Discord, Google Chat, iMessage, Matrix, Microsoft Teams, Signal, Slack, Telegram, WhatsApp, Zalo, and more — to AI coding agents like Pi. You run a single Gateway process on your own machine (or a server), and it becomes the bridge between your messaging apps and an always-available AI assistant._

Key facts from the docs:

- **Scope**: multi-channel ingress/egress gateway. Not an agent runtime. Not a memory layer. Not a tool executor.
- **Agent**: bundles "Pi" by default in RPC mode; agents are pluggable via RPC.
- **Sessions**: "isolated sessions per agent, workspace, or sender." Routing is its core competency.
- **Extras**: WebChat control UI, mobile "nodes" for canvas/camera/voice, media support (images/audio/docs), plugin channels.
- **Install**: `npm install -g openclaw@latest && openclaw onboard --install-daemon`. Node 24.
- **License**: MIT.
- **Config**: `~/.openclaw/openclaw.json`, allowlist per channel, mention rules for groups.

**Strategic implication: OpenClaw is a complement, not a competitor.** It is exactly the ingress layer Open Pincery does not have and has no business building. The pairing is obvious:

```
WhatsApp/Signal/Slack/Telegram/iMessage/Matrix/Teams/Discord/…
   ↓ (channel plugins)
OpenClaw Gateway (sessions, routing, channel I/O, media)
   ↓ (RPC or HTTP)
Open Pincery (mission substrate, pincers, event log, proofs, capabilities)
```

OpenClaw answers _"how does a human send a message to an agent from their pocket."_ Open Pincery answers _"what happens after the message arrives, durably and provably."_ They are the two halves of the self-hosted agent-OS story.

**Concrete consequences:**

1. **Do not build a chat-channel subsystem.** OpenClaw already did. Ship an OpenClaw channel plugin or an "Open Pincery as RPC agent" adapter instead. Budget: a week, maybe two.
2. **Fix the competitive-landscape.md entry.** The current entry positions OpenClaw as a competitor, which misleads strategy and misleads anyone reading the doc. It should be reclassified to a "Complementary / ecosystem" section.
3. **Borrow their install story.** `openclaw onboard --install-daemon` is dramatically better UX than "clone repo, run cargo, set up Postgres, write a Caddyfile." Section §12 already flagged the CLI gap; §15.10 item 1 already flagged the shell-tool fix; a first-class installer (see §16.4 below) belongs on that list too.
4. **Decide who owns the "user" concept.** If OpenClaw owns channels, sessions, and senders, Open Pincery should not reinvent those. Open Pincery owns missions, pincers, capabilities, events, and budgets — keyed by whatever principal OpenClaw forwards. This is a clean boundary and it should be drawn explicitly in the design doc when this direction is taken.
5. **Joint positioning is an option.** "Open Pincery + OpenClaw" can be marketed as a reference stack for self-hosted agentic infrastructure. Neither project loses identity; both gain surface area.

### 16.2 What categories of thinking this document has been missing

The previous fifteen sections are strong on **primitives, runtime, proofs, and the agent-facing surface**. A critical pass surfaces seven categories where the thinking is thin or absent. Each is a gap worth naming before any iteration is scoped.

| #   | Category                                                                                | Status before §16                                           | Why it matters                                                                                                                                  |
| --- | --------------------------------------------------------------------------------------- | ----------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| M1  | **Ingress / channel surfaces** — how work enters                                        | Assumed: API POST + webhooks only                           | Every real user gets work into agents via chat, email, cron, file-watchers, git hooks. OpenClaw solves the chat slice.                          |
| M2  | **Scheduling / time primitives** — wake-at, cron, deadlines, watchdogs                  | Not in the primitive list                                   | Missions have deadlines; wakes can be scheduled; "remind me Tuesday" is table stakes. Time is not a tool. It is a primitive.                    |
| M3  | **Model portability** — local, hybrid, fallback, routing by cost/latency/capability     | `LlmClient` is OpenAI-compatible HTTP, one adapter          | Ollama, vLLM, llama.cpp, Anthropic direct, per-mission model routing all get more important as model diversity explodes.                        |
| M4  | **Human-in-the-loop** — approvals, confirmations, escalations                           | Not mentioned                                               | Capability-scoped credentials mean _some_ actions need a human "yes." No such primitive exists.                                                 |
| M5  | **Evaluations / agent regression testing** — not just code tests                        | Not mentioned                                               | Integration tests test the runtime. Nothing tests whether the agent still makes the right decisions after a prompt/model change.                |
| M6  | **Distribution / install story** — one-liner for real humans                            | Acknowledged as weak in §12, not in §15.10                  | If the install story is "write a Caddyfile," the TAM is ~400 people.                                                                            |
| M7  | **Identity, federation, and governance** — DIDs, cross-pincery discovery, trust roots   | Not mentioned                                               | Two pinceries will eventually want to talk. Agent identity as a DID, signed capability chains across pinceries, is the right long-term design.  |
| M8  | **Data ingestion & knowledge primitives** — files, docs, email, calendar as first-class | Files/workspaces mentioned, nothing about sync or ingestion | An agent without a way to know about the user's world is a chatbot.                                                                             |
| M9  | **Error taxonomy** — typed error categories, not strings                                | Not mentioned                                               | Budget exceeded, capability denied, tool timeout, mission predicate failed, model refused — these are different things. Today they are strings. |
| M10 | **Rate limiting & fairness per capability** — not just per user                         | Not in the primitive list                                   | A runaway agent with a valid cap can still burn a user's budget in seconds. Per-capability token-bucket is cheap and essential.                 |

Gaps M1 and M6 are partially resolved by **"use OpenClaw for ingress and borrow its install UX."** That is the single biggest lift from this crawl.

### 16.3 Agentic-ecosystem signals that sharpen the bet

A broader scan of what has shipped in agent frameworks in the last ~6 months (late 2025 through Q1 2026):

- **Anthropic Computer Use / Claude Code subagents** are establishing "the agent is a programmer with a keyboard" as the default mental model. Open Pincery's `shell` tool is aligned; the sandbox is the gap.
- **OpenAI Assistants / Responses API** is consolidating on state + threads + tools on their side. Self-hosted is a diverging market, not a parallel one. Open Pincery is a self-host story, not an OpenAI-API story.
- **Letta / MemGPT** continue to push memory-as-a-service with a graph/vector-hybrid store. Open Pincery's event-sourced approach is philosophically different (append-only + projections + replay) and ages better under compliance pressure.
- **Inngest AgentKit, Temporal AI patterns, Restate, Flawless** are all converging on _durable execution_ as the primitive for agent reliability. §14 already covered this. The signal: whoever makes durable execution _feel native to agentic work_ wins that slice.
- **MCP (Model Context Protocol)** is becoming the default tool-plumbing standard. Open Pincery should speak MCP on both sides — expose its tools as an MCP server, consume external MCP servers as tools. This is a two-to-three-week slice with outsized ecosystem leverage.
- **OpenAgents / agent.dev / sim-pipelines** are pushing the "marketplace of agents" idea (Moonshot B in §15.7 is this, but with typed pincers + capability contracts as the differentiator).

**New strategic implication: speaking MCP is probably more important than any single moonshot.** It is the protocol moat of the coming year, and it is a small lift.

### 16.4 Additions to the §15.10 action list

Revising the ordered menu from §15.10 with what §16 adds:

| #   | Action                                                                  | Leverage        | Notes                                                                                       |
| --- | ----------------------------------------------------------------------- | --------------- | ------------------------------------------------------------------------------------------- |
| 1   | Shell tool timeout + cwd + allowlist                                    | Highest         | Pre-condition. Unchanged.                                                                   |
| 2   | `reqwest` timeout on `LlmClient`                                        | Highest         | Pre-condition. Unchanged.                                                                   |
| 3   | Strongly typed `event_type` + versioned payloads                        | High            | Unchanged.                                                                                  |
| 4   | Mission primitive (table + runtime + CLI)                               | High            | Unchanged.                                                                                  |
| 5   | Event chain hashing (rolling SHA-256)                                   | High            | Unchanged.                                                                                  |
| 6   | **NEW: MCP server + MCP client**                                        | **High**        | Speaks the emerging protocol. 2–3 weeks. Massive ecosystem leverage per §16.3.              |
| 7   | Typed tool registry + capability contracts                              | High            | Unchanged.                                                                                  |
| 8   | `pcy replay` / `pcy diff`                                               | Medium-high     | Unchanged.                                                                                  |
| 9   | **NEW: OpenClaw channel adapter (Open Pincery as RPC agent)**           | **Medium-high** | ~1 week per §16.1. Unlocks chat ingress on all 12+ platforms OpenClaw supports.             |
| 10  | **NEW: Time primitive — `wake_at`, deadlines, mission TTL**             | **Medium**      | Per M2. One-week slice. Makes missions actually bounded.                                    |
| 11  | **NEW: First-class installer — `pcy init` / Homebrew / Cargo binstall** | **Medium**      | Per M6. Two-to-three day slice. Dramatically widens TAM.                                    |
| 12  | Pincer spawn + inter-agent messaging                                    | Medium          | Unchanged. Unlocks "conversation is the surface."                                           |
| 13  | **NEW: Typed error taxonomy**                                           | **Medium**      | Per M9. Half-week slice. Foundation for observability and replay.                           |
| 14  | **NEW: Human-in-the-loop approval primitive**                           | **Medium-low**  | Per M4. Best layered after capabilities ship.                                               |
| 15  | **NEW: Agent eval harness** (`pcy eval`)                                | **Medium-low**  | Per M5. Fills the regression-testing gap. Can start as a CLI over the replay tooling in #8. |

Items 1–5 are pre-condition and spine work. Items 6–9 are the ecosystem bets. Items 10–15 are the rounding-out bets.

### 16.5 The updated one-sentence bet, unchanged in spirit

> **Open Pincery wins by being the most _provable_ substrate for agentic work, with the _cleanest boundaries_ to complements like OpenClaw for ingress and MCP for tools.**

Provability is still the core. The addition from §16 is **humility about scope**: Open Pincery should not try to own channels, tools, or models end-to-end. It should own missions, pincers, capabilities, events, proofs, and budgets — and speak clean protocols at every other boundary.

### 16.6 What this document still will not answer

For transparency, here is what §1–§16 still does **not** resolve. These are real questions and they require human judgment, not more analysis.

1. **Who is the first paying / sticky user, by name?** Every bet above assumes a buyer. No specific person has been named.
2. **Self-host-only or self-host + optional SaaS?** Still open from §7.1. Large consequence for ~25–30% of code.
3. **License choice for the "provable" differentiators** — MIT for everything, or BSL/AGPL for the replay/proof surfaces? Unasked so far.
4. **Governance** — single-maintainer or small circle? Who decides what ships in the substrate?
5. **The first real mission, written down.** Not a toy. What is the actual first mission a real user hands to an agent in Open Pincery and gets paid value from? Until this is a sentence on a page, everything above is pre-product.

These five questions are out of scope for a thinking document. They are the input to the next `/iterate` confirmation — or to a decision meeting with a human in the chair.

---

## 17. Should OpenClaw be a pincer? (Strategic boundary question)

Short answer: **no, and the question itself hints at the right architectural decision — make "pincer" a protocol, not a process.**

### 17.1 Why OpenClaw-as-a-pincer is a category error

OpenClaw and a pincer are shaped for opposite lifecycles:

| Dimension         | Pincer (Open Pincery agent)        | OpenClaw gateway                    |
| ----------------- | ---------------------------------- | ----------------------------------- |
| Lifecycle         | wake → act → sleep; CAS-gated      | long-lived daemon, always-on        |
| Driver            | LLM reasoning loop                 | channel I/O loop                    |
| State             | event log + projections + missions | sessions + channel connections      |
| Language          | Rust, single binary                | Node.js, npm-installed daemon       |
| Responsibility    | decide what to do next             | deliver messages to the right place |
| Success criterion | mission acceptance predicate       | message delivered, session intact   |

Running OpenClaw _inside_ a pincer wake loop is waste: you would wrap a stateful Node daemon in a CAS-gated LLM loop it does not need. Running a pincer _as_ OpenClaw is equally wrong: you would bolt channel routing into the agent runtime and lose the clean boundary from §16.1.

OpenClaw is a **peer role**, not a pincer. It is the ingress layer. Pincers are the reasoning layer. The right integration is the RPC adapter from §16.1 — not inversion of either runtime.

### 17.2 The better question hiding underneath

"Should OpenClaw be a pincer?" is really asking: **"What is the boundary of what counts as a pincer?"** That is the genuinely interesting question, and it has three plausible answers.

**Option P1 — One canonical harness.** (Status quo.) Open Pincery ships _the_ Rust wake loop. Every pincer is a row in `agents` running that loop. Cheap, consistent, single audit surface. Downside: specialization is hard; a vision-heavy pincer, a code-heavy pincer, and a planning-heavy pincer all share one prompt-assembly strategy.

**Option P2 — Pluggable harness, one implementation.** Open Pincery exposes traits (`TickLoop`, `PromptAssembler`, `ToolRouter`) but ships only one default impl. Users can replace parts in-tree with their own Rust code. Slightly more flexible; still all-Rust; still one deployment unit. Downside: "I want my pincer in Python" is not answered.

**Option P3 — Pincer as a protocol, multiple harnesses.** Open Pincery defines _what a pincer must do to count as a pincer_: a stable wire protocol over the event log, CAS lifecycle, capability consumption, budget reporting, mission participation. Any process speaking that protocol — Rust, Python, Node, a WASM module, a remote service — is a legitimate pincer. The default harness is still shipped (the current Rust loop). But a Python harness, a Node harness, a WASM-sandboxed harness, or someone else's closed-source specialist harness are all first-class citizens as long as they honor the protocol.

**The recommendation is P3, and the audit backs it up.** Five reasons:

1. **Moonshot B (marketplace of typed pincers) requires P3.** A marketplace where every seller ships Rust source is not a marketplace. A marketplace where every seller ships a protocol-compliant binary/image/WASM is.
2. **The provability bet (§15.9) is unchanged by P3.** What makes a pincer auditable is _the events it emits and the capabilities it consumes_ — not the language it was written in. Shift the invariant from "runs our Rust loop" to "emits our event protocol." Proof survives.
3. **It answers "can I write a pincer in Python" without importing LangChain.** Specialists need specialist tooling. A vision pincer probably wants Python + torch. A code-review pincer wants Rust + tree-sitter. A workflow pincer may want a WASM sandbox. P3 lets each pincer pick its own stack without fragmenting the substrate.
4. **It aligns with MCP (§16.3).** MCP is already a protocol-first design for tools. If tools speak a protocol and pincers speak a protocol, the whole system becomes composable at protocol boundaries.
5. **The audit shows the current Rust loop is small enough to be the reference impl, not the only impl.** ~3–4k lines (§15.4). A protocol spec plus the reference harness is a week or two of cleanup, not a ground-up rewrite.

### 17.3 What "pincer as a protocol" actually specifies

The protocol has to be precise or it is not a moat. A minimal first cut:

A **pincer** is any process that:

1. **Holds a CAS lease on an `agents` row** via the existing state machine (`asleep → awake → maintenance → asleep`). Expressed as HTTP endpoints or a gRPC service — not internal function calls.
2. **Reads its input** as an ordered stream of typed events (`message_received`, `tool_result`, `mission_assigned`, `capability_granted`, `webhook_received`, …) delivered via subscription or pull.
3. **Writes its output** only through a small set of typed event kinds (`assistant_message`, `tool_call_requested`, `mission_accepted`, `pincer_spawn_requested`, `wake_complete`). Never mutates rows directly.
4. **Consumes capabilities** at tool-call time, passing capability tokens whose hashes are recorded by the substrate.
5. **Reports budget usage** via a typed event (`budget_consumed`) that the substrate aggregates.
6. **Honors deadlines** — the substrate can cancel a wake; the pincer must release the CAS lease within N seconds of a cancel signal.
7. **Declares its identity** at registration: `(name, version, input_schema, output_schema, capabilities_required, cost_estimate, latency_estimate)`.

Anything satisfying all seven is a pincer. Everything else is just another RPC server.

### 17.4 Where OpenClaw lands in this picture

OpenClaw does not satisfy the pincer contract — and should not. It is a **capability-producing peer**, not a capability-consuming one. Specifically, OpenClaw:

- **Produces** `message_received` events for Open Pincery (via the adapter from §16.1).
- **Consumes** `assistant_message` events from Open Pincery (delivering them to the originating channel).
- **Holds no CAS lease**, runs no LLM, consumes no mission budget.

The clean characterization: **OpenClaw is an _actuator + sensor_ on the ingress boundary**. Pincers are the _reasoners_. Missions are the _goals_. Capabilities are the _authority_. The event log is the _proof_. Each role is separate; each role has a protocol at its edge.

### 17.5 Consequence for §15.10 / §16.4 ordering

If P3 is adopted, the action list reorders slightly:

- **Item 4 (Mission primitive)** stays spine work.
- **Item 7 (Typed tool registry + capability contracts)** grows in scope to include the pincer-protocol events (`mission_accepted`, `pincer_spawn_requested`, `budget_consumed`). Two-week slice becomes three-week.
- **A new item: "Pincer Protocol v1 spec + reference harness extraction."** Roughly a week on top of items 3, 5, and 7 — mostly documentation + a small refactor that lifts the current wake loop into a reference implementation of the public protocol.
- **Item 12 (Pincer spawn + inter-agent messaging)** now becomes trivially clean: spawn means "start another process that speaks the pincer protocol, with a scoped capability and a sub-mission." It is no longer a custom runtime feature; it is protocol invocation.
- **MCP (Item 6)** gets a sibling: the pincer protocol is Open Pincery's equivalent of MCP for the reasoning layer. Two protocols, two boundaries, clean surface.

### 17.6 The refined strategic sentence

> **Open Pincery wins by owning the _substrate of provable agentic work_ and the _protocols at its boundaries_ — MCP outward to tools, the Pincer Protocol inward to reasoners, and a thin RPC to ingress peers like OpenClaw — rather than owning any single loop, language, or channel end to end.**

This is §16.5 with one more degree of humility: the **reasoning loop itself** is not where the moat lives. The protocols, events, capabilities, and proofs are. OpenClaw's question forced the sharper answer.

---

## 18. What §1–§17 still does not see — the genuinely-missing layer

This section resists padding. Every item below is a real gap in the thinking so far, not a rephrasing. Ordered roughly by how much damage each could do if left unexamined.

### 18.1 Unit economics — the math nobody has done

The document has spoken freely about budgets, cost accounting, and per-mission spending. It has not once computed what a wake actually _costs_ or what Open Pincery's gross margin looks like at any scale.

Back-of-envelope a real user might ask for:

- Average tokens per wake (prompt + completion): the current codebase loads 200 events + projections + templates. Realistic prompts are likely 10k–30k input tokens, 1k–4k output tokens per wake turn. A typical wake has 2–8 turns.
- At Claude 3.5 Sonnet rates (~$3 / $15 per MTok as of early 2026 comparable tiers): a wake is $0.05–$0.80. At Haiku/GPT-4o-mini tiers: $0.005–$0.08.
- A user with 20 active agents each waking 10× per day: $10–$160/day in model costs alone. The Postgres and host costs are rounding error next to that.
- Open Pincery today passes model costs straight to the user's provider bill. **There is no revenue model in scope.** Even a self-host-free product needs to survive — via support, hosted SaaS, premium pincers, compliance artifacts, or patronage.

Why this matters: every strategic bet above assumes someone will care enough to run this. If the pass-through cost per user-day is $50 and the value is "I could have just used ChatGPT for $20/mo," no amount of proof or capability design saves the product. The unit economics have to be modeled before any wedge is picked.

### 18.2 Thesis-death scenarios — what kills this?

A strategy that cannot name its own failure modes is not a strategy. Specific scenarios that would kill or gut Open Pincery:

1. **OpenAI or Anthropic ships "Projects with capabilities."** If either vendor bundles durable agents + scoped credentials + audit + replay into their product, the self-host value proposition collapses for ~80% of buyers.
2. **MCP expands to include agent-runtime protocols.** If the MCP ecosystem absorbs the pincer-protocol problem (§17.3) before Open Pincery publishes one, Open Pincery becomes a minor implementation of someone else's spec.
3. **A well-funded competitor owns the compliance story.** If Temporal or Vercel or a YC-backed startup ships "auditable agents for SOC2" first, Open Pincery's provability bet becomes a feature, not a product.
4. **The "conversation is the surface" thesis is wrong.** Real enterprise buyers may want dashboards, workflow authoring, and visual pipelines. If the market consolidates around LangGraph/n8n-style authoring despite the §14 critique, Open Pincery's rejection of that surface becomes a liability.
5. **Rust is the wrong bet for the agent ecosystem.** If the ecosystem standardizes on Python (for ML integrations) or TypeScript (for tool ecosystem), a Rust substrate with a protocol boundary helps but doesn't fully rescue the contributor pool.
6. **Self-hosting AI plateaus.** If regulatory or vendor pressure forces cloud-only inference for frontier models, "self-hosted agent substrate" loses its point.

Only one of these is inside the project's control (#4 — framing). The others are bets on the environment. **Each should have a named tripwire** — the signal that would make Open Pincery pivot or fold rather than burn effort against it.

### 18.3 Threat model — actual adversaries, not just foot-guns

§15 lists foot-guns. Foot-guns are accidents. A real threat model names attackers and what they want. None of this has been written down.

- **Malicious pincer author** (post-P3): ships a protocol-compliant pincer that exfiltrates via allowed `http.fetch` or logs capability-bearing tokens. Mitigation: capability scoping, egress allowlisting, signed pincer registry, per-pincer runtime quotas.
- **Compromised capability token**: leaked via `shell` output, log file, or a peer pincer. Mitigation: short TTL, one-shot caps, capability revocation by hash, audit of every use.
- **Runaway agent burns user budget**: a well-intentioned pincer loops on a failing tool. Mitigation: per-mission budget hard stop (not just soft accounting), per-capability token buckets (M10 in §16.2), circuit breakers on tool failure rates.
- **Prompt injection via channel or webhook content**: ingested user content tricks the agent into misusing capabilities. Mitigation: capability policy that cannot be overridden by prompt content, signed capability delegation, input provenance tracking in the event log.
- **Supply-chain attack on a pincer binary or MCP server**: same risk as any registry-based ecosystem. Mitigation: reproducible builds, signed manifests, SBOM requirement in the pincer protocol.
- **Host compromise**: attacker gets shell on the box. At minimum, rotate-by-hash capability revocation, offline-signed event log head hash stored elsewhere, and event-log-chain verification on every startup.

Provability (§15.9) is about _post-hoc proof_. A threat model is about _prior containment_. Open Pincery has hand-waved the threat-model side so far.

### 18.4 Cold start — the first ten minutes problem

The thinking has stayed at the substrate level. What happens when a new user runs `pcy init` on an empty machine?

They have zero agents, zero missions, zero pincers, zero templates, zero example capabilities. The CLI is a dozen commands they've never seen. The chat surface (§14) needs someone to chat _with_.

Most new-user drop-off happens in the first ten minutes. The document has not named:

- **Starter pincers** that ship in the box (a "writer," a "researcher," a "coder"?). Who designs them? Where do they live? Are they examples or the product?
- **A guided first mission** — a conversation that creates a small, successful outcome in under ten minutes. What is it?
- **The equivalent of "Hello, world"** for a pincer — the smallest thing that proves the substrate works to a skeptical developer in five minutes flat.

This is a product question, not an engineering one. But if it is not answered, nothing else in §1–§17 reaches a user.

### 18.5 The "why not build on Temporal/Restate/Flawless" question, answered properly

§14 tentatively recommended in-app checkpoints for Phase B and re-evaluating Flawless/Restate for Phase C. That is a tactical recommendation. The strategic version of the question — **why build a new substrate at all, rather than thin agents on Temporal?** — has not been answered rigorously.

The honest answer is some combination of:

1. **Event shape.** Temporal's durable primitives are workflow and activity. The agent primitive is an append-only log with projections, missions, and capabilities. Shoehorning agents onto Temporal means reinventing the event log on top of Temporal's activity log. Possible but awkward.
2. **Single-binary deployment.** Temporal requires its own cluster. Restate needs a runtime. Open Pincery is one binary + one Postgres — matching the self-host thesis.
3. **Protocols at boundaries, not dependencies at the core.** Adopting Temporal makes Temporal a dependency; adopting the pincer protocol makes Temporal one of many possible harnesses under it.
4. **Rust-native ecosystem coupling.** Rust axum + sqlx + tokio is a coherent stack. Temporal's Rust SDK is not first-class.

That is a plausible case. It should be written down once, and _stress-tested against the thesis-death scenarios_ above. If Temporal-with-agents becomes the market, points 1–4 have to stand up.

### 18.6 Contributor pool and velocity — the hiring paradox

Rust + Postgres + event sourcing + agent runtime + protocol design is an unusually narrow skillset. The current project is effectively solo-maintained. The roadmap in §15.10 + §16.4 + §17.5 is now 15+ items, many two-to-three-week slices. Realistic solo velocity caps this at ~6–10 items per year — meaning the full thesis is a multi-year commitment _without any contributor help_.

Consequences the thinking has not confronted:

- **The protocol-boundary bet (§17) is also a contributor-pool bet.** P3 lets Python/TypeScript devs build pincers against a wire protocol without learning the Rust substrate. This dramatically expands the contributor pool — but only if the protocol is documented and the reference harness is clean.
- **The provability bet requires depth, not breadth.** Cryptographic chaining, capability tokens, replay correctness — these are hard to review. Solo-maintainer plus deep crypto primitives is a code-quality risk.
- **"Staff a small team" is a business decision, not a technical one.** If it is not going to happen, the roadmap has to be triaged against one person's realistic throughput, which means choosing _one_ of {provability, protocol, marketplace, replay, MCP} as the first-year focus and deferring the rest.

### 18.7 Compliance wedge — name the specific regime or drop the claim

"Provable" and "auditable" are abstract. The concrete compliance regimes that care about these properties and would pay for them:

- **EU AI Act** (obligations for high-risk AI systems effective 2026–27). Article 12 requires automatic event logging; Article 13 requires transparency; Article 14 requires human oversight. Open Pincery's event log + capabilities + HITL primitive maps naturally.
- **NIST AI RMF** (US, voluntary framework). GOVERN-4, MAP-3, MANAGE-2 subcategories speak directly to audit trails and decision provenance.
- **SOC 2 Type II** for a SaaS AI vendor (not Open Pincery itself, but anyone deploying agents under SOC 2). Auditors want a trail; Open Pincery can be _that_ trail.
- **HIPAA / PHI workflows**. Provable non-disclosure, capability-scoped data access, and provable action logs are exactly what OCR auditors ask about.
- **Financial services (SR 11-7 model risk, MiFID II recordkeeping)**. Regulators already require replayable decision evidence. Agents doing financial work without this are unsellable into regulated buyers.

**The wedge question: which of these is first?** Picking one narrows the replay/proof feature set dramatically. "Auditable" without a regime is marketing. "EU AI Act Article 12-compliant event log" is a product.

### 18.8 The benchmark that ends arguments

Every winning infra project has one demo that kills the debate. Postgres has `EXPLAIN ANALYZE` on a big join. Rust has the zero-cost abstractions compile output. Temporal has the "we killed the worker mid-task and it resumed" demo.

Open Pincery's equivalent, on a napkin: **"An agent ran autonomously for 30 days under a $50 budget, executed 847 missions, and here is a cryptographically-signed audit bundle for every action it took — including one where it refused a capability escalation and asked the human for approval."**

No such demo exists, or could exist, from the current codebase. But it is a concrete target. Every item in §15.10 / §16.4 / §17.5 either contributes to this demo or it doesn't. That is a useful triage function.

### 18.9 Data gravity, lock-in, and the exit story

If Open Pincery succeeds, a user will have months of events, missions, projections, and capability chains inside one instance. Two unasked questions:

1. **Can they leave?** Is there an export that produces usable data elsewhere? Signed event bundles (§15.7 Bet 1) answer part of this but not the "resume this agent on another system" part.
2. **Can they federate?** Can two Open Pincery instances share missions, delegate capabilities, or merge event logs? §16.2-M7 named this; §17.6 hinted at it with "thin RPC to ingress peers." It has not been designed.

Data-gravity without an exit story is a moat that becomes a liability the moment trust is tested. Data-gravity _with_ a clean exit story is the strongest possible position: "we're not locked in, but nothing is better."

### 18.10 Long-horizon agent psychology — the month-30 problem

One concern no existing framework has solved: **what does a pincer that has existed for 30 days feel like to its owner?**

- How is its identity expressed when its event log is 40,000 entries long?
- What does a "prompt" look like when most of context is auto-retrieved from past decisions?
- Does the agent drift? Does it become more _itself_ over time, or more _noise_?
- When the owner disagrees with a decision, how is that disagreement recorded, weighted, and used?
- Is there a UI for an agent's "personality" that lets the owner correct it without rewriting a prompt?

This is the "continuous identity" claim from `competitive-landscape.md` (_"Agents aren't sessions... durable, evolving sense of self"_) cashed out as an actual UX. Moonshot C (continuous self-compression) gestures at it. Nobody has answered what it _looks like_ on day 30.

This is both the riskiest and most differentiating UX bet in the whole project. It deserves its own design document eventually — not its own paragraph.

### 18.11 Operational maturity — what happens when something breaks in production

§15 surfaced foot-guns. §16 surfaced the install story. Neither addressed operations _after_ install:

- **Upgrades.** A user running v1 with 20 agents and 3 months of events gets `pcy upgrade` — what does that do? Is there an event-schema migration story? Are projections rebuilt? Can a migration fail partway and leave the system unbootable?
- **Backups.** The event log _is_ the product. What is the backup ritual? Is it documented? Is it verified on a cadence?
- **Monitoring.** `/ready` is solid (§15.1). But a user wants "is my agent working"-level monitoring, not "is the process alive." That is a dashboard, a notifier, or both.
- **Recovery from a bad wake.** An agent enters a wake, spends $40, produces garbage. What's the ritual? Is it `pcy rollback <wake_id>`? Is it `pcy quarantine <agent_id>`? Does the event log support causal rewinds?
- **Incident response when the agent itself is the incident.** If a pincer does something destructive under capability, what is the post-mortem process? The tooling? The reversal path?

"Operational maturity" is the difference between a hobby and a thing people bet their jobs on. The document has not named any of it.

### 18.12 The meta-question — what should this document become?

This file is now ~1000 lines of critical thinking, across 18 sections. It is still labeled as pre-EXPAND, in `docs/input/`. Before the next `/iterate`, a decision has to be made about _what role this document plays going forward_:

- **Option M1** — treat it as a throwaway thinking doc, distill the chosen bets into a new `scaffolding/strategy.md` or into `scope.md` v6 directly, and archive this.
- **Option M2** — treat it as a living strategy document under `docs/input/` that future iterations revisit, quote, and refine.
- **Option M3** — extract the stable conclusions (the bets, the moonshots, the refined sentence, the compliance wedge) into a new ADR or "North Star" document in `docs/reference/`, and keep this file as the audit/thinking log.

Most likely: **M3.** The conclusions belong in a terse, linkable artifact. The reasoning belongs preserved as evidence. Today it is mixed.

### 18.13 The uncomfortable truth: honest count of unresolved questions

After eighteen sections, the document has accumulated this many unresolved, human-required decisions:

1. First sticky user, by name. (§16.6)
2. Self-host-only vs + SaaS. (§7.1, §16.6)
3. License choice for provability surfaces. (§16.6)
4. Governance model. (§16.6)
5. First real mission, written down. (§16.6)
6. Compliance regime to wedge on. (§18.7)
7. Revenue model. (§18.1)
8. Tripwires for thesis-death scenarios. (§18.2)
9. Which one of {provability, protocol, marketplace, replay, MCP} to make the first-year focus. (§18.6)
10. Treatment of this document going forward. (§18.12)

**Ten decisions. None of them are engineering.** All of them are the kind of decision only a human in the chair can make. The role of the harness and of this document has been to surface them clearly, not to resolve them. That work is now done. The next meaningful action is a conversation with a human, not another analysis pass.
