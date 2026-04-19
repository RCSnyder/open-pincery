---
description: "Reconcile scaffolding documents against the codebase. Detects and fixes drift between scope.md, design.md, readiness.md, log.md, and the actual code."
agent: "reconcile"
argument-hint: "Optional: specific concern or axis to focus on..."
---

Run the reconciliation agent to cross-check all scaffolding documents against the actual codebase.

Use this:

- After REVIEW, before VERIFY
- After resuming from a session drop
- After a BLOCKED → unblock cycle
- Whenever documents feel out of sync with the codebase
- After manual code edits outside the pipeline

The agent checks seven axes: directory structure, interfaces, acceptance criteria, external integrations, stack/deploy config, log accuracy, and readiness/traceability. It auto-fixes cosmetic and structural drift, and flags spec-violating drift for your decision.
