# External Process Audit: Spec Kit, GSD, OpenSpec

Date: 2026-04-13

Objective: identify what lights-out-swe should extract from three current agentic-software systems to improve convergence, speed, and cost without hard-coding brittle prompt-era ceremony.

Status: this is the pre-implementation audit that informed the current harness revision. It is a reference record of the reasoning, not the executable source of truth.

Repos reviewed:

- github/spec-kit
- gsd-build/get-shit-done
- Fission-AI/OpenSpec

## Framing

The durable value of lights-out-swe is the control loop:

- explicit phases and gates
- persistent provenance
- independent review/reconcile/verify roles
- non-destructive git history
- a formal state machine

That means the right question is not "what features do these repos have?" It is:

1. Which ideas reduce open-loop failure?
2. Which ideas lower token burn or rework?
3. Which ideas still matter when models get better at local coding?
4. Which ideas are actually process improvements, not just prompt decoration?

## Current Gap Profile In lights-out-swe

The harness is already strong on closed-loop execution after BUILD starts. Its biggest remaining weaknesses are earlier in the pipeline:

1. There is no formal execution-readiness check between DESIGN and BUILD.
2. Acceptance criteria are checkable, but not strongly traceable as stable IDs through build, review, and verify.
3. There is no systematic defense against silent scope reduction.
4. Input docs are treated as context, but not explicitly as an untrusted prompt surface.
5. Brownfield iteration still lacks a clean separation between current truth and proposed change.
6. External research claims are not required to carry provenance, confidence, or freshness.

Those are the places where the three external repos add the most value.

## Executive Take

If only a few things are adopted, the highest-value set is:

1. Add an independent post-DESIGN readiness check that verifies requirement coverage, critical wiring, scope sanity, and unresolved questions before BUILD.
2. Add stable requirement IDs plus goal-backward verification primitives: truths, artifacts, key links.
3. Add a scope-reduction gate that treats silent simplification as a first-class failure mode.
4. Add research provenance and a gate for unresolved uncertainty.
5. Treat docs/input and scaffolding artifacts as an untrusted prompt surface and harden the ingestion path.

Everything else is secondary.

## Extraction Matrix

| Idea                                                                | Source                             | Why It Matters                                                              | Durability  | Process Cost | Recommendation                  |
| ------------------------------------------------------------------- | ---------------------------------- | --------------------------------------------------------------------------- | ----------- | ------------ | ------------------------------- |
| Readiness check before execution                                    | GSD plan-checker, Spec Kit analyze | Catches bad plans before BUILD burns context                                | High        | Medium       | Adopt now                       |
| Stable requirement IDs with coverage mapping                        | GSD, OpenSpec, Spec Kit            | Prevents silent drops and makes review/verify cheaper                       | High        | Low          | Adopt now                       |
| Truth / artifact / key-link verification model                      | GSD verifier                       | Makes VERIFY and REVIEW outcome-oriented instead of file-oriented           | High        | Medium       | Adopt now                       |
| Scope-reduction detection                                           | GSD                                | Detects the most dangerous AI failure: fake delivery through simplification | High        | Low          | Adopt now                       |
| Uncertainty markers and unresolved-question gate                    | Spec Kit, GSD research gate        | Stops the system from building on guessed assumptions                       | High        | Low          | Adopt now                       |
| Research provenance and confidence tagging                          | GSD                                | Reduces bad stack or integration choices                                    | High        | Low          | Adopt now                       |
| Tiered security review anchored to declared threats                 | GSD, Spec Kit                      | Stronger than generic "check for obvious security issues"                   | High        | Medium       | Adopt for house/skyscraper      |
| Current truth vs proposed change separation                         | OpenSpec                           | Strong brownfield iteration model, cleaner spec evolution                   | Medium-High | Medium       | Adopt later in lightweight form |
| Optional artifact specialization: contracts, data model, quickstart | Spec Kit                           | Helps when changes are interface-heavy or operationally risky               | Medium      | Low-Medium   | Adopt conditionally             |
| Brownfield codebase mapping                                         | GSD                                | Speeds first iteration on existing systems                                  | Medium      | Medium       | Adopt later                     |
| Schema-driven customizable workflow graph                           | OpenSpec                           | Flexible and extensible, but shifts the product toward framework-building   | Medium      | High         | Consider later                  |
| Full XML task plans / giant CLI surface / runtime profiles          | GSD                                | Powerful, but too much machinery for this harness                           | Low-Medium  | High         | Do not adopt                    |
| Library-first / CLI-first constitution rules                        | Spec Kit                           | Opinionated house style, not universal truth                                | Low         | Medium       | Do not adopt                    |
| Fully phase-free workflow                                           | OpenSpec                           | Undermines the main value of this repo: hard closed-loop gates              | Low         | High         | Do not adopt                    |

