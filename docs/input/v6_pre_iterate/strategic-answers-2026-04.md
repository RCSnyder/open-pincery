# Open Pincery — Strategic Answers with Literature

**Date:** 2026-04-20
**Companion to:** [`docs/input/v6_pre_iterate/first-principles-assessment.md`](./first-principles-assessment.md) §1–§18
**Status:** Opinionated answer document. Each position is defended with literature where literature applies, and named as a business recommendation where no literature applies.

This document answers the ten unresolved decisions surfaced in §18.13 of the first-principles assessment, plus the threat-model, tripwire, and operational questions from §18.2, §18.3, §18.7, and §18.11. Every answer is written as a position, not a survey. The literature backing is cited inline; the full bibliography is in §B.

No answer here binds the project. The purpose is to make each decision _reviewable_ — defended well enough to either adopt or push back against with equal force.

---

## Part I — Foundational claims and their literature

Before answering the ten decisions, this section establishes what each strategic claim in §1–§18 actually rests on. Every subsequent recommendation inherits these foundations.

### F1. Event sourcing as substrate is a mature pattern, not a novelty

The claim in §15.1 and Bet 1 (§15.7) — that an append-only event log with projections is the right substrate — rests on a well-established body of work.

- Fowler's canonical definition of event sourcing [Fowler 2005] establishes the pattern: capture all changes as a sequence of events, derive state by replay. The reliability and auditability properties fall out mechanically.
- Helland's _Immutability Changes Everything_ [Helland 2015] argues at CIDR that the cost of storage has collapsed to the point where the design default should be "never mutate." Open Pincery's event table follows this directly.
- Kleppmann's _Designing Data-Intensive Applications_ [Kleppmann 2017, ch. 11] frames event logs as the backbone of stream-processing systems and argues that the log _is_ the database, with derived views as caches.
- Lamport's classic [Lamport 1978] gives the theoretical foundation for why ordered event logs are the correct primitive for reasoning about distributed state.

**Strategic consequence:** the provability bet (§15.9) is not experimental. It sits on 40+ years of systems research. The risk is not "will this work architecturally" — it is "can we execute it cleanly enough to matter."

### F2. Capability-based security is the right authority model for multi-agent systems

Bet 2 (§15.7) and the threat model (§18.3) both depend on capability security.

- Hardy's _The Confused Deputy_ [Hardy 1988] remains the cleanest argument for why ambient authority (ACLs, role-based access control) fails in compositional systems. Every agent framework that uses an env-file of API keys is a confused-deputy vulnerability waiting to happen.
- Miller's _Robust Composition_ [Miller 2006] — the PhD thesis behind the E language and Google's Caja — formalizes capability security as the correct model for systems where untrusted components compose.
- Klein et al.'s seL4 verification work [Klein 2009] demonstrates that capability security is implementable at microkernel scale with full formal verification.
- Levy's survey [Levy 1984] documents two decades of capability systems and the consistent finding that capability tokens with cryptographic provenance outperform ACLs for composability and auditability.

**Strategic consequence:** the capability-token model proposed in §15.7 Bet 2 is not novel research. It is a 40-year-old pattern newly relevant to agents because agents are the first widely-deployed software where untrusted-composition is the _default_ mode of operation.

### F3. Actor-model + durable execution is the right runtime shape for agents

The CAS lifecycle, the event-driven wake, and the pincer-as-process framing all sit on this foundation.

- Hewitt's original actor model [Hewitt 1973] and Hoare's CSP [Hoare 1978] established the two competing formalizations of concurrent message-passing systems. Erlang/OTP [Armstrong 2003] operationalized Hewitt's model at telecom reliability scale and demonstrated that "let it crash" plus durable message logs produces systems with nine-nines uptime.
- Temporal's architecture [Temporal 2023 docs; Cadence background in Uber Engineering 2020] generalizes durable execution to arbitrary workflows by making every effect a logged, replayable step.
- The saga pattern [Garcia-Molina & Salem 1987] predates all of this and remains the correct model for long-running transactional workflows — which a mission-driven agent absolutely is.

**Strategic consequence:** the §14 thesis ("pincers are the runtime, conversation is the surface") is supported by 50 years of systems research on concurrent message-passing and 30 years of work on long-running transactions. The combination with LLM reasoning is new; the substrate is not.

### F4. The agent-as-process research direction is consolidating around a small set of primitives

The 2022–2026 agent literature has converged on a recognizable shape.

