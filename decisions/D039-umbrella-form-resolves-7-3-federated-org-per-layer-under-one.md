# D39 — Umbrella form (resolves §7.3): federated org-per-layer under one master thesis


**Date:** 2026-07-17. **Resolves** the umbrella-form decision deferred in `docs/THESIS.md` §7.3
and `PLAN.md`.

- **Context:** §7.3 left three options open — (a) meta-repo with submodules, (b) docs-only
  umbrella site, (c) Cargo/workspace consolidation — and `PLAN.md` deferred the choice "until the
  context-engine demo reveals the natural structure." As of 2026-07-17 the cluster is *already*
  split across GitHub orgs by thesis layer: **`agentic-execution-governance`** (action +
  capability: `mcp-tool-projection`, `cedar-world-playground`), **`Intent-Hub`** (intent +
  knowledge: `intentos-core`, `intentos-specs`, `intent-workbench`), and **`sv-pro`** (the
  `ai2rules` action flagship + everything else). Two documents each claim source-of-truth status:
  this repo's `docs/THESIS.md` (the border) and `Intent-Hub/intentos-specs` ("the single source
  of truth" for the intent layer). That is the `semlens` spec-drift failure mode, one level up.
- **Decision:** Adopt a **federated** umbrella — org-per-layer, unified by one master thesis:
  - **`docs/THESIS.md` (this repo) is the single master thesis** for the whole program (the
    border + five layers). There is exactly one.
  - Each layer keeps its own org and may keep its own specs (e.g. `Intent-Hub/intentos-specs`),
    but **those specs point *up* to the master thesis and never restate it** — the same anti-drift
    rule the control-room workspace follows. Layer specs govern implementation detail *within* a
    layer; the thesis governs what the layers are and why.
  - **No forced consolidation into a single repo.** Crates remain the unit of modularity *within*
    a repo; orgs remain the unit *across* layers.
- **Why:** it matches the structure already built instead of fighting it; it kills the two-SSOT
  drift by subordinating every layer spec to one thesis; migration cost is ~zero. It rejects
  "one repo" specifically because the evidence for it (10 crates already work) argues for
  crate-granularity *within* a repo, not for collapsing three orgs into one.
- **Alternatives rejected:**
  - (c) single consolidated repo — absorb the org repos as crates, archive the orgs: real
    migration cost, and it fights a deliberate org structure; the crate evidence supports
    intra-repo granularity, not cross-layer collapse.
  - fully independent projects with co-equal SSOTs — exactly the drift this entry prevents.
  - keep deferring — the deferral itself was the management cost that prompted this.
- **Follow-ups (non-blocking):** open `intentos-specs` with a pointer to the master thesis; add a
  "layers & homes" table to §7 listing each org; resolve the `agent-harness` name collision (D38).
- **Related:** D23, D30, **D38**, §7.3, `PLAN.md` "Next step".
