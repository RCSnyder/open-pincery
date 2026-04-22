# Open Pincery — Security Threat Model

> **Status.** First published with v9 under AC-54.  This document is
> the canonical statement of what Open Pincery defends against, what
> it does not, and how to report a vulnerability.  It is versioned with
> the codebase and every change is reviewed against
> [`scaffolding/scope.md`](../scaffolding/scope.md).

Open Pincery runs durable, event-driven AI agents that hold real
credentials, reach real external services, and operate autonomously
between human interactions.  That combination makes it an unusually
high-leverage target: a single successful exploit can exfiltrate
secrets, impersonate a user, or corrupt the event log that is the
platform's only source of truth.  v9 closes that gap by elevating the
sandbox, credential, session, and multi-tenant boundaries from
aspirational to enforced.  This document describes the resulting
security posture.

---

## Adversary Capabilities

We reason about three adversaries whose capabilities overlap but whose
trust levels differ.

### Malicious prompt

A user or upstream system supplies input that is eventually read by an
LLM call.  The adversary controls the textual content of that input
only.  They cannot read the event log, cannot call tools directly,
cannot observe environment variables, and cannot influence the prompt
template beyond the message body itself.  They can attempt to persuade
the LLM to exfiltrate data through tool calls or through response
text, and can embed instructions designed to subvert the system
prompt.

### Compromised LLM or LLM provider

The LLM returns a response under adversary control.  The adversary can
choose any text, any tool-call JSON, and any argument payload allowed
by the response schema.  They cannot inject raw syscalls, cannot
bypass the tool registry, and cannot forge events — every tool call
routes through the runtime harness and every event is signed by the
process writing it.

### Compromised tool output

A tool invocation reaches a real external service which returns
adversarial output.  That output flows back into the next LLM turn
(directly or via a projection).  The adversary's reach is the same as
a malicious prompt, plus the ability to shape multi-turn feedback
loops.  They cannot escape the sandbox that executed the tool because
the sandbox is destroyed before the output is consumed; they cannot
reach credentials because plaintext never entered the tool process in
the first place (see AC-71).

---

## In-Scope Attacks

v9 commits to defending against the following attack classes.  Each
row names the primary mitigating acceptance criterion; the full chain
of evidence is in [`scaffolding/readiness.md`](../scaffolding/readiness.md).

### Prompt-injection exfiltration

An adversary crafts input designed to coax the LLM into emitting a
tool call that leaks a secret through an attacker-controlled sink
(arbitrary URL, DNS lookup, crafted filename).  Mitigated by:

- **AC-53 Zerobox sandbox** — every tool runs under bubblewrap +
  seccomp-bpf + landlock + cgroup v2 with a default-deny network
  namespace, so unapproved egress is impossible even if the LLM is
  persuaded to try.
- **AC-72 Per-agent egress allowlist** — approved destinations are
  enumerated per agent and enforced by slirp4netns; every blocked
  connection emits a `sandbox_blocked` event.
- **AC-71 Secret injection proxy** — credentials never enter the agent
  or tool address space, so even a successful exfil primitive has no
  plaintext to leak.

### Tool-sandbox escape

An adversary persuades the LLM (or controls the tool output) to try
to break out of the sandbox: read `/etc/shadow`, attach to host pid 1,
mount a fork bomb, escalate via `unshare`.  Mitigated by:

- **AC-53 Zerobox** — six-layer defence (namespaces, seccomp-bpf
  allowlist, landlock LSM, cgroup v2 resource caps, `no_new_privs`,
  full capability drop) enforced per-call.
- **Adversarial test matrix** — `tests/sandbox_escape_test.rs` runs at
  least twelve escape payloads across four categories (filesystem,
  network, privilege, resources); every one must fail and emit a
  `sandbox_blocked` event.

### Credential leak via event log

An adversary persuades the platform to write plaintext credentials,
API keys, or session tokens into the event log, projection blobs, or
structured logs.  Mitigated by:

- **AC-71 Secret injection proxy** — plaintext lives only in the proxy
  process memory and is handed to tools via env, stdin, or header; it
  is never serialised through a tool argument or event payload.
- **AC-74 Credential plaintext hygiene** — `SecretBuffer<ZeroizeOnDrop>`
  + `mlock` + a tracing `RedactionLayer` with six credential-shape
  regexes; an event-emit filter rejects any event whose JSON payload
  matches a credential pattern and emits `credential_plaintext_rejected`
  instead.
- **AC-54** (this document) — every deviation from the plaintext
  discipline is a security bug.

### Session hijack

An adversary steals or forges a session cookie and authenticates as a
legitimate user.  Mitigated by:

- **AC-58 Session TTL + refresh + revoke** — sessions expire after
  24 h by default; refresh and revoke are first-class endpoints and
  CLI verbs.
- Session cookies are served with `HttpOnly; Secure; SameSite=Strict`.
- Session tokens are compared with `subtle::ConstantTimeEq` to prevent
  timing-based recovery.