- Wei et al.'s _Chain-of-Thought Prompting_ [Wei 2022] established that explicit intermediate reasoning improves LLM performance — legitimizing the "agent thinks step-by-step" pattern.
- Yao et al.'s _ReAct_ [Yao 2023] established the tool-use + reasoning interleaving that every modern agent framework (including Open Pincery's wake loop) now uses.
- Schick et al.'s _Toolformer_ [Schick 2023] and follow-up work established that tool use is learnable behavior, not just prompted behavior — meaning tool-surface design matters more than prompt design.
- Shinn et al.'s _Reflexion_ [Shinn 2023] introduced the self-critique + memory loop that Moonshot C (§15.7) generalizes.
- Park et al.'s _Generative Agents_ [Park 2023] demonstrated continuous identity over long horizons using memory streams + reflection + planning — the direct intellectual ancestor of the §18.10 "month-30 problem."
- Packer et al.'s _MemGPT_ [Packer 2023] operationalized context-window management as OS-style memory hierarchy.

**Strategic consequence:** the primitives Open Pincery has chosen (events, wakes, tool use, reflection via wake summaries, missions) line up with what the research literature has _already_ identified as the durable kernel of agent design. The bet is not on novelty — it is on _execution of a known-correct shape at production quality_.

### F5. The Model Context Protocol (MCP) is becoming the tool-plumbing standard

- Anthropic's MCP specification [Anthropic 2024] is explicitly designed as an open protocol analogous to LSP (Language Server Protocol), with client/server/transport layers and a growing ecosystem of implementations. The analogue to LSP is deliberate and strategically apt — LSP succeeded because it lowered the (editor × language) cost from N×M to N+M.
- Because MCP is open, transport-agnostic, and deliberately shaped like a standard integration protocol rather than a product feature, it has a plausible path to becoming shared tool-plumbing across agent systems.

**Strategic consequence:** §16.3 recommendation (speak MCP on both sides) is backed by the protocol-as-standard thesis. The risk of betting _against_ MCP (by inventing a closed tool protocol) is substantially higher than the risk of betting _for_ it.

---

## Part II — Answers to the ten unresolved decisions

Each answer below names the position, the literature or business rationale, and what the decision forecloses.

### D1. First sticky user, by name

**Position:** The founder of Open Pincery. Then people exactly like the founder: solo operators running companies of one (CEO / CTO / support / ops are the same person) and small companies with a single technical CTO responsible for most surfaces.

The buyer is not a compliance persona, not a platform engineer at a mid-market company, and not a hobbyist LangChain user. The buyer is an operator who has no one else to delegate to and refuses to let AI output degrade the quality of what leaves their company.

**Rationale:**

- Dogfooding as forcing function. The founder is the harshest available reviewer, and the acceptance bar for running one's own company is much higher than any external ICP-validation exercise will produce early on. Pieter Levels, early Basecamp, and early Linear all built this way.
- Demand clarity. Solo operators and one-CTO shops have concrete, recurring, painful work and no team to absorb the pain. They are the audience for which "the agent actually shipped it correctly" beats "the agent produced something impressive-looking."
- Distribution follows authenticity. A founder visibly running their own company on the substrate is a stronger credibility story than any pitch built from abstract personas.
- Christensen's _Innovator's Dilemma_ [Christensen 1997] still applies: frontier vendors cannot credibly ship a professional-grade, capability-scoped, replayable substrate for one-person operators on any short timeline; their business model is aimed elsewhere.

**What this forecloses:** the compliance-first mid-market wedge as the _first_ buyer (it remains a valid year-two path for the small-CTO segment). The hobbyist-developer wedge. The Fortune-50 platform-team wedge. All three are dropped as first-buyer candidates for year one.

**First concrete step:** treat the founder's own operations as the discovery surface. Instrument the recurring manual work that the founder performs each week and use it as the demand signal for which mission types enter the Tier 1 catalog (see D5).

### D2. Self-host-only versus self-host + optional SaaS

**Position:** **Self-host-first, SaaS-optional as a year-two product.** The substrate and reference implementation stay fully self-hostable under Apache 2.0 in year 1 (see D3). A hosted offering may appear later as a _convenience_ for buyers who want the substrate but not the operational burden — priced as infrastructure, not per-seat.

**Literature basis:**

