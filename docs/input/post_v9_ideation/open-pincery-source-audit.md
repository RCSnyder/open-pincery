# open-pincery source audit (subagent-reported)

> **Provenance**: This document was produced by a subagent that fetched and read source files from the `main` branch of https://github.com/RCSnyder/open-pincery on May 7, 2026. The findings below are the subagent's report, not independently verified by re-reading the source in this session. Treat all quoted SQL/paths as second-hand until spot-checked.

> **CRITICAL CORRECTION (2026-05-07 evening)**: This audit was performed against `main` (which is v5 / v1.0.1). It did **not** sample the `v6-01_implementation` branch (PR #4), which contains 238 commits of v6→v9 work and explicitly delivers AC-53 (bubblewrap `RealSandbox`), AC-77 (seccomp default-deny + clone arg-filter), AC-78 (event-log SHA-256 hash chain + startup gate), AC-79 (`wake_system_prompt` v3 + canary + jsonschema validation + per-wake rate limit), AC-80 (single-use 60s-TTL capability nonces), AC-38–43 (AES-256-GCM credential vault + REST API + CLI + PLACEHOLDER resolution), and Landlock production-enforcement hardening. Therefore the §2 verdict-table rows marked "❌ NOT IMPLEMENTED" for sandbox / OneCLI vault / prompt-injection defense / capability gating / hash-chain audit are **wrong against the live development branch** and correct **only against the publicly-tagged v1.0.1 release on `main`**. The README is _ahead_ of `main` but only modestly _ahead_ of `v6-01_implementation` once that PR lands.

> **What still stands**: claims about the wake loop CAS, NOTIFY/LISTEN wiring, event-log + projections, llm_calls cost capture, agent-as-prose-projections authoring model, and absence of replay-from-cache are derived from code that exists on `main` and is unlikely to have changed in PR #4 (no AC in the PR addresses LLM replay or pincer authoring boundary). Postgres RLS appears to remain unimplemented even on the PR branch.

> **Action**: re-run a source-level audit against `v6-01_implementation` HEAD (`d635698`) before treating any negative finding as definitive.

---

## 1. What the code actually is

### Confirmed architecture

- **Wake loop**: real CAS, real NOTIFY/LISTEN. The atomic claim is:
  ```sql
  UPDATE agents
  SET status = 'awake'
  WHERE id = $1 AND status = 'asleep' AND is_enabled = TRUE
  RETURNING *
  ```
  This is the lifecycle correctness story. It's a single SQL statement; no distributed coordination. Fine. Defensible.
- **Episode boundary**: bounded by one of {sleep signal, completed, iteration_cap, llm_error}. So episodes are bounded by _runtime policy_, not by the LLM's "I'm done" alone. Good.
- **Event log**: append-only, with `events` and `llm_calls` tables among the 16 migrations. Replay-able in principle.
- **NOTIFY/LISTEN wiring**: webhook → event row → `pg_notify` → in-process listener → CAS attempt → wake_loop → maintenance. End-to-end traced.
- **Pincer authoring is conversation-driven**: pincer behavior lives in `agent_projections` TEXT fields, versioned in the database. **A user can define a new pincer without recompiling the Rust binary.** This is a bigger deal than the README conveys — see §3.
- **Cost tracking is real**: `llm_calls` row per call, with cost. So the "cost-aware durable execution" angle from the ideation doc is _closer than I thought_ — the data is there; what's missing is the budget-cap-and-shutdown primitive.
- **Tests**: 25+ test files exercising lifecycle, wake_loop, maintenance, events, webhooks, budget. Test coverage is real.
- **TLA+ spec**: ~4600 lines. Core wake/sleep/CAS state machine matches implementation. Approval gates, MCP, vault, RLS appear in the spec but **not in the code**.

### Material gaps the README does not flag

1. **No sandbox at all.** Tool execution is `Command::new("sh").arg("-c")` straight to host. README claims six security layers (zerobox / OneCLI / prompt-injection defense / Greywall / Postgres RLS / HMAC). Subagent reports: HMAC exists; **the other five are absent or aspirational**. This is the biggest gap between marketing and code.
2. **No Postgres RLS.** Subagent reports zero `ROW LEVEL SECURITY` / `POLICY` statements in the migrations. Multi-tenant isolation is app-level filtering only. The four-deployment-mode story (individual / team / SaaS / enterprise) is _not_ enforced at the database layer.
3. **No MCP**, client or server. The ideation doc's "ship MCP" recommendation is a _new_ capability, not a refactor.
4. **No SDK boundary.** Pincer behavior is database-driven (good — means external callers can author pincers via HTTP), but there's no Python/TS client library that wraps the API. A user authoring via curl can do it; a user expecting a `@pincer` decorator can't.
5. **LLM calls are NOT replayed from the event log on recovery.** The subagent reports the LLM is re-invoked on replay (no response caching). So **replay is not deterministic across the LLM boundary** today. This is the single most important strategic finding — see §4.

---

## 2. Verdict table (claim vs. code)

| Claim                                  | Status                          | Evidence                                   |
| -------------------------------------- | ------------------------------- | ------------------------------------------ |
| Durable identity (conversation-driven) | ✅ CONFIRMED                    | `agent_projections` TEXT fields, versioned |
| Event log + projections                | ✅ CONFIRMED                    | `events` + `llm_calls` tables              |
| CAS wake lifecycle                     | ✅ CONFIRMED                    | UPDATE...WHERE status='asleep' RETURNING   |
| NOTIFY/LISTEN wiring                   | ✅ CONFIRMED                    | end-to-end webhook→wake traced             |
| Shell tool executor                    | ✅ CONFIRMED but ⚠️ unsandboxed | `Command::new("sh")` direct                |
| zerobox sandbox                        | ❌ NOT IMPLEMENTED              | absent from source                         |
| OneCLI vault                           | ❌ NOT IMPLEMENTED              | absent from source                         |
| Prompt-injection defense               | ❌ NOT IMPLEMENTED              | absent from source                         |
| Greywall outer sandbox                 | ❌ NOT IMPLEMENTED              | absent from source                         |
| Postgres RLS                           | ❌ NOT IMPLEMENTED              | no POLICY statements in migrations         |
| HMAC webhook auth                      | ✅ CONFIRMED                    | per-agent secret + signature verify        |
| Rate limiting (10/60 per min)          | ✅ CONFIRMED                    | enforced in middleware                     |
| LLM cost tracking                      | ✅ CONFIRMED                    | `llm_calls.cost` populated                 |
| Cost budgets / caps                    | ⚠️ PARTIAL                      | data captured, no enforcement primitive    |
| Approval gates                         | ❌ NOT IMPLEMENTED              | TLA+ spec only                             |
| MCP                                    | ❌ NOT IMPLEMENTED              | absent                                     |
| Multi-language SDK                     | ❌ NOT IMPLEMENTED              | absent                                     |
| TLA+ spec                              | ✅ CONFIRMED                    | ~4600 lines, core matches                  |
| Deterministic replay across LLM        | ❌ NOT IMPLEMENTED              | LLM re-invoked on replay                   |

---

## 3. The biggest _positive_ surprise: pincer-authoring is already database-driven

If pincer behavior lives in `agent_projections` (TEXT, versioned, in Postgres), then **the SDK boundary is essentially "write to those rows."** That changes the priority order in [`docs/north-star-adjacent-ideation.md`](north-star-adjacent-ideation.md):

- "Python SDK" is no longer a refactor against an HTTP API — it's a thin wrapper that POSTs structured prose updates to the projection endpoint.
- Authoring a new pincer is `pcy agent create` + `pcy agent set-identity` + `pcy agent set-tools` (or whatever the API surface is), and these are presumably already HTTP routes.
- **The thing that's missing is not the boundary; it's the ergonomics layer over it.** A 200-line Python library with `@pincer` and `@tool` decorators that POST to `/api/agents/...` would close the gap.

This is _much cheaper_ than a Temporal/Restate-style SDK that has to do durable-execution magic in user-language. Pincers don't need that; the durability is server-side.

---

## 4. The biggest _negative_ surprise: replay is not deterministic across the LLM call

The TLA+ spec implies deterministic state-machine semantics. The implementation re-invokes the LLM on replay rather than replaying from the recorded `llm_calls` row. This means:

- **Recovering from a crash mid-episode produces a different sequence than the original.** The event log tells you what _happened_; it does not let you reconstruct _what would have happened_ deterministically.
- The "Orleans-grade correctness" framing in §6 of the ideation doc **does not hold today**. Orleans's correctness story includes deterministic replay. Open-pincery's does not, yet.
- This is fixable cheaply: when replaying, read `llm_calls` for the corresponding event ID and return the recorded response instead of calling the provider. The data is already captured. **What's missing is the replay code path that prefers cache over re-invocation.**

This is the single highest-leverage correctness improvement available. It's also the natural home for the "deterministic LLM replay" research thread mentioned in the ideation doc — and now we know it's a _small_ implementation change, not a research project.

---

## 5. Strategic implications (revised)

The ideation document's recommendations stand, but priority order shifts:

| Priority | Item                                  | Status given audit                                                                    |
| -------- | ------------------------------------- | ------------------------------------------------------------------------------------- |
| 1        | **Replay-from-llm_calls cache**       | New top priority. Cheap. Closes the deterministic-replay gap.                         |
| 2        | **README honesty pass**               | Drop or qualify the six-layer security story. RLS not shipped.                        |
| 3        | **Python SDK as ergonomics layer**    | Cheaper than I claimed; behavior already lives in DB.                                 |
| 4        | **MCP surface**                       | Still high-leverage. Still one week of work.                                          |
| 5        | **Mailroom-pincer reference example** | Same.                                                                                 |
| 6        | **gbrain integration**                | Same.                                                                                 |
| 7        | **RLS for real multi-tenant**         | Required before "team / SaaS / enterprise" claims are honest.                         |
| 8        | **Sandbox (real, not aspirational)**  | Required before any public hosted offering. Not required for self-hosted single-user. |
| 9        | **Cost-cap enforcement primitive**    | Data exists; primitive doesn't. Real differentiator if shipped.                       |

---

## 6. Three things the code reveals that the README does not

1. **The "agent-as-prose-projections" pattern is the real authoring model**, and it's quietly more interesting than the actor-lineage framing. You don't _write_ a pincer; you _describe_ one in the database, and the runtime keeps it alive. That's closer to "agent-as-config" than "actor-as-code." Worth a paragraph in the README — it's a positioning angle nothing else has.
2. **The security story in the README is significantly ahead of the code.** Five of six claimed security layers are absent. This needs a public correction before it costs trust.
3. **Deterministic replay is one small refactor away.** The data is captured, the boundary is identifiable, the missing piece is the prefer-cache code path. This is a 1–2 day improvement that materially upgrades the correctness pitch.

---

## Caveats

- All findings here are subagent-reported. The subagent listed paths and quoted SQL, but the file was never independently re-read in this session. Spot-check before acting.
- "Subagent reports X is absent" is a stronger negative claim than is warranted from a single pass. Confirm absences by `rg`-style search before publishing the verdict table.
- Test files exist; the subagent named them but did not quote individual assertions deeply. Coverage _quality_ (vs. quantity) is unverified.
