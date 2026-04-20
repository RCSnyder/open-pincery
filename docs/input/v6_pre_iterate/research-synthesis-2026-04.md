# Research Synthesis — 2026-04-20

**Status:** Thinking record for v6 EXPAND input. Distills three external research sources into the load-bearing insights that affect Open Pincery's strategy. Sources live in `docs/input/v6_pre_iterate/research/` only while this note is being written; once this synthesis is committed, the source files can be removed from the repo and cited by URL/DOI instead.

**Companion note:** `agent-taxonomy-2026-04.md` (Category 5 / Continuous Agents claim). This note is the deeper substrate that supports, corrects, and extends it.

---

## Sources

| ID   | Source                                                                                                                                                                                                                                   | Role                                                                                 |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `S1` | Nisa, Shirazi, Saip, Pozi. _Agentic AI: The age of reasoning — A review._ Journal of Automation and Intelligence 5 (2026) 69–89. DOI: [10.1016/j.jai.2025.08.003](https://doi.org/10.1016/j.jai.2025.08.003). Open access (CC BY-NC-ND). | Cognitive taxonomy of 7 agent types; capability progression; challenges + future.    |
| `S2` | "Agentic AI Frameworks" survey (CrewAI, LangGraph, AutoGen, Semantic Kernel, Agno, Google ADK, MetaGPT). arXiv:2508.10146. Identified communication protocols: MCP, A2A, ANP, ACP, Agora.                                                | Comparative analysis of existing agent frameworks; service-computing readiness.      |
| `S3` | _Agentic Information Systems (IS)._ Electronic Markets / Springer, [DOI link](https://link.springer.com/article/10.1007/s12525-025-00861-0). Open access (CC BY 4.0).                                                                    | IS-paradigm framing: archetypes, delegation patterns, accountability, metaknowledge. |
| `S4` | Nate's Newsletter, ["There Are 4 Kinds of Agents (And You're Probably Using the Wrong One)"](https://natesnewsletter.substack.com/p/there-are-4-kinds-of-agents-and-youre).                                                              | Deployment-shape taxonomy used in `agent-taxonomy-2026-04.md`.                       |

Footnoted inline as `[S1]`, `[S2]`, `[S3]`, `[S4]`. No source text is reproduced; only distilled claims with attribution.

---

## 1. Two taxonomies, different axes — both apply to OP

Two mutually compatible classifications need to coexist in our heads:

- **Cognitive taxonomy** `[S1 §3.2]`: 7 types on a capability ladder — reactive → proactive → limited-memory → model-based → goal-driven → theory-of-mind → self-aware. Describes _what the agent can do internally_.
- **Deployment taxonomy** `[S4]`: 4 architectures — coding harness / dark factory / auto-research loop / orchestration framework. Describes _what shape the system takes in use_.

Neither alone captures OP. Open Pincery is **Cognitive types 3–5** (limited-memory + model-based + goal-driven, aspirationally type 6 / theory-of-mind-lite, explicitly not type 7) running on a **Deployment category that isn't in the four** — the fifth category we've named Continuous / Resident Agents.

Implication for v6: don't pick one taxonomy; state both explicitly to preempt the reviewer who'll accuse the Category 5 claim of confusing levels.

## 2. Memory as the defining substrate

Both the academic review `[S1 §3.3.2]` and the frameworks survey `[S2 §IV.B]` identify **memory** as the under-solved differentiator. Key load-bearing points:

- `[S1]` names three memory kinds: **parametric** (model weights), **working** (context window), **external** (retrieval / DB). Current systems fail at coordinating them; hallucinations, knowledge cutoffs, and context-window limits are symptoms of that gap.
- `[S1]` explicitly calls for _"centralized memory controllers that dynamically coordinate between parametric, working, and external memory"_ and for _"agent-level memory policies"_ (learning what to store, when to retrieve, how to apply).
- `[S2]` categorizes framework memory as short-term vs long-term, with further subtypes: semantic, procedural, episodic. Observes that CrewAI/LangGraph/AutoGen/Google-ADK all "implement memory in various ways depending on target use case" — i.e., there is no standard.
- `[S2]` identifies this inconsistency as an interoperability barrier.

**Implication for OP:** the event log + projections is exactly the "centralized memory controller" shape the literature is asking for. This should be named explicitly as a **Durable Bet** in north-star — probably _"Memory-as-substrate: event log + projections are the differentiating primitive, not the reasoner."_ Missing from the current north-star.

## 3. The IS paradigm gives OP a philosophical spine (and three mandatory concepts)

`[S3]` reframes the entire problem as **Agentic Information Systems** — a paradigm shift from passive-tool IS to active-agent IS. This gives OP three concepts it implicitly needs but has not named:

### 3.1 Three archetypes of agentic IS `[S3]`

| Archetype         | Agency balance                      | OP fit                                                     |
| ----------------- | ----------------------------------- | ---------------------------------------------------------- |
| **Assisting**     | Humans primary; IS reactive         | Classic SaaS. Not OP.                                      |
| **Autonomous**    | IS primary; humans lower agency     | Self-driving cars. Not OP Year 1.                          |
| **Collaborative** | Both high-agency, shared objectives | **This is OP.** Tier 1 catalog = collaborative agentic IS. |
| **Hybrid**        | Merged; no clear agency boundary    | Brain-computer interfaces. Not OP.                         |

**Implication:** OP should claim "Collaborative Agentic IS" in addition to Continuous / Category 5. They are compatible: _deployment shape = Continuous; IS archetype = Collaborative_.

### 3.2 Three delegation patterns `[S3]` — the missing strategic axis

| Pattern                        | Who initiates       | Example                                        |
| ------------------------------ | ------------------- | ---------------------------------------------- |
| **User-invoked**               | Human → IS          | Inbox triage (founder assigns)                 |
| **Bidirectional**              | Either; transitions | Codebase steward (wakes founder for PR review) |
| **IS-invoked** (weak / strong) | IS → human          | Weekly digest suggests priority tasks          |

The Tier 1 catalog implicitly mixes all three but never names them. This matters because:

- Bidirectional delegation is where OP differentiates from Claude Projects (pure user-invoked).
- IS-invoked delegation is the hardest to get right and the most regulated (EU AI Act concerns).
- Each pattern needs **different acceptance-contract shapes** and **different guardrails**.

**Implication for v6:** add delegation direction as a required field on every mission type. A separate companion note (`delegation-patterns-2026-04.md`) should classify each Tier 1 mission.

### 3.3 Accountability is a hard constraint, not a nice-to-have

`[S3]` is unambiguous: regulations like the EU AI Act **prohibit transferring legal accountability to IS artifacts**. Human accountability must be preserved even when IS is operationally autonomous. This creates a mandatory structural concept:

> Every mission instance MUST have a named accountable human. That identity is part of the mission record, not the auth layer.

OP currently has this implicitly (workspace membership), but it is not elevated to a first-class design concept. This is a **v6 must**, not a deferred.

### 3.4 Metaknowledge is the named hole

`[S3]` (and `[S1 §3.3.3]` via metacognition) both flag metaknowledge — _"accurately assessing one's own capabilities and those of collaborative partners"_ — as the gap that undermines delegation. Humans tend to over- or under-trust IS outputs because they lack metaknowledge; IS-invoked delegation can actually delegate better than humans, but that reduces control.

OP's acceptance contracts cover _what missions are supposed to do_. They do not currently cover _how the agent signals "out of depth."_ Year-one OP should not attempt full metacognition (that's Nisa type 7), but should name a bridge primitive — e.g., _confidence-below-threshold → bounce to human_.

