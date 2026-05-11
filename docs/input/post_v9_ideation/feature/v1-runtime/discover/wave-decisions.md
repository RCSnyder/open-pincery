# Wave Decisions — DISCOVER → DISCUSS Handoff

> Required summary per nWave DISCOVER skill. Every entry traces to evidence in `interview-log.md`, `problem-validation.md`, or repo evidence ([RCSnyder/lights-out-swe](https://github.com/RCSnyder/lights-out-swe), [RCSnyder/fire-legasy](https://github.com/RCSnyder/fire-legasy)).

---

> **Revision 2 (2026-05-07, post open-pincery review)**: Original framing as a standalone runtime substrate was retracted after the user surfaced [RCSnyder/open-pincery](https://github.com/RCSnyder/open-pincery) (v1.0.1 on crates.io). The credential vault (AC-38), secrets-by-reference (AC-43), capability nonces (AC-80), sandbox stack (AC-76/77), audit chain (AC-78), prompt-injection defense (AC-79), and continuous-agent runtime are already shipped there. This DISCOVER's product is now scoped as an **open-pincery subsystem**, not a new platform.

## Decisions

- **[D1]** Product is a **GPU-lease subsystem of open-pincery**: a thin tool a `pcy` agent (or operator) can use to bring up an open-model vLLM endpoint on a spot GPU, point a workspace's `LLM_API_BASE_URL` at it for a bounded window, and tear it down. Not a new agent platform; not a new harness; not a GHCP replacement.
- **[D2]** **No new agent runtime, no new vault, no new sandbox.** Inherits open-pincery's existing AC-38 vault, AC-43 PLACEHOLDER secret resolution, AC-80 capability nonces, AC-76/77 sandbox, AC-78 audit chain, AC-30s runtime budget caps. The new code does _one_ thing: provision → health-check → expose endpoint → tear down on lease expiry or budget exhaustion.
- **[D3]** Canonical benchmark: **run a real `pcy` agent against a leased vLLM pod for a wake cycle** that produces non-trivial output (e.g., a code-review wake on the open-pincery repo itself). Fire Legasy replay is now a _secondary_ validation, not the primary; primary is dogfooding inside open-pincery. (see: `interview-log.md` Q3.2)
- **[D4]** Ephemeral GPU substrate via **SkyPilot** (Vast/RunPod/Prime Intellect/Lambda abstracted). Rationale: spot recovery, budget caps, OSS, non-capturing. SkyPilot YAML lives in this repo; called from `pcy gpu lease` (new subcommand) or directly via CLI. (see: `lean-canvas.md` Solution)
- **[D5]** **Async/batch only**. No warm pool. Cold start (60–180s provision + 30–60s model load) acceptable per user. (see: `interview-log.md` Q2.3)
- **[D6]** **Hybrid model strategy**: open models on rented GPUs for long-running pcy agent fleets; user keeps frontier APIs (GHCP/Cursor) for hard interactive sessions. Not a GHCP replacement. (see: `interview-log.md` Q2.2)
- **[D7]** Sovereignty/privacy is a **side-benefit, not a wedge**. (see: `interview-log.md` Q1.4)
- **[D8]** Plan/execute split is **deferred indefinitely**. Open-pincery's wake/sleep cycle already amortizes one LLM call per useful work episode; the marginal value of plan/execute on top is unclear and unmeasured. (see: `solution-testing.md` R6)
- **[D9]** **NOT built via lights-out-swe.** This subsystem is intentionally low-ceremony: ad-hoc Rust or Python, ~500–1500 LOC, no AC empire, no separate scaffolding/, no separate TLA+ spec. Discipline mechanism: hard 1500-LOC budget, hard 2-week wall-clock budget. If the implementation exceeds either, stop and reassess scope rather than expand the harness. **This is the scope-discipline correction the user identified as the real bottleneck.**
- **[D10]** Feature slug remains `v1-runtime`. Repo name remains `remote-agent-assistent`. May later move into open-pincery's tree as `crates/pcy-gpu-lease/` if the boundary feels artificial. (see: `interview-log.md` Q4.2)
- **[D11]** **Deliverables of this subsystem (v1 contract)**:
  1.  `rar lease <gpu-class> --budget=<usd> --duration=<minutes>` — provisions, prints `LLM_API_BASE_URL` and a teardown handle.
  2.  `rar release <handle>` — explicit teardown.
  3.  `rar status <handle>` — health, spend-so-far, time-remaining.
  4.  SkyPilot YAML templates for at least one (model, GPU class) combo.
  5.  README documenting how to wire a leased endpoint into an open-pincery workspace's `.env`.
      That is the v1 product. Everything else is deferred.

---

## Constraints

- **[C1]** **Solo founder, single stakeholder.** Scope discipline is non-negotiable. The pattern of starting new repos to escape harness-driven scope inflation is itself a scope-discipline failure mode. This subsystem must NOT recapitulate vault/sandbox/audit/agent-platform work that already exists in open-pincery.
- **[C2]** **Market-of-one is fine for this scope.** This is a personal-tool subsystem of an existing personal platform. Customer-development with external users is not gated by this DISCOVER.
- **[C3]** **Hard implementation budget**: 1500 LOC, 2 weeks wall-clock, $200 GPU spend. If any cap is hit, **stop and reassess** rather than expand. This is the explicit corrective mechanism for the scope-inflation pattern observed in open-pincery's v6→v9 trajectory (AC-37 → AC-88+ in 3 weeks).
- **[C4]** **No local GPU sufficient for coding-class open models.** Confirms the rented-GPU mechanism is the only path.
- **[C5]** **Must not require modifying open-pincery, lights-out-swe, or fire-legasy.** All three are upstream. Any change those repos need to consume the leased endpoint is upstream work, not subsumed here. The simplest contract is: this tool produces an OpenAI-compatible URL + key; open-pincery's existing `LLM_API_BASE_URL` mechanism consumes it.
- **[C6]** **Single GPU class in v1** (one H100/H200 80GB instance). No multi-GPU, no multi-node.
- **[C7]** **Cannot beat hyperscalers** on substrate breadth. Differentiation is irrelevant at this scope — this is internal infrastructure for one user's existing platform, not a positioned product.
- **[C8]** **Built ad-hoc, not via lights-out-swe.** The harness is excellent for greenfield SWE projects with broad scope. It is the wrong tool for tightly-bounded internal infrastructure. Using it here would re-trigger the AC-inflation pattern. (Evidence: open-pincery v6→v9 trajectory.)

---

## Validated Assumptions

- **[VA0]** **The agent platform exists and is shipped.** Open-pincery v1.0.1 on crates.io with vault, sandbox, audit chain, capability nonces, multi-agent messaging, continuous-agent runtime, 321 passing tests. The original DISCOVER framing of "build a runtime substrate" was based on incomplete context; the substrate already exists. Confidence: **High**. Evidence: [RCSnyder/open-pincery](https://github.com/RCSnyder/open-pincery), [PR #4](https://github.com/RCSnyder/open-pincery/pull/4) v9 security push.
- **[VA1]** **LLM cost pain is real, recurring, and dollar-quantified.** Confidence: **High**. Evidence: $9 single-query, daily/weekly GHCP rate-limit hits, 27× Opus 4.7 price hike. (`interview-log.md` Round 1)
- **[VA2]** **User will not downgrade harness UX.** Confidence: **High**. Evidence: zero workarounds tried despite cost pain; explicit love of state-of-the-art harnesses. Implication: any solution must preserve harness-grade behavior. (`interview-log.md` Q1.3, Q2.1)
- **[VA3]** **Async/batch is the actual mode** of the desired tool, not real-time. Confidence: **High**. Evidence: explicit selection of "cold start irrelevant; batch into long sessions". (`interview-log.md` Q2.3)
- **[VA4]** **lights-out-swe protocol is the input format.** Confidence: **High**. Evidence: user's verbatim description of inputs (TLA+ + docs + harness instructions) matches the lights-out-swe `docs/input/` + `preferences.md` + `.github/copilot-instructions.md` structure exactly. (`interview-log.md` Q2.5; repo evidence)
- **[VA5]** **User can ship autonomous loops.** Confidence: **High**. Evidence: Fire Legasy is a deployed, production application built end-to-end through the lights-out-swe harness. (`interview-log.md` Q3.2; firelegasy.com)
- **[VA6]** **The harness-runtime decoupling gap is real and acknowledged by the harness's own author.** Confidence: **High**. Evidence: lights-out-swe README literally documents the gap as future work. (Repo evidence)

---

## Invalidated Assumptions

- **[IA0]** _"This project requires a new agent platform / runtime / vault / sandbox stack."_ **Invalidated.** Open-pincery already provides all of these. Reinventing them in a new repo would be the founder-trap pattern of recapitulating prior work to escape scope friction. The actual new code needed is a thin GPU-lease tool that integrates with open-pincery's existing `LLM_API_BASE_URL` mechanism. (Evidence: open-pincery README + PR #4 inventory of shipped ACs.)
- **[IA1]** _"Sovereignty / private agent harness is a buying motivation."_ **Invalidated.** Evidence: user describes it as "important" but cannot cite any past dollar or opportunity cost; sovereignty has not changed any decision they've made. Demoted from wedge to side-benefit. (`interview-log.md` Q1.4)
- **[IA2]** _"User is competing with / replacing GHCP for interactive coding."_ **Invalidated.** Evidence: explicit statement that user "absolutely loves" current harnesses and will keep using frontier APIs for hard problems. The new tool is complementary, not substitutive. (`interview-log.md` Q2.1, Q2.2)
- **[IA3]** _"Plan/execute split is a workflow the user already practices and just needs tooling for."_ **Invalidated.** Evidence: user explicitly says "no, I was ideating" — the split is a hypothesis derived from first principles, not past behavior. Deferred from v1. (`interview-log.md` Q2.4)
- **[IA4]** _"There is a customer segment beyond the founder ready to be served."_ **Invalidated for v1.** Evidence: zero named individuals, zero customer-development conversations, lights-out-swe at 0 stars. May validate later, but v1 is positioned as personal tool. (`interview-log.md` Q1.5; repo metadata)
- **[IA5]** _"v1 should ship a generic agentic-loop runtime supporting multiple loop classes from day one."_ **Invalidated as scope.** Evidence: user has past behavior on exactly one loop class (SWE via lights-out-swe). Generalization without examples to abstract from is the founder trap. Resolution: substrate stays generic by interface (`Job` shape); v1 ships exactly one protocol pack. (`interview-log.md` Q4.1, Q4.3)
- **[IA6]** _"Cold-start latency is a constraint to engineer around."_ **Invalidated.** Evidence: explicit user selection that cold-start is irrelevant due to batch usage pattern. Removes a major engineering area (warm pools, pre-warming) from v1 scope. (`interview-log.md` Q2.3)

---

## Gate Status

| Gate                                                  | Status | Note                                                        |
| ----------------------------------------------------- | ------ | ----------------------------------------------------------- |
| G1 — Decisions have rationale entries                 | ✅     | All D1–D10 cite evidence sources                            |
| G2 — Constraints have evidence sources                | ✅     | All C1–C7 cite evidence                                     |
| G3 — Validated assumptions have confidence levels     | ✅     | All VA1–VA6 stated; all "High" with cited past behavior     |
| G4 — Invalidated assumptions have evidence references | ✅     | All IA1–IA6 cite specific interview rounds or repo evidence |

---

## Handoff

**To**: implementation. **No further nWave waves at this scope.**

This subsystem is too small to justify the full nWave pipeline. The DISCOVER artifacts in this folder are the entire pre-implementation specification. Skip DISCUSS, SPIKE, DESIGN, DEVOPS, DISTILL, DELIVER as formal waves — they are appropriate for greenfield products, not for a 1500-LOC, 2-week internal-infrastructure subsystem.

**Implementation plan** (concrete, no AC inflation):

1. **Day 1–2**: SkyPilot YAML for one (model, GPU) combo (e.g., Qwen3-Coder-480B on H200 80GB via Vast.ai spot). Hand-test: can it boot vLLM and serve `/v1/chat/completions`?
2. **Day 3–4**: Thin Rust or Python CLI wrapping `sky launch` / `sky down` with a budget cap. Three commands: `lease`, `status`, `release`.
3. **Day 5**: Health-check loop and budget-watchdog (auto-`release` on cap or duration).
4. **Day 6–7**: Wire into open-pincery: README section showing a `pcy` workspace using a leased endpoint, run a real wake cycle, observe events in the Postgres event log.
5. **Day 8–10**: Dogfood. Run open-pincery against the leased endpoint for one week of normal use. Measure cost vs. hosted-API equivalent. Document failure modes.

**Done = product success criteria**:

- [ ] At least one full open-pincery wake cycle completes against a leased endpoint, end-to-end
- [ ] Lease auto-tears-down when budget cap reached, observed at least once
- [ ] Cost per wake cycle (open-model on leased GPU) < cost per equivalent hosted-API wake cycle, measured over ≥5 wakes
- [ ] LOC ≤ 1500 (hard cap; if exceeded, stop and revisit scope)
- [ ] Time-to-first-working-lease ≤ 14 days from start (hard cap)

**Explicitly NOT done by this subsystem**:

- Multi-protocol support (lights-out-swe replay, generic Job interface). Open-pincery is the agent platform; this is its GPU power supply.
- Plan/execute split (deferred per D8).
- Multi-cloud routing optimization (SkyPilot already does this).
- Web UI / dashboard. CLI is sufficient for one user.

**Scope-discipline tripwires** (the corrective mechanism):

- If you find yourself adding ACs, stop.
- If you find yourself writing TLA+ for this, stop.
- If you find yourself spawning the lights-out-swe harness on this repo, stop.
- If you find yourself reimplementing a vault or audit log, stop — use open-pincery's.
- If LOC trends past 1000, stop and ask whether the remaining 500 is real product or feature creep.
