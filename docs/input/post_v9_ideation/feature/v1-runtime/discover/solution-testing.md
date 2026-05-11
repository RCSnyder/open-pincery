# Solution Testing — v1-runtime

> **Revision 2 (2026-05-07, post open-pincery review)**: Original solution-test framing assumed building a runtime substrate. After surfacing [RCSnyder/open-pincery](https://github.com/RCSnyder/open-pincery), most original risks (R2: "can OSS agent runtimes be adapted?", R5: "does the harness execute faithfully?") are **moot** — open-pincery IS the agent runtime, already shipped. Operative risks collapse to:
>
> **R1' (open-model viability on rented GPU)**: Can Qwen3-Coder-480B (or comparable) on a single H100/H200 spot instance via vLLM serve open-pincery wake cycles end-to-end without crashing or producing unusable output? **Test**: 1–2 day spike. Boot vLLM, point an open-pincery workspace at it, run one real wake cycle. Pass = events appear in event log, output is non-empty and parseable.
>
> **R2' (cost vs. hosted API)**: Is per-wake-cycle cost on the leased GPU lower than the hosted-API equivalent over a representative sample? **Test**: dogfood for 1 week of normal open-pincery use, log spend, compare against equivalent hosted-API calls. Pass = leased ≤ hosted, with explicit margin.
>
> **R3' (lease tear-down robustness)**: Does the budget-watchdog actually tear down a runaway lease before exceeding the cap? **Test**: deliberately throttle health-check responses, observe auto-`release`. Pass = no lease exceeds cap by >10%.
>
> Total spike budget: **$200, 2 weeks wall-clock, 1500 LOC**. If any cap is hit, stop and reassess. The original (R1–R6) framing below is preserved for traceability but is no longer the operative test plan. See `wave-decisions.md` for the authoritative spec.

> Risk-ranked assumption testing for the DISCOVER → SPIKE handoff. Each riskiest assumption gets a falsifiable test. Tests are ordered to fail-fast on the cheapest, highest-risk items.

---

## Riskiest Assumptions (ranked)

| #      | Assumption                                                                                                                                                                                                                                                                                  | If false, project...                                                                                  | Confidence                             | Test wave                                      |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- | -------------------------------------- | ---------------------------------------------- |
| **R1** | An open coding-class model on a single rented GPU can faithfully execute the lights-out-swe gated-phase protocol at ≥30% pass rate on Fire-Legasy-class workloads                                                                                                                           | ...is non-viable. No CLI polish, no substrate work, no abstraction recovers from this. **HARD KILL.** | Unknown                                | SPIKE                                          |
| **R2** | An OSS agent-loop runtime (Aider / OpenHands / SWE-agent / Cline / custom) can be configured or lightly adapted to execute lights-out-swe's protocol semantics (`.copilot-instructions.md`, `.prompt.md`, `.agent.md`, restricted-tool agents, gate retries) without a from-scratch rewrite | ...requires building a harness runtime from scratch — too large for solo. Probable kill.              | Unknown                                | SPIKE                                          |
| **R3** | Per-build economics on spot GPUs beat GHCP token equivalent for the same workload at the user's required ceiling (target: <50% of GHCP cost)                                                                                                                                                | ...the cost claim collapses; user will probably stay on GHCP despite UX irritation. **SOFT KILL.**    | Unknown                                | SPIKE (free byproduct of R1 test)              |
| **R4** | Project-state transport (project dir up, artifacts down) does not dominate cost or wall-clock                                                                                                                                                                                               | ...optimization work needed but probably not fatal                                                    | Medium                                 | SPIKE (free byproduct of R1 test)              |
| **R5** | A generic `Job = {mission, context, harness_protocol, model_spec, budget}` abstraction can accommodate future cybernetic-loop classes without breaking changes                                                                                                                              | ...v2 generalization requires substrate rewrite                                                       | Low (we can review-validate, not test) | DESIGN wave                                    |
| **R6** | Plan/execute split is a meaningful cost lever vs. plain agent loop                                                                                                                                                                                                                          | ...no impact (deferred from v1)                                                                       | Low                                    | Deferred — only revisit if R3 fails marginally |

---

## R1 + R2 + R3 + R4 → ONE SPIKE EXPERIMENT

Single experiment kills four risks because they are coupled. Run the experiment once with multiple model/runtime combinations to maximize information per dollar.

### Experiment: "Replay Fire Legasy on rented GPU"

**Reference**: the existing [RCSnyder/fire-legasy](https://github.com/RCSnyder/fire-legasy) repo and its `scaffolding/` artifacts. The original GHCP-driven build is the golden reference.

**Setup**:

1. Fresh empty repo, lights-out-swe template applied
2. Identical `preferences.md` and `docs/input/` as Fire Legasy v1
3. Identical "build me" prompt
4. Single H100/H200 80GB rented via SkyPilot on cheapest available spot provider
5. Open coding-class model loaded via vLLM (model TBD — pre-spike, evaluate Qwen3-Coder-480B variants, DeepSeek-V4, Llama-4 coding-tuned; pick top 2 by recent SWE-bench / Aider leaderboard)
6. Agent-loop runtime: best-fit OSS option (start with OpenHands or Aider; evaluate fit before launch)
7. Hard budget cap: $30 per run, $200 total spike spend

**Procedure**:

- Run N=5 attempts per (model, runtime) combination — 2 combinations max — total N=10 runs
- Each run executes EXPAND through VERIFY (DEPLOY can be stubbed to local Docker)
- Capture for each run: wall-clock per phase, $ spent, scaffolding artifacts produced, gate pass/fail history, final state, did software run

**Falsification thresholds**:

| Metric                                                                             | Threshold       | Resulting decision                                                                  |
| ---------------------------------------------------------------------------------- | --------------- | ----------------------------------------------------------------------------------- |
| ≥3/10 runs produce a working game (pass rate ≥30%)                                 | Pass            | Proceed to DESIGN wave                                                              |
| 1–2/10 runs pass                                                                   | Marginal        | Spike a second iteration with better model / harness adapter; if still <30% → kill  |
| 0/10 runs pass                                                                     | Hard fail       | Kill project; pivot or abandon                                                      |
| Median per-build cost <50% of GHCP estimate                                        | Pass on R3      | Cost claim validated                                                                |
| Median per-build cost 50–100% of GHCP                                              | Marginal R3     | Investigate plan/execute split (R6) before kill                                     |
| Median per-build cost ≥ GHCP                                                       | Fail R3         | Soft kill — UX gain alone unlikely to motivate switch                               |
| Transport overhead >20% wall-clock or >30% cost                                    | R4 partial fail | Optimization work needed; not fatal                                                 |
| At least one OSS agent runtime executes the protocol with <2 weeks of adapter work | Pass on R2      | Proceed                                                                             |
| All OSS runtimes require >2 weeks of work                                          | R2 partial fail | Re-evaluate scope; possibly pivot to "lights-out-swe protocol port" as v0.5 instead |

---

## R5 → REVIEW-BASED VALIDATION (no experiment)

Cannot be empirically tested in v1 (only one protocol pack exists). Mitigated by:

- Documenting the `Job` interface as part of DESIGN wave
- Reviewer (DESIGN wave) checks that the interface is _not_ coupled to lights-out-swe specifics (no SWE-only fields, no scaffolding-specific assumptions in the runtime)
- Architecture decision records track every place where lights-out-swe leaks into the substrate; each is either justified or planned to be moved into the protocol pack

---

## R6 → DEFERRED

No experiment in DISCOVER or SPIKE. Activation rule: revisit only if R3 spike result is in the marginal band (50–100% of GHCP). If R3 passes outright, plan/execute split is unnecessary complexity in v1.

---

## Spike Budget & Stop Rules

- **Time box**: 1 week elapsed from SPIKE start
- **Money box**: $200 total GPU spend
- **Hard stop**: any of the kill thresholds above triggered
- **Soft stop**: time/money exhausted before N=10 runs complete; report what was learned, decide on iteration vs. pivot

This is the single most important wave of the project. Everything downstream depends on R1 passing. **Do not skip the spike.** Do not let R5/R6 enter v1 before R1 passes.

---

## Pre-Spike Checklist (DISCUSS wave deliverable)

Before SPIKE can run, the next wave (DISCUSS) must produce:

1. Acceptance criteria for the spike runs (what counts as "Fire Legasy passed"? Reference scaffolding artifacts to diff against)
2. Model shortlist with selection rationale (~2 candidates)
3. OSS agent-loop runtime shortlist with fit assessment (~2 candidates)
4. SkyPilot YAML or equivalent provisioning script (skeleton)
5. Cost-tracking instrumentation (must be in place before first run)

These are DISCUSS / SPIKE concerns, not DISCOVER. Listed here only to make the handoff explicit.
