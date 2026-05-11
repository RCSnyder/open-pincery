# Opportunity Tree — v1-runtime

> **Revision 2 (2026-05-07, post open-pincery review)**: Original tree framed the desired outcome as "run lights-out-swe projects without a frontier-API vendor." After surfacing [RCSnyder/open-pincery](https://github.com/RCSnyder/open-pincery), the operative outcome is narrower:
>
> **Operative outcome (Revision 2)**: _Run my open-pincery agent fleet against an open coding-class model on a rented spot GPU, end-to-end, at lower cost per wake cycle than hosted-API equivalents._
>
> **In-scope branch**: a single opportunity — **"point open-pincery's `LLM_API_BASE_URL` at a self-leased GPU endpoint with a budget cap."** Single solution: thin SkyPilot+vLLM wrapper CLI. Single experiment: dogfood for one week of normal open-pincery use. All other branches (decouple-from-GHCP, generic-substrate, multi-protocol) are **out of scope** because they presume products this user does not need to build. See `wave-decisions.md` for the authoritative spec.

> Continuous-discovery opportunity tree per Teresa Torres. Outcome at the root, opportunities mid-tier, solutions/experiments at leaves. v1 in-scope branch is bolded; out-of-scope branches kept for traceability.

```
                                  DESIRED OUTCOME
              "Run my lights-out-swe projects to completion at commodity-GPU
               cost without using a frontier-API harness vendor"
                                        │
        ┌───────────────────────────────┼───────────────────────────────┐
        │                               │                               │
        ▼                               ▼                               ▼
  O1. Decouple harness          O2. Reduce per-build           O3. Increase trust
  from GHCP runtime             cost vs. GHCP                  in autonomous output
        │                               │                               │
        │                               │                               │
   ┌────┴────┐                  ┌───────┴───────┐               ┌───────┴────────┐
   ▼         ▼                  ▼               ▼               ▼                ▼
**S1.1**   S1.2              **S2.1**         S2.2          **S3.1**           S3.2
SkyPilot-  Local-only        Spot-GPU        Plan/exec      Faithful           Multi-run
backed     vLLM runner       on Vast/        split          phase-gate         consensus
ephemeral  (no remote)       RunPod          (cheap         provenance         (run N times,
GPU runner                   single-GPU      planner +       (same scaffold/   diff outputs)
                             open model      fat exec       log artifacts as
                                             on rented      reference Fire
                                             GPU)           Legasy run)


              ┌───────────────────────────────┴───────────────────────────────┐
              ▼                                                               ▼
        O4. Generalize across                                          O5. Improve harness
        cybernetic-loop classes                                        (lights-out-swe)
        (research, data, ops)                                          itself
              │                                                               │
              ▼                                                               ▼
            S4.1                                                           UPSTREAM
            Pluggable protocol pack                                        (out of scope —
            interface (NOT v1 ship,                                        belongs in
            but design-time concern)                                       lights-out-swe repo)
```

---

## Outcome (root)

> **"Run my lights-out-swe projects to completion at commodity-GPU cost without using a frontier-API harness vendor."**

Falsifiable success: Fire Legasy can be replayed end-to-end on a rented GPU with an open model, producing equivalent scaffolding artifacts, at <50% of the GHCP token-equivalent cost, in <8h wall-clock, with ≥30% per-run success rate.

---

## Opportunities (mid-tier)

### O1 — Decouple harness from GHCP runtime _(in scope, primary)_

The lights-out-swe protocol is tool-agnostic by design but only executes via GHCP today. This is the gap the project's existence is justified by. Author has documented the gap in their own README.

### O2 — Reduce per-build cost vs. GHCP _(in scope, secondary)_

Cost reduction is the _enabler_ of O1, not the wedge. If runs cost the same as GHCP, the user might still prefer GHCP for UX reasons. The cost claim must be verified during SPIKE.

### O3 — Increase trust in autonomous output _(in scope, tertiary)_

For lights-out runs to be hire-able, the artifacts must be inspectable and the run reproducible. Faithful phase-gate provenance (S3.1) is non-negotiable; multi-run consensus (S3.2) is a future enhancement.

### O4 — Generalize across cybernetic-loop classes _(out of scope for v1; design-time only)_

User's stated long-term ambition. Substrate must accommodate this without committing engineering to it in v1. Captured as a design constraint, not a v1 deliverable.

### O5 — Improve lights-out-swe itself _(out of scope; upstream)_

User noted the harness can be improved. Such improvements belong in the lights-out-swe repo, not here. Cross-cutting changes that benefit both repos may surface during SPIKE and should be PR'd upstream.

---

## Solutions / Experiments (leaves)

### S1.1 — SkyPilot-backed ephemeral GPU runner _(v1 ship)_

CLI provisions a spot GPU via SkyPilot, boots an open-model runtime + agent loop adapter, ingests project, runs lights-out-swe pipeline, streams artifacts back, tears down. Why SkyPilot: abstracts Vast/RunPod/Prime Intellect/Lambda as one substrate, provides budget caps and spot recovery natively, OSS, non-capturing.

### S1.2 — Local-only vLLM runner _(rejected for v1)_

Same harness, no remote provisioning. Rejected because user does not own a GPU large enough for coding-class open models, and "ephemeral spot" is the central economic claim.

### S2.1 — Spot single-GPU on Vast/RunPod with open model _(v1 ship via S1.1)_

Single-instance, single-GPU class (H100/H200 80GB). No multi-node orchestration in v1. Model selection deferred to SPIKE; candidates are coding-class open models that fit one GPU.

### S2.2 — Plan/execute split (cheap planner + fat executor) _(deferred)_

User's original ideation but admittedly never executed. Adding it to v1 multiplies surface area without evidence the plain runner is insufficient. Defer until S2.1 is measured; revisit if cost target unmet.

### S3.1 — Faithful phase-gate provenance _(v1 ship)_

Same `scaffolding/scope.md`, `design.md`, `readiness.md`, `log.md` artifacts produced as the GHCP reference run. Same git commit cadence at each gate. Diff against reference Fire Legasy as the acceptance test.

### S3.2 — Multi-run consensus _(deferred)_

Run N times, compare results; useful when single-run pass rate is below threshold. Out of v1; revisit in v2 if SPIKE shows pass rate <50%.

### S4.1 — Pluggable protocol pack interface _(design-time constraint, no v1 implementation)_

The Job interface (`{mission, context, harness_protocol, model_spec, budget}`) must be generic enough that future protocol packs (research-loop, data-pipeline-loop) are additive, not breaking. v1 ships exactly one pack (lights-out-swe). No second pack will be written in v1; the interface is validated by review, not by use.

---

## Branch Pruning Rationale

| Branch                      | In v1?      | Rationale                                                                 |
| --------------------------- | ----------- | ------------------------------------------------------------------------- |
| O1 / S1.1 / S2.1 / S3.1     | **Yes**     | Together they form the minimum viable runtime that can replay Fire Legasy |
| O4 / S4.1                   | Design only | Substrate must not preclude; engineering effort is zero in v1             |
| O2 / S2.2 (plan-exec split) | Deferred    | Optimization without evidence of need                                     |
| O3 / S3.2 (consensus)       | Deferred    | Optimization that activates only if pass rate is too low                  |
| O5 (harness improvements)   | Upstream    | Wrong repo                                                                |
| S1.2 (local-only)           | Rejected    | Eliminates the central economic mechanism                                 |

---

## Next Wave Linkage

The riskiest assumption underlying this tree is: **S2.1 + S3.1 is achievable with current open models on rented commodity GPUs.** This is precisely what `/nw-spike` will probe in the next wave — see `solution-testing.md` for the spike design.
