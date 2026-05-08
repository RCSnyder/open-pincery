# Spec Coverage — AC ↔ Canonical TLA+ Action ↔ Invariant

This file is the single source of truth that links every v9 acceptance
criterion to the canonical TLA+ action(s) in
`docs/input/OpenPinceryCanonical.tla` (`Next ==` disjunction, line ~1949)
that implement it, plus any invariant the AC is meant to make real.

It is mechanically validated by `tests/spec_coverage_lint.rs`. Every
non-`—` token in the **Canonical Action(s)** column MUST appear verbatim
in the body of the canonical `Next` disjunction. Multiple actions are
pipe-separated. ACs that are pure documentation, UI, CLI, dev-experience,
or read-only API surface use `—` (no runtime canonical action) and are
exempt from the commit-msg trailer requirement because they do not modify
`src/runtime/**` or `src/api/**` policy code.

| AC      | Canonical Action(s)                                                                                                                              | Invariant                          |
| ------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------- |
| AC-53   | ProvisionSandbox \| AttestSandbox \| ScopeFilesystem \| ScopeNetwork \| BindShellPolicy                                                          | Inv_ToolCallRequiresBinding        |
| AC-54   | —                                                                                                                                                | —                                  |
| AC-55   | IssueToolCall \| TagProvenance                                                                                                                   | —                                  |
| AC-56   | BindSecretReferences \| TagProvenance                                                                                                            | —                                  |
| AC-57   | —                                                                                                                                                | —                                  |
| AC-58   | VerifyIdentity                                                                                                                                   | —                                  |
| AC-59   | VerifyIdentity \| BindOperator                                                                                                                   | —                                  |
| AC-60   | —                                                                                                                                                | —                                  |
| AC-61   | —                                                                                                                                                | —                                  |
| AC-62   | —                                                                                                                                                | —                                  |
| AC-63   | —                                                                                                                                                | —                                  |
| AC-64   | —                                                                                                                                                | —                                  |
| AC-65   | ResolveTenant \| BindTenantIsolation                                                                                                             | —                                  |
| AC-66   | IssueToolCall \| ScopeNetwork                                                                                                                    | —                                  |
| AC-67   | ReserveBudget                                                                                                                                    | —                                  |
| AC-68   | —                                                                                                                                                | —                                  |
| AC-69   | —                                                                                                                                                | —                                  |
| AC-70   | —                                                                                                                                                | —                                  |
| AC-71   | BindSecretReferences \| BindSecretCapability                                                                                                     | —                                  |
| AC-72   | ScopeNetwork \| BindEgressPolicy                                                                                                                 | —                                  |
| AC-73   | ProvisionSandbox \| AttestSandbox                                                                                                                | —                                  |
| AC-74   | BindSecretReferences                                                                                                                             | —                                  |
| AC-75   | —                                                                                                                                                | —                                  |
| AC-76   | ProvisionSandbox \| AttestSandbox                                                                                                                | Inv_ToolCallRequiresBinding        |
| AC-77   | ProvisionSandbox                                                                                                                                 | Inv_ToolCallRequiresBinding        |
| AC-78   | VerifyAuditChain \| AuditChainBroken \| CommitSuccessChain \| CommitDenialChain \| CommitFailureChain \| CommitRevocationChain                   | Inv_AuditChainBeforeExecution      |
| AC-79   | ClassifyInput \| QuarantineInput \| ScanModelResponse \| RouteRejectedResponse                                                                   | —                                  |
| AC-80   | AuthorizeExecution \| IssueToolCall                                                                                                              | Inv_ToolCallRequiresBinding        |
| AC-81   | —                                                                                                                                                | —                                  |
| AC-82   | AttemptWakeAcquire \| WakeAcquireSucceeds \| PromptAssemblyCompletes \| ToolDispatches \| AuthorizeExecution \| ReceiveToolResult \| ToolResultProcessedToolLoop \| MidWakePollFindsNothing \| WakeEndTransitionsToMaintenance \| TerminalEndsWake | Inv_TerminalSuccession             |
| AC-83   | ProvisionSandbox \| BindShellPolicy \| AttestSandbox                                                                                             | —                                  |
| AC-84   | AttestSandbox                                                                                                                                    | —                                  |
| AC-85   | AttestSandbox                                                                                                                                    | —                                  |
| AC-86   | ProvisionSandbox                                                                                                                                 | —                                  |
| AC-87   | BindShellPolicy                                                                                                                                  | —                                  |
| AC-88   | AuditPrivilegeUsage                                                                                                                              | —                                  |

## Notes

- ACs flagged `—` in **Canonical Action(s)** are documentation,
  developer-experience, UI, CLI, or read-only API surface work. They
  intentionally have no runtime canonical action and are exempt from the
  commit-msg `canonical_action=` trailer requirement enforced by
  `.github/hooks/commit-msg-spec-ref` (because they do not modify
  `src/runtime/**` or `src/api/**`).
- `AmendScope` is a process-only convention used in commit messages for
  scope/design/readiness edits. It is **not** a canonical TLA+ action and
  does not appear here. Scope edits do not touch `src/runtime/**` or
  `src/api/**` and so are unaffected by the hook.
- AC-78's invariant `Inv_AuditChainBeforeExecution` is the v9.0 target
  the `events.prev_hash` / `entry_hash` chain makes mechanically real;
  the action `VerifyAuditChain` in the spec is no longer cosmetic once
  that AC ships.
- AC-79 in scope.md cites `RouteModelResponse` informally; the canonical
  spec action is `RouteRejectedResponse` (the model-response routing
  action that exists in `Next`). The lint validates the canonical name.
- AC-82's lifecycle bindings cover the wake-loop CAS state machine end
  to end, with the canonical `Inv_TerminalSuccession` invariant pinning
  the rule that every break out of the wake loop transitions through
  `WakeEnding` exactly once before reaching `Maintenance`. The runtime
  enforcement lives in `src/runtime/wake_loop.rs` (single
  `enter_wake_ending` + `wake_end_transitions_to_maintenance` chain at
  the loop terminal) and is statically pinned by
  `tests/status_writes_lint_test.rs::assert_status_writes_are_cas_only`,
  which forbids any `UPDATE agents SET status` outside
  `src/models/agent.rs`.
