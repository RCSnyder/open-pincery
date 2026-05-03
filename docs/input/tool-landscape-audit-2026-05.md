# Tool Landscape Audit — 2026-05

> **Frame.** Sibling document to `competitive-landscape.md` and `agent-harness-landscape-2026-04.md`.
> Where those documents map the agent-product market, this one audits four concrete peers
> that surfaced during recent ideation: three open-source codebases — **NVIDIA/OpenShell**,
> **mattpocock/sandcastle**, **badlogic/pi-mono** — plus one productized methodology,
> **BuildLoop's Founder OS**. The goal is not "what should we copy" but
> "where do these projects actually sit relative to the pincer / mission / sandbox split
> in `north-star-2026-04.md` Bet #11a, and what do we have to name and price-in?"
>
> Honest correction up front: a previous internal pass listed "pimono" as something the
> assistant didn't recognize. **pimono = badlogic/pi-mono.** The miss is recorded here so the
> next pass can grep for it.
>
> Founder OS is included even though it is a 1:1 coaching engagement, not a codebase, because
> it is the cleanest market signal yet for what end-users want a pincer-shaped product to
> *be* — and its four-part vocabulary (Skills / Agents / Routines / You) maps onto Open
> Pincery's mission template scaffold almost one-to-one.

---

## 1. The three peers in one paragraph each

### 1.1 NVIDIA/OpenShell — Apache-2.0, Rust, ~5.5k stars

A "safe, private runtime for autonomous AI agents." The shipped product is a **per-agent
Kubernetes pod inside Docker** (K3s, gateway/control-plane separation), with a YAML
**policy engine** that locks the filesystem at sandbox-create and hot-reloads network /
inference rules. Egress goes through a privacy router doing **L7 method+path filtering**.
Bring-your-own coding harness: shipped community sandboxes include `claude`, `codex`,
`copilot`, `opencode`, **OpenClaw**, and Ollama. Credential bundles inject as env vars at
sandbox creation. **It is a sandbox runtime, not an agent.**

### 1.2 mattpocock/sandcastle — MIT, TypeScript, ~3.2k stars

A **TypeScript orchestration library**, not a runtime. `sandcastle.run({ agent, sandbox, task })`
is one-shot: pick a sandbox provider (docker / podman / Vercel Firecracker microVMs / your
own via `createBindMountSandboxProvider` or `createIsolatedSandboxProvider`), pick an agent
provider (`claudeCode` / `codex` / `opencode` / `pi`), pick a branch strategy
(`head` / `merge-to-head` / `branch`), and the library shepherds worktrees, idle-timeout
(default 600s), max iterations, and a `<promise>COMPLETE</promise>` completion signal. **It
is an orchestration shell that delegates both sandbox and agent.**

### 1.3 badlogic/pi-mono — MIT, TypeScript, **44.1k stars**, v0.72.1, ~211 releases, ~198 contributors

A TypeScript monorepo whose headline product is `pi`, "a minimal terminal coding harness."
Five published packages:

| Package | What it is |
| --- | --- |
| `@mariozechner/pi-ai` | Unified multi-provider LLM API (Anthropic / OpenAI Responses & Codex / Google / Vertex / Mistral / xAI / Groq / Cerebras / Bedrock / Cloudflare / OpenRouter / Vercel / MiniMax / Fireworks / Kimi / Xiaomi MiMo / GitHub Copilot / Ollama / vLLM / LM Studio / OpenAI-compatible). TypeBox tools, partial-JSON tool streaming, cross-provider context handoffs, browser-safe (sans Bedrock+OAuth), JSON-serializable `Context`, faux provider for tests. |
| `@mariozechner/pi-agent-core` | Agent runtime — tool calling + state management. (README 404 at audit time; surface inferred from coding-agent's SDK usage.) |
| `@mariozechner/pi-coding-agent` (`pi`) | Interactive coding agent CLI. Four built-in tools (`read`, `write`, `edit`, `bash`) plus `grep` / `find` / `ls`. **No MCP, no sub-agents, no permission popups, no plan mode, no background bash, no built-in TODOs** — by design. Extensions are TypeScript modules; skills follow agentskills.io. Sessions are **JSONL trees with `id` + `parentId`**, `/tree` for in-place branching, `/fork` for new sessions, and `/share` to upload to HuggingFace via sister project `badlogic/pi-share-hf`. SDK + RPC + JSON modes for embedding. |
| `@mariozechner/pi-tui` | Terminal UI library, differential rendering. |
| `@mariozechner/pi-web-ui` | Web components for AI chat interfaces. |

**It is an opinionated harness + a TypeScript reasoner abstraction.** Sandcastle's `pi()`
agent provider _is_ this `pi`. Of the three open-source peers, pi-mono has by far the most market
traction.

### 1.4 BuildLoop / Founder OS — productized methodology, $2,950–$3,950 one-time, founding cohort May 2026

Not a codebase. A **four-week 1:1 coaching engagement** by Luuk Alleman (BuildLoop) that
builds a personal automation system for solo founders on top of Claude Skills + Anthropic's
file-based agent primitives. The shipped artifact is a **"Founder OS"** with four parts:

| Part | Founder OS name | Open Pincery analogue |
| --- | --- | --- |
| Skills | Markdown prompts Claude runs from (`gmail-triage.md`, `weekly-brief.md`, `money-monitor.md`) | Prompt templates + tool bindings |
| Agents | Composite workflows that read context across skills and decide (`monday-operator` reads calendar + inbox + Stripe → 7am brief) | **Pincer** (durable identity + wake loop) |
| Routines | Schedules (Mon 07:00 brief, Fri 17:00 close-out, day-3/day-7 follow-ups) | Wake schedule attached to a pincer-mission |
| You | The operator — directs, reviews, extends by writing prompts | Operator surface (control plane) |

Filtering rule for what gets automated: (1) happens 2+ times a week, (2) output has the
same shape every time, (3) Claude can make the call vs. only the human can. Outcomes are
binned into Build / Hybrid (Claude drafts, human decides) / Keep manual. Deliverable
includes 2–4 custom workflows + 1 composite agent + a written "playbook" for self-extension.
Client "runtime" is whatever Claude ships — Skills + scheduled jobs on Anthropic's
infrastructure. **No isolation, no event log, no replay, no portability** beyond the
Markdown files.

**It is a market-validated specification of Tier 1 missions** — the same five missions named
in `north-star-2026-04.md` (codebase steward, inbox triage, commitments tracker, weekly
digest, exploratory runner) are the same shape Founder OS sells as automatable. The novel
piece is the **admission filter** (volume / shape / judgment) — an explicit, named test for
whether a workflow belongs in an automated mission at all. The pricing ($2,950 for a build
that saves ~30 hrs/month vs. ~$5k/month VA equivalent) is also signal: solo founders are
already paying low four figures one-time for this exact concept.

---

## 2. Layer map — where each peer sits

```
                                      Open Pincery (Rust, Postgres, axum)
   +---------------------------------------------------------------------+
   | OPERATOR SURFACE  (control plane, replay, subpoena)                 |
   |   v4 vanilla-JS dashboard                                           |
   |   <----- pi-tui / pi-web-ui inform shape (TS reference only)        |
   +---------------------------------------------------------------------+
   | MISSION  (business intent: codebase steward, inbox, digest, ...)    |
   |   prompt templates + tools + budgets + tripwires                    |
   |   <----- sandcastle = a *single mission* shape: "one-shot dev task" |
   |   <----- pi-coding-agent = a *single mission* shape: "interactive   |
   |          terminal coding session"                                   |
   +---------------------------------------------------------------------+
   | PINCER  (durable agent identity, wake loop, event log)              |
   |   src/runtime/wake_loop.rs, src/api/, models/                       |
   |   <----- pi-agent-core = TS peer (one-process, no event log claim)  |
   |   <----- sandcastle.run = stateless invocation, no pincer identity  |
   +---------------------------------------------------------------------+
   | REASONER  (Bet #10: provider, model, governance_class, role)        |
   |   currently: thin Anthropic wrapper                                 |
   |   <===== pi-ai is the *reference design* for this abstraction       |
   |          (TS, but the four-axis shape is exactly what we need)      |
   +---------------------------------------------------------------------+
   | SANDBOX  (process isolation today; microVM deferred to v10)         |
   |   landlock + seccomp + capability gate                              |
   |   <===== OpenShell is a drop-in *runtime* candidate for code_run    |
   |          (Apache-2.0, K3s+Docker, L7 egress, YAML policy)           |
   +---------------------------------------------------------------------+
```

Boxes with `<-----` are inspirational; boxes with `<=====` are concrete integration candidates.

**Founder OS doesn't appear on this map** because it operates one level up: it sells the
*choice of which missions to install* and the *human-in-the-loop methodology* to install
them, then leaves the runtime entirely to Anthropic's hosted Claude. In Open Pincery's
frame, Founder OS is a **mission catalog + onboarding methodology**, not a runtime peer.
That's the right layer to learn from for v10+ go-to-market — not for any v9 implementation.

---

## 3. Where the security boundaries actually work

| Layer | OpenShell | Sandcastle | pi-mono | Founder OS | Open Pincery (today) |
| --- | --- | --- | --- | --- | --- |
| Filesystem | Locked at sandbox-create (YAML, immutable for life of pod) | Worktree per run, host-mounted | None — pi runs in your shell ("run in a container" is the user's job) | None — hosted Claude environment | Landlock per-process, capability gate |
| Network | L7 method+path egress proxy + hot-reload | Inherits from sandbox provider (docker/podman/Firecracker) | None | Whatever Claude tools allow | None at runtime; planned egress allowlist deferred |
| Inference | Privacy router + hot-reload allowlist | Picks one agent provider, no governance | pi-ai picks model per call; **no governance class** | Single vendor (Anthropic), single model class | Bet #10 reasoner abstraction (planned) |
| Credentials | Env-var injection at sandbox create (per-bundle) | Env via sandbox provider | env vars + OAuth (auth.json on disk) | Connectors / Skills metadata, scoped per-skill | sqlx Postgres credential vault (AC-43..45, encrypted at rest) |
| Tenant isolation | Per-agent K3s pod | One sandbox per `run()` | Single-user CLI | Single-user Claude account | Single-tenant today (multi-tenant deferred) |
| Audit / replay | Logs, no event-sourced replay | None — fire and forget | JSONL session tree + `/tree` + `/share` to HF datasets | None beyond Claude's own conversation history | **Event-sourced log + projections + replay (core differentiator)** |

**Reading.** OpenShell is the strongest peer at the **sandbox** layer; pi-mono is the
strongest peer at the **reasoner+harness** layer; sandcastle is the strongest peer at the
**single-mission orchestration** layer; **Founder OS is the strongest peer at the
mission-catalog + onboarding layer** — it has the *what to automate* answer that all three
codebases punt on. **None of them have the event-sourced operator surface that Open
Pincery treats as the product.** Bet #11's tripwire — "if the durable-agent-substrate
market gets eaten by a single hosted vendor before we have operator-grade replay and
subpoena, we lose" — survives this audit, *but* Founder OS is direct evidence that the
hosted-Anthropic path is already being productized at the methodology layer.

---

## 4. Naming reconciliation — what is a "pincer," precisely?

The user's question: _"Could a pincer be a simple Hermes agent? OpenClaw? pimono?"_

After this audit, the answer is **no, but they share DNA** with the pincer:

- **Hermes** (and the broader inference-runner class): solves the reasoner layer only.
  A pincer needs durable identity + event log + wake loop on top.
- **OpenClaw**: shipped as an OpenShell sandbox flavor — i.e., it's a **mission template
  that runs inside someone else's sandbox**. Pincer-adjacent at the mission layer, but
  not durable.
- **pi-mono / pi-coding-agent**: closest peer at the harness layer. Has tools, has state,
  has session tree, has SDK + RPC. **Misses three things to be a pincer:** (1) no
  event-sourced log distinct from the JSONL session — replay and subpoena conflate; (2) no
  wake/sleep loop — it's interactive or one-shot, not continuously alive; (3) no
  reasoner *governance class* — model selection is a per-call concern, not a policy
  attached to a role.
- **Founder OS "agent"**: the term Founder OS uses for what we call a pincer is literally
  **"agent"** — "composite workflow that reads context across skills and decides." The
  vocabulary collision is informative: the market already calls this thing an agent. Open
  Pincery's *pincer* word has to earn its keep by being more than that — specifically by
  being **durable across crashes and across operators**, which Founder OS's
  hosted-Claude-skill version is not.

**A pincer is the TypeBox `Context` of pi-ai + a wake loop + an event log + a governance
policy + a sandbox handle + an operator surface.** Said the other way: if you took pi-ai's
serializable `Context`, persisted it event-sourced instead of as a JSONL tree, attached a
wake schedule, scoped tool access by capability, and bolted the operator dashboard on top,
you'd have a pincer. That's a useful design scaffold. It is _not_ a build dependency: pi
is TypeScript, Open Pincery is Rust, and Bet #2 (single-binary deploy, Postgres + axum)
forecloses that path.

---

## 5. Seven gaps in the current product (carried from the prior ideation pass, now sharpened)

1. **Reasoner abstraction is shipped as a thin Anthropic wrapper.** Bet #10's
   `(provider, model, governance_class, role)` four-axis shape exists in scope but not
   in code. **pi-ai's `Model<Api>` + `Context` + `Tool` typing is a near-perfect
   reference design.** Re-implement in Rust; do not depend on TS.
2. **Sandbox is process-only.** Microvm/K3s isolation is deferred to v10. **OpenShell is
   a credible runtime to graft on for `code_run` capability** (Apache-2.0, Rust ABI is
   tractable). Cost: gateway + Docker. Reward: closes the "I let an agent run code on my
   laptop" objection in one move.
3. **No session export format.** pi-share-hf publishes pi sessions as a HuggingFace
   dataset. The "subpoenable memory" tripwire in north-star is not yet a shipped artifact —
   **we should define and ship a session-export format before a peer claims parity.**
   Likely path: a stable JSON projection of the event log + a CLI command. Add as v9
   follow-up.
4. **Credential vault exists; bundle-injection ergonomics don't.** OpenShell's
   per-sandbox credential bundle pattern is cleaner than today's per-prompt-template
   credential ref. Worth porting the *shape* (bundles named at sandbox create,
   injected as scoped env at tool exec time) without the runtime.
5. **Mission templates are not yet a first-class artifact.** pi-coding-agent's
   "skill / prompt template / extension / theme" four-class hierarchy maps cleanly to
   what mission templates should be. Tier 1 missions in the north-star (codebase steward,
   inbox triage, commitments tracker, weekly digest, exploratory runner) need this scaffold.
6. **No reference operator UI for streaming tool calls.** pi-tui's differential
   rendering + partial-JSON streaming for tool args is a mature pattern. Our v4 dashboard
   shows events post-hoc, not live. Follow-up after AC-79.
7. **No admission filter for missions.** Founder OS's three-question filter (volume ≥ 2/wk,
   stable output shape, judgment Claude can make) is the missing piece between
   `scaffolding/scope.md` and a Tier 1 mission template. Open Pincery has no documented
   test for whether a proposed mission belongs to a pincer at all vs. staying manual or
   becoming a hybrid (draft → human review). **Fix:** add an `admission_filter` section to
   the mission template scaffold and require new missions to pass it explicitly. Free,
   small, and forecloses a class of bad missions. Borrow the language directly
   ("volume / shape / judgment") with credit.

---

## 6. Four integration ideas, prioritized

### A. (Highest leverage) Adopt OpenShell as the `code_run` sandbox runtime

- **What.** Open Pincery delegates the `bash` / `code_run` capability to an OpenShell
  per-pincer sandbox. Capability gate stays in Rust; the sandbox executes inside K3s.
- **Bet alignment.** Bet #11a (substrate) + Bet #4 (defense in depth). Closes the
  microVM deferral.
- **Cost.** New runtime dep (Docker + K3s). Operator must run a daemon. Conflicts
  with Bet #2 (single-binary). **Mitigation:** make it opt-in — single-binary still works
  for sheds; OpenShell sandbox kicks in when scope.md declares a code-execution mission.
- **Risk.** NVIDIA project velocity could change; license is Apache-2.0 so a fork is
  always possible.

### B. Rebuild pi-ai's four-axis abstraction as the Rust reasoner

- **What.** Rust crate `op-reasoner` mirrors pi-ai's `Model<Api>` / `Context` / `Tool` /
  cross-provider handoff shape, plus our governance_class axis. **Zero TS dependency.**
- **Bet alignment.** Bet #10 (reasoner abstraction) directly. Unblocks Bet #11
  (substrate).
- **Cost.** ~2 weeks of Rust work, plus provider matrix testing. The provider matrix is
  the actual cost — pi-ai documents ~14 quirks per provider (cache_control format,
  developer-vs-system role, max_completion_tokens vs max_tokens, etc.).
- **Risk.** Provider drift is the long tail — pi-mono ships ~211 releases to keep up.
  Mitigation: faux-provider-style tests for the seam, real-provider tests behind feature
  flags, follow pi-mono's CHANGELOG as the canonical "what broke this week" feed.

### C. Define and ship a session-export format

- **What.** A stable JSON projection of the event log per pincer-mission run, plus
  `pcy export <pincer> --format session-json`. Bonus: a converter to the pi-mono session
  format so existing operators can replay our sessions in `pi /resume`.
- **Bet alignment.** Bet #11 tripwire (subpoenable memory).
- **Cost.** Small. The event log already exists; this is a projection + a CLI verb.
- **Risk.** Format lock-in. Mitigation: version the format, name v1 explicitly,
  document the projection rules.

### D. Adopt the Founder OS admission filter as a mission-template requirement

- **What.** Add a mandatory `admission_filter` section to mission templates with three
  fields: `volume` (≥ 2/wk evidence), `shape` (output schema must be stable; reference
  the projection or doc that proves it), `judgment` (the role/governance class is
  authorized to make this call without human review, OR it's marked Hybrid and the
  review step is wired in). Reject mission templates that fail any of the three.
- **Bet alignment.** Bet #6 (mission template hygiene) + Bet #11a (substrate fit). Cheap
  guard against scope creep into one-off automations.
- **Cost.** Hours, not days. Documentation + a single check at mission-template load time.
- **Risk.** None worth naming. Worst case the filter is a comment, not a runtime check,
  and it still pays off as a thinking tool.

**Recommended sequence: D → B → C → A.** D is hours of work and starts the discipline
before more missions land. B unblocks the reasoner story we've been promising since v6.
C defends the tripwire cheaply. A is the biggest upside but also the biggest operational
change; it should land after the reasoner is real.

---

## 7. Follow-ups (concrete, file-level)

- **`docs/input/competitive-landscape.md`**: add four rows — OpenShell (sandbox runtime
  category), Sandcastle (orchestration library category), pi-mono (coding-harness
  category, market leader at 44.1k★), and BuildLoop / Founder OS (productized methodology
  category — this is the closest market validation of Tier 1 missions Open Pincery has
  found). Add a tripwire row: "session-export parity vs pi-share-hf." Add a second
  tripwire: "hosted-Anthropic methodology vendors price-anchor at $2.9k–$3.95k one-time;
  Open Pincery's self-host story has to clear that without bundling consulting."
- **`docs/input/agent-harness-landscape-2026-04.md`**: add pi-coding-agent under
  Category 1 (Coding harness) with the explicit note that it is the market leader and
  ships **without MCP, sub-agents, plan mode, or permission popups by design**. That
  philosophy is signal for our own scope — every "no" is a deliberate trade-off worth
  understanding.
- **`scaffolding/scope.md`** Deferred section: append "session-export format (v1),"
  "reasoner abstraction crate (op-reasoner)," and "mission-template `admission_filter`
  section (volume / shape / judgment) borrowed from BuildLoop Founder OS" as named
  follow-ups so they don't get lost.
- **`docs/security/sandbox-architecture-audit.md`**: cross-reference OpenShell as a
  runtime option for the deferred microVM strategy gap.
- **(Optional)** `docs/reference/external-process-audit-2026-04.md`: note pi as a peer
  in the external-process landscape; record that `pi --mode rpc` over LF-delimited JSONL
  is the production-tested protocol shape if we ever expose Open Pincery via stdio.

---

## 8. What this audit explicitly does **not** do

- Does not propose adopting any of these projects as a runtime dependency today. The
  closest thing to a "yes" is OpenShell, and that's gated on Bet #11a's microVM
  deferral being lifted.
- Does not rename anything in the codebase. The pincer / mission / sandbox naming gap
  between code (`agent`) and north-star is a separate reconciliation, tracked in
  `scaffolding/log.md`. Founder OS's use of "agent" for what we call "pincer" sharpens
  the question but does not resolve it.
- Does not endorse, copy, or compete with Founder OS commercially. Founder OS is
  hosted-Anthropic methodology; Open Pincery is self-hostable durable substrate. They
  could coexist (a Founder OS graduate could migrate their mission set onto Open Pincery
  for sovereignty and replay) and the audit's recommendation is to design for that
  coexistence rather than against it.
- Does not score these projects against each other. They occupy different layers; the
  layer map in §2 is the comparison, not a leaderboard.

— Audit pass closed 2026-05.
