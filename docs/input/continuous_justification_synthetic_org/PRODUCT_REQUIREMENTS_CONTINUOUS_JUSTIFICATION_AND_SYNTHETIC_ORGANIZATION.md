# Product Requirements Document: Continuous Justification + Synthetic Organization Bootstrap for Open Pincery

**Document status:** implementation-grade input specification  
**Intended location:** `docs/input/continuous-justification-prd.md`  
**Primary implementation harness:** Lights Out SWE / Lights Out SWE Plugin, but the document is intentionally usable by any fresh coding-agent harness  
**Target repository:** `RCSnyder/open-pincery`  
**Grounding snapshot:** 2026-08-15; every builder MUST re-inspect current `main` before changing code  
**Foundational feature name:** **Continuous Justification**  
**Program north star:** **Synthetic Organization Bootstrap**  
**Short product phrase:** *A continuous agent should remember not only what it did, but why it is currently justified in doing what comes next; a synthetic organization should preserve that discipline while turning durable missions into coordinated, evidence-producing action.*

---

# 0. Instructions To Fresh Autonomous Builders

This document is meant to survive loss of the originating conversation. Treat it as a high-information input artifact, not as proof that every suggested implementation detail matches the repository at the moment you begin.

Before EXPAND or code changes:

1. Read the repository root `README.md`, `DELIVERY.md`, `preferences.md`, `docs/input/north-star-2026-04.md` or its current successor, `docs/SECURITY.md`, `docs/api.md`, current migrations, and the existing TLA+/spec-coverage material.
2. Inspect the current implementation of:
   - `src/runtime/capability.rs`
   - `src/runtime/capability_nonce.rs`
   - `src/runtime/maintenance.rs`
   - `src/runtime/wake_loop.rs`
   - `src/runtime/lifecycle.rs`
   - `src/runtime/tools.rs`
   - `src/models/event.rs`
   - `src/models/projection.rs`
   - `src/api/events.rs`
   - `src/api/audit.rs`
   - relevant CLI noun/verb conventions under `src/cli/`
   - current capability, nonce, event-log, maintenance, lifecycle, prompt-injection, and audit tests.
3. Reconcile this PRD against current `main`. Current code wins over stale path/detail assumptions **unless** this PRD explicitly specifies a new required behavior.
4. Do not delete or weaken shipped security/correctness controls to make this feature easier. In particular preserve the current architecture around sandboxing, event-log integrity, single-use scoped capability nonces, prompt-injection controls, credential isolation, lifecycle CAS semantics, and spec-coverage discipline.
5. Preserve Open Pincery's event-sourced source-of-truth model. Continuous Justification should be represented by durable events and deterministic/rebuildable projections where practical; do not create a mutable opaque “trust score” as a second source of truth.
6. The reasoner/LLM may **propose** claims, evidence links, desired actions, or transition explanations. The reasoner MUST NOT be the trusted authority that marks its own high-impact action justified.
7. Build the smallest useful vertical slice first. Shadow-mode observation and explainability are more important than immediately blocking all tool calls.
8. Keep user-visible product language comprehensible. Internally this feature draws from assurance, truth-maintenance, provenance, viability, and authority theory. The operator should see concrete language such as “this evidence expired,” “this change invalidated the claim,” “full shell authority was narrowed to read-only,” and “run this test to restore permission.”
9. Avoid unsupported novelty claims in docs. The primitive ingredients have extensive prior art. The product thesis is the composition described in this PRD.
10. If this PRD conflicts with a security invariant, formal model, or stronger current repository contract, STOP and write a precise blocker/options analysis rather than papering over the conflict.

The autonomous build pipeline should derive stable acceptance criteria from the `AC-CJ-*` requirements in this document and preserve those IDs through scaffolding and verification.

This revision also contains a second, broader requirement family, `AC-SO-*`, for the **Synthetic Organization Bootstrap**. Do **not** interpret all `AC-SO-*` items as one release. EXPAND/DESIGN MUST preserve the staged program structure defined below. Continuous Justification remains the first institutional primitive and may ship independently. Synthetic-organization capabilities must be introduced as vertical slices with evidence that the preceding substrate is stable.

---

# 1. Executive Summary

Open Pincery already treats an agent as a **continuous entity** rather than an ephemeral request: durable identity, append-only history, bounded wakes, between-wakes maintenance, asynchronous triggers, event-sourced projections, a sandboxed tool boundary, scoped single-use capability nonces, and a tamper-evident audit trail.

That architecture answers two important questions well:

1. **Who is this agent over time?**
2. **What actually happened?**

Continuous Justification adds the next question:

> **Given what has changed and what is currently known, what is this continuous agent justified in doing next?**

The feature creates a durable **assurance projection** for each agent/workspace. It records and derives the current applicability of claims, evidence, assumptions, counterevidence, transition witnesses, authority constraints, and recovery requirements. It uses that state to compute an **authority envelope** for consequential actions.

The authority envelope is not a generic confidence score. It is a structured decision with one of these outcomes:

- `ADMIT` — proposed scope is justified;
- `CONSTRAIN` — a smaller/reversible scope is justified;
- `REQUIRE_EVIDENCE` — a named stale/unknown claim must be resolved;
- `REQUIRE_AUTHORITY` — evidence may suffice but the actor lacks permission;
- `SHADOW` — observation/simulation without the side effect is allowed;
- `BLOCK` — explicit contradiction, unacceptable irreversibility, invalid lineage, or loss of corrective capacity makes the effect unjustified.

The first production integration point is the existing **capability nonce gate**. For selected high-consequence capability shapes, nonce issuance becomes:

```text
agent intent
  -> structured action intent
  -> current assurance projection
  -> deterministic justification decision
  -> scoped authority envelope
  -> capability nonce mint / narrow / refuse
  -> external effect
  -> durable action + outcome events
  -> assurance projection update
```

The project must initially run in **shadow mode**: compute and record what Continuous Justification *would* have done without changing current runtime authorization. This generates the evidence needed to determine whether the feature is useful and calibrate false blocks. Enforcement is introduced only for narrowly selected capability classes after shadow results demonstrate value.

The long-term differentiator is not “policy for agents,” “audit logs,” “assurance lineage,” or “runtime authorization” by themselves. Those are established categories. The product synthesis is:

```text
meaningful change
-> determine which prior reasons for trust still apply
-> preserve prior counterevidence
-> evaluate current knowledge + authority + recoverability
-> choose the smallest justified action scope
-> identify the minimum new evidence needed to widen authority
-> record the decision and observe the outcome
-> repeat across the same durable agent identity
```

This is **continuous justification**: epistemic and authority state evolves alongside the continuous agent rather than resetting every wake.

---


## 1.1 Broader Program North Star: From Continuous Agent To Synthetic Organization

Continuous Justification is intentionally smaller than the long-term opportunity. Open Pincery's current README already describes a strategic direction toward a sovereign substrate capable of running a one-person company. This PRD treats that direction seriously without forcing the first implementation to solve organization theory all at once.

The broader north star is a **synthetic organization engine**: a durable computational organization that can accept a mission, preserve its governing intent, acquire and evaluate evidence, decompose the mission into obligations, instantiate or assign roles, delegate bounded authority and resources, execute work through continuous agents and external tools, verify outcomes, preserve counterevidence and dissent, recover from failure, and revise its own operating structure through explicit governed transitions.

A synthetic organization is **not** defined as "many LLMs talking to each other." It is defined by durable organizational state and institutional machinery:

```text
mission / intent
    -> obligations and success conditions
    -> roles and accountable owners
    -> work and dependency graph
    -> evidence requirements
    -> bounded resource + authority allocation
    -> execution
    -> independent verification / outcome observation
    -> audit + learning
    -> governed reorganization
    -> next cycle
```

The long-term product question is therefore broader than whether an individual agent action is justified:

> **Can Open Pincery become a general mission substrate in which a durable organization of agents can get consequential work done while keeping knowledge, authority, resources, recovery, and institutional change coupled to evidence?**

Continuous Justification supplies the first load-bearing answer: **authority must be continuously justified rather than permanently inherited.** The rest of the synthetic-organization program builds around that principle.

This PRD therefore has two scopes:

1. **Foundational feature scope — Continuous Justification (`AC-CJ-*`).** Concrete enough to build now.
2. **Program scope — Synthetic Organization Bootstrap (`AC-SO-*`).** A staged architecture and research program that should guide future versions, preventing the implementation from optimizing into a narrow authorization feature that cannot grow into a general organization engine.


# 2. Why This Belongs In Open Pincery

Do not implement this as a separate orchestration product if it can be expressed cleanly inside Open Pincery's existing event/runtime boundaries.

Open Pincery has several architectural properties that make it unusually suitable:

## 2.1 Durable identity supplies the lineage axis

An ephemeral agent invocation cannot easily answer whether a current action is consistent with the evidence and counterevidence accumulated by “the same” agent over weeks. Open Pincery already has stable agent identity and durable history.

Continuous Justification attaches assurance continuity to that existing identity rather than inventing a separate identity system.

## 2.2 Append-only events supply the epistemic ledger

The event stream already records messages, tool calls, wake boundaries, and other durable facts. The assurance layer should add events such as:

- claim asserted;
- evidence observed;
- counterevidence observed;
- assumption invalidated;
- claim staled;
- claim revalidated;
- transition witnessed;
- authority requested;
- authority constrained;
- authority granted;
- justification decision made;
- corrective path degraded/restored;
- outcome observed.

The event log remains authoritative. The assurance projection is rebuildable derived state.

## 2.3 Between-wakes maintenance supplies a natural homeostasis point

Open Pincery already performs maintenance after a wake to update durable projections. Continuous Justification should use this phase to perform deterministic maintenance where possible:

- expire evidence/authority by TTL;
- detect assumptions whose validity window has elapsed;
- apply explicitly recorded invalidations;
- recompute claim applicability;
- narrow standing authority if supporting evidence disappeared;
- emit projection/update events where the current architecture requires them.

LLM maintenance may propose semantic interpretations, but deterministic rules own expiry, dependency propagation, and authority narrowing.

## 2.4 Existing capability nonces supply the effect boundary

The runtime already has a stronger primitive than “the prompt told the model not to.” Single-use, TTL-bounded, workspace-scoped, capability-shape-bound nonces can become the enforcement artifact produced after justification.

Continuous Justification should not replace nonce security. It should become an additional decision input to nonce issuance for configured capability classes.

## 2.5 Hash-chained events supply forensic continuity

Justification receipts, invalidations, and authority changes must be auditable. The existing tamper-evident event chain should cover the new event types through the ordinary event insertion path rather than creating a separate unchained ledger.

## 2.6 TLA+ discipline supplies a place for the core state machine

The feature has non-trivial concurrency and safety semantics: claim invalidation, decision freshness, nonce issuance, event ordering, maintenance, and wake concurrency. The project already requires formal spec coverage for critical state-machine behavior. Continuous Justification should add or extend a TLA+ model before enforcement becomes a P0 control.

---

# 3. Product Thesis And User Value

## 3.1 Problem

Long-lived autonomous agents accumulate permission and apparent competence while their environment, implementation, tools, prompts, models, policies, evidence, and assumptions change.

Most systems can answer some subset of:

- what policy applies?;
- what tool is being called?;
- who owns the agent?;
- what happened before?;
- did tests pass?;
- is the action syntactically allowed?;
- does the current runtime state look healthy?.

The difficult missing question is:

> **Do the reasons that previously justified this agent's authority still apply after what changed?**

Examples:

- A codebase-steward agent learned that a migration is unsafe because a downstream consumer still uses an old schema. Two weeks later its context is summarized and the counterexample disappears from the prompt. It must not regain migration authority merely because it forgot the evidence.
- A tool verifier is updated. Old “tool safe” evidence may remain stored, but if the verifier semantics changed the old evidence may be stale.
- A production assumption (“latency under 200 ms”) expires. The agent should automatically lose authority to perform a rollout that depended on bounded failover timing.
- A self-configuring agent modifies its durable identity/instructions. The modification may invalidate claims that justified broad shell capability.
- A full destructive action is not justified, but a read-only or 1% canary action would generate the missing evidence safely. The system should constrain rather than merely block.

## 3.2 User value

For an operator, Continuous Justification should produce four practical benefits:

1. **Fewer silent trust regressions.** A durable agent cannot regain authority just by forgetting why trust was reduced.
2. **More useful denials.** Instead of “blocked by policy,” the operator sees which claim is stale/invalid and what evidence would restore the desired capability.
3. **Progressive autonomy.** Agents can earn broader authority under well-evidenced, reversible conditions and automatically narrow when the world becomes anomalous.
4. **Forensic explainability.** The event log can reconstruct not only what the agent did, but what evidence and authority justified the action at the time.

## 3.3 Product language

Prefer:

- “justification” over “AI confidence”;
- “evidence applies / became stale” over “trust score fell”;
- “authority narrowed” over “agent punished”;
- “counterevidence” over “negative memory”;
- “transition witness” over “LLM says compatibility is fine”;
- “corrective path” over vague “safety fallback.”

---

# 4. Goals

## G1 — Durable assurance state

Open Pincery can derive, query, and audit the current assurance state for an agent/workspace from durable events and configuration.

## G2 — Change-aware applicability

When a meaning-bearing dependency changes, the system can mark dependent claims/evidence stale unless explicit revalidation or transition evidence restores applicability.

## G3 — Counterevidence preservation

A successor/wake/projection rebuild cannot silently convert a previously invalid or stale critical claim back to valid without new justification.

## G4 — Evidence-aware capability decisions

For selected consequential capability requests, nonce issuance can incorporate current claim/evidence state and produce an `ADMIT`, `CONSTRAIN`, `REQUIRE_EVIDENCE`, `REQUIRE_AUTHORITY`, `SHADOW`, or `BLOCK` decision.

## G5 — Explainable remediation

Any non-`ADMIT` decision identifies the blocking obligation and at least one concrete remediation/evidence option when a safe known path exists.

## G6 — Corrective-capacity preservation

For high-impact capability classes, decision policy can require a declared, currently available correction path before broad authority is granted.

## G7 — Shadow-first adoptability

The feature can be enabled in observation mode with zero behavior change to existing tool execution, generating an empirical disagreement dataset before enforcement.

## G8 — Tool/provider neutrality

The assurance kernel does not depend on one LLM provider, coding agent, CI product, provenance format, or policy engine.

## G9 — Rebuildable and deterministic core

Given the same authoritative events, configuration, and policy version, the core assurance projection and decision result are deterministic, except for explicitly external evidence acquisition steps.

## G10 — Does not weaken existing security boundaries

All current sandbox, nonce, vault, prompt-injection, lifecycle CAS, audit-chain, and startup-integrity controls remain at least as strong as before.

---

# 5. Non-Goals

The first implementation MUST NOT attempt to be:

1. a universal theorem prover;
2. a replacement for TLA+, Apalache, Lean, tests, fuzzers, or human review;
3. a replacement for SLSA/in-toto/Sigstore or supply-chain provenance systems;
4. a new general-purpose OPA/Cedar policy language;
5. a numeric “trust score” for agents;
6. a prediction that an action is universally safe;
7. a fully autonomous moral/ethical decision system;
8. an enterprise GRC platform;
9. a data-lineage catalog;
10. a replacement for the event log or audit chain;
11. a reason to make all existing tool calls slower by default;
12. a requirement that every normal shell command carry a complex assurance case;
13. an LLM-only classifier placed in the security-critical nonce path;
14. a system that infers deep semantic dependencies perfectly from arbitrary code in v1;
15. an automatic self-modification framework;
16. a new cryptographic signing system;
17. a hidden opaque safety layer operators cannot inspect or override under explicit audited authority.

---

# 6. Core Concepts And Terminology

## 6.1 Assurance entity

A durable object participating in justification. Initial entity kinds:

- `claim`;
- `evidence`;
- `assumption`;
- `counterevidence` (may be represented as evidence with contradicting relation rather than a separate storage kind);
- `transition_witness`;
- `authority_grant` / authority reference;
- `corrective_path`;
- `action_intent`;
- `justification_decision`.

The implementation may normalize or merge entity kinds if that yields a simpler domain model. Preserve semantics.

## 6.2 Claim

A concrete proposition used to justify a decision.

Examples:

- “This workspace checkout contains no uncommitted user changes.”
- “The database backup completed and passed restore verification within the last 24 hours.”
- “The target branch is protected by the configured review rule.”
- “This proposed migration remains readable by the rollback binary.”
- “The external endpoint is the intended production tenant.”

A claim is not just a label like `safe` or `ready`.

## 6.3 Evidence

An artifact/observation that supports or contradicts one or more claims within a stated semantic scope.

Evidence may originate from:

- deterministic local checks;
- repository tests;
- formal model checking;
- runtime telemetry;
- signed provenance;
- human approval;
- external APIs;
- restoration drills;
- observed tool outcomes.

Evidence MUST NOT be automatically generalized beyond its defined reach.

## 6.4 Assumption

A proposition relied upon by a claim/evidence relation but not established permanently.

Assumptions may have:

- observation source;
- TTL/expiry;
- validity domain;
- invalidation event;
- scope.

## 6.5 Applicability

Whether existing evidence still justifies a claim after changes in implementation, model, specification, verifier, authority, environment, or scope.

Initial statuses:

- `preserved`;
- `stale`;
- `invalid`;
- `transported`;
- `replaced`;
- `unknown`.

Do not collapse `stale` and `invalid`: stale means “may still be true but cannot currently authorize this action”; invalid means relevant counterevidence contradicts the claim.

## 6.6 Transition witness

Explicit evidence/argument that justifies moving a claim/authority relation across a meaning-bearing change.

Examples:

- a model-check result proving refinement from protocol v1 to v2;
- round-trip tests establishing backward-compatible schema semantics;
- an operator-approved policy migration mapping old authority scope to new scope;
- an independent verifier confirming that a new checker preserves a critical acceptance property.

A transition witness need not be a formal proof in all cases, but its producer, scope, and evidence must be durable.

## 6.7 Semantic layer

A classification of change depth used for invalidation and escalation. Initial vocabulary:

1. `runtime` — values, workload, environment, observed state;
2. `implementation` — source/config/dependency implementation;
3. `architecture` — interfaces, boundaries, topology, persistence/concurrency model;
4. `model` — causal/system/world model used to reason about behavior;
5. `specification` — requirements, invariants, acceptance semantics;
6. `verifier` — test/check/proof/evidence-production semantics;
7. `governance` — authority, policy, approval, capability semantics;
8. `meta_governance` — the process that changes/verifies governance or verifier policy.

This is a pragmatic initial taxonomy, not a claim of universal ontology. It MUST be represented so future versions can extend it without destructive migration.

## 6.8 Authority envelope

The currently justified scope of consequential action for an agent/action class.

Examples:

