# D40 — Repository topology: one live implementation, the rest archived reference (completes §7.3 / D39)


**Date:** 2026-07-18. **Completes** the umbrella-form decision: D39 settled umbrella
*ownership* (one master thesis); this entry settles umbrella *form*.

- **Context:** D39 adopted a federated org-per-layer umbrella and rejected "one repo" — but read
  narrowly that leaves the cluster a permanent constellation of live repos across three orgs, which
  is the management cost that started this whole exercise. Revisiting with the owner: the goal is
  not permanent federation but a **single live implementation**, with the federation as inherited
  state to consolidate. `ai2rules` already holds the entire action layer as crates and is the only
  repo with a real status (M1–M3 done, 152 tests). The intent layer (`Intent-Hub`) is the one
  still-live sibling — and its action/intent split is the two complementing sides of the *same*
  stochastic–deterministic border, so it belongs *inside* `ai2rules`, not beside it.
- **Decision:** Resolve the cluster to a **four-role topology**:
  1. **One live public implementation — `ai2rules`.** All core logic, demos, tests. The sole repo
     under active development.
  2. **One private meta / workspace — `agentic-execution-governance`** (the control room). Governs
     the cluster, owns publishing drafts, never restates the thesis.
  3. **Publishing rides on `ai2rules`** (ai2rules.dev + `blog/`); drafts stage in the meta repo and
     publish from the flagship. No dedicated publishing repo.
  4. **Everything else → archived, read-only reference**, each with a one-line README pointer here.
- **Intent layer folds in over time.** `Intent-Hub` (`intentos-core`, `intentos-specs`,
  `intent-workbench`, `intent-os`) is **not archived**; its live work migrates into `ai2rules` as
  crates, and only then do those repos archive. Until migrated, `Intent-Hub` stays live and
  `intentos-specs` keeps pointing up to the master thesis (D39).
- **First archive batch (2026-07-18):** the D38 March cluster (`safe-agent-runtime-core`, `-pro`,
  `agent-world-compiler`, `agent-world-compiler-poc`); `safe-mcp-proxy` (§5 reference, frozen but
  readable so §5 citations resolve); and the superseded intent predecessors (`intent-memory-engine`,
  `intent_core`, `ai-aikido-gateway`). Eight repos.
- **Held back this round (owner's call):** `agent-hypervisor` — the origin repo and the cluster's
  only external traction (7★); it is frozen *later*, not now. `context-engine` stays
  **live-dormant** (load-bearing for the next step). `agent-harness` (sv-pro) stays a model-eval
  fixture (retitled, D38). The capability org repos (`mcp-tool-projection`, `cedar-world-playground`)
  and the adjacent set (`semlens`, `manifest`, `claude-mem`, `mcp-workspace-gateway`,
  `cli-mcp-adapter`) are deferred sub-calls, not swept in.
- **Why:** it delivers what D39 could not — one answer to "which repo is real?" — while keeping
  every superseded repo auditable (archive, not delete). It **refines rather than reverses D39**:
  the single master thesis and the "point up, never restate" anti-drift rule both stand; what changes
  is that federation becomes a *migration path* to consolidation, not the endpoint.
- **Alternatives rejected:** keep the federation permanently (D39 read narrowly) — leaves N live
  repos and the management cost intact; delete the superseded repos — loses the lineage D38 exists
  to preserve; archive `Intent-Hub` now — discards live intent work and the other half of the border.
- **Related:** D23, D30, **D38** (the March cluster this batch archives), **D39** (umbrella
  ownership this completes), §7.3.