- **AC-59 Users + roles** — even a hijacked session is bounded by the
  victim's role (`admin`, `operator`, `viewer`); the `viewer` role
  cannot invoke tools or write state.

### Webhook replay

An adversary captures a signed webhook body and replays it, or crafts
a body whose HMAC collides with a previously observed one.  Mitigated
by:

- HMAC-SHA256 signature verification with `subtle::ConstantTimeEq`.
- Per-delivery nonce stored in the webhook deduplication table with a
  48 h TTL; the second attempt returns HTTP 409.
- Webhook secrets are rotated via `pcy webhook secret rotate`, which
  supports overlapping validity windows so live clients can migrate
  without downtime.

---

## Out-of-Scope

Open Pincery does **not** claim to defend against the following
threats; operators carrying these risks must rely on host-, database-,
and organisation-level controls instead.

### Compromised host

If an attacker has shell access on the machine running Open Pincery,
or can read the process memory, or can modify the binary, the sandbox
guarantees no longer hold.  Mitigations are operational — disk
encryption, SSH hardening, host-level EDR — and are outside the
platform's remit.  The `docs/runbooks/` operator guides document the
minimum expected host posture.

### Compromised PostgreSQL

If an attacker can issue arbitrary SQL against the Open Pincery
database, they can read the event log, export ciphertexts, modify
projections, and grant themselves admin.  v9 does not attempt to
defend against this; PostgreSQL access is treated as equivalent to
root on the platform.  Protect the database with network controls,
least-privilege roles, and audited backups.

### Insider with DB credentials

A person holding legitimate PostgreSQL credentials (for backup,
analytics, or support) is outside v9's trust model.  The vault key is
required to decrypt credential ciphertexts, but an insider with both
DB and vault-key access has full authority.  v9 assumes the operator
manages their insider risk through process (separation of duties,
rotation, auditing) rather than through cryptographic controls.

### Supply-chain compromise of upstream crates

Open Pincery pins every dependency through `Cargo.lock`, enforces
`cargo deny` in CI, and lists every sandbox crate's minimum
maintenance bar in `deny.toml`.  A compromised upstream crate that
passes CI, however, would still be dangerous; customers relying on
that guarantee should pair it with their own SBOM review.

### Denial of service against the HTTP layer

Rate limiting at `OPEN_PINCERY_RATE_LIMIT` bounds per-IP traffic, but
v9 does not ship DDoS protection.  Self-hosters should front the
service with Caddy, Cloudflare, or another CDN-class WAF for
internet-exposed deployments (see `docker-compose.caddy.yml`).

---

## Disclosure

Please report suspected vulnerabilities privately.

**Preferred channel.** Open a private advisory via
[GitHub Security Advisories](https://github.com/RCSnyder/open-pincery/security/advisories/new).
This keeps the report out of public issues, gives us a managed
workflow, and lets us coordinate a fix and disclosure schedule with
you.

**Alternate channel.** Email `security@open-pincery.dev` with the
subject line `[security] <short summary>`.  We acknowledge every
report within three business days; if you have not heard back in that
window, please nudge via the GitHub advisory form above.  A PGP key
for encrypted reports is published at
[`docs/security-pgp.asc`](./security-pgp.asc) (added in the v9.2
hardening phase — until then, please use GitHub Security Advisories
for sensitive material).

We ask that reporters:

- Give us a reasonable chance to investigate before public disclosure
  (we target 90 days).
- Avoid testing against production self-hosted instances other than
  your own.
- Do not exfiltrate data beyond what is strictly necessary to
  demonstrate impact.

We commit in return to:

- Acknowledge every report and keep reporters informed of progress.
- Credit reporters in release notes unless anonymity is requested.
- Ship a patch release for every `Critical` or `High` finding.

---

## Deployment Hardening Checklist

This checklist is not itself an acceptance criterion — it is the
operational companion to the controls above.  Self-hosters running v9
in production are expected to satisfy every item.

- [ ] TLS terminated by Caddy, Cloudflare, or another reviewed proxy.
- [ ] `OPEN_PINCERY_SANDBOX_MODE=enforce` (never `disabled` in
      production; `audit` is acceptable only during staged rollout).
- [ ] `OPEN_PINCERY_ALLOW_UNSAFE` is unset.
- [ ] Host kernel supports user namespaces and landlock (`uname -r`
      ≥ 5.13, `sysctl kernel.unprivileged_userns_clone=1`).
- [ ] PostgreSQL reachable only from the application host; no public
      `listen_addresses`.
- [ ] Vault key (`OPEN_PINCERY_VAULT_KEY_B64`) stored in a secret
      manager, not in plaintext env files.
- [ ] Webhook secrets rotated at least every 90 days.
- [ ] Backups of the events table are encrypted at rest.
- [ ] Monitoring alerts on `sandbox_blocked`, `credential_plaintext_rejected`,
      and failed `login` events.

---

*This document is living.  Propose updates by opening a PR that
touches this file and referencing the relevant `AC-*` from
`scaffolding/scope.md`.*
