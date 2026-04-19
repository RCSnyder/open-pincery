---
description: "Build phase. Takes scope.md, design.md, and readiness.md and writes the actual code, tests, and deployment config in vertical slices."
agent: "agent"
---

Read `scaffolding/scope.md`, `scaffolding/design.md`, and `scaffolding/readiness.md`. Write the actual software.

## Steps

1. Read all three scaffolding docs fully
2. Read `preferences.md` if it exists
3. Load and follow `.github/skills/build-discipline/SKILL.md` throughout BUILD. It is the default workflow for slice sizing, test-first implementation, debugging, and change summaries.
4. If `scaffolding/readiness.md` is missing, materially out of sync with `scaffolding/scope.md` or `scaffolding/design.md`, or says the project is not ready to build, stop and run `/analyze` before writing code.
5. **Write the integration/e2e test skeleton first** — before any implementation. This is the oracle that guides all subsequent work:
   - Create the test file(s) with one test case per acceptance criterion ID (`AC-*`)
   - Each test should call the real entry point (CLI, API endpoint, function) and assert the expected outcome
   - Name test cases, fixtures, comments, or table rows with the corresponding `AC-*` identifier so traceability survives refactors
   - **For external integrations**: use the test strategy declared in design.md (mock / recorded / live). If design.md says "mock," set up fakes. If "recorded," use record-replay fixtures. If "live," tests call the real API (mark these as integration tests that can be skipped offline).
   - Tests will fail initially — that's the point. They define "done."
6. Build in **vertical slices** — get one acceptance criterion working end-to-end before moving to the next:
   a. Pick the most foundational `AC-*` from `scaffolding/readiness.md`
   b. **Before writing new code, read existing code in the project.** Match the style, patterns, naming conventions, and error handling approach already established. Consistency across vertical slices matters — especially when sessions restart and a different model instance continues the work.
   c. Keep the slice small enough to verify quickly. If it would touch more than about 5 files or you write more than about 100 lines before checking the verification ladder, split it again.
   d. Write the code for it
   e. **Verification ladder**: Does it compile? → Does the unit work? → Does the test pass?
   f. Record a brief handoff note before moving on:
   - `AC`: which `AC-*` this slice addressed
   - `Changed`: what this slice altered
   - `Not touched`: nearby code intentionally left alone
   - `Concerns`: risks, follow-ups, or scope-reduction pressure worth watching
     g. Only move to the next criterion after this one passes
     h. Repeat
7. Follow the directory structure from design.md exactly
8. **Scope lock**: Only build what's in scope.md. If you think something else is needed, note it under "## Deferred" in scope.md and move on.
9. **No silent scope reduction**: Do not satisfy an acceptance criterion with placeholder text, mocked behavior, dead-end navigation, or other shells unless scope/design explicitly says that is acceptable. If an `AC-*` cannot be completed honestly within the current plan, stop and surface the issue under `## Clarifications Needed` or `## Deferred` rather than quietly weakening the feature.

## Internal QRSPI (think, don't document)

For each vertical slice:

- **Questions**: What do I need to understand?
- **Research**: Look at APIs, docs, existing code
- **Design**: How should this piece work?
- **Structure**: What files/functions?
- **Plan**: Do it

## After all slices are built

### Create project-specific agents (house/skyscraper only)

If the quality tier is **house** or **skyscraper**, evaluate whether the architecture has clear roles that would benefit from custom agents in `.github/agents/`. Only create agents when a role is obvious from the code — don't speculate.

Each `.agent.md` file should have:

- A focused `description` with trigger phrases for discovery
- Minimal `tools` — only what the role needs
- Clear constraints on what the agent should NOT do

Common patterns:

- **Explorer**: read-only, knows module boundaries (`tools: [read, search]`)
- **Test writer**: knows test patterns, fixtures, conventions (`tools: [read, edit, search]`)
- **Reviewer**: knows acceptance criteria, audits against spec (`tools: [read, search]`)
- **Ops**: knows deploy target, runbook, rollback (`tools: [read, search, execute]`)