- read-only shell in workspace;
- write access but no network;
- one production tenant but not global;
- 1% canary deployment, not 100%;
- shadow/simulation only;
- capability allowed for 15 minutes;
- destructive operation requires operator approval.

## 6.9 Corrective path

A credible route from discovering error to validating correction:

```text
observe -> diagnose -> repair/alternative -> verify -> authorize -> execute -> validate
```

The initial implementation can model corrective paths declaratively rather than automatically infer them.

## 6.10 Counterevidence preservation

A durable rule: a critical claim that became stale/invalid cannot silently return to preserved status merely because context/history was summarized, an agent woke in a new context, a branch was merged, or projections were rebuilt.

## 6.11 Epistemic resurrection

The prohibited transition where a previously weakened critical semantic claim appears valid again with no new discriminating evidence, scope change, or transition/replacement witness.

---

# 7. Product Behavior Overview

The feature has four conceptual planes.

## 7.1 Observation plane

Collect durable facts/evidence from existing runtime activity and configured checks.

Examples:

- capability request details;
- tool outcome;
- audit/event chain health;
- backup freshness;
- current workspace/agent identity;
- configured policy version;
- runtime assumption observations.

## 7.2 Assurance plane

Maintain claim/evidence/assumption relationships and compute current applicability.

## 7.3 Decision plane

Given an `ActionIntent`, compute the authority envelope/verdict.

## 7.4 Enforcement plane

Initially shadow-only. Later, for configured capability classes, use the decision to mint, narrow, require approval for, or refuse capability nonces.

These planes SHOULD be modular. Observation and assurance state should remain useful even if enforcement is disabled.

---

# 8. Primary User Stories

## US-CJ-001 — Operator can inspect why an action was allowed

As an operator, I can inspect a tool action and see the durable justification decision, supporting claims/evidence, authority scope, policy version, and recovery assumptions that were current at execution time.

## US-CJ-002 — Agent loses authority when evidence expires

As an operator, I can configure an evidence/assumption TTL such that a broad capability decision automatically becomes stale/narrower after expiry until revalidated.

## US-CJ-003 — Prior counterexample survives future wakes

As an operator, when a critical claim is invalidated by a counterexample, subsequent wakes cannot treat the claim as valid unless new evidence or an explicit transition/replacement witness resolves it.

## US-CJ-004 — Full action can be constrained rather than blocked

As an agent, when full authority is not justified but a reversible subset is, I receive a narrower capability envelope such as read-only, single-tenant, canary, rate-limited, or shadow mode.

## US-CJ-005 — Block explains what would restore authority

As an operator/agent, a `REQUIRE_EVIDENCE` decision identifies the exact stale/unknown claim and one or more configured evidence actions capable of restoring it.

## US-CJ-006 — Self-configuration invalidates dependent assurance

As an operator, if between-wakes maintenance changes durable agent identity/instructions in a way classified as meaning-bearing, dependent claims can become stale and authority can narrow.

## US-CJ-007 — Verifier change cannot silently preserve old assurances

As an operator, when the verifier/evidence-production mechanism changes, evidence whose trust semantics depend on that verifier becomes stale unless explicitly revalidated/transported.

## US-CJ-008 — Shadow deployment measures disagreement

As a maintainer, I can enable Continuous Justification in shadow mode, compare its decisions to current capability-gate outcomes, and export disagreements without changing runtime effects.

## US-CJ-009 — Emergency override is possible but audited

As an authorized operator, I can perform an explicit emergency override with reason, scope, TTL, and durable event receipt. The override does not rewrite claim truth; it records that authority accepted action despite unresolved assurance.

## US-CJ-010 — Projection can be rebuilt

As a maintainer, I can rebuild the assurance projection from authoritative events/configuration and obtain the same semantic state/fingerprint for deterministic inputs.

---

# 9. Trust Model

Continuous Justification MUST make trust boundaries explicit.

## Trusted / higher-trust components

Initially, the trusted decision path should include only deterministic Rust code and current protected runtime state required to:

- parse structured action intent;
- read current assurance projection;
- apply invalidation/applicability rules;
- apply configured decision policy;
- read authoritative capability/nonce constraints;
- emit decision events;
- mint/narrow/refuse nonces when enforcement is enabled.

## Untrusted or proposal-only inputs

Treat as untrusted/proposal-only unless independently checked:

- LLM statement “this is safe”;
- LLM-created claim relationships;
- free-form maintenance summary;
- agent-authored transition witness prose;
- external web content;
- tool output not bound to the expected command/artifact/environment;
- user-provided evidence where authenticity matters but was not verified.

LLMs may help generate structured candidates, but deterministic validation decides whether candidate data can affect authority.

## Human authority

A human approval can establish that an authorized human accepts/permits a risk within scope. It does not automatically establish an empirical claim as true.

---

# 10. Core Safety Invariants

The implementation must preserve these invariants. Assign formal/test coverage to each.

### INV-CJ-001 — Agent text is not lifecycle/authority truth

No LLM-generated message or maintenance prose directly changes a high-impact authority decision without passing the structured event/validation path.

### INV-CJ-002 — No silent resurrection

A critical semantic claim that is `invalid` or `stale` cannot transition to an authority-supporting preserved state without a valid restoring event: new applicable evidence, scope distinction, explicit revalidation, transition witness, or replacement.

### INV-CJ-003 — Invalidation propagates before authority expansion

If an event invalidates an assumption/evidence dependency used by current authority, the dependent authority MUST be considered stale/narrowed before a new broader nonce can be minted.

### INV-CJ-004 — Decision is bound to state

A justification decision is bound to the relevant agent/workspace, action intent, assurance-state fingerprint/version, policy version, and time/expiry.

### INV-CJ-005 — Nonce scope cannot exceed decision scope

When enforcement is enabled, the minted capability nonce MUST NOT grant a capability shape, workspace, TTL, or effect scope broader than the decision's authority envelope.

### INV-CJ-006 — Existing nonce security remains intact

Continuous Justification MUST NOT weaken single-use, TTL, workspace, replay, or capability-shape rules already enforced by the nonce system.

### INV-CJ-007 — Shadow mode has no effect authority

Shadow decisions MUST NOT alter tool execution behavior, nonce issuance, or current runtime authorization except for writing observation/decision events and metrics.

### INV-CJ-008 — Emergency override does not rewrite evidence

An override may authorize action under explicit human authority, but MUST NOT mutate an `invalid` claim to `preserved` merely to make the decision graph look clean.

### INV-CJ-009 — Counterevidence is append-only history

Counterevidence can be superseded/resolved, but its historical existence remains auditable through the event stream.

### INV-CJ-010 — Projection is rebuildable

Projection tables are derived state; authoritative events and configuration are sufficient to rebuild them or an explicit migration/bootstrap rule exists and is tested.

### INV-CJ-011 — Candidate cannot self-approve deep evaluator mutation

An agent-changing action that modifies verifier/governance semantics cannot be approved solely by a verifier/governance process contained entirely inside the candidate-controlled path.

### INV-CJ-012 — Fail closed only where configured

Malformed Continuous Justification state/config in enforced capability classes MUST fail closed; unrelated/unenforced capability classes preserve existing behavior. Shadow mode must surface errors without unexpectedly blocking existing actions.

---

# 11. Functional Requirements And Acceptance Criteria

Feature-scoped IDs below are stable inputs. The build harness may derive sub-criteria but MUST preserve traceability to these IDs.

## AC-CJ-001 — Feature flags and operating modes

The runtime supports at least:

```text
off
shadow
enforce
```

with workspace-level or deployment-level configuration consistent with existing project configuration patterns.

Required behavior:

- `off`: no assurance decision computation in hot path beyond compatibility-required no-op hooks;
- `shadow`: compute/record decisions but preserve pre-feature capability behavior;
- `enforce`: configured capability classes obey decision envelope.

Tests MUST prove shadow mode cannot block a capability otherwise allowed by the existing gate.

## AC-CJ-002 — Assurance event vocabulary

Add a versioned event vocabulary sufficient to represent:

- claim assertion/update/retirement;
- evidence observed;
- evidence invalidated/expired;
- assumption asserted/invalidated/expired;
- transition witness recorded;
- authority decision requested/computed;
- authority constrained/expanded;
- corrective path state change;
- explicit override;
- outcome observation.

Events MUST flow through the existing event insertion/hash-chain mechanism.

Event payloads MUST be schema/versioned or otherwise evolvable without ambiguous interpretation.

## AC-CJ-003 — Deterministic assurance projection

Implement a rebuildable projection representing current assurance state per appropriate scope (agent, workspace, or both based on current architecture).

The projection MUST expose:

- entity ID/kind;
- semantic statement/fingerprint where applicable;
- current status;
- support/contradiction dependencies;
- assumptions;
- sensitive layers;
- freshness/expiry;
- last transition event;
- current evidence source references.

Given identical event/config inputs, a rebuild MUST produce an equivalent projection fingerprint.

## AC-CJ-004 — Claim status semantics

Support at least:

```text
preserved
stale
invalid
transported
replaced
unknown
```

No implicit coercion from `unknown` to `preserved` in enforced decisions.

## AC-CJ-005 — Dependency invalidation

When an assumption/evidence/entity used by a claim becomes invalid/stale, dependent claims are recomputed/marked before they can support a new enforced decision.

Provide unit tests for multi-hop propagation:

```text
ASSUMPTION -> EVIDENCE/APPLICABILITY -> CLAIM -> AUTHORITY REQUIREMENT
```

## AC-CJ-006 — Semantic layer sensitivity

Claims/evidence MAY declare sensitivity to one or more semantic layers. A meaning-bearing change event at a sensitive layer triggers applicability review/staleness according to policy.

The initial layer vocabulary is extensible and stored without requiring a destructive schema migration for future additions.

## AC-CJ-007 — Counterevidence preservation

If a critical claim is invalidated/staled, a later wake/projection rebuild cannot restore it to authority-supporting status without an explicit restoring event or independent applicable evidence path.

Add a regression test reproducing “forgotten counterexample after wake/context compaction.”

## AC-CJ-008 — Semantic identity drift detection

When the same stable assurance identity is reused with changed meaning-bearing fields (statement/scope/assumptions/verifier/authority semantics as applicable), the system records semantic drift and does not assume continuity.

The exact fingerprint algorithm is an implementation decision but MUST be deterministic, documented, and exclude nondeterministic forensic fields such as timings/stdout where those do not affect semantic identity.

## AC-CJ-009 — Transition witness

Support durable transition witnesses connecting predecessor and successor assurance semantics.

A witness includes at least:

- witness ID;
- source/predecessor identity or state reference;
- target/successor identity or state reference;
- mode (`transport`, `replace`, `merge`, or extensible equivalent);
- supporting evidence references;
- producer/authority metadata;
- scope;
- timestamp/event reference.

A witness does not become valid solely because the LLM wrote prose; validation rules must be explicit.

## AC-CJ-010 — ActionIntent model

Represent a consequential proposed action as structured data before Continuous Justification decides on it.

Minimum fields:

- agent/workspace;
- capability/tool/action kind;
- requested scope;
- target/resource where knowable;
- reversibility class;
- consequence/risk class configured by policy;
- requested TTL/budget if applicable;
- relevant current wake/event reference.

Avoid requiring the LLM to perfectly classify consequence. Policy may infer/default based on capability shape.

## AC-CJ-011 — Five-part action judgment

Decision logic represents independently:

- `CAN` — technical feasibility/preconditions within modeled scope;
- `KNOW` — required claims currently supported/applicable;
- `MAY` — actor authority/policy;
- `ADMISSIBLE` — configured safety/normative constraints;
- `RECOVER` — configured recovery requirement.

The implementation may use more precise internal names, but API/explanation must not collapse these into an opaque single score.

## AC-CJ-012 — Decision verdicts

Support verdicts:

```text
ADMIT
CONSTRAIN
REQUIRE_EVIDENCE
REQUIRE_AUTHORITY
SHADOW
BLOCK
```

A verdict includes human-readable reason codes and machine-readable blocking obligations.

## AC-CJ-013 — Authority envelope

A decision may narrow the requested capability along dimensions supported by the existing capability model, such as:

- workspace;
- action/capability shape;
- TTL;
- single-use count (preserve existing semantics);
- read/write/destructive category if representable;
- target scope/tenant/repository;
- rate/budget;
- canary/shadow mode where the underlying tool supports it.

Do not invent scope dimensions the current capability subsystem cannot enforce. If a desired envelope cannot be represented safely, return a stronger decision (`REQUIRE_AUTHORITY`/`BLOCK`) rather than pretending.

## AC-CJ-014 — Capability nonce integration

In enforce mode for configured capabilities:

```text
ActionIntent -> JustificationDecision -> CapabilityNonce
```

Nonce scope MUST be a subset of the authority envelope.

The decision ID/fingerprint SHOULD be durably associated with nonce issuance so an audit can reconstruct why the nonce existed.

## AC-CJ-015 — Decision freshness and TOCTOU

A decision that depends on mutable assurance state MUST have freshness/version binding sufficient to prevent minting broad authority from a state already invalidated before nonce use.

Design must explicitly analyze TOCTOU between:

- decision computation;
- nonce issuance;
- nonce consumption;
- invalidation events.

Use existing DB transaction/CAS patterns where appropriate. This behavior requires formal/state-machine coverage before enforcement.

## AC-CJ-016 — Shadow disagreement logging

Shadow mode records:

- existing capability gate result;
- Continuous Justification result;
- reason codes;
- requested/envelope scopes;
- relevant assurance fingerprint;
- whether the action subsequently succeeded/failed where observable.

Provide query/export suitable for classifying disagreements.

## AC-CJ-017 — Explanation surface

For a non-ADMIT decision, expose:

- requested action;
- verdict;
- blocking claim(s)/authority/recovery requirement;
- invalidating/change event where applicable;
- current evidence status;
- safe permitted subset if any;
- next evidence/remediation options if configured;
- expiry/recheck trigger.

Do not expose secrets, raw credentials, or sensitive hidden prompt content.

## AC-CJ-018 — Evidence action catalog

Support configuration of evidence-producing actions associated with claim classes. Initial implementation may be static/configured rather than AI-planned.

Example:

```text
claim: backup_restorable
  evidence options:
    - run restore smoke check
    - operator-provided signed drill result
```

Evidence action records should specify cost/risk class, producer, semantic reach, and whether execution itself requires capability authorization.

## AC-CJ-019 — Minimum evidence recommendation

For `REQUIRE_EVIDENCE`, return at least one ranked or policy-selected evidence path that could change the decision, when one is known.

V1 need not solve a numeric optimization problem. It MUST avoid recommending evidence that cannot satisfy the blocking obligation.

## AC-CJ-020 — Emergency override

Authorized operator can issue a scoped, expiring override with:

- actor identity;
- action scope;
- reason;
- duration/expiry;
- reference to blocked decision;
- optional incident/change ticket reference.

Override MUST be durably logged and MUST NOT rewrite stale/invalid claims as true.

## AC-CJ-021 — Between-wakes expiry maintenance

Between-wakes maintenance deterministically applies relevant TTL expiries and invalidations before the next broad authority decision.

Do not depend on the maintenance LLM remembering to do this.

## AC-CJ-022 — Self-configuration change event

When durable agent configuration/identity changes in a way designated meaning-bearing, emit/classify a change event so dependent assurance can be reevaluated.

The initial implementation may conservatively classify all identity/instruction changes at a configured semantic layer until finer diff analysis exists.

## AC-CJ-023 — Verifier change handling

Evidence bound to verifier semantics can be invalidated/staled when the verifier version/config/meaning changes.

At minimum, verifier identity/version must be representable for evidence classes where it matters.

## AC-CJ-024 — Corrective path declarations

Support declaring named corrective paths for selected high-consequence action classes.

A path can initially be a structured list of required capabilities/dependencies with optional timing/failure-domain metadata.

## AC-CJ-025 — Corrective path availability

Decision policy can require that at least one declared corrective path is currently available before `ADMIT` for configured action classes.

V1 MUST NOT claim to prove systemic resilience from a single declarative path. Explanation should say what was checked.

## AC-CJ-026 — Common-cause tags

Corrective capabilities/paths can carry failure-domain tags so two paths sharing the same critical failure domain are not presented as independent redundancy.

Full min-cut computation is optional for first enforcement slice; representation and detection of obvious shared critical dependencies are required for the corrective-closure phase.

## AC-CJ-027 — Runtime assumption observation

Provide a way for deterministic/runtime observations to invalidate or refresh an assumption without an LLM round-trip.

Examples: timestamp expiry, health endpoint, backup age, audit chain failure, current branch/commit, workspace state.

## AC-CJ-028 — Query API

Expose read APIs for at least:

- current assurance summary for agent/workspace;
- claim/evidence detail;
- recent justification decisions;
- decision detail/explanation;
- shadow disagreements;
- current authority envelope for configured capability classes.

Follow existing API/auth/OpenAPI conventions.

## AC-CJ-029 — CLI

Add operator CLI using existing noun/verb conventions. Exact naming should be reconciled with current CLI style. Candidate UX:

```text
pcy assurance status <agent>
pcy assurance explain <decision-id>
pcy assurance claims <agent>
pcy assurance disagreements
pcy assurance rebuild <agent> --verify
pcy assurance override ...
```

Do not add CLI surface that is not backed by real implementation.

## AC-CJ-030 — Projection rebuild/verify

Provide a deterministic rebuild or verification command/test path. If production rebuild is too risky for first slice, provide an offline/tested verification path and document operational procedure.

## AC-CJ-031 — Audit chain compatibility

All new authoritative events participate in the existing per-agent event hash-chain mechanism and `pcy audit verify` behavior.

## AC-CJ-032 — Security isolation

Assurance state and explanation interfaces MUST NOT expose vault secrets, raw provider credentials, nonce secret material, or sandbox-host confidential data beyond existing authorized surfaces.

## AC-CJ-033 — Performance budget

Shadow mode overhead MUST be measured. The deterministic decision path should avoid LLM calls and external network calls in the synchronous nonce hot path.

Set a concrete performance target during DESIGN after measuring current capability-gate latency. A default engineering target is that cached/local shadow decision p95 adds no more than 10 ms on representative development hardware, but current repository data should determine the final AC.

## AC-CJ-034 — Failure behavior

Document and test behavior for:

- assurance projection unavailable;
- malformed assurance event;
- stale policy version;
- DB transaction failure;
- projection lag;
- evidence plugin failure;
- corrective path check failure;
- explanation serialization failure.

Enforced critical classes fail closed where security/authority cannot be established. Shadow mode records error and preserves existing capability behavior.

## AC-CJ-035 — Migration compatibility

Database migration is forward-safe under current project backup/restore/version policy. Existing agents with no assurance events continue functioning under `off` and `shadow` modes without manual data migration.

## AC-CJ-036 — Formal state-machine coverage

