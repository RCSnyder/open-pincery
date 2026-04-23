---- MODULE AgenticOsSecureBehaviorV2 ----

VARIABLE systemState

SystemStages == {
"IntentSubmitted",
"IdentityVerified",
"OperatorBound",
"OperatorMfaVerified",
"TenantResolved",
"TenantIsolationBound",
"RequestNormalized",
"ProvenanceTagged",
"PolicySignatureVerified",
"PolicyVersionChecked",
"PolicyVersionMismatch",
"InputClassified",
"InputQuarantined",
"MemoryReadPending",
"MemoryProvenanceChecked",
"MemoryQuarantined",
"MemoryMerged",
"PlanDrafted",
"PlanValidated",
"CapabilityScoped",
"CapabilityAttenuated",
"BudgetReserved",
"BudgetExhausted",
"RiskScored",
"ApprovalPending",
"ApprovalExpired",
"DualControlPending",
"ExecutionAuthorized",
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
"ToolCallIssued",
"ToolResultReceived",
"ToolResultSanitized",
"ToolResultClassified",
"ToolResultLowRisk",
"ToolResultRejected",
"ToolResultEndorsed",
"EndorsementDenied",
"ActionStepCompleted",
"ChildSpawnRequested",
"ChildCapabilityScoped",
"ChildBudgetBound",
"ChildIdentityBound",
"ChildLineageVerified",
"ChildApprovalPending",
"ChildLineageAuditPending",
"ChildLineageAuditCommitted",
"ChildRegistered",
"SecretCapturePending",
"SecretPersisting",
"SecretBound",
"SecretRotationPending",
"SecretRevocationPending",
"PrivilegeUsageAudited",
"PrivilegeViolationDetected",
"ResultSanitized",
"ResultValidated",
"MemoryWriteReviewPending",
"MemoryWriteRejected",
"MemoryWritePending",
"SuccessAuditPending",
"SuccessAuditCommitted",
"SuccessCleanupPending",
"DenialAuditPending",
"DenialAuditCommitted",
"DenialCleanupPending",
"FailureAuditPending",
"FailureAuditCommitted",
"FailureCleanupPending",
"IsolationBreachDetected",
"ContainmentEngaged",
"RevocationAuditPending",
"RevocationAuditCommitted",
"ChildRevocationPropagating",
"RevocationCleanupPending",
"Suspended",
"SuspensionExpired",
"Completed",
"Denied",
"Revoked",
"Failed"
}

(* Identity is verified before any privileged handling. *)
VerifyIdentity ==
/\ systemState = "IntentSubmitted"
/\ systemState' = "IdentityVerified"

(* The verified principal is bound to an operator session. *)
BindOperator ==
/\ systemState = "IdentityVerified"
/\ systemState' = "OperatorBound"

(* Strong operator authentication is completed before routing continues. *)
VerifyOperatorMfa ==
/\ systemState = "OperatorBound"
/\ systemState' = "OperatorMfaVerified"

(* The request is attached to the correct tenant. *)
ResolveTenant ==
/\ systemState = "OperatorMfaVerified"
/\ systemState' = "TenantResolved"

(* Tenant isolation keys, namespaces, and quotas are bound. *)
BindTenantIsolation ==
/\ systemState = "TenantResolved"
/\ systemState' = "TenantIsolationBound"

(* The incoming request is normalized into a canonical form. *)
NormalizeRequest ==
/\ systemState = "TenantIsolationBound"
/\ systemState' = "RequestNormalized"

(* All inbound bytes are tagged with provenance. *)
TagProvenance ==
/\ systemState = "RequestNormalized"
/\ systemState' = "ProvenanceTagged"

(* The active policy bundle is authenticated before use. *)
VerifyPolicySignature ==
/\ systemState = "ProvenanceTagged"
/\ systemState' = "PolicySignatureVerified"

(* The policy version is checked before classification proceeds. *)
CheckPolicyVersion ==
/\ systemState = "PolicySignatureVerified"
/\ systemState' = "PolicyVersionChecked"

(* Inputs are classified for trust, integrity, and confidentiality. *)
ClassifyInput ==
/\ systemState = "PolicyVersionChecked"
/\ systemState' = "InputClassified"