These agents are **project code** — they live alongside scaffolding as permanent project artifacts.

Run the **post-build gate**:

- [ ] Code compiles / typechecks (run the build, check for errors)
- [ ] Every `AC-*` from scope.md has a corresponding test and traceable proof path
- [ ] All tests pass
- [ ] No secrets/credentials in source code
- [ ] Dependency audit passes — run the appropriate command (`uvx pip-audit`, `npm audit`, `cargo audit`, etc.) and confirm no high/critical vulnerabilities
- [ ] Lockfile exists if project has dependencies (`uv.lock`, `Cargo.lock`, `package-lock.json`, etc.)
- [ ] Code follows design.md directory structure and interfaces (manual check — reconcile agent runs next for deeper audit)
- [ ] No `AC-*` is marked complete via placeholder or knowingly reduced behavior

If any gate condition fails, fix it and recheck. Up to 3 retries.

Log the result to `scaffolding/log.md`:

```markdown
## BUILD — [timestamp]

- **Gate**: PASS (attempt N)
- **Evidence**: [list each acceptance criterion with pass/fail]
- **Changes**: [files created/modified]
- **Retries**: [total gate attempts this phase]
- **Next**: REVIEW
```

Git checkpoint:

```
git add -A && git commit -m "feat(build): implement [project] core functionality" -m "[list acceptance criteria with pass/fail status]\nGate: post-build PASS (attempt N)."
```

**Auto-continue to REVIEW** unless the user specified stepped mode.

## Debugging Protocol

When a test or build fails, **diagnose before fixing**. Never blindly retry.

1. **Observe**: Capture the exact failure output.
2. **Reproduce**: Make the failure happen reliably before changing code.
3. **Localize and reduce**: Narrow the failing layer and reduce to the smallest case that still fails.
4. **Document**: State the root cause in one sentence before writing any fix.
5. **Fix**: Make the minimal change that removes that root cause.
6. **Guard**: Keep or add the regression test so the same failure cannot slip back in.
7. **Verify**: Re-run the targeted failing check, then broader verification.

If you cannot identify the root cause after 3 analysis attempts, STOP and report.

## Red Flags

- More than about 100 lines written before verification
- Multiple unrelated concerns mixed into one slice
- Tests added only after the code already appears complete
- Silent design drift or dependency changes
- "While I'm here" edits outside the current acceptance criterion

If you hit a red flag, shrink the slice and re-establish proof before continuing.

## Living Design Document

`scaffolding/design.md` is a **living document**. If implementation reveals that the design needs to change (e.g., an interface doesn't work as specified, a dependency behaves differently than expected, or performance requirements force an architectural change):

1. Update `design.md` to reflect the actual architecture
2. Note what changed and why in the commit message
3. Do NOT silently diverge from the design — the design must always match the code

## Database Migration Safety

If the project has a data model (scope.md Data Model ≠ "none"), follow these rules for any schema change:

- **Backward-compatible only**: Every migration must work with the previous version of the code still running. Never assume instant cutover.
- **Paired migrations**: Write both `up` and `down` (rollback) migrations. If rolling back is truly impossible, document why.
- **Never destructive in the same release**: Do not `DROP` a column/table in the same release that removes the code using it. Sequence: (1) deploy code that stops using the column, (2) next release drops the column.
- **Data backfill as separate step**: If a migration needs to backfill data, write it as a separate migration that runs after the schema change.
- **Test migrations**: Run the migration against a copy of the data (or a representative fixture) before deploying. Verify both `up` and `down`.

These rules apply at all tiers. A shed with SQLite has the same data integrity concerns as a skyscraper with Postgres.

## Rules

- Build working software, not perfect software
- If something is harder than expected, simplify the approach — don't gold-plate
- Simplifying the implementation is allowed; simplifying the promised outcome is not. Any change to promised outcome belongs in scope.md, not hidden in code.
- If you get stuck on a technical problem for more than 3 attempts, STOP and tell the user what's blocking
- Commit logical chunks — don't write everything then commit once