**Implication:** name metaknowledge as a Deferred with an explicit year-one bridge primitive, not a silent gap.

## 4. The frameworks landscape — stop competing in the wrong league

`[S2]` surveys CrewAI, LangGraph, AutoGen, Semantic Kernel, Agno, Google ADK, MetaGPT and catalogs their failure modes:

- **Rigid architectures**: static roles, cannot adapt mid-task (MetaGPT, CrewAI).
- **No runtime discovery**: agents cannot find peers at runtime; all collaborations statically defined.
- **Code safety gaps**: generated code executes without sandbox (MetaGPT, AutoGen).
- **Interoperability gaps**: each framework's task/agent/tool model is incompatible with others' — cannot invoke a CrewAI coder from a LangGraph planner without translation.
- **Service-computing immaturity** `[S2 §IV.E]`: no framework natively supports dynamic discovery, composition, and orchestration in the W3C/WS-\* sense. Semantic Kernel and Google ADK come closest; all others require external registries.
- **Protocol fragmentation**: MCP, A2A, ANP, ACP, Agora all exist; none is dominant. Most use HTTP transport but incompatible semantics.

**Critical implication for OP's positioning:** these are **libraries**, not **runtimes**. The appropriate peer set for comparison is:

| Peer                          | Category                           | OP differentiator                     |
| ----------------------------- | ---------------------------------- | ------------------------------------- |
| Zapier Agents, Lindy          | Hosted runtime                     | Sovereignty; event log audit          |
| AWS Bedrock Agents            | Cloud runtime tied to hyperscaler  | Open-weight option; self-host         |
| LangGraph Cloud               | Managed framework                  | Runtime semantics + mission primitive |
| Vercel AI / Cloudflare Agents | Edge runtime                       | Stateful / long-running               |
| Cursor Background Agents      | Coding-focused background agent    | Mission catalog beyond coding         |
| Claude Projects               | Embedded agent inside host product | Sovereign substrate you own           |

