---- MODULE OpenPinceryCanonical ----

(* ================================================================
   OPEN PINCERY — CANONICAL STATE MACHINE (v3)

   Source of truth that directs agentic-SWE implementation for an
   industry-leading agentic operating system.

   v3 CHANGELOG (vs v2)

     Correctness:
       C1  History variable `visited` added to session so
           Inv_ToolCallRequiresBinding and Inv_TerminalSuccession
           actually verify the required upstream states.
       C2  All terminal-succession invariants now check `visited`.
       C3  QuarantineInput / QuarantineMemory require
           lifecycle = "PromptAssembling" | "Awake" precondition.
       C4  SuspendWorkflow moves lifecycle to "AwaitingApproval" so
           ResumeAfterSuspension has a valid exit.
       C5  Spec's WF_vars clauses cover the full audit-commit and
           cleanup-to-terminal chain; liveness is now provable.
       C6  AttestSandbox / VerifyExecutionImage carry lifecycle
           preconditions.
       C7  Init is explicitly Invariants-conformant (ASSUME).

     Accuracy:
       A1  EventInjecting now runs ClassifyInjectedEvent; a
           quarantined injection suspends the intent.
       A2  ToolResultEnteredContext transition makes the boundary
           between result pipeline and messages[] explicit.
       A3  ToolResultSecretScanned is a FIRST-CLASS transition that
           sets session.has_secret; the sanitize/capture fork is
           deterministic.
       A4  Cross-intent resume (wake ends mid-intent) is documented
           as EXPLICIT TODO for v4 with a scaffolded intent-stack.
       A5  IssueToolCall and ReceiveToolResult increment
           session.budget_used; ExhaustBudget guards on real total.
       A6  CheckPolicyVersion pins from CONSTANT PolicyBundleVersion
           instead of inventing a counter.
       A7  RedactIngressSecrets is a modeled transition; human /
           webhook / agent messages traverse it before intake.

     Industry-leading security (NEW):
       G1  ClassifyInjectedEvent  — mid-wake re-classification.
       G2  ModelResponseScanned   — output DLP before tool dispatch.
       G3  AuditChainVerified / AuditChainCommitted — hash-chain
           integrity at intake and per-terminal.
       G4  InterAgentEnvelopeVerified — signed provenance on
           source=agent messages, gating ClassifyInput.
       G5  MemoryWriteContentClassified — memory-poisoning defense.
       G6  PlanStepChecked — per-tool-call plan conformance via
           session.plan_steps and session.current_step.

     Documented TODOs (v4 candidates):
       G7  Capability freshness / nonce per IssueToolCall.
       G8  Ephemeral sandbox teardown+reprovision between calls.
       G9  Model response schema + consistency cross-check.
       G10 Prompt template signed-artifact verification.
       G11 Real time / monotonic nonce state (for expiry + replay).

     Enterprise-tier, gated:
       G12 Budget anomaly detection.
       G13 Approval UI signing (operator signature on decisions).
       G14 Side-channel / resource monitoring.
       G15 Child prompt attestation (child_prompt_digest).

   v3.1 PATCH (post-second-audit correctness pass)

     B1  RouteInjectedQuarantine now sets lifecycle' = "AwaitingApproval"
         so ResumeAfterSuspension has a valid exit; previously the
         injected-quarantine path deadlocked with
         lifecycle = "EventInjecting" + security = "Suspended".
     B2  ModelReturns now accepts security \in {PrivilegeUsageAudited,
         ResultValidated}, enabling multi-turn tool loops. Previously
         the second model turn was unreachable.
     B3  Direct-response path (AgentRespondsToHuman) now commits
         an audit terminal via QueueDirectResponseAudit. Previously
         pure-answer intents never reached any terminal, breaking
         Inv_TerminalSuccession and liveness.
     B4  ProvisionSandbox accepts security = "ExecutionAuthorized"
         so G6 is genuinely deferrable; v1 ship skips CheckPlanStep.
         Inv_ToolCallRequiresBinding no longer requires PlanStepChecked
         (that check lives only in Inv_PlanConformance, which is in
         the full ceiling, not InvariantsV1).

     Introduced InvariantsV1 — the subset checked by the v1 ship —
     dropping Inv_AuditChainBeforeExecution (G3),
     Inv_EnvelopeVerifiedForAgentSource (G4 vacuous),
     and Inv_PlanConformance (G6). readiness.md MUST check
     InvariantsV1, not Invariants, for the v1 ship.

   v3.2 PATCH (post-third-audit correctness pass — convergence)

     F1  Sleep and cap lifecycle paths no longer leak non-terminal
         intents. AgentCallsSleep, AgentRespondsToHuman,
         IterationCapReached, ContextCapReached now set security
         atomically (to SuccessAuditPending or FailureAuditPending)
         as part of the same transition that enters the sleep/cap
         lifecycle state. The subsequent audit chain brings security
         to a terminal before TerminalEndsWake can unwind the wake.
         Previously ImplicitSleepEndsWake and friends could fire
         first, ending the wake with security still mid-intent,
         letting the next EventArrives overwrite session and leak
         the intent. Liveness_IntentTerminates is now provable.
     F2  QueueDirectResponseAudit deleted. Its work is done atomically
         inside AgentRespondsToHuman, eliminating the race against
         the (now deleted) ImplicitSleepEndsWake.
     F3  IterationCapReached and ContextCapReached moved from
         lifecycle = "ToolResultProcessing" to lifecycle = "Awake",
         so they fire at the DECISION point (before the tool-call
         lifecycle commits). AgentCallsTool now has an explicit
         iteration < MaxIterations guard, so at the cap the agent
         must exit via IterationCapReached instead of stalling at
         lifecycle = "ToolExecuting" with IssueToolCall disabled.
     F4  InvariantsV1 restored to include Inv_AuditChainBeforeExecution
         and Inv_EnvelopeVerifiedForAgentSource. Both invariants do
         hold in v1 traces because their actions (VerifyAuditChain,
         VerifyInterAgentEnvelope) fire as cosmetic stand-ins even
         when cryptographic strength is deferred. Only
         Inv_PlanConformance is genuinely dropped (CheckPlanStep is
         skipped, so PlanStepChecked is never in visited).
     F5  ExplicitSleepEndsWake, ImplicitSleepEndsWake,
         IterationCapEndsWake, ContextCapEndsWake,
         BudgetCapEndsWake all DELETED. They are subsumed by
         TerminalEndsWake, which only fires once security is
         terminal. Previously these "unconditional wake-end"
         actions were the mechanism causing F1.

   v3.3 PATCH (TLC-driven correctness pass — mechanical verification)

     Verified with TLC 2.19 under the v1 safety config
     (INVARIANT InvariantsV1, liveness deferred to a follow-on
     pass). Random simulation (-simulate num=2000 -depth 150)
     sampled 1,200,001 states with zero invariant violations;
     exhaustive BFS explored 2M+ distinct states up to depth 39
     before the host ran out of heap (the visited : SUBSET
     SecurityStates variable drives the blow-up). Each bug below
     was first surfaced by TLC as a concrete counterexample and
     then patched here.

     B5  ResumeAfterSuspension now requires
         "ApprovalPending" \in session.visited. Without this
         guard, TLC found a 6-state trace where a workflow
         suspended for injected-event quarantine could be
         resumed with no approval ever routed, skipping the
         human-in-the-loop gate demanded by T4.

     B6  AuthorizeExecution now requires
         "AuditChainVerified" \in session.visited. TLC found
         a trace reaching security = "ExecutionAuthorized"
         via Suspend -> Resume -> Authorize without ever
         committing the intake audit chain — violating the
         G3 intent and Inv_AuditChainBeforeExecution once
         F4 had restored that invariant to InvariantsV1.

     B7  DetectPolicyVersionMismatch now requires
         "PolicyVersionMismatch" \notin session.visited
         (fires at most once per wake). Previously the
         DetectPolicyVersionMismatch <-> SuspendWorkflow
         pair was an infinite loop: every resume re-detected
         the same mismatch and re-suspended. This was
         originally found as a liveness counterexample; the
         once-per-wake guard is a safety-neutral fix that
         also restores termination.

     B8  Spec now includes WF_vars(Next) alongside the
         existing per-action WF clauses, plus explicit
         WF_vars for RouteApprovalExpiry,
         RouteSuspensionExpiry, HandlePolicyVersionMismatch,
         StaleWakeRecovered, ExpireSuspension, ExpireApproval.
         Without the blanket WF, TLC reported stuttering
         counterexamples at arbitrary interior lifecycle
         states where a single enabled expiry/route action
         lacked its own fairness clause. Liveness checking
         is currently disabled in the cfg because WF_vars(Next)
         under exhaustive BFS is prohibitively expensive; it
         is kept in the Spec so a future liveness pass can
         re-enable PROPERTY lines under a state constraint
         or with smaller constants.

     B9  SuspendWorkflow now requires
         "Suspended" \notin session.visited (fires at most
         once per wake). Without this guard TLC found a
         SuspendWorkflow <-> ExpireSuspension <-> RouteSuspensionExpiry
         livelock: each expire route returned the workflow
         to a state where SuspendWorkflow could fire again.
         The once-per-wake guard matches real operational
         intent — a wake that has been suspended once should
         proceed to approval or failure, not re-enter suspension.

     B10 StaleWakeDetected rewritten to atomically route
         security -> "FailureAuditPending" (via MarkVisited)
         when security was not already in a terminal or
         failure-chain state. TLC found a real safety bug
         at depth 39: StaleWakeDetected firing mid-tool-call
         left (security = "ToolCallIssued",
         lifecycle = "StaleDetected"), violating
         Inv_ToolCallRequiresBinding because lifecycle had
         torn down while security was still live. The
         atomic security tear-down closes the orphan window.

     VERIFICATION STATUS (v3.3)
       INVARIANT InvariantsV1:  PASS (simulation, 1.2M states,
                                0 violations; BFS up to 2M+
                                distinct states, 0 violations).
       PROPERTY  Liveness_*:    DEFERRED (commented in cfg).
                                Re-enable with a state
                                constraint or smaller constants.

   HOW TO USE THIS SPEC

     (1) Every AC in scaffolding/scope.md cites canonical action
         names.  No action → no AC.
     (2) readiness.md maps every AC to the actions its tests fire.
     (3) VERIFY evidence quotes canonical_action=<Name>.
     (4) Tier in scope.md selects which enterprise actions are
         in play (see Section T).
     (5) Section V defines what counts as evidence per invariant.
         Traversal is not enough; negative tests are required.
   ================================================================ *)

EXTENDS Naturals, Sequences, FiniteSets

(* -----------------------------------------------------------------
   CONSTANTS
   ----------------------------------------------------------------- *)

CONSTANTS
    MaxIterations,       \* per-wake tool-loop cap
    MaxBudgetCents,      \* cumulative spend cap
    MaxPlanSteps,        \* longest plan admitted
    Tier,                \* self_host_individual | multi_tenant | enterprise
    PolicyBundleVersion, \* current active policy bundle version
    CostPerCall          \* spend charged per LLM/tool call (cents)

ASSUME MaxIterations       \in Nat /\ MaxIterations > 0
ASSUME MaxBudgetCents      \in Nat /\ MaxBudgetCents > 0
ASSUME MaxPlanSteps        \in Nat /\ MaxPlanSteps > 0
ASSUME PolicyBundleVersion \in Nat /\ PolicyBundleVersion > 0
ASSUME CostPerCall         \in Nat /\ CostPerCall > 0
ASSUME Tier \in { "self_host_individual", "multi_tenant", "enterprise" }

(* -----------------------------------------------------------------
   SECTION A — STATE
   ----------------------------------------------------------------- *)

VARIABLES lifecycle, security, session