(* Suspicious input is quarantined instead of merged. *)
QuarantineInput ==
/\ systemState = "InputClassified"
/\ systemState' = "InputQuarantined"

(* Quarantined input is rejected from the workflow. *)
RejectQuarantinedInput ==
/\ systemState = "InputQuarantined"
/\ systemState' = "DenialAuditPending"

(* Policy-aware retrieval loads durable memory for the request. *)
QueueMemoryRead ==
/\ systemState = "InputClassified"
/\ systemState' = "MemoryReadPending"

(* Retrieved memory is checked for provenance and integrity. *)
CheckMemoryProvenance ==
/\ systemState = "MemoryReadPending"
/\ systemState' = "MemoryProvenanceChecked"

(* Suspicious memory is quarantined instead of merged. *)
QuarantineMemory ==
/\ systemState = "MemoryProvenanceChecked"
/\ systemState' = "MemoryQuarantined"

(* Quarantined memory is rejected from the workflow. *)
RejectQuarantinedMemory ==
/\ systemState = "MemoryQuarantined"
/\ systemState' = "DenialAuditPending"

(* Trusted memory is merged into the working set. *)
MergeMemory ==
/\ systemState = "MemoryProvenanceChecked"
/\ systemState' = "MemoryMerged"

(* The planner drafts a candidate execution plan. *)
DraftPlan ==
/\ systemState = "MemoryMerged"
/\ systemState' = "PlanDrafted"

(* The drafted plan is validated against typed policy rules. *)
ValidatePlan ==
/\ systemState = "PlanDrafted"
/\ systemState' = "PlanValidated"

(* Validated work is reduced to explicit capabilities. *)
ScopeCapabilities ==
/\ systemState = "PlanValidated"
/\ systemState' = "CapabilityScoped"

(* Delegable capabilities are attenuated before execution. *)
AttenuateCapabilities ==
/\ systemState = "CapabilityScoped"
/\ systemState' = "CapabilityAttenuated"

(* Time, token, spend, and recursion budget are reserved. *)
ReserveBudget ==
/\ systemState = "CapabilityAttenuated"
/\ systemState' = "BudgetReserved"

(* Budget exhaustion is surfaced as an explicit failure condition. *)
ExhaustBudget ==
/\ systemState \in {"BudgetReserved", "ActionPlanAdmitted", "ToolCallIssued", "ToolResultReceived", "ToolResultSanitized", "ToolResultClassified", "ToolResultLowRisk", "ToolResultEndorsed", "ActionStepCompleted", "ChildSpawnRequested", "ChildCapabilityScoped", "ChildBudgetBound", "ChildIdentityBound", "ChildLineageVerified", "ChildApprovalPending", "ChildLineageAuditPending", "ChildLineageAuditCommitted", "ChildRegistered", "SecretCapturePending", "SecretPersisting", "SecretBound", "SecretRotationPending", "SecretRevocationPending", "PrivilegeUsageAudited", "ResultSanitized", "ResultValidated", "MemoryWriteReviewPending", "MemoryWritePending"}
/\ systemState' = "BudgetExhausted"

(* Exhausted budget routes into the failure pipeline. *)
RouteBudgetExhaustion ==
/\ systemState = "BudgetExhausted"
/\ systemState' = "FailureAuditPending"

(* The reserved plan is scored for execution risk. *)
ScoreRisk ==
/\ systemState = "BudgetReserved"
/\ systemState' = "RiskScored"

(* The scored plan enters the approval gate. *)
RequestApproval ==
/\ systemState = "RiskScored"
/\ systemState' = "ApprovalPending"

(* High-risk work escalates to a second operator. *)
RequestDualControl ==
/\ systemState = "ApprovalPending"
/\ systemState' = "DualControlPending"

(* Approved work is admitted for execution. *)
AuthorizeExecution ==
/\ systemState \in {"ApprovalPending", "DualControlPending"}
/\ systemState' = "ExecutionAuthorized"

(* Timed-out approvals are rejected instead of lingering. *)
ExpireApproval ==
/\ systemState \in {"ApprovalPending", "DualControlPending", "ChildApprovalPending", "MemoryWriteReviewPending"}
/\ systemState' = "ApprovalExpired"

