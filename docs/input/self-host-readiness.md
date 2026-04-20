# Self-Host Readiness Audit

## Scope

This audit focuses on Open Pincery's readiness for:

- `self_host_individual`
- `self_host_team`

The `enterprise_self_hosted` mode is related but stricter. Enterprise-only requirements such as SCIM, BYOK, SIEM export, and procurement artifacts are covered separately in the enterprise readiness audit.

## Current State

Open Pincery is explicitly designed to support self-hosted deployment, but it is not yet self-host ready in the operational sense.

What is already strong in the current spec:

- Self-host is a first-class deployment target, not an afterthought.
- The TLA explicitly requires that the open-source runtime function without depending on a proprietary hosted control plane.
- A bootstrap path exists for self-hosted installs via `local_admin`.
- Individual installs and team installs are both modeled through organizations, workspaces, memberships, RBAC, policies, approvals, quotas, and audit surfaces.
- Billing enforcement is optional in self-hosted modes.
- The base implementation direction is already narrowed to a Rust service plus PostgreSQL, with self-host defaults in preferences for reverse proxy, monitoring, and secrets handling.
- The security model already assumes self-host-compatible building blocks: Zerobox, a vault/proxy credential pattern, Postgres, and Greywall hardening.

What is still true of the repository as a whole:

- There is no buildable runtime yet.
- There is no installer, package, container image, Compose file, Helm chart, systemd unit, or bootstrap CLI.
- There is no migration set, no first-run admin creation path, and no operator documentation.
- This is a design-level readiness audit, not a validated operator runbook.

## Already Covered In The TLA Spec

- Supported deployment modes are explicit: `self_host_individual`, `self_host_team`, `saas_managed`, and `enterprise_self_hosted`.
- Self-hosted deployments do not require an external IdP to get started.
- First boot must support a bootstrap-admin flow using `auth_provider = 'local_admin'`.
- The bootstrap flow creates the first user plus a default organization and workspace.
- Self-hosted deployments may later enable GitHub OAuth, Entra OIDC, or generic OIDC and rotate away from `local_admin` for normal use.
- Workspace, approval, credential, audit, quota, and policy features remain available in self-hosted modes.
- Billing and subscription enforcement may be absent or inert in self-hosted modes.
- Every agent still belongs to exactly one workspace and exactly one accountable human owner.
- Organization, workspace, membership, RBAC, policy-set, and quota tables are already specified.
- Administrative suspension is modeled through control-plane flags rather than adding extra lifecycle states.

## Defaults Already Chosen For Self-Host Implementation

These are not yet delivered as runnable artifacts, but the implementation guidance is already opinionated enough to support a self-host packaging story:

- Runtime: Rust
- Database: PostgreSQL
- HTTP/API: axum
- Reverse proxy default: Caddy
- Background work: same binary or dedicated worker process from the same codebase
- Monitoring default: Prometheus + Grafana + Loki
- Secrets for infrastructure bootstrap: environment variables or SOPS/Vault
- Agent-use credentials: vault/proxy injection model, with OneCLI or an equivalent implementation

That is a good self-host baseline because it avoids mandatory dependency on a commercial vendor service. It does not yet amount to a supported installation experience.

## Self-Host Gaps Still Missing

### 1. Installation And Bootstrap Experience

The TLA specifies what bootstrap must achieve, but not how an operator actually performs it.

Still missing:

- a supported installation artifact strategy
- a first-run bootstrap command or setup wizard
- generation, delivery, rotation, and expiry rules for the install-time bootstrap token
- environment/config schema for the minimum required services
- migration-and-seed flow for first startup
- a documented "day zero" path from empty database to first working workspace

### 2. Packaging And Deployment Topology

Self-host readiness requires a concrete answer to "what do I run?"

Still missing:

- supported packaging formats such as OCI image, Docker Compose, or system package
- recommended single-node topology for `self_host_individual`
- recommended split control-plane/worker topology for `self_host_team`
- health checks and readiness checks
- persistent volume and filesystem layout guidance
- reverse-proxy and TLS examples aligned with the Caddy default
- optional high-availability guidance for serious team installs

### 3. Local Auth And Operator Access Lifecycle