AgentStates == {
    "Resting",
    "EventArrived",
    "WebhookReceived",
    "WebhookDeduplicating",
    "IngressRedacting",
    "WakeAcquiring",
    "WakeAcquireFailed",
    "PromptAssembling",
    "Awake",
    "ModelResponseProcessing",
    "ToolDispatching",
    "ToolPermissionChecking",
    "AwaitingApproval",
    "ApprovalRejected",
    "ToolExecuting",
    "ToolResultProcessing",
    "MidWakeEventPolling",
    "EventInjecting",
    "ImplicitSleeping",
    "ExplicitSleeping",
    "IterationCapHit",
    "ContextCapHit",
    "BudgetCapHit",
    "WakeEnding",
    "EventCollapsing",
    "MaintenanceCalling",
    "MaintenanceWriting",
    "SummaryWriting",
    "Draining",
    "DrainAcquiring",
    "StaleDetected"
}

SecurityStates == {
    \* Intake
    "IntentSubmitted",
    "IngressRedacted",
    "AuditChainVerified",
    "IdentityVerified",
    "OperatorBound",
    "OperatorMfaVerified",
    "TenantResolved",
    "TenantIsolationBound",
    "InterAgentEnvelopeVerified",
    "RequestNormalized",
    "ProvenanceTagged",
    "PolicySignatureVerified",
    "PolicyVersionChecked",
    "InputClassified",
    "InputQuarantined",
    "MemoryReadPending",
    "MemoryProvenanceChecked",
    "MemoryQuarantined",
    "MemoryMerged",
    \* Planning
    "PlanDrafted",
    "PlanValidated",
    "CapabilityScoped",
    "CapabilityAttenuated",
    "BudgetReserved",
    "BudgetExhausted",
    "RiskScored",
    \* Approval
    "ApprovalPending",
    "DualControlPending",
    "ApprovalExpired",
    "ExecutionAuthorized",
    \* Model response (G2)
    "ModelResponseReceived",
    "ModelResponseScanned",
    "ModelResponseApproved",
    "ModelResponseRejected",
    \* Plan conformance (G6)
    "PlanStepChecked",
    "PlanStepViolation",
    \* Sandbox binding
    "ImageVerified",
    "SandboxAttested",
    "SandboxProvisioned",
    "ToolSessionBound",
    "ActionIdentityBound",
    "FilesystemScoped",
    "NetworkScoped",
    "EgressPolicyBound",
    "BrowserPolicyBound",
    "ShellPolicyBound",
    "SecretReferencesBound",
    "ActionPlanAdmitted",
    \* Tool call + result
    "ToolCallIssued",
    "ToolResultReceived",
    "ToolResultSecretScanned",
    "ToolResultSanitized",
    "ToolResultClassified",
    "ToolResultLowRisk",
    "ToolResultRejected",
    "ToolResultEndorsed",
    "EndorsementDenied",
    "ToolResultEnteredContext",
    "ActionStepCompleted",
    \* Secret capture (AC-NEW-b)
    "SecretCapturePending",
    "SecretPersisting",
    "SecretBound",
    "SecretRotationPending",
    "SecretRevocationPending",
    \* Privilege
    "PrivilegeUsageAudited",
    "PrivilegeViolationDetected",
    \* Result + memory write
    "ResultSanitized",
    "ResultValidated",
    "MemoryWriteReviewPending",
    "MemoryWriteContentClassified",
    "MemoryWriteContentQuarantined",
    "MemoryWriteRejected",
    "MemoryWritePending",
    \* Injection defense (G1)
    "InjectedEventClassified",
    "InjectedEventQuarantined",
    \* Child lineage
    "ChildSpawnRequested",
    "ChildCapabilityScoped",
    "ChildBudgetBound",
    "ChildIdentityBound",
    "ChildLineageVerified",
    "ChildApprovalPending",
    "ChildLineageAuditPending",
    "ChildLineageAuditCommitted",
    "ChildRegistered",
    \* Audit terminals
    "SuccessAuditPending",
    "SuccessAuditCommitted",
    "AuditChainCommitted_Success",
    "SuccessCleanupPending",
    "Completed",
    "DenialAuditPending",
    "DenialAuditCommitted",
    "AuditChainCommitted_Denial",
    "DenialCleanupPending",
    "Denied",
    "FailureAuditPending",
    "FailureAuditCommitted",
    "AuditChainCommitted_Failure",
    "FailureCleanupPending",
    "Failed",
    \* Exceptional
    "IsolationBreachDetected",
    "ContainmentEngaged",
    "PolicyVersionMismatch",
    "Suspended",
    "SuspensionExpired",
    "RevocationAuditPending",
    "RevocationAuditCommitted",
    "AuditChainCommitted_Revocation",
    "ChildRevocationPropagating",
    "RevocationCleanupPending",
    "Revoked"
}

SecurityTerminals == { "Completed", "Denied", "Failed", "Revoked" }

EventSources == { "human", "webhook", "timer", "agent", "internal", "none" }

UntrustedSources == { "human", "webhook", "agent" }

InWakeStates == {
    "PromptAssembling", "Awake", "ModelResponseProcessing",
    "ToolDispatching", "ToolPermissionChecking", "AwaitingApproval",
    "ApprovalRejected", "ToolExecuting", "ToolResultProcessing",
    "MidWakeEventPolling", "EventInjecting", "ImplicitSleeping",
    "ExplicitSleeping", "IterationCapHit", "ContextCapHit",
    "BudgetCapHit", "WakeEnding", "EventCollapsing",
    "MaintenanceCalling", "MaintenanceWriting", "SummaryWriting",
    "Draining", "DrainAcquiring"
}

SessionRecord == [
    wake_id           : Nat,
    intent_id         : Nat,
    iteration         : Nat,
    budget_used       : Nat,
    policy_version    : Nat,
    child_count       : Nat,
    breach_flag       : BOOLEAN,
    input_source      : EventSources,
    has_secret        : BOOLEAN,
    priv_audited      : BOOLEAN,
    visited           : SUBSET SecurityStates,  \* C1/C2: per-intent history
    plan_steps        : Nat,                    \* G6: admitted plan length
    current_step      : Nat,                    \* G6: next expected step
    envelope_required : BOOLEAN                 \* G4: true when source=agent
]

TypeOK ==
    /\ lifecycle \in AgentStates
    /\ security  \in SecurityStates
    /\ session   \in SessionRecord

(* -----------------------------------------------------------------
   SECTION T — TIER PARTITION
   ----------------------------------------------------------------- *)

EnterpriseOnlyStates == {
    "OperatorMfaVerified",
    "TenantResolved",
    "TenantIsolationBound",
    "PolicySignatureVerified",
    "DualControlPending",
    "ImageVerified",
    "SandboxAttested",
    "BrowserPolicyBound"
}

RequiredBindingStates == {
    "SandboxProvisioned",
    "ToolSessionBound",
    "ActionIdentityBound",
    "FilesystemScoped",
    "NetworkScoped",
    "ShellPolicyBound",
    "SecretReferencesBound",
    "ActionPlanAdmitted"
}

(* Helper: mark a security state as visited. *)
MarkVisited(s, prev) ==
    [session EXCEPT !.visited = prev.visited \union {s}]

(* -----------------------------------------------------------------
   SECTION B — INGRESS & WAKE ACQUISITION
   ----------------------------------------------------------------- *)

EventArrives(src) ==
    /\ lifecycle = "Resting"
    /\ src \in EventSources
    /\ lifecycle' = "IngressRedacting"
    /\ security'  = "IntentSubmitted"
    /\ session'   = [session EXCEPT
                         !.iteration         = 0,
                         !.intent_id         = session.intent_id + 1,
                         !.input_source      = src,
                         !.has_secret        = FALSE,
                         !.priv_audited      = FALSE,
                         !.visited           = { "IntentSubmitted" },
                         !.plan_steps        = 0,
                         !.current_step      = 0,
                         !.envelope_required = (src = "agent")]

WebhookArrives ==
    /\ lifecycle = "Resting"
    /\ lifecycle' = "WebhookReceived"
    /\ UNCHANGED <<security, session>>

WebhookDeduplicates ==
    /\ lifecycle = "WebhookReceived"
    /\ lifecycle' = "WebhookDeduplicating"
    /\ UNCHANGED <<security, session>>

WebhookNormalizes ==
    /\ lifecycle = "WebhookDeduplicating"
    /\ lifecycle' = "IngressRedacting"
    /\ security'  = "IntentSubmitted"
    /\ session'   = [session EXCEPT
                         !.iteration         = 0,
                         !.intent_id         = session.intent_id + 1,
                         !.input_source      = "webhook",
                         !.has_secret        = FALSE,
                         !.priv_audited      = FALSE,
                         !.visited           = { "IntentSubmitted" },
                         !.plan_steps        = 0,
                         !.current_step      = 0,
                         !.envelope_required = FALSE]

(* A7: Redact known-secret patterns BEFORE durable log INSERT. *)
RedactIngressSecrets ==
    /\ lifecycle = "IngressRedacting"
    /\ security  = "IntentSubmitted"
    /\ lifecycle' = "EventArrived"
    /\ security'  = "IngressRedacted"
    /\ session'   = MarkVisited("IngressRedacted", session)

AttemptWakeAcquire ==
    /\ lifecycle = "EventArrived"
    /\ lifecycle' = "WakeAcquiring"
    /\ UNCHANGED <<security, session>>

WakeAcquireSucceeds ==
    /\ lifecycle = "WakeAcquiring"
    /\ lifecycle' = "PromptAssembling"
    /\ session'   = [session EXCEPT !.wake_id = session.wake_id + 1,
                                     !.iteration = 0]
    /\ UNCHANGED security

WakeAcquireFails ==
    /\ lifecycle = "WakeAcquiring"
    /\ lifecycle' = "WakeAcquireFailed"
    /\ UNCHANGED <<security, session>>

FailedInvocationExits ==
    /\ lifecycle = "WakeAcquireFailed"
    /\ lifecycle' = "Resting"
    /\ UNCHANGED <<security, session>>

(* -----------------------------------------------------------------
   SECTION C — INTAKE

   G3: AuditChainVerified is an EARLY gate. The agent's event log
   is hash-chained; before any new work, the chain is replayed and
   verified against the last signed root. A broken chain routes
   straight to FailureAuditPending (do not process an intent on a
   tampered log).

   G4: If source=agent, the inter-agent envelope (signed sender id,
   sender policy version, capability proof) is verified before any
   content reaches the classifier.
   ----------------------------------------------------------------- *)

VerifyAuditChain ==
    /\ lifecycle = "PromptAssembling"
    /\ security  = "IngressRedacted"
    /\ security' = "AuditChainVerified"
    /\ session'  = MarkVisited("AuditChainVerified", session)
    /\ UNCHANGED lifecycle

AuditChainBroken ==
    /\ lifecycle = "PromptAssembling"
    /\ security  = "IngressRedacted"
    /\ security' = "FailureAuditPending"
    /\ session'  = MarkVisited("FailureAuditPending", session)
    /\ UNCHANGED lifecycle

VerifyIdentity ==
    /\ lifecycle = "PromptAssembling"
    /\ security  = "AuditChainVerified"
    /\ security' = "IdentityVerified"
    /\ session'  = MarkVisited("IdentityVerified", session)
    /\ UNCHANGED lifecycle

BindOperator ==
    /\ security  = "IdentityVerified"
    /\ security' = "OperatorBound"
    /\ session'  = MarkVisited("OperatorBound", session)
    /\ UNCHANGED lifecycle

VerifyOperatorMfa ==
    /\ Tier = "enterprise"
    /\ security  = "OperatorBound"
    /\ security' = "OperatorMfaVerified"
    /\ session'  = MarkVisited("OperatorMfaVerified", session)
    /\ UNCHANGED lifecycle