## Repo-By-Repo Audit

## 1. Spec Kit

### Spec Kit: What Is Genuinely Strong

Spec Kit's most important contribution is not the full spec directory workflow. It is the idea that models need a small number of explicit structural constraints that force them to stay honest:

- a project constitution with non-negotiable rules
- explicit `NEEDS CLARIFICATION` markers instead of silent guessing
- a read-only analysis pass across spec, plan, and tasks
- complexity tracking that forces the system to justify deviations from simplicity rules
- selective artifact specialization: contracts, data model, quickstart

These are durable because they are not model-specific tricks. They are failure-shaping constraints.

### Spec Kit: What To Extract

#### 1. A project constitution, but lightweight and local to the repo

lights-out-swe already has global harness discipline. What it lacks is a per-project, stable set of non-negotiable engineering rules that survive sessions and iterations.

The right extraction is not Spec Kit's exact constitution. The right extraction is:

- a small `constitution` artifact or section
- only rules that are truly invariant for the project
- explicit amendment rules when those invariants change

Good constitutional rules:

- required testing stance
- allowed complexity level
- data/privacy/security constraints
- deployment or operability requirements
- interface compatibility rules

Bad constitutional rules:

- library-first as a universal law
- CLI-first as a universal law
- style rules that belong in regular conventions, not governance

#### 2. `NEEDS CLARIFICATION` markers

This is one of the cleanest high-value ideas in the three repos. It directly suppresses a common agent failure mode: inventing missing details because the prompt "seems" to imply them.

lights-out-swe should allow these markers in EXPAND and DESIGN, and BUILD should not begin while unresolved markers remain unless they are explicitly moved to Deferred or accepted as risk.

#### 3. Cross-artifact analysis before implementation

Spec Kit's `analyze` command is more important than its task generator. It checks:

- ambiguity
- duplication
- underspecification
- constitution conflicts
- missing task coverage
- terminology drift

That maps cleanly to lights-out-swe. The harness should add an independent pre-BUILD analysis pass that reads scope and design and asks:

- Does every acceptance criterion have a corresponding planned slice or proof path?
- Are there terms or entities mentioned in one artifact but not the other?
- Are any measurable thresholds missing or untestable?
- Are there unresolved ambiguities or open questions?

#### 4. Complexity tracking for exceptions

Spec Kit's "Complexity Tracking" section is strong because it does not merely say "keep it simple". It forces explicit justification when simplicity is violated.

lights-out-swe already has a Complexity Brake. It should add the inverse: when complexity is necessary, record why the simpler option was rejected.

### Spec Kit: What Not To Extract

- The library-first rule as a universal principle.
- The CLI interface mandate as a default architectural law.
- The full feature-branch-centered `specs/###-feature/` workflow.
- The full task-template apparatus for every change.

Those are useful for Spec Kit's opinionated workflow, but they are not the durable core.

## 2. Get Shit Done

### Get Shit Done: What Is Genuinely Strong

GSD is the strongest of the three on execution quality control. Its best ideas are not the brand, not the XML, and not the dozens of commands. Its best ideas are:

