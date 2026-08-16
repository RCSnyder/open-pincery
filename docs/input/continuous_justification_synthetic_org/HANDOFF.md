# Handoff: Synthetic Organization Bootstrap

## Primary input

Copy `PRODUCT_REQUIREMENTS_CONTINUOUS_JUSTIFICATION_AND_SYNTHETIC_ORGANIZATION.md` into `open-pincery/docs/input/` and run the normal Lights Out SWE input-distillation / EXPAND workflow.

## Critical build discipline

- Re-inspect current `main` first.
- Preserve all existing `AC-CJ-*` IDs.
- Preserve all new `AC-SO-*` IDs.
- Do not attempt all `AC-SO-*` criteria in one release. Map them to the staged SO-0 through SO-6 roadmap.
- Treat Section 52 equations marked `[SYNTHESIS]` as design/research inspiration, not established laws or literal implementation formulas.
- Treat Section 59 as a provenance ledger: read primary sources when a design decision materially depends on them.
- Continuous Justification is the first feature; Synthetic Organization is the program architecture.

## Suggested first autonomous command

Use `/lo-swe:distill` if available, then `/lo-swe:expand`. Require EXPAND to identify the smallest useful Continuous Justification release and explicitly defer later Synthetic Organization stages.
