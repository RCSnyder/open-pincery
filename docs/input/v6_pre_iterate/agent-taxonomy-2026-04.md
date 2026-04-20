# Agent Taxonomy & Category Claim — 2026-04-20

**Status:** Thinking record for v6 EXPAND input. Not yet reconciled into `north-star-2026-04.md`, `strategic-answers-2026-04.md`, or `tripwires-2026-04.md` — but should be in the v6 cycle.

**Source prompt:** Nate's Newsletter, ["There Are 4 Kinds of Agents (And You're Probably Using the Wrong One)"](https://natesnewsletter.substack.com/p/there-are-4-kinds-of-agents-and-youre). The article argues the word "agent" covers four architectures with little in common: **coding harnesses**, **dark factories**, **auto research loops**, and **orchestration frameworks** — and that picking the wrong one is the expensive mistake.

This note takes that taxonomy seriously, checks how Open Pincery slots into it, and concludes that four is under-counting. OP is a fifth category, and we should claim it publicly rather than let the market mis-slot us.

---

## The four categories (from the source)

| #   | Category                | Canonical examples                        | Governing principle                    |
| --- | ----------------------- | ----------------------------------------- | -------------------------------------- |
| 1   | Coding harness          | Cursor, Claude Code, Copilot              | Decomposition + human-in-loop-per-step |
| 2   | Dark factory            | SWE-agent, Devin, background task runners | Specification-as-code                  |
| 3   | Auto research loop      | AlphaEvolve, autoresearch                 | Metric-plus-guardrail (optimization)   |
| 4   | Orchestration framework | CrewAI, LangGraph, AutoGen                | Handoff contracts between role-agents  |

The article's one-question diagnostic is roughly: _what shape of problem are you pointing this at?_ — because the architectures are not substitutable.

## Where Open Pincery fits in the four

| Category                | Fit?                | Notes                                                                                                                                              |
| ----------------------- | ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Coding harness          | No                  | No IDE tether. No per-keystroke human review.                                                                                                      |
| Dark factory            | Closest of the four | But one-shot task runners lack OP's persistent state, event log, wake/sleep, and mission continuity.                                               |
| Auto research           | No                  | There's no fitness function being optimized. Missions have acceptance contracts, not objective functions.                                          |
| Orchestration framework | Partial             | OP coordinates multiple agents, but missions are first-class; "roles" are not. The primitive is _what needs to happen_, not _who plays what part_. |

If buyers classify OP using only these four, the default slot is **dark factory**, which invites a direct comparison to Devin. That under-sells the continuity bet and picks the wrong benchmark.

## Categories the four-agent taxonomy misses

1. **Persistent / resident / continuous agents.** Event-driven, durable, stateful, mission-bounded, wake/sleep. Peers: Zapier Agents, Lindy, n8n agentic flows, the direction Cursor Background Agents + Devin are drifting toward. **This is OP's actual category.** Governing principle: **event log + acceptance contract**. Failure mode: drift, not divergence.
2. **Embedded / in-product agents.** Shopify Sidekick, Intercom Fin, Claude Projects, GitHub Copilot Workspace. The user doesn't wield them (harness) and doesn't fire-and-forget (dark factory) — they're a persistent feature of a host product, tied to that host's data model.
3. **Retrieval chatbots.** The boring 80% of "enterprise AI agents" in 2026. Many arguably aren't agents; many buyers don't know the difference. Ignoring them lets the taxonomy pretend the market is more sophisticated than it is.
4. **Simulation / world agents.** Voyager, Stanford generative agents, game NPCs. Research and entertainment. Different objective function entirely.

## OP's category claim: Continuous Agents (Category 5)

**Public claim:** "The four-agent taxonomy is incomplete. Persistent mission-bounded agents are a distinct category — Category 5, Continuous Agents — with a different governing principle (event log + acceptance contract) and a different failure mode (drift, not divergence). Open Pincery is a sovereign substrate for Category 5 agents."

### Why claim it

- **Matches the architecture honestly.** Wake/sleep, event sourcing, CAS lifecycle, missions as first-class — this is not dark-factory behavior, and pretending it is would require lying about the code. The README already cites the Continuous Agent Architecture; the category claim makes that public-facing.
- **Resolves "Why not Claude Projects" in one sentence.** Projects is Category 2b (embedded agent inside a host product). OP is Category 5 (sovereign substrate for resident agents you own).
- **Gives sovereignty its natural home.** Only a persistent-agent substrate has a meaningful data-governance axis. A one-shot dark factory doesn't care where your data lives between runs because there is no _between_. Bet 9 (sovereignty) requires Category 5 to be coherent.
- **Passes a diagnostic the other four fail.** _"Is the agent still running while you're asleep?"_ Harness: no. Dark factory: finishes. Auto research: finishes when objective hits. Orchestration: during a task. **Continuous: yes, by design.**

### The alternative we're rejecting

"Position as dark factory + persistence." Lower cognitive load for buyers, easier sale, but frames OP as a feature of Category 2 rather than a category-defining substrate. Weaker moat narrative. Doesn't accommodate the Tier 1 catalog (codebase steward, inbox triage, commitments tracker, weekly digest are all resident, not fire-and-forget). Rejected.

## Implications for v6 EXPAND

Carry these into v6 scope work, don't edit the current reference docs in place:

1. **north-star**: add a **Taxonomy & Category Claim** section near the top. Name the five categories, claim #5, reconcile with the existing Continuous Agent Architecture reference in the README.
2. **strategic-answers**:
   - **D6 (first wedge)**: prepend a one-line category declaration — _"Open Pincery competes in Category 5 (Continuous Agents), not Category 2 (dark factories)."_
   - **D9 (competitive positioning)**: generalize the "Why Not Claude Projects" section into a **"Why Not the Other Four Categories"** matrix. One row per category, one-line disqualifier.
3. **tripwires**: add a row — _"Market slots us as dark factory in > 30% of outbound conversations."_ Signal the taxonomy misfit before it becomes permanent positioning.
4. **DELIVERY.md** (next release): one-paragraph framing — _"Open Pincery is a sovereign substrate for Category 5 agents."_
5. **README.md**: consider moving the Continuous Agent Architecture reference higher, adjacent to the category claim, so the public story matches the internal story.

## Open questions

- Does "Continuous Agent" win over "Resident Agent" or "Persistent Agent" as the category label? Current preference: **Continuous** because it matches the existing architecture name and has the clearest diagnostic question ("is it still running while you're asleep?"). Revisit if Zapier/Lindy/n8n converge on different terminology.
- Is Category 2b (embedded agents) worth enumerating separately, or should it be folded into the taxonomy as a sub-type of Category 5 with the host-product constraint? Current preference: **separate**, because the data-governance story is fundamentally different — embedded agents inherit the host's data model; continuous agents own theirs.
- Do we want to publish the five-category taxonomy as a standalone post before v6 ships? It would establish the vocabulary before we need it for positioning. Defer the decision to v6 EXPAND.

## Citation

Nate's Newsletter, ["There Are 4 Kinds of Agents (And You're Probably Using the Wrong One)"](https://natesnewsletter.substack.com/p/there-are-4-kinds-of-agents-and-youre). Accessed 2026-04-20. Used as the four-category baseline this note extends.
