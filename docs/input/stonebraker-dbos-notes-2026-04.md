# Stonebraker on DBOS, Databases, and Agentic AI — Technical Notes

**Source:** Podcast interview with Mike Stonebraker (Apr 2026 Ryan Pterman). [Turing Award Winner: Postgres, Disagreeing with Google, Future Problems | Mike Stonebraker](https://www.youtube.com/watch?v=YPObBOwIrHk)

**Why this doc exists:** Stonebraker is the Postgres / Vertica / VoltDB / Streambase / DBOS lineage. His takes are opinionated and load-bearing; they should inform the north-star's Durable Bets (especially Bet #2: memory is the substrate), not be accepted wholesale. This file extracts only the technical claims and arguments.

---

## Headline Claims

1. **Durable workflows belong in the database.** DBOS's thesis: "most everything you do in an operating system is managing data at scale — do it with database technology." The product form is annotated functions (`@workflow`, `@step`) whose state is persisted in Postgres; on crash, execution resumes from the last committed step.
2. **Agentic AI is about to become a distributed-database problem.** Today most agentic workloads are read-only (predict, classify, summarize). The moment agents start doing read-write work — moving money between accounts, mutating external systems — they need ACID, atomicity across steps, and distributed commit. Eventual consistency does not work for this.
3. **One-size-fits-all databases give up an order of magnitude.** Row stores, column stores, stream engines, and vector stores are architecturally distinct. A single engine trying to serve all of them pays a 10× penalty somewhere. A common parser on top of multiple specialized implementations is viable; a single implementation pretending to serve all shapes is not.
4. **Text-to-SQL does not work on real data warehouses.** On clean academic benchmarks (Spider, Bird) LLMs hit ~85% accuracy. On the Beaver benchmark (anonymized real production warehouses) they hit **0%**. With RAG, 10%. Given the full `FROM` clause and all join predicates, 35%. A competent human with the schema hits ~90%.
5. **Structured-data joins should be done in SQL, not in an LLM.** If you need to join two structured sources, turn them both into tables and let a query optimizer join them. Do not ask an LLM to correlate tables in its context.

## Technical Claims with Arguments

### On durable execution

- **Persist state in the database, then make it fast.** DBOS's design move is unglamorous: treat the database as the source of truth for every step transition, then engineer around the resulting performance cost. The payoff is _atomic workflows_ — either the whole workflow completes or it looks like it never happened — which is what read-write agents will require.
- **A file system written on top of a DBMS beats Linux's; a scheduler on top of a DBMS is competitive with dedicated schedulers.** The original academic DBOS project showed the file-system win and the scheduler parity (not superiority). Stonebraker's summary: "there's really no downside" — other than political resistance from OS and programming-language communities whose turf it crosses. Note the asymmetric claim: DBOS beat Linux's file system and matched (not beat) its scheduler.
- **Workflows are the right shape for cloud applications.** Cloud-shaped programs are already decomposed into steps with handoffs and retries. Making those steps transactional and the workflow durable is a natural fit. He claims DBOS is "a great deal faster and a great deal easier to use than the competition."

### On agentic AI

- **Read-write agents need atomicity across agents.** His example: two agents cooperating to move $100 between accounts — debit one, credit the other. Either both commit or both roll back. This is a distributed-transaction problem, and "eventual consistency solves a problem that rarely occurs in practice" (paraphrased).
- **Why eventual consistency fails for agent workflows.** If two agents simultaneously decrement the last widget in inventory, eventual consistency lets both succeed and settle to `-1`. For any business with real referential integrity (stock ≥ 0), this is broken. Google abandoned eventual consistency when Spanner shipped; agentic systems will arrive at the same conclusion.
- **Most current agent deployments get away with sloppy consistency because they are read-only.** That is not the steady state. Production agentic systems will need the same correctness guarantees as any other OLTP workload — and will discover they need a transactional workflow engine underneath them.

### On databases in general

- **Postgres is the correct default at the low end.** "Until you're trying to do a million transactions a second, it works just fine. Until you're trying to support a petabyte data warehouse, it works just fine." Large programming community, rich extension ecosystem, free, easy to staff.
- **Postgres is not correct at the high end.** No native column store, no first-class multi-node support. For serious data warehouses, a dedicated column store (Vertica, Clickhouse) beats it by an order of magnitude.
- **Graph databases are almost never the performant option.** If you like the nodes-and-edges interface, build that interface on top of a relational engine.
- **The hardest part of a database is the query optimizer.** Still true today; it is "algorithmically difficult" and remains the dominant source of complexity in any serious engine.
- **Indexing does not parallelize on GPUs.** B-tree traversal is pointer-chasing (root → interior → leaf), which is the antithesis of SIMD. Whenever indexing is the right answer, GPUs are not.

### On LLMs and text-to-SQL

Why LLMs fail on real data warehouses, per Stonebraker:

1. **Data not in the training pile.** Production warehouse schemas are private. "If you haven't seen the data a couple times before, you have no chance of regurgitating it."
2. **Real queries are 5–10× longer than benchmark queries.** Spider / Bird queries are 10–20 lines of SQL. Real warehouse queries are 100+ lines.
3. **Real schemas are not mnemonic.** Materialized views cause redundancy. Column names are underscore-prefixed abbreviations. Multiple tables have overlapping meanings.
4. **Idiosyncratic per-org vocabulary.** "J-term" at MIT (January intensive month). Every real warehouse has dozens of these.

**What works instead:**

- Supply the `FROM` clause and join predicates as part of the prompt; let the model fill in the `SELECT` and `WHERE`.
- Decompose the query into simpler pieces before handing any piece to the model.
- When multiple structured sources are involved, translate everything to tables and join in SQL. The model is for turning natural language into structured _pieces_; it is not the join engine.

---

## Implications for Open Pincery

These are my reading of what the above means for the north-star, not Stonebraker's claims.

1. **Bet #2 (memory as substrate) is reinforced, not challenged.** A senior operator at the Postgres / Vertica / DBOS scale independently arrived at "put the state in the database and engineer around it." Open Pincery's choice of event-sourced Postgres as the memory substrate is on the same line of reasoning.
2. **The read-write inflection is real and is coming.** Tier 1 missions today are mostly read-heavy (codebase steward reviews PRs, inbox triage drafts replies, weekly digest reads). The catalog will drift toward read-write as operators trust it more (commitments tracker writes calendar events, pipeline follow-up writes CRM records, exploratory runner spins up and tears down infrastructure). The substrate's transactional guarantees — today implicit in single-Postgres durability — will be load-bearing within two to three releases.
3. **Atomic multi-step missions belong on the roadmap.** DBOS's "the whole workflow either finishes or looks like it never happened" is exactly what Open Pincery wants at the mission level. Today the substrate offers durable-step (the event log records each step) but not atomic-mission (partial mission failure does not roll back earlier writes to external systems). This is a known gap; Stonebraker's claim makes the timing sharper.
4. **The pincer protocol should think about transactional handoffs between pincers.** Two pincers cooperating on a budget transfer have the exact two-phase-commit problem from the transcript. The pincer protocol (Bet #9) should not punt on this; it should declare where the transaction boundary lives, even if the initial implementation is "workflow-scoped, single-database, single-node."
5. **Text-to-SQL failure modes apply to Open Pincery's own memory queries.** When pincers issue recall queries over the event log and projections ("what did I decide about X three weeks ago?"), the same failure modes apply: private schema, long queries, idiosyncratic vocabulary. The memory controller should expose structured query primitives — not free-text-to-SQL over the whole schema — and decompose natural-language recall into smaller pieces against narrow views. This validates the Bet #2 choice to own the memory controller interface rather than exposing raw SQL to reasoners.
6. **Do not adopt DBOS.** DBOS is a library in Python / TS / Go / Java. Open Pincery is Rust. More importantly, adopting DBOS would replace plumbing that is already working (wake loop, event log, tool audit) with plumbing that is the _same shape_ but in a different language with a vendor relationship. The right move is to _watch DBOS for primitive ideas worth porting_ — atomic-workflow semantics, their Conductor dashboard's operator affordances, their `@step` annotation ergonomics — not to take a dependency.

## What to Discard

- **Stonebraker's "CS may not be a growth industry" and "18-year-olds should go into the trades" claims** are off-topic for Open Pincery. They are interesting career advice; they are not technical claims about the substrate.
- **His dismissal of graph databases** conflicts with Bet #2's v10 CozoDB plan. His argument is about _performance of graph engines vs. relational engines on graph-shaped queries_, which is a real concern but not the only concern: sovereignty, single-binary deployment, and embeddable Datalog drove the CozoDB choice, and those matter more than query-shape performance for a solo-operator substrate. Keep the CozoDB bet; record that Stonebraker would disagree.
