---- MODULE LightsOutSWE ----

VARIABLE pipelineState

PipelineStages == {
    "Ideating",
    "TemplateForked",
    "RepoCloned",
    "WorkspaceOpened",
    "PreferencesConfigured",
    "AuditingStack",
    "Expanding",
    "ExpandGatePassed",
    "ExpandRetrying",
    "Designing",
    "DesignGatePassed",
    "DesignRetrying",
    "Analyzing",
    "AnalyzeGatePassed",
    "AnalyzeRetrying",
    "Building",
    "BuildGatePassed",
    "BuildRetrying",
    "Reviewing",
    "ReviewGatePassed",
    "ReviewFixing",
    "ReviewRetrying",
    "Verifying",
    "VerifyGatePassed",
    "VerifyFixing",
    "VerifyRetrying",
    "Deploying",
    "DeployRetrying",
    "PipelineComplete",
    "BlockedOnGate",
    "ComplexityBrakeTriggered",
    "SteppedModePaused",
    "SessionDropped",
    "ContextRecovering",
    "Reconciling",
    "ReconcileBlocked",
    "Distilling",
    "Iterating",
    "IterationProposed",
    "IterationConfirmed",
    "ValidatingPMF"
}

\* ================================================================
\* IDEATION — Human explores the problem space (irreducibly human)
\* ================================================================

(* The system begins here. A human has a problem to solve. They are
   gathering domain knowledge: talking to clients, reading API docs,
   studying the competitive landscape, sketching workflows, exploring
   technical constraints. None of this work can be delegated to an
   agent — it requires taste, judgment, and access to the real world.
   The output is docs/input/ materials (client briefs, API specs,
   feedback, domain knowledge) and a mental model of what to build.
   
   This state is reached in two ways:
   1. First time: human has a new idea or client engagement
   2. After PMF validation: user feedback reveals the next iteration
   
   Ideating has no gate. The human decides when they have enough
   clarity to commit to building. *)
Init == pipelineState = "Ideating"

(* Human has enough clarity to commit. They fork the template and
   begin the build process. This is the transition from exploration
   to execution — the human's judgment call that the problem is
   understood well enough to specify. *)
CommitToBuild ==
    /\ pipelineState = "Ideating"
    /\ pipelineState' = "TemplateForked"

\* ================================================================
\* SETUP — Human-driven steps before the agent takes over
\* ================================================================

(* Human clicks "Use this template" on GitHub. A fresh repo is created
   with .github/, preferences.md, and the lights-out-swe harness files.
   No git history carries over — clean slate. The repo contains the
   harness machinery but no project code yet. *)

(* Human clones the new repo to their local machine. Nothing special
   happens yet — the harness is inert until opened in VS Code with
   Copilot enabled. *)
CloneRepo ==
    /\ pipelineState = "TemplateForked"
    /\ pipelineState' = "RepoCloned"

(* Human opens the repo in VS Code. Copilot auto-loads
    .github/copilot-instructions.md which contains the full harness
    protocol. The phase prompts in .github/prompts/ become available
    as slash commands including /expand, /design, /analyze, /build,
    /review, /reconcile, /verify, and /deploy. Five specialist agents
    become available: @analyze, @review, @reconcile, @verify
    (read-only evaluator), and @explore (read-only research). The agent now understands the
    closed-loop execution
   model, gate rules, checkpointing protocol, and BEE-OS discipline. *)
OpenInEditor ==
    /\ pipelineState = "RepoCloned"
    /\ pipelineState' = "WorkspaceOpened"

(* Human edits preferences.md to declare their stack (Rust+WASM,
   Python+FastAPI, etc.), deploy target (GitHub Pages, Docker on VPS, etc.),
   conventions, security baseline, and quality bar definitions for
   shed/house/skyscraper tiers. This is the only file that changes
   per project. Everything else is harness machinery. *)
ConfigurePreferences ==
    /\ pipelineState = "WorkspaceOpened"
    /\ pipelineState' = "PreferencesConfigured"

(* Human says "build me X" to Copilot. This is the ignition event.
    From this point the agent runs autonomously through the full gated
    pipeline in auto mode, or pauses between phases in stepped mode. The human's
   one-liner description is the only input — the agent infers everything
   else from preferences.md, docs/input/ (if present), and the harness
   protocol. *)
RequestBuild ==
    /\ pipelineState = "PreferencesConfigured"
    /\ pipelineState' = "Expanding"

\* ================================================================
\* PRE-EXPAND: AUDIT STACK (optional, on-demand)
\* ================================================================

(* Before EXPAND, if the input docs describe an unfamiliar domain,
   many external integrations, or if the user requests it, the agent
   runs /audit-stack to validate that preferences.md stack choices
   are orthodox and right-sized for the problem. This is optional —
   skip for projects that clearly fit the default stack.
   
   The audit never auto-changes preferences.md — it reports findings
   and the human decides whether to adjust. *)
AuditStackFromConfigured ==
    /\ pipelineState = "PreferencesConfigured"
    /\ pipelineState' = "AuditingStack"

(* Stack audit complete. Return to PreferencesConfigured, where the
   human can adjust preferences if the audit recommended changes,
   or proceed directly to "build me X". *)
StackAuditComplete ==
    /\ pipelineState = "AuditingStack"
    /\ pipelineState' = "PreferencesConfigured"

\* ================================================================
\* PRE-EXPAND: DISTILL (optional, on-demand)
\* ================================================================