Before `enforce` is considered shipped for P0/high-consequence capability classes, TLA+ or the repository's current formal state-machine tool covers at least:

- no nonce broader than decision envelope;
- no broad nonce minted from invalidated/stale decision state;
- shadow mode never changes effect authorization;
- invalidation precedes subsequent authority expansion;
- single active wake/lifecycle invariants remain compatible;
- emergency override cannot mutate claim truth;
- decision/state versioning avoids replaying obsolete broad authority.

Add spec-coverage manifest entry consistent with current project rules.

## AC-CJ-037 — Property/negative tests

Include known-bad tests demonstrating the gate rejects or narrows:

- expired evidence;
- explicit counterexample;
- semantic identity drift;
- verifier change without revalidation;
- authority missing;
- missing recovery path for configured high-risk action;
- stale decision replay;
- nonce scope attempted wider than decision.

## AC-CJ-038 — Shadow evaluation report

Before enforcement rollout, dogfood Continuous Justification long enough to produce an evidence report containing:

- number of evaluated consequential actions;
- agreement/disagreement rate with existing gate;
- operator classification of disagreements;
- false-block estimate;
- useful catches;
- added latency;
- top reason codes;
- evidence/remediation burden.

This report is a release gate for broad enforcement.

## AC-CJ-039 — Scoped enforcement rollout

Enforcement initially applies only to one or two well-understood capability classes where:

- action structure is representable;
- evidence obligations are observable;
- recovery semantics are known;
- false blocks are operationally tolerable.

Do not flip global enforcement for all shell actions in the first release.

## AC-CJ-040 — Documentation honesty

README/security/docs describe only shipped behavior. Clearly label experimental/shadow controls and do not market Continuous Justification as proving safety or truth.

---

# 12. Suggested Architecture

This section proposes a landing shape. Fresh builders MUST reconcile with current code and simplify where current abstractions make a better fit.

## 12.1 Proposed module boundaries

Candidate Rust modules:

```text
src/assurance/
  mod.rs
  model.rs          # domain types: Claim, Evidence, Status, ChangeLayer, Decision
  projection.rs     # deterministic projection/application rules
  applicability.rs  # invalidation, transport, replacement semantics
  decision.rs       # CAN/KNOW/MAY/ADMISSIBLE/RECOVER policy evaluation
  evidence.rs       # evidence registration/validation interface
  recovery.rs       # corrective-path representation and availability checks
  explain.rs        # machine + human explanation construction

src/runtime/
  capability.rs         # existing; integrate decision input carefully
  capability_nonce.rs   # existing; nonce scope <= authority envelope
  maintenance.rs        # existing; call deterministic assurance maintenance
  wake_loop.rs          # action-intent/decision hooks only as needed
```

Alternative: if the current architecture strongly prefers runtime-local modules, place assurance under `src/runtime/assurance/`. Preserve cohesion and avoid circular dependencies.

The assurance domain SHOULD NOT depend directly on LLM/provider modules.

## 12.2 Projection architecture

Preferred direction:

```text
append-only events
      |
      v
assurance projector
      |
      +--> current assurance entities
      +--> dependency edges
      +--> current status / fingerprint
      +--> current decision-support indexes
```

Projection tables are optimizations/derived state. Rebuildability is required.

## 12.3 Decision architecture

```text
ActionIntent
  + current capability policy
  + assurance projection fingerprint
  + current authority grants
  + required corrective path state
  -> DecisionEngine (deterministic)
  -> JustificationDecision
```

No synchronous LLM call in `DecisionEngine`.

LLM assistance may be used before this point to propose an `ActionIntent` or after a block to draft remediation, but security-sensitive fields must be validated/defaulted deterministically.

## 12.4 Enforcement architecture

```text
existing capability request
  -> existing baseline gate
  -> (if CJ configured for class)
       compute/retrieve justification decision
       intersect requested scope with authority envelope
       if admissible -> mint current nonce format
       else -> refuse / require approval / return constrained proposal
```

Do not fork a second nonce system.

---

# 13. Data Model Guidance

Exact SQL is a design task. The following semantics must be representable.

## 13.1 Assurance entity projection

Candidate fields:

```text
workspace_id
agent_id nullable where workspace-scoped
entity_id
entity_kind
semantic_version / schema_version
statement / structured payload
semantic_fingerprint
status
criticality / consequence class
valid_from_event_id
last_event_id
expires_at nullable
created_at
updated_at (projection metadata only)
```

## 13.2 Dependency edges

```text
from_entity_id
to_entity_id
relation_kind
scope
created_event_id
retired_event_id nullable
```

Relations may include:

```text
supports
contradicts
assumes
sensitive_to
transported_by
replaces
authorizes
requires_recovery
observed_by
```

Avoid an uncontrolled free-form relation explosion in v1. Use a versioned enum plus extensibility plan.

## 13.3 Decisions

Persist or event-record at least:

```text
decision_id
agent_id
workspace_id
action_intent_hash / canonical payload reference
assurance_fingerprint
policy_version
verdict
requested_scope
permitted_scope
blocking_obligations
reason_codes
expires_at
created_event_id
```

If decisions are represented solely as events plus projection, do not duplicate a second authoritative table unnecessarily.

## 13.4 Canonicalization

Any fingerprint used for continuity/security must be calculated over canonical deterministic data:

- sort unordered collections;
- exclude nondeterministic timing/stdout/diagnostic fields unless semantically relevant;
- version canonicalization algorithm;
- test stable output.

Never use free-form LLM explanation text as the semantic identity hash.

---

# 14. Event Contracts

The repository's existing `Event` representation and canonical JSON/hash behavior MUST be reused. Names below are conceptual.

## `assurance_claim_asserted`

```json
{
  "schema_version": 1,
  "entity_id": "claim:backup-restorable",
  "statement": "Latest backup is restorable under current schema version",
  "scope": {"workspace_id": "..."},
  "sensitive_layers": ["runtime", "implementation"],
  "criticality": "high",
  "assumptions": ["assumption:backup-format-supported"]
}
```

## `assurance_evidence_observed`

```json
{
  "schema_version": 1,
  "entity_id": "evidence:restore-drill:...",
  "kind": "command_result",
  "producer": "pcy backup restore-check",
  "subject_digest": "sha256:...",
  "semantic_reach": ["claim:backup-restorable"],
  "result": "support",
  "expires_at": "..."
}
```

## `assurance_evidence_invalidated`

Contains evidence ID, reason code, invalidating event/observation, and scope.

## `assurance_change_observed`

```json
{
  "schema_version": 1,
  "change_id": "change:...",
  "layers": ["verifier"],
  "subjects": ["capability-policy-checker"],
  "semantic_fingerprint_before": "...",
  "semantic_fingerprint_after": "...",
  "source_event_ids": ["..."]
}
```

## `assurance_transition_witnessed`

Records old/new entity references, mode, support evidence, producer/authority, scope.

## `justification_decision`

```json
{
  "schema_version": 1,
  "decision_id": "...",
  "action_intent": { ... },
  "assurance_fingerprint": "...",
  "policy_version": "...",
  "judgment": {
    "can": "pass",
    "know": "fail",
    "may": "pass",
    "admissible": "pass",
    "recover": "pass"
  },
  "verdict": "REQUIRE_EVIDENCE",
  "requested_scope": { ... },
  "permitted_scope": {"mode": "shadow"},
  "blocking_obligations": ["claim:backup-restorable"],
  "next_evidence_options": ["evidence_action:restore-drill"],
  "expires_at": "..."
}
```

## `justification_override`

Must include operator identity, reason, decision reference, scope, expiry, and authorization source.

---

# 15. Applicability And Invalidation Semantics

## 15.1 Default principle

Evidence is valid for a semantic state and scope, not forever.

A meaning-bearing change does not automatically mean the old claim is false. It means the system must determine whether old evidence remains applicable.

## 15.2 Dependency propagation

Implement deterministic propagation rules over declared dependencies.

Example:

```text
assumption:backup-format-supported becomes INVALID
    -> claim:backup-restorable becomes STALE or INVALID according to relation semantics
    -> action requirement KNOW(claim:backup-restorable) fails
    -> destructive migration decision cannot ADMIT
```

## 15.3 Alternative support

A claim with two truly independent support paths should remain supported if one becomes stale and the other remains applicable.

The implementation must not invalidate the claim merely because one support edge failed if policy says any one support environment is sufficient.

Conversely, do not treat two evidence objects as independent if they are both bound to the same invalidated assumption/verifier.

V1 may represent support logic conservatively (for example explicit `all_of` / `any_of` groups) rather than implement a full ATMS.

## 15.4 Expiry

Expiry is deterministic. Once evidence expires, it may remain historically visible but cannot support a fresh decision unless policy explicitly permits stale evidence for that action class.

## 15.5 Invalid vs stale

Use `invalid` when relevant counterevidence contradicts the proposition/scope. Use `stale` when support cannot currently justify the proposition but the proposition is not directly contradicted.

## 15.6 Replacement

If semantics genuinely change, prefer new identity plus `replaces`/transition relation rather than silently rewriting the old entity.

## 15.7 Merge/disagreement

Open Pincery is not currently described as a branching assurance-case repository, so full DAG-merge semantics are not required for initial runtime delivery. However, if two sources provide conflicting evidence, preserve both and surface the conflict; do not overwrite “older” counterevidence simply because a newer assertion is positive.

---

# 16. Authority Envelope Semantics

The authority envelope is the bridge from knowledge to effect.

## 16.1 Never infer authority from capability alone

Existing technical ability to call a tool is not proof the current situation justifies the requested scope.

## 16.2 Requested vs permitted scope

Every decision should conceptually compare:

```text
RequestedScope
PermittedScope
```

If equal and all requirements pass: `ADMIT`.

If permitted is a safe strict subset: `CONSTRAIN`.

If no effect but observation is allowed: `SHADOW`.

## 16.3 Evidence-sensitive authority

Configured policy may narrow authority when:

- a required claim becomes stale;
- environment leaves a validity domain;
- verifier changes;
- agent instructions/configuration change;
- recovery path becomes unavailable;
- requested effect becomes more irreversible;
- consequence class increases;
- current state is anomalous/unknown.

## 16.4 Progressive restoration

When new evidence restores the blocking claim, the next decision may broaden scope. Do not maintain a punitive permanent reduction after justification is restored unless policy independently requires it.

## 16.5 No unsupported fine-grained scope

If the existing capability model cannot reliably enforce “only delete files under X but not Y,” do not emit a misleading envelope. The envelope must map to actual enforceable capability shape.

---

# 17. Evidence Acquisition And Remediation

A key product requirement is to avoid becoming a denial engine operators disable.

## 17.1 Evidence action catalog

Each blocking claim class may reference zero or more known evidence actions.

Example:

```yaml
claim_type: clean-working-tree
options:
  - id: git-status-check
    command_kind: deterministic_runtime_check
    cost: low
    side_effect: none
    semantic_reach: detects tracked/untracked workspace changes
```

Do not execute arbitrary agent-authored shell strings from this catalog without passing through the ordinary capability/sandbox boundary.

## 17.2 Minimal path

V1 can rank using fixed policy order:

1. deterministic/no-side-effect check;
2. local test;
3. shadow/read-only observation;
4. reversible canary;
5. human review;
6. high-cost formal/field experiment.

Later versions may optimize cost/latency/risk more explicitly.

## 17.3 Evidence action itself requires authority

Running a production query or canary is itself an action. Avoid recursion bugs by defining base evidence capabilities and bounded recursion/decision rules.

## 17.4 Human-friendly explanation

Example:

```text
Full database migration not justified.

Why:
- CLAIM backup-restorable became stale 25h after the last restore drill.
- This migration is configured as irreversible after the old schema is dropped.

You can proceed by either:
1. Run `pcy ...` restore verification and retry, or
2. Request a 30-minute operator override, or
3. Constrain the action to schema-additive/shadow steps only.
```

---

# 18. Corrective Closure: Phased Implementation

Corrective closure is important but can become an overbuilt theoretical subsystem. Implement in phases.

## Phase CC-0 — Explicit recovery requirement

For a configured high-impact action, require a named recovery plan/claim such as `backup-restorable` or `rollback-binary-available`.

## Phase CC-1 — Structured corrective path

Represent path steps and current availability:

```text
observe
-> diagnose
-> repair
-> verify
-> authorize
-> execute
-> validate
```

## Phase CC-2 — Failure domains

Tag dependencies with failure domains and warn when two declared fallback paths share a critical dependency.

## Phase CC-3 — Time margin

For action classes with meaningful irreversibility horizons, compare declared/observed correction latency to the horizon.

Do not advertise mathematically complete system resilience unless the model actually supports it.

---

# 19. Between-Wakes Maintenance Integration

Between-wakes maintenance is a major strategic integration point.

## 19.1 Deterministic first

Before/around LLM maintenance, run deterministic assurance maintenance:

- apply expirations;
- process pending invalidation events;
- recompute dependency statuses;
- mark stale authority decisions;
- update cached projection/fingerprint;
- emit metrics.

## 19.2 LLM-assisted proposals

The maintenance LLM MAY propose:

- “identity instructions changed in a way that may affect repository-write policy”;
- new candidate claim/evidence links;
- possible next evidence action;
- summarized explanation.

These proposals MUST be clearly marked untrusted until validated by deterministic policy/operator action.

## 19.3 Identity/configuration changes

When maintenance changes durable identity/instructions, create a durable semantic change record if configured. Conservative v1 default can be:

```text
any durable behavior-instruction change -> implementation/model-like assurance change class
```

Do not automatically classify normal prose summary changes as deep governance changes.

## 19.4 Authority narrowing before next wake

If maintenance discovers deterministic expiry/invalidation, ensure the next wake cannot mint broad capability based on the old cached decision.

---

# 20. Capability Nonce Integration Details

This feature should reuse the strongest part of existing Open Pincery rather than bypass it.

## 20.1 Existing nonce properties to preserve

As current main describes, nonces are single-use, TTL-bounded, workspace-scoped, and capability-shape-bound. All of those remain mandatory.

## 20.2 New binding

Candidate additional nonce metadata or associated event reference:

```text
justification_decision_id
assurance_fingerprint
policy_version
```

Do not put sensitive verbose evidence in the nonce itself.

## 20.3 Revalidation strategy

The DESIGN phase must decide whether assurance is checked:

- only at nonce mint;
- at mint and consume;
- through decision fingerprint/version checked at consume.

For mutable critical assumptions, mint-only can create a TOCTOU window. Prefer a lightweight consume-time validation of decision freshness/version where practical.

## 20.4 Expiry interaction

Nonce TTL MUST NOT exceed the authority envelope/decision expiry.

```text
nonce.expires_at <= decision.expires_at
```

This should be a tested invariant.

## 20.5 Shadow mode

In shadow mode generate the same hypothetical decision but issue nonce exactly as pre-feature behavior would have, preserving current authorization semantics.

---

# 21. API Requirements

Follow existing auth/workspace ownership patterns.

Candidate endpoints, subject to current API conventions:

```text
GET  /agents/{id}/assurance
GET  /agents/{id}/assurance/claims
GET  /agents/{id}/assurance/claims/{claim_id}
GET  /agents/{id}/justification-decisions
GET  /justification-decisions/{decision_id}
GET  /assurance/disagreements?workspace=...
POST /justification-decisions/{decision_id}/override
POST /agents/{id}/assurance/rebuild-or-verify   # likely operator/admin only
```

Do not expose an API for arbitrary unauthenticated clients to assert high-trust evidence.

Evidence ingestion endpoints, if added, must authenticate producer/scope and distinguish assertion from verified observation.

OpenAPI tests/docs MUST remain consistent with project conventions.

---

# 22. CLI Requirements

The CLI should make the system operable without reading DB rows.

Suggested noun: `assurance` or `justify`. Pick based on current noun/verb convention and tests.

Minimum useful commands after first enforced slice:

```text
pcy assurance status <agent>
pcy assurance explain <decision-id>
pcy assurance disagreements [--json]
pcy assurance verify <agent> [--json]
```

Optional later:

```text
pcy assurance claim show ...
pcy assurance evidence show ...
pcy assurance override ...
pcy assurance recovery ...
```

Every CLI fenced example added to docs must correspond to a real clap path, consistent with existing onboarding documentation tests.

---

# 23. Observability And Metrics

Use existing observability infrastructure.

Metrics should include at least:

```text
cj_decisions_total{verdict,capability_class,mode}
cj_shadow_disagreements_total{existing_result,cj_verdict,reason}
cj_decision_latency_seconds
cj_stale_claims{reason}
cj_invalid_claims{reason}
cj_evidence_expirations_total
cj_overrides_total{capability_class}
cj_projection_rebuild_failures_total
cj_nonce_scope_constrained_total{reason}
cj_recovery_requirement_failures_total
```

Avoid high-cardinality labels with raw claim IDs/agent IDs if current telemetry conventions prohibit them.

Log/trace fields should include decision ID and event correlation IDs without secrets.

---

# 24. Threat Model Additions

Existing `docs/SECURITY.md` remains authoritative. Add Continuous Justification-specific threats.

## T-CJ-001 — Agent fabricates evidence

Mitigation: LLM statements are proposals; evidence producers have explicit kind/trust semantics; deterministic checks bind to actual artifacts/state.

## T-CJ-002 — Agent manipulates its own authority claim

Mitigation: authority policy/decision engine lives outside editable prompt/identity; capability nonce remains external enforcement.

## T-CJ-003 — Prompt injection asks agent to mark blocker resolved

Mitigation: free-form model output cannot directly flip projection status; restoring transitions require validated event types/evidence.

## T-CJ-004 — Replay old favorable decision

Mitigation: decision fingerprint/state version/expiry; nonce single-use; stale decision checks.

## T-CJ-005 — Evidence freshness race

Mitigation: transaction/version binding; consume-time freshness check for configured critical classes.

## T-CJ-006 — Semantic ID reuse

Mitigation: deterministic semantic fingerprints and drift detection.

## T-CJ-007 — Suppress counterevidence

Mitigation: append-only event history; projection rebuild; invalidation events cannot be deleted through normal API; audit-chain verification.

## T-CJ-008 — Malicious verifier update blesses action

Mitigation: verifier changes invalidate verifier-bound evidence; high-depth evaluator changes require external/independent authority and transition evidence.

## T-CJ-009 — Denial-of-service via endless stale claims

Mitigation: feature scoped to consequential classes; clear remediation; operator override; rate limits; bounded evidence planning; no LLM loop on hot path.

## T-CJ-010 — Evidence data leaks sensitive content

Mitigation: store digest/reference/structured result instead of raw secrets; reuse existing credential boundaries; redaction in explanations.

## T-CJ-011 — Attacker floods event log with assurance events

Mitigation: normal authentication/rate/budget controls; restrict which producers can assert which event types; no untrusted external event becomes high-trust evidence automatically.

