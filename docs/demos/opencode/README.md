# Demo: governing OpenCode's native tools (E17 / D35)

**The point:** the *same* `WorldManifest` and the *same* kernel that govern Claude Code
(`harness cc-hook`) and any MCP server (`harness mcp-gateway`) also govern **OpenCode's
native tools** — through a thin `.opencode/plugin/` adapter that calls the `harness gate`
wire ABI (D24). No policy or taint logic is reimplemented in the plugin (D35).

This adds a **third host class** to the E16 governability scorecard:

| Host | Native-tool seam | How governed |
|---|---|---|
| **Claude Code** | `PreToolUse` hook with structured `allow/deny/ask` | `harness cc-hook` (in-process) |
| **OpenCode** | `tool.execute.before` plugin hook (block by **throwing**; no structured return yet) | `.opencode/plugin/ai2rules-gate.ts` → `harness gate` |
| **Copilot / JetBrains** | none | MCP surface only (`harness mcp-gateway`) |

OpenCode being **open source** is the strategic point: even though `tool.execute.before`
lacks a structured `allow/deny/ask` return today (so `ASK` is surfaced as a block in this
first slice), the seam is open and extensible — not a vendor gate you must wait on.

## How it works

`.opencode/plugin/ai2rules-gate.ts` hooks `tool.execute.before` on every OpenCode tool call:

1. Sends the **raw** OpenCode tool name — since **D36** the *kernel* classifies `bash`
   by command shape into `bash_network` / `bash_destructive` / `bash_unclassified`
   from the world's `command_classes` block (no pattern lists live in the plugin);
   everything else maps 1:1.
2. Builds a `GateRequest` and shells to `harness gate --world opencode-world.yaml` (the wire
   ABI — D34: non-Rust hosts use the subprocess). `AI2RULES_MODE=background` threads
   `context.mode` so the kernel collapses ASK→DENY unattended.
3. **`ALLOW`** → returns, OpenCode runs the tool. **`DENY` / `ABSENT` / `REPLAN` / `ASK`** →
   **throws** with the kernel's decision label + effective action (`verdict.action`).
4. Persists monotonic session taint in `.opencode/ai2rules-state.json`.
5. **Fail-open:** any adapter/process error logs a warning and allows — only an explicit
   kernel verdict blocks, so a broken gate never bricks a session.

## Proven decisions

Verified via `harness gate` against [`opencode-world.yaml`](opencode-world.yaml) for the
action names the plugin sends:

```
  read               clean    -> ALLOW
  grep               clean    -> ALLOW
  webfetch           clean    -> ALLOW          (escalates session taint)
  webfetch           tainted  -> DENY   taint_invariant       ← taint floor severs egress
  edit               tainted  -> ALLOW                        ← workspace writes aren't egress
  bash_destructive   clean    -> ASK    approval_required     ← rm -rf / sudo / mkfs …
  bash_network       tainted  -> DENY   taint_invariant       ← curl / wget / ssh … after taint
  bash_unclassified  clean    -> ASK    approval_required     ← unmatched shell fails closed
  task               clean    -> ALLOW
  <unknown tool>     clean    -> ABSENT unknown_to_ontology   ← not in this world
```

## Files

| File | Purpose |
|---|---|
| [`opencode-world.yaml`](opencode-world.yaml) | the `WorldManifest` governing OpenCode's native tools |
| [`../../../.opencode/plugin/ai2rules-gate.ts`](../../../.opencode/plugin/ai2rules-gate.ts) | the live plugin adapter (dogfoods this repo, like `.claude/`) |

## Try it

```bash
cargo build --offline                       # builds the `harness` binary
opencode                                     # launch OpenCode in this repo
```

The plugin at `.opencode/plugin/ai2rules-gate.ts` auto-loads for this project (OpenCode
loads project plugins from `.opencode/plugin/`). Then, inside OpenCode:

- Ask it to **read/grep** files → allowed.
- Ask it to **fetch a web page**, then run a shell **`curl`** → the curl is **blocked**
  (taint floor: the fetch tainted the session).
- Ask it to **`rm -rf`** something → **blocked** (destructive → ASK, surfaced as a block).

Disable temporarily with `AI2RULES_DISABLE=1 opencode`. Point at a different manifest with
`AI2RULES_WORLD=/path/to/world.yaml`, or a specific binary with `AI2RULES_HARNESS=/path/to/harness`.

> **Plugin directory:** this repo uses `.opencode/plugin/` (singular — the dominant
> convention in OpenCode 1.17). If your version doesn't auto-load it, add it explicitly to
> `opencode.jsonc`: `{ "plugin": ["./.opencode/plugin/ai2rules-gate.ts"] }`.

**To govern another repo:** copy `.opencode/plugin/ai2rules-gate.ts` and an
`opencode-world.yaml` into it (set `AI2RULES_WORLD` if the path differs), and make `harness`
reachable (on `PATH` or via `AI2RULES_HARNESS`).

## Defense-in-depth: OpenCode `permission` rules (E17.4)

The plugin is the *kernel* layer. OpenCode's own coarse `permission` rules are a useful
host layer on top — they give an interactive `ask` UX the plugin can't (yet). An example
`opencode.jsonc`:

```jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "permission": {
    "edit": "allow",
    "webfetch": "ask",
    "external_directory": "deny",
    "bash": {
      "*": "ask",
      "git status": "allow",
      "git diff*": "allow",
      "cargo *": "allow",
      "rm *": "deny",
      "sudo *": "deny"
    }
  }
}
```

Kernel decisions and host permissions reinforce each other: the kernel enforces the
compiled-world / taint / ABSENT model; OpenCode permissions add a familiar allow/ask/deny UX.

## Status (E17)

- ✅ **E17.1** Host mapping note — tool/event shapes + `GateRequest` mapping (this README + plugin).
- ✅ **E17.2** Demo world + runbook ([`opencode-world.yaml`](opencode-world.yaml) + this file).
- ✅ **E17.3** Minimal plugin adapter ([`ai2rules-gate.ts`](../../../.opencode/plugin/ai2rules-gate.ts)).
- ✅ **E17.4** Defensive `opencode.jsonc` example (above).
- ✅ **E17.5** Rust contract tests — `crates/cli-harness/tests/opencode_world.rs` (gate verdicts
  against this world) + D36 classification golden vectors in `harness-preview` gate tests (incl. the
  substring-false-positive regression). 123 tests green.
- ⏳ **E17.6/E17.7** `harness opencode-init` emitter; optional WASM/in-process adapter.