- Spolsky's _Commoditize Your Complement_ [Spolsky 2002] is the canonical strategic argument: a product commoditizes the layer it sits on top of, and captures value from the layer it sits _below_. Open Pincery's natural position is _below_ hosted model APIs (commoditizing model choice) and _above_ the OS (leveraging the host substrate). Self-host is the natural posture because it maximizes the model-commoditization surface.
- Lerner & Tirole's _Some Simple Economics of Open Source_ [Lerner & Tirole 2002] shows that open-source infrastructure projects succeed when the value is in _deployed operation_, not in the source itself — which is the GitLab/HashiCorp/Elastic pattern that has consistently outperformed closed-source alternatives in infrastructure categories.
- The Open Core pattern documented in [Riehle 2012] provides a well-trodden path to commercial sustainability without abandoning self-host as the primary deployment mode.

**What this forecloses:** the pure-SaaS path. This is a correct foreclosure — a pure-SaaS AI-agent substrate against OpenAI's and Anthropic's offerings is a losing trade. It also forecloses the "closed-source with a free tier" path, which is strictly dominated by open-source-with-hosted-option for infrastructure categories.

### D3. License choice

**Position:** **Apache 2.0 for the substrate and reference implementation in year 1.** If commercialization happens in year 2 or later, charge for hosted operation, packaged compliance artifacts, and support before introducing any source-available licensing split.

**Literature basis:**

- Apache 2.0 is the default for modern infrastructure projects and provides explicit patent grants [Apache Foundation 2004]. It is the license under which virtually every successful recent infrastructure project (Kubernetes, Kafka, Tonic, Tokio) ships.
- Lerner & Tirole's economics of open source [Lerner & Tirole 2002] and Riehle's work on commercial open source [Riehle 2012] both support the more general point: infrastructure projects usually create value first through adoption and operation, then capture value later through hosting, support, and packaged enterprise conveniences.
- MIT is tempting but provides no patent grant; Apache 2.0 is the stronger permissive default for infrastructure.
- A source-available split such as BSL remains a later option, but it should follow real customer validation rather than precede it. In this document, that is a business sequencing judgment, not a literature-backed claim.

**What this forecloses:** a premature license split before the project has reference customers. It also forecloses AGPL for the core, because the adoption penalty is too high for the intended buyer. The commercial path, if it exists, should begin with hosting, support, and packaged evidence workflows rather than with relicensing the substrate.

### D4. Governance model

**Position:** **Single maintainer (the project owner) with a published decision-record (ADR) practice, moving to a 3-person steering group once there are two full-time contributors.**

**Literature basis:**

- Raymond's _The Cathedral and the Bazaar_ [Raymond 1999] and follow-up work [Raymond 2000] establish that the BDFL pattern is the correct shape for early-stage projects — consensus-by-committee at this scale kills throughput. Linux, Python (pre-2018), Vue, and Rails all used BDFL governance through their formative years.
- Brooks's _Mythical Man-Month_ [Brooks 1975, ch. 7 on surgical team] remains the best articulation of why small-team surgical-style ownership outperforms committee design for systems work.
- The ADR pattern [Nygard 2011] provides the bridge between BDFL-speed and auditability — decisions are fast but the reasoning is permanent and reviewable.

