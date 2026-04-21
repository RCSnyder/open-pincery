# Cloudflare's Internal AI Engineering Stack — Technical Notes

**Source:** Cloudflare blog post (Apr 2026), [The AI engineering stack we built internally — on the platform we ship](https://blog.cloudflare.com/internal-ai-engineering-stack/)

**Why this doc exists:** Cloudflare is one of the only organizations publicly describing an at-scale (3,683 users active in the last 30 days, 47.95M requests / 30 days, 241.37B tokens routed) internal agentic AI platform. The architecture is unusually concrete and unusually aligned with Open Pincery's substrate instincts (single proxy, central audit, per-user attribution, typed repo context). This file extracts the technical claims and arguments that are worth absorbing.

---

## Headline Claims

1. **Route every LLM request through one proxy Worker from day one, even if direct connection is simpler.** That single choke point is what later makes per-user attribution, model catalog management, permission enforcement, Zero Data Retention injection, and catalog hot-refresh possible without touching any client config. Centralizing was worth the day-one overhead.
2. **An MCP Portal with one OAuth beats N MCP servers with N auth flows.** Cloudflare aggregates 13 internal MCP servers and 182+ tools behind a single Access-gated endpoint. Without this, each agent client has to negotiate auth against each server; with it, one OAuth covers everything, and governance lives in one place.
3. **Every tool definition costs context window tokens before the model starts working — solve it at the portal, not per server.** 34 GitLab tools consumed ~15,000 tokens (7.5% of a 200K window) on every request. The "Code Mode" fix collapses N server-side tools into two portal-level tools (`portal_codemode_search`, `portal_codemode_execute`) and lets the agent discover tools through code.
4. **Repos need a structured, curated `AGENTS.md` file or agents produce plausible-but-wrong code.** The failure mode they kept seeing: agent-generated changes that looked right and were wrong for that repo. Root cause: missing local context (test command, conventions, off-limits files). Fix: an AGENTS.md in every repo that makes that context explicit.
5. **Auto-generated `AGENTS.md` across 3,900 repos is better than nothing, but staleness kills it.** An AI code reviewer in CI flags when repository changes imply AGENTS.md should be updated. Without that feedback loop, the files rot and become worse than having none.
6. **You need a service catalog with structured ownership before MCP servers are useful.** Backstage — not a Cloudflare product — tracks 2,055 services, 544 systems, 1,302 databases, 375 teams, 6,389 users, and the dependency edges between them. The MCP server exposes it; the catalog itself is the foundation.

## Technical Claims with Arguments

### Platform layer

- **Single URL configures everything.** One `opencode auth login <url>` command returns auth + full config (providers, models, MCP servers, agents, commands, default permissions) from a `.well-known/opencode` discovery endpoint served by the proxy Worker. No manual API keys, no per-machine MCP setup.
- **Config-as-code, compiled at deploy time.** Agents and commands are authored as markdown files with YAML frontmatter. A build script compiles them into a single JSON config validated against the OpenCode schema. Every new session picks up the latest version automatically. A new rollout to 3,000+ users is `wrangler deploy`.
- **Local config always overrides shared config.** The shared defaults are organization-wide, but individual users can override the default model, add agents, or adjust scoped permissions without affecting anyone else.
- **No API keys on user machines, ever.** The proxy Worker injects the real provider key server-side after JWT validation. The client config has an empty `apiKey` field.
- **Per-user attribution without exposing identity to providers.** After JWT validation, the Worker maps the user's email to a UUID (D1 for persistence, KV for read cache). AI Gateway and providers only see the anonymous UUID in `cf-aig-metadata`. This gives cost tracking and usage analytics without leaking user identities to model vendors.
- **Zero Data Retention is injected per request, not per deploy.** An hourly cron fetches the current OpenAI model list, caches it in Workers KV, and injects `store: false` on every model. New models get ZDR automatically; no config redeploy.

### Knowledge layer

- **The knowledge graph lives outside the LLM.** Backstage holds 16K+ entities and their relationships (ownership, dependency, API schema, Tech Insights scores). The agent queries it through the Backstage MCP server (13 tools). Without this, the agent "reads the code in front of it but can't see the system around it."
- **`AGENTS.md` structure.** A short, high-signal file per repo:
  - **Repository**: runtime, test command, lint command.
  - **How to navigate this codebase**: where workers live, where MCP definitions live, test layout convention.
  - **Conventions**: testing framework, API patterns, links to internal RFCs.
  - **Boundaries**: do not edit generated files, do not introduce new background jobs without updating config.
  - **Dependencies**: what this service depends on, what depends on it.
- **AGENTS.md generation pipeline.** Pull entity metadata from Backstage, analyze the repo structure (language, build system, test framework, directory layout), map the detected stack to relevant internal standards, let a capable model draft the file, open a merge request so the owning team reviews and refines. 3,900 repos processed this way.
- **Staleness is worse than absence.** A wrong AGENTS.md actively misleads agents. The AI code reviewer in CI watches for changes that imply the file should be updated. Without that loop, the generator's initial pass decays.

### Enforcement layer

- **Every MR gets an automatic AI review.** Integration is a single CI component teams add to their pipeline. Reviews check against the Engineering Codex (internal standards) and the repo's AGENTS.md.
- **Enforcement is how quality holds at scale.** The platform layer makes agents easy to use; the knowledge layer tells them how the codebase works; the enforcement layer catches what the first two miss. Without the third, the other two decay.
- **Reviewer is a multi-agent coordinator, not a monolith.** The CI job runs OpenCode with a coordinator that first classifies each MR into a risk tier (trivial / lite / full), then delegates to specialist agents: code quality, security, codex compliance, documentation, performance, release impact. Each specialist reads the repo's AGENTS.md, pulls relevant Engineering Codex rules from a central repo, and posts findings back as structured MR comments. The coordinator is stateless per execution.
- **Model-per-role routing is configured centrally, not per-CI-job.** A Workers-based config service maps each reviewer agent to a model; Workers AI (Kimi K2.5) handles ~15% of reviewer traffic — primarily documentation review — while frontier models (Opus 4.6, GPT 5.4) handle security-sensitive and architecturally complex reviews. Swapping a model is a config change, not a template change.
- **Findings are tiered and cite stable rule IDs.** Each finding has a severity (Critical / Important / Suggestion / Optional Nit) and, when it maps to a standard, cites the specific Engineering Codex rule ID. Reviews are broken into categories (Security, Code Quality, Performance) so reviewers scan headers, not walls of text. The reviewer remembers prior rounds — it does not re-raise issues that were fixed between iterations.
- **Engineering Codex: standards as agent-readable skills.** Codex rules are authored through a multi-stage distillation process that outputs both (a) machine-citable rules of the shape _"If you need X, use Y. You must do X if doing Y or Z"_ and (b) an agent skill using progressive disclosure and a nested markdown hierarchy. Engineers invoke the skill locally ("how should I handle errors in my Rust service?"); the reviewer invokes the same skill at MR time. One team ran a compliance audit as a multi-agent consensus against these rules, each requirement scored COMPLIANT / PARTIAL / NON-COMPLIANT with remediation steps — reducing what took weeks manually to a repeatable process.

### Execution environment

- **Agent-generated code runs in disposable sandboxes, not on the host.** Cloudflare's Dynamic Workers pattern ("Code Mode") and the Sandbox SDK (GA during Agents Week) give agents isolated environments for cloning repos, installing dependencies, and running tests. The host that orchestrates the agent never executes agent-authored code directly.
- **Background agents reuse the sandbox primitive for long-running work.** Architecture: Durable Objects + Agents SDK for orchestration, delegating to Sandbox containers for full dev environments. The Agents SDK now supports sessions that run long enough to clone a large repo, run a full test suite, iterate on failures, and open an MR in a single session — previously this required workarounds.
- **Stateful, long-running sessions are a named primitive.** `McpAgent` + Durable Objects hold session state across the long-running agent's lifetime; Workflows (scaled 10× during Agents Week) handle the durable multi-step execution. The sandbox is the per-step execution surface; the Durable Object is the session state; the Workflow is the durable sequencing. Three distinct primitives, not one conflated runtime.

### Volume and outcome claims

- 3,683 internal users active in the last 30 days (60% company-wide, 93% of R&D) of ~6,100 total employees.
- 47.95M AI requests / 30 days.
- 295 teams on agentic AI tools and coding assistants.
- 20.18M AI Gateway requests / month.
- 241.37B tokens through AI Gateway; 51.83B tokens on Workers AI (serverless inference on open-weight models).
- Merge-request rate: 4-week rolling average ~5,600/week → 8,700/week in 11 months; one week hit 10,952, nearly 2× the Q4 baseline.
- Workers AI cost comparison: a security agent processing 7B tokens/day on Kimi K2.5 would cost ~$2.4M/year on a mid-tier proprietary model; on Workers AI it is **77% cheaper**.
- Provider mix: Frontier labs (OpenAI, Anthropic, Google) 91% of requests; Workers AI (open-weight) 9%, growing.

---

## Implications for Open Pincery

These are my reading of what the above means for the north-star, not Cloudflare's claims.

1. **"Single proxy Worker from day one" maps directly to the reasoner abstraction (Bet #10).** Open Pincery already plans a three-axis reasoner abstraction (provider / model / governance-class). The Cloudflare evidence says: _do not skip it, do not let clients hit providers directly, do not defer the attribution layer_. The reasoner abstraction is the proxy. Every argument for centralizing at Cloudflare — per-user (per-mission, per-pincer) attribution, Zero Data Retention injection, catalog refresh, governance-class enforcement, budget tracking — is already a reasoner-abstraction concern. This reinforces existing intent; it does not add new work.
2. **`AGENTS.md` is the repo-level analog of a mission acceptance contract.** The failure mode Cloudflare describes — "plausible changes that are wrong for the repo because the agent lacks local context" — is the same failure the acceptance contract exists to prevent at the mission level. An acceptance contract ("review every PR against these rules; escalate if confidence < 0.6") is to a mission what AGENTS.md is to a repo. The mission catalog is Open Pincery's version of "every repo needs one." This is a new framing for Bet #5 (every mission type has an acceptance contract), not a new bet.
3. **The codebase steward (Tier 1 mission #1) should consume and maintain AGENTS.md.** When the codebase steward reviews a PR, it should read the repo's `AGENTS.md` as part of its context, and flag when the PR invalidates the file. This is a concrete feature the founder can use on day one and a natural fit for the first Tier 1 mission.
4. **A service catalog (Backstage-like) matters more than expected, and Open Pincery does not have one.** At one-person scale, the "catalog" is the founder's head, the repo layout, and the CRM. At team scale the catalog becomes load-bearing. The substrate's memory projections already hint at this — pincers, missions, acceptance contracts, capability grants are catalog-shaped — but "services the operator runs" is not currently modeled. **Durable Bet candidate:** the substrate should register the operator's owned systems as a catalog the agents can query, not just the missions and pincers. This belongs in the north-star if v8 or v9 find the founder adding it organically; flag it as a watch item for now.
5. **Tool-definition context cost is a real constraint.** If Open Pincery exposes 50 typed tools through MCP, the context-window overhead will hit the same 7.5%+ ceiling Cloudflare documented. Code-Mode-style tool discovery (a search tool and an execute tool, with the catalog loaded lazily) is the proven fix. This is a concrete constraint on the MCP tool surface Bet #9 commits to. Not a crisis; a design constraint.
6. **Per-pincer attribution is the direct equivalent of Cloudflare's per-user UUID.** Every LLM call the substrate issues should carry `pincer_id`, `mission_id`, and `capability_scope` as metadata, so budget tracking, audit, and governance-class enforcement can attribute cost and behavior back to the right entity. This is already implicit in the event log; the Cloudflare evidence says make it explicit at the proxy layer too.
7. **Config-as-code, compiled at deploy time, with local override.** Cloudflare's pattern is the right pattern for Open Pincery's catalog: Tier 1 missions authored as structured files (YAML frontmatter + markdown charter), compiled into a validated JSON catalog at build, with operator-local overrides layered on top. No one should hand-edit a canonical catalog at runtime. This is a design-layer decision for v7+.
8. **The cost delta for open-weight on sovereign inference is material, not theoretical.** 77% cheaper on Workers AI vs a mid-tier proprietary model, at 7B tokens/day. For Open Pincery's sovereignty ladder (Stage 4: self-hosted open-weight default), this is the economic argument the pitch currently under-uses. A solo founder running persistent agents at even a fraction of that volume cannot afford to route everything through frontier endpoints; sovereignty is a cost-survival feature, not just a data-governance feature. (The 7B/day figure is a single Cloudflare agent at enterprise scale; scale the math down for the solo case but the direction is the same.)

9. **Ephemeral sandboxes belong in the substrate as a first-class primitive.** This is the biggest substrate gap the Cloudflare evidence exposes. For the exploratory runner and codebase steward Tier 1 missions — and for _any_ pincer that builds or spins up a sub-pincer — agent-authored code must execute in a disposable, capability-scoped environment that is not the substrate host. Candidate axes of authority for a pincer to receive:
   - **Capabilities** (what tools / credentials it may use) — already a Durable Bet (#3).
   - **Budget** (dollars + wall-clock) — already a Durable Bet (#4).
   - **Execution environment** (a sandbox with declared filesystem / network / compute limits) — _not yet a Durable Bet; this is the gap_.
     An exploratory mission with $200 and no sandbox is not executable without either compromising the host or punting back to the operator, defeating the mission. For Open Pincery specifically, the sandbox primitive should be Rust-native and operator-host-friendly: a `Sandbox` capability backed by ephemeral Linux containers, Firecracker microVMs, or a `pocket-computer`-style subprocess jail depending on governance class.

10. **Acceptance contracts should be authored as skills with stable rule IDs, not as free-form prose.** Cloudflare's Engineering Codex distills standards into (a) machine-citable rules ("use X if doing Y") and (b) an agent skill with progressive disclosure. The same rules are consumed by the human author, the reviewing agent, and the CI job — one source of truth, three surfaces. For Open Pincery: Tier 1 mission acceptance contracts should live as skills under `.github/skills/contracts/<mission>/` with stable rule IDs (`CONTRACT-CODEBASE-STEWARD-01`, etc.) that are citable in review output and mission escalations. This is a concrete design pattern for Bet #5, not a new bet.

11. **Missions compose through a classify-and-fanout pattern, not just free-form sub-pincer spawning.** Cloudflare's reviewer coordinator first classifies the work (trivial / lite / full) and then delegates to named specialists (code quality, security, codex compliance, docs, performance, release impact). This is more specific than "spawn sub-pincers" — it is a named orchestration convention: **classify → tier → fan out to specialists → aggregate**. For Open Pincery, this is a catalog-level convention above the substrate (Bet #12 says conventions live there), not a substrate primitive. Name it when the codebase-steward mission starts fanning out; it is the natural pattern for any mission complex enough to have a risk assessment.

12. **The reasoner abstraction needs a model-per-role axis, not just provider / model / governance-class.** Cloudflare routes documentation review to Kimi K2.5 (open-weight, 15% of reviewer traffic) and security-sensitive review to frontier models — chosen by _task_ not by _data governance_. A Workers-based config service does the mapping so models can be swapped without changing CI. For Open Pincery: the reasoner abstraction should accept a _role_ on each call (`{role: "security-review", governance_class: "enterprise-bounded"}`) and resolve `(role, governance_class) → (provider, model)` through catalog config. This is an additive constraint on Bet #10, not a rewrite.

## What to Discard (or defer)

- **The CI-integrated AI code reviewer as a distinct product layer.** Open Pincery's codebase-steward Tier 1 mission _is_ this, at one-person scale. Cloudflare needed a separate CI component because they had 3,000+ repos and 295 teams; the founder has one repo and one team. Do not replicate Cloudflare's three-layer (platform / knowledge / enforcement) decomposition at solo scale.
- **Backstage itself.** The catalog concept is worth borrowing; Backstage the product is a team-scale tool with a self-hosted server, a database, a plugin ecosystem, and ownership rituals. The solo founder's catalog should be a small table in the operator's own Postgres. Borrow the idea; do not adopt the artifact.
- **MCP Server Portal as infrastructure.** At one-person scale, Open Pincery _is_ the portal — it is already aggregating tools and capability scopes behind a single auth plane. The north-star's "MCP outward to tools, MCP / A2A inward to peers" (Bet #9) covers this. Do not introduce a separate portal product.