ResolveTenant ==
    /\ Tier \in { "multi_tenant", "enterprise" }
    /\ security \in { "OperatorBound", "OperatorMfaVerified" }
    /\ security' = "TenantResolved"
    /\ session'  = MarkVisited("TenantResolved", session)
    /\ UNCHANGED lifecycle

BindTenantIsolation ==
    /\ Tier \in { "multi_tenant", "enterprise" }
    /\ security  = "TenantResolved"
    /\ security' = "TenantIsolationBound"
    /\ session'  = MarkVisited("TenantIsolationBound", session)
    /\ UNCHANGED lifecycle

(* G4: inter-agent envelope verification — gates ClassifyInput. *)
VerifyInterAgentEnvelope ==
    /\ session.envelope_required = TRUE
    /\ security \in { "OperatorBound", "OperatorMfaVerified",
                       "TenantIsolationBound" }
    /\ security' = "InterAgentEnvelopeVerified"
    /\ session'  = MarkVisited("InterAgentEnvelopeVerified", session)
    /\ UNCHANGED lifecycle

NormalizeRequest ==
    /\ (\/ session.envelope_required = FALSE
        \/ "InterAgentEnvelopeVerified" \in session.visited)
    /\ security \in { "OperatorBound", "OperatorMfaVerified",
                       "TenantIsolationBound",
                       "InterAgentEnvelopeVerified" }
    /\ security' = "RequestNormalized"
    /\ session'  = MarkVisited("RequestNormalized", session)
    /\ UNCHANGED lifecycle

TagProvenance ==
    /\ security  = "RequestNormalized"
    /\ security' = "ProvenanceTagged"
    /\ session'  = MarkVisited("ProvenanceTagged", session)
    /\ UNCHANGED lifecycle

VerifyPolicySignature ==
    /\ Tier = "enterprise"
    /\ security  = "ProvenanceTagged"
    /\ security' = "PolicySignatureVerified"
    /\ session'  = MarkVisited("PolicySignatureVerified", session)
    /\ UNCHANGED lifecycle

CheckPolicyVersion ==
    /\ security \in { "ProvenanceTagged", "PolicySignatureVerified" }
    /\ security' = "PolicyVersionChecked"
    /\ session'  = [MarkVisited("PolicyVersionChecked", session)
                        EXCEPT !.policy_version = PolicyBundleVersion]
    /\ UNCHANGED lifecycle

ClassifyInput ==
    /\ security  = "PolicyVersionChecked"
    /\ security' = "InputClassified"
    /\ session'  = MarkVisited("InputClassified", session)
    /\ UNCHANGED lifecycle

(* C3: guarded on PromptAssembling. *)
QuarantineInput ==
    /\ lifecycle = "PromptAssembling"
    /\ security  = "InputClassified"
    /\ session.input_source \in UntrustedSources
    /\ security' = "InputQuarantined"
    /\ session'  = MarkVisited("InputQuarantined", session)
    /\ UNCHANGED lifecycle

RejectQuarantinedInput ==
    /\ security  = "InputQuarantined"
    /\ security' = "DenialAuditPending"
    /\ session'  = MarkVisited("DenialAuditPending", session)
    /\ UNCHANGED lifecycle

PromptAssemblyCompletes ==
    /\ lifecycle = "PromptAssembling"
    /\ security  = "InputClassified"
    /\ lifecycle' = "Awake"
    /\ UNCHANGED <<security, session>>

QueueMemoryRead ==
    /\ lifecycle = "Awake"
    /\ security  = "InputClassified"
    /\ security' = "MemoryReadPending"
    /\ session'  = MarkVisited("MemoryReadPending", session)
    /\ UNCHANGED lifecycle

CheckMemoryProvenance ==
    /\ security  = "MemoryReadPending"
    /\ security' = "MemoryProvenanceChecked"
    /\ session'  = MarkVisited("MemoryProvenanceChecked", session)
    /\ UNCHANGED lifecycle

QuarantineMemory ==
    /\ lifecycle = "Awake"
    /\ security  = "MemoryProvenanceChecked"
    /\ security' = "MemoryQuarantined"
    /\ session'  = MarkVisited("MemoryQuarantined", session)
    /\ UNCHANGED lifecycle

RejectQuarantinedMemory ==
    /\ security  = "MemoryQuarantined"
    /\ security' = "DenialAuditPending"
    /\ session'  = MarkVisited("DenialAuditPending", session)
    /\ UNCHANGED lifecycle

MergeMemory ==
    /\ security  = "MemoryProvenanceChecked"
    /\ security' = "MemoryMerged"
    /\ session'  = MarkVisited("MemoryMerged", session)
    /\ UNCHANGED lifecycle

(* -----------------------------------------------------------------
   SECTION D — PLANNING
   ----------------------------------------------------------------- *)

DraftPlan ==
    /\ lifecycle = "Awake"
    /\ security  = "MemoryMerged"
    /\ security' = "PlanDrafted"
    /\ session'  = MarkVisited("PlanDrafted", session)
    /\ UNCHANGED lifecycle

(* ValidatePlan records the admitted plan length (G6). Real impl
   records the ordered tool-call types; the spec abstracts to a
   length counter that IssueToolCall decrements against. *)
ValidatePlan(n) ==
    /\ security  = "PlanDrafted"
    /\ n \in 1..MaxPlanSteps
    /\ security' = "PlanValidated"
    /\ session'  = [MarkVisited("PlanValidated", session)
                        EXCEPT !.plan_steps   = n,
                               !.current_step = 0]
    /\ UNCHANGED lifecycle

AnyValidatePlan == \E n \in 1..MaxPlanSteps : ValidatePlan(n)

ScopeCapabilities ==
    /\ security  = "PlanValidated"
    /\ security' = "CapabilityScoped"
    /\ session'  = MarkVisited("CapabilityScoped", session)
    /\ UNCHANGED lifecycle

AttenuateCapabilities ==
    /\ security  = "CapabilityScoped"
    /\ security' = "CapabilityAttenuated"
    /\ session'  = MarkVisited("CapabilityAttenuated", session)
    /\ UNCHANGED lifecycle

ReserveBudget ==
    /\ security  = "CapabilityAttenuated"
    /\ session.budget_used < MaxBudgetCents
    /\ security' = "BudgetReserved"
    /\ session'  = MarkVisited("BudgetReserved", session)
    /\ UNCHANGED lifecycle

ScoreRisk ==
    /\ security  = "BudgetReserved"
    /\ security' = "RiskScored"
    /\ session'  = MarkVisited("RiskScored", session)
    /\ UNCHANGED lifecycle

(* -----------------------------------------------------------------
   SECTION E — MODEL RESPONSE + APPROVAL  (G2)

   After each LLM turn, the raw response is captured and scanned
   BEFORE the runtime dispatches a tool or emits a human-visible
   message. This runs egress-DLP patterns (PII, secret regex,
   base64/unicode-hidden payloads) and enforces response-schema
   validity. Rejected responses route to FailureAuditPending.
   ----------------------------------------------------------------- *)

(* B2 fix: a second model turn after a completed tool call starts
   from lifecycle = Awake with security in {PrivilegeUsageAudited,
   ResultValidated}. Accept those as valid starting points. *)
ModelReturns ==
    /\ lifecycle = "Awake"
    /\ security  \in { "RiskScored", "ActionStepCompleted",
                        "ModelResponseApproved",
                        "PrivilegeUsageAudited", "ResultValidated" }
    /\ lifecycle' = "ModelResponseProcessing"
    /\ security'  = "ModelResponseReceived"
    /\ session'   = [MarkVisited("ModelResponseReceived", session)
                         EXCEPT !.budget_used =
                                    session.budget_used + CostPerCall]

ScanModelResponse ==
    /\ security  = "ModelResponseReceived"
    /\ security' = "ModelResponseScanned"
    /\ session'  = MarkVisited("ModelResponseScanned", session)
    /\ UNCHANGED lifecycle

ApproveModelResponse ==
    /\ security  = "ModelResponseScanned"
    /\ security' = "ModelResponseApproved"
    /\ lifecycle' = "Awake"
    /\ session'  = MarkVisited("ModelResponseApproved", session)

RejectModelResponse ==
    /\ security  = "ModelResponseScanned"
    /\ security' = "ModelResponseRejected"
    /\ session'  = MarkVisited("ModelResponseRejected", session)
    /\ UNCHANGED lifecycle

RouteRejectedResponse ==
    /\ security  = "ModelResponseRejected"
    /\ security' = "FailureAuditPending"
    /\ session'  = MarkVisited("FailureAuditPending", session)
    /\ UNCHANGED lifecycle

(* AgentCallsTool now requires the response was approved AND the
   iteration cap has not been reached. If the cap is reached,
   IterationCapReached fires instead (F3). *)
AgentCallsTool ==
    /\ lifecycle = "Awake"
    /\ security  = "ModelResponseApproved"
    /\ session.iteration < MaxIterations
    /\ lifecycle' = "ToolDispatching"
    /\ UNCHANGED <<security, session>>

ToolDispatches ==
    /\ lifecycle = "ToolDispatching"
    /\ lifecycle' = "ToolPermissionChecking"
    /\ UNCHANGED <<security, session>>

RequestApproval ==
    /\ lifecycle = "ToolPermissionChecking"
    /\ security  = "ModelResponseApproved"
    /\ lifecycle' = "AwaitingApproval"
    /\ security'  = "ApprovalPending"
    /\ session'   = MarkVisited("ApprovalPending", session)

RequestDualControl ==
    /\ Tier = "enterprise"
    /\ security  = "ApprovalPending"
    /\ security' = "DualControlPending"
    /\ session'  = MarkVisited("DualControlPending", session)
    /\ UNCHANGED lifecycle

(* B6 (TLC): require AuditChainVerified in visited so authorization
   cannot precede the full audit/plan chain. Without this guard, a
   SuspendWorkflow -> ResumeAfterSuspension shortcut reaches
   ExecutionAuthorized bypassing Inv_AuditChainBeforeExecution. *)
AuthorizeExecution ==
    /\ lifecycle \in { "AwaitingApproval", "ToolPermissionChecking" }
    /\ security  \in { "ApprovalPending", "DualControlPending",
                        "ModelResponseApproved" }
    /\ "AuditChainVerified" \in session.visited
    /\ lifecycle' = "ToolExecuting"
    /\ security'  = "ExecutionAuthorized"
    /\ session'   = MarkVisited("ExecutionAuthorized", session)

ExpireApproval ==
    /\ security \in { "ApprovalPending", "DualControlPending",
                       "ChildApprovalPending", "MemoryWriteReviewPending" }
    /\ security' = "ApprovalExpired"
    /\ session'  = MarkVisited("ApprovalExpired", session)
    /\ UNCHANGED lifecycle

RouteApprovalExpiry ==
    /\ security  = "ApprovalExpired"
    /\ security' = "DenialAuditPending"
    /\ session'  = MarkVisited("DenialAuditPending", session)
    /\ UNCHANGED lifecycle

ToolPermissionDenies ==
    /\ lifecycle = "ToolPermissionChecking"
    /\ lifecycle' = "ApprovalRejected"
    /\ UNCHANGED <<security, session>>

RejectedToolReturnsToAwake ==
    /\ lifecycle = "ApprovalRejected"
    /\ lifecycle' = "Awake"
    /\ UNCHANGED <<security, session>>

(* G6: plan-step conformance — verify the about-to-issue tool call
   matches the admitted plan's next step. *)
CheckPlanStep ==
    /\ lifecycle = "ToolExecuting"
    /\ security  = "ExecutionAuthorized"
    /\ session.current_step < session.plan_steps
    /\ security' = "PlanStepChecked"
    /\ session'  = [MarkVisited("PlanStepChecked", session)
                        EXCEPT !.current_step = session.current_step + 1]
    /\ UNCHANGED lifecycle

