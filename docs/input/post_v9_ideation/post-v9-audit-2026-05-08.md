# Post-v9 Audit — Claims vs. Codebase (2026-05-08)

> **Purpose**: Re-run the deep-dive audit produced in chat after PR #4 merged
> (commit `23d8d0b`, 2026-05-08), checking each claim against `main` HEAD
> (`091c61d`). Flag confirmed, corrected, and falsified items. Update
> the opportunity assessment in light of `feature/v1-runtime/discover/`,
> which the prior pass had not read.
>
> Scope: read-only audit. No code edits, no commits. Filed as input alongside
> [`north-star-adjacent-ideation.md`](north-star-adjacent-ideation.md) and
> [`open-pincery-source-audit.md`](open-pincery-source-audit.md).
>
> Branch checked: `main` at `091c61d` (`docs(readme): v9.0 ship status update`).
> Method: `git log`, `grep`, file reads against the actual tree.

---

## 1. Verdict Table — Prior Claims vs. Reality

| Prior claim                                                                         | Verdict                                                                                                                                            | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ----------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| PR #4 merged into `main`                                                            | ✅ **Confirmed**                                                                                                                                   | `git log` shows `23d8d0b Merge pull request #4 from RCSnyder/v6-01_implementation`. README rewritten (`091c61d`) with "v9.0 shipped" banner.                                                                                                                                                                                                                                                                                      |
| PR #4 = 238 commits                                                                 | ⚠️ **Corrected — 256 commits**                                                                                                                     | `git rev-list --count 23d8d0b ^091c61d~2` → 256. Off by ~7%.                                                                                                                                                                                                                                                                                                                                                                      |
| TLA+ spec ~4,600 lines                                                              | ⚠️ **Corrected — 4,751 lines across two files**                                                                                                    | `OpenPinceryAgent.tla` 2,506 + `OpenPinceryCanonical.tla` 2,245 = 4,751.                                                                                                                                                                                                                                                                                                                                                          |
| AC-78 SHA-256 hash chain shipped                                                    | ✅ **Confirmed**                                                                                                                                   | [src/background/audit_chain.rs](src/background/audit_chain.rs) implements `compute_entry_hash`, `prev_hash` walk, tamper detection. Migration `20260501000001_add_event_hash_chain.sql` present.                                                                                                                                                                                                                                  |
| AC-80 capability nonces shipped                                                     | ✅ **Confirmed**                                                                                                                                   | Migration `20260501000003_create_capability_nonces.sql` with `UNIQUE (workspace_id, nonce)`. Last commit `d635698 docs(verify): AC-80 closed`.                                                                                                                                                                                                                                                                                    |
| AC-77 default-deny seccomp allowlist + SIGSYS                                       | ✅ **Confirmed**                                                                                                                                   | [src/observability/seccomp_audit.rs](src/observability/seccomp_audit.rs) parses `AUDIT_SECCOMP` records; ties into AC-88 audit-netlink unified pass.                                                                                                                                                                                                                                                                              |
| Landlock ABI ≥ 6, audit-netlink AC-88                                               | ✅ **Confirmed (newer than my notes — AC-88 is real)**                                                                                             | [src/observability/landlock_audit_netlink.rs](src/observability/landlock_audit_netlink.rs) parses `LANDLOCK_DENIED` records.                                                                                                                                                                                                                                                                                                      |
| AC-82 fine-grained 10-state lifecycle with CAS-only transitions                     | ✅ **Confirmed**                                                                                                                                   | Commits `ac2d00e` … `56c8209` ship `_G7a` … `_G7g` slices. `lifecycle_transition` events emitted at every CAS. `Inv_TerminalSuccession` lint added (`d7dad7c`).                                                                                                                                                                                                                                                                   |
| AC-81 spec-coverage manifest + commit-msg hook                                      | ✅ **Confirmed**                                                                                                                                   | `e6364f6 feat(build): AC-81 binding commitments — spec_coverage table + commit-msg hook + lint`.                                                                                                                                                                                                                                                                                                                                  |
| Marketing/code drift on `main` ("six layers" claimed but only HMAC real)            | 🟡 **Partly corrected — drift is now smaller but not zero**                                                                                        | README still leads the **Security Model** section with a six-numbered-layers list (`Zerobox` / `OneCLI` / prompt-injection / `Greywall` / DB / webhook). The numbered layers list is unchanged from the pre-merge README. The _status banner above it_ has been rewritten. Net: the banner is now true; the six-layer list still describes the design vocabulary, not the runtime topology actually compiled into `main`. See §3. |
| LLM replay-from-cache punted                                                        | ✅ **Confirmed still absent**                                                                                                                      | Only one match for `llm_calls`: an `INSERT` in [src/models/llm_call.rs](src/models/llm_call.rs#L64). No `prefer_cache`, no replay path, no `SELECT … FROM llm_calls WHERE prompt_hash = …` consumer. The `llm_calls` table is write-only telemetry.                                                                                                                                                                               |
| MCP not shipped                                                                     | ✅ **Confirmed still absent**                                                                                                                      | One match repo-wide for `mcp` in [src/api/openapi.rs](src/api/openapi.rs#L52) — and it's an aspirational doc-comment ("the same schema drives `pcy` CLI generation **and the MCP tool bridge**"). No server, no stdio transport, no tool registration.                                                                                                                                                                            |
| Reference pincers (mailroom / brain / lease) not shipped                            | ✅ **Confirmed still absent**                                                                                                                      | Zero hits for `mailroom` repo-wide.                                                                                                                                                                                                                                                                                                                                                                                               |
| `pcy doctor / backup / restore / upgrade` operator surface not shipped              | ✅ **Confirmed still absent**                                                                                                                      | [src/cli/commands/](src/cli/commands/) contains: `agent`, `audit`, `budget`, `completion`, `credential`, `demo`, `events`, `login`, `message`, `status`, `whoami`. No `doctor`, `backup`, `restore`, `upgrade`.                                                                                                                                                                                                                   |
| Multi-tenant scaffolding (`workspace_id`) sits underneath "single-operator" framing | ✅ **Confirmed**                                                                                                                                   | `workspace_id` is a hard FK in `agents`, `credentials`, `memberships`, `capability_nonces`, `events`. Cross-workspace isolation matrix (AC-65) is in scope. The single-operator framing is a positioning choice layered on top of an honestly multi-tenant schema.                                                                                                                                                                |
| 70 test files                                                                       | ✅ **Confirmed** (note: `DELIVERY.md` says "321 passing tests" — those are individual `#[test]` functions, not files; both numbers are consistent) | `ls tests/*.rs \| wc -l` → 70.                                                                                                                                                                                                                                                                                                                                                                                                    |
| 23 migrations                                                                       | ✅ **Confirmed**                                                                                                                                   | `ls migrations/ \| wc -l` → 23.                                                                                                                                                                                                                                                                                                                                                                                                   |
| `DELIVERY.md` updated to v9.0                                                       | ⚠️ **Half-true**                                                                                                                                   | Top heading still reads `# DELIVERY.md — Open Pincery v8.0`. Body has a v9 progress trail and the "v9.0 ship gate now CLEAR" entry, but the document title is stale. Minor, but it is exactly the kind of drift §3 is about.                                                                                                                                                                                                      |

**Headline correction**: the prior pass treated the "merge discipline gap"
as the project's #1 risk. **That risk is now resolved.** PR #4 landed.
The audit must move on.

---

## 2. New Finding (was missed in the prior pass)

The folder [`feature/v1-runtime/discover/`](feature/v1-runtime/discover/)
contains a complete, evidence-based DISCOVER wave run by the user against
their own situation. The prior pass listed these files as "unread" and
recommended reading them before further positioning work. Reading them now
**materially changes the opportunity memo**.

### What the discovery actually validated

From [`wave-decisions.md`](feature/v1-runtime/discover/wave-decisions.md)
(Revision 2, 2026-05-07) and
[`problem-validation.md`](feature/v1-runtime/discover/problem-validation.md):

- **`[VA0]`** "The agent platform exists and is shipped." Open Pincery itself
  is the validated substrate. **No further substrate-positioning work is
  load-bearing for v1.**
- **`[VA1]`** LLM cost pain is real, recurring, dollar-quantified ($9 single
  query, daily/weekly GHCP rate-limit hits, 27× Opus 4.7 price hike).
- **`[VA3]`** Async/batch is the actual mode. Cold start is irrelevant.
- **`[IA0]`** "This project requires a new agent platform / runtime / vault /
  sandbox stack" — **invalidated**. The user explicitly classifies that as
  the founder-trap pattern of recapitulating prior work to escape scope
  friction.
- **`[IA4]`** "There is a customer segment beyond the founder ready to be
  served" — **invalidated for v1**. Zero named individuals. Market-of-one
  is the explicitly accepted scope.

### What the validated v1 actually is

A **GPU-lease subsystem of open-pincery** (not a new platform):

- `rar lease <gpu-class> --budget=<usd> --duration=<minutes>` → provisions
  a vLLM endpoint on a SkyPilot-managed spot GPU, prints `LLM_API_BASE_URL`
  - teardown handle.
- `rar release <handle>` and `rar status <handle>`.
- Open coding-class model (e.g. Qwen3-Coder-480B on H200) so an open-pincery
  workspace can run wake cycles overnight without paying frontier-API token
  prices.
- Hard caps: **1,500 LOC**, **2 weeks wall-clock**, **$200 GPU spend**.
  If any cap is hit, _stop and reassess_ — explicit corrective for the
  AC-inflation pattern that produced v6→v9 (AC-37 → AC-88+ in three weeks).
- Lives in a separate repo (`remote-agent-assistent`), explicitly **not**
  built via lights-out-swe (`[D9]`, `[C8]`).
- Canonical benchmark per `[D3]`: a real `pcy` agent runs a wake cycle
  against a leased vLLM pod and produces non-trivial output (e.g. a
  code-review wake on the open-pincery repo itself).

### Implication for the prior opportunity memo

The prior memo's #1 ranked market — "substrate for the Tan stack" — is
**not** what discovery validated. Discovery's market is "**a power supply for
my own existing platform.**" The Tan-stack market may still be real later,
but it is not v1 and there is zero past behavior backing it.

The prior memo also pushed for: reference pincers, MCP, README rewrite,
`pcy doctor`. Discovery's `[IA0]` and `[C1]` say the wedge is **not** more
features inside open-pincery; it is _running open-pincery cheaper_.

This is the most important correction in this audit. Not because the prior
recommendations were bad ideas in the abstract, but because they were
ranked above an item with explicit user evidence, and they would
re-trigger the AC-inflation failure mode that the user has already
diagnosed in writing.

---

## 3. Re-scored Deep Intelligence Audit

| Dimension               | Prior score | New score           | Reason for delta                                                                                                                                                                                                                                                                                                                         |
| ----------------------- | ----------- | ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Concept creation        | 4/5         | **4/5** (unchanged) | "Pincer / substrate / single-operator agent OS" still novel + appropriate. The OS framing is _aspirational_ relative to actual runtime topology, but the underlying concept structure stands.                                                                                                                                            |
| Relational transfer     | 4/5         | **4/5** (unchanged) | Postgres-as-kernel + actor-as-LLM-process mapping unchanged.                                                                                                                                                                                                                                                                             |
| Generative power        | 4/5         | **3/5** ↓           | Downgraded. The discovery found the user's own next move is **not** any of the items the substrate concept generates (reference pincers, MCP, op surface). When a concept's generated agenda diverges from the validated agenda, generative power for _this user's situation_ is lower than absolute generative power.                   |
| Expertise depth         | 4/5         | **5/5** ↑           | Upgraded. The discovery itself — explicit `[IA0]` invalidation, hard LOC/time/dollar caps as a corrective, refusing to use the harness on a tightly-bounded internal tool — is principal-engineer-grade self-diagnosis. The team is classifying its own past behavior by deep principle (founder-trap pattern), not by surface symptoms. |
| Design intelligence     | 5/5         | **5/5** (unchanged) | AC-83…88 architecture rework + AC-88 audit-netlink unified pass + the explicit decision to _not_ build the GPU lease via lights-out-swe are all top-tier design judgment about means/ends fit.                                                                                                                                           |
| Opportunity recognition | 3/5         | **4/5** ↑           | Upgraded. The market-of-one scope, dollar-quantified pain, validated workload, and explicit kill criteria meet Shane/Venkataraman + Sarasvathy bars. The reason it isn't 5/5: there is still no plan for whether/how this generalizes beyond the founder, and `[IA4]` flags that as out-of-scope rather than answered.                   |

**Composite**: 22/30 → **25/30**. The single biggest move is from "great
kernel, no users" worry to "great kernel, founder-customer with a
specific cheaper-substrate problem and a 14-day affordable-loss test."

---

## 4. Corrected Tensions

The prior memo listed five tensions. Updated state:

1. **Single-operator vs. multi-tenant scaffolding.** ✅ **Still real.** AC-65
   ships cross-workspace isolation matrix; `workspace_id` is everywhere.
   Discovery `[C2]` says "market-of-one is fine for this scope" — i.e. the
   user has implicitly chosen single-operator-now. Recommend: defer the
   public AC-65 marketing until there's a multi-tenant customer; keep the
   schema honest internally.

2. **Marketing/code drift.** 🟡 **Reduced but not eliminated.** README's six-layer
   security section is still vocabulary-of-the-design, not topology-of-the-binary.
   The "v9.0 shipped" banner above it is true. Net: a careful reader can
   reconcile, a casual reader can't. `DELIVERY.md` heading is still "v8.0".
   Cheap fixes; not blocking.

3. **LLM replay-from-cache punted.** ✅ **Still real.** Code confirms `llm_calls`
   is INSERT-only. The "Orleans-grade correctness" framing in the lineage
   doc cannot honestly be claimed until this lands. Discovery `[D8]`
   defers plan/execute split, but doesn't speak to replay-from-cache; this
   item is orthogonal to the GPU-lease wedge and could ship cheaply
   without violating discovery's scope discipline.

4. **Harness producing ACs faster than ship.** ✅ **The user has now diagnosed
   this in writing.** Discovery `[D9]`, `[C1]`, `[C3]`, `[C8]`, and the
   "scope-discipline tripwires" list are exactly the corrective. The
   diagnosis is sharper than mine was. Treat this as resolved-by-policy.

5. **No external user observed.** ✅ **Resolved by re-scoping, not by finding
   users.** Discovery `[IA4]` retires this concern by accepting market-of-one.
   The risk reappears the moment someone re-positions for a second user.

---

## 5. Concept-Card Updates

### Updated card: "Single-Operator Agent OS"

- **Status after audit**: Aspirational positioning, not topology of `main`.
  The actual shipped artifact is "Postgres-backed continuous-agent runtime
  with workspace-scoped tenancy and a hard kernel sandbox floor." That is
  a less marketable but more honest definition.
- **Failure mode** (unchanged): claiming the OS contract (deploy / supervise
  / maintain / provenance / governance / security / continuity) when only
  4 of those 7 are demonstrably implemented in `main`. Specifically
  missing as runtime surfaces: `pcy doctor`, `pcy backup`, `pcy restore`,
  `pcy upgrade`, MCP bridge.
- **Test**: a fresh operator can complete `install → run agent → backup →
restore → upgrade → audit-verify` from CLI alone. Currently 2 of 5
  steps work end-to-end (`install`, `run agent`).

### New card: "Founder-Trap Pattern" (lifted from `wave-decisions.md`)

- **Definition**: starting a new repo / scaffolding / TLA+ spec / AC empire
  to escape the scope-friction of an existing project, when the scope
  friction is itself a signal that prior work has already solved the load-
  bearing problem.
- **Mechanism**: harness rewards visible AC closure → AC inflation
  outpaces shippable surface → friction accumulates → next ambitious
  thought spawns a new repo to escape that friction → previous repo's
  unshipped surface piles up → repeat.
- **Detection signals**: new repo whose stated purpose duplicates a
  shipped capability of an existing repo; AC count growing faster than
  user-visible surface; new TLA+ spec for a sub-feature instead of an
  invariant addition to the existing one.
- **Corrective** (per `[C3]`): hard LOC + wall-clock + dollar caps with a
  "stop and reassess" tripwire, not "expand scope".
- **Why this concept is load-bearing**: it is the same pattern Simon
  describes as designers solving the wrong well-defined problem because
  the right ill-defined one is harder to frame. Naming it makes it
  detectable.

---

## 6. Revised Next-Move Ranking

The prior pass ranked: (1) merge PR #4, (2) README claim audit, (3) reference
pincer, (4) MCP, (5) replay cache, (6) decide single-operator framing,
(7) read discovery folder.

Re-ranked, post-audit:

1. **Ship the GPU-lease subsystem** per discovery `[D11]`. 14-day
   affordable-loss bound. Validated pain, validated user, validated
   benchmark. _This is the only item with explicit user evidence._
2. **Trim README's six-layer list to match `main`-shipped topology.** 1-day
   fix. Eliminates the residual marketing/code drift. While there: rename
   `DELIVERY.md` heading from v8.0 to v9.0.
3. **LLM replay-from-cache.** 1–2 days. Cheap, cleanly orthogonal to the
   GPU-lease wedge, materially upgrades the correctness story for any
   future external user, and a leased open-model endpoint is exactly the
   case where deterministic replay matters most (slow + sometimes flaky).
4. **Defer (explicitly):** reference pincers, MCP, `pcy doctor/backup/
restore/upgrade`, public single-operator-vs-multi-tenant decision,
   AC-89+. Each of these would be valuable in absolute terms; each would
   re-trigger the founder-trap pattern relative to the validated v1.

Items 1–3 together form a coherent next quarter without re-entering the
AC-inflation regime.

---

## 7. What This Audit Got Wrong (post-mortem)

The prior pass's biggest miss was failing to read `feature/v1-runtime/
discover/` before producing an opportunity memo. The opportunity-recognition
score (3/5) was correct as an absolute, but the _direction_ of the memo was
misaligned with already-validated user evidence sitting two folders over.

Lesson for future audits of this kind: **inventory all `discover/` artifacts
before any opportunity ranking**. Discovery artifacts pre-empt any
analytical opportunity claim, because they contain past behavior and the
analytical claim contains only inference from artifacts.

---

## 8. One-Line Updated Verdict

**The kernel is real and on `main`. The merge-discipline risk is resolved.
The next correctly-scoped move is not more substrate — it is shipping the
GPU-lease subsystem the user already validated, then ringing the
README/replay-cache cleanup items that are cheap and honest, and
explicitly _not_ expanding open-pincery's surface until there is external
evidence that something inside it is the bottleneck.**
