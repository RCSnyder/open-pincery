# Interview Log — v1-runtime DISCOVER

**Subject**: R. Cooper Snyder (project owner, sole stakeholder)
**Method**: Mom Test (past-behavior questioning, no future-intent claims accepted as evidence)
**Date**: 2026-05-07
**Format**: 3 question rounds + repo evidence review
**Interview depth**: comprehensive

---

## Round 1 — Problem & Pain Validation

### Q1.1 — Current LLM API spend (last 30 days) and top tasks driving it

> "opus 4.7 is going up 27x and i spent $9 on 1 query in ghcp. harnesses are going to token based pricing. i need to be closest to the hardware, but still have agentic harness powers"

**Annotation**: Strong past-behavior evidence. `$9/query` is a concrete artifact, not a projection. `27×` price increase on Opus 4.7 is a real catalyst. "Closest to the hardware, agentic harness powers" is the value statement to preserve through the rest of the interview.

### Q1.2 — Most recent incident where cost/lock-in changed a decision

> "yes i run into daily and weekly usage limits in ghcp"

**Annotation**: Recurring (daily/weekly) friction. Pain is sustained, not one-off.

### Q1.3 — Workarounds tried (Ollama, rented GPU, SkyPilot, cheaper tier, batching, nothing)

> Selected: "Nothing yet — still on hosted APIs" / "using github copilot in vscode agent mode"

**Annotation**: **Critical signal.** User complains about cost but has not tried a single cheaper alternative. Two readings:

1. Harness UX is so much better than DIY that the user refuses to downgrade — implies the moat is _agent-loop quality_, not GPU access.
2. DIY friction is too high to attempt — implies the gap is a usable bridge, not a new model or new GPU layer.

Both readings point away from "cost optimization tool" and toward "harness-quality experience without harness-vendor lock-in."

### Q1.4 — Has sovereignty (private/sovereign) ever cost a real dollar or opportunity?

> "i think its important to not have to rely on 3 companies for intelligence work"

**Annotation**: Future-intent / philosophical language ("I think it's important"). Mom Test fails this as buying motivation. **Demoted from wedge to side-benefit.**

### Q1.5 — Specific people with the same pain

> "just me and anyone else trapped in the large llm providers ecosystem"

**Annotation**: Zero named individuals. Has not had a customer-development conversation. Project is **a market of one** until proven otherwise. Valid for a personal tool; not yet validated as a market product.

---

## Round 2 — Quality Floor & Workflow Reality

### Q2.1 — 1–2 specific tasks in GHCP that cost most / hit limits most

> "no i absolutely love the state of the art agentic harnesses"

**Annotation**: User reaffirms harness UX is non-negotiable. **Confirms reading #1 of Q1.3.**

### Q2.2 — Would you accept ~85% of Opus quality at ~10% cost for daily work?

> "they will eventually get there. i will probably still use state of the art for hard problems to see whats possible"

**Annotation**: Hybrid model accepted. Frontier APIs retained for hard interactive problems. **This means the new tool is not a GHCP replacement; it's a complement for a specific workload class.** Reframe forced.

### Q2.3 — Cold-start tolerance

> Selected: "I'd batch work into long sessions; cold start is irrelevant"

**Annotation**: **Critical architectural signal.** Cold start being irrelevant means:

- Spot instances are acceptable (no warm pool tax)
- Workload is async/batched, not interactive
- This is **not** a real-time agent harness. It's a fire-and-forget batch runner.

### Q2.4 — Have you ever manually drafted a plan in one LLM and executed in another?

> "no i was ideating, might need some thinking through, sometimes i go back and forth with a model iterating on something and sometimes i have a spec and i want to do darkfactory loop work fully automated, so i think being able to deliver that consistently, as deterministically as possible on commodity hardware… i essentially want to have cheaper dark factory loops for SWE work and dont want to use the large providers, or make sure i can build my own"

**Annotation**: **The reframe lands here.** "Dark factory loop" = lights-out manufacturing metaphor. User is describing autonomous, batched, deterministic-as-possible execution on commodity hardware. This is **not** an interactive harness; it is a runtime for spec-driven autonomous loops.

The plan/execute split was **ideation, not past behavior** — should not be load-bearing in v1.

### Q2.5 — Kill criterion

> "i can send in a tla+ state machine, some input docs, agentic harness instructions and get working software or an message back with current state of everything and a message. so i can iterate on it or prompt and send it back to the system. and its robust and cheaper / allows for more usage than ghcp or any other providers"

**Annotation**: Concrete, measurable, falsifiable. The contract is:

```
Input:  TLA+ state machine + input docs + harness instructions
Output: working software OR { state, message }
Constraint: more usage / cheaper than GHCP, robust
```

This is the v1 acceptance contract. It also reveals user already has the input format in mind — TLA+ + docs + harness instructions, which is **exactly** the lights-out-swe input format.

---

## Round 3 — Past Behavior on Autonomous Loops & Investment

### Q3.1 — Have you successfully run an agentic loop end-to-end without intervention?

> "https://github.com/RCSnyder/lights-out-swe yeah this repo has the idea in it okay. Also the swe idea is a good economic thing, but generically this can be any task btw so i want it generic like current agent systems. like send a mission + context and get state + message back"

**Annotation**: **Decisive.** User has built and shipped `lights-out-swe`, a complete autonomous SWE harness with TLA+ state machine, gated phases, restricted-tool agents, and persistent provenance. The harness exists. Generalization instinct is correct _in the abstract_ — `mission + context → state + message` — but past behavior covers exactly one loop class (SWE).

### Q3.2 — First real spec to feed it

> "i built this repo with the lights-out-swe https://github.com/RCSnyder/fire-legasy"

**Annotation**: Fire Legasy is a shipped, production-deployed snake game (live at firelegasy.com), built end-to-end through the lights-out-swe harness on GHCP agent mode. **This is the canonical benchmark workload for the v1 spike** — replay this build on a rented GPU with an open model and measure pass rate, time, cost.

### Q3.3 — Investment budget before kill

> Selected: "Open-ended — this is my main project"

**Annotation**: Maximum commitment. Reduces budget as a variable; raises the bar on rigor of the kill criterion (must be self-imposed).

---

## Round 4 — Reframe Confirmation

### Q4.1 — Does "remote-agent-assistant = runtime substrate for lights-out-swe" capture intent?

> "yes mostly, because the lights-out-swe harness can be improved as well. and any sort of class of problems that can be defined in a cybernetic loop like that needs a generic gpu agentic loop system"

**Annotation**: Partial yes. User wants the substrate to be generic across cybernetic-loop classes, not coupled to lights-out-swe. Tension noted.

### Q4.2 — Feature-id slug

> "i named it remote-agent-assistant"

**Annotation**: Project name only; no feature slug chosen. Folder convention requires one. **Decision**: `v1-runtime` for the first DISCOVER cycle.

### Q4.3 — Minimum useful v1

> "well look through the industry, stuff liek this exists, and google also has agentic systems for ephemerally spinning up their stack, https://blog.cloudflare.com/project-think/ so does aws. i think i'd like a generic version. idk what v1 ought to be"

**Annotation**: User correctly identifies that ephemeral-agent systems exist at hyperscale (Cloudflare Project Think, AWS, Google). Concedes uncertainty on v1 scope. Generalization instinct present but not backed by past behavior on multiple loop classes.

**Resolution applied**: Specialize the application, generalize the substrate. v1 ships one protocol pack (lights-out-swe) over a generic Job interface. Future loops plug in without rewriting. This is documented as Decision D5 in `wave-decisions.md`.

---

## Repo Evidence Reviewed

