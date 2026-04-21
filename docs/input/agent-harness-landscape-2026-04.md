# Agent Harness Landscape + the Deepresearcher Pincer

**Status:** Live exploration doc. Not a scope input yet.
**Date:** 2026-04
**Prompt:** "is there anything else like GenericAgent? ... maybe we can make a deepresearcher one to see how far agentic loops can go on various missions"
**Companion reference:** [karpathy/autoresearch](https://github.com/karpathy/autoresearch) (75k⭐, MIT, March 2026)

## Why this doc exists

GenericAgent showed one shape of a minimalist self-evolving loop. It is one data point in a much bigger landscape of minimalist agent harnesses, and many of them ship real ideas worth absorbing. None of these have been benchmarked against each other on a common set of missions — the field is pre-evidence. This doc:

1. Surveys the peer harnesses so we stop discovering them one-at-a-time.
2. Traces the theoretical lineage so we know which primitives are genuinely new and which are repackaged.
3. Extracts karpathy/autoresearch's design ideas — it is the cleanest articulation of **autonomous overnight experiment loops with evidence** we have seen.
4. Proposes a concrete **Deepresearcher pincer**: a Tier 1 mission whose job is to run autonomous benchmark loops across OP's own pincers and mission fixtures. The substrate learns how far its own agentic loops go on real missions, with evidence, not vibes.

This doc is inspiration for the north star. It is not scope.

---

## Peer harnesses worth knowing

Grouped by the primitive they most clearly demonstrate. Not exhaustive.

### Minimalist self-evolving loops (direct GenericAgent peers)

- **GenericAgent** (lsdefine, Python, MIT, 5k⭐). ~3K LoC core, ~100-line loop, L0–L4 memory, auto-skill-crystallization. Already covered in [genericagent-notes-2026-04.md](genericagent-notes-2026-04.md).
- **SmolAgents** (HuggingFace, Python, Apache-2, ~10k⭐). Code-first agent — the model writes Python that calls tools instead of emitting JSON. Same core claim as Cloudflare's "Code Mode": let the model write code, don't pre-declare 30 tools. Very small core, heavily integrated with HF ecosystem.
- **BabyAGI / AutoGPT** (original 2023-era minimalist loops). Historical interest only. BabyAGI's "task list in memory, loop until done" shape is present in every harness since.
- **AgentZero** (frdel, Python). "Dynamic, self-organizing" agent — tries to avoid hardcoded workflows; closer in spirit to GenericAgent's emergence claim. Harder to evaluate because the project is more vibe than spec.
- **Pyspur / Chidori** (experimental Rust agent runtimes). Chidori is Rust + reactive runtime; interesting for substrate-language alignment but pre-alpha.

### Multi-agent orchestration (Bet #12 peers)

- **MetaGPT** (Python, ~45k⭐). Role-based multi-agent ("PM", "engineer", "architect") with a software-company metaphor. First widely-used instance of fixed-role fanout.
- **CrewAI** (Python, ~25k⭐). Simpler than MetaGPT; role + task + process abstraction. Widely adopted. Demonstrates that role-based orchestration is a market, not a research topic.
- **AutoGen** (Microsoft, Python). Conversation-first multi-agent; heavy. The reference for "agents talking to agents in turns."
- **LangGraph** (LangChain, Python). Graph-structured agent flows; the serious engineering face of LangChain. Real production use.
- **OpenManus** / **Manus clones** (Python). Open re-implementations of Manus's general-agent demo. Useful as a reference for browser-agent UX.

### Code agents (Aider / Claude Code peers)

- **OpenHands** (formerly OpenDevin, Python, ~35k⭐). Container-sandboxed software-dev agent; directly validates the Bet #11a sandbox premise. Heavy but real.
- **SWE-agent** (Princeton, Python). Research-grade SWE-bench solver; introduced the "agent-computer interface" abstraction — a restricted shell the agent sees, not the raw terminal. Relevant design for tool-shaping.
- **Aider** (Python, ~25k⭐). Pair-programming agent, git-native, no sandbox. The clean version of "the agent edits files in your repo and commits."
- **Cline / Roo Code** (VSCode extensions). IDE-embedded code agents. Not substrate-comparable but useful as UX reference.
- **Goose** (Block, Python). Extensible agent with first-class MCP. Closer to OP's substrate framing than most. Worth watching.
- **Claude Code / Codex / Gemini CLI**. Reference harnesses from frontier vendors. Closed enough that we can't port ideas directly, but `AGENTS.md` and the skill-file pattern were demonstrated here first.

### Skill-library / curriculum ancestors (Bet #6a's real lineage)

- **Voyager** (Wang et al., NeurIPS 2023). **This is the paper Bet #6a is descended from.** LLM agent in Minecraft with an automatic curriculum, an iterative prompting mechanism, and a skill library of verified JavaScript programs. Every successful novel task was crystallized into a named skill that later tasks could call. Near-identical shape to what GenericAgent rediscovered for general tool use.
- **Generative Agents** (Park et al., "Smallville", UIST 2023). Memory-stream + reflection + planning architecture. The reflection primitive ("periodically synthesize observations into higher-level insights") is the direct ancestor of L4 archive distillation.
- **Reflexion** (Shinn et al., NeurIPS 2023). Verbal reinforcement learning — an agent maintains a textual self-critique that conditions the next attempt. Lightweight and shockingly effective on coding benchmarks. Primitive worth porting into the wake-summary loop.

### Program-not-prompt frameworks (different category, adjacent concerns)

- **DSPy** (stanfordnlp, Python, MIT, ~34k★, v3.1 series). Not a harness — a _framework for programming language models_. You write modules with typed input/output signatures (`question -> answer: str`), compose them into programs (`ChainOfThought`, `ReAct`, `ProgramOfThought`), and then run an **optimizer** that compiles the program into a version with better prompts and few-shot demos, evaluated against a metric on a small dev set. The central shift: prompts are an output of compilation, not a hand-authored input.
  - Key primitives: **Signatures** (typed LM call contracts), **Modules** (composable program units), **Optimizers / Teleprompters** (BootstrapFewShot, MIPRO, **GEPA** — reflective prompt evolution that outperforms RL in the Jul'25 paper), **Assertions** (computational constraints that re-trigger generation on failure).
  - Backed by a serious research line: the Oct'23 DSPy paper, the Dec'23 Assertions paper, the Jun'24 MIPRO paper, and Jul'25's GEPA paper.
  - 396 contributors, 106 releases, active development — this is a mature ecosystem artifact, not a toy.
- **TextGrad** (Yuksekgonul et al., Stanford, 2024). Backpropagation through LLM calls using textual gradients — natural-language critiques play the role of gradient signal. Close cousin to DSPy's GEPA in spirit; more research-y in packaging.
- **Outlines / Guidance / Instructor** (structured-output libraries). Constrained decoding + typed I/O. Overlaps with DSPy's Signatures at a lower level. Relevant to OP's tool-call reliability more than to the harness question.
- **LMQL** (ETH, Python). Language-model query language with constraints. Earlier attempt at "structured LM programs." Less momentum than DSPy now.

**Why this category matters for OP:** every agent harness listed above treats _prompts as the unit_. DSPy treats _prompts as compiler output_. For OP, this is the missing primitive that makes the Deepresearcher pincer stop being vibes: if a benchmark run has a metric and a dev set, DSPy-style compilation can produce the concrete prompt a pincer uses, and the substrate can version it like any other artifact. See the Deepresearcher section below for how this composes.

### Deep research / autonomous experimentation (autoresearch peers)

- **karpathy/autoresearch** (Python, MIT, 75k⭐). The specimen under the microscope below.
- **GPT Researcher** (Python, ~20k⭐). Web-native autonomous research agent that produces long-form reports with citations. Well-engineered; closest existing "Deepresearcher" shape.
- **Stanford STORM** (Python). Outline-first Wikipedia-article generator. Strong example of structured multi-pass research with explicit planning/conversation/writing phases.
- **OpenAI Deep Research / Anthropic Research** (closed). Reference implementations; their behavior is the public benchmark for what "long-horizon research pincer" should feel like.

### Theoretical spine

Ideas every minimalist harness reuses, often without citing the source:

- **ReAct** (Yao et al. 2022) — interleave reasoning and action in one trace. The grandfather of every "think/act/observe" loop.
- **Toolformer** (Schick et al. 2023) — self-supervised tool-use. Introduced the frame of models deciding when to call tools.
- **Tree of Thoughts** (Yao et al. 2023) — deliberate search over reasoning steps. Underexploited in agentic harnesses.
- **Self-Refine** (Madaan et al. 2023) — generate, critique, refine; feedback from the same model. Cheap correctness uplift.
- **Self-Consistency** (Wang et al. 2022) — sample multiple reasoning paths, vote. Cheap robustness uplift.
- **Constitutional AI** (Bai et al. 2022) — a constitution (written rules) constrains behavior through self-critique. This is what Bet #1's Professional Bar is, architecturally.
- **Reflexion** (above) — verbal RL.
- **Voyager** (above) — skill library + automatic curriculum.
- **Absolute Zero / self-play RL for agents** (2025) — models generating their own training tasks. Not yet practical at substrate scale but watch.
- **DSPy + GEPA** (Khattab et al. 2023; Agrawal et al. 2025) — compiling declarative LM calls into self-improving pipelines; reflective prompt evolution outperforming RL on multi-stage LM programs. The first primitive that treats prompt authoring as a compiled artifact rather than a written one.

**Observation:** OP's "durable bets" are largely a _composition_ of these primitives into a single sovereign substrate with explicit authority bounds. The novelty is not any one primitive; it is the composition, the capability model, and the sovereignty story.

---

## What karpathy/autoresearch actually is

A minimal harness that lets an AI agent run autonomous overnight research on a single-GPU LLM training setup. Three files. Fixed 5-minute training budget per experiment. The agent edits one Python file, runs it, reads the result, decides what to change next, repeats.

### The design ideas worth stealing

1. **Fixed time budget per experiment makes results comparable across arbitrary agent changes.** Training always runs exactly 5 minutes regardless of hardware, architecture, batch size, optimizer. This is the single most important design decision: it turns a non-stationary search (model-quality-vs-everything) into a **stationary, comparable** one. For OP: mission-family benchmarks should have a fixed wall-clock budget per attempt, not a fixed step budget.

2. **One editable surface, everything else pinned.** The agent only modifies `train.py`. `prepare.py` (data, eval) is locked; `program.md` (instructions) is human-owned. This keeps the experiment loop honest: the agent cannot move the metric by changing the eval. For OP: a Deepresearcher pincer must only touch the surface declared in its capability grant; fixtures, graders, and acceptance contracts are not in scope.

3. **~12 experiments per hour, ~100 per overnight run.** This is the volume that turns "agentic research" from a gimmick into a real feedback loop. Most agent demos run one attempt and declare victory. For OP: a benchmark run should plan for dozens or hundreds of attempts per mission-family, not three.

4. **`results.tsv` as ground truth.** Experiments append to a tab-separated log. Analysis is a notebook. No hidden state; no mystique. The only evidence is the log. For OP: the event log is already this — but a benchmark pincer needs a thin projection that looks like autoresearch's `results.tsv`: one row per attempt, comparable metrics, no narrative.

5. **`program.md` is the program.** The repo is explicit that the human's job is to iterate on the markdown instruction file, not the code. The agent iterates the code. Two clocks: human edits `program.md` weekly; agent edits `train.py` hourly. For OP: acceptance contracts are the equivalent of `program.md` — an operator-owned surface that the pincer does not modify. Skills are the equivalent of `train.py` edits — agent-owned, fast-cycling.

6. **Autonomy plus evidence plus a metric.** The loop only works because there is a well-defined, cheap-to-compute metric (`val_bpb`, validation bits per byte) and a fixed-length evaluation. Remove any of those three and the loop degenerates into vibes. For OP: a mission family cannot have a benchmark until it has a machine-checkable acceptance contract with a scalar or bounded-categorical outcome.

7. **Small surface, not small ambition.** autoresearch's total line count is trivial. The ambition — fully autonomous overnight research with a persistent experiment log — is not. For OP: this is further evidence that the wake loop should stay tiny (Bet #11's implied minimalism) and that complexity should live in the memory controller and the skill tree, not in the loop.

### What autoresearch does _not_ solve

- No multi-agent orchestration. One agent.
- No capability model. The README says "disable all permissions" (i.e. grant everything). Fine for a single-repo experiment on a locked-down GPU machine; unacceptable for a sovereign substrate.
- No sandbox; the agent is executing code directly against the host GPU.
- No memory beyond `results.tsv` and the agent's own context.
- No skill tree; no crystallization. Each iteration is a fresh diff against the current `train.py`.

These gaps are exactly the things OP's durable bets are for. autoresearch proves the _loop shape_ is powerful; OP's job is to wrap it in the sovereignty and memory primitives.

---

## The Deepresearcher pincer: proposal sketch

**One-line:** A Tier 1 pincer whose mission family is "run autonomous overnight benchmark loops against a declared mission fixture, producing comparable experiment logs and a summary of how far OP's own agentic loops got."

This is meta-experimentation: a pincer whose subject is other pincers. It is the mechanism by which the substrate learns its own limits with evidence.

### Why it fits OP

- It turns "does this pincer actually work on Mission X" from a vibe call into a loggable experiment.
- It produces exactly the evidence the Sovereignty Ladder, the governance-class routing, and Bet #6a's skill-promotion decisions need. Right now those decisions are operator intuition.
- It is a concrete, bounded use of Bet #11a's ephemeral sandbox — the most naturally isolated mission family we can field. A benchmark run has no production side effects by design.
- It gives the operator a nightly "how is the substrate doing" artifact without another SaaS dependency.

### Minimum viable shape

Mirror autoresearch's shape, wrapped in OP's capability model:

- **Mission fixture** — a locked, versioned directory containing: an acceptance contract (Bet #5), one or more input payloads, a grader (deterministic scoring function or an LLM-as-judge prompt with a small rubric), and a metric definition. This is the `program.md` equivalent. Operator-owned.
- **Candidate under test** — a pincer binary reference and a capability grant. The capability grant is narrower than the pincer's normal production grant — only what's needed to run the fixture.
- **Attempt budget** — wall-clock per attempt (e.g. 3 minutes), total run length (e.g. 8 hours overnight), max concurrent attempts (e.g. 4). Fixed. Honest.
- **Substrate** — each attempt runs in a fresh ephemeral sandbox (Bet #11a). The Deepresearcher pincer reads the grader output, appends a row to the run's results projection, and decides what the next attempt should vary. Variation budget is a capability too: `vary:prompt`, `vary:model`, `vary:tool_subset`, `vary:skill_invocation_order`. Without the vary grant, the pincer can only re-run.
- **Results projection** — per-run append-only log (one row per attempt) and per-family rollup (pass rate, mean metric, failure mode histogram). These are first-class projections on the event log, queryable like anything else.
- **Exit evidence** — a wake summary naming the run, the attempt count, the top-k attempts by metric, the crystallized skills that helped or hurt, and any acceptance-contract violations encountered. This is the artifact the operator reads in the morning.

### What it experiments _on_

- Pincer comparison: same fixture, two reasoner-stack configurations (provider, model, role-axis from Bet #10). Which one gets further?
- Skill-tree ablation: run the same fixture with L3 disabled vs. enabled. Does the skill tree actually help, on what mission families, by how much?
- Context-budget sensitivity: vary the memory controller's per-call budget. Find the point where stalled-mission rate spikes. This is the evidence behind the tripwire.
- Governance-class routing: run the same fixture at each routing class. Does the open-weight path actually hold up on Tier 1 missions, or does it fail silently on edge cases?
- Skill-crystallization half-life: let the Deepresearcher run the same fixture family daily for a month. Watch which crystallized skills survive, which rot, which get re-crystallized in a different shape. This is the real validation of Bet #6a.
- **Prompt compilation (DSPy/GEPA-style):** treat the pincer's prompt set as an artifact produced by an optimizer run over the fixture's dev-set + metric, not as a hand-authored file. The Deepresearcher's `vary:prompt` grant is the natural substrate-side home for this: instead of randomly tweaking wording, it runs a declared optimizer (bootstrap-few-shot, GEPA, etc.) and records the compiled prompt as the attempt's artifact. Compiled prompts are versioned, stored in memory (likely L2 or L3), and subject to the same crystallization / pruning rules as any other skill.

### What it must not do

- It must not mutate production missions, production projections, or the operator's owned systems. Its capability grant excludes them.
- It must not run without an explicit fixture. "Let it figure out what to benchmark" is exactly the kind of ambient authority Bet #11 forbids.
- It must not self-extend its variation budget. If it wants to try a new axis of variation, it emits a proposal event; the operator or the next substrate version grants it.
- It must not be the thing that decides which pincers are promoted. It produces evidence; the operator decides.

### Fit with existing bets

- **Bet #1 (Professional Bar):** the fixture's acceptance contract is the Professional Bar for this mission family. Deepresearcher is the first pincer whose whole job is to produce evidence against Professional-Bar items.
- **Bet #2 (memory):** Deepresearcher reads L1/L3 to choose variations; writes L4 archive entries for each completed run.
- **Bet #5 (acceptance contracts):** Deepresearcher cannot run without one. This pincer enforces the bet rather than assuming it.
- **Bet #6 vs #6a:** Deepresearcher runs are the evidence source that lets the operator promote a skill from L3 pincer-scoped into a canonical Tier N mission type with confidence.
- **Bet #10 (reasoner as abstraction):** Deepresearcher is the mechanism for actually comparing reasoner stacks on real missions, not just on public benchmarks.
- **Bet #11a (sandbox):** Deepresearcher is the simplest natural consumer of the ephemeral-sandbox primitive; its existence stress-tests the sandbox under load.
- **Bet #12 (one pincer builds the rest):** Deepresearcher is plausibly among the first ~5 pincers worth writing, because every subsequent pincer's quality claim needs its evidence.

### When to build it

Not v7. v7 is event log + memory + first Tier 1 mission. The Deepresearcher pincer wants:

- Bet #11a sandboxes working in practice (v8ish).
- At least two Tier 1 missions shipped and stable, so there is something to benchmark.
- L3 skill-tree primitive in place, so skill-ablation experiments are possible.

Earliest plausible landing: v9. Candidate for the "fifth or sixth pincer" slot once the category's shape is clear.

### Naming note

"Deepresearcher" is descriptive, not load-bearing. The mission family is **Autonomous Overnight Benchmark**. The pincer's canonical name should come from that, once it lands.

---

## Absorbed advice for the north star

These are the harness-level takeaways that should flow into the technical-advice section of the north star, alongside the Stonebraker/Cloudflare/GenericAgent absorbed items:

- **Fixed-budget experiment loops are the evidence mechanism OP lacks.** Acceptance contracts produce pass/fail; benchmark runs produce _distributions_ over attempts. Distributions are what let you argue one stack is better than another. Plan the substrate to host benchmark runs as a first-class mission family.
- **Voyager's skill library primitive is 2.5 years old; we are not inventing Bet #6a, we are composing it into a sovereign substrate.** Cite the lineage in the bet to keep the team honest about what is novel and what is table stakes.
- **Reflexion's verbal self-critique is cheap and worth a prototype inside the wake-summary loop.** One extra LLM call per mission that conditions the next attempt's prompt on "what failed last time and why". Plausibly larger quality delta than another capability.
- **`program.md` / `train.py` decomposition validates the operator-owned-contract vs. agent-owned-skill split.** The two-clock model (weekly human, hourly agent) is a design principle, not a coincidence. Make it explicit in the catalog conventions.
- **The meta-experimentation pincer (Deepresearcher) is the missing feedback loop between "we wrote a pincer" and "we promoted a pincer to Tier N."** Without it, promotion stays intuition. With it, promotion becomes a claim with evidence attached.

---

## Companion reading

- [stonebraker-dbos-notes-2026-04.md](stonebraker-dbos-notes-2026-04.md)
- [cloudflare-ai-infra-notes-2026-04.md](cloudflare-ai-infra-notes-2026-04.md)
- [genericagent-notes-2026-04.md](genericagent-notes-2026-04.md)
- [north-star-2026-04.md](north-star-2026-04.md)

External sources referenced above are cited inline; this doc does not maintain its own link index.