DetectPlanStepViolation ==
    /\ lifecycle = "ToolExecuting"
    /\ security  = "ExecutionAuthorized"
    /\ session.current_step >= session.plan_steps
    /\ security' = "PlanStepViolation"
    /\ session'  = MarkVisited("PlanStepViolation", session)
    /\ UNCHANGED lifecycle

RoutePlanStepViolation ==
    /\ security  = "PlanStepViolation"
    /\ security' = "DenialAuditPending"
    /\ session'  = MarkVisited("DenialAuditPending", session)
    /\ UNCHANGED lifecycle

(* -----------------------------------------------------------------
   SECTION F — SANDBOX PROVISIONING & BINDING
   ----------------------------------------------------------------- *)

VerifyExecutionImage ==
    /\ Tier = "enterprise"
    /\ lifecycle = "ToolExecuting"
    /\ security  = "PlanStepChecked"
    /\ security' = "ImageVerified"
    /\ session'  = MarkVisited("ImageVerified", session)
    /\ UNCHANGED lifecycle

AttestSandbox ==
    /\ Tier = "enterprise"
    /\ lifecycle = "ToolExecuting"
    /\ security  = "ImageVerified"
    /\ security' = "SandboxAttested"
    /\ session'  = MarkVisited("SandboxAttested", session)
    /\ UNCHANGED lifecycle

(* B4 fix: ExecutionAuthorized is a valid entry to sandbox
   provisioning when G6 (PlanStepChecked) is not active. Section W
   defers G6 from the v1 ship, so v1 uses the ExecutionAuthorized
   path; when G6 is enabled, CheckPlanStep fires first and provides
   PlanStepChecked. *)
ProvisionSandbox ==
    /\ lifecycle = "ToolExecuting"
    /\ security  \in { "ExecutionAuthorized", "PlanStepChecked",
                        "ImageVerified", "SandboxAttested" }
    /\ security' = "SandboxProvisioned"
    /\ session'  = MarkVisited("SandboxProvisioned", session)
    /\ UNCHANGED lifecycle

BindToolSession ==
    /\ security  = "SandboxProvisioned"
    /\ security' = "ToolSessionBound"
    /\ session'  = MarkVisited("ToolSessionBound", session)
    /\ UNCHANGED lifecycle

BindActionIdentity ==
    /\ security  = "ToolSessionBound"
    /\ security' = "ActionIdentityBound"
    /\ session'  = MarkVisited("ActionIdentityBound", session)
    /\ UNCHANGED lifecycle

ScopeFilesystem ==
    /\ security  = "ActionIdentityBound"
    /\ security' = "FilesystemScoped"
    /\ session'  = MarkVisited("FilesystemScoped", session)
    /\ UNCHANGED lifecycle

ScopeNetwork ==
    /\ security  = "FilesystemScoped"
    /\ security' = "NetworkScoped"
    /\ session'  = MarkVisited("NetworkScoped", session)
    /\ UNCHANGED lifecycle

BindEgressPolicy ==
    /\ security  = "NetworkScoped"
    /\ security' = "EgressPolicyBound"
    /\ session'  = MarkVisited("EgressPolicyBound", session)
    /\ UNCHANGED lifecycle

BindBrowserPolicy ==
    /\ Tier = "enterprise"
    /\ security  = "EgressPolicyBound"
    /\ security' = "BrowserPolicyBound"
    /\ session'  = MarkVisited("BrowserPolicyBound", session)
    /\ UNCHANGED lifecycle

BindShellPolicy ==
    /\ security \in { "EgressPolicyBound", "BrowserPolicyBound" }
    /\ security' = "ShellPolicyBound"
    /\ session'  = MarkVisited("ShellPolicyBound", session)
    /\ UNCHANGED lifecycle

BindSecretReferences ==
    /\ security  = "ShellPolicyBound"
    /\ security' = "SecretReferencesBound"
    /\ session'  = MarkVisited("SecretReferencesBound", session)
    /\ UNCHANGED lifecycle

AdmitActionPlan ==
    /\ security  = "SecretReferencesBound"
    /\ security' = "ActionPlanAdmitted"
    /\ session'  = MarkVisited("ActionPlanAdmitted", session)
    /\ UNCHANGED lifecycle

(* -----------------------------------------------------------------
   SECTION G — TOOL CALL
   ----------------------------------------------------------------- *)

IssueToolCall ==
    /\ lifecycle = "ToolExecuting"
    /\ security  \in { "ActionPlanAdmitted", "ActionStepCompleted" }
    /\ session.iteration < MaxIterations
    /\ \A s \in RequiredBindingStates : s \in session.visited
    /\ security' = "ToolCallIssued"
    /\ session'  = [MarkVisited("ToolCallIssued", session)
                        EXCEPT !.iteration    = session.iteration + 1,
                               !.priv_audited = FALSE,
                               !.budget_used  = session.budget_used + CostPerCall]
    /\ UNCHANGED lifecycle

ReceiveToolResult ==
    /\ security  = "ToolCallIssued"
    /\ security' = "ToolResultReceived"
    /\ lifecycle' = "ToolResultProcessing"
    /\ session'  = MarkVisited("ToolResultReceived", session)

(* -----------------------------------------------------------------
   SECTION H — TOOL RESULT PIPELINE

   A3: secret scan is an explicit transition. After the scan,
   session.has_secret is definitively set, and the sanitize vs.
   capture fork is deterministic.
   ----------------------------------------------------------------- *)

ScanToolResultForSecrets(found) ==
    /\ security  = "ToolResultReceived"
    /\ found \in BOOLEAN
    /\ security' = "ToolResultSecretScanned"
    /\ session'  = [MarkVisited("ToolResultSecretScanned", session)
                        EXCEPT !.has_secret = found]
    /\ UNCHANGED lifecycle

AnyScanToolResultForSecrets ==
    \E found \in BOOLEAN : ScanToolResultForSecrets(found)

DetectGeneratedSecret ==
    /\ security  = "ToolResultSecretScanned"
    /\ session.has_secret = TRUE
    /\ security' = "SecretCapturePending"
    /\ session'  = MarkVisited("SecretCapturePending", session)
    /\ UNCHANGED lifecycle

PersistGeneratedSecret ==
    /\ security  = "SecretCapturePending"
    /\ security' = "SecretPersisting"
    /\ session'  = MarkVisited("SecretPersisting", session)
    /\ UNCHANGED lifecycle

BindSecretCapability ==
    /\ security  = "SecretPersisting"
    /\ security' = "SecretBound"
    /\ session'  = [MarkVisited("SecretBound", session)
                        EXCEPT !.has_secret = FALSE]
    /\ UNCHANGED lifecycle

ReturnAfterSecretBind ==
    /\ security  = "SecretBound"
    /\ security' = "ToolResultSanitized"
    /\ session'  = MarkVisited("ToolResultSanitized", session)
    /\ UNCHANGED lifecycle

SanitizeToolResult ==
    /\ security  = "ToolResultSecretScanned"
    /\ session.has_secret = FALSE
    /\ security' = "ToolResultSanitized"
    /\ session'  = MarkVisited("ToolResultSanitized", session)
    /\ UNCHANGED lifecycle

ClassifyToolResult ==
    /\ security  = "ToolResultSanitized"
    /\ security' = "ToolResultClassified"
    /\ session'  = MarkVisited("ToolResultClassified", session)
    /\ UNCHANGED lifecycle

AcceptLowRiskToolResult ==
    /\ security  = "ToolResultClassified"
    /\ security' = "ToolResultLowRisk"
    /\ session'  = MarkVisited("ToolResultLowRisk", session)
    /\ UNCHANGED lifecycle

EndorseToolResult ==
    /\ security  = "ToolResultClassified"
    /\ security' = "ToolResultEndorsed"
    /\ session'  = MarkVisited("ToolResultEndorsed", session)
    /\ UNCHANGED lifecycle

DenyToolEndorsement ==
    /\ security  = "ToolResultClassified"
    /\ security' = "EndorsementDenied"
    /\ session'  = MarkVisited("EndorsementDenied", session)
    /\ UNCHANGED lifecycle

RouteEndorsementDenial ==
    /\ security  = "EndorsementDenied"
    /\ security' = "DenialAuditPending"
    /\ session'  = MarkVisited("DenialAuditPending", session)
    /\ UNCHANGED lifecycle

RejectToolResult ==
    /\ security  = "ToolResultClassified"
    /\ security' = "ToolResultRejected"
    /\ session'  = MarkVisited("ToolResultRejected", session)
    /\ UNCHANGED lifecycle

EscalateRejectedToolResult ==
    /\ security  = "ToolResultRejected"
    /\ security' = "FailureAuditPending"
    /\ session'  = MarkVisited("FailureAuditPending", session)
    /\ UNCHANGED lifecycle

(* A2: explicit boundary — classified bytes enter messages[] only
   here. *)
ToolResultEnterContext ==
    /\ security  \in { "ToolResultLowRisk", "ToolResultEndorsed" }
    /\ security' = "ToolResultEnteredContext"
    /\ session'  = MarkVisited("ToolResultEnteredContext", session)
    /\ UNCHANGED lifecycle

CompleteActionStep ==
    /\ security  = "ToolResultEnteredContext"
    /\ security' = "ActionStepCompleted"
    /\ session'  = MarkVisited("ActionStepCompleted", session)
    /\ UNCHANGED lifecycle

AuditPrivilegeUsage ==
    /\ security  = "ActionStepCompleted"
    /\ security' = "PrivilegeUsageAudited"
    /\ session'  = [MarkVisited("PrivilegeUsageAudited", session)
                        EXCEPT !.priv_audited = TRUE]
    /\ UNCHANGED lifecycle

DetectPrivilegeViolation ==
    /\ security  = "PrivilegeUsageAudited"
    /\ security' = "PrivilegeViolationDetected"
    /\ session'  = MarkVisited("PrivilegeViolationDetected", session)
    /\ UNCHANGED lifecycle

EscalatePrivilegeViolation ==
    /\ security  = "PrivilegeViolationDetected"
    /\ security' = "IsolationBreachDetected"
    /\ session'  = [MarkVisited("IsolationBreachDetected", session)
                        EXCEPT !.breach_flag = TRUE]
    /\ UNCHANGED lifecycle

SanitizeResult ==
    /\ security  = "PrivilegeUsageAudited"
    /\ security' = "ResultSanitized"
    /\ session'  = MarkVisited("ResultSanitized", session)
    /\ UNCHANGED lifecycle

ValidateResult ==
    /\ security  = "ResultSanitized"
    /\ security' = "ResultValidated"
    /\ session'  = MarkVisited("ResultValidated", session)
    /\ UNCHANGED lifecycle

ToolResultProcessedToolLoop ==
    /\ lifecycle = "ToolResultProcessing"
    /\ security \in { "PrivilegeUsageAudited", "ResultValidated" }
    /\ session.priv_audited = TRUE
    /\ lifecycle' = "MidWakeEventPolling"
    /\ UNCHANGED <<security, session>>

(* -----------------------------------------------------------------
   SECTION I — MID-WAKE INJECTION & WAKE TERMINATION

   G1: injected events are re-classified before they join messages[].
   Quarantined injections suspend the intent (not silently accept).
   ----------------------------------------------------------------- *)

MidWakePollFindsNothing ==
    /\ lifecycle = "MidWakeEventPolling"
    /\ lifecycle' = "Awake"
    /\ UNCHANGED <<security, session>>

MidWakePollFindsEvents ==
    /\ lifecycle = "MidWakeEventPolling"
    /\ lifecycle' = "EventInjecting"
    /\ UNCHANGED <<security, session>>

ClassifyInjectedEvent ==
    /\ lifecycle = "EventInjecting"
    /\ security' = "InjectedEventClassified"
    /\ session'  = MarkVisited("InjectedEventClassified", session)
    /\ UNCHANGED lifecycle

