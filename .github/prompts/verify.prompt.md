---
description: "Verify phase. Run all tests, verify acceptance criteria, check security, confirm deployment readiness."
agent: "verify"
---

Verify the built software against the spec.

## Evaluator Mindset

You are now the **evaluator**, not the builder. Assume the build has bugs until proven otherwise. Agents consistently over-approve their own work — resist this tendency.

- **Be skeptical by default.** "It looks like it works" is not evidence. Run it and check.
- **Probe edge cases**, not just the happy path. If a feature handles normal input, try unusual input.
- **Do not talk yourself out of real issues.** If something feels off, investigate. Do not rationalize it away.
- Grade against the acceptance criteria literally — if the criterion says "< 200ms" and it takes 250ms, that's a fail, not "close enough."

## Steps

1. Read `scaffolding/scope.md` for acceptance criteria, `scaffolding/design.md` for architecture, and `scaffolding/readiness.md` when it exists for truths, key links, and planned runtime proofs
2. Run all tests. Record exact command, exit code, pass/fail count, any failure output.
3. **Audit test quality** — read the test files and check that tests are non-trivial:
   - Tests assert meaningful properties (not just `assert True` or `is not None`)
   - Tests exercise real code paths (not just mocks testing mocks)
   - Tests actually verify what the acceptance criterion states (timing tests for latency criteria, output validation for correctness criteria)
   - Tests retain `AC-*` traceability so the verifier can tell which criterion is being proven
   - At least 2 criteria have edge case tests beyond the happy path
   - If tests are vacuous: FAIL with specific examples of what's wrong
4. **Actually exercise the software** — verify each acceptance criterion with real evidence:
   - **CLI tool**: Run it with representative input, check output matches expected
   - **Web API**: `curl` or equivalent HTTP request to each endpoint, check response status + body
   - **Web UI / SPA**: Run locally and use Playwright (`uv run playwright ...`) to load pages, check elements exist, interact with controls
   - **Data pipeline**: Run with sample data, verify output files/database state
   - **Cron/script**: Execute once manually, check side effects
   - Use the planned runtime proof from `scaffolding/readiness.md` when it exists
   - For each `AC-*`, record the **exact command run** and **exact output** as evidence
   - Verify the readiness `Truths` as part of this exercise. If a truth cannot be checked directly, say why.
5. Security check:
   - `grep -r` for common secret patterns (API_KEY, SECRET, password, token) in source
   - If web: no obvious XSS, SQL injection, or CSRF
   - Dependencies are from known sources
   - If auth exists: check it actually works
6. **Frontend quality check** (if the project has a web UI):
   - Run Lighthouse CI against the local dev server — report Performance, Accessibility, Best Practices, SEO scores
   - Run axe-core if available — flag accessibility violations
   - Any score below 80 is a concern to report
   - Aesthetic/design criteria (typography, color, layout) require human review — note them as "human review recommended" rather than auto-passing
7. Check deployment config exists and matches the target from scope.md
8. **Load/concurrency check** (if acceptance criteria include throughput, concurrency, or latency-under-load requirements):
   - Use a lightweight load tool (`hey`, `wrk`, `k6`, `ab`, or language-native equivalent) against the local dev server
   - Run at the concurrency level stated in the acceptance criteria (or 10 concurrent users as a baseline if no specific number)
   - Record: requests/sec, p50/p95/p99 latency, error rate
   - If any acceptance criterion has a latency or throughput threshold, verify it holds under concurrent load — not just single-request
   - If no concurrency criteria exist, skip this step

## Verify Agent Handoff

This prompt delegates to the `verify` agent, which has `tools: [read, search, execute]` — it can run tests and exercise the app but **cannot edit source code**.

If the verify agent finds failures:

1. The verify agent produces a **Verification Report** with exact reproduction steps
2. Control returns to the **main agent** (with edit capability) to fix the issues
   - Before fixing anything, load `.github/skills/build-discipline/SKILL.md`
   - Fix one reproduced failure at a time using the skill's root-cause debugging loop
   - Re-run the targeted failing check before returning to the full verify pass
3. After fixes, the verify agent re-runs verification
4. This cycle repeats up to 3 times before escalating to BLOCKED

**If the verify-fix cycle changes code significantly** (new files, interface changes, architecture adjustments), re-run the reconcile agent before the final verify pass to ensure scaffolding docs still match the code.

## Post-Verify Gate

- [ ] All tests pass
- [ ] Tests are non-trivial (verify agent confirms tests exercise real code paths with meaningful assertions)
- [ ] Application runs locally without errors
- [ ] Every `AC-*` is verified, explicitly failed, or explicitly blocked with evidence
- [ ] At least one acceptance criterion verified by actually running the app
- [ ] No critical security issues found
- [ ] Deployment config exists and looks correct

If any gate condition fails, the verify agent reports failures. The **main agent** fixes them, then the verify agent re-checks. Up to 3 retries.

Log the result to `scaffolding/log.md`:

```markdown
## VERIFY — [timestamp]

- **Gate**: PASS (attempt N)
- **Evidence**: [list each `AC-*`: ✓ verified / ✗ failed / ? blocked, plus truth checks]
- **Changes**: [any fixes applied during verify-fix cycle]
- **Retries**: [total gate attempts this phase]
- **Next**: DEPLOY
```

Git checkpoint:

```
git add -A && git commit -m "test(verify): all acceptance criteria verified" -m "[list each AC-*: \u2713 verified / \u2717 failed / ? blocked]\nGate: post-verify PASS (attempt N)."
```

**Auto-continue to DEPLOY** (unless user specified stepped mode).
