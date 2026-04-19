# Lights Out SWE

_Set the spec. Walk away. Come back to shipped software._

A gated agentic harness for lights-out software engineering. You say "build me X," the agent runs autonomously through phased quality gates, and you come back to working software — or a specific blocker.

## What This Is

A **lights-out software engineering** system. Like a [lights-out factory](<https://en.wikipedia.org/wiki/Lights_out_(manufacturing)>) in manufacturing — fully automated, no humans on the floor. You provide intent + preferences, the agent builds through quality gates, you come back to deployed software or a precise blocker.

Three layers:

- **Harness** (this repo) — permanent. The gated protocol that drives autonomous builds.
- **Scaffolding** (`scaffolding/`) — persistent. Versioned scopes, design decisions, experiment logs. The project's provenance record.
- **Software** (the delivered product) — permanent. Stands alone. Zero runtime dependency on the harness.

## How to Use It

### Quick Start

1. Click **"Use this template"** on GitHub → create a new repo for your project
2. Clone your new repo and open it in VS Code
3. Edit `preferences.md` to set your stack, deploy targets, and conventions
4. (Optional) Add reference materials to `docs/input/` — client briefs, API specs, domain knowledge
5. (Optional) Run `/distill` if input docs are messy and need structuring
6. Open Copilot chat in agent mode
7. Say `build me [description of what you want]`

The agent takes it from there.

### Prerequisites

- VS Code with GitHub Copilot (agent mode enabled)
- Git initialized (the template handles this)

### The Loop (Auto Mode — Default)

1. Open the project in VS Code
2. Start a Copilot chat in agent mode
3. Say "build me [description of what you want]"
4. Agent runs autonomously: EXPAND → DESIGN → ANALYZE → BUILD → REVIEW → RECONCILE → VERIFY → DEPLOY
5. At each phase, the agent checks a gate, logs results to `scaffolding/log.md`, and git-commits a checkpoint
6. If a gate passes → agent auto-continues to the next phase
7. If a gate fails 3× → agent STOPS and reports what's blocking
8. After DEPLOY → agent stops and reports the final result

You come back to either working deployed software, or a specific blocker with options.

### Stepped Mode

Say "stepped mode" for high-stakes projects. Agent pauses after each gate for your confirmation. Say "auto" to switch back.

### The Phases

| Phase         | Input          | Output                     | Gate Checks                                                                                            |
| ------------- | -------------- | -------------------------- | ------------------------------------------------------------------------------------------------------ |
| **EXPAND**    | "build me X"   | `scaffolding/scope.md`     | Has stable `AC-*` IDs, deployment target, stack, smallest useful version                               |
| **DESIGN**    | scope.md       | `scaffolding/design.md`    | Has directory structure, interfaces, integration handling, complexity exceptions                       |
| **ANALYZE**   | scope + design | `scaffolding/readiness.md` | Every `AC-*` has planned tests, runtime proof, truths, and build order                                 |
| **BUILD**     | readiness.md   | Working code               | Compiles, tests pass, no secrets in code, no placeholder closure of `AC-*`                             |
| **REVIEW**    | Built code     | Review report              | No blocking correctness, readability, architecture, security, performance, or scope-reduction findings |
| **RECONCILE** | Code + docs    | Synced scaffolding         | Documents match codebase, no spec-violating drift                                                      |
| **VERIFY**    | Running code   | Verified system            | Tests pass, runs locally, every `AC-*` verified with evidence, no security issues                      |
| **DEPLOY**    | Verified code  | Live system                | Deployed, accessible, README + DELIVERY.md exist, data persistence verified                            |
| **ITERATE**   | Feedback       | Next version               | User confirms proposal, scope versioned, re-enters pipeline at right point                             |

### Using Prompt Files

The `.github/prompts/` directory has one prompt file per phase. You can invoke them directly:

- `/distill` — Structure raw input materials in `docs/input/` into consumable reference docs
- `/audit-stack` — Validate preferences.md stack choices against input docs for orthodox, idiomatic fit
- `/expand` — Generate scope from a project idea (reads `docs/input/` if present)
- `/design` — Generate architecture from scope
- `/analyze` — Turn scope + design into a build-readiness handoff before BUILD
- `/build` — Build code from readiness
- `/review` — Audit built code before reconciliation and verification
- `/reconcile` — Sync scaffolding docs with actual codebase after review
- `/verify` — Run verification checks (delegates to the read-only verify agent)
- `/deploy` — Deploy, write project README and DELIVERY.md
- `/iterate` — Post-delivery: propose and build the next version from feedback

Or just let the instructions in `.github/copilot-instructions.md` drive the full loop automatically.

### Agents

The `.github/agents/` directory has specialist agents with restricted tool access:

- `@analyze` — `read`, `edit`, `search`: pre-build admission control that maps `AC-*` to proofs, truths, and build order
- `@review` — `read`, `search`, `execute`: independent code review across correctness, architecture, security, and maintainability
- `@reconcile` — `read`, `edit`, `search`, `execute`: detect and fix drift between scaffolding docs and codebase
- `@verify` — `read`, `search`, `execute`: independent evaluator that can run code but cannot edit it
- `@explore` — `read`, `search`: read-only codebase exploration and Q&A

**Why agents?** Tool restrictions enforce behavioral boundaries. The review and verify agents _cannot_ edit source code, which prevents the "grade your own homework" problem. The explore agent _cannot_ modify anything, making it safe for context recovery and research.

You can invoke agents directly (`@analyze`, `@review`, `@verify`, `@reconcile`, `@explore`) or let the prompts/pipeline invoke them automatically.

The `.github/skills/` directory holds reusable execution workflows that prompts and instructions can load on demand. The first one in this repo, `build-discipline`, tightens the BUILD and verify-fix loops around small slices, proof-first changes, and root-cause debugging.

## File Structure

```text
.github/
  copilot-instructions.md   # The pipeline loop + discipline rules (auto-loaded by Copilot)
  agents/
    analyze.agent.md         # Pre-build admission control before BUILD
    review.agent.md          # Independent code review before reconcile/verify
    reconcile.agent.md       # Drift detection + document sync
    verify.agent.md          # Independent evaluator (read-only + execute)
    explore.agent.md         # Read-only codebase exploration
  skills/
    build-discipline/
      SKILL.md               # On-demand BUILD execution discipline for thin slices and debugging
  prompts/
    distill.prompt.md        # Pre-expand: structure raw input materials
    audit-stack.prompt.md    # Pre-expand: validate stack choices against problem domain
    expand.prompt.md         # Phase 1: scope generation (reads docs/input/)
    design.prompt.md         # Phase 2: architecture from scope
    analyze.prompt.md        # Phase 2.5: build-readiness handoff
    build.prompt.md          # Phase 3: code from readiness
    review.prompt.md         # Phase 3.5: multi-axis code review before reconcile
    reconcile.prompt.md      # Phase 3.6: sync docs with code
    verify.prompt.md         # Phase 4: testing + acceptance
    deploy.prompt.md         # Phase 5: deployment + README + DELIVERY.md
    iterate.prompt.md        # Phase 6: post-delivery version iteration
preferences.md               # Stack, infra, conventions, security, quality bar
docs/
  input/                     # Reference materials — client briefs, API specs, feedback, state machines
  reference/
    system_state_machine.tla # TLA+ formal spec of the pipeline state machine
scaffolding/                  # Persistent — scope, design, readiness, log (project provenance)
  scope.md                   # What we're building (versioned across iterations)
  design.md                  # How we're building it (living document)
  readiness.md               # Why BUILD is allowed to start: truths, traceability, build order
  log.md                     # Experiment log — every gate check, every result
```

### State Machine

The full pipeline — including gate retries, stepped mode, session recovery, complexity brakes, on-demand reconcile, and post-delivery iteration — is formally specified as a TLA+ state machine in [`docs/reference/system_state_machine.tla`](docs/reference/system_state_machine.tla).

To view the state machine diagram interactively, paste the spec into the source editor at [tlaplus-process-studio.com](https://tlaplus-process-studio.com).

### How It Maps to autoresearch

| autoresearch       | lights-out-swe                               |
| ------------------ | -------------------------------------------- |
| `program.md`       | `copilot-instructions.md` + `preferences.md` |
| `train.py`         | project source code                          |
| val_bpb metric     | gate checks (pass/fail)                      |
| 5-min training run | gate evaluation                              |
| keep experiment    | `git commit` checkpoint                      |
| discard experiment | `git revert HEAD` (non-destructive)          |
| experiment log     | `scaffolding/log.md`                         |
| autonomous loop    | auto-continue on gate pass                   |

## Customization

### `preferences.md`

Edit this to match your stack, infrastructure, conventions, and quality bar. The agent references it during EXPAND (stack selection) and BUILD (conventions).

### Quality Bar

Projects scale on a formality dial:

- **Shed** — Personal tool / script. Works, runs. Tests for verification loop.
- **House** — Real project with users. Tests for key paths, README required, deploy automated.
- **Skyscraper** — Complex system, multiple users, money. Full tests, formal design, staged deploy, monitoring, runbook.

All tiers get the same DELIVERY.md structure — depth scales naturally with complexity. Pick the right level in scope.md. Don't build skyscraper process for a shed.

### Portability

The file formats (`.github/copilot-instructions.md`, `.prompt.md`, `.agent.md`) are GitHub Copilot-specific. The protocol — gated phases, verification ladder, scope lock, evidence rule — is tool-agnostic. To port to a different agentic IDE, translate the instructions and phase prompts to that tool's format. The [state machine spec](docs/reference/system_state_machine.tla) is the canonical, tool-independent definition.

### Durable Value

The durable value of this repo is the **control loop**, not any one tactical instruction set.

The parts most likely to remain useful as models improve are the process invariants:

- explicit phase boundaries and gates
- stable `AC-*` identifiers plus a readiness handoff before BUILD
- persistent provenance in `scaffolding/` and `docs/input/`
- independent REVIEW / RECONCILE / VERIFY roles
- non-destructive checkpointing in git
- the formal transition model in the [state machine spec](docs/reference/system_state_machine.tla)

The structural additions — the ANALYZE readiness handoff, stable `AC-*` traceability, and scope-reduction detection — were informed by ideas from Spec Kit [5], Get Stuff Done [6], and OpenSpec [7]. The more tactical parts — slice guidance, anti-rationalization checks, framework/source nudges, and similar execution scaffolding — are intentionally modular. Some of those ideas were adapted from Agent Skills [4], but in this repo they are helpers around the loop, not the product itself. As models absorb more of that a priori engineering discipline, those tactical layers should be audited, simplified, or removed without changing the core harness.

### Scope

This system is designed to see how far **one agent** can go autonomously — solo developer, single agent, closed loop. Multi-agent coordination, PR-based workflows, and team code review processes are out of scope.

**Future: Parallel BUILD.** The architecture supports parallelizing the BUILD phase using git worktrees. Each worktree gets its own VS Code window and agent session, implementing independent acceptance criteria behind shared interface contracts from design.md. What it would require:

- **Slice planning** in DESIGN: identify parallelizable criteria, assign module boundaries, declare which slice owns the schema
- **Coordinator prompt** (`/parallel-build`): creates worktrees (`git worktree add`), generates per-slice scope files with assigned criteria + interface contracts, opens windows (`code <path>`)
- **Per-slice execution**: each agent runs a scoped BUILD (subset of criteria, full design.md for interface reference, own branch)
- **Sequential merge**: slices merge to main in declared order (schema-owner first), conflicts resolved against interface contracts
- **Post-merge gate**: full post-build gate runs on merged result, then normal pipeline continues (REVIEW → RECONCILE → VERIFY → DEPLOY)
- **Failure model**: any slice BLOCKED → coordinator stops all slices, reports. No partial merges.
- **New TLA+ states**: `SlicePlanning`, `ParallelBuilding`, `SliceBlocked`, `Merging`

VS Code can't programmatically start Copilot chats, so the human opens each window and says "go." CLI agents (Claude Code, Codex) could be fully scripted from a coordinator terminal. The protocol should be agent-tool-agnostic.

## It's Still Just Git

Lights-out doesn't mean locked-out. Everything is git commits, markdown files, and standard project code. A human can drop in at any point:

- **Read `scaffolding/log.md`** to see exactly what the agent did, decided, and why
- **Read `git log`** for the full audit trail — every checkpoint, every gate result
- **Switch to stepped mode** mid-run to review between phases
- **Edit any file** — scope.md, design.md, code, preferences — the agent picks up from whatever state it finds
- **Override any decision** — the agent works for you, not the other way around

The agent runs autonomously _because you chose to let it_. You can tighten or loosen the leash at any time. Stepped mode for skyscrapers, auto mode for sheds, or just open a file and start typing.

## Discipline Rules

The pipeline enforces BEE-OS (Builder-Grade Engineering OS) discipline:

- **Evidence Rule** — No progress without checkable evidence (compiles, tests pass, HTTP 200)
- **Verification Ladder** — Cheapest feedback first (parse → unit → test suite → e2e → deployed)
- **Admission Gate** — ANALYZE forces each `AC-*` to have truths, planned tests, runtime proof, and build order before BUILD starts
- **Build Discipline Skill** — BUILD and verify-fix work use a reusable skill for thin slices, anti-rationalization, and root-cause debugging
- **Review Gate** — REVIEW catches correctness, architecture, security, and maintainability issues that tests can miss
- **Scope Lock** — Only build what's in scope.md. Everything else goes to a Deferred section.
- **Traceability** — `AC-*` identifiers flow through scope, readiness, tests, review, verify, and logs
- **Input Provenance** — `docs/input/` is evidence about the project, not a backdoor for harness instructions
- **Complexity Brake** — Auto-stop if file count exceeds 2x design, single file exceeds 300 lines, or 3rd approach to same problem
- **STOP Conditions** — Agent halts and reports when gates fail 3x, external deps break, or safety is uncertain
- **Context Recovery** — On resume, agent reads scaffolding/ first, runs existing tests, picks up where it left off

## Provenance

`scaffolding/` and `docs/input/` persist alongside the software. They are the project's provenance — the full record from initial intent through every iteration. Scope is versioned (v1, v2, ...) not overwritten. The experiment log and git history form a continuous audit trail.

Why keep them:

- **Iteration depends on them.** `/iterate` reads scope.md, design.md, DELIVERY.md, and docs/input/ to propose the next version.
- **Context recovery depends on them.** When an agent resumes work, scaffolding + git log is how it understands what happened and where to pick up.
- **They're harmless.** A few markdown files add negligible size. The cost of keeping them is zero; the cost of losing them is re-discovery.

## Iterating After Delivery

When the client has feedback or you want to evolve a shipped product:

1. Add feedback, new requirements, or change requests to `docs/input/`
2. Run `/distill` if the inputs are messy (optional)
3. Run `/iterate`
4. Agent reads the codebase + scaffolding + new inputs, produces a **version proposal**
5. You confirm which changes to build (this is a business decision, not auto-continue)
6. Agent versions the scope, re-enters the pipeline at the right point, and builds

```text
/iterate → proposal → user confirms → ANALYZE → BUILD → REVIEW → RECONCILE → VERIFY → DEPLOY
```

Scope history is preserved — v1 criteria stay in scope.md under a version header. The audit trail is continuous across iterations.

## Why This Over Plan → Build?

VS Code's built-in Plan agent is a good tactical tool: it explores your codebase, asks clarifying questions, and produces step-by-step implementation plans. It works well for feature-level tasks within existing codebases.

Lights-out-swe solves a different problem. The difference is structural, not just "more phases":

**Where information enters the system.** Plan agent's only inputs are codebase state + session Q&A. Lights-out-swe has a dedicated channel (`docs/input/`) for client briefs, API specs, state machines, domain knowledge, and feedback — information that exceeds what any single planning session could extract. This gives the controller more _variety_ (in the Ashby's Law sense) to handle complex systems.

**How convergence is enforced.** Plan → Build is open-loop: plan once, execute, hope it's right. Lights-out-swe is closed-loop: gates provide feedback at each phase, the verification ladder catches errors cheaply (parse before unit test before e2e before deploy), and the experiment log tracks the trajectory. Open-loop works for predictable systems; closed-loop is necessary when there's uncertainty.

**What persists.** A Plan lives in session memory — ephemeral, not versioned, lost when the session ends. Scaffolding persists across sessions, is versioned across iterations, and forms a provenance chain that enables context recovery and iteration. The agent can resume where it left off because the scaffolding tells it what happened.

**The potential energy metaphor.** The collaborative a priori work — long conversations with the client, distilling domain knowledge, setting preferences, defining acceptance criteria — is _potential energy_. Each input doc, each preference, each constraint narrows the solution space. By the time the agent enters BUILD, the search space is already small, and ANALYZE has already mapped each `AC-*` to proofs and runtime evidence. Plan → Build starts at zero potential energy for greenfield projects and has to build it during the session through exploration.

The hypothesis: **rich specification + constrained solution space + closed-loop execution converges more reliably than plan + open-loop execution.** The engineering happens in the conversation and the docs, not in the code generation.

### What's Borrowed From Plan

Plan has genuine strengths this system incorporates:

- **Interactive alignment**: Plan cycles Discovery → Alignment → Design → Refinement. The EXPAND phase (and `/distill` before it) does the same thing through input docs + preferences confirmation.
- **askQuestions for clarification**: Plan doesn't guess — it asks. The EXPAND phase flags conflicts with preferences and pauses for resolution.
- **Parallel research**: Plan launches multiple Explore subagents for different areas. The explore agent (`@explore`) serves the same purpose during context recovery and BUILD.
- **Explicit scope boundaries**: Plan includes "what's included and what's deliberately excluded." The Deferred section in scope.md + Scope Lock discipline enforce this.

The key difference: Plan's output is a step list for a single session. Lights-out-swe's output is a specification that survives across sessions and iterations.

## Where Humans Matter Most

The system has two irreducibly human states, formally specified in the [state machine](docs/reference/system_state_machine.tla). No agent transition can skip either one.

**Ideating** (`Init`). Before any code exists, a human explores the problem space: client conversations, domain research, competitive analysis, workflow sketching, technical constraint discovery. This work produces the `docs/input/` materials and `preferences.md` that narrow the solution space before the agent runs. The agent cannot do this — it has no access to clients, users, markets, or the real world.

**Validating product-market fit** (`ValidatingPMF`). After deploy, a human puts the software in front of real users and observes: do they use it? Does it solve their problem or just pass its own tests? What do they work around, complain about, or ignore? PMF is a property of the relationship between software and its market, not a property of the software alone. No test suite can measure it.

These two states form the **outer loop**:

```text
Ideating → [agent pipeline] → ValidatingPMF → Ideating → [iterate pipeline] → ValidatingPMF → ...
```

The agent's inner loop (EXPAND → DEPLOY) optimizes against acceptance criteria. The outer loop validates whether those were the right criteria. Every other state in the system — expanding, designing, building, reviewing, reconciling, verifying, deploying — is agent-executable. These two are not. They are where the engineering judgment lives.

The system is designed around this fact. The mandatory human pause after DEPLOY exists because the agent has no way to evaluate whether it built the right thing — only whether it built the thing right. The `/iterate` confirmation gate exists because iteration is a business decision informed by PMF signals the agent cannot observe. The `docs/input/` directory exists because the most valuable engineering work — translating human problems into machine-checkable specifications — happens in conversation, not in code generation.

## Citations

[1] A. Karpathy, "autoresearch," GitHub, 2025. [Online]. Available: [github.com/karpathy/autoresearch](https://github.com/karpathy/autoresearch)

[2] P. Rajasekaran, "Harness design for long-running application development," Anthropic Engineering, Mar. 2026. [Online]. Available: [anthropic.com/engineering/harness-design-long-running-apps](https://www.anthropic.com/engineering/harness-design-long-running-apps)

[3] J. Blocklove et al., "Design Conductor: An agent autonomously builds a 1.5 GHz Linux-capable RISC-V CPU," arXiv:2603.08716 [cs.AR], Mar. 2026. [Online]. Available: [arxiv.org/abs/2603.08716](https://arxiv.org/abs/2603.08716)

[4] A. Osmani et al., "Agent Skills," GitHub, 2026. [Online]. Available: [github.com/addyosmani/agent-skills](https://github.com/addyosmani/agent-skills)

[5] GitHub, "Spec Kit: Spec-Driven Development," GitHub, 2026. [Online]. Available: [github.com/github/spec-kit](https://github.com/github/spec-kit/blob/main/spec-driven.md)

[6] GSD Build, \"Get Stuff Done,\" GitHub, 2026. [Online]. Available: [github.com/gsd-build/get-'stuff'-done](https://github.com/gsd-build/get-shit-done/)

[7] Fission AI, "OpenSpec," GitHub, 2026. [Online]. Available: [github.com/Fission-AI/OpenSpec](https://github.com/Fission-AI/OpenSpec)

[8] S. Levin, J. Corbet, "AI Coding Assistants," Linux Kernel Documentation, 2026. [Online]. Available: [github.com/torvalds/linux/.../coding-assistants.rst](https://github.com/torvalds/linux/blob/master/Documentation/process/coding-assistants.rst)
