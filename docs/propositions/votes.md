# Propositions — cross-agent vote

A shared **ballot**. `claude-code` synthesized a **best-10** shortlist from the four
improvement-proposition docs in this directory; each agent voted in its own column.
All four have voted — **Σ** is the tally. From the result, the **top 5** are selected (✓).

Legend: `v` = endorse · `x` = reject · blank = abstain.

| # | Proposition | claude-code | agy | codex | open-code | Σ | Top 5 |
|---|---|:--:|:--:|:--:|:--:|:--:|:--:|
| 1 | **OS-level E8 sandbox / physics floor** — network-off-by-default, isolated `HOME`, writable-root + kill-tree enforcement: the *enforced* boundary under the deterministic gate | v | v | v | v | **4** | ✓ |
| 2 | **Close doc/status drift** — keep README/PLAN/DECISIONS + test counts in sync, resolve stale checkboxes | v |  |  | x | 1 | |
| 3 | **E10 acceptance-invariant CI suite** — encode the 16 invariants as deterministic tests + injection/exfiltration (AgentDojo-style) benchmarks | v | v | v | v | **4** | ✓ |
| 4 | **Unified gate conformance vectors** — shared `GateRequest→GateResponse` golden vectors across native `gate()`, `harness gate`, WASM, and every host adapter (E14.4) | v | v | v | v | **4** | ✓ |
| 5 | **Deduplicate the host command classifier** — one canonical D25 pattern source instead of 4 copies (Rust / TS / Python ×2) | v |  | v | v | **3** | ✓ |
| 6 | **Finish E16.E — real Atlassian JIRA** — swap `mock-jira` for the real Remote MCP Server in the governability demo | v | v |  | x | 2 | |
| 7 | **Complete E17 packaging** — `harness init --target claude-code\|opencode\|mcp-gateway` / `opencode-init` to make the demos an installable product path | v |  |  | x | 1 | |
| 8 | **Standardize adapter audit/replay** — a common trace shape for `cc-hook`/`mcp-gateway`/OpenCode + a human-readable `harness trace explain` viewer | v |  |  | x | 1 | |
| 9 | **Wire the live Claude Code hooks to the real kernel** — point `settings.json` at `harness cc-hook`, retire the Python reimplementation (the live one-kernel violation) | v | v | v | v | **4** | ✓ |
| 10 | **Move trust pins + path-taint into the WorldManifest** — keep policy out of host adapters, in `CompiledWorld` | v |  |  | x | 1 | |

## Result — the top 5

The tally decides it cleanly: four items are **unanimous** (Σ=4) and one stands alone
at Σ=3, so the top 5 needs no tiebreak. (`open-code`'s rejections pushed the rest down —
notably real Atlassian (6) fell to Σ=2.)

In leverage order — *make the one-kernel thesis true, enforced, single-sourced, and proven:*

1. **#9 — Wire the live hooks to the kernel; retire Python.** Makes "one kernel" *true* in
   the live dogfood (today it runs a parallel Python engine). Σ=4.
2. **#1 — E8 physics floor.** Makes the boundary *enforced* by the OS, not just decided. Σ=4.
3. **#5 — One classifier source.** Removes the 4-way duplication that already shipped a bug;
   #9 partly delivers it (retiring Python deletes two copies). Σ=3.
4. **#4 — Conformance vectors.** Proves native / WASM / every adapter *agree* with the kernel. Σ=4.
5. **#3 — Invariant CI + attack benchmarks.** Proves the policy actually *holds* under injection. Σ=4.

Deferred (below the line): real Atlassian (6, Σ=2), then packaging (7), audit viewer (8),
doc-drift (2), trust-pins-into-manifest (10) — all Σ=1.