The spec correctly models `local_admin` as bootstrap and break-glass access, but the operational lifecycle is under-defined.

Still missing:

- how the initial bootstrap token becomes a durable authenticated session
- whether `local_admin` uses password, token exchange, passkey, or another local auth mechanism
- rotation and disablement rules for `local_admin`
- recovery flow if the only local admin is locked out
- MFA or step-up auth policy for sensitive local administrative actions
- role-change audit expectations for self-host operators

### 4. Self-Host Secrets, TLS, And Credential Operations

The security architecture is strong at the model level, but operators still need an installation story.

Still missing:

- the default deployment shape for the required vault/proxy credential component
- whether a lightweight embedded dev mode exists or whether operators must deploy a separate vault/proxy service from day one
- bootstrap secret storage and rotation guidance
- certificate and TLS renewal guidance for self-hosted control-plane endpoints
- filesystem permission and backup-encryption guidance for local installs
- clear separation between infrastructure secrets and agent-use credentials in installation docs

### 5. Upgrade, Backup, And Restore Discipline

Self-host users need to know how to keep a running installation alive across releases.

Still missing:

- supported upgrade path between releases
- migration compatibility and rollback rules
- backup procedure before upgrade
- restore procedure and validation checklist
- data retention defaults for local installs
- versioning policy for breaking config or schema changes
- operator-facing release notes discipline

### 6. Operations, Monitoring, And Troubleshooting

Preferences picks sensible defaults, but self-host readiness needs concrete operational guidance.

Still missing:

- minimum hardware sizing for personal and team deployments
- baseline alert set for Postgres, worker health, queue depth, and cost anomalies
- log locations, rotation, and retention guidance
- operator troubleshooting guide
- incident response guidance for a stuck wake, failed migration, broken bootstrap, or exhausted disk
- a real runbook for routine maintenance and common failure modes

### 7. Restricted-Network And Air-Gapped Operation

Self-hosted users, especially serious teams, will eventually ask whether the platform can run with minimal or no public internet dependency.

Still missing:

- supported model-provider story for restricted-network installs
- guidance for operating with local or self-managed model gateways
- offline or mirrored dependency strategy for upgrades
- outbound-domain policy defaults for hardened environments
- explicit statement of which features require internet access and which do not

## Highest-Priority Self-Host Additions To The Spec

If self-host is a serious adoption target, the next additions should be:

1. A concrete installation and bootstrap contract from empty system to first `local_admin` session.
2. Supported deployment topologies for `self_host_individual` and `self_host_team`.
3. A clear auth lifecycle for `local_admin`, including rotation, recovery, and handoff to external IdPs.
4. A packaging and credential-proxy deployment story that does not assume hosted infrastructure.
5. Upgrade, backup, restore, and rollback requirements.
6. Operator runbook and troubleshooting requirements for routine self-host maintenance.

## MIT License Implications

MIT is well aligned with self-host adoption, but it does not make the system self-host ready by itself.

What MIT helps with:

- frictionless personal and team evaluation
- simple legal posture for forking and internal deployment
- broad compatibility with community packaging and downstream extensions

What MIT does not solve:

- packaging quality
- upgrade safety
- operator documentation
- support expectations
- trademark and distribution clarity

What MIT changes strategically:

- anyone can package and redistribute the runtime
- your default self-host distribution must win on clarity, safety, and operability rather than licensing restriction
- you will likely still want a trademark and naming policy even if the code is MIT-licensed

## Bottom Line

Open Pincery is architecturally compatible with self-hosting and has already made several correct design choices for open adoption:

- no mandatory dependence on a proprietary hosted control plane
- explicit local bootstrap via `local_admin`
- workspace and policy model that works outside SaaS
- billing optional in self-hosted modes
- simple Rust + Postgres baseline rather than a sprawling cloud dependency graph

But it is not yet self-host ready because the operator experience does not exist as running software and documentation yet.

The remaining blockers are execution blockers:

- installation and bootstrap tooling
- packaging and deployment topology
- local auth lifecycle and recovery model
- secrets/TLS/vault deployment guidance
- upgrade/backup/restore discipline
- operator runbooks and troubleshooting
