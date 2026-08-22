# D38 — The March-2026 runtime cluster is superseded; record the lineage


**Date:** 2026-07-17.

- **Context:** In March 2026 the border ideas were first stated as four separate `sv-pro`
  repos, all dormant since late March: `safe-agent-runtime-core` (deterministic policy kernel +
  IRBuilder + taint, 43 commits, Mar 18–22), `safe-agent-runtime-pro` (typed models / capability
  DSL / presets, 21 commits), `agent-world-compiler` (workflow → world manifest → capability
  surface, 26 commits), `agent-world-compiler-poc` (least-privilege-from-observed-execution PoC,
  30 commits). `docs/THESIS.md` §5 credits only `agent-hypervisor` and `safe-mcp-proxy` as
  primitive sources and is silent on these four — the single biggest "which repo is real?"
  ambiguity in the cluster. Silence is not a decision; this entry makes it one.
- **Decision:** Declare the March cluster **superseded**, and record where each idea now lives:
  - `safe-agent-runtime-core` (kernel, IRBuilder, taint/provenance) → **`crates/world-kernel`**.
    The lineage is concrete: `-core`'s final commits added the "Safe MCP Proxy / Agent Runtime
    Firewall" positioning that `safe-mcp-proxy` carried forward a month later (→ `ABSENT ≠ DENY`,
    §5).
  - `agent-world-compiler` (workflow → manifest compiler) → **`crates/compiler`**.
  - `safe-agent-runtime-pro` (typed models / capability DSL / presets) → the manifest schema
    across **`crates/compiler` + `crates/harness-types`**.
  - `agent-world-compiler-poc` → **spent**; its PoC role is fulfilled by `crates/compiler`.
  - Capability projection as a *concept* now lives in
    `agentic-execution-governance/mcp-tool-projection` (a §5 primitive source) and
    `cedar-world-playground`, not in the dead compiler.
  - **Archive** all four on GitHub with a one-line README pointer here. Archive, not delete —
    the provenance trail is what makes this supersession auditable.
- **Not superseded by this entry:** `sv-pro/agent-harness` is a **model-eval fixture**
  (`HARD_TASK.md`, hard-opus vs hard-fable), not part of this lineage — keep it; retitle its
  README so it stops reading as a product. It is distinct from the third-party
  `agent-harness-generator`/MetaHarness rejected in D24, and from the separate 1-commit
  `agentic-execution-governance/agent-harness` placeholder (a name collision resolved elsewhere).
- **Why:** converts §5's silence into an explicit decision and closes the largest source of
  cluster ambiguity, while preserving lineage.
- **Alternatives rejected:** keep them as separate active repos (N drifting statements of one
  thesis, none authoritative — the fragmentation D23 exists to end); delete them (loses the
  lineage record).
- **Related:** D23 (unify under one thesis), D30 (rename to `ai2rules`), **D39** (umbrella form),
  §5 / §7.3.
