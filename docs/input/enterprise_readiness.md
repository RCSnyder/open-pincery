# Enterprise Readiness Audit

## Current State

Open Pincery is strong on agent-runtime correctness and governance design, but it is not yet enterprise ready.

What is already strong in the current spec:

- Durable agent lifecycle with CAS wake ownership and append-only auditability.
- Human ownership and accountability for every agent action.
- Explicit authentication model with Entra OIDC and generic OIDC allowed for enterprise deployments.
- Explicit self-host bootstrap path via local_admin.
- Organization, workspace, tenant, RBAC, policy-set, quota, and commercial control-plane requirements are now specified in the TLA.
- Approval gates for sensitive actions.
- Credential isolation, sandboxing, budget limits, and audit logging.
- Append-only retention plus GDPR-aware redaction / crypto-shredding notes.

What is still true of the repository as a whole:

- The repo is still architecture-first. There is no buildable runtime, control plane, migrations directory, or operational stack yet.
- The enterprise readiness file was previously only a placeholder. This audit is design-level, not deployment-certification.

## Already Covered In The TLA Spec

- Enterprise auth is acknowledged: Entra OIDC and generic OIDC are supported auth_provider values.
- Self-host bootstrap is acknowledged: local_admin is a defined provider for install-time bootstrap and break-glass access.
- Authenticated human ownership is required before agent creation or operation.
- Organizations, workspaces, memberships, and tenant boundaries are now specified.
- Durable RBAC roles and separation-of-duties requirements are now specified.
- Org/workspace policy sets and usage quotas are now specified.
- Session and auth audit tables are specified.
- Role-based audit access is specified at the API level: accountable owner, scoped workspace/org admins, and auditor/security roles.
- Audit endpoints for events, LLM calls, tool calls, messages, credentials, and cost are specified.
- Retention policy, append-only privacy-redaction semantics, and audit export concepts are present.

## Enterprise Gaps Still Missing

### 1. Enterprise Identity Lifecycle

OIDC is present conceptually, but enterprise identity lifecycle is under-modeled.

Still missing:

- SCIM provisioning and deprovisioning.
- JIT provisioning rules.
- Group-to-role mapping from IdP claims.
- Domain verification.
- SAML support decision, if enterprise customers demand it.
- Session lifetime, re-authentication, and step-up auth policy for sensitive actions.
- Audit rules for role grants, role revocations, and failed access attempts.

### 2. Enterprise Audit Export And SIEM Integration

The spec has audit APIs, but enterprises usually need machine-consumable export and streaming.

Still missing:

- Signed audit export jobs.
- Streaming to Splunk, Sentinel, Datadog, Chronicle, or S3.
- Export integrity checks and retention classes.
- Support for periodic evidence packages.
- Query boundaries by org, workspace, and user role.

### 3. Key Management And Crypto Posture

Secret isolation is strong, but enterprise buyers will ask deeper questions.

Still missing:

- KMS integration strategy.
- BYOK or customer-managed key option.
- Key rotation schedule and auditability.
- Backup key handling.
- Encryption boundary documentation for database, backups, and logs.

### 4. Operational Readiness

The TLA is good at behavior. Enterprises also need boring operational maturity.

Still missing:

- HA deployment model for multiple runtime instances.
- Background job ownership for stale wake recovery, dedup cleanup, and compaction.
- Disaster recovery target definitions.
- Backup and restore drills.
- Versioned upgrade path and zero-downtime migration process.
- SLOs, alert thresholds, and on-call expectations.
- Incident runbook and status communication model.

### 5. Procurement And Compliance Pack

The architecture mentions SOC 2, HIPAA, and GDPR, but enterprise readiness needs concrete artifacts.

Still missing:

- Security questionnaire answers.
- DPA template.
- Subprocessor inventory.
- Vulnerability disclosure policy.
- SBOM generation and signed release artifacts.
- Pen test cadence and summary process.
- Control mapping against SOC 2, GDPR, NIST AI RMF, and EU AI Act if targeted.

## Highest-Priority Enterprise Additions To The Spec

If enterprise is a serious target, the next spec additions should be:

1. SCIM and group-mapping lifecycle rules.
2. Tenant-aware audit export model.
3. Deployment and recovery model for multi-instance runtime operation.
4. Explicit KMS/BYOK and encryption-boundary requirements.
5. Evidence-export and SIEM-streaming requirements.
6. Operational SLO, backup, and restore requirements.

## MIT License Implications

MIT is compatible with enterprise distribution, but it does not make the system enterprise ready by itself.

What MIT helps with:

- Easy adoption.
- Low-friction procurement for open-source evaluation.
- Simple legal posture for source availability.

What MIT does not solve:

- Support commitments.
- Compliance evidence.
- SaaS trust.
- Enterprise procurement requirements.
- Operational maturity.

What MIT changes strategically:

- Anyone can self-host, fork, and commercialize the code.
- Your moat must come from execution: hosted product quality, integrations, brand, enterprise controls, support, and operations.
- If you choose MIT, add a LICENSE file, a contributor policy, and likely a trademark policy for the Open Pincery name/logo.

## Bottom Line

Open Pincery already looks like a serious enterprise-capable control-plane architecture at the runtime and audit layer.

It is not yet enterprise ready because the control-plane architecture is now specified, but still not implemented or operationalized.

The remaining blockers are execution blockers:

- enterprise identity lifecycle
- SIEM-grade audit export
- KMS/BYOK posture
- HA/DR operations
- procurement and compliance artifacts
- real implementation, tests, and deployment discipline