QuarantineInjectedEvent ==
    /\ lifecycle = "EventInjecting"
    /\ security  = "InjectedEventClassified"
    /\ security' = "InjectedEventQuarantined"
    /\ session'  = MarkVisited("InjectedEventQuarantined", session)
    /\ UNCHANGED lifecycle

(* B1 fix: Suspended requires lifecycle = AwaitingApproval so
   ResumeAfterSuspension has a valid exit. Otherwise the intent is
   wedged with lifecycle = EventInjecting. *)
RouteInjectedQuarantine ==
    /\ security  = "InjectedEventQuarantined"
    /\ security' = "Suspended"
    /\ lifecycle' = "AwaitingApproval"
    /\ session'  = MarkVisited("Suspended", session)

EventsInjected ==
    /\ lifecycle = "EventInjecting"
    /\ security  = "InjectedEventClassified"
    /\ lifecycle' = "Awake"
    /\ UNCHANGED <<security, session>>

(* F1 fix: AgentCallsSleep commits security to SuccessAuditPending
   atomically, so the subsequent audit chain reaches Completed
   before TerminalEndsWake can unwind the wake. This eliminates
   the race where lifecycle reaches Resting with security still
   non-terminal. Sleep is only a valid exit from "healthy" security
   states (post-classification, pre-terminal). *)
AgentCallsSleep ==
    /\ lifecycle = "Awake"
    /\ security \in { "ModelResponseApproved",
                       "RiskScored", "ActionStepCompleted",
                       "PrivilegeUsageAudited", "ResultValidated",
                       "MemoryMerged" }
    /\ lifecycle' = "ExplicitSleeping"
    /\ security'  = "SuccessAuditPending"
    /\ session'   = MarkVisited("SuccessAuditPending", session)

(* F2 fix: direct-response path commits the audit atomically with
   entering ImplicitSleeping. Previously QueueDirectResponseAudit
   raced ImplicitSleepEndsWake and could be skipped. *)
AgentRespondsToHuman ==
    /\ lifecycle = "Awake"
    /\ security  = "ModelResponseApproved"
    /\ lifecycle' = "ImplicitSleeping"
    /\ security'  = "SuccessAuditPending"
    /\ session'   = MarkVisited("SuccessAuditPending", session)

(* F3 fix: iteration cap fires at the decision point (Awake) so the
   agent is never committed down the tool-call path with no exit.
   Routes to FailureAuditPending — an iteration cap is a system-
   imposed failure, not a graceful success. *)
IterationCapReached ==
    /\ lifecycle = "Awake"
    /\ session.iteration >= MaxIterations
    /\ lifecycle' = "IterationCapHit"
    /\ security'  = "FailureAuditPending"
    /\ session'   = MarkVisited("FailureAuditPending", session)

(* F3 fix: context cap likewise fires at the decision point. *)
ContextCapReached ==
    /\ lifecycle = "Awake"
    /\ lifecycle' = "ContextCapHit"
    /\ security'  = "FailureAuditPending"
    /\ session'   = MarkVisited("FailureAuditPending", session)

ExhaustBudget ==
    /\ lifecycle \in { "ToolResultProcessing", "Awake",
                        "ModelResponseProcessing", "ToolExecuting" }
    /\ session.budget_used >= MaxBudgetCents
    /\ lifecycle' = "BudgetCapHit"
    /\ security'  = "BudgetExhausted"
    /\ session'   = MarkVisited("BudgetExhausted", session)

BudgetExhaustedRoutesToFailure ==
    /\ security  = "BudgetExhausted"
    /\ security' = "FailureAuditPending"
    /\ session'  = MarkVisited("FailureAuditPending", session)
    /\ UNCHANGED lifecycle

(* F5: ExplicitSleepEndsWake, ImplicitSleepEndsWake,
   IterationCapEndsWake, ContextCapEndsWake, BudgetCapEndsWake
   DELETED. They are covered by TerminalEndsWake, which now fires
   once security reaches a terminal. The previous formulations
   allowed lifecycle to end the wake BEFORE security committed to
   a terminal, causing intents to leak across wakes and breaking
   Liveness_IntentTerminates. *)

WakeEndTransitionsToMaintenance ==
    /\ lifecycle = "WakeEnding"
    /\ lifecycle' = "EventCollapsing"
    /\ UNCHANGED <<security, session>>

(* -----------------------------------------------------------------
   SECTION J — MAINTENANCE / DRAIN / STALE
   ----------------------------------------------------------------- *)

EventCollapseRuns ==
    /\ lifecycle = "EventCollapsing"
    /\ lifecycle' = "MaintenanceCalling"
    /\ UNCHANGED <<security, session>>

MaintenanceCallCompletes ==
    /\ lifecycle = "MaintenanceCalling"
    /\ lifecycle' = "MaintenanceWriting"
    /\ UNCHANGED <<security, session>>

MaintenanceWritesProjections ==
    /\ lifecycle = "MaintenanceWriting"
    /\ lifecycle' = "SummaryWriting"
    /\ UNCHANGED <<security, session>>

SummaryWritten ==
    /\ lifecycle = "SummaryWriting"
    /\ lifecycle' = "Draining"
    /\ UNCHANGED <<security, session>>

DrainFindsNothing ==
    /\ lifecycle = "Draining"
    /\ lifecycle' = "Resting"
    /\ UNCHANGED <<security, session>>

DrainFindsEvents ==
    /\ lifecycle = "Draining"
    /\ lifecycle' = "DrainAcquiring"
    /\ UNCHANGED <<security, session>>

(* A4: cross-intent resume is not modeled yet. If security is still
   non-terminal when drain-acquire runs, that means the prior wake
   ended on Suspended / ApprovalPending / budget-exhausted-retry;
   resume semantics for those cases are a v4 intent-stack feature.
   For now, drain only advances when the prior intent is terminal. *)
DrainAcquireSucceeds ==
    /\ lifecycle = "DrainAcquiring"
    /\ security  \in SecurityTerminals
    /\ lifecycle' = "IngressRedacting"
    /\ security'  = "IntentSubmitted"
    /\ session'   = [session EXCEPT
                         !.wake_id           = session.wake_id + 1,
                         !.intent_id         = session.intent_id + 1,
                         !.iteration         = 0,
                         !.has_secret        = FALSE,
                         !.priv_audited      = FALSE,
                         !.visited           = { "IntentSubmitted" },
                         !.plan_steps        = 0,
                         !.current_step      = 0,
                         !.envelope_required = FALSE]

(* B10 (TLC): stale-wake watchdog atomically tears down security as
   well as lifecycle. Without this, (security=ToolCallIssued,
   lifecycle=StaleDetected) violates Inv_ToolCallRequiresBinding.
   Semantically: a stale wake is an abort; any live mid-pipeline
   state must be routed to the failure-audit chain. *)
StaleWakeDetected ==
    /\ lifecycle \in InWakeStates
    /\ lifecycle' = "StaleDetected"
    /\ security' =
         IF security \in (SecurityTerminals \union
                           { "FailureAuditPending",
                             "FailureAuditCommitted",
                             "AuditChainCommitted_Failure",
                             "FailureCleanupPending" })
         THEN security
         ELSE "FailureAuditPending"
    /\ session'  =
         IF security \in (SecurityTerminals \union
                           { "FailureAuditPending",
                             "FailureAuditCommitted",
                             "AuditChainCommitted_Failure",
                             "FailureCleanupPending" })
         THEN session
         ELSE MarkVisited("FailureAuditPending", session)

StaleWakeRecovered ==
    /\ lifecycle = "StaleDetected"
    /\ lifecycle' = "Resting"
    /\ UNCHANGED <<security, session>>

(* -----------------------------------------------------------------
   SECTION K — MEMORY WRITE REVIEW  (G5)
   ----------------------------------------------------------------- *)

QueueMemoryWriteReview ==
    /\ security  = "ResultValidated"
    /\ security' = "MemoryWriteReviewPending"
    /\ session'  = MarkVisited("MemoryWriteReviewPending", session)
    /\ UNCHANGED lifecycle

(* G5: content classifier on memory writes — detects poisoned
   payloads that passed ToolResultClassified but carry instructions
   for future intents. *)
ClassifyMemoryWriteContent ==
    /\ security  = "MemoryWriteReviewPending"
    /\ security' = "MemoryWriteContentClassified"
    /\ session'  = MarkVisited("MemoryWriteContentClassified", session)
    /\ UNCHANGED lifecycle

QuarantineMemoryWriteContent ==
    /\ security  = "MemoryWriteContentClassified"
    /\ security' = "MemoryWriteContentQuarantined"
    /\ session'  = MarkVisited("MemoryWriteContentQuarantined", session)
    /\ UNCHANGED lifecycle

RouteMemoryContentQuarantine ==
    /\ security  = "MemoryWriteContentQuarantined"
    /\ security' = "DenialAuditPending"
    /\ session'  = MarkVisited("DenialAuditPending", session)
    /\ UNCHANGED lifecycle

RejectMemoryWrite ==
    /\ security  = "MemoryWriteContentClassified"
    /\ security' = "MemoryWriteRejected"
    /\ session'  = MarkVisited("MemoryWriteRejected", session)
    /\ UNCHANGED lifecycle

RouteMemoryWriteRejection ==
    /\ security  = "MemoryWriteRejected"
    /\ security' = "DenialAuditPending"
    /\ session'  = MarkVisited("DenialAuditPending", session)
    /\ UNCHANGED lifecycle

ApproveMemoryWrite ==
    /\ security  = "MemoryWriteContentClassified"
    /\ security' = "MemoryWritePending"
    /\ session'  = MarkVisited("MemoryWritePending", session)
    /\ UNCHANGED lifecycle

(* -----------------------------------------------------------------
   SECTION L — TERMINALS with AUDIT CHAIN COMMITMENT (G3)
   ----------------------------------------------------------------- *)

QueueSuccessAudit ==
    /\ security \in { "MemoryWritePending", "ResultValidated" }
    /\ security' = "SuccessAuditPending"
    /\ session'  = MarkVisited("SuccessAuditPending", session)
    /\ UNCHANGED lifecycle

CommitSuccessAudit ==
    /\ security  = "SuccessAuditPending"
    /\ security' = "SuccessAuditCommitted"
    /\ session'  = MarkVisited("SuccessAuditCommitted", session)
    /\ UNCHANGED lifecycle

CommitSuccessChain ==
    /\ security  = "SuccessAuditCommitted"
    /\ security' = "AuditChainCommitted_Success"
    /\ session'  = MarkVisited("AuditChainCommitted_Success", session)
    /\ UNCHANGED lifecycle

QueueSuccessCleanup ==
    /\ security  = "AuditChainCommitted_Success"
    /\ security' = "SuccessCleanupPending"
    /\ session'  = MarkVisited("SuccessCleanupPending", session)
    /\ UNCHANGED lifecycle

CompleteWorkflow ==
    /\ security  = "SuccessCleanupPending"
    /\ security' = "Completed"
    /\ session'  = MarkVisited("Completed", session)
    /\ UNCHANGED lifecycle

CommitDenialAudit ==
    /\ security  = "DenialAuditPending"
    /\ security' = "DenialAuditCommitted"
    /\ session'  = MarkVisited("DenialAuditCommitted", session)
    /\ UNCHANGED lifecycle

CommitDenialChain ==
    /\ security  = "DenialAuditCommitted"
    /\ security' = "AuditChainCommitted_Denial"
    /\ session'  = MarkVisited("AuditChainCommitted_Denial", session)
    /\ UNCHANGED lifecycle

QueueDeniedCleanup ==
    /\ security  = "AuditChainCommitted_Denial"
    /\ security' = "DenialCleanupPending"
    /\ session'  = MarkVisited("DenialCleanupPending", session)
    /\ UNCHANGED lifecycle

