# D33 — Pivot: one Rust binary, lead with the *governability gap* (not the least-governable host)


**Date:** 2026-06-28. **Supersedes the demo *mechanics* of D32** (keeps D32's finding that
Copilot is governable only at the MCP surface).

- **Context:** the maintainer disliked the D32 setup — a two-repo split, three runtimes
  (Rust kernel + Python proxy + Node `mcp-remote`), the demo running a *parallel* Python
  engine instead of the real kernel, and "aligning down" to the least-governable host.
- **Decision:** build the internal demo **Rust-only, inside `ai2rules`** (no second repo, no
  Python, no Node in the core), running the **real `world-kernel`** via the gate ABI. Three
  pieces, one `harness` binary:
  - **Claude Code = deep:** its `PreToolUse` hook calls the Rust **`harness gate`** binary
    (retiring the Python hooks for the demo) — governs **native tools + MCP** (taint floor,
    ABSENT, ASK).
  - **Copilot = shallow:** a new **`harness mcp-gateway`** (world-kernel in-process) governs
    the **MCP tool surface only** — the only place Copilot is governable.
  - **`harness mock-jira`:** a self-contained Rust MCP upstream (jira_* tools incl.
    destructive), so the demo runs anywhere with **no creds / Node / Python**. Real
    Atlassian is a later *skin* (would add `mcp-remote` or a Rust SSE client).
- **The demo's payload is the gap, not the tool.** Same intent on both hosts; CC covers
  native+MCP, Copilot covers MCP-only; the uncovered native action on Copilot is the
  awareness point. Output artifact: a **host governability scorecard** (also blog fuel) —
  "platforms aren't equal in *governability*, not just features."
- **Why:** collapses the two-repo + three-runtime sprawl into **one Rust binary**, makes the
  demo run the **actual moat** (the kernel, not a parallel engine), and reframes around a
  more original, thesis-aligned message. The Python `mcp_gateway` (safe-mcp-proxy
  `feat/mcp-gateway`) stays as the throwaway prototype that proved the shape.
- **Alternatives rejected:** keep the Python proxy (two repos, parallel engine, drift);
  Copilot-only / align-down (hides the real story); a Node bridge for real Atlassian in v1
  (reintroduces a runtime — deferred to the real-JIRA skin).
- **Related:** D24 (gate ABI), D31, **D32** (the finding this keeps), E16 (re-cut), **E13.4**
  (this *is* the Rust MCP projection shim, brought forward).
