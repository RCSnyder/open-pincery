# GenericAgent — Technical Notes

**Source:** [lsdefine/GenericAgent](https://github.com/lsdefine/GenericAgent) (MIT, Python, 5k⭐, 544 forks). README reviewed Apr 2026; technical report PDF and experiments repo exist but are not extracted here. V1.0 released 2026-01-16; active development through Apr 2026.

**Why this doc exists:** GenericAgent is the most compact public instance of a _self-evolving_ agent: ~3K lines of core code, ~100-line agent loop, 9 atomic tools, a 5-layer memory system that keeps context under 30K tokens, and a skill-crystallization loop that converts each successful novel task into a permanent reusable SOP. The architecture directly addresses "how does an agent stay relentless on a mission instead of stalling or forgetting?" — a question Open Pincery's current bets do not fully answer. This file extracts the technical claims worth absorbing and flags the tensions with the north-star.

---

## Headline Claims

1. **Don't preload skills — evolve them.** Ship a tiny seed (3K LoC, 9 atomic tools) and let the agent grow its own skill tree from actual task executions. After a few weeks of use, the agent has a skill tree no other instance has.
2. **Each successful novel task auto-crystallizes into a reusable SOP.** First solve: the agent autonomously installs deps, writes scripts, debugs, verifies. On success, the execution path is written to the L3 memory layer as a callable skill. Next similar task is a one-line invocation.
3. **Layered memory keeps context small and relevant.** Five tiers (L0–L4) with strict roles; the agent loads only what the current step needs. Sustained context is <30K tokens vs. 200K–1M for peers — less noise, fewer hallucinations, higher success rate, ~order-of-magnitude lower cost.
4. **A ~100-line agent loop is enough.** `agent_loop.py` implements the whole `perceive → reason → act → remember → loop` cycle in roughly 100 lines. Complexity lives in skills and memory, not in the loop.
5. **Nine atomic tools + a code-run escape hatch covers byte-mediated work.** The agent never hits "I don't have a tool for this" — `code_run` lets it dynamically install packages, write scripts, call external APIs, or control hardware, then crystallize the result into a permanent tool.
6. **Self-bootstrap proof.** Per the README: every git operation in the repo — from `git init` through individual commit messages — was performed autonomously by GenericAgent. The author "never opened a terminal once." This is a strong falsifiable claim about end-to-end capability.

## Technical Claims with Arguments

### Layered memory (L0–L4)

| Layer | Name               | Role                                                                                   |
| ----- | ------------------ | -------------------------------------------------------------------------------------- |
| L0    | Meta Rules         | Core behavioral rules and system constraints. Always loaded.                           |
| L1    | Insight Index      | Minimal index for fast routing and recall. Small by design.                            |
| L2    | Global Facts       | Stable knowledge accumulated over long-term operation.                                 |
| L3    | Task Skills / SOPs | Reusable workflows for specific task types. Grows via crystallization.                 |
| L4    | Session Archive    | Distilled records of finished sessions for long-horizon recall. Introduced 2026-04-11. |

The separation of _rules_ (L0) from _facts_ (L2) from _skills_ (L3) from _archives_ (L4) is the key move. Each layer has a different update cadence, retention policy, and recall pattern. "Insight Index" (L1) is the working primitive that makes the whole thing searchable without loading everything.

### Nine atomic tools

The agent's entire interface to the outside world (per the README):

- `code_run` — execute arbitrary code.
- `file_read`, `file_write`, `file_patch` — filesystem.
- `web_scan`, `web_execute_js` — browser interaction (real browser, session preserved).
- `ask_user` — human-in-the-loop confirmation.
- Plus 2 memory-management tools: `update_working_checkpoint`, `start_long_term_update`.

The README is somewhat inconsistent on the count — "9 atomic tools" is the claim, the execution table shows 7, and memory tools are described as additional. Best reading: 7 execution primitives + 2 memory primitives = 9 total atomic tools. Hardware / mobile control (ADB) is a specialization _inside_ `code_run`, not a separate tool.

### `code_run` as dynamic capability extension

`code_run` is the universal escape hatch. When a task needs something outside the current tool set, the agent:

1. Installs whatever Python package provides the capability.
2. Writes a script that wraps the new capability behind a narrow interface.
3. Calls the script to solve the current task.
4. On success, crystallizes the script + invocation pattern into L3 as a permanent "tool" (actually a skill, but operationally tool-shaped).

This is the substrate-level answer to "how does the agent keep going when the current toolbox isn't enough?" It does not ask the operator for a new tool; it builds one.

### Real-browser injection (operational characteristic)

GenericAgent controls a real browser, preserving the operator's logged-in sessions. Trade: powerful access to sites that refuse automated flows, strong coupling between operator's personal state and agent authority. The agent inherits whatever is logged in to the operator's browser when it runs.

### Self-evolution loop

```
[New task] → [Autonomous exploration] (install deps, write scripts, debug, verify) →
  [Crystallize execution path into skill] → [Write to memory layer] →
    [Direct recall on next similar task]
```

Promotion to L3 is **automatic on successful task completion**, not operator-gated. The operator can prune or edit the skill tree after the fact, but the default motion is crystallize-first.

### Context budget as a discipline

<30K tokens sustained (~15% of a 200K context, ~3% of a 1M context). This is not achieved by one clever trick; it is the output of (a) layered memory, (b) the Insight Index routing only relevant skills into the prompt, and (c) checkpointing working state to L0–L2 rather than accumulating it in the conversation.

---

## Implications for Open Pincery

These are my reading of what the above means for the north-star, not GenericAgent's claims.

1. **"Skill crystallization" is a missing primitive in OP's current bets.** OP Bet #6 says the mission catalog grows when the operator hits the same work three times. That is the _canonical catalog_ policy and it is correct for Tier 1 mission types. But it does not cover _intra-mission_ learning. When a pincer inside an exploratory mission figures out how to scrape a specific API or navigate a specific vendor portal, that execution path is valuable and should not be lost when the sandbox is torn down. There are **two layers of catalog**, not one:
   - **Operator catalog** (existing Bet #6): canonical Tier 1 mission types, promoted by the operator after three repetitions.
   - **Pincer skill tree** (new): auto-crystallized execution paths scoped to a pincer or a mission family, promoted on first success, pruned on failure or disuse.
     This is a new bet candidate; see north-star absorbed-advice section.
2. **Layered memory (L0–L4) is a better framing for Bet #2 than "event log + projections."** OP's current bet says "the memory controller is parametric + working + external; external = event log + projections + vector + graph." That is correct at the architecture level but weak at the operator-experience level. GenericAgent's L0–L4 decomposition maps cleanly onto OP's reality:
   - **L0 (Meta Rules)** ≈ the Professional Bar + acceptance-contract rules. Always loaded.
   - **L1 (Insight Index)** ≈ a small queryable index over projections for fast recall without loading the event log.
   - **L2 (Global Facts)** ≈ stable operator facts (their preferences, vendors, conventions) — _not_ currently modeled in OP as a distinct layer.
   - **L3 (Task Skills / SOPs)** ≈ the pincer skill tree (above).
   - **L4 (Session Archive)** ≈ completed-mission digests for long-horizon recall.
     The event log remains the ground truth _underneath_ all five layers. This is not a new memory system; it is a way to organize what we already have so operators and pincers can reason about it.
3. **A `code_run` primitive is the natural implementation of Bet #11a's sandbox.** The sandbox primitive we added last pass is about _safety_ — agent-authored code runs somewhere disposable. GenericAgent's `code_run` is about _capability_ — the agent can write and run code to extend itself. These are the same primitive at two viewing angles: the sandbox is the container, `code_run` is the operation against it. OP should spec `code_run` explicitly (not just "sandbox exists") as a capability that any pincer can be granted, scoped by governance class.
4. **Real-browser-session authority is a capability-model test case.** OP's capability model must be able to express "this pincer may use the operator's authenticated browser session for domain X, but not domain Y." Naively granting "use my browser" is ambient-authority territory — exactly what Bet #3 says to avoid. The capability grant should be domain-scoped and session-scoped, with the pincer required to emit a signal when attempting to navigate outside its declared domain list.
5. **A ~100-line agent loop is a useful complexity ceiling.** OP's wake loop is already similar in shape. The discipline worth importing: if the loop itself grows much beyond a page, complexity is leaking from _skills_ and _memory_ (where it belongs) into the _loop_ (where it doesn't). Treat loop line count as an informal tripwire.
6. **Auto-crystallization does _not_ violate Bet #11's "cannot silently expand its own authority."** This needs to be explicit because it looks like it might. Crystallizing an execution path into an L3 skill compresses _already-granted, already-executed_ work into a faster re-run. It does not grant new capabilities, raise budget, or rewrite the charter. The distinction: "I now know how to do this faster" vs. "I now have permission to do something I didn't have permission for." The first is fine; the second remains forbidden. The substrate should enforce this by requiring crystallized skills to declare their capability dependencies up front — a skill that requires `github:write` cannot be invoked by a pincer that was only granted `github:read`.
7. **Context-budget discipline belongs as a Professional Bar adjacent criterion.** The Professional Bar currently has six items. GenericAgent's discipline ("keep context <30K so the agent sees signal, not noise") is the substrate-level analog of "a professional does not work while drowning in paperwork." Not a seventh bar item — a property the memory controller enforces on behalf of all six.

## What to Discard (or Defer)

- **Python.** GenericAgent is Python; OP is Rust. This rules out adoption as a library. Fine — we are absorbing ideas, not code.
- **Streamlit / PyQt / WeChat-bot frontends.** Operator UI is out of scope for OP (Non-Goal #1: no React-heavy product UI; inbox surfaces come via OpenClaw). These are specific to GenericAgent's distribution model.
- **ADB / mobile control.** Not currently on OP's roadmap; revisit if a Tier 1 mission ever requires it.
- **Skill Library "million-scale" claim** (2026-03-10 news item). Interesting if it materializes as a verifiable corpus, but the README does not link a reviewable artifact. Treat as aspirational for now.
- **The "never opened a terminal" self-bootstrap claim.** Strong and interesting, but asymmetric: the author is also the framework author, so there's a selection effect. Worth testing independently before relying on it in OP's pitch. Do not cite as external validation.