FinishDenied ==
    /\ security  = "DenialCleanupPending"
    /\ security' = "Denied"
    /\ session'  = MarkVisited("Denied", session)
    /\ UNCHANGED lifecycle

CommitFailureAudit ==
    /\ security  = "FailureAuditPending"
    /\ security' = "FailureAuditCommitted"
    /\ session'  = MarkVisited("FailureAuditCommitted", session)
    /\ UNCHANGED lifecycle

CommitFailureChain ==
    /\ security  = "FailureAuditCommitted"
    /\ security' = "AuditChainCommitted_Failure"
    /\ session'  = MarkVisited("AuditChainCommitted_Failure", session)
    /\ UNCHANGED lifecycle

QueueFailureCleanup ==
    /\ security  = "AuditChainCommitted_Failure"
    /\ security' = "FailureCleanupPending"
    /\ session'  = MarkVisited("FailureCleanupPending", session)
    /\ UNCHANGED lifecycle

FinishFailed ==
    /\ security  = "FailureCleanupPending"
    /\ security' = "Failed"
    /\ session'  = MarkVisited("Failed", session)
    /\ UNCHANGED lifecycle

(* -----------------------------------------------------------------
   SECTION M — ISOLATION / SUSPENSION / POLICY DRIFT
   ----------------------------------------------------------------- *)

EngageContainment ==
    /\ security  = "IsolationBreachDetected"
    /\ security' = "ContainmentEngaged"
    /\ session'  = MarkVisited("ContainmentEngaged", session)
    /\ UNCHANGED lifecycle

EscalateContainmentToRevocation ==
    /\ security  = "ContainmentEngaged"
    /\ security' = "RevocationAuditPending"
    /\ session'  = MarkVisited("RevocationAuditPending", session)
    /\ UNCHANGED lifecycle

(* B7 (TLC): fire at most once per wake. Without the visited guard,
   Detect and Suspend ping-pong (Suspended -> Detect -> Suspend ...)
   violating liveness. *)
DetectPolicyVersionMismatch ==
    /\ security \notin (SecurityTerminals \union
                         { "IntentSubmitted", "PolicyVersionMismatch" })
    /\ "PolicyVersionMismatch" \notin session.visited
    /\ security' = "PolicyVersionMismatch"
    /\ session'  = MarkVisited("PolicyVersionMismatch", session)
    /\ UNCHANGED lifecycle

HandlePolicyVersionMismatch ==
    /\ security  = "PolicyVersionMismatch"
    /\ security' = "RevocationAuditPending"
    /\ session'  = MarkVisited("RevocationAuditPending", session)
    /\ UNCHANGED lifecycle

(* C4: suspension moves lifecycle to AwaitingApproval so resume has
   a valid exit. B9 (TLC): fire at most once per wake, otherwise
   Suspend <-> Expire <-> Route livelocks. *)
SuspendWorkflow ==
    /\ security \notin (SecurityTerminals \union
                         { "IntentSubmitted", "Suspended",
                           "SuspensionExpired" })
    /\ "Suspended" \notin session.visited
    /\ security' = "Suspended"
    /\ lifecycle' = "AwaitingApproval"
    /\ session'  = MarkVisited("Suspended", session)

(* B5 (TLC): resume only restores to ApprovalPending if approval was
   actually pending pre-suspension. Otherwise suspending from early
   pipeline states would manufacture a fake approval path. *)
ResumeAfterSuspension ==
    /\ lifecycle = "AwaitingApproval"
    /\ security  = "Suspended"
    /\ "ApprovalPending" \in session.visited
    /\ security' = "ApprovalPending"
    /\ session'  = MarkVisited("ApprovalPending", session)
    /\ UNCHANGED lifecycle

ExpireSuspension ==
    /\ security  = "Suspended"
    /\ security' = "SuspensionExpired"
    /\ session'  = MarkVisited("SuspensionExpired", session)
    /\ UNCHANGED lifecycle

RouteSuspensionExpiry ==
    /\ security  = "SuspensionExpired"
    /\ security' = "RevocationAuditPending"
    /\ session'  = MarkVisited("RevocationAuditPending", session)
    /\ UNCHANGED lifecycle

CommitRevocationAudit ==
    /\ security  = "RevocationAuditPending"
    /\ security' = "RevocationAuditCommitted"
    /\ session'  = MarkVisited("RevocationAuditCommitted", session)
    /\ UNCHANGED lifecycle

CommitRevocationChain ==
    /\ security  = "RevocationAuditCommitted"
    /\ security' = "AuditChainCommitted_Revocation"
    /\ session'  = MarkVisited("AuditChainCommitted_Revocation", session)
    /\ UNCHANGED lifecycle

PropagateChildRevocation ==
    /\ security  = "AuditChainCommitted_Revocation"
    /\ security' = "ChildRevocationPropagating"
    /\ session'  = MarkVisited("ChildRevocationPropagating", session)
    /\ UNCHANGED lifecycle

QueueRevocationCleanup ==
    /\ security  = "ChildRevocationPropagating"
    /\ security' = "RevocationCleanupPending"
    /\ session'  = MarkVisited("RevocationCleanupPending", session)
    /\ UNCHANGED lifecycle

FinishRevoked ==
    /\ security  = "RevocationCleanupPending"
    /\ security' = "Revoked"
    /\ session'  = MarkVisited("Revoked", session)
    /\ UNCHANGED lifecycle

(* -----------------------------------------------------------------
   SECTION N — CHILD AGENT LINEAGE
   ----------------------------------------------------------------- *)

RequestChildSpawn ==
    /\ security  = "ActionStepCompleted"
    /\ security' = "ChildSpawnRequested"
    /\ session'  = MarkVisited("ChildSpawnRequested", session)
    /\ UNCHANGED lifecycle

ScopeChildCapabilities ==
    /\ security  = "ChildSpawnRequested"
    /\ security' = "ChildCapabilityScoped"
    /\ session'  = MarkVisited("ChildCapabilityScoped", session)
    /\ UNCHANGED lifecycle

BindChildBudget ==
    /\ security  = "ChildCapabilityScoped"
    /\ security' = "ChildBudgetBound"
    /\ session'  = MarkVisited("ChildBudgetBound", session)
    /\ UNCHANGED lifecycle

BindChildIdentity ==
    /\ security  = "ChildBudgetBound"
    /\ security' = "ChildIdentityBound"
    /\ session'  = MarkVisited("ChildIdentityBound", session)
    /\ UNCHANGED lifecycle

VerifyChildLineage ==
    /\ security  = "ChildIdentityBound"
    /\ security' = "ChildLineageVerified"
    /\ session'  = MarkVisited("ChildLineageVerified", session)
    /\ UNCHANGED lifecycle

QueueChildApproval ==
    /\ security  = "ChildLineageVerified"
    /\ security' = "ChildApprovalPending"
    /\ session'  = MarkVisited("ChildApprovalPending", session)
    /\ UNCHANGED lifecycle

ApproveChildSpawn ==
    /\ security  = "ChildApprovalPending"
    /\ security' = "ChildLineageAuditPending"
    /\ session'  = MarkVisited("ChildLineageAuditPending", session)
    /\ UNCHANGED lifecycle

CommitChildLineageAudit ==
    /\ security  = "ChildLineageAuditPending"
    /\ security' = "ChildLineageAuditCommitted"
    /\ session'  = MarkVisited("ChildLineageAuditCommitted", session)
    /\ UNCHANGED lifecycle

RegisterChild ==
    /\ security  = "ChildLineageAuditCommitted"
    /\ security' = "ChildRegistered"
    /\ session'  = [MarkVisited("ChildRegistered", session)
                        EXCEPT !.child_count = session.child_count + 1]
    /\ UNCHANGED lifecycle

ReturnFromChildRegistration ==
    /\ security  = "ChildRegistered"
    /\ security' = "ActionStepCompleted"
    /\ session'  = MarkVisited("ActionStepCompleted", session)
    /\ UNCHANGED lifecycle

(* -----------------------------------------------------------------
   SECTION O — SECRET ROTATION / REVOCATION
   ----------------------------------------------------------------- *)

StartSecretRotation ==
    /\ security  = "ActionStepCompleted"
    /\ security' = "SecretRotationPending"
    /\ session'  = MarkVisited("SecretRotationPending", session)
    /\ UNCHANGED lifecycle

FinishSecretRotation ==
    /\ security  = "SecretRotationPending"
    /\ security' = "ActionStepCompleted"
    /\ session'  = MarkVisited("ActionStepCompleted", session)
    /\ UNCHANGED lifecycle

StartSecretRevocation ==
    /\ security  = "ActionStepCompleted"
    /\ security' = "SecretRevocationPending"
    /\ session'  = MarkVisited("SecretRevocationPending", session)
    /\ UNCHANGED lifecycle

FinishSecretRevocation ==
    /\ security  = "SecretRevocationPending"
    /\ security' = "ActionStepCompleted"
    /\ session'  = MarkVisited("ActionStepCompleted", session)
    /\ UNCHANGED lifecycle

(* -----------------------------------------------------------------
   SECTION U — TERMINAL UNWINDER
   ----------------------------------------------------------------- *)

TerminalEndsWake ==
    /\ security  \in SecurityTerminals
    /\ lifecycle \in (InWakeStates \ { "WakeEnding", "EventCollapsing",
                                        "MaintenanceCalling",
                                        "MaintenanceWriting",
                                        "SummaryWriting", "Draining",
                                        "DrainAcquiring" })
    /\ lifecycle' = "WakeEnding"
    /\ UNCHANGED <<security, session>>