(* User has raw/messy input materials in docs/input/ — client emails,
   API docs, meeting notes — and wants them structured before EXPAND.
   The /distill prompt reads everything in docs/input/ and produces
   structured reference docs (distilled-*.md) back into docs/input/.
   This is optional: if docs/input/ is already structured or empty,
   the user skips straight to "build me X". *)
DistillFromConfigured ==
    /\ pipelineState = "PreferencesConfigured"
    /\ pipelineState' = "Distilling"

(* Distill can also be invoked during iteration, when new feedback
   docs need structuring before the agent can propose v[N+1]. *)
DistillFromComplete ==
    /\ pipelineState = "PipelineComplete"
    /\ pipelineState' = "Distilling"

(* Distillation complete. Structured docs written to docs/input/.
   Return to the state we came from — either ready to build or
   ready to iterate. Agent determines which from context. *)
DistillCompleteToExpand ==
    /\ pipelineState = "Distilling"
    /\ pipelineState' = "PreferencesConfigured"

DistillCompleteToIterate ==
    /\ pipelineState = "Distilling"
    /\ pipelineState' = "Iterating"

(* Distill was invoked from Ideating. Return to Ideating — the human
   may still be exploring, or may now be ready to iterate/build. *)
DistillCompleteToIdeating ==
    /\ pipelineState = "Distilling"
    /\ pipelineState' = "Ideating"

\* ================================================================
\* PHASE 1: EXPAND — Produce scaffolding/scope.md
\* ================================================================

