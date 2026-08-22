# D23 — Unify the sibling repos under one thesis: Agentic Governance at the stochastic–deterministic border

- **Status:** Accepted (positioning)
- **Decision:** Treat the harness and the sibling reference repos as **one
  project** seen from layers, not separate efforts. Headline **category** =
  *Agentic Governance*; core **thesis/mechanism** = *the stochastic–deterministic
  border* ("design-time stochastic, runtime deterministic"). Five layers, each a
  fragment applying the same border move to a different governed resource: Action
  (this harness / `world-kernel`), Capability (`cedar-world-playground`), Knowledge
  (`context-engine` + HippoRAG-2-style retrieval), Intent
  (`intent-memory-engine`/`intentos-core`), Substrate
  (`llm-service-stack`/`personal-llm-box`, peripheral). Canonical spine is
  `docs/THESIS.md`; the cross-layer claim is demonstrated by
  `agent-core/examples/poisoned_knowledge_demo` (a poisoned KB document cannot
  escalate into a forbidden action — the taint floor flips an identical
  `fetch_web` from ALLOW to DENY).
- **Alternatives:** (a) **keep them as separate projects** — honest about their
  different maturities, but forgoes the compounding narrative and the shared
  primitive kit (taint, sealed intent, ABSENT≠DENY, capability projection) that
  actually makes them one idea; (b) **lead with the thesis name alone**
  ("Stochastic–Deterministic Border") — sharpest for engineers but opaque to a
  security/enterprise audience and to search; (c) **lead with the category alone**
  ("Agentic Governance") — legible but generic, loses the mechanism that is the
  real contribution; (d) **IntentOS-only branding** (from `intent-memory-engine`)
  — a product name, not a thesis, and overweights the least-mature fragment.
- **Why:** category + thesis layered keeps the work legible to outsiders *and*
  precise to engineers, and the §5 claim — one primitive kit governs actions,
  capabilities, knowledge, and intent — is what makes five fragments cohere. The
  umbrella *form* (meta-repo vs docs site vs Cargo-workspace consolidation) is
  deliberately deferred: the structure should fall out of the cross-layer demo, so
  it will be recorded as a separate decision when taken.