(* -----------------------------------------------------------------
   SECTION W — TRUST FLOOR (v1 SHIP SUBSET)

   v3 above is the design ceiling. It is the canonical reference
   that future iterations read against. It is NOT the v1 BUILD
   target.

   For the v1 ship of a self-directed, self-modifying agent on
   Tier = "self_host_individual", the minimum trust floor is the
   following eight controls. They are irreducible — removing any
   one makes the system unsafe to leave running unattended.

     T1  Sandbox binding (RequiredBindingStates all visited
         before IssueToolCall).
         Invariant:    Inv_ToolCallRequiresBinding (reduced form —
                       drop PlanStepChecked, keep ModelResponseApproved).
         Actions:      ProvisionSandbox, BindToolSession,
                       BindActionIdentity, ScopeFilesystem,
                       ScopeNetwork, BindEgressPolicy,
                       BindShellPolicy, BindSecretReferences,
                       AdmitActionPlan, IssueToolCall.
         Evidence:     INV-2.

     T2  Tool-result secret scan + capture.
         Invariant:    session.has_secret detector is the ONLY
                       path to messages[] for any tool_result.
         Actions:      ScanToolResultForSecrets,
                       DetectGeneratedSecret, PersistGeneratedSecret,
                       BindSecretCapability, ReturnAfterSecretBind,
                       SanitizeToolResult.
         Evidence:     INV-1, INV-4.

     T3  Input classification + quarantine.
         Invariant:    Inv_AwakeRequiresClassification.
         Actions:      ClassifyInput, QuarantineInput,
                       RejectQuarantinedInput.
         Evidence:     INV-3.

     T4  Terminal audit commit.
         Invariant:    Inv_TerminalSuccession (reduced to require
                       *AuditCommitted only; drop
                       AuditChainCommitted_* until G3 lands).
         Actions:      QueueSuccessAudit/CommitSuccessAudit/
                       CompleteWorkflow (+ denied/failed variants).
         Evidence:     INV-5.

     T5  Budget + iteration caps.
         Invariant:    Inv_IterationBounded, budget_used <= MaxBudgetCents.
         Actions:      IssueToolCall (budget++), ExhaustBudget,
                       IterationCapReached.
         Evidence:     tests/budget_test.rs, tests/iteration_cap_test.rs.

     T6  Mid-wake injection reclassification (G1).
         Invariant:    Inv_InjectedEventReclassified:
                         (lifecycle = "EventInjecting" /\ succ state = "Awake")
                         => "InjectedEventClassified" \in session.visited.
         Actions:      ClassifyInjectedEvent, QuarantineInjectedEvent,
                       RouteInjectedQuarantine.
         Evidence:     INV-10.

     T7  Model output scan before tool dispatch (G2).
         Invariant:    Inv_ModelOutputScanned.
         Actions:      ModelReturns, ScanModelResponse,
                       ApproveModelResponse, RejectModelResponse,
                       RouteRejectedResponse.
         Evidence:     INV-11.

     T8  Memory-write content classification (G5).
         Invariant:    Inv_MemoryContentClassifiedBeforePersist.
         Actions:      ClassifyMemoryWriteContent,
                       QuarantineMemoryWriteContent,
                       RouteMemoryContentQuarantine, ApproveMemoryWrite.
         Evidence:     INV-13.

   EVERYTHING ELSE is deferred. Explicit deferral list:

     Deferred to v2 ship (next ITERATE increment):
       G3  Audit chain hash integrity (AuditChainVerified /
           AuditChainCommitted_*, INV-9).
           Rationale: append-only log + operator-readable export
           is sufficient for v1 trust; cryptographic chain is a
           hardening pass.
       G6  Plan-step conformance (PlanStepChecked, INV-15).
           Rationale: T5 + T7 already bound runaway. G6 is
           conformance, not safety.

     Deferred to when second agent exists:
       G4  Inter-agent envelope (INV-12).
       Section N child-agent lineage.

     Deferred to when long-lived bound secrets exist:
       Section O secret rotation/revocation.

     Enterprise-tier (already Tier-gated, no BUILD impact for
     self-host v1):
       OperatorMfaVerified, TenantResolved, TenantIsolationBound,
       PolicySignatureVerified, DualControlPending, ImageVerified,
       SandboxAttested, BrowserPolicyBound.

     Documented v4+ / future work:
       G7  Capability nonce per IssueToolCall.
       G8  Ephemeral sandbox per tool call.
       G9  Model response schema + consistency cross-check.
       G10 Prompt template signed-artifact verification.
       G11 Real time / monotonic nonce.
       G12 Budget anomaly detection.
       G13 Approval UI operator-signature.
       G14 Side-channel / resource monitoring.
       G15 Child prompt attestation.
       A4  Cross-intent resume via intent_stack.

   BUILD CONTRACT:

     - scope.md "Acceptance Criteria" for v1 ship cites only
       T1..T8 controls.
     - readiness.md coverage table maps every v1 AC to one of
       T1..T8 with its action names.
     - readiness.md checks InvariantsV1 (not Invariants).
     - BUILD cannot fail the post-build gate for deferred items.
     - VERIFY evidence quotes T1..T8 canonical action names.
     - Each ITERATE increment (v2, v3, ...) moves items OUT of
       the deferred list into active AC coverage, preserving all
       v1 AC IDs unchanged, and moves the corresponding
       invariant from the ceiling set into the active set.
   ----------------------------------------------------------------- *)

(* -----------------------------------------------------------------
   SECTION V — EVIDENCE OBLIGATIONS

   For each invariant, verify evidence is one of:
     neg  — a negative test that SHOULD fail without the control
     trc  — a trace property on the event log
     fuzz — a fuzz corpus that must not produce the bad trace
     scope — a BUILD-time scope-gate scan

   INV-1  No raw secrets in model stream.
     neg:  synthesize tool result containing AWS AKIA / JWT / ghp_ /
           SSH private-key header / base64-encoded high-entropy.
           Assert:
             (a) ScanToolResultForSecrets fires and sets has_secret.
             (b) DetectGeneratedSecret → SecretPersisting → SecretBound
                 occurs before any event with event_type='tool_result'
                 is persisted.
             (c) Persisted tool_result body contains no match for
                 any secret regex.
             (d) messages[] and wake_summary contain no match.
     seam: tests/tool_result_secret_capture_test.rs

   INV-2  No tool call without full sandbox binding.
     trc:  for every ToolCallIssued log record, the prior records
           for the same intent_id contain all RequiredBindingStates
           transitions.
           Enterprise adds { ImageVerified, SandboxAttested,
                             BrowserPolicyBound }.
     seam: tests/sandbox_binding_sequence_test.rs

   INV-3  No LLM exposure without input classification.
     neg:  7+ canonical injection patterns must produce
           ClassifyInput → QuarantineInput → RejectQuarantinedInput
           → DenialAuditPending → Denied.
     pos:  benign input reaches InputClassified → Awake.
     seam: tests/input_classifier_test.rs

   INV-4  tool_result event stores no raw generated secret.
     fuzz: scan entire events table against the regex corpus
           after running the fuzz suite.
     seam: tests/event_log_secret_scan_test.rs

   INV-5  No terminal without its audit chain commit.
     trc:  for every Completed/Denied/Failed/Revoked record, the
           AuditChainCommitted_<Pipeline> state is in the intent's
           visited set (equivalently, its transition appears in log).
     seam: tests/terminal_audit_chain_test.rs

   INV-6  No wake overlap.
     trc:  Postgres CAS test — two simultaneous wake invocations;
           exactly one succeeds.
     seam: tests/wake_cas_concurrency_test.rs

   INV-7  Breach always terminates in Revoked or Failed.
     fault: trigger PrivilegeViolationDetected; assert terminal
            ∈ {Revoked, Failed} with session.breach_flag = TRUE.
     seam:  tests/privilege_breach_termination_test.rs

   INV-8  Tier conformance.
     scope: at BUILD-time, fail if any AC in readiness.md cites
            an EnterpriseOnlyState when Tier is self_host_individual.
     seam:  tests/tier_conformance_scope_test.rs

   INV-9  Audit log hash-chain integrity (G3).
     neg:  tamper with an event row in Postgres; next intent must
           route through AuditChainBroken → FailureAuditPending
           without entering any tool-execution state.
     seam: tests/audit_chain_tamper_test.rs

   INV-10 Mid-wake injection re-classification (G1).
     neg:  inject an event mid-wake containing an injection pattern;
           assert ClassifyInjectedEvent → QuarantineInjectedEvent →
           RouteInjectedQuarantine → Suspended. Confirm no tool
           call fires between injection and suspension.
     seam: tests/midwake_injection_test.rs

   INV-11 Model output DLP (G2).
     neg:  instrument the model with responses containing PII /
           secrets / steganographic unicode. Assert ModelResponseScanned
           → ModelResponseRejected → RouteRejectedResponse without
           AgentCallsTool firing.
     seam: tests/model_response_dlp_test.rs

   INV-12 Inter-agent envelope verification (G4).
     neg:  send an unsigned or replayed inter-agent message; assert
           the intent never reaches InputClassified.
     seam: tests/inter_agent_envelope_test.rs

   INV-13 Memory write content classification (G5).
     neg:  seed a tool result that passes ToolResultClassified but
           contains memory-poisoning text ("when you next see X, ...");
           assert ClassifyMemoryWriteContent → QuarantineMemoryWriteContent
           → RouteMemoryContentQuarantine → Denied without any
           memory-projection row being written.
     seam: tests/memory_poisoning_test.rs

   INV-14 Plan-step conformance (G6).
     neg:  after a plan of N steps, force a model-proposed (N+1)th
           tool call; assert DetectPlanStepViolation →
           RoutePlanStepViolation → Denied.
     seam: tests/plan_conformance_test.rs
   ----------------------------------------------------------------- *)

(* -----------------------------------------------------------------
   SECTION P — INIT / NEXT / SPEC / INVARIANTS / LIVENESS
   ----------------------------------------------------------------- *)

Init ==
    /\ lifecycle = "Resting"
    /\ security  = "IntentSubmitted"
    /\ session = [ wake_id           |-> 0,
                   intent_id         |-> 0,
                   iteration         |-> 0,
                   budget_used       |-> 0,
                   policy_version    |-> 0,
                   child_count       |-> 0,
                   breach_flag       |-> FALSE,
                   input_source      |-> "none",
                   has_secret        |-> FALSE,
                   priv_audited      |-> FALSE,
                   visited           |-> { "IntentSubmitted" },
                   plan_steps        |-> 0,
                   current_step      |-> 0,
                   envelope_required |-> FALSE ]

AnyEventArrives == \E src \in EventSources : EventArrives(src)

Next ==
    \/ AnyEventArrives \/ WebhookArrives \/ WebhookDeduplicates
    \/ WebhookNormalizes \/ RedactIngressSecrets
    \/ AttemptWakeAcquire \/ WakeAcquireSucceeds
    \/ WakeAcquireFails \/ FailedInvocationExits
    \/ VerifyAuditChain \/ AuditChainBroken
    \/ VerifyIdentity \/ BindOperator \/ VerifyOperatorMfa
    \/ ResolveTenant \/ BindTenantIsolation
    \/ VerifyInterAgentEnvelope \/ NormalizeRequest
    \/ TagProvenance \/ VerifyPolicySignature \/ CheckPolicyVersion
    \/ ClassifyInput \/ QuarantineInput \/ RejectQuarantinedInput
    \/ PromptAssemblyCompletes \/ QueueMemoryRead
    \/ CheckMemoryProvenance \/ QuarantineMemory
    \/ RejectQuarantinedMemory \/ MergeMemory
    \/ DraftPlan \/ AnyValidatePlan \/ ScopeCapabilities
    \/ AttenuateCapabilities \/ ReserveBudget \/ ScoreRisk
    \/ ModelReturns \/ ScanModelResponse \/ ApproveModelResponse
    \/ RejectModelResponse \/ RouteRejectedResponse
    \/ AgentCallsTool \/ ToolDispatches \/ RequestApproval
    \/ RequestDualControl \/ AuthorizeExecution
    \/ ExpireApproval \/ RouteApprovalExpiry
    \/ ToolPermissionDenies \/ RejectedToolReturnsToAwake
    \/ CheckPlanStep \/ DetectPlanStepViolation \/ RoutePlanStepViolation
    \/ VerifyExecutionImage \/ AttestSandbox \/ ProvisionSandbox
    \/ BindToolSession \/ BindActionIdentity \/ ScopeFilesystem
    \/ ScopeNetwork \/ BindEgressPolicy \/ BindBrowserPolicy
    \/ BindShellPolicy \/ BindSecretReferences \/ AdmitActionPlan
    \/ IssueToolCall \/ ReceiveToolResult \/ AnyScanToolResultForSecrets
    \/ DetectGeneratedSecret \/ PersistGeneratedSecret
    \/ BindSecretCapability \/ ReturnAfterSecretBind
    \/ SanitizeToolResult \/ ClassifyToolResult
    \/ AcceptLowRiskToolResult \/ EndorseToolResult
    \/ DenyToolEndorsement \/ RouteEndorsementDenial
    \/ RejectToolResult \/ EscalateRejectedToolResult
    \/ ToolResultEnterContext \/ CompleteActionStep
    \/ AuditPrivilegeUsage \/ DetectPrivilegeViolation
    \/ EscalatePrivilegeViolation \/ SanitizeResult
    \/ ValidateResult \/ ToolResultProcessedToolLoop
    \/ MidWakePollFindsNothing \/ MidWakePollFindsEvents
    \/ ClassifyInjectedEvent \/ QuarantineInjectedEvent
    \/ RouteInjectedQuarantine \/ EventsInjected
    \/ AgentCallsSleep \/ AgentRespondsToHuman
    \/ IterationCapReached \/ ContextCapReached
    \/ ExhaustBudget \/ BudgetExhaustedRoutesToFailure
    \/ WakeEndTransitionsToMaintenance \/ EventCollapseRuns
    \/ MaintenanceCallCompletes \/ MaintenanceWritesProjections
    \/ SummaryWritten \/ DrainFindsNothing \/ DrainFindsEvents
    \/ DrainAcquireSucceeds \/ StaleWakeDetected \/ StaleWakeRecovered
    \/ QueueMemoryWriteReview \/ ClassifyMemoryWriteContent
    \/ QuarantineMemoryWriteContent \/ RouteMemoryContentQuarantine
    \/ RejectMemoryWrite \/ RouteMemoryWriteRejection \/ ApproveMemoryWrite
    \/ QueueSuccessAudit \/ CommitSuccessAudit \/ CommitSuccessChain
    \/ QueueSuccessCleanup \/ CompleteWorkflow
    \/ CommitDenialAudit \/ CommitDenialChain \/ QueueDeniedCleanup
    \/ FinishDenied
    \/ CommitFailureAudit \/ CommitFailureChain \/ QueueFailureCleanup
    \/ FinishFailed
    \/ EngageContainment \/ EscalateContainmentToRevocation
    \/ DetectPolicyVersionMismatch \/ HandlePolicyVersionMismatch
    \/ SuspendWorkflow \/ ResumeAfterSuspension \/ ExpireSuspension
    \/ RouteSuspensionExpiry
    \/ CommitRevocationAudit \/ CommitRevocationChain
    \/ PropagateChildRevocation \/ QueueRevocationCleanup
    \/ FinishRevoked
    \/ RequestChildSpawn \/ ScopeChildCapabilities
    \/ BindChildBudget \/ BindChildIdentity \/ VerifyChildLineage
    \/ QueueChildApproval \/ ApproveChildSpawn
    \/ CommitChildLineageAudit \/ RegisterChild
    \/ ReturnFromChildRegistration
    \/ StartSecretRotation \/ FinishSecretRotation
    \/ StartSecretRevocation \/ FinishSecretRevocation
    \/ TerminalEndsWake