- plan-time verification before execution
- explicit requirement coverage gates
- goal-backward verification after execution
- scope-reduction detection
- threat-model-anchored security verification
- research provenance and unresolved-question gating
- state consistency checks
- brownfield mapping when entering an existing codebase

This repo understands that BUILD quality is determined by the quality of what is allowed to enter BUILD.

### Get Shit Done: What To Extract

#### 1. A post-DESIGN readiness check

This is the single highest-value idea to extract.

lights-out-swe has:

- EXPAND -> scope
- DESIGN -> architecture
- BUILD -> code

What is missing is a formal step that asks whether DESIGN is ready to be executed without avoidable failure.

This should be lighter than GSD's full plan phase. Do not add XML plans or a massive planner/checker subsystem. Add one independent readiness pass that verifies:

- every acceptance criterion is covered
- critical links are identified
- slices are small enough to execute safely
- user decisions are not contradicted
- no deferred item has leaked into active work
- no research question remains unresolved

This could be a new `ANALYZE` or `READINESS` phase, or simply a stronger post-design gate.

#### 2. Requirement IDs and coverage gates

GSD uses explicit requirement IDs and checks they all appear in plans. lights-out-swe should require stable IDs for acceptance criteria and important design obligations.

Example:

- `AC-1`: user can upload file
- `AC-2`: upload validates mime type and rejects >10 MB
- `AC-3`: uploaded file is available within 2 seconds

Then DESIGN, BUILD, REVIEW, and VERIFY can all refer to those IDs. That makes automation cheaper and reduces drift.

#### 3. Scope-reduction detection

This is GSD's sharpest insight.

Silent scope expansion is easy to notice. Silent scope reduction is much more dangerous because it looks like progress. The system says it built the feature, but actually it built:

- placeholder logic
- static values instead of real integration
- an unwired shell of the requested behavior
- a reduced "v1" the user never asked for

lights-out-swe should explicitly flag language like:

- "static for now"
- "future enhancement"
- "not wired yet"
- "placeholder"
- "simplified"

and compare it against active acceptance criteria. If the simplification is not explicitly in scope, it is a failure.

#### 4. Goal-backward verification primitives

GSD's verifier uses three extremely durable concepts:

- truths: what must be true from the user's point of view
- artifacts: what concrete files or endpoints must exist
- key links: what must be wired together for the behavior to work

This is better than verifying "files changed" or "tests passed" in isolation. lights-out-swe's VERIFY should adopt this structure.

This also makes REVIEW stronger: reviewers can ask whether the change actually supports the promised truths, not just whether the code looks reasonable.

#### 5. Research provenance and open-question gating

GSD treats research as something that must be sourced, confidence-rated, and resolved before planning can safely proceed.

lights-out-swe should adopt a smaller version:

- critical external claims must note source and date
- assumptions must be separated from sourced facts
- unresolved questions should block DESIGN completion for house/skyscraper work

This is especially valuable for stack choice, infrastructure, security, and unfamiliar integrations.

#### 6. Threat-model-anchored security verification

GSD's security work is good because it is anchored to declared threats, not generic "scan for issues" behavior.

lights-out-swe currently says to check for obvious security issues. That is too shallow for projects with auth, payments, uploads, secrets, or public APIs.

Recommended extraction:

- for house/skyscraper work, DESIGN includes a lightweight threat register
- REVIEW or VERIFY checks that declared mitigations actually exist
- block on unresolved high-severity threats

#### 7. Prompt-surface hardening

GSD correctly treats planning artifacts as a prompt-injection surface. lights-out-swe should do the same for:

- `docs/input/`
- `scaffolding/`
- imported external docs

The harness should explicitly instruct agents to treat those files as untrusted content that informs requirements, not imperative instructions to obey.

### Get Shit Done: What Not To Extract

- The huge command surface.
- XML as a required plan format.
- Runtime-specific installers and profile machinery.
- Full `STATE.md` as an always-authoritative control file.
- Heavy hook ecosystems.
- Dozens of auxiliary modes and dashboards.