**What this forecloses:** the "open foundation from day one" path (premature, dilutes velocity) and the "closed decision-making" path (incompatible with §18.6's contributor-pool bet).

### D5. First real mission, written down

**Position:** The first real mission is not a single task. It is the **Tier 1 bootstrap catalog**: the minimum set of mission types required for the founder to operate a one-person company on Open Pincery. Built in order, each held to an acceptance contract, each added to the catalog only once it is trustworthy enough to live in daily use.

**Tier 1 catalog (in build order):**

1. **Codebase steward.** PR review, dependency hygiene, release notes, changelog maintenance, security-sensitive-file flagging. Closest to the current GitHub-integrated substrate and the shortest path to a credible reference.
2. **Inbox triage.** Surface items needing a human response, draft replies for the rest, escalate anything with a deadline or a dollar sign.
3. **Commitments tracker.** What has been promised to whom by when. Surface upcoming obligations, overdue items, and founder decisions needed.
4. **Weekly digest.** Monday-morning artifact: what the agents did last week, what needs decisions this week, what is overdue, where escalations are waiting. This is the human-in-the-loop surface that ties the whole catalog together.

**Why this shape:**

- Each mission type has a narrow acceptance contract. Execution inside the mission is unbounded in depth, but "done" is defined.
- Each mission addresses work whose absence would actually stop a solo company from functioning. None of them are speculative.
- The catalog is sequenced so the earliest missions are closest to the existing substrate (GitHub, event log, missions) and the later missions add integrations (email, calendar) only after the earlier ones are proven.
- The catalog is _observable from the outside_. Anyone watching can see what shipped, what was caught, what was escalated, and what the agents refused to do.

**Literature basis:** Park et al.'s _Generative Agents_ [Park 2023] demonstrated that specific, bounded, recurring tasks with clear success criteria are the correct shape for long-running agents. The "virtual town" experiment succeeded because each agent had a defined role and measurable outputs. The Tier 1 catalog applies the same principle to the founder's own operations.

**What this forecloses:** any first mission that cannot be contracted ("be helpful," "do anything the chat asks for"). The "coding-agent-like-Cursor" first mission (not a mission, a tool). The speculative-enterprise-first mission (no founder-facing pain to validate it). Tier 2 missions (pipeline follow-up, content, competitor watch, financial summary) are deferred until Tier 1 is solid in daily use. Tier 3 missions (contract review, customer-support auto-response, research synthesis) are year-two or later.

### D6. First wedge

**Position:** **Sovereign agentic workforce for one-person operators who also ship code.** Professional-grade execution is the quality commitment; sovereignty is the category. The wedge is trust, leverage, and operator-controlled infrastructure, not compliance. Governance and audit-style evidence packaging (NIST AI RMF, SOC 2-compatible evidence, EU AI Act alignment) are deferred to year-two repackaging for the small-CTO segment.

The one-sentence wedge is: _the only AI substrate I would actually let run my company overnight, on infrastructure I control, against models I choose._

**Sovereignty is defined along three axes** and enforced by the substrate, not by convention:

1. _Provider_ — who serves inference (self-hosted, hyperscaler-under-your-account, vendor-hosted).
2. _Model_ — what weights (open-weight such as Llama, Mistral, Qwen, DeepSeek; or proprietary such as GPT, Claude, Gemini).
3. _Data-governance class_ — sovereign (operator-controlled infrastructure, open-weight models, no external data flow), enterprise-bounded (hyperscaler under enterprise terms that forbid training and retention, e.g. Bedrock, Azure AI, Vertex), or vendor-consumer (e.g. ChatGPT, Claude.ai, Gemini apps).

Each mission type's acceptance contract declares a minimum governance class. The end-state target is the sovereign class. The acceptable compromise for frontier quality is enterprise-bounded. Vendor-consumer is allowed as an operator-chosen runtime option but never an architectural assumption.

**Rationale:**

- The first buyer is the founder and operators like the founder (D1). They have no compliance owner, no internal auditor, and no procurement committee. Selling them compliance evidence is selling to the wrong organ.
- The durable differentiator for this buyer is a sovereign agentic workforce that does not silently degrade in quality and does not quietly route their company's data through a single vendor's servers. That falls out of the substrate bets (acceptance contracts, capabilities, replay, reasoner abstraction) applied to a bounded mission catalog, not out of compliance packaging.
- Sovereignty is not a compliance artifact. It is an architectural property. Compliance artifacts follow naturally once the substrate is sovereign, but the sovereignty comes first.
- The substrate's governance properties (typed events, chained provenance, scoped capabilities, replay, governance-class enforcement) remain load-bearing, but in year one they are load-bearing _for the operator's personal trust and control_, not for an external auditor.
- Governance-framed repackaging is preserved for year two because the underlying mechanics are the same. Once the substrate is proven on the founder's own operations, selling the same mechanics as NIST AI RMF / SOC 2-compatible evidence to small-CTO shops becomes a distribution question, not a re-architecture question.

**Why not just Claude Projects / ChatGPT Tasks / Gemini:** those products are vendor-consumer class endpoints by architectural construction. They can approximate the Tier 1 _missions_; they cannot approximate the _sovereignty_. Open Pincery is not competing on reasoning quality. It is competing on whose infrastructure runs the agentic workforce, whose terms govern it, and what happens when those terms change.

**What this forecloses:** the compliance-first go-to-market in year one. The HIPAA-first wedge, financial-services-first wedge, and EU-AI-Act-first wedge are all deferred. The "we are enterprise governance for AI agents" positioning is explicitly not the first story. Any architecture that treats a specific vendor's hosted endpoint as a load-bearing assumption is also foreclosed.

**The right first wedge is a sovereign agentic workforce** because it matches the first buyer (the founder, and operators like the founder), the first mission catalog (Tier 1), the reasoner abstraction required by Durable Bet 9 of the north-star, and the 90-day founder-operated benchmark. Compliance repackaging is a year-two move on top of the same substrate.

### D7. Revenue model

**Position:** **Year 1: zero. Year 2: hosted offering plus packaged compliance evidence for the small-CTO segment. Year 3: support contracts plus a premium pincer marketplace.**

**Year-1 detail:** Apache 2.0 substrate and reference implementation. No monetization attempt. The year-one goal is the 90-day founder-operated benchmark (see north-star), a Tier 1 mission catalog in daily use, and a visible track record of what the substrate does and refuses to do. External adoption is a welcome side effect, not a target.

**Year-2 detail:** two paid products, both aimed at operators who have watched the founder run the system and want the same thing.

1. **Open Pincery Cloud** (hosted): managed substrate for solo operators and one-CTO shops who will not self-host. Priced as infrastructure markup, targeting $50–$300/month/deployment, with a free self-host path always available.
2. **Evidence bundles**: packaged signed audit-export tooling and NIST AI RMF / SOC 2-compatible evidence templates for the small-CTO segment that starts asking for them. Priced per deployment, roughly $500–$2000/month. This is the year-two repackaging of governance mechanics that already exist in the substrate.

**Year-3 detail:** enterprise support contracts ($25k–$100k/year) and a premium pincer marketplace (30% revenue share on paid pincers, analogous to the Steam Workshop or GitHub Marketplace models).

**Literature basis:**

- The Open Core pattern [Riehle 2012] shows that the viable revenue split is permissive-core + restrictive-periphery, with the periphery focused on operational and compliance concerns rather than core features. This matches exactly.
- HashiCorp's public-company revenue breakdown [HashiCorp S-1, 2021] demonstrated that ~85% of revenue came from hosted offerings and enterprise support, with <5% from the open-source project itself. This is the realistic shape.
- Marketplace economics literature supports the broader point that trusted two-sided platforms capture value through discovery, trust, and transaction coordination [Parker, Van Alstyne, & Choudary 2016]. A 20–30% take rate is a business benchmark, not a result derived from the cited literature.

**What this forecloses:** the "fundraise and hire ten engineers" path (premature without D1 validation). The "consultancy-first" path (traps the project in custom work, prevents product-market fit discovery).

### D8. Tripwires for thesis-death scenarios

Named concretely. Each tripwire includes a _measurable signal_ and a _predetermined response_.

| Scenario (from §18.2)                                         | Tripwire                                                                                                                                                                                  | Response                                                                                                                                                                                                                                                                    |
| ------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OpenAI/Anthropic bundle durable agents + capabilities + audit | Either vendor announces "projects with compliance-grade audit export" at a public event                                                                                                   | Pivot to _hybrid_ — sell Open Pincery as the _on-prem-auditor_ for hosted agents. Build the "import OpenAI project logs, export our signed bundle" feature in 90 days.                                                                                                      |
| MCP expands to agent-runtime protocols                        | The MCP working group publishes a draft agent-runtime extension                                                                                                                           | _Adopt it_. Abandon the pincer protocol, refactor to speak the MCP extension. §17 design accepts this explicitly — protocols, not ownership.                                                                                                                                |
| Competitor owns the compliance story                          | Any other substrate ships a compelling AI-governance / evidence-export bundle for engineering and platform teams before Open Pincery does                                                 | Accelerate the D6 wedge. If they are visibly further along, consider interoperability rather than competition — export a bundle their format can consume.                                                                                                                   |
| "Conversation is the surface" is wrong                        | Year-1 sales data shows buyers consistently ask for workflow-authoring UI before buying                                                                                                   | Rebuild thesis. This is a three-month pause-and-reconsider, not a shrug.                                                                                                                                                                                                    |
| Rust is the wrong ecosystem bet                               | Contributor signups to Rust-code PRs drop below 1/quarter for two consecutive quarters                                                                                                    | Lean harder on P3 (protocol, not implementation). Commission a Python reference harness to expand the effective contributor pool.                                                                                                                                           |
| Self-hosting AI plateaus                                      | Frontier model vendors explicitly forbid on-prem inference, or price local models above competitive hosted offerings for >12 months                                                       | Reposition to "hosted-agents auditor" as the core product. Self-host becomes a secondary story.                                                                                                                                                                             |
| Mission catalog discipline breaks                             | The founder handles the same manual task three or more times without either promoting it to a cataloged mission type with an acceptance contract or recording an explicit decision not to | Stop new substrate work. Either formalize the mission type (contract, capabilities, required evidence) and add it to the catalog, or write an ADR explaining why it stays manual. Ambiguous drift between manual and agent work is the primary way this plan produces slop. |

**Literature basis:** tripwire methodology is informed by Boyd's OODA framing as treated in Coram 2002 and by the pre-mortem technique [Klein 2007]. The point of a tripwire is not to predict the future; it is to pre-decide the response so that cognitive load in the moment does not produce indecision.

### D9. First-year focus — one of {provability, protocol, marketplace, replay, MCP}

**Position:** **Substrate plus Tier 1 mission catalog, dogfooded by the founder, with MCP adoption as a parallel low-cost track.**

The year-one focus is not an abstract capability ("provability") but a concrete operating configuration: the substrate is built, the Tier 1 catalog is built on top, the founder runs the company on it, and the 90-day founder-operated benchmark (see north-star) is the forcing function.

**Reasoning:**

- Substrate and Tier 1 are coupled. Each Tier 1 mission type exists only because the substrate (events, capabilities, missions, acceptance contracts, replay) is strong enough to support it. Building the substrate without Tier 1 produces shelfware; building Tier 1 without the substrate produces slop.
- MCP is cheap and leveraged (§16.3 estimates 2–3 weeks) and does not compete with Tier 1 for attention. It enables external tool use for the mission types regardless of what else ships.
- Marketplace is year-three work per D7.
- Pincer protocol extraction (P3 from §17) is year-two work unless it unblocks a Tier 1 mission. It ships as documentation + reference-harness-extraction later, not as a year-one headline feature.
- Replay is load-bearing for every Tier 1 mission (it is how the founder audits overnight work), so it ships in year one by necessity — but as a property of the substrate, not a separate product track.

**Literature basis:**

- Moore's _Crossing the Chasm_ [Moore 1991] argues for a _single beachhead_ in year one. The Tier 1 catalog plus the founder-operated benchmark is that beachhead: one buyer persona, one operating configuration, one demonstrable artifact.
- Park et al.'s _Generative Agents_ [Park 2023] supports the corresponding mission-design principle: a small set of specific, bounded, recurring tasks with clear success criteria is the correct shape for long-running agents.

**What this forecloses:** marketplace in year one (correctly, per D7). Workflow-authoring UI (per §14 thesis). Protocol-first branding (correctly — protocol is infrastructure, not product). Broad external discovery-call programs in year one (replaced by the founder's own operations as the discovery surface).

### D10. Treatment of the first-principles assessment going forward

**Position:** **Option M3 from §18.12.** The stable conclusions (the bets, the moonshots, the one-sentence thesis, the compliance wedge, the answers in this document) live in `docs/reference/` as versioned north-star artifacts. The ~1,140-line first-principles assessment itself is preserved in `docs/input/` as the _thinking record_ — evidence of how the conclusions were reached, reviewable but not authoritative.

> **Note (2026-04-20):** As part of opening the v6 iteration cycle, this document and its companions have been relocated into `docs/input/v6_pre_iterate/` to serve as EXPAND input. The D10 principle still holds: stable conclusions remain canonical, the thinking record remains provenance. The relocation is administrative.

**Concrete structure:**

- `docs/input/north-star-2026-04.md` — canonical direction doc (promoted to top level of `docs/input/` on 2026-04-20).
- `docs/input/v6_pre_iterate/strategic-answers-2026-04.md` — this document.
- `docs/input/v6_pre_iterate/tripwires-2026-04.md` — extracted from D8, with owner and check cadence.
- `docs/input/v6_pre_iterate/first-principles-assessment.md` — unchanged, preserved as provenance.

**Literature basis:** the ADR pattern [Nygard 2011] and modern RFC-driven design processes both rely on the same principle: conclusions get a short, citable home; reasoning gets preserved separately as evidence.

---

## Part III — The 12-month plan that falls out of these answers

No time estimates, per project conventions. Ordering and gating only.

**Phase 1 — Pre-conditions (gates everything else):**

- Harden the shell tool (timeout, cwd, allowlist). §15.10 item 1.
- Add `reqwest` timeout to `LlmClient`. §15.10 item 2.
- Strongly typed `event_type` with versioned payloads. §15.10 item 3.
- Typed error taxonomy. §16.4 item 13.

**Phase 2 — Substrate spine (D9 focus):**

- Event chain hashing. §15.10 item 5.
- Capability tokens with cryptographic provenance. §15.7 Bet 2.
- Mission primitive as first-class table + runtime. §15.10 item 4.
- **Acceptance-contract primitive** attached to mission types, including a declared minimum data-governance class. (New: required for Tier 1 missions.)
- **Reasoner abstraction** with three axes (provider / model / governance class), with at least one frontier-hosted implementation and one self-hosted open-weight implementation proven end-to-end against a Tier 1 mission. (New: required by Durable Bet 9 of the north-star.)
- Signed audit bundle export (still built; year-one use is founder self-trust, year-two use is external packaging). §15.7 Bet 1.
- `pcy replay` and `pcy diff`. §15.10 item 8.

**Phase 3 — Tier 1 mission catalog (D5):**

- Codebase steward mission type.
- Inbox triage mission type.
- Commitments tracker mission type.
- Weekly digest mission type.
- MCP server + client, shipped as a parallel low-cost track. §16.4 item 6.

**Phase 4 — Founder-operated validation:**

- Run Open Pincery as the operating substrate of the founder's one-person company.
- 90-day continuous founder-operated benchmark (see north-star).
- Every repeated manual task the founder performs three times is evaluated against the catalog discipline tripwire.

**Phase 5 — Year-two readiness:**

- Hosted-offering infrastructure (Open Pincery Cloud preview) aimed at solo operators and one-CTO shops.
- Evidence-bundle repackaging for the small-CTO segment (NIST AI RMF / SOC 2-compatible) once the year-one run produces real demand.
- Pincer Protocol v1 spec + reference harness extraction, and OpenClaw channel adapter — promoted from year one to year two unless they unblock a Tier 1 mission.

---

## Part IV — What this document is not

- It is not a business plan. It is a position document that a business plan would cite.
- It is not a commitment. Every answer is reviewable and reversible.
- It is not comprehensive. The threat model (§18.3), month-30 UX problem (§18.10), and operational maturity (§18.11) each deserve their own companion documents. Those are follow-on work.
- It is not the end of the thinking. It is a _commitment to what has been decided_ so that the next decisions have a stable foundation.

---

## Part V — Appendix: The ten decisions at a glance

| #   | Decision           | Position                                                                                     |
| --- | ------------------ | -------------------------------------------------------------------------------------------- |
| D1  | First sticky user  | The founder, then operators like the founder (solo CEO/CTO, one-CTO small companies)         |
| D2  | Self-host vs SaaS  | Self-host-first, hosted-optional year 2                                                      |
| D3  | License            | Apache 2.0 for year-1 substrate and reference implementation                                 |
| D4  | Governance         | Single maintainer + published ADRs, 3-person group once 2 FTE                                |
| D5  | First mission      | Tier 1 bootstrap catalog: codebase steward, inbox triage, commitments tracker, weekly digest |
| D6  | First wedge        | Sovereign agentic workforce for one-person operators who also ship code                      |
| D7  | Revenue            | Y1 zero, Y2 hosted + packaged evidence for small-CTO segment, Y3 support + marketplace       |
| D8  | Tripwires          | Seven scenarios named, each with signal and response (see `tripwires-2026-04.md`)            |
| D9  | Year-1 focus       | Substrate + Tier 1 catalog, dogfooded by the founder, with MCP as a parallel low-cost track  |
| D10 | Document treatment | M3 — stable conclusions in `docs/reference/`, thinking preserved in `docs/input/`            |

---

## §B — Bibliography

Classical systems research, security literature, and recent agent work cited throughout this document. Entries are grouped by topic for reading order.

### Event sourcing, logs, and distributed systems

- **Fowler, M.** (2005). _Event Sourcing_. https://martinfowler.com/eaaDev/EventSourcing.html
- **Helland, P.** (2015). _Immutability Changes Everything_. Proceedings of CIDR 2015.
- **Kleppmann, M.** (2017). _Designing Data-Intensive Applications_. O'Reilly Media. Chapters 11 ("Stream Processing") and 12 ("The Future of Data Systems") are directly relevant.
- **Lamport, L.** (1978). _Time, Clocks, and the Ordering of Events in a Distributed System_. Communications of the ACM, 21(7), 558–565.
- **Lamport, L.** (1998). _The Part-Time Parliament_. ACM Transactions on Computer Systems, 16(2), 133–169. (Paxos.)
- **Ongaro, D. & Ousterhout, J.** (2014). _In Search of an Understandable Consensus Algorithm_. USENIX ATC 2014. (Raft.)
- **Brewer, E.** (2000). _Towards Robust Distributed Systems_. PODC 2000 keynote. (CAP theorem.)

### Capability security

- **Hardy, N.** (1988). _The Confused Deputy (or why capabilities might have been invented)_. ACM SIGOPS Operating Systems Review, 22(4), 36–38.
- **Levy, H. M.** (1984). _Capability-Based Computer Systems_. Digital Press.
- **Miller, M. S.** (2006). _Robust Composition: Towards a Unified Approach to Access Control and Concurrency Control_. PhD thesis, Johns Hopkins University.
- **Klein, G., Elphinstone, K., Heiser, G., et al.** (2009). _seL4: Formal Verification of an OS Kernel_. SOSP 2009.

### Actor model, CSP, durable execution

- **Hewitt, C., Bishop, P., & Steiger, R.** (1973). _A Universal Modular Actor Formalism for Artificial Intelligence_. IJCAI 1973.
- **Hoare, C. A. R.** (1978). _Communicating Sequential Processes_. Communications of the ACM, 21(8), 666–677.
- **Armstrong, J.** (2003). _Making Reliable Distributed Systems in the Presence of Software Errors_. PhD thesis, KTH. (Erlang/OTP foundations.)
- **Garcia-Molina, H. & Salem, K.** (1987). _Sagas_. ACM SIGMOD Record, 16(3), 249–259.
- **Gray, J. & Reuter, A.** (1993). _Transaction Processing: Concepts and Techniques_. Morgan Kaufmann.
- **Temporal Technologies.** (2023). _Temporal Architecture Documentation_. https://docs.temporal.io/ (Referenced as industry system documentation, not peer-reviewed.)

### Agent research and LLM systems

- **Wei, J., Wang, X., Schuurmans, D., et al.** (2022). _Chain-of-Thought Prompting Elicits Reasoning in Large Language Models_. NeurIPS 2022.
- **Yao, S., Zhao, J., Yu, D., et al.** (2023). _ReAct: Synergizing Reasoning and Acting in Language Models_. ICLR 2023.
- **Schick, T., Dwivedi-Yu, J., Dessì, R., et al.** (2023). _Toolformer: Language Models Can Teach Themselves to Use Tools_. NeurIPS 2023.
- **Shinn, N., Cassano, F., Gopinath, A., et al.** (2023). _Reflexion: Language Agents with Verbal Reinforcement Learning_. NeurIPS 2023.
- **Park, J. S., O'Brien, J. C., Cai, C. J., et al.** (2023). _Generative Agents: Interactive Simulacra of Human Behavior_. UIST 2023.
- **Packer, C., Fang, V., Patil, S. G., et al.** (2023). _MemGPT: Towards LLMs as Operating Systems_. arXiv:2310.08560.
- **Anthropic.** (2024). _Model Context Protocol Specification_. https://modelcontextprotocol.io/

### Compliance and standards

- **European Parliament & Council.** (2024). _Regulation (EU) 2024/1689 on harmonised rules on artificial intelligence_ (EU AI Act). Official Journal of the European Union, 12 July 2024. Articles 9, 12, 13, 14 directly applicable.
- **NIST.** (2023). _AI Risk Management Framework (AI 100-1)_. National Institute of Standards and Technology.
- **AICPA.** (2022). _Trust Services Criteria for Security, Availability, Processing Integrity, Confidentiality, and Privacy_. American Institute of CPAs.
- **NIST.** (2020). _SP 800-53 Rev. 5 — Security and Privacy Controls for Information Systems and Organizations_. (AU-2 audit events.)

### Strategy, open source, and software engineering

- **Brooks, F. P.** (1975, anniversary ed. 1995). _The Mythical Man-Month_. Addison-Wesley.
- **Moore, G. A.** (1991). _Crossing the Chasm_. HarperBusiness.
- **Christensen, C. M.** (1997). _The Innovator's Dilemma_. Harvard Business Review Press.
- **Raymond, E. S.** (1999). _The Cathedral and the Bazaar_. O'Reilly Media.
- **Lerner, J. & Tirole, J.** (2002). _Some Simple Economics of Open Source_. The Journal of Industrial Economics, 50(2), 197–234.
- **Spolsky, J.** (2002). _Strategy Letter V: The Economics of Open Source_ ("Commoditize Your Complement"). https://www.joelonsoftware.com/2002/06/12/strategy-letter-v/
- **Riehle, D.** (2012). _The Single-Vendor Commercial Open Source Business Model_. Information Systems and e-Business Management, 10(1), 5–17.
- **Parker, G., Van Alstyne, M., & Choudary, S. P.** (2016). _Platform Revolution_. W. W. Norton.
- **Nygard, M.** (2011). _Documenting Architecture Decisions_. https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions
- **Klein, G.** (2007). _Performing a Project Premortem_. Harvard Business Review, September 2007.
- **Coram, R.** (2002). _Boyd: The Fighter Pilot Who Changed the Art of War_. Little, Brown. (Secondary treatment of Boyd and OODA.)

### License texts

- **Apache Software Foundation.** (2004). _Apache License 2.0_. https://www.apache.org/licenses/LICENSE-2.0

---

**Document version:** 2026-04-20 first draft.
**Review cadence:** quarterly, or at any tripwire event per D8.
**Owner:** project maintainer (D4).