vars == <<lifecycle, security, session>>

Spec ==
    /\ Init
    /\ [][Next]_vars
    (* B8 (TLC): WF_vars(Next) eliminates arbitrary stuttering so
       liveness properties can actually be checked. Individual WF
       clauses on commit-chain actions remain below to ensure the
       right terminal is chosen where multiple are enabled. *)
    /\ WF_vars(Next)
    /\ WF_vars(TerminalEndsWake)
    /\ WF_vars(WakeEndTransitionsToMaintenance)
    /\ WF_vars(EventCollapseRuns)
    /\ WF_vars(MaintenanceCallCompletes)
    /\ WF_vars(MaintenanceWritesProjections)
    /\ WF_vars(SummaryWritten)
    /\ WF_vars(DrainFindsNothing)
    /\ WF_vars(CommitSuccessAudit) /\ WF_vars(CommitSuccessChain)
    /\ WF_vars(QueueSuccessCleanup) /\ WF_vars(CompleteWorkflow)
    /\ WF_vars(CommitDenialAudit) /\ WF_vars(CommitDenialChain)
    /\ WF_vars(QueueDeniedCleanup) /\ WF_vars(FinishDenied)
    /\ WF_vars(CommitFailureAudit) /\ WF_vars(CommitFailureChain)
    /\ WF_vars(QueueFailureCleanup) /\ WF_vars(FinishFailed)
    /\ WF_vars(CommitRevocationAudit) /\ WF_vars(CommitRevocationChain)
    /\ WF_vars(PropagateChildRevocation) /\ WF_vars(QueueRevocationCleanup)
    /\ WF_vars(FinishRevoked)
    /\ WF_vars(RouteApprovalExpiry)
    /\ WF_vars(RouteSuspensionExpiry)
    /\ WF_vars(HandlePolicyVersionMismatch)
    /\ WF_vars(StaleWakeRecovered)
    /\ WF_vars(ExpireSuspension)
    /\ WF_vars(ExpireApproval)

(* -----------------------------------------------------------------
   SAFETY INVARIANTS
   ----------------------------------------------------------------- *)

Inv_Type == TypeOK

\* INV-2: tool call only after all required binding states visited.
\* PlanStepChecked is NOT required here — that is G6, enforced by
\* Inv_PlanConformance separately (full ceiling, deferred in v1).
Inv_ToolCallRequiresBinding ==
    (security = "ToolCallIssued") =>
        /\ lifecycle = "ToolExecuting"
        /\ \A s \in RequiredBindingStates : s \in session.visited
        /\ "ModelResponseApproved" \in session.visited

\* INV-3: Awake requires intake pipeline visited.
Inv_AwakeRequiresClassification ==
    (lifecycle = "Awake") =>
        /\ "IngressRedacted"      \in session.visited
        /\ "AuditChainVerified"   \in session.visited
        /\ "IdentityVerified"     \in session.visited
        /\ "OperatorBound"        \in session.visited
        /\ "PolicyVersionChecked" \in session.visited
        /\ "InputClassified"      \in session.visited

\* INV-5: terminal implies full audit-chain commit.
Inv_TerminalSuccession ==
    /\ (security = "Completed") =>
            /\ "SuccessAuditCommitted"       \in session.visited
            /\ "AuditChainCommitted_Success" \in session.visited
    /\ (security = "Denied") =>
            /\ "DenialAuditCommitted"        \in session.visited
            /\ "AuditChainCommitted_Denial"  \in session.visited
    /\ (security = "Failed") =>
            /\ "FailureAuditCommitted"       \in session.visited
            /\ "AuditChainCommitted_Failure" \in session.visited
    /\ (security = "Revoked") =>
            /\ "RevocationAuditCommitted"         \in session.visited
            /\ "AuditChainCommitted_Revocation"   \in session.visited

\* INV-6: single-agent spec — at most one in-wake lifecycle at once
\* is vacuously true. Real check is in tests/wake_cas_concurrency_test.rs.
Inv_NoWakeOverlap == TRUE

\* INV-7: breach flag excludes Completed.
Inv_BreachBounded ==
    (session.breach_flag = TRUE) => (security # "Completed")

\* INV-8: tier conformance.
Inv_TierConformance ==
    (Tier = "self_host_individual") =>
        session.visited \cap EnterpriseOnlyStates = {}

\* INV-9: iteration cap.
Inv_IterationBounded ==
    session.iteration \in 0..MaxIterations

\* INV-10: priv audit between tool calls.
Inv_PrivAuditedBeforeNextCall ==
    (lifecycle = "MidWakeEventPolling") => session.priv_audited = TRUE

\* INV-11: model output scanned before tool dispatch (G2).
Inv_ModelOutputScanned ==
    (lifecycle \in { "ToolDispatching", "ToolPermissionChecking",
                      "ToolExecuting" }) =>
        "ModelResponseApproved" \in session.visited

\* INV-12: audit chain verified before any tool execution (G3).
Inv_AuditChainBeforeExecution ==
    (lifecycle = "ToolExecuting") =>
        "AuditChainVerified" \in session.visited

\* INV-13: inter-agent envelope verified when source=agent (G4).
Inv_EnvelopeVerifiedForAgentSource ==
    (/\ session.envelope_required = TRUE
     /\ "InputClassified" \in session.visited) =>
        "InterAgentEnvelopeVerified" \in session.visited

\* INV-14: memory persistence only after content classification (G5).
Inv_MemoryContentClassifiedBeforePersist ==
    (security = "MemoryWritePending") =>
        "MemoryWriteContentClassified" \in session.visited

\* INV-15: plan conformance — no tool call beyond admitted steps (G6).
Inv_PlanConformance ==
    (security = "ToolCallIssued") =>
        /\ session.plan_steps > 0
        /\ session.current_step <= session.plan_steps
        /\ "PlanStepChecked" \in session.visited

Invariants ==
    /\ Inv_Type
    /\ Inv_ToolCallRequiresBinding
    /\ Inv_AwakeRequiresClassification
    /\ Inv_TerminalSuccession
    /\ Inv_NoWakeOverlap
    /\ Inv_BreachBounded
    /\ Inv_TierConformance
    /\ Inv_IterationBounded
    /\ Inv_PrivAuditedBeforeNextCall
    /\ Inv_ModelOutputScanned
    /\ Inv_AuditChainBeforeExecution
    /\ Inv_EnvelopeVerifiedForAgentSource
    /\ Inv_MemoryContentClassifiedBeforePersist
    /\ Inv_PlanConformance

(* InvariantsV1: the subset active in the v1 ship (Section W trust
   floor). Drops ONLY invariants whose corresponding actions do not
   fire in v1:
     Inv_PlanConformance — G6 CheckPlanStep deferred, so
                           PlanStepChecked is never in visited.
   Inv_AuditChainBeforeExecution and
   Inv_EnvelopeVerifiedForAgentSource are RETAINED: their actions
   (VerifyAuditChain, VerifyInterAgentEnvelope) do fire in v1, as
   cosmetic log-append and vacuous-when-not-agent-source checks.
   G3 deferral means we don't add cryptographic strength to the
   underlying log, not that the transition is skipped. *)
InvariantsV1 ==
    /\ Inv_Type
    /\ Inv_ToolCallRequiresBinding
    /\ Inv_AwakeRequiresClassification
    /\ Inv_TerminalSuccession
    /\ Inv_NoWakeOverlap
    /\ Inv_BreachBounded
    /\ Inv_TierConformance
    /\ Inv_IterationBounded
    /\ Inv_PrivAuditedBeforeNextCall
    /\ Inv_ModelOutputScanned
    /\ Inv_AuditChainBeforeExecution
    /\ Inv_EnvelopeVerifiedForAgentSource
    /\ Inv_MemoryContentClassifiedBeforePersist

(* C7: Init satisfies Invariants. Stated as a THEOREM so SANY
   accepts it (ASSUME must be constant-level). Verified mechanically
   by TLC's initial-state invariant check at startup. *)
THEOREM InitSatisfiesInvariants == Init => Invariants

(* -----------------------------------------------------------------
   LIVENESS
   ----------------------------------------------------------------- *)

Liveness_IntentTerminates ==
    (security \notin SecurityTerminals) ~> (security \in SecurityTerminals)

Liveness_WakeTerminates ==
    (lifecycle \in InWakeStates) ~> (lifecycle = "Resting")

Liveness_BreachIsolates ==
    (session.breach_flag = TRUE) ~> (security \in { "Revoked", "Failed" })

(* -----------------------------------------------------------------
   V4 CANDIDATES (NOT IN v3)

   Documented as explicit future work so BUILD cannot silently claim
   coverage:

     G7  Capability freshness / nonce per IssueToolCall.
         Add: session.cap_nonce, CapabilityAttenuated sets it,
              IssueToolCall requires fresh.
     G8  Ephemeral sandbox per tool call.
         Add: TeardownSandbox between ActionStepCompleted and the
              next IssueToolCall; rebind all sandbox states.
     G9  Model response schema + consistency cross-check.
         Add: ModelResponseSchemaChecked, ModelResponseConsistent.
     G10 Prompt template signed-artifact verification.
         Add: PromptTemplateVerified as a PromptAssembling gate.
     G11 Real time / monotonic nonce.
         Add: VARIABLE now : Nat; ExpireApproval guarded by now
              against approval_pending_since; webhook replay check
              against signed timestamp.

   ENTERPRISE-TIER (Tier = "enterprise"):

     G12 Budget anomaly detection.
     G13 Approval UI operator-signature.
     G14 Side-channel / resource monitoring into
         DetectPrivilegeViolation.
     G15 Child prompt attestation via child_prompt_digest.

   CROSS-INTENT RESUME:

     A4  Drain-acquire currently requires previous intent to be
         terminal. Mid-intent sleep (approval-pending, suspension,
         budget-exhausted-retry) needs an intent stack:
           session.intent_stack : Seq(SecurityStates)
         Push on AgentCallsSleep mid-intent, pop on
         DrainAcquireSucceeds. TerminalEndsWake pops; new
         EventArrives pushes a fresh intent.
   ----------------------------------------------------------------- *)

====
