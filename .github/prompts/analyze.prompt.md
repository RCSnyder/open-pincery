---
description: "Analyze phase. Turns scope.md and design.md into readiness.md with traceability, truths, build order, and scope-risk checks before BUILD."
agent: "analyze"
---

Read `scaffolding/scope.md` and `scaffolding/design.md`. Produce `scaffolding/readiness.md`.

## Purpose

ANALYZE is the pre-BUILD admission gate. BUILD should start only after the plan is concrete enough that each `AC-*` has:

- a clear implementation target
- a planned test or proof path
- a runtime verification path
- any ambiguity or scope-reduction risk called out explicitly

## Steps

1. Read `scaffolding/scope.md` fully
2. Read `scaffolding/design.md` fully
3. Read `preferences.md` if it exists
4. Read relevant `docs/input/` materials when they materially affect acceptance criteria meaning, integrations, or constraints
5. Produce `scaffolding/readiness.md` with these exact sections:

### readiness.md format

```markdown
# Readiness: [Project Name]

## Verdict

[READY / NOT READY]

## Truths

- [T-1] [Non-negotiable statement that must be true in the shipped system]

## Key Links

- [L-1] [AC-*] -> [design component or interface] -> [planned test or artifact] -> [runtime proof]

## Acceptance Criteria Coverage

| AC ID | Build Slice | Planned Test | Planned Runtime Proof | Notes |
| ----- | ----------- | ------------ | --------------------- | ----- |

## Scope Reduction Risks

- [Risk that could tempt BUILD to ship a shell, placeholder, or weakened behavior. If none, write "None."]

## Clarifications Needed

- [Question or bounded assumption that still matters for truthful delivery. If none, write "None."]

## Build Order

1. [AC-*] [Why it goes first]

## Complexity Exceptions

- [Justified exception carried forward from design.md, or "None."]
```

6. Run the **post-analyze gate**:
   - [ ] `scaffolding/readiness.md` exists
   - [ ] Has a `Verdict` of `READY`
   - [ ] Every `AC-*` from scope.md appears in `Acceptance Criteria Coverage`
   - [ ] Every `AC-*` has a planned test and planned runtime proof
   - [ ] `Truths` and `Clarifications Needed` are separated
   - [ ] `Scope Reduction Risks` is explicit (it may say `None.`)
   - [ ] `Build Order` covers all `AC-*` items
   - [ ] `Complexity Exceptions` is explicit (it may say `None.`)

7. If any gate condition fails, fix it and recheck.

8. Log the result to `scaffolding/log.md`:

```markdown
## ANALYZE — [timestamp]

- **Gate**: PASS (attempt N)
- **Evidence**: [which AC IDs were traced, what truths and risks were captured]
- **Changes**: scaffolding/readiness.md created
- **Retries**: [total gate attempts this phase]
- **Next**: BUILD
```

9. Git checkpoint:

   ```
   git add -A && git commit -m "docs(analyze): prepare build readiness for [project]" -m "[summarize AC coverage, truths, and build order]\nGate: post-analyze PASS (attempt N)."
   ```

10. **Auto-continue to BUILD** (unless user specified stepped mode).

## Rules

- Do not invent new scope. If a needed behavior is not in scope.md, surface it under `Scope Reduction Risks` or `Clarifications Needed`.
- If a clarification would change pass/fail semantics for an `AC-*`, the verdict is `NOT READY` until the scope is corrected.
- Keep readiness.md lean. It is an admission control artifact, not another design essay.
