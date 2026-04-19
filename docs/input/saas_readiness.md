# SaaS Readiness Audit

## Current State

Open Pincery now explicitly includes GitHub sign-in in the TLA spec for SaaS deployments.

That is a meaningful step, but it is only one piece of SaaS readiness.

What is already strong in the current design:

- GitHub OAuth is the SaaS default authentication model.
- Authenticated sessions and auth audit are specified.
- Human ownership of agents is explicit.
- Workspaces, organizations, memberships, policy sets, quotas, and commercial control-plane surfaces are now specified in the TLA.
- Cost tracking, approvals, credential audit, and activity audit are specified.
- Agent actions are traceable enough to support a customer-facing activity surface.

What is still true of the repo as a whole:

- There is no buildable SaaS product yet.
- There is no customer control plane, billing system, tenant model, or deployed app.
- The current readiness state is architectural, not product-ready.

## Already Covered In The Spec

- GitHub OAuth login flow.
- Stable user identity via provider subject ID.
- Session table with hashed tokens.
- Auth audit table.
- Agent ownership linked to authenticated users.
- Workspaces, organizations, memberships, tenant boundaries, and control-plane surfaces are specified.
- Usage quotas and billing account placeholders are specified.
- Approval queue, audit APIs, and cost breakdown APIs.

## SaaS Gaps Still Missing

### 1. Billing, Metering, And Quotas

The spec tracks cost, but cost tracking is not the same thing as SaaS billing.

Still missing:

- plans and plan entitlements
- subscriptions
- seat counts
- usage meters
- included usage vs overage pricing
- invoice generation
- payment provider integration
- failed payment handling
- hard and soft quota enforcement

The current budget model is agent-level operational safety, not customer billing.

### 2. Customer-Facing Control Plane

The TLA now names the required control-plane surfaces, but the concrete UX flows, endpoint contracts, and running product are not yet implemented.

Still missing:

- agent list and agent detail pages
- approval inbox
- credential management UI
- activity feed / factory view
- cost and usage pages
- workspace settings
- member management
- billing portal
- support/contact surfaces

This is still a major SaaS gap. The backend audit trail and required surfaces are specified architecturally, but the customer product layer does not exist yet as running software.

### 3. Abuse Prevention And Trust Controls

Public SaaS introduces abuse risks that do not exist in self-hosted mode.

Still missing:

- signup abuse prevention
- per-user and per-org rate limits
- suspicious automation detection
- workspace suspension model
- acceptable use enforcement
- high-risk action throttling for new accounts
- support and moderation tooling

### 4. Customer Data Lifecycle

The spec mentions retention and GDPR, but a SaaS needs self-serve lifecycle controls.

Still missing:

- account deletion flow
- data export flow
- retention by plan
- backup retention promises
- restore support model
- offboarding workflow
- customer-visible privacy and retention controls

### 5. Productized GitHub Sign-In

GitHub sign-in is now in the spec, but secure auth productization still needs implementation detail.

Still missing or still needing hardening detail:

- CSRF-resistant OAuth state handling
- session cookie policy
- account linking rules if more providers are added later
- verified-email policy for first-time signup
- handling GitHub account email changes
- disabled-account behavior
- login error UX and support flow

### 6. Team Collaboration And Sharing

SaaS value usually comes from team use, not just solo use.

Still missing:

- shared access and delegated operation without breaking the single accountable owner model
- workspace-level approvals
- mention/notification model
- role-based delegation and visibility rules
- cross-user audit views within the same workspace
- comment/review model around agent actions

### 7. Commercial And Legal Surfaces

SaaS needs customer-facing legal and business surfaces in addition to code.

Still missing:

- Terms of Service
- Privacy Policy
- acceptable use policy
- subprocessor list
- support policy
- pricing page assumptions
- incident communication model

## Highest-Priority SaaS Additions To The Spec

If SaaS is the target, the next product-control-plane additions should be:

1. Subscription and payment-provider model.
2. Customer-facing activity feed backed by the existing audit trail.
3. Abuse controls and account suspension model.
4. Team collaboration and review surfaces.
5. Customer lifecycle flows: export, delete, restore, offboarding.
6. Legal and support surfaces.

## MIT License Implications

MIT works fine for a SaaS, but it changes the competitive posture.

What MIT means in practice:

- The code can be self-hosted by anyone.
- The hosted service can still be commercial.
- Terms of service for the hosted platform are separate from the code license.
- You cannot rely on license restriction as your moat.

If you choose MIT, your moat becomes:

- best hosted experience
- trust and security posture
- operational reliability
- integrations
- enterprise controls
- brand and trademark

That is a workable model, but it is an execution moat, not a licensing moat.

## Bottom Line

GitHub sign-in is now included in the TLA spec, so SaaS authentication is no longer undefined.

But SaaS readiness still requires the specified control plane to be implemented as a real product:

- billing and subscription operations
- customer UI
- abuse controls
- collaboration surfaces
- customer data lifecycle
- legal and support surfaces

Open Pincery is now closer to a credible SaaS architecture, but it is still not a complete SaaS application until the control plane exists as running software.
