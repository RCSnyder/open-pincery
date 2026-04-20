# Open Pincery Tripwires

Date: 2026-04-20

Purpose: make the strategic failure conditions operational. Each tripwire defines a signal, a check cadence, an owner, and the required response.

Status: this document is an operating companion to `docs/input/v6_pre_iterate/strategic-answers-2026-04.md`, not a speculative note. If a tripwire fires, the response is required unless explicitly overruled in a recorded decision.

## Operating Rule

A tripwire exists to reduce hesitation when the environment changes.

If a tripwire fires:

1. Record the evidence.
2. Open or update an ADR within 7 days.
3. Re-evaluate the current roadmap against the triggered scenario.
4. Either confirm the current direction or change it explicitly.

Do not leave a triggered tripwire in an ambiguous state.

## Owner

Current owner: project maintainer.

If governance changes, update this file and assign a concrete owner to each row.

## Review Cadence

Run a light review monthly and a full review quarterly.

Also review immediately after:

1. Major OpenAI, Anthropic, or Google agent-platform announcements.
2. MCP specification changes relevant to runtime protocols.
3. A material pricing change in frontier or local-model economics.
4. Any failed discovery call sequence that challenges the first-buyer thesis.
5. Any material change in hyperscaler enterprise-inference terms (Bedrock, Azure AI, Vertex) that affects the enterprise-bounded governance class.
6. Any advance in open-weight models that materially changes the viability of the sovereign governance class for Tier 1 missions.

## Tripwire Table

| Scenario                                                                          | Signal                                                                                                                                                                                                                               | Evidence Source                                                                                                      | Check Cadence                                                            | Owner              | Required Response                                                                                                                                                                                                                                                                                                |
| --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ | ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OpenAI or Anthropic bundles durable agents, scoped capabilities, and audit export | A major vendor publicly ships a product that combines hosted agent runtime, scoped action control, and compliance-grade evidence export                                                                                              | Vendor launch notes, product docs, conference keynotes                                                               | Monthly and after major launch events                                    | Project maintainer | Pivot to a hybrid posture: position Open Pincery as the on-prem auditor and evidence layer for hosted agents. Open an ADR and re-scope the next iteration around log ingestion and evidence export for third-party runtimes.                                                                                     |
| MCP expands into agent-runtime protocols                                          | The MCP working group publishes a credible runtime-orchestration or agent-lifecycle extension draft                                                                                                                                  | MCP spec repo, working-group notes, vendor implementations                                                           | Monthly and after MCP releases                                           | Project maintainer | Adopt the MCP direction unless a clear technical blocker exists. Freeze custom pincer-protocol branding and reframe the internal protocol work as compliance or implementation work, not product identity.                                                                                                       |
| Another product owns the AI-governance evidence story first                       | A competing substrate ships a compelling governance or evidence-export package for engineering and platform teams before Open Pincery has a reference deployment                                                                     | Competitor docs, product launches, customer references                                                               | Monthly                                                                  | Project maintainer | Accelerate the evidence wedge. Either ship the minimal evidence-bundle path in the next cycle or choose interoperability over head-on competition. Do not continue with a vague differentiator.                                                                                                                  |
| The conversation-as-surface thesis is wrong                                       | Discovery calls, pilot feedback, or sales conversations consistently show buyers want workflow authoring before they will buy or adopt                                                                                               | Discovery notes, pilot reviews, issue backlog from real users                                                        | Quarterly and after every five discovery calls                           | Project maintainer | Pause the current thesis, write an ADR, and decide whether to add a thin authoring surface, narrow the buyer, or abandon the direction. Do not half-build a workflow UI by drift.                                                                                                                                |
| Rust is the wrong ecosystem bet for contribution and extension                    | External contribution to Rust substrate work stays near zero for two consecutive quarters, while prospective contributors ask for Python or TypeScript extension points                                                              | GitHub contribution history, inbound contributor conversations, issue discussions                                    | Quarterly                                                                | Project maintainer | Double down on the protocol boundary. Prioritize a Python reference harness or SDK and stop assuming the Rust implementation alone can grow the ecosystem.                                                                                                                                                       |
| Self-hosted AI stops making economic or operational sense                         | Frontier vendors make self-host or local inference non-competitive for a sustained period, or buyer conversations consistently reject self-host as a requirement                                                                     | Pricing reviews, vendor terms, discovery calls, local-model benchmarks                                               | Quarterly                                                                | Project maintainer | Reposition toward hosted-agent audit and evidence tooling. Treat self-host as a supported deployment mode, not the core identity of the product.                                                                                                                                                                 |
| Mission catalog discipline breaks                                                 | The founder handles the same manual task three or more times without either promoting it to a cataloged mission type with an acceptance contract or recording an explicit decision not to                                            | Personal log of repeated manual tasks; weekly digest review; honest founder self-audit                               | Weekly (during the 90-day founder-operated benchmark), otherwise monthly | Project maintainer | Stop new substrate work until the repeated task is either formalized as a mission type (contract, capabilities, required evidence) and added to the catalog, or explicitly recorded in an ADR as intentionally manual. Ambiguous drift between manual and agent work is the primary way this plan produces slop. |
| Reasoner sovereignty erodes                                                       | A Tier 1 mission, substrate feature, or acceptance contract becomes dependent on a vendor-consumer endpoint or a proprietary-only capability with no equivalent available via the enterprise-bounded or sovereign governance classes | Substrate dependency graph; reasoner-abstraction implementation inventory; per-mission governance-class declarations | Monthly, and on every new Tier 1 mission added                           | Project maintainer | Stop feature work. Either provide an equivalent implementation under the enterprise-bounded or sovereign class, or open an ADR explicitly accepting the erosion and naming which Durable Bet is being relaxed. No silent coupling.                                                                               |
| Bootstrap ladder stalls at Stage 0                                                | The 90-day founder-operated benchmark completes while the sovereignty ladder remains at Stage 0, with no decision recorded to advance to Stage 1 or to explicitly stay                                                               | Sovereignty-ladder stage declared in the latest weekly digest; git log; ADR history                                  | Monthly, and at the end of the 90-day benchmark                          | Project maintainer | Open an ADR within 7 days. Either commit to advancing to Stage 1 within a named window, or accept Stage 0 explicitly and state the cost that decision implies for the sovereignty wedge. Staying at Stage 0 by drift is a governance failure.                                                                    |

## Notes On Interpretation

### "Credible" means market-relevant, not merely announced

A launch page alone is not enough. The signal should be credible because at least one of these is true:

1. There is production documentation and an addressable product.
2. There is visible adoption by target buyers.
3. The feature materially changes the comparison buyers will make.

### Discovery evidence outranks internal preference

If repeated real-user conversations contradict the current thesis, the conversations win. This document exists to prevent attachment to a favorite framing after the market has moved.

### Not every trigger means panic

A tripwire does not automatically mean "abandon the project." It means "stop assuming the old plan still holds." The required response is deliberate reassessment, not reflexive retreat.

## Companion Docs

- `docs/input/north-star-2026-04.md`
- `docs/input/v6_pre_iterate/strategic-answers-2026-04.md`
- `docs/input/v6_pre_iterate/first-principles-assessment.md`

## Next Review

Next light review: 2026-05-20.
Next full review: 2026-07-20.