- **[RCSnyder/lights-out-swe](https://github.com/RCSnyder/lights-out-swe)** — Complete autonomous SWE harness. TLA+ state machine, 9-phase pipeline, restricted-tool agents, persistent scaffolding, BEE-OS discipline. Currently runs on GHCP agent mode in VS Code. Author's own README explicitly notes the gap: _"VS Code can't programmatically start Copilot chats, so the human opens each window and says 'go.' CLI agents (Claude Code, Codex) could be fully scripted from a coordinator terminal."_ — `remote-agent-assistant` is the answer to this future-work question.
- **[RCSnyder/fire-legasy](https://github.com/RCSnyder/fire-legasy)** — Real, deployed proof of harness viability (TS+Python+Postgres+Caddy+Docker, live at firelegasy.com). Built autonomously through the harness. **Designated benchmark workload** for v1 spike.

---

## Mom Test Compliance Audit

| Round | Past behavior cited                                                                                     | Future intent (excluded)           | Decision impact                                                    |
| ----- | ------------------------------------------------------------------------------------------------------- | ---------------------------------- | ------------------------------------------------------------------ |
| 1     | $9/query, daily limits, $0 tried elsewhere                                                              | "I think sovereignty is important" | Cost real; sovereignty demoted                                     |
| 2     | Loves harness, won't downgrade, cold-start irrelevant                                                   | "Open models will get there"       | Reframed to async/batch substrate                                  |
| 3     | Built+shipped lights-out-swe + Fire Legasy                                                              | "any task generic"                 | Anchor to proven loop; generalize substrate only                   |
| 4     | n/a                                                                                                     | n/a                                | Resolution: specialize app, generalize substrate                   |
| 5     | Open-pincery v1.0.1 shipped on crates.io with vault, sandbox, audit chain, capability nonces, 321 tests | "feel a bit lost in the sauce"     | **Reframe: this is an open-pincery subsystem, not a new platform** |

**Compliance**: All product decisions back-referenced to past behavior. Future-intent statements logged but excluded from the wedge.

---

## Round 5 — Post open-pincery surface (2026-05-07)

**Context**: User surfaced a third repo not previously in scope: [RCSnyder/open-pincery](https://github.com/RCSnyder/open-pincery), a Rust multi-agent platform v1.0.1 published on crates.io. Inventory of what's already shipped:

- AC-38: AES-256-GCM credential vault
- AC-43: PLACEHOLDER credential resolution (= secrets-by-reference)
- AC-80: capability nonces (single-use, time-bounded, workspace-scoped)
- AC-76 / AC-77: bubblewrap + seccomp + landlock sandbox
- AC-78: hash-chained audit log with startup gate
- AC-79: prompt-injection defense with canary tokens
- Continuous agents with CAS wake/sleep, async inter-agent messaging
- TLA+ formal spec, Postgres event store, `pcy` CLI
- 321 passing tests, 88+ ACs across 9 versions in ~3 weeks

The original DISCOVER framing of "build a runtime substrate" was authored without this context. Every architectural concern raised in Round 1\u20134 (vault, secrets-by-reference, sub-job budgets, multi-agent spawning, sandbox) is already partially or fully solved upstream.

### Q5.1 \u2014 Diagnosis

> **Buddy**: Is the real bottleneck scope discipline, not missing tooling? Open-pincery is on a v6\u2192v9 trajectory adding ~50 ACs in 3 weeks; starting `remote-agent-assistent` as a separate repo is a way of escaping the harness's scope-lock by opening a new one.

> **User**: "yeah partially i hope its solid, but it doesn't feel complete as a product yet and i feel a bit lost in the sauce"

**Annotation**: User confirms the scope-discipline diagnosis (partially), and surfaces the real underlying anxiety: **product incompleteness**. Open-pincery v1.0.1 is shipped on crates.io but does not feel like a product because there is no named user using it for a named job. The harness can ship code forever; it can't manufacture product-market fit. lights-out-swe README itself documents that `Ideating` and `ValidatingPMF` are the two irreducibly-human states the harness cannot do for the founder. **This is the actual unresolved problem, and `remote-agent-assistent` does not solve it.**

### Q5.2 \u2014 Product placement

> **Buddy**: Three options: (A) kill the separate repo, add GPU-lease feature inside open-pincery; (B) deliberate scope-reset \u2014 tiny tool, no harness, weekend of work; (C) ditch prior repos, this is the real attempt.

> **User**: "i guess something that can run inside open-pincery"

**Annotation**: Selection is essentially Option A with hedging (\"i guess\"). Operative reading: this repo's output should plug into open-pincery rather than compete with it. May later be moved into open-pincery's tree as `crates/pcy-gpu-lease/`. Code in this repo for now; boundary remains negotiable.

### Q5.3 \u2014 Disposition of the stale DISCOVER artifacts

> **Buddy**: Mark them v0/superseded? Edit them to reflect Option A or B? Delete? Leave alone for now?

> **User**: \"Edit them to reflect Option A or B\"

**Annotation**: User chose minimal-disruption editing. Implementation: each artifact gets a Revision-2 banner that supersedes the operative framing while preserving the historical record. `wave-decisions.md` is the canonical spec post-revision. The other documents' bodies remain as evidence of how the framing evolved.

### Round 5 Outputs

- **Reframe**: Product is an open-pincery GPU-lease subsystem, not a runtime substrate, not a generic Job interface, not a competing platform.
- **Hard scope discipline**: 1500 LOC, 2 weeks wall-clock, $200 GPU spend. Tripwires documented in `wave-decisions.md` Handoff section.
- **Skip remaining waves**: DISCUSS / SPIKE / DESIGN / DEVOPS / DISTILL / DELIVER are not appropriate for a 1500-LOC internal-infrastructure subsystem. Implementation plan in `wave-decisions.md` is the entire spec.
- **Unresolved \u2014 not for this DISCOVER**: the \"lost in the sauce\" / product-incompleteness signal. This is a customer-development problem, not a tooling problem. Surfaces a future investigation: who is open-pincery for, and have you talked to them yet?
