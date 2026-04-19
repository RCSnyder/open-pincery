---
description: "Design phase. Takes scope.md and produces design.md with architecture, directory structure, interfaces, integration details, and justified complexity exceptions."
agent: "agent"
---

Read `scaffolding/scope.md`. Produce `scaffolding/design.md`.

## Steps

1. Read `scaffolding/scope.md` fully
2. Read `preferences.md` if it exists (for stack conventions)
3. Produce `scaffolding/design.md` with these exact sections:

### design.md format

````markdown
# Design: [Project Name]

## Architecture

[How the pieces fit together. Include a simple ASCII diagram if the system has more than 2 components.]

## Directory Structure

[The actual file tree you'll create at the repo root. Be specific.]

```
├── src/
│   └── ...
├── tests/
├── scaffolding/
├── README.md
└── ...
```

## Interfaces

[Key data shapes, API contracts, module boundaries. At least one concrete type/shape.]

## External Integrations

[What this talks to outside itself. For each:

- What it is
- How you call it
- What happens when it fails
- **Test strategy**: mock (fake responses) / recorded (record-replay) / live (real API calls in tests)]

[Or "None — self-contained" if applicable.]

## Observability

[What needs logging, monitoring, or tracing. What would you check first if this breaks at 2am?

- **Shed**: Structured logging to stdout. Errors include context (what input caused it, what state was expected).
- **House**: Structured logging to Loki + Grafana alerting on error patterns. OpenTelemetry traces for request flows. Correlation IDs. Health endpoint.
- **Skyscraper**: OpenTelemetry instrumentation (traces + metrics + logs). Prometheus + Loki + Grafana dashboards. Alerting on SLO breach.]

## Complexity Exceptions

[Any justified place where BUILD may need to exceed the normal slice/file limits, share a cross-cutting abstraction, or stage work across multiple `AC-*` items. If none, write "None."]

## Open Questions

[Anything uncertain. Resolve these before building. Or "None — straightforward."]
````

4. **Design Review** (house/skyscraper only — skip for sheds):
   Walk through 2-3 key scenarios from the acceptance criteria against the design. For each:
   - Trace the data/control flow through the architecture
   - Identify where it could fail or degrade
   - Note any concerns (critical / major / minor)

   If critical issues are found, fix the design before proceeding. Append findings to design.md under a `## Design Review` section.

5. Run the **post-design gate**:
   - [ ] `scaffolding/design.md` exists
   - [ ] Has "Directory Structure" section
   - [ ] Has "Interfaces" section with at least one data shape
   - [ ] Every external integration has error handling noted
   - [ ] Every external integration has a test strategy declared (mock / recorded / live)
   - [ ] Has "Observability" section
   - [ ] Has "Complexity Exceptions" section (it may say `None.`)
   - [ ] No open questions remain unresolved (or explicitly deferred with rationale)
   - [ ] Design review completed (house/skyscraper) or skipped (shed) with rationale

6. If any gate condition fails, fix it and recheck.

7. Log the result to `scaffolding/log.md`:

```markdown
## DESIGN — [timestamp]

- **Gate**: PASS (attempt N)
- **Evidence**: [what was checked — directory structure, interfaces, integration test strategies]
- **Changes**: scaffolding/design.md created
- **Retries**: [total gate attempts this phase]
- **Next**: ANALYZE
```

8. Git checkpoint:

   ```
   git add -A && git commit -m "docs(design): architecture for [project]" -m "[summarize architecture, key interfaces, integration count]\nGate: post-design PASS (attempt N)."
   ```

9. **Auto-continue to ANALYZE** (unless user specified stepped mode).

```
✓ DESIGN complete. Gate passed.
Ready for ANALYZE. Continue?
```

## Rules

- Design should be pragmatic, not theoretical
- Match the scale to the project — don't over-architect a shed
- If an `AC-*` appears to require unusual complexity, record that explicitly under `## Complexity Exceptions` so BUILD does not discover it accidentally.
- If scope.md says "GitHub Pages," don't design a Kubernetes deployment
