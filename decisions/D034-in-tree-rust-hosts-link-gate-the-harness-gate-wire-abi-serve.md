# D34 — In-tree Rust hosts link `gate()`; the `harness gate` wire ABI serves out-of-process / non-Rust hosts


**Date:** 2026-06-29. **Refines D33's mechanism** (which said the CC hook would shell to the
`harness gate` *binary*).

- **Context:** building E16.C, the natural shape was `harness cc-hook` — a `PreToolUse`
  adapter that, being Rust *inside this workspace*, can call `harness_preview::gate()`
  **directly**. The same is already true of `harness mcp-gateway` (E16.B). D33's wording
  ("the hook calls the `harness gate` binary") implied a subprocess per call.
- **Decision:** an agent host that is **Rust and in-tree links `gate()` in-process**
  (`cc-hook`, `mcp-gateway`). The **`harness gate` subprocess (D24 wire ABI)** is for hosts
  that are **out-of-process or non-Rust** (a Python/Node adapter, a different repo, an IDE
  plugin) — they marshal `GateRequest`/`GateResponse` JSON over stdio. Both paths call the
  *same* pure function, so verdicts are identical by construction.
- **Why:** a `PreToolUse` hook fires on **every** tool call; spawning a process + recompiling
  the world per call is needless overhead, and the value of D24 (no *reimplementation* of the
  kernel) is preserved either way — in-process is the same `gate()`, not a parallel engine.
  The wire ABI keeps its real job: a language/process boundary, not a Rust↔Rust one.
- **Alternatives rejected:** force `cc-hook` to shell to `harness gate` (two extra processes
  + double JSON marshalling per native tool call, for no isolation benefit between two halves
  of the same binary); drop the wire ABI entirely (breaks non-Rust hosts — D24's whole point).
- **Related:** **D24** (the wire ABI this scopes), **D33** (the mechanism this refines), E16.B/E16.C.