(* Expired approvals route into the denial pipeline. *)
RouteApprovalExpiry ==
/\ systemState = "ApprovalExpired"
/\ systemState' = "DenialAuditPending"

(* The execution image is measured before sandboxing. *)
VerifyExecutionImage ==
/\ systemState = "ExecutionAuthorized"
/\ systemState' = "ImageVerified"

(* Remote attestation establishes trust in the execution boundary. *)
AttestSandbox ==
/\ systemState = "ImageVerified"
/\ systemState' = "SandboxAttested"

(* The isolated execution environment is provisioned. *)
ProvisionSandbox ==
/\ systemState = "SandboxAttested"
/\ systemState' = "SandboxProvisioned"

(* Typed tool channels are bound into the sandbox session. *)
BindToolSession ==
/\ systemState = "SandboxProvisioned"
/\ systemState' = "ToolSessionBound"

(* Every side effect is bound to a signed acting identity. *)
BindActionIdentity ==
/\ systemState = "ToolSessionBound"
/\ systemState' = "ActionIdentityBound"

(* Filesystem scope is fixed before any execution. *)
ScopeFilesystem ==
/\ systemState = "ActionIdentityBound"
/\ systemState' = "FilesystemScoped"

(* Network scope is fixed before any egress occurs. *)
ScopeNetwork ==
/\ systemState = "FilesystemScoped"
/\ systemState' = "NetworkScoped"

(* Egress rules are bound to destinations and protocols. *)
BindEgressPolicy ==
/\ systemState = "NetworkScoped"
/\ systemState' = "EgressPolicyBound"

(* Browser policy is bound before browsing can occur. *)
BindBrowserPolicy ==
/\ systemState = "EgressPolicyBound"
/\ systemState' = "BrowserPolicyBound"

(* Shell policy is bound before commands can run. *)
BindShellPolicy ==
/\ systemState = "BrowserPolicyBound"
/\ systemState' = "ShellPolicyBound"

(* Secret handles are attached without revealing secret bytes. *)
BindSecretReferences ==
/\ systemState = "ShellPolicyBound"
/\ systemState' = "SecretReferencesBound"

(* The fully mediated action plan is admitted into execution. *)
AdmitActionPlan ==
/\ systemState = "SecretReferencesBound"
/\ systemState' = "ActionPlanAdmitted"

(* A single typed tool call is issued under current authority. *)
IssueToolCall ==
/\ systemState \in {"ActionPlanAdmitted", "ActionStepCompleted"}
/\ systemState' = "ToolCallIssued"

(* The raw tool result enters the control plane. *)
ReceiveToolResult ==
/\ systemState = "ToolCallIssued"
/\ systemState' = "ToolResultReceived"

(* Fresh credentials are intercepted before they can reach the model stream. *)
DetectGeneratedSecret ==
/\ systemState = "ToolResultReceived"
/\ systemState' = "SecretCapturePending"

(* Generated secret bytes are written directly to the secret plane. *)
PersistGeneratedSecret ==
/\ systemState = "SecretCapturePending"
/\ systemState' = "SecretPersisting"

(* The persisted secret is rebound as an opaque capability. *)
BindSecretCapability ==
/\ systemState = "SecretPersisting"
/\ systemState' = "SecretBound"

(* Sanitized metadata resumes the tool-result pipeline after secret binding. *)
ReturnAfterSecretBind ==
/\ systemState = "SecretBound"
/\ systemState' = "ToolResultSanitized"

(* Raw tool output is sanitized before model visibility. *)
SanitizeToolResult ==
/\ systemState = "ToolResultReceived"
/\ systemState' = "ToolResultSanitized"

(* Sanitized tool output is classified for trust and provenance. *)
ClassifyToolResult ==
/\ systemState = "ToolResultSanitized"
/\ systemState' = "ToolResultClassified"

(* Low-risk tool output can proceed without endorsement. *)
AcceptLowRiskToolResult ==
/\ systemState = "ToolResultClassified"
/\ systemState' = "ToolResultLowRisk"

