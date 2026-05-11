# Problem Validation — v1-runtime

> **Revision 2 (2026-05-07, post open-pincery review)**: This document was authored before [RCSnyder/open-pincery](https://github.com/RCSnyder/open-pincery) (v1.0.1 on crates.io) was in scope. Open-pincery is the user's shipped multi-agent platform with credential vault (AC-38), secrets-by-reference (AC-43), capability nonces (AC-80), sandbox (AC-76/77), audit chain (AC-78), prompt-injection defense (AC-79), 321 passing tests. The ORIGINAL JTBD below ("runtime substrate for lights-out-swe") is **superseded**. The operative product is now an **open-pincery GPU-lease subsystem**.
>
> **Operative JTBD (Revision 2)**: _When my open-pincery agent fleet needs to run for hours/days on real work, I want to point its `LLM_API_BASE_URL` at an open coding-class model on a rented spot GPU — provisioned by one CLI command, with a hard budget cap, and torn down automatically — so I can keep using the platform I already shipped without paying frontier-API token prices on every wake cycle._
>
> Pain evidence below remains valid. Anti-patterns below remain valid. The "runtime substrate" framing is retracted; see `wave-decisions.md` for the authoritative spec.

---

## Job-To-Be-Done (JTBD)

> When I have a well-specified piece of work expressed as a cybernetic loop (mission + context + harness instructions), I want to fire it to a self-hosted open-model runtime on rented commodity GPUs as an autonomous batch job, so I get back working artifacts (or a precise blocker) without paying frontier-API token prices and without staying at the keyboard.

**Trigger context**: User has a project specified in lights-out-swe format (TLA+ state machine, `docs/input/`, `preferences.md`, scoped acceptance criteria) and wants to run it lights-out, but the only runtime that executes the harness today is GHCP agent mode in VS Code, which is rate-limited, costly, and human-driven (one window per agent session).

**Hire criteria** (what makes the user pick _this_ tool over alternatives): cheaper-or-more-permissive than GHCP for equivalent work; faithful execution of the existing harness protocol; fire-and-forget ergonomics; deterministic-as-possible behavior on commodity hardware.

**Fire criteria** (what makes the user abandon it): pass rate on Fire-Legasy-class workloads <30%; per-build cost ≥ GHCP equivalent; requires constant babysitting (defeats lights-out premise); requires writing a new harness from scratch (project becomes too large for solo).

---

## Pain Evidence (past behavior, not future intent)

| Pain                            | Evidence (verbatim)                                                                                                                   | Strength                                         |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ |
| Frontier-API price escalation   | "opus 4.7 is going up 27x"                                                                                                            | Strong — concrete catalyst                       |
| Per-query cost shock            | "i spent $9 on 1 query in ghcp"                                                                                                       | Strong — concrete artifact                       |
| Recurring rate-limit friction   | "i run into daily and weekly usage limits in ghcp"                                                                                    | Strong — sustained, not one-off                  |
| Token-pricing trend             | "harnesses are going to token based pricing"                                                                                          | Medium — market read, not personal incident      |
| Harness UX is non-negotiable    | "i absolutely love the state of the art agentic harnesses"                                                                            | Strong — explains absence of workaround attempts |
| Harness coupled to GHCP runtime | RCSnyder/lights-out-swe README: _"VS Code can't programmatically start Copilot chats, so the human opens each window and says 'go.'"_ | Strong — author has formally documented this gap |

## Anti-Patterns Avoided (Mom Test fails — explicitly excluded from wedge)

| Claim made by user                    | Why excluded                                                                                                         |
| ------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| "Sovereignty is important"            | Future-intent / philosophical; no past dollar or opportunity cost. Demoted to side-benefit.                          |
| "Cheaper plan-then-execute saves GPU" | Pure ideation; user has never executed a plan/execute split. Not load-bearing in v1.                                 |
| "Anyone trapped in the LLM ecosystem" | No named individual; market-of-one until proven otherwise.                                                           |
| "Generic across all task classes"     | Past behavior on exactly one loop class (SWE via lights-out-swe). Generalize substrate only; specialize application. |

---

## Validated Workload (what v1 must run)

**Canonical benchmark**: replay the [Fire Legasy](https://github.com/RCSnyder/fire-legasy) build end-to-end on a rented GPU using an open model.

- Stack: TypeScript + HTML5 Canvas frontend, Python FastAPI backend, PostgreSQL, Caddy, Docker Compose, Hetzner deploy
- Harness phases exercised: EXPAND → DESIGN → ANALYZE → BUILD → REVIEW → RECONCILE → VERIFY → DEPLOY (DEPLOY may be stubbed in spike)
- Acceptance contract: same scope.md / readiness.md / scaffolding artifacts produced as the original GHCP run
- Comparable scale: ~10 acceptance criteria, mid-hundreds of LOC, multi-file, multi-language

This is the **only workload v1 must satisfy**. Generalization across other cybernetic loops is a v2+ concern.

---

## What Is _Not_ a Problem (correctly bounded)

- **Inventing a new harness** — lights-out-swe exists, is shipped, is dogfooded. Out of scope. Improvements to lights-out-swe happen upstream in that repo.
- **Real-time interactive agent UX** — user keeps GHCP/Cursor for interactive work. v1 is async/batch only.
- **Frontier-quality on hard problems** — user accepts hybrid: open models for batched lights-out work, frontier APIs for hard interactive sessions.
- **Cold-start latency** — explicitly irrelevant per user. Spot instances acceptable. No warm-pool engineering needed in v1.
- **Multi-tenancy / SaaS** — single user, personal tool, local CLI.
- **Replicating Cloudflare Project Think / AWS Bedrock Agents** — those are vertically-integrated proprietary stacks; competing on substrate breadth is not viable solo. v1 differentiates on harness-protocol fidelity, not substrate features.

---

## Confidence Summary

| Validation                                                          | Confidence             | Source                                           |
| ------------------------------------------------------------------- | ---------------------- | ------------------------------------------------ |
| Cost pain is real, recurring, and dollar-quantified                 | **High**               | Q1.1, Q1.2 — past behavior                       |
| User will not downgrade harness UX                                  | **High**               | Q1.3, Q2.1 — past behavior (refused workarounds) |
| Async/batch is the actual mode (not real-time)                      | **High**               | Q2.3 — direct selection                          |
| Lights-out-swe protocol is the input format                         | **High**               | Repo evidence + Q2.5                             |
| User can ship autonomous loops                                      | **High**               | Fire Legasy is deployed proof                    |
| Sovereignty motivates buying behavior                               | **Low**                | Q1.4 — philosophical only                        |
| Plan/execute split saves real money                                 | **Unknown**            | Pure ideation — defer to spike                   |
| Open models can execute lights-out-swe protocol at useful pass rate | **Unknown — RISKIEST** | No past behavior; SPIKE required                 |
| Spot-GPU economics beat GHCP per build                              | **Unknown**            | No measurement; SPIKE required                   |