**Strategic answer D9's competitive matrix must be rebuilt with this peer set.** Comparing OP to CrewAI is a category error that makes OP look bigger than it is and invites the wrong benchmark.

## 5. Capability progression and what OP can honestly claim

`[S1 §3.3]` enumerates six capability dimensions and describes the progression across the 7 cognitive types:

| Dimension           | What OP actually has today                                  | Honest type ceiling           |
| ------------------- | ----------------------------------------------------------- | ----------------------------- |
| Perception          | LLM-driven + tool-based; no grounded sensor fusion          | Type 4 (model-based)          |
| Memory              | Parametric + working + external (event log)                 | Type 4 approaching type 5     |
| Reasoning           | Chain-of-thought + tool use; no neuro-symbolic verification | Type 5 (goal-driven)          |
| Learning            | In-context only; no continual learning                      | Type 3 (limited-memory)       |
| Autonomy            | User-invoked today; bidirectional latent in Tier 1          | Type 5 with type 6 aspiration |
| Social intelligence | Natural-language interaction; no real ToM                   | Type 4                        |

**Implication for positioning:** OP today is a **type-4-to-5 agent substrate** with a **type-6 aspiration** and an **explicit rejection of type 7**. This is defensible, honest, and closes the "can it do anything?" ambiguity with a bounded answer.

## 6. Evaluation is open — and N=1 is not enough

`[S1 §4.2.6–4.2.10]` is scathing about agent benchmarking:

- No standardized benchmarks; teams hand-roll and hand-score.
- Data contamination in LLM training corrupts results.
- Benchmarks test single-step reasoning, missing multi-step planning and tool use.
- Real-world relevance of benchmarks (WildBench, SWE-bench) remains unclear.
- Bias inherited from underlying LLMs is under-measured.

The 90-day founder-operated benchmark we proposed is useful as a **smoke test** but is **N=1 with maximum selection bias** (the founder is the builder). For the "professional software" claim to survive external scrutiny year-two must demonstrate:

- ≥ 3 non-founder operators running Tier 1 missions continuously for ≥ 30 days;
- per-AC pass rate on a public dashboard;
- mission-type-specific acceptance-contract success rate;
- an acknowledged sample of failures with root-cause narration.

**Implication:** add a tripwire — _"Year two starts with fewer than 3 non-founder operators running Tier 1."_

## 7. Interoperability as a deferred strategic axis