(* Unsafe or malformed tool output is rejected. *)
RejectToolResult ==
/\ systemState = "ToolResultClassified"
/\ systemState' = "ToolResultRejected"

(* Rejected tool output routes into the failure pipeline. *)
EscalateRejectedToolResult ==
/\ systemState = "ToolResultRejected"
/\ systemState' = "FailureAuditPending"

(* High-impact tool output is explicitly endorsed before use. *)
EndorseToolResult ==
/\ systemState = "ToolResultClassified"
/\ systemState' = "ToolResultEndorsed"

(* Failed endorsement blocks the workflow. *)
DenyToolEndorsement ==
/\ systemState = "ToolResultClassified"
/\ systemState' = "EndorsementDenied"

(* Endorsement denial routes into the denial pipeline. *)
RouteEndorsementDenial ==
/\ systemState = "EndorsementDenied"
/\ systemState' = "DenialAuditPending"

(* Low-risk tool output completes one execution step. *)
CompleteLowRiskActionStep ==
/\ systemState = "ToolResultLowRisk"
/\ systemState' = "ActionStepCompleted"

(* Endorsed tool output completes one execution step. *)
CompleteEndorsedActionStep ==
/\ systemState = "ToolResultEndorsed"
/\ systemState' = "ActionStepCompleted"

(* The current step requests a child agent for delegated work. *)
RequestChildSpawn ==
/\ systemState = "ActionStepCompleted"
/\ systemState' = "ChildSpawnRequested"

(* Child work is reduced to a scoped capability set. *)
ScopeChildCapabilities ==
/\ systemState = "ChildSpawnRequested"
/\ systemState' = "ChildCapabilityScoped"

