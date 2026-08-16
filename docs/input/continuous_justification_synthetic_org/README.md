# Continuous Justification + Synthetic Organization Bootstrap Handoff

This bundle is intended for fresh autonomous coding agents.

## Contents

- `PRODUCT_REQUIREMENTS_CONTINUOUS_JUSTIFICATION_AND_SYNTHETIC_ORGANIZATION.md` — authoritative Open Pincery input PRD; copy into `docs/input/`.
- `RESEARCH_CANON.md` — extracted research/provenance ledger from the PRD for easier reading/distillation.
- `HANDOFF.md` — minimal instructions for the Lights Out SWE pipeline.
- `QA.json` — structural QA counts/digest for the PRD.

## Recommended build entry

1. Re-inspect current `open-pincery/main`.
2. Place the PRD in `docs/input/`.
3. Run `/lo-swe:distill` if available.
4. Run `/lo-swe:expand` and require a staged scope: ship Continuous Justification first; preserve later `AC-SO-*` criteria as explicit deferred program milestones.
5. Do not implement equations marked `[SYNTHESIS]` literally unless a later design document turns them into a tested requirement.