## T-CJ-012 — Common-mode recovery illusion

Mitigation: failure-domain tags and operator explanation; do not claim two fallback paths are independent without modeled independence.

---

# 25. Privacy And Data Retention

Assurance data can include sensitive operational reasoning. Requirements:

- Do not persist raw secrets.
- Prefer references/digests to large tool output.
- If evidence contains user/customer data, follow existing event retention/security boundaries.
- Human override reasons may contain sensitive incident context; expose only to authorized operators.
- Determine whether evidence/event retention follows current event-log retention (likely durable) or needs redacted derivative storage.
- Projection deletion/rebuild must not bypass the audit/event source of truth.

---

# 26. Performance And Reliability

## 26.1 No LLM in synchronous enforcement path

Hard requirement for initial design.

## 26.2 Local/cached decision

Hot-path decision should use current projection, policy, and small deterministic checks.

## 26.3 Projection lag

Define maximum acceptable projection lag for enforced decisions. Prefer transactionally updated projection or decision-time catch-up for critical events rather than eventually consistent broad authority.

## 26.4 Database load

Index query paths for current claims/required obligations/decisions. Do not scan entire event history per tool call.

## 26.5 Failure isolation

Assurance subsystem failure should not take down health endpoints/onboarding/unenforced tool paths. Enforced capability class must fail according to explicit security policy.

---

# 27. Formal Model Requirements

The feature adds enough state-machine complexity that a formal model is required before broad enforcement.

Suggested variables:

```text
AssuranceVersion
ClaimStatus
DecisionVersion
DecisionVerdict
DecisionExpiry
NonceState
NonceScope
PolicyVersion
InvalidationPending
OverrideState
WakeLifecycle
```

Suggested actions:

```text
ObserveEvidence
InvalidateEvidence
RecomputeClaim
RequestAction
ComputeDecision
IssueNonce
ConsumeNonce
ExpireEvidence
ExpireDecision
ApplyOverride
ChangeVerifier
RecordWitness
MaintenanceStep
```

Safety properties:

1. `NonceScopeSubsetOfDecisionEnvelope`
2. `NoNonceFromExpiredDecision`
3. `NoAuthorityExpansionBeforeInvalidationApplied`
4. `ShadowDoesNotAffectExistingAuthorization`
5. `InvalidClaimCannotSupportKnowPass`
6. `OverrideDoesNotChangeClaimTruth`
7. `SingleUseNoncePreserved`
8. `WorkspaceScopePreserved`
9. `DecisionExpiryBoundsNonceExpiry`
10. `VerifierChangeInvalidatesBoundEvidenceUnlessWitnessed`

Liveness properties, bounded by existing runtime assumptions:

- valid new evidence can eventually restore an eligible stale claim;
- a blocked action with a completed required evidence event can eventually be reconsidered;
- maintenance does not permanently prevent wake progression absent an actual blocking lifecycle condition.

Model-check races around invalidation vs nonce mint/consume.

---

# 28. Test Strategy

The feature is not complete because unit tests pass. Use a layered verifier stack.

## 28.1 Unit tests

- status transition table;
- support-group evaluation;
- semantic fingerprint canonicalization;
- expiry behavior;
- transition-witness validation;
- requested/permitted scope intersection;
- verdict reason codes;
- evidence-option ranking;
- corrective-path availability.

## 28.2 Integration tests

- event insertion -> projection update;
- projection rebuild;
- capability request -> shadow decision event;
- enforce decision -> nonce issuance;
- stale evidence -> constrained/refused nonce;
- override -> scoped nonce without claim mutation;
- audit verify covers new events;
- maintenance expiry before next wake.

## 28.3 Known-bad contrast tests

For every high-value gate, construct a known-bad state that MUST fail.

Examples:

- same claim ID with changed statement;
- expired backup evidence;
- verifier identity changed;
- counterexample event followed by context reset;
- decision from old assurance fingerprint replayed;
- nonce request scope widened after decision;
- two “recovery paths” sharing same unavailable dependency.

## 28.4 Property tests

Properties worth fuzz/property coverage:

- applying invalidation never expands authority;
- permitted scope never exceeds requested scope or policy maximum;
- nonce expiry never exceeds decision expiry;
- projection rebuild is idempotent/equivalent;
- ordering-independent handling for commutative independent support events where designed.

## 28.5 Mutation tests where practical

Mutate decision branch conditions to verify tests fail if:

- invalid is treated as preserved;
- shadow result accidentally enforces;
- nonce scope check is inverted;
- expiry comparison is removed.

## 28.6 Security tests

- prompt injection cannot directly assert privileged evidence;
- unauthorized API cannot add override;
- evidence explanation redacts secret-like fields;
- stale decision replay rejected;
- event-chain tamper detected with CJ events.

## 28.7 Performance tests

Benchmark:

- current capability gate baseline;
- shadow CJ local decision;
- enforce CJ decision;
- projection rebuild on representative event history.

## 28.8 End-to-end dogfood scenario

A codebase-steward agent should exercise:

1. request a consequential repository mutation;
2. pass with fresh evidence;
3. evidence expires or counterexample appears;
4. full authority becomes constrained;
5. agent runs permitted evidence check;
6. evidence restores claim;
7. full authority returns;
8. audit explains all transitions.

---

# 29. Rollout Plan

## Milestone M0 — Reconciliation and design proof

No feature code beyond scaffolding.

Deliver:

- current architecture map;
- threat-model delta;
- DB/event design;
- TLA+ state-machine draft;
- selected first capability class;
- baseline performance measurement;
- explicit list of non-goals/deferred theory.

Gate: design can be implemented without weakening nonce/event/security invariants.

## Milestone M1 — Assurance ledger + projection, no decision effect

Deliver:

- event vocabulary;
- projection schema/engine;
- manual/deterministic evidence events;
- query API/CLI minimal status;
- rebuild test.

Gate: projection deterministic; event chain passes; existing runtime unchanged.

## Milestone M2 — Shadow decisions

Deliver:

- ActionIntent;
- five-part judgment;
- verdict/explanation;
- shadow integration with one selected capability class;
- disagreement metrics/export.

Gate: zero authorization behavior change; measurable overhead acceptable.

## Milestone M3 — Evidence expiry and counterevidence continuity

Deliver:

- TTL/expiry;
- invalidation propagation;
- no-resurrection regression tests;
- between-wakes deterministic maintenance.

Gate: stale support demonstrably changes shadow decision; fresh evidence restores correctly.

## Milestone M4 — Constrained authority + first enforcement class

Deliver:

- authority envelope mapped to real capability shape;
- decision-to-nonce binding;
- TOCTOU/freshness protection;
- emergency override;
- formal spec coverage.

Gate: shadow evaluation report meets agreed quality threshold; only one narrow class enters enforce mode.

## Milestone M5 — Corrective path requirement

Deliver:

- structured recovery declaration;
- availability check;
- failure-domain tags;
- explanation.

Gate: known-bad “last recovery path lost” scenario is blocked/constrained.

## Milestone M6 — Evidence planning UX

Deliver:

- evidence action catalog;
- minimum/ranked remediation path;
- ability for agent to request permitted evidence action and retry.

Gate: majority of dogfood `REQUIRE_EVIDENCE` decisions have actionable, successful remediation without manual database edits.

## Milestone M7 — Broader progressive autonomy

Only after evaluation:

- more capability classes;
- runtime assumption monitors;
- self-configuration semantic change integration;
- optional external evidence adapters.

---

# 30. First Enforcement Candidate Selection

Do not begin with the most dangerous arbitrary shell action.

Choose a capability class with:

- clear action structure;
- known scope dimensions;
- objective evidence preconditions;
- reliable recovery semantics;
- low operator ambiguity;
- enough frequency to gather shadow data.

Candidate examples to evaluate against current runtime:

1. workspace/repository write operation with clean-working-tree and protected-path claims;
2. credential/provider mutation with explicit ownership/freshness claims;
3. webhook rotation or other existing bounded admin action;
4. destructive file operation inside workspace if current tool schema exposes enough structure.

Avoid network-wide arbitrary shell or production database actions until scope representation is strong.

---

# 31. Example Scenario: Codebase Steward

Open Pincery's strategic direction names codebase stewardship as an important mission class. Continuous Justification can make that mission materially safer/useful.

## Baseline

Agent wants to modify dependency versions and push a branch.

Required claims:

```text
C1 repository checkout corresponds to intended repo/workspace
C2 working tree has no unpreserved user edits
C3 requested dependency change passes configured tests
C4 push target is an allowed non-protected branch or has required approval
C5 rollback/revert path exists
```

## First action

Fresh checks support C1-C5. Decision `ADMIT` within workspace, branch, TTL.

## Later wake

A user manually edits files. Runtime observation invalidates C2.

Agent asks for same write/push capability.

Decision:

```text
CONSTRAIN or REQUIRE_EVIDENCE
full write/push not justified
read-only/git status allowed
```

Suggested remediation:

- inspect diff;
- ask operator whether edits may be incorporated;
- stash/commit only under explicit user authority.

The agent does not lose identity; it loses only unsupported authority.

---

# 32. Example Scenario: Verifier Change

A maintainer upgrades the tool/check that previously supported `claim:prompt-tool-schema-safe`.

The evidence objects from old verifier remain in history.

Continuous Justification observes semantic layer `verifier` changed.

Policy says this claim is verifier-sensitive.

Result:

```text
claim -> STALE
broad high-impact tool authority -> narrowed
```

Restoration requires:

- new verifier-bound regression/known-bad evidence; or
- explicit transition witness establishing equivalent semantics.

This prevents “old green checkmark, new checker meaning” confusion.

---

# 33. Example Scenario: Prior Counterexample Survives Context Loss

Wake 10 discovers a destructive migration corrupts a fixture. Event:

```text
E counterexample observed
C migration-safe -> INVALID
```

The model context is later compacted and does not contain the narrative.

At wake 25 the agent proposes the migration again.

The assurance projection, not prompt memory, still contains the invalidation. Decision cannot `ADMIT` until new evidence resolves it.

This is a key product behavior and should have an explicit end-to-end test.

---

# 34. Example Scenario: Constrained Experiment Generates Evidence

Agent wants to switch all workspaces to a new provider/tool policy.

Current evidence supports compatibility only on one test workspace.

Decision:

```text
CONSTRAIN
scope = test workspace only
TTL = short
mandatory observation = enabled
```

Outcome provides evidence. If success meets configured discriminator, follow-up decision may expand to a small canary, then broader scope.

This is progressive autonomy rather than binary permission.

---

# 35. External Integration Strategy

The core feature should be useful without external standards, but design adapters around existing artifacts rather than inventing everything.

Potential later adapters:

## SLSA / in-toto / Sigstore

Use for artifact provenance/signature facts. Do not interpret provenance as behavioral correctness.

## TLA+ / Apalache

Model-check result can support/contradict a formal claim within the model scope. Counterexample should become explicit counterevidence.

## OpenTelemetry

Runtime observation can refresh/violate assumptions.

## W3C PROV-like concepts

Can inform provenance relationships, but do not require a heavyweight generic provenance DB for v1.

## OPA/Cedar/external policy

If Open Pincery later delegates policy, Continuous Justification can provide structured facts and consume policy decision. Do not duplicate policy ecosystems prematurely.

---

# 36. Compatibility With `swe-dev` Justified Change

A companion `swe-dev` skill may produce a human/agent-readable Justified Change Packet containing:

- changed semantic layers;
- critical claims;
- evidence applicability;
- counterevidence;
- action judgment;
- corrective path;
- minimum evidence plan;
- transition witnesses.

Open Pincery MUST NOT require that plugin at runtime. The useful composition is:

```text
swe-dev reasoning -> structured proposal/contract -> Open Pincery durable assurance events -> runtime decision/enforcement
```

The runtime validates and owns authority. The reasoning plugin improves authoring/explanation.

---

# 37. Compatibility With Lights Out SWE

Lights Out SWE should use this PRD as input, but Continuous Justification is a product feature, not a permanent runtime dependency on the build harness.

During BUILD:

- `scaffolding/` records decisions and evidence;
- each `AC-CJ-*` maps to named proof/test/runtime evidence;
- formal state-machine change is created before enforcement code if required by current harness convention;
- REVIEW explicitly hunts for ways the feature could weaken existing capability/audit/security controls;
- RECONCILE confirms docs and code match;
- VERIFY runs all new and existing relevant tests;
- DEPLOY/dogfood starts shadow-only.

The finished Open Pincery feature must stand alone after the harness is removed.

---

# 38. Open Questions With Recommended Defaults

Fresh builders should resolve these during DESIGN rather than silently choosing.

## Q1. Scope: per-agent, per-workspace, or both?

**Recommended default:** evidence/claims can be workspace-scoped with optional agent-scoped entities; decisions always bind the acting agent and workspace. This avoids duplicating shared facts for every agent while preserving individual authority.

## Q2. Projection storage shape?

**Recommended default:** normalized current entity + relation projection tables, with authoritative events in existing event log. Avoid a single giant JSON blob if it prevents indexed dependency queries.

## Q3. How much support logic in v1?

**Recommended default:** explicit support groups (`all_of` / `any_of`) sufficient for known cases; do not build a complete generic theorem/truth-maintenance engine.

## Q4. Who may assert evidence?

**Recommended default:** producer classes with policy: deterministic runtime, trusted system check, operator, agent proposal. Agent proposal cannot directly satisfy high-consequence KNOW requirement without validation.

## Q5. How are semantic changes detected?

**Recommended default:** explicit system-generated events for known changes (policy/verifier/config/version), plus conservative configured classification for durable identity changes. Automated semantic diff can come later.

## Q6. Do decisions live as rows or events?

**Recommended default:** authoritative decision event + optimized projection/index if needed.

## Q7. Do we re-check decision on nonce consume?

**Recommended default:** for critical enforced classes, check decision expiry/state version at consume; exact mechanism depends on existing nonce transaction architecture.

## Q8. How should human override work?

**Recommended default:** separate authority event with scope/TTL/reason; never mutate claim truth.

## Q9. Is `RECOVER` always required?

**Recommended default:** no. Policy maps action consequence/reversibility classes to required recovery evidence. Routine reversible actions can skip complex recovery analysis.

## Q10. What if evidence planning has no known option?

**Recommended default:** return explicit blocker and escalate; do not hallucinate a verifier.

---

# 39. Deferred Advanced Research Features

These ideas motivated the architecture but SHOULD NOT block useful product delivery.

1. automatic causal discovery of assurance dependencies;
2. full graph min-cut calculation of corrective distance;
3. dynamic generativity/depth-gap metrics;
4. automatic ontology migration/proof transport;
5. formal epistemic type system across all tool I/O;
6. multi-agent structured disagreement/knowledge CRDT;
7. automated scientific experiment design;
8. self-evolving verifier kernel;
9. global assurance “capital” score;
10. universal safe-open-endedness guarantees.

Revisit only when simpler features produce demonstrated user value.

---

# 40. Novelty And Positioning Guardrail

Do not claim the following as new categories:

- agent policy enforcement;
- exact-action authorization;
- assurance cases;
- assurance lineage;
- event sourcing;
- provenance;
- runtime assurance;
- formal verification;
- dynamic assurance;
- progressive deployment/canaries;
- self-evolving agents;
- audit trails;
- data/change lineage.

The product hypothesis to test is the integration:

```text
continuous identity + durable events
  -> semantic assurance applicability under change
  -> counterevidence-preserving continuity
  -> evidence-aware authority envelope
  -> capability enforcement
  -> outcome evidence
  -> next justified action
```

The useful differentiator is not the terminology; it is whether this catches real situations where existing capability/policy/audit controls would allow an action even though the reasons previously supporting that authority no longer apply, and whether it can offer a low-friction path to restore justified authority.

---

# 41. Success Metrics

Do not judge success by number of claims/events generated.

Primary product metrics after dogfood:

1. **Useful disagreement rate:** fraction of `existing gate allows / CJ narrows-or-blocks` cases judged by operator as a real continuity/evidence/recovery gap.
2. **False-block burden:** fraction of enforced constraints judged unnecessary after review.
3. **Remediation success:** fraction of `REQUIRE_EVIDENCE` cases resolved through suggested evidence path without manual state hacking.
4. **Time to explanation:** operator can identify why action was constrained within a target time (e.g. <2 minutes in usability test).
5. **Decision latency overhead:** measured p50/p95 in shadow/enforce.
6. **Counterevidence survival:** regression scenarios never silently regain authority after context/restart/projection rebuild.
7. **Audit reconstructibility:** sampled actions can reconstruct decision state/authority/evidence from durable records.
8. **Progressive-autonomy value:** cases where `CONSTRAIN` enabled useful safe progress that binary policy would have blocked.
9. **Operator override frequency:** high rates may indicate poor evidence models/policy and should trigger redesign.
10. **Maintenance cost:** configuration burden per new capability class remains acceptable.

---

# 42. Kill / Reframe Criteria

This feature should be weakened, reframed, or stopped if dogfood shows:

- almost every decision exactly duplicates existing capability policy without additional actionable information;
- useful decisions require hindsight-only manual annotations that cannot be maintained in normal operation;
- false blocks are high enough that operators routinely bypass the system;
- evidence maintenance costs more than the risk/useful autonomy it enables;
- the projection becomes an opaque second source of truth that drifts from events;
- runtime latency is unacceptable and cannot be moved off hot path;
- semantic applicability rules are too domain-specific to support even a few high-value mission classes;
- operators cannot understand or trust explanations;
- the feature weakens current nonce/sandbox/audit security architecture.

A successful research result can be “this belongs only in a narrower set of mission classes.” Do not force universality.

---

# 43. Recommended Build Order By Files/Subsystems

Fresh agent MUST update this list after inspecting current main.

1. **Domain + events**
   - new assurance domain module;
   - extend/version event payload handling;
   - migration/projection structures.
2. **Projection tests**
   - deterministic replay;
   - expiry/invalidation/no-resurrection.
3. **Read-only API/CLI**
   - inspect assurance state before any enforcement.
4. **ActionIntent + shadow decision**
   - hook one capability request;
   - record disagreement only.
5. **Observability/export**
   - metrics and operator explanations.
6. **Formal model**
   - decision/nonce/invalidation race semantics.
7. **Enforcement integration**
   - narrow first capability class;
   - decision fingerprint/nonce binding.
8. **Override**
   - explicit audited operator path.
9. **Corrective path**
   - structured declarations for selected class.
10. **Evidence remediation**
   - evidence action catalog and retry UX.

---

# 44. Definition Of Done For First Useful Release

A first useful Continuous Justification release is done when all of the following are true:

1. Existing Open Pincery installation behavior remains intact with feature off.
2. Shadow mode can run in real dogfood without changing action authorization.
3. A durable critical claim can be supported by deterministic evidence.
4. The evidence can expire/be invalidated and the claim becomes stale.
5. The stale claim changes the shadow verdict for a configured consequential capability.
6. A prior explicit counterexample survives process restart/wake/context reset/projection rebuild.
7. The decision explanation names the exact blocker and a concrete evidence action.
8. Fresh evidence restores the claim and changes the subsequent verdict.
9. All transitions are covered by the existing event hash chain/audit verification.
10. Projection rebuild reproduces the same semantic assurance state.
11. Relevant unit/integration/security/property tests pass.
12. Formal model covers the planned enforcement race/invariants even if enforcement remains shadow in this release.
13. Dogfood report quantifies disagreement usefulness, latency, and false-block burden.
14. README/security/docs make no claim stronger than shipped behavior.

Enforcement can ship as a later acceptance milestone after shadow evidence justifies it.

---

# 45. Required Autonomous Build Artifacts

When using Lights Out SWE, require the following in scaffolding before BUILD advances:

## EXPAND

- stable mapping of every `AC-CJ-*` to scope or explicit deferral;
- chosen first capability class;
- explicit non-goals;
- feature mode defaults;
- current repo/version reconciliation notes.

## DESIGN

- directory/module plan grounded in current source;
- event schemas;
- DB migration plan;
- projection ownership/rebuild semantics;
- trust boundary diagram;
- nonce integration sequence;
- decision freshness/TOCTOU handling;
- override design;
- API/CLI deltas;
- formal model delta.

## ANALYZE

For every included AC:

- named test/proof/runtime evidence;
- known-bad contrast where material;
- security impact;
- migration/compatibility impact;
- rollback path;
- build order/dependencies.

No BUILD if a high-impact AC has only “manual verification” when an automated/discriminating proof path is feasible.

## REVIEW

Review must specifically attack:

- self-approval paths;
- stale-decision replay;
- nonce-scope widening;
- shadow accidentally enforcing;
- event/projection divergence;
- counterevidence loss;
- semantic drift under stable IDs;
- secret leakage in evidence/explanation;
- performance regressions;
- bypass through uninstrumented capability paths.

## VERIFY

Map each shipped `AC-CJ-*` to an actual passing artifact. A passing test suite without AC traceability is insufficient under this build harness.

---

# 46. Product Narrative For Future README (Do Not Ship Until True)

This section is copy direction, not an instruction to claim unshipped behavior.

> Open Pincery agents are continuous: they keep the same identity and history between wakes. Continuous Justification extends that continuity to authority. The runtime remembers which evidence and assumptions justified consequential actions, notices when those reasons expire or are contradicted, and can narrow an agent's capability until the missing evidence is restored. Every decision remains auditable in the same event history as the action itself.

Possible one-line differentiator:

> **Continuous agents with continuous justification.**

Do not use “provably safe autonomous agents.”

---

# 47. Final Product Principle

The deepest design rule for this feature is simple:

> **Do not grant a continuous agent more causal authority than its current evidence, legitimate policy, and corrective capacity justify. When justification weakens, narrow the action—not the historical truth. When evidence returns, authority may expand again.**

The system should make uncertainty operational rather than rhetorical.

A durable agent should be able to say, in machine-enforced terms:

```text
I can technically do this.
I was once allowed to do this.
But the evidence that justified that authority is now stale.
I am therefore taking the smaller safe action that remains justified,
and here is the exact observation or approval needed to proceed further.
```

That is the target product behavior.

---


# 48. Synthetic Organization Program North Star

## 48.1 Working definition

For this program, a **synthetic organization** is a persistent, inspectable, governable system of human and machine participants that:

- owns one or more durable missions rather than isolated prompts;
- represents work as explicit obligations with completion evidence;
- creates, assigns, retires, and restructures roles;
- delegates authority with scope, expiry, provenance, and revocation;
- allocates scarce resources and budgets;
- preserves evidence, assumptions, counterevidence, decisions, and rejected alternatives;
- coordinates multiple continuous agents without requiring a shared transcript;
- distinguishes proposal, verification, authorization, execution, audit, and amendment responsibilities;
- can narrow authority or reorganize when its models become stale;
- retains enough corrective capacity to discover and recover from consequential error;
- can learn across missions without silently converting historical inference into permanent truth;
- can evolve its own operating structure through explicit, auditable transitions.

A synthetic organization MAY have one human owner. It MAY operate a one-person business. It MAY eventually coordinate hundreds of agents. Its defining property is not participant count; it is **durable institutional structure around action**.

The term is intentionally product-oriented. It does not imply legal personhood, moral agency, consciousness, or that human organizations are reducible to software.

## 48.2 Product ambition: an engine for getting consequential work done

The practical ambition is a general mission loop:

```text
INTENT
  -> FRAME
  -> INVESTIGATE
  -> CONSTITUTE
  -> PLAN
  -> STAFF / DELEGATE
  -> EXECUTE
  -> VERIFY
  -> DELIVER
  -> OBSERVE OUTCOMES
  -> AUDIT
  -> LEARN
  -> REORGANIZE
  -> REPEAT
```

"Anything" in the product aspiration means **any mission whose required actions can be represented through available human/tool/service interfaces and whose governing constraints can be expressed sufficiently for safe execution**. It does not mean arbitrary physical capability or perfect autonomous operation.

The first high-value domain remains software engineering because Open Pincery can compose with `lights-out-swe`, `lights-out-swe-plugin`, `swe-dev`, GitHub, CI, formal methods, and ordinary shell tools. The architecture MUST NOT, however, encode "software project" as the only kind of mission.

## 48.3 Durable organization state

[SYNTHESIS] A useful conceptual state for the organization is:

\[
\mathcal O_t =
(A_t, W_t, K_t, M_t, E_t, G_t, B_t, C_t, V_t, H_t)
\]

where:

- \(A_t\) — agents, humans, roles, identities, and reporting/delegation relationships;
- \(W_t\) — missions, obligations, work items, dependencies, deadlines, and completion state;
- \(K_t\) — executable capabilities and constructors: tools, skills, procedures, pipelines, external services;
- \(M_t\) — models of the environment, organization, customers, systems, and causal assumptions;
- \(E_t\) — evidence, provenance, observations, claims, counterevidence, and epistemic status;
- \(G_t\) — governance: policies, delegations, authorities, approval rules, and amendment procedures;
- \(B_t\) — budgets and scarce resources: money, compute, tokens, time, rate limits, human attention;
- \(C_t\) — corrective capacity: observation, diagnosis, rollback, repair, escalation, alternative suppliers/agents;
- \(V_t\) — mission objectives, values, non-goals, risk tolerances, and constraints;
- \(H_t\) — durable history and lineage sufficient to reconstruct why current state exists.

The organization evolves as:

\[
\mathcal O_{t+1}
= \Gamma(\mathcal O_t, Obs_t, Decisions_t, Actions_t, Outcomes_t)
\]

The implementation MUST NOT attempt to literally store this tuple as one object. It is a model for architecture and tests. Open Pincery's event log remains the source of truth, with projections over the relevant dimensions.

## 48.4 Why Open Pincery is a plausible bootstrap substrate

Current Open Pincery already provides several unusually aligned primitives:

- stable continuous-agent identity;
- append-only, hash-chained event history;
- exactly-one-active-wake lifecycle semantics;
- asynchronous agent messaging;
- between-wakes maintenance;
- sandboxed shell execution;
- scoped, single-use, TTL-bound capability nonces;
- credential isolation;
- PostgreSQL as one durable source of truth;
- TLA+ coverage discipline for critical state transitions.

The synthetic-organization program should **compose** these primitives rather than replacing them. The organization is a higher-order projection and governance layer over durable agents and events.

Current repository source of truth to re-check before each build phase:

- https://github.com/RCSnyder/open-pincery
- https://github.com/RCSnyder/open-pincery/blob/main/docs/input/north-star-2026-04.md

## 48.5 What makes this different from an agent swarm

A swarm can parallelize tasks. A synthetic organization additionally requires:

1. durable mission ownership;
2. explicit role/authority semantics;
3. resource constraints;
4. institutional memory and counterevidence preservation;
5. provenance of decisions;
6. independent verification paths;
7. conflict/disagreement representation;
8. correction and recovery structure;
9. governed organizational change;
10. continuity across replacement of individual models/agents/providers.

If deleting all current LLM sessions destroys the organization, the organization is not yet durable enough.

---

# 49. Synthetic Organization Architecture

## 49.1 Layer model

Implement synthetic-organization capabilities as composable layers rather than one orchestrator class:

```text
Human / external mission authority
            |
            v
+------------------------------+
| Mission + Constitution Layer |
+------------------------------+
            |
            v
+------------------------------+
| Portfolio / Work Layer       |
+------------------------------+
            |
            v
+------------------------------+
| Roles + Delegation Layer     |
+------------------------------+
            |
            v
+------------------------------+
| Evidence + Knowledge Layer   |
+------------------------------+
            |
            v
+------------------------------+
| Justification / Governance   |
+------------------------------+
            |
            v
+------------------------------+
| Resource / Budget Layer      |
+------------------------------+
            |
            v
+------------------------------+
| Agent Runtime / Capability   |  <-- existing Open Pincery core
+------------------------------+
            |
            v
+------------------------------+
| World / Tools / Services     |
+------------------------------+
            |
            v
      observations / outcomes
            |
            +-----------------------> evidence + audit + learning
```

The existing runtime is the execution substrate. New organization layers supply durable **reasons, obligations, and constraints** around execution.

## 49.2 Mission contract

A mission MUST eventually have a durable, versioned contract independent of any one prompt. Candidate fields:

```text
mission_id
version
sponsor / root authority
intent
outcomes
non_goals
constraints
stakeholders
risk_class
budget_envelope
deadlines / review horizons
evidence_policy
approval_policy
completion_criteria
stop_conditions
recovery_expectations
supersedes / derived_from
```

A mission contract is not immutable forever. Changes to intent, non-goals, risk, or completion criteria are **deep changes** and require explicit lineage events.

## 49.3 Work contract

Work items SHOULD become stronger than prose to-do entries. A work contract should be able to express:

```text
work_id
mission_id
objective
owner_role / owner_agent
inputs
outputs
preconditions
constraints
dependencies
budget
allowed capabilities
required evidence
reviewer / verifier
completion test
failure / escalation rule
status
```

The work graph SHOULD remain compatible with the current work-list projection during incremental adoption.

## 49.4 Roles and accountable identity

Roles are durable organizational functions distinct from individual agents. Examples:

- mission owner;
- researcher;
- planner;
- builder;
- verifier;
- release authority;
- operator;
- auditor;
- incident commander;
- constitutional/amendment authority.

An agent MAY occupy multiple roles in low-risk deployments. High-consequence policies MAY require separation.

A role assignment MUST be revocable and time/scope bounded where appropriate.

## 49.5 Delegation graph

Authority is represented as a graph, not a Boolean `is_admin`:

```text
root human authority
   -> mission authority
      -> role authority
         -> agent authority envelope
            -> single-use capability nonce
```

Every non-root delegation SHOULD answer:

- who delegated it?;
- under which mission/policy?;
- what scope?;
- what budget?;
- what expiry/revocation condition?;
- what evidence/assurance state was required?;
- may the recipient redelegate?;
- what audit record must result from use?.

## 49.6 Resource and budget layer

A synthetic organization that lacks resource accounting is only a workflow engine. Represent scarce resources explicitly:

- money;
- model/API spend;
- compute;
- token budgets;
- wall-clock deadlines;
- external-service quotas;
- human-review budget;
- concurrency slots;
- sandbox/storage/network quotas.

Delegated authority MUST NOT imply unlimited resource authority.

## 49.7 Evidence/knowledge layer

The organization MUST distinguish at least:

```text
Observed
Derived
FormallyVerified
EmpiricallyValidated
Assumed
Contested
Stale
Invalid
Unknown
```

Do not force these into one scalar confidence score.

Long term, claims SHOULD retain:

- supporting evidence;
- counterevidence;
- assumptions;
- model/ontology dependence;
- provenance;
- validity domain;
- expiry/freshness;
- rejected alternatives and reasons.

## 49.8 Decision records

Consequential organizational decisions SHOULD be reconstructible as:

```text
proposal
available alternatives
material evidence
material counterevidence
assumptions
policy / authority basis
decision maker(s)
verifier / reviewer where required
decision
scope
expiry / review trigger
expected outcome
actual outcome
```

This is stronger than a chat transcript and should survive model/provider replacement.

## 49.9 Organization topology

The organization itself is part of the designed system. Record enough structure to inspect:

- roles;
- delegation edges;
- communication edges;
- critical dependency edges;
- shared model/provider/dataset dependencies;
- corrective-path dependencies;
- mission ownership.

This supports Conway-style analysis: organizational communication topology can shape the systems it builds, so organization design should be an explicit object rather than accidental prompt routing.

---

# 50. Organizational Operating Loops

## 50.1 Mission loop

The high-level mission loop is:

```text
Frame -> Survey -> Model -> Plan -> Constitute -> Execute -> Verify -> Deliver -> Observe -> Learn
```

For uncertain missions, planning MUST be revisable rather than a one-time frozen decomposition.

## 50.2 Epistemic/scientific loop

When the organization does not know enough to act:

```text
Observation
 -> Question
 -> Hypothesis / competing models
 -> Safe experiment / information acquisition
 -> Evidence
 -> Model update
 -> Decision
```

The system SHOULD represent `INVESTIGATE`, `ASK`, `SIMULATE`, `SHADOW`, and `DEFER` as legitimate outcomes rather than forcing an action.

## 50.3 Corrective loop

For consequential failures:

```text
Detect
 -> Represent
 -> Diagnose
 -> Generate alternatives
 -> Verify candidate correction
 -> Authorize
 -> Intervene
 -> Validate
 -> Learn
```

The organization is not considered correctable if this chain depends on one unmodeled common-cause component for all paths.

## 50.4 Organizational evolution loop

Synthetic organizations eventually need to alter themselves:

```text
Observe recurring organizational failure
 -> propose role/process/policy change
 -> classify intervention depth
 -> evaluate affected assurances and delegations
 -> test in shadow / limited scope
 -> authorize amendment
 -> migrate durable state
 -> validate successor
 -> retain rollback or forward-recovery path
```

No agent should be able to unilaterally rewrite the rules that define its own authority for high-impact operations.

## 50.5 Multi-timescale control

[SYNTHESIS] The organization should explicitly recognize nested timescales:

```text
milliseconds-seconds  capability checks, nonce use, sandboxing
seconds-minutes       wake reasoning, individual tool actions
hours-days            work-item execution, evidence acquisition, review
weeks                  mission planning, staffing, role adaptation
months                 governance changes, strategy, provider/model migrations
long horizon           institutional learning, constitution, regenerative capability
```

Slower loops set constraints for faster loops. Faster loops provide evidence upward. A slower governance loop that cannot respond before a fast catastrophic process reaches irreversibility is insufficient regardless of theoretical correctness.

---

# 51. Constitutional Computation And Separation Of Powers

## 51.1 Why a constitution exists

A general mission engine must distinguish:

- **doing work**;
- **deciding whether work is justified**;
- **changing the rules by which justification is decided**.

If one autonomous component can propose a claim, fabricate its own evidence, approve the claim, mint its own authority, execute the action, and declare success, the architecture has no meaningful institutional separation.

## 51.2 Functional powers

The system SHOULD model these as separable functions even when one actor temporarily fills multiple low-risk roles:

1. **Proposal** — generate candidate plans/actions.
2. **Evidence production** — measure or derive relevant facts.
3. **Verification** — test claims/actions against requirements.
4. **Authorization** — decide whether the actor may proceed.
5. **Execution** — perform the side effect.
6. **Audit** — reconstruct what happened and challenge claims.
7. **Amendment** — alter rules, role topology, or the assurance machinery.

## 51.3 Constitutional root

The initial bootstrap MUST retain an explicit human/operator root of authority. Synthetic organizational autonomy is delegated, not metaphysically self-created.

The root defines at minimum:

- who may create missions;
- budget ceilings;
- prohibited capability classes;
- which changes require human review;
- emergency-stop/revocation semantics;
- who may amend these rules.

The system MAY later support multi-human governance, but do not bury the root in implicit application configuration.

## 51.4 Amendment depth

[SYNTHESIS] Treat change depth roughly as:

```text
D0 runtime value / ordinary action
D1 implementation / work procedure
D2 architecture / workflow generator
D3 world or organizational model
D4 specification / mission criteria
D5 verifier / evidence acceptance rules
D6 governance / authority allocation
D7 constitutional amendment rules
```

The exact depth ontology may evolve. The invariant is that deeper change requires deeper independent assurance.

---

# 52. Mathematical Models And Design Inspiration

**Important:** Unless a subsection is explicitly attributed to a source, equations in this section are **[SYNTHESIS]**: proposed conceptual models for design and research, not established scientific laws and not production formulas to implement literally.

## 52.1 Counterfactual capability

Constructor theory motivates describing systems partly by possible and impossible transformations rather than only present state. As an engineering abstraction:

\[
\mathcal P=(X,\mathcal T^+,\mathcal T^-)
\]

where \(X\) is a modeled state space, \(\mathcal T^+\) are currently constructible transformations, and \(\mathcal T^-\) are forbidden/impossible transformations under the relevant model.

**Open Pincery inspiration:** a capability/role/tool changes the set of transitions an organization can cause. Governance changes which physically/computationally possible transitions are authorized.

Do **not** claim Open Pincery implements constructor theory.

## 52.2 Constructor hierarchy and generativity

[SYNTHESIS]

An ordinary action changes an artifact. A generative action changes a mechanism that creates future artifacts/actions.

\[
Generativity(a) \approx
\left|\text{future transition structures materially altered by }a\right|
\]

Examples of increasing generativity:

```text
edit one record
-> edit application code
-> edit code generator
-> edit build/deployment pipeline
-> edit verifier
-> edit policy that chooses verifiers
-> edit amendment procedure
```

**Product implication:** authority decisions SHOULD become stricter as generativity rises.

## 52.3 Closure of constraints -> organizational maintenance closure

Montévil and Mossio formalize biological organization in terms of constraints that participate in maintaining other constraints. The organizational analogy is [SYNTHESIS]: critical capabilities form a maintenance graph.

\[
K_i \xrightarrow{maintains} K_j
\]

A synthetic organization is stronger when observation, verification, repair, identity, credential, and deployment capabilities can be regenerated through multiple independent paths.

## 52.4 Corrective-closure graph

[SYNTHESIS]

Represent corrective capability as a typed directed hypergraph:

\[
\mathcal C=(V,H,\tau,\chi)
\]

where:

- \(V\) are capabilities/resources;
- \(H\) are AND/OR dependency hyperedges;
- \(\tau\) is correction latency;
- \(\chi\) identifies common-cause failure domains.

A valid correction path may require:

\[
Observe \to Diagnose \to Synthesize \to Verify \to Authorize \to Act \to Validate
\]