`[S2 §V`] names interoperability gaps as a defining problem of the current era. OP today speaks **MCP outward** (consume tools). It has no story for **MCP / A2A / ACP inward** — another agent invoking an OP mission as a service. Year-three work, but year-one decisions (especially auth model + mission identity) must not preclude it.

**Implication:** name this in the design doc as a _forward-compatibility constraint_, not as a built feature.

## 8. Hard-earned warnings worth inheriting verbatim

Claims from `[S1 §4.2–§4.3]` and `[S3 Challenges]` that OP should either adopt as constraints or rebut as non-goals:

- **Value alignment is unsolved for dynamic human objectives** `[S1]`. OP's response: acceptance contracts, not value alignment.
- **RLHF does not scale for long-horizon tasks** `[S1]`. OP's response: no RLHF-based learning; rely on mission-type catalog discipline.
- **Distribution shift between training and deployment reduces performance** `[S1]`. OP's response: bounded mission types + active reconcile / verify gates catch drift.
- **Humans' metaknowledge of agent capabilities is unreliable** `[S3]`. OP's response: confidence-threshold bridge primitive (see §3.4).
- **Skill erosion from delegation diminishes oversight capacity** `[S3]`. OP's response: founder operates manually for 90 days before delegating; re-operates quarterly to refresh metaknowledge.
- **Trust overshoot produces irresponsible collaboration** `[S3]`. OP's response: every mission surfaces its own evidence; no black-box "trust me."
- **Regulatory frameworks prohibit legal accountability transfer to IS** `[S3]`. OP's response: named accountable human per mission (§3.3).

## 9. What this means for v6 EXPAND — concrete carry-ins

These should propagate into v6 scope work, **not** back-edited into the strategy docs:

1. **Memory-as-substrate as Durable Bet 10** in north-star (§2).
2. **Dual-taxonomy positioning** in north-star opening paragraph (§1): _Continuous deployment × Collaborative IS archetype × Cognitive types 3–5_.
3. **Delegation direction as a required field** on every mission type (§3.2). Companion note `delegation-patterns-2026-04.md` to draft year-one classification.
4. **Accountable-human as a first-class mission concept** (§3.3). Affects auth model + mission record shape.
5. **Metaknowledge bridge primitive** as a named Deferred with year-one confidence-threshold shim (§3.4).
6. **D9 competitive matrix rebuild** against runtimes, not libraries (§4).
7. **Honest capability ceiling** — type-4-to-5 today, type-6 aspirational, type-7 rejected (§5). Goes into non-goals.
8. **Public operator dashboard as year-two gate** (§6). Add to tripwires.
9. **Interoperability-inward forward-compatibility constraint** on v6 design (§7).
10. **Warnings §8 folded into design Open Questions** as named responses, not hand-waved.

## 10. Convergence judgment

With these adjustments folded into v6 EXPAND, the strategy converges on a coherent, defensible product: _a sovereign substrate for Collaborative Continuous Agents running bounded mission types with named accountable humans, evaluated against public per-AC metrics, differentiated on memory-as-primitive and sovereignty-by-default._

Without them, v6 ships acceptance contracts but leaves accountability, delegation direction, and metaknowledge as implicit assumptions — which is exactly how `[S1]` and `[S3]` predict these systems fail in regulated and bidirectional-delegation settings.

---

## Citations used in this note

- `[S1]` Nisa et al. 2026. _Agentic AI: The age of reasoning — A review._ J. Automation & Intelligence 5. DOI [10.1016/j.jai.2025.08.003](https://doi.org/10.1016/j.jai.2025.08.003). CC BY-NC-ND.
- `[S2]` Agentic AI Frameworks survey. arXiv:2508.10146.
- `[S3]` Agentic Information Systems. Electronic Markets / Springer. [Link](https://link.springer.com/article/10.1007/s12525-025-00861-0). CC BY 4.0.
- `[S4]` Nate's Newsletter. [There Are 4 Kinds of Agents](https://natesnewsletter.substack.com/p/there-are-4-kinds-of-agents-and-youre).
