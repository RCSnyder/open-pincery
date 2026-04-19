---
description: "Review phase. Audit built code for correctness, readability, architecture, security, and performance before reconcile and verify."
agent: "review"
---

Run the review agent to audit the built code before reconciliation and verification.

## Purpose

BUILD proves the software works. REVIEW checks whether it is safe to keep.

Use REVIEW to catch issues that tests often miss:

- over-engineered or confusing code
- hidden architecture drift
- dead code after refactors
- security weaknesses that still pass tests
- obvious performance hazards

## Steps

1. Read `scaffolding/scope.md`, `scaffolding/design.md`, and `scaffolding/readiness.md` if it exists
2. Run the `review` agent
3. The review agent audits tests first, then implementation, across five axes:
   - correctness
   - readability and simplicity
   - architecture
   - security
   - performance
     It also explicitly checks for scope reduction, placeholder behavior, and broken `AC-*` traceability.
4. If the review agent reports **Critical** or **Required** findings:
   - Control returns to the main agent
   - Before editing, load `.github/skills/build-discipline/SKILL.md`
   - Fix one blocking finding at a time
   - Re-run the targeted proof for that finding
   - Re-run REVIEW
5. If the review agent reports only `Consider` or `FYI`, proceed

## Post-Review Gate

- [ ] No `Critical` review findings remain
- [ ] No `Required` review findings remain
- [ ] Any BUILD evidence invalidated by review-fix work has been re-run
- [ ] Dead code, dependency, and maintainability concerns are either resolved or explicitly documented
- [ ] No unapproved scope reduction, placeholder behavior, or broken `AC-*` traceability remains

If any gate condition fails, fix the issue and re-run REVIEW. Up to 3 retries.

Log the result to `scaffolding/log.md`:

```markdown
## REVIEW — [timestamp]

- **Gate**: PASS (attempt N)
- **Evidence**: [summary of review scope and findings]
- **Changes**: [files modified during review-fix cycle, if any]
- **Retries**: [total gate attempts this phase]
- **Next**: RECONCILE
```

Git checkpoint:

```text
git add -A && git commit -m "docs(review): record multi-axis review pass" -m "Review axes: correctness, readability, architecture, security, performance. Gate: post-review PASS (attempt N)."
```

**Auto-continue to RECONCILE** unless the user specified stepped mode.