Define corrective distance:

\[
d_C=\min\{|F|:\mathcal C-F\text{ loses corrective closure}\}
\]

and closure reserve under modeled fault budget \(k\):

\[
\kappa_C=d_C-k.
\]

**Product implication:** do not count two paths as independent if both depend on the same model provider, root credential, database, human reviewer, verifier implementation, or network control plane.

## 52.5 Tempo / irreversibility

[SYNTHESIS]

\[
T_C=T_{detect}+T_{diagnose}+T_{plan}+T_{verify}+T_{authorize}+T_{act}
\]

\[
\rho_T=\frac{T_C}{T_{irreversible}}.
\]

Candidate safety condition:

\[
\rho_T<1.
\]

**Product implication:** an approval mechanism that takes two days cannot govern a failure that becomes irreversible in ten seconds. Use faster automatic constraints for faster hazards.

## 52.6 Assurance state

[SYNTHESIS]

\[
A_t=(I_t,M_t,\Sigma_t,E_t,P_t,V_t,G_t,R_t)
\]

where implementation, model, specification, evidence, proofs, verifier, governance, and recovery structure all evolve.

The important transition is:

\[
A_t\xrightarrow{\delta}A_{t+1}
\]

not merely code version \(I_t\to I_{t+1}\).

**Product implication:** changing a verifier or mission objective must invalidate different things than changing an implementation detail.

## 52.7 Assurance applicability / invalidation

[SYNTHESIS]

If claim \(c\) depends on assumption/evidence \(e\):

\[
e\leadsto c
\]

then invalidating \(e\) propagates to dependents unless an independent support path survives.

\[
Invalid(e) \land e\leadsto c \Rightarrow Status(c)\in\{Stale,Invalid\}
\]

**Product implication:** a continuous agent cannot regain authority because context compaction forgot the counterexample.

## 52.8 Semantic transport across ontology/model change

[SYNTHESIS]

When the representational language changes:

\[
F:L_t\rightarrow L_{t+1},
\]

critical old propositions need explicit transport or retirement:

\[
p\rightsquigarrow F(p).
\]

Where relevant, assurance should satisfy a commuting-style requirement:

```text
old implementation ----change----> new implementation
       |                              |
   old assurance                 new assurance
       |                              |
       v                              v
   old claims --------transport--> new claims
```

**Product implication:** same claim ID does not guarantee same semantics; migration needs witnesses.

## 52.9 Action judgment: CAN / KNOW / MAY / ADMISSIBLE / RECOVER

[SYNTHESIS]

A consequential action is eligible only when distinct modalities are satisfied:

\[
CAN(a)\land KNOW(a)\land MAY(a)\land ADMISSIBLE(a)\land RECOVER(a).
\]

- `CAN` — technically feasible;
- `KNOW` — relevant beliefs have sufficient evidence;
- `MAY` — delegated authority permits it;
- `ADMISSIBLE` — policy/safety/normative constraints permit the resulting transition;
- `RECOVER` — adequate corrective paths remain.

These MUST NOT be collapsed into one confidence score.

## 52.10 Authority envelope

[SYNTHESIS] Authority should vary with evidence and consequence rather than be permanently attached to an agent identity.

Conceptual relationship:

\[
AuthorityScope(a,t) \propto
\frac{EvidenceStrength\cdot Reversibility\cdot CorrectiveReserve}
{Uncertainty\cdot Impact\cdot InterventionDepth\cdot Generativity}.
\]

Do **not** implement this as a literal multiplication formula. Use it to identify decision inputs and monotonicity expectations.

Expected monotonicities:

- stronger relevant evidence should not reduce authority all else equal;
- greater irreversibility should not expand authority all else equal;
- deeper self-modification should not require less assurance;
- loss of corrective paths should not expand authority.

## 52.11 Matched-depth assurance

[SYNTHESIS]

\[
AssuranceDepth \ge InterventionDepth.
\]

If an agent changes the verifier while the assurance machinery only evaluates application behavior, the evaluator is conceptually too shallow.

## 52.12 Generative requisite variety

Ashby's requisite-variety idea motivates the regulator needing adequate response variety. For open-ended systems, [SYNTHESIS] proposes a generative extension:

\[
\Gamma_A \ge \Gamma_C
\]

where \(\Gamma_C\) is effective capability-generation rate/depth and \(\Gamma_A\) is assurance/governance's ability to create relevant new checks, models, evidence requirements, and constraints.

Define generativity gap:

\[
\Delta_\Gamma=\Gamma_C-\Gamma_A.
\]

Persistent positive gap is **assurance debt**.

## 52.13 Governability phase space

[SYNTHESIS]

\[
z=(\Delta_D,\Delta_\Gamma,\rho_T,\kappa_C)
\]

where:

\[
\Delta_D=CapabilityDepth-AssuranceDepth.
\]

Candidate governability envelope:

\[
\Delta_D\le0,\qquad
\Delta_\Gamma\le0,\qquad
\rho_T<1,\qquad
\kappa_C>0.
\]

This is a research model, not a guarantee. Open Pincery MAY collect proxies for these dimensions in shadow experiments.

## 52.14 Risk geometry

[SYNTHESIS]

A useful qualitative risk model is:

\[
Risk(a)\sim Impact\times Irreversibility\times Depth\times Generativity\times Uncertainty.
\]

The purpose is to prevent shallow measures such as "number of shell commands" or "lines changed" from being mistaken for consequence.

## 52.15 Viable empowerment / optionality

Empowerment literature measures an agent's potential control in information-theoretic terms. A proposed constrained extension is:

\[
VE_H(x)=\max_{\pi\in\Pi_{viable}}
I(A_{t:t+H};O_{t+H}\mid x).
\]

This **viable empowerment** equation is [SYNTHESIS], not the standard definition.

**Product inspiration:** preserve multiple useful, authorized, recoverable future options rather than greedily maximizing immediate task completion.

## 52.16 Regenerative kernel and regenerative complexity

[SYNTHESIS]

Let \(K_R\) be the smallest retained set from which critical correction/operation capabilities can be reconstructed.

\[
RC(K)=\text{minimum cost to reconstruct capability }K\text{ from }K_R.
\]

**Product implication:** the organization should detect when a critical capability still works today but has become unreconstructible because all expertise, credentials, suppliers, or specifications collapsed into one fragile dependency.

## 52.17 Maintenance burden

[SYNTHESIS]

Every durable constraint costs resources to maintain:

\[
\sum_i MaintenanceCost(C_i) \le AvailableResourceFlow.
\]

**Product implication:** roles, policies, compatibility promises, monitors, workflows, and approval paths are not free. The organization needs deletion/retirement mechanisms, not only accumulation.

## 52.18 Error hierarchy

[SYNTHESIS] Distinguish failure depth:

```text
measurement error
parameter error
model error
causal error
boundary error
scale error
ontology error
governance error
value/mission error
```

The remediation differs. Retraining parameters cannot repair an invalid mission objective.

## 52.19 Structured disagreement

[SYNTHESIS]

Do not model organizational knowledge as one majority belief. Preserve records of:

\[
Belief=(Claim,Status,Evidence,Model,Assumptions,Owner).
\]

Independent disagreement is itself evidence that may require investigation.

**Product implication:** merge and synthesis operations must not silently discard minority counterevidence or incompatible assumptions.

## 52.20 Best-next-question / evidence planning

[SYNTHESIS]

When action is blocked, find the most useful affordable evidence action:

\[
q^*=\arg\max_q
\frac{ExpectedDecisionUncertaintyReduction(q)}{Cost(q)}.
\]

Or more operationally:

\[
E^*=\arg\min_E Cost(E)
\quad s.t.\quad Admit(a\mid E)=true
\]

subject to deadlines and safety limits.

**Product implication:** the engine should increasingly answer "what should we learn/do next to become justified?" rather than only `BLOCK`.

---

# 53. Research-Backed Design Principles

The following are **derived design principles**, not claims that the cited literature states them verbatim.

## DP-SO-001 — Continuous justification over permanent trust

Identity continuity is not authority continuity. Every consequential capability should be explainable from current applicable evidence and delegated authority.

## DP-SO-002 — Preserve counterevidence

Evidence that previously weakened a claim is durable organizational memory until explicitly superseded, reinterpreted, or invalidated with provenance.

## DP-SO-003 — Separate generator from acceptance where consequences justify it

Self-improving/searching agents may generate plans or modifications broadly, while smaller trusted mechanisms enforce hard acceptance conditions.

## DP-SO-004 — Preserve corrective channels

Never consume the last path to observe, diagnose, challenge, repair, escalate, or transfer control merely for efficiency.

## DP-SO-005 — Authority follows evidence and reversibility

Broader authority should require stronger applicable evidence or stronger recovery/containment.

## DP-SO-006 — Deep change requires deep assurance

Verifier, ontology, governance, and constitutional changes require stronger transition evidence than ordinary execution changes.

## DP-SO-007 — Reality constrains belief; governance constrains action

Preserve both flows:

```text
world -> observation -> evidence -> model -> decisions
values/policy -> authority -> action -> world
```

A system that loses the upward evidence path becomes dogmatic; a system that loses the downward control path becomes impotent.

## DP-SO-008 — Design for open systems

Every agent, mission, and organization has an environment and changing boundary. Interfaces and assumptions are first-class.

## DP-SO-009 — Preserve lineage across replacement

Replacing models, providers, agents, workflows, or verifiers must not erase commitments, counterevidence, or audit obligations.

## DP-SO-010 — Optimize for correctable capability, not raw autonomy

The target is useful work completed per unit of justified authority while preserving future correction, not maximum action throughput.

---

# 54. Synthetic Organization Acceptance Criteria

These requirements describe the staged program beyond the initial Continuous Justification slice. EXPAND MUST assign them to milestones rather than attempting a big-bang implementation.

## AC-SO-001 — Mission entity and versioned intent

The system can persist a mission with stable identity, versioned intent, outcomes, non-goals, constraints, sponsor/root authority, and completion/stop criteria.

## AC-SO-002 — Mission changes are lineage events

Changing mission intent, completion criteria, prohibited outcomes, or risk class emits an explicit durable transition rather than mutating history invisibly.

## AC-SO-003 — Work contracts

Mission work can be represented as durable work contracts with objective, accountable owner, dependencies, expected outputs, constraints, budget, required evidence, and completion proof.

## AC-SO-004 — Work dependency DAG / graph

The organization can query upstream/downstream work dependencies and detect blocked/invalidated work when a prerequisite changes.

## AC-SO-005 — Durable organizational roles

Roles exist independently of individual agents and can be assigned, revoked, superseded, and queried historically.

## AC-SO-006 — Delegated authority graph

Non-root authority has provenance, scope, expiry/revocation, redelegation rules, and mission linkage.

## AC-SO-007 — Resource envelopes

A mission/role/agent can receive bounded resource budgets separate from capability permission.

## AC-SO-008 — Budget exhaustion is an explicit state

Resource exhaustion does not become an unhandled tool failure; it results in an organizational event and a decision such as request-more, replan, defer, or terminate.

## AC-SO-009 — Agent creation is governed

Creating/spawning a durable agent requires mission/role justification and an explicit initial authority/resource envelope.

## AC-SO-010 — Agent retirement preserves obligations

Retiring or replacing an agent cannot silently delete outstanding work, delegated obligations, counterevidence, or audit responsibility.

## AC-SO-011 — Role succession

A successor agent can inherit selected role obligations and authorities only through an explicit succession/delegation event.

## AC-SO-012 — Organization projection

The system can derive an inspectable organization projection: missions, roles, participants, reporting/delegation edges, active work, resource envelopes, and critical corrective dependencies.

## AC-SO-013 — Decision record

Configured consequential decisions produce a durable record containing the proposal, applicable evidence, material counterevidence, authority basis, decision, scope, and expected outcome.

## AC-SO-014 — Outcome linkage

Observed outcomes can be linked back to the decision/action/work that produced them and used as new evidence.

## AC-SO-015 — Explicit rejected alternatives

High-risk decision records can preserve materially considered alternatives and why they were rejected.

## AC-SO-016 — Structured disagreement

Two roles/agents may hold incompatible claims/models without one being automatically overwritten. A decision can cite the disagreement and required resolution rule.

## AC-SO-017 — Independent epistemic channels

The system can tag shared evidence/model/provider ancestry so nominally multiple reviewers are not automatically counted as independent.

## AC-SO-018 — Evidence freshness and validity domain

Evidence can expire or become out-of-domain without being deleted from history; dependent authority narrows accordingly.

## AC-SO-019 — Best-next-evidence recommendation

For a configured blocked/constrained decision, the system can enumerate candidate evidence actions and explain which blocking obligation each could satisfy.

## AC-SO-020 — Shadow experiment support

The organization can authorize bounded observation/simulation/canary actions whose purpose is evidence acquisition without granting full production authority.

## AC-SO-021 — Mission review cadence

Long-running missions can schedule durable review points that re-evaluate goals, assumptions, progress, budget, and authority.

## AC-SO-022 — Corrective capability registry

Critical organizational functions can declare observation, diagnosis, verification, recovery, escalation, and regeneration dependencies.

## AC-SO-023 — Common-cause corrective analysis

Corrective-path analysis can mark shared providers/models/credentials/databases/humans so false redundancy is visible.

## AC-SO-024 — Corrective degradation event

When an action or external observation reduces modeled corrective reserve below policy, the system emits a durable degradation event and narrows relevant authority.

## AC-SO-025 — Incident mode

The organization can enter an incident/recovery mode that prioritizes containment, evidence preservation, escalation, and restoration over ordinary optimization.

## AC-SO-026 — Constitutional policy layer

The system has an explicit representation for rules governing mission creation, agent creation, delegated authority, prohibited actions, deep-change review, and amendment authority.

## AC-SO-027 — Amendment is different from ordinary policy use

Changing constitutional/governance rules requires a distinct authorization path from exercising those rules.

## AC-SO-028 — Self-modification classification

Changes to agent identity/instructions, skills, tools, model/provider, verifier, evidence policy, governance, or constitutional machinery are classified and can invalidate dependent assurance.

## AC-SO-029 — Transition witnesses for deep change

Configured deep changes require an explicit transition witness, revalidation, or authority downgrade; old evidence is not silently assumed applicable.

## AC-SO-030 — Provider/model replacement continuity

Switching LLM/model/tool provider preserves durable work/mission/decision/evidence state independently of provider-specific session memory.

## AC-SO-031 — Organizational branch and merge

The system can eventually represent organizational experiments/branches and detect conflicting assurance or governance semantics when merging successful branches.

## AC-SO-032 — Performance without reasoning bottleneck

Routine low-risk actions can be handled by deterministic/cached envelopes; the organization does not require an LLM organization-wide deliberation for every read or benign tool call.

## AC-SO-033 — Human root authority remains explicit

The system can always identify the human/operator/team authority from which autonomous mission authority ultimately derives unless a future explicitly designed multi-principal constitution replaces it.

## AC-SO-034 — Emergency revocation

Root authority can revoke mission/role/agent authority rapidly without deleting historical evidence or falsifying prior decision records.

## AC-SO-035 — Organizational audit reconstruction

Given a consequential outcome, an operator can reconstruct which mission, work item, role, delegation, evidence state, decision, capability authorization, and external action led to it.

## AC-SO-036 — Organizational learning is explicit

Post-outcome learning can update claims, procedures, skills, or proposed policies, but high-impact learned changes pass through the same change/justification machinery as human-authored changes.

## AC-SO-037 — Skill/procedure lineage

Reusable skills/procedures can accumulate evidence and counterevidence across missions and can be retired or versioned when their validity changes.

## AC-SO-038 — Regenerative-risk visibility

The system can surface configured critical capabilities that have become single-source or unreconstructible even while they remain operational.

## AC-SO-039 — Mission completion is evidence-backed

A mission cannot be marked complete solely because an agent states that it is done; configured completion criteria must have evidence or explicit human override.

## AC-SO-040 — Synthetic organization remains composable

The core organization model does not require software-development-specific concepts. `lights-out-swe` is an adapter/work procedure, not the organization kernel.

---

# 55. Bootstrap Roadmap: From Open Pincery To Synthetic Organization

## Stage SO-0 — Continuous-agent substrate (already substantially present)

Required base:

- durable identity;
- event log;
- wake lifecycle;
- async messaging;
- sandbox/tool boundary;
- capability nonce;
- credential isolation;
- lifecycle formalization.

Do not duplicate these.

## Stage SO-1 — Continuous Justification

Ship the `AC-CJ-*` vertical slice:

- assurance events/projection;
- counterevidence persistence;
- change-aware applicability;
- decision envelope;
- shadow mode;
- narrow capability integration;
- explanation and evidence-planning UX.

**Outcome:** one continuous agent has durable reasons around its authority.

## Stage SO-2 — Mission and organization primitives

Introduce:

- mission contracts;
- role entities;
- work contracts;
- delegation graph;
- resource envelopes;
- organization projection.

Use a software-engineering mission as the first dogfood path, composing with `lights-out-swe` rather than rewriting its pipeline.

**Outcome:** multiple continuous agents can behave as an accountable team around one mission.

## Stage SO-3 — Evidence-producing organization

Introduce:

- durable decision records;
- structured disagreement;
- outcome/evidence linkage;
- evidence freshness;
- best-next-evidence planning;
- explicit mission review loops.

**Outcome:** the organization can recognize when it does not know enough and can organize work to find out.

## Stage SO-4 — Resource-governed autonomous operations

Introduce:

- budget allocation;
- progressive autonomy;
- resource requests/reallocation;
- multi-mission portfolio scheduling;
- incident/recovery mode;
- corrective topology.

**Outcome:** the organization can pursue multiple goals without treating compute/money/time/attention as unlimited.

## Stage SO-5 — Organizational self-reconfiguration

Introduce:

- role creation/retirement;
- process/skill evolution;
- model/provider migration;
- transition witnesses;
- shadow organizational experiments;
- branch/merge semantics;
- explicit governance amendments.

**Outcome:** the organization can improve its own operating structure without silently rewriting the reasons that justified it.

## Stage SO-6 — General mission substrate

Prove the kernel across at least three materially different domains, for example:

1. software delivery / codebase steward;
2. research and evidence synthesis;
3. business operations / market or customer workflow.

A concept that only works for software should remain in the software adapter.

---

# 56. How `lights-out-swe` Fits The Synthetic Organization

`lights-out-swe` is not a competing organization runtime. It is a highly structured **software-production procedure** with explicit persistent artifacts and gated phases.

Current pipeline:

```text
EXPAND -> DESIGN -> ANALYZE -> BUILD -> REVIEW -> RECONCILE -> VERIFY -> DEPLOY
```

This maps naturally into a synthetic organization:

| Lights Out SWE concept | Synthetic organization interpretation |
|---|---|
| `docs/input/` | mission/domain evidence and governing input |
| `scope.md` + `AC-*` | mission contract / obligations |
| `design.md` | proposed system/work model |
| `readiness.md` | pre-execution assurance / evidence plan |
| BUILD | execution role |
| REVIEW | independent critique role |
| RECONCILE | semantic drift / lineage check |
| VERIFY | evidence-producing verification role |
| DEPLOY | consequential action requiring authority |
| `scaffolding/log.md` | persistent experiment/provenance history |
| git checkpoints | reversible lineage boundaries |
| `/iterate` | mission/scope evolution transition |

The Open Pincery organization MAY invoke Lights Out SWE as a skill/procedure for software missions. It SHOULD ingest its stable AC IDs and verification artifacts rather than reimplementing its phases.

Relevant current repositories:

- https://github.com/RCSnyder/lights-out-swe
- https://github.com/RCSnyder/lights-out-swe-plugin

Fresh agents MUST re-check their current pipeline semantics before building adapters.

---

# 57. How `swe-dev` Fits The Synthetic Organization

`swe-dev` should remain a harness-neutral judgment/reasoning layer. The **Justified Change** skill can help agents/humans author:

- change classification;
- affected claims;
- transition witnesses;
- counterevidence searches;
- corrective-path analysis;
- evidence acquisition plans;
- structured explanations.

It SHOULD NOT be required by the Open Pincery runtime. The runtime must enforce deterministic durable semantics without depending on Copilot/Claude-specific skill execution.

The relationship is:

```text
swe-dev / human reasoning
      -> proposes structured assurance/governance artifacts
      -> Open Pincery records/evaluates them
      -> runtime mints bounded capability
      -> outcome creates new evidence
```

Relevant repository:

- https://github.com/RCSnyder/swe-dev

---

# 58. Research Program And Falsifiable Hypotheses

Do not treat the synthetic-organization thesis as validated merely because the software can be implemented. Dogfood it against simpler baselines.

## H-SO-001 — Counterevidence continuity

**Hypothesis:** durable counterevidence preservation reduces repeated reintroduction of previously discovered unsafe/incorrect actions after context compaction, model replacement, or agent replacement.

**Baseline:** normal event/history retrieval without claim invalidation semantics.

## H-SO-002 — Dynamic authority envelopes

**Hypothesis:** progressive/constrained authority prevents a meaningful class of high-consequence mistakes with less productivity loss than binary allow/block policy.

**Baseline:** static RBAC/capability policy.

## H-SO-003 — Corrective closure

**Hypothesis:** common-cause-aware corrective-path metrics detect dangerous brittleness earlier than ordinary availability/health metrics.

**Baseline:** uptime, replica count, ordinary fallback count.

## H-SO-004 — Evidence planning

**Hypothesis:** minimum-evidence recommendations reduce time-to-unblock compared with generic denial explanations.

**Baseline:** reason-code-only block.

## H-SO-005 — Structured disagreement

**Hypothesis:** retaining incompatible model/evidence branches improves discovery of novel failures compared with immediate majority/LLM synthesis.

**Baseline:** single consensus summary.

## H-SO-006 — Mission compiler usefulness

**Hypothesis:** durable mission/work/role contracts reduce human orchestration overhead for multi-day agent work compared with ad-hoc prompt orchestration.

**Baseline:** shared chat/task list with the same models/tools.

## H-SO-007 — Provider independence

**Hypothesis:** organization-level durable state reduces loss of obligations/evidence during model/provider replacement.

**Baseline:** provider-native session/thread persistence.

## H-SO-008 — Self-reconfiguration safety

**Hypothesis:** transition witnesses and deep-change classification catch organization/process changes that pass ordinary tests but invalidate prior assurance.

**Baseline:** tests + static policy + git/event history.

## H-SO-009 — Separation of powers

**Hypothesis:** independent verifier/authorizer roles reduce correlated self-approval failures for high-risk changes at acceptable coordination cost.

**Baseline:** same agent proposes, validates, and executes.

## H-SO-010 — Regenerative-risk detection

**Hypothesis:** explicit reconstruction-dependency tracking identifies capabilities likely to become unrecoverable before current operation fails.

**Baseline:** current health / availability monitoring.

Every hypothesis MUST have a kill/reframe threshold before claiming product value.

---

# 59. Research Canon And Provenance Ledger

This section records foundational and emerging work discussed during the design conversation. It exists so fresh agents can recover the intellectual context without hallucinating that the synthesis is already an established discipline.

**Usage rule:** each record separates (A) what the cited source actually studies from (B) the design inspiration taken here. The Open Pincery design inspiration is often a new synthesis.

## 59.1 Counterfactuals, information, viability, and autonomy

### R-001 — David Deutsch, “Constructor Theory” (2012/2013)

- **Link:** https://arxiv.org/abs/1210.7439
- **Research status:** foundational proposal / active research program.
- **Source contribution:** proposes formulating fundamental laws in terms of possible and impossible transformations rather than only trajectories from initial conditions.
- **Open Pincery inspiration:** represent capability and governance as changes to which organizational transitions are constructible/authorized.
- **Do not infer:** Open Pincery is not a constructor-theory implementation; organization-level `CAN/MAY` is an analogy/synthesis.

### R-002 — David Deutsch & Chiara Marletto, “Constructor Theory of Information” (2014)

- **Link:** https://arxiv.org/abs/1405.5563
- **Research status:** active foundational physics/information program.
- **Source contribution:** develops information concepts through possible/impossible physical transformations.
- **Open Pincery inspiration:** distinguish durable information/evidence from arbitrary text and reason about the transformations evidence enables.

### R-003 — Artemy Kolchinsky & David H. Wolpert, “Semantic information, autonomous agency, and nonequilibrium statistical physics” (2018)

- **Link:** https://arxiv.org/abs/1806.08053
- **Research status:** published theoretical framework.
- **Source contribution:** defines a notion of semantic information through information whose causal presence matters to viability under counterfactual interventions.
- **Open Pincery inspiration:** prioritize information by decision/viability relevance; connect evidence acquisition to meaningful action rather than raw storage volume.
- **Do not infer:** our claim-status or evidence-planning semantics are not derived theorems from this paper.

### R-004 — Maël Montévil & Matteo Mossio, “Biological organisation as closure of constraints” (2015)

- **Link:** https://pubmed.ncbi.nlm.nih.gov/25752259/
- **Research status:** theoretical biology.
- **Source contribution:** formalizes biological organization via constraints that participate in maintaining one another under open thermodynamic conditions.
- **Open Pincery inspiration:** corrective/organizational capabilities form maintenance dependencies; durable autonomy requires maintaining the mechanisms that preserve agency.
- **Do not infer:** synthetic organizations are not literally organisms.

### R-005 — Alexander Klyubin, Daniel Polani & Chrystopher Nehaniv, “Empowerment: A Universal Agent-Centric Measure of Control” (2005); Christoph Salge et al., “Empowerment — an Introduction” (2013)

- **Links:** https://researchprofiles.herts.ac.uk/en/publications/empowerment-a-universal-agent-centric-measure-of-control/ ; https://arxiv.org/abs/1310.1863
- **Research status:** established information-theoretic agency/control literature.
- **Source contribution:** empowerment measures potential control using channel capacity between actions and future sensor states.
- **Open Pincery inspiration:** preserve useful future options; `viable empowerment` in this PRD is explicitly our constrained extension.

## 59.2 Reflexivity, logical uncertainty, and embedded agency

### R-006 — Fallenstein, Taylor & Christiano, “Reflective Oracles: A Foundation for Classical Game Theory” (2015)

- **Link:** https://arxiv.org/abs/1508.04145
- **Research status:** theoretical AI/game theory.
- **Source contribution:** develops reflective oracles that handle certain self-referential probabilistic queries without standard diagonalization collapse.
- **Open Pincery inspiration:** a sufficiently advanced organization cannot assume perfect external self-modeling; reflective uncertainty must be explicit.

### R-007 — Garrabrant, Benson-Tilsen, Critch, Soares & Taylor, “Logical Induction” (2016)

- **Link:** https://arxiv.org/abs/1609.03543
- **Research status:** theoretical AI / logical uncertainty.
- **Source contribution:** a computable reasoner assigns and refines probabilities over logical statements without instant deductive omniscience, including statements involving its own beliefs.
- **Open Pincery inspiration:** represent `proved`, `likely`, `unknown`, `currently uncomputed`, and `stale` distinctly; never assume a reasoning agent has deductive closure over its own knowledge.

### R-008 — López-Díaz & Gershenson, “A Matter of Time: Towards a General Theory of Agency” (2026)

- **Link:** https://arxiv.org/abs/2606.23122
- **Research status:** emerging preprint; not established general theory.
- **Source contribution:** proposes a graded organizational theory of agency emphasizing timescales, closure, anticipation, and reconstruction of future possibility spaces.
- **Open Pincery inspiration:** explicitly model multi-timescale organizational loops and distinguish autonomy, goal-directedness, agency, and open-endedness.
- **Do not infer:** the paper is not a validated foundation for synthetic organizations.

## 59.3 Open-ended evolution and self-modification

### R-009 — Russell K. Standish, “Open-Ended Artificial Evolution” (2002)

- **Link:** https://arxiv.org/abs/nlin/0210027
- **Research status:** artificial-life/open-ended evolution research.
- **Source contribution:** discusses the challenge of sustained novelty/open-endedness and distinguishes it from simple size/complexity growth.
- **Open Pincery inspiration:** do not equate agent self-improvement with open-ended useful development.

### R-010 — Adams, Zenil, Davies & Walker, “Formal Definitions of Unbounded Evolution and Innovation Reveal Universal Mechanisms for Open-Ended Evolution in Dynamical Systems” (2016)

- **Link:** https://arxiv.org/abs/1607.01750
- **Research status:** theoretical/experimental artificial-life work.
- **Source contribution:** proposes formal definitions for unbounded evolution and innovation and studies changing-rule cellular automata.
- **Open Pincery inspiration:** open-ended organizational evolution requires changes to generators/rules, not merely state optimization.

### R-011 — López-Díaz, Rivera Torres, Febres & Gershenson, “Characterizing Open-Ended Evolution Through Undecidability Mechanisms in Random Boolean Networks” (2025/2026)

- **Link:** https://arxiv.org/abs/2512.15534
- **Research status:** emerging preprint.
- **Source contribution:** proposes an OEE metric in a particular Boolean-network setting and studies mechanisms associated with sustained novelty.
- **Open Pincery inspiration:** measure whether organizational/process evolution produces durable novelty rather than repeated local optimization.
- **Do not infer:** its \(\Omega\) metric is not adopted as an Open Pincery organizational metric.

### R-012 — Gao et al., “A Survey of Self-Evolving Agents: On Path to Artificial Super Intelligence” (2025)

- **Link:** https://arxiv.org/abs/2507.21046
- **Research status:** survey.
- **Source contribution:** organizes self-evolving-agent work by what, when, and how agents evolve, including memory, tools, architecture, models, and evaluation.
- **Open Pincery inspiration:** self-modification needs explicit component/change classification and safety evaluation.

## 59.4 Verifier-grounded evolution and dynamic assurance

### R-013 — Calinescu et al., “Engineering Trustworthy Self-Adaptive Software with Dynamic Assurance Cases” / ENTRUST (2017)

- **Link:** https://arxiv.org/abs/1703.06350
- **Research status:** established self-adaptive software assurance research.
- **Source contribution:** combines design-time/runtime modeling and verification with dynamic assurance cases for self-adaptive systems.
- **Open Pincery inspiration:** Continuous Justification extends an existing dynamic-assurance lineage; do not claim dynamic assurance as novel.

### R-014 — Banerjee, Xu & Singh, “SEVerA: Verified Synthesis of Self-Evolving Agents” (2026)

- **Link:** https://arxiv.org/abs/2603.25111
- **Research status:** emerging agent/formal-methods research.
- **Source contribution:** combines self-evolving agent synthesis with hard formal behavioral contracts and verified fallbacks.
- **Open Pincery inspiration:** mutable generators can be surrounded by smaller trusted acceptance mechanisms.

### R-015 — Li, Wu, Gan & Liu, “Self-Modifying Lean Proof Agents with Verifier-Grounded Benchmark Coevolution” (2026)

- **Link:** https://arxiv.org/abs/2607.17352
- **Research status:** recent preprint.
- **Source contribution:** evolves proof-agent workflows while retaining a fixed trusted Lean-grounded acceptance loop.
- **Open Pincery inspiration:** distinguish a mutable workspace/organization from a smaller trusted evolution/admission base.

### R-016 — Yang et al., “Formally Verifiable Self-Evolving Skills for Physical AI Agents” / VASO (2026)

- **Link:** https://arxiv.org/abs/2606.05395
- **Research status:** recent preprint.
- **Source contribution:** turns verifier counterexample traces into updates to reusable skill contracts.
- **Open Pincery inspiration:** counterexamples should update durable reusable skills/assurance rather than disappear inside one session.

### R-017 — Protogeros, Schneider & Vanbever, “Self-evolving network verifiers” (2026)

- **Link:** https://arxiv.org/abs/2608.11340
- **Research status:** very recent research preprint.
- **Source contribution:** a coding agent extends a symbolic network verifier using disagreement with executable router behavior as an oracle.
- **Open Pincery inspiration:** assurance machinery can itself evolve from grounded discrepancies, but changes to the verifier need their own transition assurance.

## 59.5 Causality, scale, and abstraction

### R-018 — Comolatti & Hoel, “Causal emergence is widespread across measures of causation” (2022)

- **Link:** https://arxiv.org/abs/2202.01854
- **Research status:** active causal-emergence research.
- **Source contribution:** argues and demonstrates across multiple causation measures that some macro descriptions can exhibit stronger/cleaner causal relations than micro descriptions.
- **Open Pincery inspiration:** organizational diagnosis should search for the useful causal scale; transistor/tool-call detail is not always the right explanation.

### R-019 — Englberger & Dhami, “Causal Abstractions, Categorically Unified” (2025)

- **Link:** https://arxiv.org/abs/2510.05033
- **Research status:** recent causal/category-theory work.
- **Source contribution:** formalizes relations between causal models at different abstraction levels using natural transformations between Markov functors.
- **Open Pincery inspiration:** semantic/causal model migrations should have explicit mappings rather than informal equivalence claims.

### R-020 — Lorenz & Tull, “Causal and Compositional Abstraction” (2026)

- **Link:** https://arxiv.org/abs/2602.16612
- **Research status:** recent theoretical work.
- **Source contribution:** provides a categorical account of high/low-level abstractions and distinguishes upward/downward abstraction maps for compositional models.
- **Open Pincery inspiration:** transport evidence and guarantees between operational, team, mission, and organizational scales only through explicit mappings.

## 59.6 Composition and open systems

### R-021 — Libkind & Myers, “Towards a double operadic theory of systems” (2025)

- **Link:** https://arxiv.org/abs/2505.18329
- **Research status:** applied category theory / categorical systems theory.
- **Source contribution:** develops a framework packaging open systems, interactions, interfaces, and maps into compositional categorical structures.
- **Open Pincery inspiration:** interfaces and composition are first-class; avoid a monolithic organization state machine when systems/teams can be composed through explicit boundaries.

### R-022 — John C. Baez, “Double Categories of Open Systems: the Cospan Approach” (2025/2026)

- **Link:** https://arxiv.org/abs/2509.22584
- **Research status:** categorical open-systems overview/research.
- **Source contribution:** develops double-categorical composition of open systems through shared variables/interfaces using structured/decorated cospans.
- **Open Pincery inspiration:** organizations, agents, workflows, and external services should be modeled as open composable systems rather than sealed boxes.

### R-023 — Furter, Huang & Zardini, “Composable Uncertainty in Symmetric Monoidal Categories for Design Problems” (2025/2026)

- **Link:** https://arxiv.org/abs/2503.17274
- **Research status:** applied category theory / systems/control.
- **Source contribution:** studies compositional treatment of uncertainty in open-system design problems.
- **Open Pincery inspiration:** uncertainty should compose through system boundaries rather than disappear when local components are wired together.

## 59.7 Software/system invariants, distribution, authority, and organization

### R-024 — C. A. R. Hoare, “An Axiomatic Basis for Computer Programming” (1969)

- **Link:** https://dl.acm.org/doi/10.1145/363235.363259
- **Research status:** foundational program verification.
- **Source contribution:** establishes axiomatic reasoning about program correctness with pre/postconditions.
- **Open Pincery inspiration:** consequential work/actions should carry explicit obligations and proof/evidence conditions rather than only imperative plans.

### R-025 — Leslie Lamport, “Time, Clocks, and the Ordering of Events in a Distributed System” (1978)

- **Link:** https://www.microsoft.com/en-us/research/publication/time-clocks-ordering-events-distributed-system/
- **Research status:** foundational distributed systems.
- **Source contribution:** defines happened-before partial ordering and logical clocks for distributed events.
- **Open Pincery inspiration:** organizational/event lineage is partially ordered; do not assume a single globally known real-time ordering for independent agents.

### R-026 — Fischer, Lynch & Paterson, “Impossibility of Distributed Consensus with One Faulty Process” (1985)

- **Link:** https://dl.acm.org/doi/10.1145/3149.214121
- **Research status:** foundational impossibility result.
- **Source contribution:** deterministic consensus in a fully asynchronous model can fail to terminate with one crash fault.
- **Open Pincery inspiration:** organizational coordination must state timing/failure assumptions; do not promise universal agreement/liveness.

### R-027 — Leslie Lamport, “The Part-Time Parliament” (1998)

- **Link:** https://www.microsoft.com/en-us/research/publication/part-time-parliament/
- **Research status:** foundational consensus work (Paxos).
- **Source contribution:** consensus/replicated-log protocol under failures and message uncertainty.
- **Open Pincery inspiration:** durable organization state and distributed authority need explicit agreement semantics where multiple replicas/principals exist.

### R-028 — Gilbert & Lynch, “Brewer's Conjecture and the Feasibility of Consistent, Available, Partition-Tolerant Web Services” (2002)

- **Link:** https://dl.acm.org/doi/10.1145/564585.564601
- **Research status:** foundational distributed-systems tradeoff result in its model.
- **Source contribution:** formalizes the consistency/availability impossibility under partitions for the relevant asynchronous model.
- **Open Pincery inspiration:** synthetic organizations must expose availability/consistency tradeoffs in distributed deployments rather than assume perfect coordination.

### R-029 — Jerome Saltzer & Michael Schroeder, “The Protection of Information in Computer Systems” (1975)

- **Link:** https://ieeexplore.ieee.org/document/1451869
- **Research status:** foundational computer security.
- **Source contribution:** develops protection mechanisms and enduring principles including least privilege, fail-safe defaults, complete mediation, separation of privilege, and least common mechanism.
- **Open Pincery inspiration:** capability envelopes, complete mediation at effect boundaries, small trusted bases, and separation of duties.

