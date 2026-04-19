---
name: build-discipline
description: "Execute BUILD, ITERATE, or verify-fix work in small, test-first vertical slices. Use for multi-file implementation, failing tests, root-cause debugging, scope-creep risk, or when maintainability matters."
user-invocable: false
---

# Build Discipline

Use this skill during BUILD, iteration builds, and any verify-fix cycle. It is the execution contract for turning the high-level BUILD phase into repeatable, evidence-driven implementation behavior.

## When to Use

- BUILD phase work that touches more than one file
- Fixing failures discovered by VERIFY
- Iteration work that re-enters BUILD
- Any change where scope creep, speculative abstractions, or debugging by guesswork are realistic risks

## Goals

- Keep each increment small enough to understand, test, and revert
- Prove behavior with tests before broadening scope
- Fix root causes instead of patching symptoms
- Leave a clear trail of what changed and what was deliberately left alone

## Pre-Flight for Medium/Large Work

Before the first slice, do two things when the work is bigger than a tiny change:

1. **Write a short slice plan** when there are more than about 3 acceptance criteria or the project is house/skyscraper tier.
   - List the slice order
   - Name the proof for each slice
   - Note likely files touched
2. **Check the source of truth** when a slice depends on an unfamiliar framework, library, or external integration.
   - Prefer official documentation or existing repo patterns over memory
   - Do this before locking in the implementation approach

## Procedure

1. **Choose one slice**
   - Pick the smallest end-to-end behavior that moves an acceptance criterion forward.
   - If the slice would touch more than about 5 files or cannot be described in 1-2 sentences, split it again.

2. **Anchor the slice to the spec**
   - Re-read the relevant part of `scaffolding/scope.md` and `scaffolding/design.md`.
   - State what this slice includes.
   - Record anything useful but out of scope as `Noticed, not touching`.

3. **Write proof first**
   - Add or extend the failing test that demonstrates the desired behavior.
   - For bug fixes, write the reproduction test before changing the code.

4. **Implement the simplest working change**
   - Prefer direct, obvious code over a reusable abstraction unless the third real use case already exists.
   - Match existing patterns before introducing a new one.
   - If design reality changes, update `scaffolding/design.md` rather than silently drifting.

5. **Run the verification ladder after every slice**
   - Parse / compile / typecheck
   - Focused test for the slice
   - Broader suite that covers nearby behavior
   - Runtime/manual check when the acceptance criterion requires real execution evidence

6. **Summarize before moving on**
   - `Changed`: the files and behaviors altered in this slice
   - `Not touched`: nearby areas intentionally left alone
   - `Concerns`: risks, follow-ups, or assumptions to watch

7. **Only then take the next slice**
   - Do not accumulate multiple half-finished slices.
   - If the current slice is not proven, stay on it.

## Debugging Loop

When a test, build, or runtime check fails:

1. Capture the exact failure output.
2. Reproduce it reliably.
3. Localize the failing layer or component.
4. Reduce the scenario to the smallest case that still fails.
5. State the root cause in one sentence.
6. Make the smallest fix that removes that root cause.
7. Keep or add the regression test.
8. Re-run targeted verification, then broader verification.

Do not guess. Do not stack unrelated fixes. Do not proceed to the next slice while the current failure is unresolved.

## Maintainability Rules

- Keep feature work and cleanup separate unless the cleanup is required for correctness.
- Prefer deletion or deferral over speculative generalization.
- Do not leave dead experiments half-wired into the codebase.
- If a name or control flow is hard to explain, simplify it before broadening the change.
- Do not silently add dependencies or architecture changes; justify them against the spec and design.

## Red Flags

- More than about 100 lines written before running verification
- Multiple unrelated concerns in one slice
- Tests added only after the code already "looks done"
- Fixes based on guesswork instead of a reproduced failure
- Silent design drift or dependency additions
- "While I'm here" edits outside the current acceptance criterion

If any red flag appears, shrink the slice, re-establish the failing proof, and continue from a known-good state.

## Rationalizations To Reject

| Rationalization                           | Better decision                                                                         |
| ----------------------------------------- | --------------------------------------------------------------------------------------- |
| "I'll test everything at the end."        | Small failures compound. Prove each slice before taking the next one.                   |
| "This change is too small for a test."    | Small behavior still needs evidence, and the test becomes future regression protection. |
| "I already know what the bug is."         | Reproduce it first or you may patch the symptom and miss the cause.                     |
| "This abstraction will be useful later."  | Build for the current requirement. Generalize after repeated real demand.               |
| "I'll clean it up after the gate passes." | Confusing code gets harder to clean up after more slices land on top of it.             |

## Exit Checklist

- [ ] The slice does one logical thing
- [ ] A test or reproduction existed before the fix
- [ ] Targeted verification passed
- [ ] Broader verification passed
- [ ] Scope stayed within the current criterion or documented design change
- [ ] `Changed / Not touched / Concerns` is clear enough for the next session to understand