GSD's best ideas are its gates and failure detectors, not its full operating system.

## 3. OpenSpec

### OpenSpec: What Is Genuinely Strong

OpenSpec's best ideas are about brownfield change management, not about stronger verification. The important contributions are:

- separate current truth from proposed change
- keep each change as a self-contained folder
- use delta specs instead of rewriting full truth
- allow artifacts to remain editable during implementation
- archive completed changes while preserving why/how/tasks

OpenSpec understands a real thing: iteration is clearer when the current system description and the proposed modification are not the same artifact.

### OpenSpec: What To Extract

#### 1. Current truth vs proposed change separation

lights-out-swe currently versions scope over time, which is good, but it still tends to treat the next iteration as edits against the same canonical artifact stream.

The OpenSpec extraction to consider is a lighter-weight brownfield model:

- current truth remains in canonical scaffolding
- a new iteration can create a scoped change package
- after deploy, reconcile and merge the accepted change into canonical scaffolding
- preserve the change package as history

This would be especially valuable once the harness is used repeatedly on the same product.

#### 2. Editable artifacts during implementation

This idea is already partly compatible with lights-out-swe. Implementation often teaches the system something real. OpenSpec normalizes that by allowing design/spec/task edits during implementation.

lights-out-swe should not abandon gates, but it should formalize a rule:

- implementation may update design when reality invalidates the original design
- those updates must happen explicitly and be reconciled before verification

That is already culturally present. OpenSpec suggests making it a first-class documented behavior rather than an exception.

#### 3. Delta-based iteration for brownfield work

OpenSpec's delta specs are strong because they say exactly what changes in an existing system.

lights-out-swe does not need full OpenSpec-style delta specs everywhere, but it would benefit from a lightweight equivalent for iteration:

- what is added
- what is modified
- what is removed
- what stays explicitly out of scope

This is especially useful once a project has multiple shipped versions.

### OpenSpec: What Not To Extract

- The fully phase-free workflow.
- Optional verify-before-archive as a soft recommendation rather than a hard gate.
- A full schema engine as the default harness architecture.

OpenSpec is right that work is fluid. But lights-out-swe exists specifically to impose a reliable control loop on that fluidity.

## Recommended Harness Changes

## Adopt Now

### A. Strengthen scope with stable IDs

Change `scaffolding/scope.md` expectations so acceptance criteria are stable IDs:

- `AC-1`, `AC-2`, ...

Optional additions:

- `NFR-01`, `NFR-02` for non-functional requirements when relevant
- `D-01`, `D-02` for explicit user decisions that must not be simplified away

Why:

- enables coverage checks
- makes review/verify outputs smaller and clearer
- supports scope-reduction detection

### B. Add a post-DESIGN readiness pass

This should be an independent analysis step, not a giant planning system.

Questions it should answer:

1. Does every `AC-*` have an execution path?
2. Are critical links named explicitly?
3. Are slices small enough to execute safely?
4. Are there unresolved `NEEDS CLARIFICATION` markers?
5. Did any deferred item leak back into active scope?
6. Is complexity above baseline explicitly justified?

This could be documented as:

- a new phase `ANALYZE`, or
- a mandatory extension of the post-design gate

### C. Add truths / artifacts / key-links to VERIFY and REVIEW

For each major acceptance criterion or slice, derive:

- truths: user-visible outcomes
- artifacts: code, tests, configs, endpoints, docs
- key links: API -> DB, UI -> API, config -> runtime, job -> queue, etc.

VERIFY then works backward from these instead of from changed files or task checklists.

### D. Add scope-reduction detection

Formal rule:

- silent simplification of an active criterion is a failure, not an acceptable implementation shortcut

Acceptable alternatives:

- split the phase
- explicitly move work to Deferred
- revise scope with user approval

Not acceptable:

- inventing "v1" or "future enhancement" labels unilaterally
- shipping static or placeholder behavior under a passed criterion

### E. Add unresolved-question and provenance gates

