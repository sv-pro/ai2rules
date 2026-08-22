# D19 — Govern Claude Code by emitting config from one WorldManifest

- **Epic:** E13 · **Status:** Proposed (sketch; E13.2 first slice landed, emitter is E13.3)
- **Decision:** Apply the harness to the **Claude Code** host by *compiling* one
  `WorldManifest` into Claude Code config, rather than hand-authoring that config
  or reimplementing governance in JS. A `harness compile --target claude-code`
  subcommand emits, from the same `CompiledWorld` the harness runs on:
  - **`.claude/cc-world.json` + a `PreToolUse` hook** (`world-gate.py`) — the
    runtime `decide()` gate over **native** tools: ABSENT-for-native, the taint
    floor (`transition_policies`), and ASK (`approval_required`). *(E13.2, shipped
    as a hand-written first slice.)*
  - **`.mcp.json` → an MCP projection shim** — projection + scoped-capability
    arg-locking for MCP tools, reusing `safe-mcp-proxy` / `mcp-tool-projection`.
    *(E13.4.)*
  - **subagent `tools` allowlists** — one subagent per trust level (the
    capability-by-trust matrix → distinct projected surfaces).
  - optionally a **`PostToolUse` logging hook** for audit/trace parity + redaction.
- **Manifest → host mapping:** projected actions + capability matrix → subagent
  allowlists / `cc-world.projected_tools`; `transition_policies` (taint ×
  side-effect) → `cc-world.egress`/`taint_sources`; `approval_required` →
  `cc-world.ask`; `scoped_capabilities` literals → MCP-shim re-exposed schemas;
  `observability.redact` → the PostToolUse logger.
- **Alternatives:** (1) hand-author `settings.json` + `.mcp.json` + allowlists
  separately — *this is the drift problem we exist to solve*; (2) one big Claude
  Code **plugin** bundling agents/commands/hooks/MCP — viable later as the
  distribution wrapper, but still wants a single compiled source; (3) reimplement
  the kernel logic inside the hook in JS/TS — rejected for the same reason as
  D17/D18 (double maintenance, inevitable drift).
- **Why:** one `CompiledWorld` is the single source of truth, so Claude Code's
  otherwise-scattered governance (settings permissions + `.mcp.json` + subagent
  allowlists + hooks) can't drift; the emitter is a **pure projection**
  (deterministic, no LLM); and the hook layer governs **native** tools
  (`Bash`/`Edit`/`Write`/`Read`/`WebFetch`) that an MCP proxy alone can't see —
  the highest-leverage gap. It also dogfoods the harness on its own repo.
- **Known limits (host fidelity):** `PreToolUse` gates (allow/deny/ask) but does
  not reliably *rewrite* native-tool args — so scoped-cap arg-locking lives in the
  MCP shim, while native tools are validate-and-deny. Taint is heuristic on this
  host (inferred from which tool touched an untrusted source; monotonicity kept in
  the sidecar). Fidelity is highest for ABSENT (surface), the taint floor, and
  ASK — exactly the three the E13.2 slice ports.
