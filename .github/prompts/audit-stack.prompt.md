---
description: "Audit preferences.md stack choices against input docs for orthodox, idiomatic fit. Use before /expand when you want to validate that your technology choices match the problem domain."
agent: "agent"
argument-hint: "Optional: specific concern to audit (e.g., 'is Rust right for this?' or 'check deploy target')..."
---

Audit `preferences.md` against the problem described in `docs/input/` to check whether the stack choices are orthodox, idiomatic, and right-sized for the project.

## When to use

- Before `/expand` when starting a new project and you want a sanity check
- When input docs describe a problem domain you haven't built in before
- When you suspect the default stack in preferences.md might be over- or under-engineered for this problem
- When a client has strong opinions about technology and you want to validate them
- After `/distill` when the structured requirements reveal integration constraints you didn't anticipate

## Steps

1. Read `preferences.md` — extract the current stack preferences, deploy targets, and conventions
2. Read all files in `docs/input/` — understand the problem domain, integrations, constraints, scale, and user expectations
3. Infer the likely quality tier (shed / house / skyscraper) from the input docs

4. Evaluate the stack against these axes:

### Orthodoxy

Is this the **standard, boring choice** for this problem domain? What would a senior principal engineer at a serious company reach for?

- If input docs describe a CRUD web app and preferences say Rust + WASM → flag it
- If input docs describe a high-performance data pipeline and preferences say Python → note the tradeoff
- If input docs describe a simple CLI and preferences say full-stack framework → flag over-engineering
- Check whether the ecosystem for the chosen stack has mature libraries for the integrations described in input docs

### Idiomaticity

Do the conventions in preferences.md align with how the chosen stack's community actually works?

- Build tools (is `uv` standard for Python? Is `trunk` standard for Rust WASM? etc.)
- Project structure conventions
- Testing patterns
- Deployment patterns for the target platform

### Right-sizing

Is the stack complexity proportional to the quality tier?

- Shed → single language, minimal deps, simple deploy
- House → reasonable stack, automated deploy, standard infra
- Skyscraper → justified complexity, staged deploy, monitoring, the works

Flag if the stack is **over-engineered** for the tier (common) or **under-engineered** (rare but dangerous).

### Compatibility

Do the chosen components work well together?

- Language ↔ framework ↔ deploy target ↔ database ↔ auth provider
- Are there known friction points? (e.g., Rust + serverless cold starts, Python + WASM, etc.)
- Do the external integrations in the input docs have good library support in the chosen language?

### Deploy target fit

Does the deploy target match the application type?

- Static site → GitHub Pages ✓, Docker on VPS ✗ (overkill)
- API server → Docker on VPS ✓, GitHub Pages ✗ (impossible)
- CLI tool → binary release ✓, Docker ✗ (wrong model)
- Background worker → cron/queue ✓, GitHub Pages ✗

5. Produce an audit report:

```markdown
## Stack Audit — [date]

**Input docs reviewed**: [list files]
**Inferred quality tier**: [shed / house / skyscraper]
**Current stack**: [from preferences.md]

### Verdict: [GOOD FIT / MINOR CONCERNS / RECONSIDER]

### Orthodoxy

[Is this the boring, standard choice? What would the community reach for?]

### Right-sizing

[Is the complexity proportional to the tier?]

### Compatibility

[Do the components play well together? Library support for integrations?]

### Deploy Target

[Does the target match the application type?]

### Recommendations

- [Specific, actionable suggestions — or "No changes needed"]

### If changing stack

[Only if recommending a change: what to update in preferences.md and why]
```

6. Present the report to the user. **Do not modify preferences.md automatically** — stack changes are a human decision.

7. If the user agrees with changes, update preferences.md accordingly.

## Rules

- **Never auto-change preferences.md.** Present findings, let the user decide.
- **Bias toward the current stack.** Switching stacks has high cost. Only recommend a change if there's a clear mismatch, not just a marginally better option.
- **"Orthodox" means what the community uses**, not what's theoretically optimal. If 80% of Python web apps use FastAPI or Django, that's orthodox. If 3% use a custom async framework, that's not.
- **Consider the human.** If preferences.md reflects the user's expertise, the "orthodox" choice for them might differ from the industry default. A Rust expert building a web app in Rust is fine; a Python expert doing the same thing should probably use Python.
