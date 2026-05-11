# Lean Canvas — remote-agent-assistant (v1-runtime)

> Status: DISCOVER wave output. Single-stakeholder canvas (founder-as-customer). Updated after evidence-based interview, 2026-05-07.

> **Revision 2 (2026-05-07, post open-pincery review)**: Original canvas framed this as a standalone runtime substrate. After surfacing [RCSnyder/open-pincery](https://github.com/RCSnyder/open-pincery), the operative product is an **open-pincery GPU-lease subsystem**, not a separate platform.
>
> **Operative UVP (Revision 2)**: _Lease an ephemeral spot GPU as a transient `LLM_API_BASE_URL` for an open-pincery workspace, in one CLI command, with a hard budget cap and automatic teardown._
>
> **Operative Solution (Revision 2)**: ~500–1500 LOC of glue code (Rust or Python) wrapping SkyPilot + vLLM. Three CLI commands: `lease`, `status`, `release`. SkyPilot YAML templates for one (model, GPU class) combo. README on how to wire into open-pincery's existing `.env` mechanism. **Not built via lights-out-swe** — deliberately low-ceremony to break the AC-inflation pattern.
>
> **Operative Customers (Revision 2)**: market-of-one (founder), and that is fine — this is internal infrastructure for the platform the founder already shipped. External customer-development is not gated by this DISCOVER. See `wave-decisions.md` for the authoritative spec.

---

## 1. Problem

**Top three** (ranked by interview evidence, strongest first):

1. **Existing autonomous SWE harness (lights-out-swe) is bound to GHCP runtime.** The protocol is tool-agnostic on paper but only executes via VS Code agent mode in practice. Author has formally acknowledged this gap in the lights-out-swe README.
2. **Frontier-API token costs are escalating** (Opus 4.7 27× hike, $9 single queries observed) and harness vendors are shifting to token-based pricing — the cost trajectory of running lights-out-swe via GHCP is unsustainable for solo operators.
3. **No off-the-shelf agent runtime executes the lights-out-swe protocol** (`.github/copilot-instructions.md`, `.prompt.md`, `.agent.md`, restricted-tool agents, gated phases) on open models on commodity GPUs. Aider/OpenHands/SWE-agent each assume different protocols.

### Existing alternatives (and why they fail)

| Alternative                                           | Why it fails for this JTBD                                                                          |
| ----------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| GHCP agent mode (status quo)                          | Rate-limited, costly, human-driven (one window per session), ties product evolution to GHCP roadmap |
| Aider / OpenHands / SWE-agent                         | Don't natively execute lights-out-swe gated-phase protocol; would require fork or adapter           |
| SkyPilot + manual SSH                                 | Substrate exists but no harness-protocol layer; user must hand-stitch                               |
| Modal / Cloudflare Project Think / AWS Bedrock Agents | Vertically integrated proprietary stacks; user explicitly rejects vendor lock-in trajectory         |
| Local Ollama / vLLM                                   | No agent loop, no harness protocol, no remote ergonomics                                            |

---

## 2. Customer Segments

- **Early adopter (n=1)**: project owner. Solo developer running lights-out-swe on personal projects, hitting GHCP rate/cost limits, has TLA+ + spec-driven workflow already.
- **Plausible secondary (untested)**: other lights-out-swe users (currently 0 stars on the template repo — market-of-one until proven otherwise).
- **Adjacent (out of scope for v1)**: teams running other cybernetic-loop classes (research agents, data pipelines). Substrate must not preclude these but v1 ships zero adapters for them.

**Mom Test status**: Customer development with anyone _other than_ the founder has not occurred. Treat as personal-tool until external interviews validate broader segment.

---

## 3. Unique Value Proposition

> **Run your lights-out-swe project on rented GPUs, fire-and-forget, at commodity prices — same harness, same provenance, no GHCP.**

**Anti-positioning** (what this is _not_): not a new harness; not a real-time agent; not a Modal/Cloudflare competitor; not a sovereignty pitch.

**Differentiator**: faithful execution of the existing lights-out-swe protocol on open models — a near-empty quadrant. Substrate-only competitors (SkyPilot) lack the protocol; harness competitors (Aider, OpenHands) lack the gated-phase fidelity; hyperscaler agents (Cloudflare/AWS) are vendor-coupled.

---

## 4. Solution

**Generic abstraction**: `Job = { mission, context, harness_protocol, model_spec, budget } → Result = { state, message, artifacts }`

**v1 ships exactly one concrete pack**:

- One protocol pack: `lights-out-swe` (faithfully executes the gated pipeline)
- One substrate adapter: SkyPilot-backed (Vast/RunPod/Prime Intellect) — chosen because it abstracts spot GPU procurement and budget caps
- One model recipe: a single open coding-class model on a single GPU tier (TBD in spike — Qwen3-Coder-480B / DeepSeek-V4 / Llama-4 candidate)
- One agent loop adapter: TBD in SPIKE wave (closest match among Aider / OpenHands / SWE-agent / custom)
- CLI ergonomics: `rar run <project-dir>` → provisions, runs, streams logs, returns artifacts, tears down on completion or budget cap

**Strategic constraint**: substrate stays generic (Job interface, pluggable protocol packs); v1 dogfoods exactly one application. Future loop classes (research, data) plug in by adding a protocol pack — they do not require substrate changes.

---

## 5. Channels

- Eat-your-own-dogfood (founder uses it on personal projects; lights-out-swe template gains a `remote-agent-assistant` integration path)
- lights-out-swe README cross-link once R1 spike passes
- Zero paid acquisition in v1; product is a personal tool until external validation occurs

---

## 6. Revenue Streams

**v1**: none. Personal tool. Zero monetization in scope.

**Future possibilities (out of scope, not committed)**:

- Open-source CLI; managed hosting if other lights-out-swe users emerge
- Sponsored protocol packs for specialized domains
- Not pursued until segment expansion is evidence-validated

---

## 7. Cost Structure

**Build phase (DISCOVER → DELIVER)**:

- Founder time (open-ended commitment per Q3.3)
- GPU spike costs: ~$50–200 for SPIKE wave (Fire Legasy replay benchmarks across 2–3 model/runtime combinations)
- No paid SaaS dependencies required (SkyPilot OSS, vLLM OSS, OSS agent loop, OSS model)

**Run phase (per build, target)**:

- Spot GPU rental: $1.50–$6.30/hr × build duration
- Storage/egress: marginal (project is markdown + code, hundreds of MB at most)
- Target per-build cost: meaningfully below equivalent GHCP token spend (concrete number set after spike)

---

## 8. Key Metrics

| Metric                                | v1 target                                               | How measured                                   | Why it matters                                                                             |
| ------------------------------------- | ------------------------------------------------------- | ---------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Pass rate on Fire-Legasy-class replay | ≥30% (1 in 3 runs produces deployable software)         | Manual scoring of N≥10 spike runs              | If <30%, open models cannot execute the protocol — no amount of CLI work saves the project |
| Per-build cost vs. GHCP equivalent    | <50% of GHCP token cost for same workload               | Wall-clock × spot rate vs. GHCP usage estimate | Core economic claim; must be verified, not assumed                                         |
| Wall-clock per build                  | <8h for Fire-Legasy-class workload                      | End-to-end timing                              | Async tolerance is high but extreme runs become impractical                                |
| Faithful protocol execution           | All 9 phase gates emit expected scaffolding artifacts   | Diff against reference Fire Legasy scaffolding | Differentiator from generic agent runners                                                  |
| Successful teardown                   | 100% of runs free their GPU on completion or budget cap | Provider API audit                             | Cost cap is non-negotiable                                                                 |

---

## 9. Unfair Advantage

- **Owns the only proven user of the protocol**: founder is both author of lights-out-swe and the canonical user. Protocol-fidelity bugs surface fast; competitors lack this feedback loop.
- **Real shipped benchmark**: Fire Legasy is a deployed, production app built end-to-end through the harness — a concrete, replayable golden test that competitors building generic agent runtimes cannot easily match.
- **Methodology depth**: BEE-OS / nWave / TLA+ formal-spec discipline is unusual in the agent-runtime space; most competitors optimize for raw code generation, not gated convergence.
- **Solo speed on a narrow target**: hyperscalers cannot ship a lights-out-swe-faithful runtime; their economics demand horizontal generality.

---

## Coherence Check

- ✅ Problem #1 directly maps to Solution (decouple harness from GHCP runtime)
- ✅ UVP is anti-positioned against the actual alternatives
- ✅ Key Metrics are falsifiable and tie to Cost Structure assumptions
- ✅ Unfair Advantage is grounded in shipped artifacts, not aspiration
- ⚠️ Customer Segments is n=1; this is a known risk and is documented in `wave-decisions.md` as Constraint C2