### R-030 — Saltzer, Reed & Clark, “End-to-End Arguments in System Design” (1984)

- **Link:** https://dl.acm.org/doi/10.1145/357401.357402
- **Research status:** foundational systems-design principle.
- **Source contribution:** argues that some functions can only be completely/correctly implemented with knowledge at application endpoints and may be redundant at lower layers.
- **Open Pincery inspiration:** do not force semantic mission correctness into the sandbox/capability layer; low-level enforcement and end-to-end verification are complementary.

### R-031 — David Parnas, “On the Criteria To Be Used in Decomposing Systems into Modules” (1972)

- **Link:** https://dl.acm.org/doi/10.1145/361598.361623
- **Research status:** foundational software architecture.
- **Source contribution:** modularization around information-hiding/design decisions improves flexibility and comprehensibility.
- **Open Pincery inspiration:** organize modules around volatile decisions—authority, evidence policy, mission semantics, provider adapters—rather than one giant synthetic-organization service.

### R-032 — Jim Gray, “The Transaction Concept: Virtues and Limitations” (1981)

- **Link:** https://infolab.usc.edu/csci599/Fall2008/papers/b-2.pdf
- **Research status:** foundational database/transaction systems.
- **Source contribution:** discusses transactions as state transformations with atomicity, consistency, durability and their limitations, including long-lived activity.
- **Open Pincery inspiration:** some organizational changes need atomic transition semantics; long-lived missions also require compensating/forward-recovery patterns beyond database rollback.

### R-033 — Melvin Conway, “How Do Committees Invent?” (1968)

- **Link:** https://www.melconway.com/research/committees.html
- **Research status:** foundational organization/system design observation.
- **Source contribution:** relates design-system structure to organizational communication structure.
- **Open Pincery inspiration:** synthetic organization topology is an engineering input because it may shape the systems and work products the organization produces.

## 59.8 Cybernetics, replication, long-lived evolution, and implementation architecture

### R-034 — W. Ross Ashby, *An Introduction to Cybernetics* (1956)

- **Link:** https://www.ashby.info/
- **Research status:** foundational cybernetics monograph; not a research paper.
- **Source contribution:** develops regulation, feedback, variety, stability, and the law of requisite variety in a general systems vocabulary.
- **Open Pincery inspiration:** closed-loop mission execution, explicit regulator capacity, and the later [SYNTHESIS] notion of generative requisite variety.
- **Do not infer:** `Gamma_A >= Gamma_C` is not Ashby's formula; it is a proposed extension inspired by requisite variety.

### R-035 — Shapiro, Preguiça, Baquero & Zawirski, “Conflict-free Replicated Data Types” (2011)

- **Link:** https://inria.hal.science/hal-00932836
- **Research status:** foundational replicated-data work.
- **Source contribution:** develops replicated data types whose concurrent updates can converge according to mathematically defined merge semantics without global coordination for every update.
- **Open Pincery inspiration:** if future organization deployments become multi-primary/federated, merge semantics must be explicit; "eventually merge agent state" is not a sufficient design.
- **Do not infer:** organizational disagreement or assurance merge is automatically a CRDT; some semantic conflicts must remain unresolved rather than mechanically joined.

### R-036 — Butler Lampson, “Hints and Principles for Computer System Design” (1983; expanded 2020 version)

- **Link:** https://arxiv.org/abs/2011.02455
- **Research status:** foundational/retrospective systems-design guidance.
- **Source contribution:** presents durable principles and techniques for simple, timely, efficient, adaptable, dependable system construction.
- **Open Pincery inspiration:** keep the synthetic-organization kernel small, incremental, and composable; avoid turning every theoretical idea into synchronous runtime machinery.

### R-037 — M. M. Lehman, “Programs, Life Cycles, and Laws of Software Evolution” (1980)

- **Link:** https://ieeexplore.ieee.org/document/1456074/
- **Research status:** foundational software-evolution work; later literature revises and debates the laws.
- **Source contribution:** studies long-lived software embedded in changing environments and recurring dynamics of continuing change and complexity.
- **Open Pincery inspiration:** a synthetic organization and the systems it owns should be treated as continuously evolving socio-technical objects; architecture must include retirement/simplification as well as growth.

### R-038 — Hellerstein, Stonebraker & Hamilton, “Architecture of a Database System” (2007)

- **Link:** https://dl.acm.org/doi/10.1561/1900000002
- **Research status:** influential systems architecture synthesis.
- **Source contribution:** describes how mature database systems decompose complex responsibilities into process, query, transaction, storage, and shared service layers.
- **Open Pincery inspiration:** persistent organization state should remain grounded in orthodox database/event-sourcing architecture rather than placing organization semantics inside one agent prompt or monolithic orchestrator.

## 59.9 Reading discipline

Fresh agents MUST NOT cite this ledger as proof of product novelty. Use it to:

- identify existing mathematical vocabulary;
- avoid reinventing established mechanisms;
- motivate experiments;
- distinguish established results from the product synthesis.

Where a source is a recent preprint, label it as such in downstream docs. Where a concept is only our synthesis, retain `[SYNTHESIS]` or equivalent language until evidence justifies stronger framing.

---

# 60. Theory-To-Product Translation Matrix

| Theoretical idea | Product interpretation | Initial Open Pincery primitive | Later extension |
|---|---|---|---|
| Possible/impossible transformations | capability surface + prohibited actions | capability shapes/nonces | mission-level transition constraints |
| Closure of constraints | maintenance/corrective dependencies | maintenance + event projections | corrective-closure graph |
| Semantic information | evidence relevant to decisions/viability | assurance evidence | decision-directed evidence planning |
| Logical uncertainty | graded epistemic status | claim status | bounded reasoning / uncertainty-aware policy |
| Reflective uncertainty | no perfect self-verifier | trusted runtime boundary | governed verifier evolution |
| Open-ended evolution | evolving skills/roles/processes | self-configuration | organizational self-reconfiguration |
| Causal emergence | useful macro-level diagnosis | metrics/events | causal org/system models |
| Causal abstraction | explicit high/low model mappings | transition witness | semantic/ontology migration |
| Open-system composition | explicit interfaces | async messaging + tool boundary | composable organizations/workflows |
| Least privilege | minimal authority | capability nonces | dynamic authority envelopes |
| Complete mediation | every consequential effect checked | nonce gate | mission/governance-aware mediation |
| Separation of privilege | independent powers | human + runtime controls | proposer/verifier/authorizer separation |
| Partial order / logical time | causal event history | append-only event log | organization-wide causal lineage |
| Consensus limits | explicit coordination assumptions | single DB / CAS today | federated/multi-principal deployments |
| Information hiding | module around volatile decisions | runtime modules | independent mission/evidence/governance modules |
| Conway's law | organization topology shapes outputs | async agent graph | deliberate org architecture |
| Dynamic assurance | assurance evolves with system | Continuous Justification | organization-wide assurance metabolism |
| Verifier-grounded evolution | mutable generator, trusted acceptance | sandbox + gate | self-evolving skills/processes |

---

# 61. Synthetic Organization Data Objects (Guidance, Not Final Schema)

Do not commit to these database tables before DESIGN. They define semantic objects builders must account for.

## 61.1 `Mission`

```text
id
version
sponsor
intent
outcomes
non_goals
risk_class
status
budget_envelope
review_at
created_event
supersedes
```

## 61.2 `Role`

```text
id
mission_scope
purpose
responsibilities
authority_template
separation_constraints
status
```

## 61.3 `RoleAssignment`

```text
role_id
principal_id
starts_at
expires_at
delegated_by
constraints
status
```

## 61.4 `Delegation`

```text
id
from_principal
to_principal
mission_id
action/capability scope
budget scope
redelegation
requires_assurance
issued_at
expires_at
revoked_at
```

## 61.5 `WorkContract`

```text
id
mission_id
objective
owner
reviewer
inputs
outputs
dependencies
constraints
budget
evidence_obligations
completion_rule
status
```

## 61.6 `DecisionRecord`

```text
id
mission_id
proposal
alternatives
claims/evidence refs
counterevidence refs
assumptions
authority refs
verifier refs
verdict/scope
expected_outcome
review/expiry
```

## 61.7 `OutcomeObservation`

```text
id
decision/action/work ref
observer/instrument
observation
provenance
confidence/status
recorded_at
```

## 61.8 `OrganizationSnapshot`

A snapshot is a deterministic projection for query/display, not source of truth:

```text
missions
roles
assignments
work
active delegations
resource envelopes
current assurance summaries
corrective degradation warnings
```

---

# 62. Organizational Safety And Failure Modes

## 62.1 The organization optimizes the proxy instead of the mission

Mitigations:

- durable non-goals and constraints;
- outcome evidence rather than only internal completion metrics;
- human mission review;
- structured counterevidence;
- separation between work producer and verifier for high-risk outputs.

## 62.2 The organization grows agents/processes faster than it can govern them

Signal:

\[
\Delta_\Gamma>0
\]

Mitigations:

- creation budgets;
- new-role shadow period;
- assurance requirements for generative processes;
- periodic topology/complexity review.

## 62.3 The organization becomes epistemically closed

Symptoms:

- no evidence can falsify key claims;
- dissent is automatically overwritten;
- all reviewers share identical model/data/provider lineage;
- policies permit suppressing negative observations.

Mitigations:

- falsifier fields for important claims;
- independent evidence channels;
- counterevidence immutability;
- audit/review role with explicit challenge authority.

## 62.4 The organization self-approves its own constitutional rewrite

Mitigation: deep governance/constitution changes require a distinct authority route and preferably independent verifier/human root approval.

## 62.5 The organization remains productive while losing regenerative capacity

Example: all work succeeds, but only one credential holder/tool/provider can rebuild the critical execution path.

Mitigation: regenerative-risk visibility and common-cause tags.

## 62.6 Coordination overwhelms useful work

Mitigation: risk-tier separation of powers. Low-risk reversible actions remain cheap; formal institutional machinery scales with consequence.

## 62.7 Infinite deliberation / evidence gathering

Mitigation:

- bounded evidence budgets;
- deadlines;
- stop conditions;
- explicit `DEFER` / human escalation;
- value-of-information threshold.

## 62.8 Authority drift through long-lived identity

Mitigation: expiry, event-driven invalidation, mission-scope linkage, revalidation on deep changes, and no standing high-risk authority without review.

---

# 63. Dogfood Missions For Bootstrapping The Organization

## Mission 1 — Codebase steward

Why first:

- already aligned with Open Pincery's north star;
- objective evidence is abundant: tests, git, static analysis, TLA+, CI, deployment;
- can integrate with `lights-out-swe`;
- strong reversible workflow boundaries.

Organization roles might include:

```text
Mission Owner
Scout / Researcher
Planner
Builder
Verifier
Release Authority
Operator / Observer
Auditor
```

## Mission 2 — Research/capstone organization

Use the existing research/capstone discipline as an epistemic stress test:

```text
question
-> source survey
-> claim/evidence graph
-> competing theories
-> synthesis
-> falsification
-> artifact
-> source change / retraction
-> incremental re-evaluation
```

This tests whether Continuous Justification generalizes beyond code.

## Mission 3 — Small business operational workflow

Choose a bounded operation such as:

- content/research pipeline;
- customer-support triage with human send approval;
- market research and CRM enrichment;
- bookkeeping preparation without autonomous money movement;
- procurement research with purchase requiring human authority.

This tests resources, deadlines, external systems, human approvals, and non-software completion evidence.

## Mission 4 — Internal organization improvement

After earlier stages are stable, allow the organization to propose changes to:

- role definitions;
- work templates;
- skill instructions;
- reviewer topology;
- evidence policies.

Run changes in shadow/branch mode before adoption.

This is the first true test of correctable organizational evolution.

---

# 64. Anti-Local-Maximum Rules For The Program

The originating design process repeatedly identified a risk of optimizing into a narrow local product maximum. Preserve these rules:

1. A concept belongs in the organization kernel only after demonstrating usefulness in **at least two materially different mission types**.
2. Domain-specific parsers/checkers remain adapters.
3. Do not equate agent count with organizational sophistication.
4. Do not optimize authority throughput at the expense of correction/evidence quality.
5. Do not add a metric merely because it is mathematically attractive; it must change a decision or experimental prediction.
6. Preserve simple paths for ordinary users; advanced assurance depth must scale with consequence.
7. Prefer interoperability with existing standards/tools over owning the stack.
8. Keep current Open Pincery security/correctness primitives below the organization layer.
9. Measure operator effort and false blocks; theoretically principled unusability is still product failure.
10. Retain kill/reframe criteria for every research-heavy feature.

---

# 65. Long-Horizon Definition Of Success

Open Pincery has become a credible synthetic-organization substrate when a fresh human can provide a bounded mission and the system can, under explicit resource and authority limits:

1. preserve the mission as durable versioned intent;
2. determine what it needs to know;
3. organize work and roles;
4. delegate only the authority required;
5. execute through durable agents/tools;
6. obtain independent evidence of results;
7. maintain an auditable causal/epistemic history;
8. preserve counterevidence and disagreement;
9. recover or escalate when assumptions fail;
10. learn reusable procedures without silently weakening constraints;
11. reorganize roles/processes under governed transitions;
12. survive replacement of individual models/agents/providers while preserving obligations and institutional memory.

The strongest long-horizon formulation remains [SYNTHESIS]:

> **Build a lineage of organizations that can become more capable, more specialized, and more autonomous without losing the ability to discover that they are wrong, constrain what they may do, and construct a corrective alternative.**

That is the north star. Continuous Justification is the first shippable institutional primitive on the path.


# Appendix A — Compact Decision Example

```yaml
action_intent:
  agent: codebase-steward
  capability: repo-write
  requested_scope:
    repository: owner/repo
    branch: main
    operation: dependency-upgrade

requirements:
  can:
    - claim:repo-checkout-valid
  know:
    - claim:working-tree-preserved
    - claim:tests-pass
  may:
    - authority:main-write-approved
  admissible:
    - claim:no-protected-path-violation
  recover:
    - claim:revert-path-available

current_status:
  claim:repo-checkout-valid: preserved
  claim:working-tree-preserved: stale
  claim:tests-pass: preserved
  authority:main-write-approved: preserved
  claim:no-protected-path-violation: preserved
  claim:revert-path-available: preserved

decision:
  verdict: REQUIRE_EVIDENCE
  blocking:
    - claim:working-tree-preserved
  permitted_scope:
    operation: read-only-inspection
  next_evidence:
    - evidence_action:git-status-and-diff
```

# Appendix B — Suggested Reason Codes

Use stable machine-readable reason codes plus human text. Initial candidates:

```text
CJ_EVIDENCE_EXPIRED
CJ_EVIDENCE_INVALIDATED
CJ_COUNTEREVIDENCE_ACTIVE
CJ_CLAIM_UNKNOWN
CJ_CLAIM_STALE
CJ_SEMANTIC_DRIFT
CJ_TRANSITION_WITNESS_REQUIRED
CJ_AUTHORITY_MISSING
CJ_SCOPE_TOO_BROAD
CJ_RECOVERY_UNAVAILABLE
CJ_RECOVERY_COMMON_CAUSE
CJ_DECISION_STALE
CJ_POLICY_VERSION_CHANGED
CJ_VERIFIER_CHANGED
CJ_OVERRIDE_ACTIVE
CJ_SHADOW_ONLY
CJ_PROJECTION_ERROR
CJ_EVIDENCE_ACTION_AVAILABLE
```

Avoid reason-code explosion before real dogfood data.

# Appendix C — Evidence Quality Checklist

For an evidence item supporting high-consequence authority:

```text
[ ] actual artifact/observation exists
[ ] subject is bound (agent/workspace/artifact/commit/environment as relevant)
[ ] producer/verifier identity is known where needed
[ ] freshness/expiry is explicit
[ ] semantic reach is explicit
[ ] known failure would make it fail, or limitation is explicit
[ ] dependencies/assumptions are recorded
[ ] contradiction is preserved if present
[ ] raw secret material is not persisted
[ ] change layer that would invalidate it is identified
```

# Appendix D — Change Layer Examples

| Change | Likely layer(s) | Typical consequence |
|---|---|---|
| workload spike | runtime | assumption validity may change |
| code bug fix | implementation | affected behavioral evidence stale |
| database split/shard | architecture | topology/invariant evidence may need revalidation |
| new causal model for user state | model | claims derived under old model need transport |
| acceptance criterion redefined | specification | old passing tests may no longer prove the requirement |
| test harness semantics changed | verifier | verifier-bound evidence stale |
| policy gives agent broader production scope | governance | new authority requires independent review/evidence |
| agent changes process that approves policy changes | meta-governance | highest scrutiny; candidate cannot self-approve |

# Appendix E — Evaluation Dataset Template

For each shadow disagreement:

```yaml
case_id:
action_class:
existing_gate: allow|deny
cj_verdict:
cj_reason_codes: []
operator_classification:
  - useful_continuity_gap
  - useful_recovery_gap
  - missing_evidence_but_benign
  - modeling_error
  - false_positive
  - existing_gate_already_caught
remediation_taken:
time_to_resolve:
would_have_changed_outcome: yes|no|unknown
notes:
```

# Appendix F — Prior-Art / Standards Integration Notes

Builders should verify current versions before implementation. Useful external concepts include:

- W3C PROV for provenance relationships;
- OMG SACM for structured assurance arguments;
- SLSA and in-toto for software build provenance/attestation;
- TLA+ / Apalache for state-machine/property evidence;
- OPA/Cedar/capability systems for policy/effect authorization;
- OpenTelemetry for runtime observation;
- truth-maintenance/assumption-based reasoning for dependency invalidation;
- runtime assurance and safety-case literature for dynamic claim/evidence management.

Continuous Justification should consume or interoperate with these where useful, not claim to replace them.

# Appendix G — Repository Grounding Snapshot

As of the grounding pass used to write this PRD, public `main` showed:

- Rust + PostgreSQL + axum + tokio + sqlx;
- `src/runtime/` with `capability.rs`, `capability_nonce.rs`, `lifecycle.rs`, `maintenance.rs`, `tools.rs`, `wake_loop.rs`, sandbox/vault components;
- `src/models/` with agent/event/projection/workspace models;
- `src/api/` with agents/audit/events/health/messages/providers/webhooks/OpenAPI modules;
- broad integration tests including capability gate/nonce, audit chain, maintenance, lifecycle, prompt injection, sandbox, spec coverage, and wake loop;
- README describing continuous agents, append-only events, between-wakes maintenance, scoped single-use capability nonces, hash-chained event log, prompt-injection floor, sandbox and credential boundaries;
- `DELIVERY.md` and north-star docs that may be on different release narration points.

Fresh builders MUST re-check all of these. Do not code from this snapshot alone.
