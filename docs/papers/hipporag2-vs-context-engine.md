# HippoRAG 2 vs. context-engine — two memory models at the stochastic–deterministic border

A comparison note for the paper in [`hipporag2.pdf`](hipporag2.pdf):
**"From RAG to Memory: Non-Parametric Continual Learning for Large Language
Models"** (Gutiérrez, Shu, Qi, Zhou, Su — arXiv:2502.14802v2, ICML 2025).

It is compared against the `context-engine` reference repo (`repos/context-engine`),
a neurosymbolic RAG knowledge system.

> Both systems independently converged on the same core mechanism — an LLM
> extracts an open triple graph, and a PageRank variant retrieves over it. They
> diverge on *what memory is for*: HippoRAG 2 optimizes **recall**;
> context-engine optimizes **governed correctness**.

---

## 1. The two models in one line each

- **HippoRAG 2** — a *retrieval index* modeled on hippocampal memory theory
  (LLM = neocortex, KG + Personalized PageRank = hippocampus, retrieval encoder =
  parahippocampal regions). Non-parametric, append-only, recall-optimized. Targets
  three human-memory capabilities at once: **factual, sense-making, associativity.**
- **context-engine** — a *governed knowledge base*. An LLM distills documents into
  typed, versioned, provenanced primitives (**Facts / Rules / Capsules**) retrieved
  via adaptive routing (semantic / graph-hybrid / ReAct). Correctness- and
  auditability-optimized ("correctness > completeness").

## 2. What HippoRAG 2 adds over HippoRAG v1

Structure-augmented RAG (RAPTOR, GraphRAG, HippoRAG v1, LightRAG) beats plain
embeddings on associativity/sense-making but *regresses on simple factual recall*.
HippoRAG 2 fixes all three axes with three refinements:

1. **Dense–sparse integration (passage nodes).** v1 had only *phrase* nodes
   (sparse/concept coding — concise but context-lossy). v2 adds **passage nodes**
   (dense/context coding) linked to their phrases by **context edges** (`contains`).
   Attacks the "concept–context tradeoff" head-on.
2. **Query-to-triple linking** (replaces v1's NER-to-node). The *whole query* is
   embedded and matched to **triples**, not entities → richer contextual alignment
   (+12.5% Recall@5 over NER-to-node in their ablation).
3. **Recognition memory (LLM triple filtering).** After retrieving top-k triples,
   an **LLM filters** them to the relevant subset before they seed PPR. This is the
   key shift: **HippoRAG 2 puts an LLM inside the online retrieval loop** — v1's
   online path was pure PPR.

Pipeline: query → retrieve top-k triples (query-to-triple) + rank passages by
embedding → recognition-memory LLM filter → filtered phrase nodes + all passage
nodes become PPR seeds (phrases by rank score, passages by embedding sim × 0.05
weight) → run PPR → rank passages → top-5 to the QA reader. Still non-parametric
and append-only; Fig. 3 shows it stays robust as the corpus grows.

## 3. Side-by-side

| Axis | HippoRAG 2 | context-engine | Verdict |
|---|---|---|---|
| LLM in online retrieval | **Yes** — recognition-memory triple *filter* (single pass) | Yes — full **DSPy ReAct** agent (multi-step think/act/observe) | Converged; context-engine goes further |
| Concept + context blend | phrase nodes (sparse) + **passage nodes** (dense) | facts/capsules + `notes`/`chunks` | Converged on mixing granularities |
| Node/edge typing | phrase vs passage; relation/synonym/context edges | **typed primitives**: Facts, **Rules (normative)**, Capsules | context-engine richer; HippoRAG nodes are never executable constraints |
| Graph algorithm | **Personalized PageRank** (rigorous, benchmarked) | "PageRank-lite" hub detection + query-aware edge filter (hand-rolled) | Same instinct; HippoRAG is the validated version |
| Query→KG linking | **query-to-triple** | semantic / graph / super-hybrid via a classifier | Different mechanism, same goal |
| Provenance | implicit (passage nodes, P matrix) | **first-class**: `doc_id` + line span on every fact/rule | Divergent |
| Lifecycle / governance | **none** | `status: active/deprecated/invalid`, versioning, **gated promotion** | Divergent |
| Trust signals | recall@5 / F1 only | `answer_mode: kb-backed/mixed/fallback` + `confidence` | Divergent |
| Continual learning | append edges; **empirically robust** to corpus growth | investigation→fact/rule **promotion** under policy; unproven | Both append-only; HippoRAG validated, context-engine governed |
| Optimized for | **recall** (factual + associativity + sense-making) | **correctness / auditability** | The real philosophical split |

## 4. Convergence and the persistent divergence

With v2, the two systems are closer on the **retrieval** side than v1 was:

- **HippoRAG 2's recognition-memory filter is a lean version of `context-engine`'s
  `brain-react`.** HippoRAG runs one LLM filter pass over candidate triples;
  context-engine runs a full ReAct loop with tools (`get_facts`, `get_rules`, …).
  Same idea — *use the LLM online to decide which structured knowledge is
  relevant* — at two cost/rigor points.
- **HippoRAG 2's passage-node + phrase-node design is the principled answer to
  context-engine's chunks-vs-facts split.** context-engine stores chunks and facts
  in separate tables and routes between them; HippoRAG 2 unifies both as nodes in
  one PPR graph with context edges. A cleaner architecture context-engine could
  borrow.
- The **three-axis framing (factual / sense-making / associativity)** is something
  context-engine implicitly chases with its adaptive classifier but never names or
  measures. A good evaluation lens to import.

What still does not exist in HippoRAG 2 — and is context-engine's entire reason for
being: **rules as normative/executable objects, provenance lifecycle, gated
promotion, and per-answer trust signals.** HippoRAG 2 proves you can get
state-of-the-art *recall* with none of that; context-engine bets that *acting on*
memory requires it.

## 5. Relevance to the harness

The two map cleanly onto the harness's stochastic–deterministic split:

- **HippoRAG 2 = the validated retrieval/recall core** (LLM extracts at index time;
  deterministic PPR retrieves at query time, with an LLM recognition gate).
- **context-engine = the governance shell** — and that shell is what maps onto the
  kernel's invariants: provenance ↔ taint, status-lifecycle/gated-promotion ↔
  *"nothing untrusted silently becomes authoritative,"* rules-as-objects ↔
  architecture invariants.

The clean lesson: **these are separable layers.** Put context-engine's governance on
top of HippoRAG 2's retrieval and you get governed recall — the memory analog of
what the harness does for actions (design-time stochastic, runtime deterministic).

---

*Source: full read of `hipporag2.pdf` pp. 1–9 (methodology, experiments, discussion,
conclusion). Compared against `repos/context-engine` README / ARCHITECTURE.md /
NEUROSYMBOLIC_ARCHITECTURE.md.*