(* The child's budget is explicitly bounded by the parent. *)
BindChildBudget ==
/\ systemState = "ChildCapabilityScoped"
/\ systemState' = "ChildBudgetBound"

(* The child receives its own signed acting identity. *)
BindChildIdentity ==
/\ systemState = "ChildBudgetBound"
/\ systemState' = "ChildIdentityBound"

(* Child lineage and provenance are cryptographically checked. *)
VerifyChildLineage ==
/\ systemState = "ChildIdentityBound"
/\ systemState' = "ChildLineageVerified"

(* Child creation enters its approval gate. *)
QueueChildApproval ==
/\ systemState = "ChildLineageVerified"
/\ systemState' = "ChildApprovalPending"

(* Approved child creation is queued for tamper-evident lineage audit. *)
ApproveChildSpawn ==
/\ systemState = "ChildApprovalPending"
/\ systemState' = "ChildLineageAuditPending"

(* Child lineage is committed before registration completes. *)
CommitChildLineageAudit ==
/\ systemState = "ChildLineageAuditPending"
/\ systemState' = "ChildLineageAuditCommitted"

(* The approved child is registered into the live system. *)
RegisterChild ==
/\ systemState = "ChildLineageAuditCommitted"
/\ systemState' = "ChildRegistered"

(* Control returns to the parent after child registration. *)
ReturnFromChildRegistration ==
/\ systemState = "ChildRegistered"
/\ systemState' = "ActionStepCompleted"

(* A completed step may initiate credential rotation. *)
StartSecretRotation ==
/\ systemState = "ActionStepCompleted"
/\ systemState' = "SecretRotationPending"

(* Successful rotation returns control to the execution loop. *)
FinishSecretRotation ==
/\ systemState = "SecretRotationPending"
/\ systemState' = "ActionStepCompleted"

(* A completed step may initiate credential revocation. *)
StartSecretRevocation ==
/\ systemState = "ActionStepCompleted"
/\ systemState' = "SecretRevocationPending"

(* Successful revocation returns control to the execution loop. *)
FinishSecretRevocation ==
/\ systemState = "SecretRevocationPending"
/\ systemState' = "ActionStepCompleted"

(* Observed privilege usage is audited against declared scope. *)
AuditPrivilegeUsage ==
/\ systemState = "ActionStepCompleted"
/\ systemState' = "PrivilegeUsageAudited"

(* Over-privileged behavior is surfaced as a security violation. *)
DetectPrivilegeViolation ==
/\ systemState = "PrivilegeUsageAudited"
/\ systemState' = "PrivilegeViolationDetected"

(* Privilege violations escalate into isolation handling. *)
EscalatePrivilegeViolation ==
/\ systemState = "PrivilegeViolationDetected"
/\ systemState' = "IsolationBreachDetected"

(* Normal privilege audit flows into result sanitization. *)
SanitizeResult ==
/\ systemState = "PrivilegeUsageAudited"
/\ systemState' = "ResultSanitized"

(* Sanitized results are validated against expected effects. *)
ValidateResult ==
/\ systemState = "ResultSanitized"
/\ systemState' = "ResultValidated"

(* Durable writes enter a review gate before persistence. *)
QueueMemoryWriteReview ==
/\ systemState = "ResultValidated"
/\ systemState' = "MemoryWriteReviewPending"

(* Rejected writes are denied instead of persisted. *)
RejectMemoryWrite ==
/\ systemState = "MemoryWriteReviewPending"
/\ systemState' = "MemoryWriteRejected"

(* Write rejection routes into the denial pipeline. *)
RouteMemoryWriteRejection ==
/\ systemState = "MemoryWriteRejected"
/\ systemState' = "DenialAuditPending"

(* Approved writes are queued for durable persistence. *)
ApproveMemoryWrite ==
/\ systemState = "MemoryWriteReviewPending"
/\ systemState' = "MemoryWritePending"

(* Successful writes enter the success audit pipeline. *)
QueueSuccessAudit ==
/\ systemState = "MemoryWritePending"
/\ systemState' = "SuccessAuditPending"

(* Success evidence is committed before cleanup runs. *)
CommitSuccessAudit ==
/\ systemState = "SuccessAuditPending"
/\ systemState' = "SuccessAuditCommitted"

(* Success cleanup releases resources after audit commitment. *)
QueueSuccessCleanup ==
/\ systemState = "SuccessAuditCommitted"
/\ systemState' = "SuccessCleanupPending"

(* The workflow terminates successfully after cleanup. *)
CompleteWorkflow ==
/\ systemState = "SuccessCleanupPending"
/\ systemState' = "Completed"

(* Early identity or tenant failure is denied before planning. *)
RejectIdentityOrTenant ==
/\ systemState \in {"IntentSubmitted", "IdentityVerified", "OperatorBound", "OperatorMfaVerified", "TenantResolved", "TenantIsolationBound"}
/\ systemState' = "DenialAuditPending"

(* Policy or approval rejection before execution enters denial handling. *)
DenyRequest ==
/\ systemState \in {"RequestNormalized", "ProvenanceTagged", "PolicySignatureVerified", "PolicyVersionChecked", "InputClassified", "MemoryReadPending", "MemoryProvenanceChecked", "MemoryMerged", "PlanDrafted", "PlanValidated", "CapabilityScoped", "CapabilityAttenuated", "BudgetReserved", "RiskScored", "ApprovalPending", "DualControlPending", "ChildApprovalPending", "MemoryWriteReviewPending"}
/\ systemState' = "DenialAuditPending"

(* Denial evidence is committed before denied cleanup runs. *)
CommitDenialAudit ==
/\ systemState = "DenialAuditPending"
/\ systemState' = "DenialAuditCommitted"

(* Denied workflows release resources after denial audit. *)
QueueDeniedCleanup ==
/\ systemState = "DenialAuditCommitted"
/\ systemState' = "DenialCleanupPending"

(* The workflow terminates denied after cleanup. *)
FinishDenied ==
/\ systemState = "DenialCleanupPending"
/\ systemState' = "Denied"

(* Runtime or control-plane faults enter the failure pipeline. *)
RecordFailure ==
/\ systemState \in {"RequestNormalized", "ProvenanceTagged", "PolicySignatureVerified", "PolicyVersionChecked", "InputClassified", "MemoryReadPending", "MemoryProvenanceChecked", "MemoryMerged", "PlanDrafted", "PlanValidated", "CapabilityScoped", "CapabilityAttenuated", "BudgetReserved", "RiskScored", "ApprovalPending", "DualControlPending", "ExecutionAuthorized", "ImageVerified", "SandboxAttested", "SandboxProvisioned", "ToolSessionBound", "ActionIdentityBound", "FilesystemScoped", "NetworkScoped", "EgressPolicyBound", "BrowserPolicyBound", "ShellPolicyBound", "SecretReferencesBound", "ActionPlanAdmitted", "ToolCallIssued", "ToolResultReceived", "ToolResultSanitized", "ToolResultClassified", "ToolResultLowRisk", "ToolResultEndorsed", "ActionStepCompleted", "ChildSpawnRequested", "ChildCapabilityScoped", "ChildBudgetBound", "ChildIdentityBound", "ChildLineageVerified", "ChildApprovalPending", "ChildLineageAuditPending", "ChildLineageAuditCommitted", "ChildRegistered", "SecretCapturePending", "SecretPersisting", "SecretBound", "SecretRotationPending", "SecretRevocationPending", "PrivilegeUsageAudited", "ResultSanitized", "ResultValidated", "MemoryWriteReviewPending", "MemoryWritePending", "Suspended"}
/\ systemState' = "FailureAuditPending"

(* Failure evidence is committed before failed cleanup runs. *)
CommitFailureAudit ==
/\ systemState = "FailureAuditPending"
/\ systemState' = "FailureAuditCommitted"

(* Failed workflows release resources after failure audit. *)
QueueFailureCleanup ==
/\ systemState = "FailureAuditCommitted"
/\ systemState' = "FailureCleanupPending"

(* The workflow terminates failed after cleanup. *)
FinishFailed ==
/\ systemState = "FailureCleanupPending"
/\ systemState' = "Failed"

(* Policy drift is detected during long-running execution. *)
DetectPolicyVersionMismatch ==
/\ systemState \in {"PolicyVersionChecked", "InputClassified", "MemoryReadPending", "MemoryProvenanceChecked", "MemoryMerged", "PlanDrafted", "PlanValidated", "CapabilityScoped", "CapabilityAttenuated", "BudgetReserved", "RiskScored", "ApprovalPending", "DualControlPending", "ExecutionAuthorized", "ImageVerified", "SandboxAttested", "SandboxProvisioned", "ToolSessionBound", "ActionIdentityBound", "FilesystemScoped", "NetworkScoped", "EgressPolicyBound", "BrowserPolicyBound", "ShellPolicyBound", "SecretReferencesBound", "ActionPlanAdmitted", "ToolCallIssued", "ToolResultReceived", "ToolResultSanitized", "ToolResultClassified", "ToolResultLowRisk", "ToolResultEndorsed", "ActionStepCompleted", "ChildSpawnRequested", "ChildCapabilityScoped", "ChildBudgetBound", "ChildIdentityBound", "ChildLineageVerified", "ChildApprovalPending", "ChildLineageAuditPending", "ChildLineageAuditCommitted", "ChildRegistered", "SecretCapturePending", "SecretPersisting", "SecretBound", "SecretRotationPending", "SecretRevocationPending", "PrivilegeUsageAudited", "ResultSanitized", "ResultValidated", "MemoryWriteReviewPending", "MemoryWritePending", "Suspended"}
/\ systemState' = "PolicyVersionMismatch"

(* Policy drift invalidates the active workflow and triggers revocation. *)
HandlePolicyVersionMismatch ==
/\ systemState = "PolicyVersionMismatch"
/\ systemState' = "RevocationAuditPending"

(* Isolation failure can be detected from the execution boundary or runtime. *)
DetectIsolationBreach ==
/\ systemState \in {"ImageVerified", "SandboxAttested", "SandboxProvisioned", "ToolSessionBound", "ActionIdentityBound", "FilesystemScoped", "NetworkScoped", "EgressPolicyBound", "BrowserPolicyBound", "ShellPolicyBound", "SecretReferencesBound", "ActionPlanAdmitted", "ToolCallIssued", "ToolResultReceived", "ToolResultSanitized", "ToolResultClassified", "ToolResultLowRisk", "ToolResultEndorsed", "ActionStepCompleted", "ChildSpawnRequested", "ChildCapabilityScoped", "ChildBudgetBound", "ChildIdentityBound", "ChildLineageVerified", "ChildLineageAuditPending", "ChildLineageAuditCommitted", "ChildRegistered", "SecretCapturePending", "SecretPersisting", "SecretBound", "SecretRotationPending", "SecretRevocationPending", "PrivilegeUsageAudited", "ResultSanitized", "ResultValidated", "MemoryWritePending"}
/\ systemState' = "IsolationBreachDetected"

(* Containment engages before revocation cleanup begins. *)
EngageContainment ==
/\ systemState = "IsolationBreachDetected"
/\ systemState' = "ContainmentEngaged"

(* Containment escalates into the revocation audit pipeline. *)
EscalateContainmentToRevocation ==
/\ systemState = "ContainmentEngaged"
/\ systemState' = "RevocationAuditPending"

(* Suspicious execution can be paused for review. *)
SuspendWorkflow ==
/\ systemState \in {"InputClassified", "MemoryReadPending", "MemoryProvenanceChecked", "MemoryMerged", "PlanDrafted", "PlanValidated", "CapabilityScoped", "CapabilityAttenuated", "BudgetReserved", "RiskScored", "ApprovalPending", "DualControlPending", "ExecutionAuthorized", "ImageVerified", "SandboxAttested", "SandboxProvisioned", "ToolSessionBound", "ActionIdentityBound", "FilesystemScoped", "NetworkScoped", "EgressPolicyBound", "BrowserPolicyBound", "ShellPolicyBound", "SecretReferencesBound", "ActionPlanAdmitted", "ToolCallIssued", "ToolResultReceived", "ToolResultSanitized", "ToolResultClassified", "ToolResultLowRisk", "ToolResultEndorsed", "ActionStepCompleted", "ChildSpawnRequested", "ChildCapabilityScoped", "ChildBudgetBound", "ChildIdentityBound", "ChildLineageVerified", "ChildApprovalPending", "ChildLineageAuditPending", "ChildLineageAuditCommitted", "ChildRegistered", "SecretCapturePending", "SecretPersisting", "SecretBound", "SecretRotationPending", "SecretRevocationPending", "PrivilegeUsageAudited", "ResultSanitized", "ResultValidated", "MemoryWriteReviewPending", "MemoryWritePending"}
/\ systemState' = "Suspended"

(* Suspended execution returns to approval before resuming. *)
ResumeAfterSuspension ==
/\ systemState = "Suspended"
/\ systemState' = "ApprovalPending"

(* Long suspensions expire into revocation handling. *)
ExpireSuspension ==
/\ systemState = "Suspended"
/\ systemState' = "SuspensionExpired"

(* Expired suspension routes into the revocation pipeline. *)
RouteSuspensionExpiry ==
/\ systemState = "SuspensionExpired"
/\ systemState' = "RevocationAuditPending"

(* Operator revocation can halt active or suspended workflows. *)
RevokeWorkflow ==
/\ systemState \in {"CapabilityScoped", "CapabilityAttenuated", "BudgetReserved", "RiskScored", "ApprovalPending", "ApprovalExpired", "DualControlPending", "ExecutionAuthorized", "ImageVerified", "SandboxAttested", "SandboxProvisioned", "ToolSessionBound", "ActionIdentityBound", "FilesystemScoped", "NetworkScoped", "EgressPolicyBound", "BrowserPolicyBound", "ShellPolicyBound", "SecretReferencesBound", "ActionPlanAdmitted", "ToolCallIssued", "ToolResultReceived", "ToolResultSanitized", "ToolResultClassified", "ToolResultLowRisk", "ToolResultEndorsed", "ActionStepCompleted", "ChildSpawnRequested", "ChildCapabilityScoped", "ChildBudgetBound", "ChildIdentityBound", "ChildLineageVerified", "ChildApprovalPending", "ChildLineageAuditPending", "ChildLineageAuditCommitted", "ChildRegistered", "SecretCapturePending", "SecretPersisting", "SecretBound", "SecretRotationPending", "SecretRevocationPending", "PrivilegeUsageAudited", "ResultSanitized", "ResultValidated", "MemoryWriteReviewPending", "MemoryWritePending", "Suspended"}
/\ systemState' = "RevocationAuditPending"

(* Revocation evidence is committed before descendant cleanup begins. *)
CommitRevocationAudit ==
/\ systemState = "RevocationAuditPending"
/\ systemState' = "RevocationAuditCommitted"

(* Revocation propagates through any registered child lineage. *)
PropagateChildRevocation ==
/\ systemState = "RevocationAuditCommitted"
/\ systemState' = "ChildRevocationPropagating"

(* Revocation cleanup runs after child propagation completes. *)
QueueRevocationCleanup ==
/\ systemState = "ChildRevocationPropagating"
/\ systemState' = "RevocationCleanupPending"

(* The workflow terminates revoked after cleanup. *)
FinishRevoked ==
/\ systemState = "RevocationCleanupPending"
/\ systemState' = "Revoked"

Init == systemState = "IntentSubmitted"

Next ==
/ VerifyIdentity
/ BindOperator
/ VerifyOperatorMfa
/ ResolveTenant
/ BindTenantIsolation
/ NormalizeRequest
/ TagProvenance
/ VerifyPolicySignature
/ CheckPolicyVersion
/ ClassifyInput
/ QuarantineInput
/ RejectQuarantinedInput
/ QueueMemoryRead
/ CheckMemoryProvenance
/ QuarantineMemory
/ RejectQuarantinedMemory
/ MergeMemory
/ DraftPlan
/ ValidatePlan
/ ScopeCapabilities
/ AttenuateCapabilities
/ ReserveBudget
/ ExhaustBudget
/ RouteBudgetExhaustion
/ ScoreRisk
/ RequestApproval
/ RequestDualControl
/ AuthorizeExecution
/ ExpireApproval
/ RouteApprovalExpiry
/ VerifyExecutionImage
/ AttestSandbox
/ ProvisionSandbox
/ BindToolSession
/ BindActionIdentity
/ ScopeFilesystem
/ ScopeNetwork
/ BindEgressPolicy
/ BindBrowserPolicy
/ BindShellPolicy
/ BindSecretReferences
/ AdmitActionPlan
/ IssueToolCall
/ ReceiveToolResult
/ DetectGeneratedSecret
/ PersistGeneratedSecret
/ BindSecretCapability
/ ReturnAfterSecretBind
/ SanitizeToolResult
/ ClassifyToolResult
/ AcceptLowRiskToolResult
/ RejectToolResult
/ EscalateRejectedToolResult
/ EndorseToolResult
/ DenyToolEndorsement
/ RouteEndorsementDenial
/ CompleteLowRiskActionStep
/ CompleteEndorsedActionStep
/ RequestChildSpawn
/ ScopeChildCapabilities
/ BindChildBudget
/ BindChildIdentity
/ VerifyChildLineage
/ QueueChildApproval
/ ApproveChildSpawn
/ CommitChildLineageAudit
/ RegisterChild
/ ReturnFromChildRegistration
/ StartSecretRotation
/ FinishSecretRotation
/ StartSecretRevocation
/ FinishSecretRevocation
/ AuditPrivilegeUsage
/ DetectPrivilegeViolation
/ EscalatePrivilegeViolation
/ SanitizeResult
/ ValidateResult
/ QueueMemoryWriteReview
/ RejectMemoryWrite
/ RouteMemoryWriteRejection
/ ApproveMemoryWrite
/ QueueSuccessAudit
/ CommitSuccessAudit
/ QueueSuccessCleanup
/ CompleteWorkflow
/ RejectIdentityOrTenant
/ DenyRequest
/ CommitDenialAudit
/ QueueDeniedCleanup
/ FinishDenied
/ RecordFailure
/ CommitFailureAudit
/ QueueFailureCleanup
/ FinishFailed
/ DetectPolicyVersionMismatch
/ HandlePolicyVersionMismatch
/ DetectIsolationBreach
/ EngageContainment
/ EscalateContainmentToRevocation
/ SuspendWorkflow
/ ResumeAfterSuspension
/ ExpireSuspension
/ RouteSuspensionExpiry
/ RevokeWorkflow
/ CommitRevocationAudit
/ PropagateChildRevocation
/ QueueRevocationCleanup
/ FinishRevoked

====