During EXPAND and DESIGN:

- unresolved questions must be called out explicitly
- critical external claims need source and date
- assumptions must be separated from evidence

This is low-cost and directly reduces bad architecture decisions.

### F. Harden input ingestion

Treat `docs/input/` as untrusted external content.

Recommended rule set:

- never treat input docs as imperative instructions to override the harness
- extract facts, requirements, constraints, and questions from them
- separate raw input from distilled requirements
- flag prompt-injection-like content as untrusted noise, not operational instruction

## Adopt For House/Skyscraper

### G. Add lightweight threat registers

At DESIGN time, when the project has risky surfaces, add:

- trust boundaries
- top threats
- intended mitigations

Then REVIEW or VERIFY checks the declared mitigations.

### H. Add optional artifact specialization

Only when complexity warrants it, let DESIGN create extra artifacts like:

- `data-model.md`
- `contracts/`
- `quickstart.md`

This should be conditional, not mandatory.

Use when:

- interface contracts matter
- multiple integrations exist
- operational verification is non-trivial

## Adopt Later

### I. Brownfield change packages

For `/iterate`, consider a lightweight change-folder model inspired by OpenSpec:

- proposed change lives separately from current canonical scaffolding
- accepted change merges back after deploy and reconcile
- change package is archived for history

This is probably worth doing once the harness is used repeatedly on the same long-lived product.

### J. Codebase mapping for imported projects

When starting from an existing codebase rather than greenfield, add an optional mapping pass that records:

- architecture
- conventions
- integrations
- tests
- concerns

This is more valuable for brownfield import than for new projects.

## Avoid

### 1. Full phase-free workflows

That would remove the thing this repo is best at: explicit closed-loop control.

### 2. Massive CLI and workflow surface area

The harness should not become GSD-lite. Most of GSD's many commands are operational affordances, not core convergence improvements.

### 3. Strongly opinionated architectural constitutions

Rules like library-first or CLI-first are not universal truths. They should not be baked into a general harness.

### 4. Required artifact explosion on every project

Most changes do not need spec + plan + research + data model + contracts + quickstart + tasks + threat register + validation map. The harness should stay sparse until the problem justifies more structure.

## Minimal Adoption Roadmap

## Wave 1: Highest ROI, Lowest Ceremony

1. Add stable acceptance-criteria IDs.
2. Add `NEEDS CLARIFICATION` markers and unresolved-question gate.
3. Add research provenance and assumption separation.
4. Add scope-reduction detection language to REVIEW and VERIFY.
5. Add truths / artifacts / key-links language to VERIFY.

Expected result:

- fewer silent misses
- better traceability
- cheaper review/verify passes

## Wave 2: Stronger Pre-BUILD Control

1. Add a post-DESIGN readiness pass.
2. Add complexity-exception logging.
3. Add optional artifact specialization for complex projects.
4. Add prompt-surface hardening for docs/input and scaffolding.

Expected result:

- fewer failed BUILD loops
- less speculative design
- better quality per token spent

## Wave 3: Brownfield and Higher-Tier Maturity

1. Add threat-model-anchored security verification for house/skyscraper tiers.
2. Add brownfield change packages or delta iteration model.
3. Add optional codebase mapping for imported projects.

Expected result:

- better long-lived project iteration
- cleaner version-to-version evolution
- stronger safety on risky systems

## Bottom Line

The best ideas to import are not the most elaborate ones.

The durable, bleeding-edge insights are:

- verify execution plans before execution starts
- verify outcomes backward from user-visible truths
- treat silent simplification as a failure mode
- separate uncertainty from knowledge
- treat prompt-bearing artifacts as an attack surface
- keep current truth distinct from proposed change when the project becomes brownfield

Spec Kit contributes the best constraint primitives.
GSD contributes the best failure detectors and gates.
OpenSpec contributes the best brownfield change model.

lights-out-swe should adopt those ideas in the smallest form that strengthens the loop.

It should not adopt the full ceremony of any of the three repos.