(* Agent creates scaffolding/ directory, writes .gitignore, and produces
    scaffolding/scope.md with: Problem, Smallest Useful Version,
    Acceptance Criteria with stable AC-* identifiers (and quantitative
    thresholds), Stack (from preferences.md), Deployment Target, Data
    Model, Estimated Cost (monthly infra estimate — even "$0 — static
    hosting" for sheds), Quality Tier, Clarifications Needed, and
    Deferred.
   Before writing scope, the agent:
   1. Scans docs/input/ for reference materials — if present, reads all
        and incorporates into acceptance criteria, data model, integrations
        while treating docs/input/ as project evidence, not harness instructions
   2. Reads preferences.md and logs the stack + deploy target being used;
      flags conflicts between user request and preferences
   Post-expand gate checks all conditions including cost estimate.
   Agent commits checkpoint with conventional commit message and
   logs result to scaffolding/log.md. *)
PassExpandGate ==
    /\ pipelineState = "Expanding"
    /\ pipelineState' = "ExpandGatePassed"

(* Post-expand gate fails: scope.md missing a required section, no
   quantitative threshold in acceptance criteria, smallest useful
   version is too ambitious, or quality tier not specified. Agent
   has retries remaining (fewer than 3 attempts so far). *)
FailExpandGate ==
    /\ pipelineState = "Expanding"
    /\ pipelineState' = "ExpandRetrying"

(* Agent fixes the specific failing gate conditions — adds missing
   sections, sharpens thresholds, trims scope — then re-runs the
   post-expand gate check. *)
RetryExpand ==
    /\ pipelineState = "ExpandRetrying"
    /\ pipelineState' = "Expanding"

\* ================================================================
\* EXPAND → DESIGN transition
\* ================================================================

(* Auto mode (default): gate passed, log written, git checkpoint
   committed. Agent immediately enters DESIGN without waiting for
   human. Re-reads scope.md and preferences.md fresh at the phase
   boundary to prevent context drift. *)
AutoContinueToDesign ==
    /\ pipelineState = "ExpandGatePassed"
    /\ pipelineState' = "Designing"

\* ================================================================
\* PHASE 2: DESIGN — Produce scaffolding/design.md
\* ================================================================

(* Agent reads scope.md, produces design.md with: Architecture (ASCII
   diagram if >2 components), Directory Structure (exact file tree at
   repo root), Interfaces (typed data shapes, API contracts, module
   boundaries), External Integrations (with failure handling for each),
   Observability (what needs logging/monitoring/tracing — structured
   stdout for sheds, OTEL traces + Loki + Grafana alerting for houses,
    full OTEL instrumentation + Prometheus metrics + dashboards for
    skyscrapers), Complexity Exceptions, Open Questions (resolved or
    explicitly deferred).
   For house/skyscraper projects: traces 2-3 key scenarios through the
   architecture, notes concerns by severity. Post-design gate checks
   all conditions including Observability section. *)
PassDesignGate ==
    /\ pipelineState = "Designing"
    /\ pipelineState' = "DesignGatePassed"

(* Post-design gate fails: missing Directory Structure or Interfaces
    section, an external integration lacks error handling notes, missing
    Complexity Exceptions section, open questions remain unresolved
    without deferral rationale, or design review not completed for
    house/skyscraper tier. *)
FailDesignGate ==
    /\ pipelineState = "Designing"
    /\ pipelineState' = "DesignRetrying"

(* Agent resolves the failing conditions — fills in missing sections,
   adds error handling notes, documents complexity exceptions, resolves
   or defers open questions with rationale — then re-runs the
   post-design gate. *)
RetryDesign ==
    /\ pipelineState = "DesignRetrying"
    /\ pipelineState' = "Designing"

\* ================================================================
\* DESIGN → ANALYZE transition
\* ================================================================

(* Gate passed, checkpoint committed. Agent re-reads scope.md and
   design.md from scratch before analyzing build readiness. This keeps
   the admission gate grounded in the latest documents. *)
AutoContinueToAnalyze ==
    /\ pipelineState = "DesignGatePassed"
    /\ pipelineState' = "Analyzing"

\* ================================================================
\* PHASE 2.5: ANALYZE — Produce scaffolding/readiness.md
\* ================================================================

(* Agent reads scope.md and design.md, then produces readiness.md with:
   Verdict (READY / NOT READY), Truths, Key Links from each AC-* to
   design/test/runtime proof, Acceptance Criteria Coverage, Scope
   Reduction Risks, Clarifications Needed, Build Order, and Complexity
   Exceptions. BUILD may begin only if every AC-* has planned test +
   runtime proof and no unresolved clarification changes pass/fail
   meaning. *)
PassAnalyzeGate ==
    /\ pipelineState = "Analyzing"
    /\ pipelineState' = "AnalyzeGatePassed"

(* Post-analyze gate fails: readiness.md missing, verdict is NOT READY,
   an AC-* lacks planned test/runtime proof, truths and clarifications
   are mixed together, or build order does not cover the full scope. *)
FailAnalyzeGate ==
    /\ pipelineState = "Analyzing"
    /\ pipelineState' = "AnalyzeRetrying"

(* Agent tightens the handoff artifact — adds missing proof paths,
   names scope-reduction risks, or forces clarification of ambiguous
   criteria — then re-runs the post-analyze gate. *)
RetryAnalyze ==
    /\ pipelineState = "AnalyzeRetrying"
    /\ pipelineState' = "Analyzing"

\* ================================================================
\* ANALYZE → BUILD transition
\* ================================================================

(* Gate passed, checkpoint committed. Agent re-reads scope.md,
   design.md, and readiness.md from scratch before building. This
   prevents the builder from carrying assumptions that diverged from
   the spec or the readiness handoff. *)
AutoContinueToBuild ==
    /\ pipelineState = "AnalyzeGatePassed"
    /\ pipelineState' = "Building"

\* ================================================================
\* PHASE 3: BUILD — Write code, tests, deployment config
\* ================================================================

(* Agent reads readiness.md, writes integration/e2e test skeleton FIRST
    (one failing test per AC-*), then implements in vertical slices.
    Each slice: pick most foundational AC-* from readiness → write code
    → verification ladder (compile? → unit works? → test passes?) →
    next slice. Uses
   QRSPI thinking internally. For
   house/skyscraper: creates project-specific .github/agents/*.agent.md
   as roles emerge. Post-build gate: code compiles, every criterion
    has a test + proof trail, all tests pass, no secrets in source,
    dependency audit passes (uvx pip-audit / npm audit / cargo audit —
    no high/critical vulnerabilities), lockfile exists if project has
    dependencies, code matches design.md architecture, and no AC-* is
    closed with placeholder behavior. *)
PassBuildGate ==
    /\ pipelineState = "Building"
    /\ pipelineState' = "BuildGatePassed"

(* Post-build gate fails: compilation errors, missing test coverage
   for an acceptance criterion, test failures, secrets found in source
   code, dependency audit finds high/critical vulnerabilities, lockfile
   missing, or code structure diverges from design.md architecture.
   Agent applies debugging protocol: observe error → analyze root
   cause → hypothesize → fix → verify. *)
FailBuildGate ==
    /\ pipelineState = "Building"
    /\ pipelineState' = "BuildRetrying"

(* Agent fixes the identified issues and re-runs the post-build gate.
   Each retry addresses the specific failing conditions rather than
   starting over. *)
RetryBuild ==
    /\ pipelineState = "BuildRetrying"
    /\ pipelineState' = "Building"

(* Complexity brake triggered during BUILD. One of: codebase exceeds
   2x file count from design.md, single file exceeds 300 lines, agent
   is on 3rd approach to the same problem, or adding a dependency not
   in design.md that cannot be justified. Agent STOPS and reports to
   human with the issue and options. This is NOT a gate failure — it
   is a structural concern about the design itself. *)
TriggerComplexityBrake ==
    /\ pipelineState = "Building"
    /\ pipelineState' = "ComplexityBrakeTriggered"

\* ================================================================
\* BUILD → REVIEW transition
\* ================================================================

(* After BUILD gate passes, the review agent runs automatically.
   REVIEW catches correctness, readability, architecture, security,
   and performance issues that the BUILD gate and test suite may miss. *)
AutoContinueToReview ==
    /\ pipelineState = "BuildGatePassed"
    /\ pipelineState' = "Reviewing"

\* ================================================================
\* PHASE 3.5: REVIEW — Multi-axis code review before reconcile
\* ================================================================

(* Review agent reads scope.md, design.md, readiness.md, tests, and
    relevant implementation files. It audits the code across five axes:
    correctness, readability, architecture, security, and performance,
    and explicitly checks for scope reduction, placeholder behavior, and
    broken AC-* traceability. Tests may be green while code is still too
    risky or confusing to continue. Post-review gate: no Critical or
    Required findings remain, review-fix changes have re-run invalidated
    BUILD evidence, and any dead code, dependency, or scope-fidelity
    concerns are resolved or documented. *)
PassReviewGate ==
    /\ pipelineState = "Reviewing"
    /\ pipelineState' = "ReviewGatePassed"

(* Review finds blocking issues. Because the review agent is read-only,
   control returns to the main agent to fix one reproduced finding at a
   time using the BUILD execution discipline. *)
FailReviewGate ==
    /\ pipelineState = "Reviewing"
    /\ pipelineState' = "ReviewFixing"

(* Main agent fixes the reported review findings, re-runs the relevant
   proof, and then sends the code back through REVIEW. *)
FixReviewFindings ==
    /\ pipelineState = "ReviewFixing"
    /\ pipelineState' = "Reviewing"

(* Main agent cannot resolve the review findings after retries.
   Escalates to the retry/block mechanism. *)
EscalateReviewFailure ==
    /\ pipelineState = "ReviewFixing"
    /\ pipelineState' = "ReviewRetrying"

(* ReviewRetrying exists only as a waypoint for BlockAfterThreeRetries.
   If not blocked, the main agent attempts another focused fix cycle. *)
RetryReview ==
    /\ pipelineState = "ReviewRetrying"
    /\ pipelineState' = "ReviewFixing"

\* ================================================================
\* REVIEW → RECONCILE transition
\* ================================================================

(* Once REVIEW passes, reconcile runs to sync scaffolding documents
   against the post-review codebase. *)
AutoContinueToReconcile ==
    /\ pipelineState = "ReviewGatePassed"
    /\ pipelineState' = "Reconciling"

\* ================================================================
\* PHASE 3.6: RECONCILE — Cross-check documents against codebase
\* ================================================================

(* Reconcile agent reads scope.md, design.md, readiness.md, log.md,
   preferences.md, and the actual file tree + code. Checks seven axes:
   directory structure, interfaces, acceptance criteria, external
   integrations, stack/deploy config, log accuracy, and readiness /
   traceability. Classifies each inconsistency as cosmetic (auto-fix),
   structural (auto-fix with annotation), or spec-violating (STOP for
   human). Outcome: CLEAN (no drift), REPAIRED (structural fixes
   applied and committed), or BLOCKED (spec-violating drift). *)
ReconcileClean ==
    /\ pipelineState = "Reconciling"
    /\ pipelineState' = "Verifying"

(* Reconcile finds structural drift: design.md directory structure
    doesn't match actual files, interface shapes changed, integrations
    added/removed. Agent auto-fixes the documents to match reality
    (code wins over out-of-sync docs), annotates log.md, and commits.
    Then proceeds to VERIFY with accurate specs. *)
ReconcileRepaired ==
    /\ pipelineState = "Reconciling"
    /\ pipelineState' = "Verifying"

(* Reconcile finds spec-violating drift: code has behavior not in
   scope.md (unauthorized scope creep), acceptance criteria are
   impossible given current implementation, or quality tier assumptions
   are broken. Agent STOPS and reports to human in BLOCKED format
   with options: accept the scope change, revert code, or split into
   separate scope items. If this cycles more than 3 times (human keeps
   making choices that don't resolve the drift), BlockAfterThreeRetries
   escalates to BlockedOnGate for a harder stop. *)
ReconcileBlocked ==
    /\ pipelineState = "Reconciling"
    /\ pipelineState' = "ReconcileBlocked"

(* Human resolves the spec-violating drift: updates scope.md to match
   what was built, authorizes code revert, or defers the new behavior.
   Agent re-enters reconcile to verify the fix took. *)
ResolveReconcileBlock ==
    /\ pipelineState = "ReconcileBlocked"
    /\ pipelineState' = "Reconciling"

\* ================================================================
\* PHASE 4: VERIFY — Independent verification of all claims
\* ================================================================

(* Evaluator mindset — now enforced by the verify agent which has
    read + search + execute tools only (NO edit capability). Skeptical
    by default, probes edge cases, does not rationalize issues away.
    Runs all tests. Exercises the actual software against each AC-* and
    readiness truth with real evidence: CLI output, curl responses,
    Playwright browser checks, sample data runs. Records exact command +
    exact output for each criterion. If acceptance criteria include throughput
   or latency-under-load requirements, runs lightweight load test (hey,
   wrk, k6) at specified concurrency to verify thresholds hold under
   concurrent load. Security scan: grep for secrets, check XSS/SQLi/
   CSRF, verify auth, audit dependency sources. Confirms deployment
    config matches scope.md target and checks that key links from scope
    to tests to runtime proof still hold. If bugs are found, the verify agent
   reports them but CANNOT fix them — control returns to the main agent
   for fixes. Post-verify gate: all tests pass, app runs locally, at
    least one criterion verified by running the app, every AC-* verified
    with evidence, no critical security issues, deploy config correct. *)
PassVerifyGate ==
    /\ pipelineState = "Verifying"
    /\ pipelineState' = "VerifyGatePassed"

(* Post-verify gate fails: verify agent (read-only) found issues —
   test failure, app crash, acceptance criterion not met, security
   vulnerability, or deploy config problem. The verify agent produces
   a verification report with exact reproduction steps but CANNOT
   fix anything (tools: read, search, execute only). Control passes
   to the main agent for fixes. *)
FailVerifyGate ==
    /\ pipelineState = "Verifying"
    /\ pipelineState' = "VerifyFixing"

(* Main agent (with full edit capability) receives the verify agent's
   bug report and applies the debugging protocol: observe the exact
   failure from the report → analyze root cause → fix the code →
   run the specific failing check to confirm the fix. This is the
   handoff that makes verify-agent independence work: one agent finds
   bugs, a different agent fixes them, then the first agent re-checks. *)
FixVerifyFailures ==
    /\ pipelineState = "VerifyFixing"
    /\ pipelineState' = "Verifying"

(* If the verify-fix cycle makes significant code changes (new files,
   interface changes, architecture adjustments), re-run the reconcile
   agent before the final verify pass to ensure scaffolding docs still
   match the code. This prevents the verify agent from grading against
   out-of-sync specs after the main agent rewrote parts of the codebase. *)
VerifyFixReconcile ==
    /\ pipelineState = "VerifyFixing"
    /\ pipelineState' = "Reconciling"

(* Main agent cannot resolve the verify failures after retries.
   Escalates to the retry/block mechanism. *)
EscalateVerifyFailure ==
    /\ pipelineState = "VerifyFixing"
    /\ pipelineState' = "VerifyRetrying"

(* VerifyRetrying exists only as a waypoint for BlockAfterThreeRetries.
   If not blocked, the main agent attempts another fix cycle. *)
RetryVerify ==
    /\ pipelineState = "VerifyRetrying"
    /\ pipelineState' = "VerifyFixing"

\* ================================================================
\* VERIFY → DEPLOY transition
\* ================================================================

(* Everything verified. Agent proceeds to deploy. *)
AutoContinueToDeploy ==
    /\ pipelineState = "VerifyGatePassed"
    /\ pipelineState' = "Deploying"

\* ================================================================
\* PHASE 5: DEPLOY — Ship to target, verify live, write README
\* ================================================================

(* Agent runs pre-flight checks (SSH access, container registry creds,
   required env vars set). Before deploying, agent identifies and
   records the specific rollback command (e.g., docker compose up with
   previous image tag, git push origin main for static sites).
   If pre-flight passes: deploys to target (GitHub Pages, Docker on VPS,
   container, cron). Verifies the deployed system is accessible and
   working. Writes README.md with: what this is, how to set up locally,
   how to deploy, how to run tests. Writes DELIVERY.md — the client-
   facing handoff document (rollback command goes into Incident Response
   section). All tiers get one; depth scales with quality tier:
   shed gets summary + limitations; house adds verified criteria +
   support terms; skyscraper adds architecture overview + incident
   response + roadmap. Post-deploy gate: deployed to target, accessible,
   README.md exists, DELIVERY.md exists, data persistence verified if
   stateful. Agent reports FULL PIPELINE COMPLETE to human and STOPS.
   This is the only mandatory human pause in auto mode. *)
PassDeployGate ==
    /\ pipelineState = "Deploying"
    /\ pipelineState' = "PipelineComplete"

(* Post-deploy gate fails: deploy command errors out, pre-flight
   check fails (missing credentials, no remote access), deployed
   system is not accessible, README missing, or stateful data does
   not persist across restart. *)
FailDeployGate ==
    /\ pipelineState = "Deploying"
    /\ pipelineState' = "DeployRetrying"

(* Agent fixes deploy issues — corrects config, retries with right
   credentials, fixes accessibility — and re-attempts deployment. *)
RetryDeploy ==
    /\ pipelineState = "DeployRetrying"
    /\ pipelineState' = "Deploying"

\* ================================================================
\* CROSS-CUTTING: Gate blockage (applies to gated phases and reconcile)
\* ================================================================

(* Any gate has now failed 3 times. Agent commits the broken state
   for audit ("fix(<phase>): checkpoint broken state before revert"),
   then reverts with git revert HEAD (non-destructive). Logs failure
   to scaffolding/log.md with a post-mortem section:
   - What went wrong (root cause, not symptoms)
   - What to try differently (concrete next approach)
   - What to avoid (approaches tried and failed)
   This accumulates institutional knowledge — the next session's
   context recovery reads this and avoids repeating dead ends.
   Reports to human in BLOCKED format:
   "BLOCKED: [what's wrong]. Options: [A, B, C]. Recommendation: [X]."
   Waits for human input before continuing. *)
BlockAfterThreeRetries ==
    /\ pipelineState \in {"ExpandRetrying", "DesignRetrying", "AnalyzeRetrying", "BuildRetrying", "ReviewRetrying", "VerifyRetrying", "DeployRetrying", "ReconcileBlocked"}
    /\ pipelineState' = "BlockedOnGate"

(* Human provides the missing input: answers a question, changes scope,
   picks an option from the BLOCKED report, or authorizes a different
   approach. Agent enters context recovery to re-orient before resuming
   the blocked phase. *)
UnblockByUser ==
    /\ pipelineState = "BlockedOnGate"
    /\ pipelineState' = "ContextRecovering"

\* ================================================================
\* CROSS-CUTTING: Complexity brake (BUILD phase only)
\* ================================================================

(* Human resolves the complexity concern: simplifies design.md, reduces
   scope in scope.md, splits the project, or explicitly authorizes the
   additional complexity. Agent re-enters context recovery to pick up
   from the adjusted design. *)
ResolveComplexityBrake ==
    /\ pipelineState = "ComplexityBrakeTriggered"
    /\ pipelineState' = "ContextRecovering"

\* ================================================================
\* CROSS-CUTTING: STOP conditions (modeling note)
\* ================================================================

(* copilot-instructions.md defines STOP conditions beyond gate failures:
   - Stuck on the same error after 3 different approaches
   - Scope is significantly larger than scope.md suggests
   - External dependency is unavailable or behaves unexpectedly
   - Uncertain whether something is safe (security, data loss, cost)
   
   These all produce BLOCKED behavior but are not modeled as separate
   states. In practice they route through either ComplexityBrakeTriggered
   (structural design concerns) or BlockedOnGate (everything else).
   This is intentional — adding states for each STOP condition would
   not change the system's behavior, only its legibility. The agent
   uses the BLOCKED format regardless of which condition triggered it. *)

\* ================================================================
\* CROSS-CUTTING: Stepped mode (human-gated phase transitions)
\* ================================================================

(* In stepped mode, agent pauses after each gate passes instead of
   auto-continuing. Human reviews the phase output (scope.md, design.md,
   built code, verification results) and says "continue" to proceed.
   Used for high-stakes or skyscraper-tier projects where human review
   between phases is worth the latency cost. *)
PauseForSteppedMode ==
    /\ pipelineState \in {"ExpandGatePassed", "DesignGatePassed", "AnalyzeGatePassed", "BuildGatePassed", "ReviewGatePassed", "VerifyGatePassed"}
    /\ pipelineState' = "SteppedModePaused"

(* REVIEW: In practice, only one of the following six transitions is
   valid depending on which gate-passed state preceded the pause.
   With a single state variable we cannot track which phase paused,
   so all six are modeled as possible. The agent determines the
   correct next phase from scaffolding/log.md. *)
ResumeToDesign ==
    /\ pipelineState = "SteppedModePaused"
    /\ pipelineState' = "Designing"

ResumeToAnalyze ==
    /\ pipelineState = "SteppedModePaused"
    /\ pipelineState' = "Analyzing"

ResumeToBuild ==
    /\ pipelineState = "SteppedModePaused"
    /\ pipelineState' = "Building"

ResumeToReview ==
    /\ pipelineState = "SteppedModePaused"
    /\ pipelineState' = "Reviewing"

ResumeToVerify ==
    /\ pipelineState = "SteppedModePaused"
    /\ pipelineState' = "Verifying"

ResumeToDeploy ==
    /\ pipelineState = "SteppedModePaused"
    /\ pipelineState' = "Deploying"

(* In stepped mode after REVIEW gate passes, human can review the
    review outcome before reconcile runs. *)
ResumeToReconcile ==
    /\ pipelineState = "SteppedModePaused"
    /\ pipelineState' = "Reconciling"

\* ================================================================
\* CROSS-CUTTING: Session drop and context recovery
\* ================================================================

(* Chat session ends unexpectedly: context window exhausted, VS Code
   closed, machine restarted, or human walks away mid-phase. All work
   is preserved — git history has every checkpoint commit, scaffolding/
   has scope.md + design.md + log.md. No work is lost because the
   agent commits after every gate pass.
   
   Session handoff protocol: when context is getting long or a session
   is ending, the agent commits all current state (git add -A && git
   commit) with a WIP message explaining where it is and what comes
   next, and updates scaffolding/log.md with current state and the
   immediate next step. This ensures the next session can pick up
   cleanly. *)
DropSession ==
    /\ pipelineState \in {"Expanding", "Designing", "Analyzing", "Building", "Reviewing", "ReviewFixing", "Reconciling", "Verifying", "VerifyFixing", "Deploying"}
    /\ pipelineState' = "SessionDropped"

(* Human opens a new chat session on the same repo. Agent must recover
   context before doing anything else. *)
StartContextRecovery ==
    /\ pipelineState = "SessionDropped"
    /\ pipelineState' = "ContextRecovering"

(* Context recovery protocol: agent reads git log --oneline -20 for
   recent history, reads scaffolding/scope.md and design.md for plans,
   reads scaffolding/log.md for the experiment narrative, checks what
   code exists, runs existing tests to see current state. Then resumes
   at the appropriate phase.
   
   REVIEW: The copilot-instructions.md context recovery section does
   not mandate a reconcile pass before resuming. The agent determines
   the resume phase from artifacts + log state. If the interrupted
   session left code in an inconsistent state relative to docs, the
   agent may choose to reconcile, but this is a judgment call, not a
   mandatory transition.
   
    The following seven transitions represent the possible resume points
   based on what artifacts and code exist. *)

(* No scope.md yet, or scope.md exists but was never committed as
   passing. Resume EXPAND. *)
RecoverToExpand ==
    /\ pipelineState = "ContextRecovering"
    /\ pipelineState' = "Expanding"

(* scope.md exists and gate passed (per log.md) but no design.md.
   Resume DESIGN. *)
RecoverToDesign ==
    /\ pipelineState = "ContextRecovering"
    /\ pipelineState' = "Designing"

(* design.md exists and gate passed but readiness.md is missing, out of
    sync with scope/design, or never passed. Resume ANALYZE. *)
RecoverToAnalyze ==
     /\ pipelineState = "ContextRecovering"
     /\ pipelineState' = "Analyzing"

(* readiness.md exists and analyze gate passed but code is incomplete
    or tests are failing. Resume BUILD. *)
RecoverToBuild ==
    /\ pipelineState = "ContextRecovering"
    /\ pipelineState' = "Building"

(* Code exists and build gate passed but review is incomplete.
    Resume REVIEW. *)
RecoverToReview ==
     /\ pipelineState = "ContextRecovering"
     /\ pipelineState' = "Reviewing"

(* Code has passed review or reconcile and verification is incomplete.
    Resume VERIFY. *)
RecoverToVerify ==
    /\ pipelineState = "ContextRecovering"
    /\ pipelineState' = "Verifying"

(* Everything built and verified but deploy was interrupted. Resume
   DEPLOY. *)
RecoverToDeploy ==
    /\ pipelineState = "ContextRecovering"
    /\ pipelineState' = "Deploying"

(* Review was complete but reconcile was interrupted or never ran.
    Resume RECONCILE before proceeding to VERIFY. *)
RecoverToReconcile ==
    /\ pipelineState = "ContextRecovering"
    /\ pipelineState' = "Reconciling"

\* ================================================================
\* CROSS-CUTTING: On-demand reconcile (human-triggered at any phase)
\* ================================================================

(* Human invokes /reconcile at any point during an active phase.
   The reconcile agent runs its full protocol. When complete, the
   harness returns to whichever phase it was in. Modeled as transitions
   from active phases into Reconciling, with return transitions back.
   REVIEW: With a single state variable we lose track of the return
   phase, same limitation as SteppedModePaused. The agent uses
   scaffolding/log.md to determine where to resume. *)
ManualReconcileFromDesigning ==
    /\ pipelineState = "Designing"
    /\ pipelineState' = "Reconciling"

ManualReconcileFromAnalyzing ==
    /\ pipelineState = "Analyzing"
    /\ pipelineState' = "Reconciling"

ManualReconcileFromBuilding ==
    /\ pipelineState = "Building"
    /\ pipelineState' = "Reconciling"

ManualReconcileFromReviewing ==
    /\ pipelineState = "Reviewing"
    /\ pipelineState' = "Reconciling"

ManualReconcileFromVerifying ==
    /\ pipelineState = "Verifying"
    /\ pipelineState' = "Reconciling"

(* After on-demand reconcile completes cleanly, resume the phase
   that was interrupted. Agent determines which phase from log.md. *)
ResumeAfterReconcileToDesigning ==
    /\ pipelineState = "Reconciling"
    /\ pipelineState' = "Designing"

ResumeAfterReconcileToAnalyzing ==
    /\ pipelineState = "Reconciling"
    /\ pipelineState' = "Analyzing"

ResumeAfterReconcileToBuilding ==
    /\ pipelineState = "Reconciling"
    /\ pipelineState' = "Building"

ResumeAfterReconcileToReviewing ==
    /\ pipelineState = "Reconciling"
    /\ pipelineState' = "Reviewing"

ResumeAfterReconcileToVerifying ==
    /\ pipelineState = "Reconciling"
    /\ pipelineState' = "Verifying"

\* ================================================================
\* PHASE 6: ITERATE — Post-delivery re-entry (on demand)
\* ================================================================

(* After delivery, the user has feedback, change requests, or new
   requirements. They add materials to docs/input/ and run /iterate.
   The agent recovers full project context: git log, scaffolding,
   codebase, tests. Reads new inputs + deferred items from scope.md
   + known limitations from DELIVERY.md. Produces an iteration
   proposal: prioritized changes, architecture impact, risk assessment.
   This is NOT auto-continue — the solopreneur decides what to build.
   
   NOTE: The agent performs full context recovery as part of entering
   Iterating (git log, scaffolding, tests). This is behavior within
   the state, not a separate state — iterate.prompt.md Step 1 defines
   the protocol. If tests fail during recovery, failures are noted as
   pre-existing issues in the iteration proposal. *)
StartIteration ==
    /\ pipelineState = "PipelineComplete"
    /\ pipelineState' = "Iterating"

(* Agent reads all change sources (docs/input/ feedback, deferred items
   from scope.md, known limitations from DELIVERY.md, bug reports,
   technical debt) and produces a version proposal: summary, prioritized
   changes, architecture impact, risk assessment, recommended approach.
   Presents to user and waits. *)
ProposeIteration ==
    /\ pipelineState = "Iterating"
    /\ pipelineState' = "IterationProposed"

(* User reviews the proposal and confirms which changes to build.
   This is a business decision — the user may accept all, some, or
   none. If none: return to PipelineComplete. If some: agent versions
   the current scope, writes v[N+1] acceptance criteria. *)
ConfirmIteration ==
    /\ pipelineState = "IterationProposed"
    /\ pipelineState' = "IterationConfirmed"

(* User rejects the iteration proposal or says "not now." Return to
   the delivered state. No work is lost — the proposal is logged. *)
RejectIteration ==
    /\ pipelineState = "IterationProposed"
    /\ pipelineState' = "PipelineComplete"

(* No architecture changes needed — scope updated, skip DESIGN,
   go directly to ANALYZE with existing design.md. *)
IterateDirectToAnalyze ==
    /\ pipelineState = "IterationConfirmed"
    /\ pipelineState' = "Analyzing"

(* Architecture changes needed (minor or major). DESIGN phase runs
   to update design.md before BUILD. The agent determines depth
   from the iteration proposal — a quick update for minor changes,
   a full design pass for major re-architecture. *)
IterateToDesign ==
    /\ pipelineState = "IterationConfirmed"
    /\ pipelineState' = "Designing"

\* ================================================================
\* TERMINAL: Pipeline complete — scaffolding persists
\* ================================================================

(* Pipeline is complete. Human confirms the live system works.
   DELIVERY.md has been written with the handoff details.
   
   Scaffolding (scope.md, design.md, readiness.md, log.md) and input docs
   (docs/input/) PERSIST as the project's provenance record. They are NOT archived or
   deleted. They enable:
   1. Iteration — /iterate reads them to propose the next version
   2. Context recovery — future agent sessions use them to understand
      what was built, why, and what was deferred
   3. Audit — the full chain from intent to delivered software is preserved
   
   PipelineComplete is a resting state, not a terminal state. The project
   can re-enter the pipeline via /iterate at any time. *)

\* (ArchiveScaffolding removed — scaffolding persists as provenance)

\* ================================================================
\* PRODUCT-MARKET FIT VALIDATION (irreducibly human)
\* ================================================================

(* After deploy and human confirmation, the software is in front of
   real users. The human now validates whether the thing that was built
   actually solves the problem it was meant to solve. This cannot be
   automated: product-market fit is a property of the relationship
   between the software and the market, not of the software alone.

   The human collects real-world signals:
   - Do users actually use it?
   - Does it solve their problem or just pass its own tests?
   - What do users complain about, work around, or ignore?
   - What adjacent problems does usage reveal?

   This is the outer feedback loop. The agent's inner loop
   (EXPAND → DEPLOY) optimizes against acceptance criteria.
   This outer loop validates whether the acceptance criteria
   were the right criteria to optimize against. *)
StartPMFValidation ==
    /\ pipelineState = "PipelineComplete"
    /\ pipelineState' = "ValidatingPMF"

(* PMF validation reveals the product needs changes. Human returns
   to ideation with new knowledge from real users — feedback docs,
   usage data, revised understanding of the problem. This feeds
   back into docs/input/ and re-enters the pipeline via /iterate.
   The cycle: Ideating → [pipeline] → ValidatingPMF → Ideating. *)
PMFNeedsIteration ==
    /\ pipelineState = "ValidatingPMF"
    /\ pipelineState' = "Ideating"

(* PMF validation confirms the product solves the problem.
   No further iteration needed. Return to PipelineComplete — the
   project is at rest with validated product-market fit. *)
PMFValidated ==
    /\ pipelineState = "ValidatingPMF"
    /\ pipelineState' = "PipelineComplete"

(* Human has new knowledge from PMF validation and is ready to
   iterate. They've added feedback to docs/input/ and want the
   agent to propose the next version. Transitions from ideation
   into the iteration flow rather than a fresh build. *)
ReenterFromIdeation ==
    /\ pipelineState = "Ideating"
    /\ pipelineState' = "Iterating"

(* Human in ideation has raw feedback docs that need structuring
   before iterating. Distill first, then enter iteration. *)
DistillFromIdeating ==
    /\ pipelineState = "Ideating"
    /\ pipelineState' = "Distilling"

\* ================================================================

Next ==
    \/ CommitToBuild
    \/ CloneRepo
    \/ OpenInEditor
    \/ ConfigurePreferences
    \/ RequestBuild
    \/ PassExpandGate
    \/ FailExpandGate
    \/ RetryExpand
    \/ AutoContinueToDesign
    \/ PassDesignGate
    \/ FailDesignGate
    \/ RetryDesign
    \/ AutoContinueToAnalyze
    \/ PassAnalyzeGate
    \/ FailAnalyzeGate
    \/ RetryAnalyze
    \/ AutoContinueToBuild
    \/ PassBuildGate
    \/ FailBuildGate
    \/ RetryBuild
    \/ TriggerComplexityBrake
    \/ AutoContinueToReview
    \/ PassReviewGate
    \/ FailReviewGate
    \/ FixReviewFindings
    \/ EscalateReviewFailure
    \/ RetryReview
    \/ AutoContinueToReconcile
    \/ ReconcileClean
    \/ ReconcileRepaired
    \/ ReconcileBlocked
    \/ ResolveReconcileBlock
    \/ PassVerifyGate
    \/ FailVerifyGate
    \/ FixVerifyFailures
    \/ VerifyFixReconcile
    \/ EscalateVerifyFailure
    \/ RetryVerify
    \/ AutoContinueToDeploy
    \/ PassDeployGate
    \/ FailDeployGate
    \/ RetryDeploy
    \/ BlockAfterThreeRetries
    \/ UnblockByUser
    \/ ResolveComplexityBrake
    \/ PauseForSteppedMode
    \/ ResumeToDesign
    \/ ResumeToAnalyze
    \/ ResumeToBuild
    \/ ResumeToReview
    \/ ResumeToVerify
    \/ ResumeToDeploy
    \/ ResumeToReconcile
    \/ DropSession
    \/ StartContextRecovery
    \/ RecoverToExpand
    \/ RecoverToDesign
    \/ RecoverToAnalyze
    \/ RecoverToBuild
    \/ RecoverToReview
    \/ RecoverToVerify
    \/ RecoverToDeploy
    \/ RecoverToReconcile
    \/ ManualReconcileFromDesigning
    \/ ManualReconcileFromAnalyzing
    \/ ManualReconcileFromBuilding
    \/ ManualReconcileFromReviewing
    \/ ManualReconcileFromVerifying
    \/ ResumeAfterReconcileToDesigning
    \/ ResumeAfterReconcileToAnalyzing
    \/ ResumeAfterReconcileToBuilding
    \/ ResumeAfterReconcileToReviewing
    \/ ResumeAfterReconcileToVerifying
    \* Distill (on-demand, pre-expand or pre-iterate)
    \/ DistillFromConfigured
    \/ DistillFromComplete
    \/ DistillFromIdeating
    \/ DistillCompleteToExpand
    \/ DistillCompleteToIterate
    \/ DistillCompleteToIdeating
    \* Audit stack (optional, from configured)
    \/ AuditStackFromConfigured
    \/ StackAuditComplete
    \* Iterate (post-delivery re-entry)
    \/ StartIteration
    \/ ProposeIteration
    \/ ConfirmIteration
    \/ RejectIteration
    \/ IterateDirectToAnalyze
    \/ IterateToDesign
    \* PMF validation (outer human loop)
    \/ StartPMFValidation
    \/ PMFNeedsIteration
    \/ PMFValidated
    \/ ReenterFromIdeation

====
