# D35 — OpenCode target uses plugin `tool.execute.before` + host permissions, not a forked policy engine


**Date:** 2026-06-30. **Extends:** D24 / D34 to a non-Rust host adapter.

- **Context:** OpenCode exposes two useful control planes: config-level `permission` rules
  (`allow` / `ask` / `deny`) and TypeScript/JavaScript plugins with a `tool.execute.before`
  hook that can inspect/mutate tool arguments or throw to block execution. This is close enough
  to Claude Code's `PreToolUse` to govern native tools, but it is not the same structured hook
  protocol: there is no documented `permissionDecision: allow|deny|ask` return value from the
  plugin hook itself.
- **Decision:** add OpenCode as a planned host target (**E17**) through a thin
  `.opencode/plugins/` adapter. The first slice calls the existing **`harness gate` wire ABI**
  as an out-of-process/non-Rust host (per D34), persists monotonic taint in an OpenCode sidecar,
  lets `ALLOW` continue, and blocks `DENY` / `ABSENT` / `REPLAN` by throwing. `ASK` is delegated
  to OpenCode's `permission` layer where possible, and otherwise surfaced as an explicit block
  until a cleaner approval UX is proven. The plugin must not reimplement taint, policy, or
  descriptor logic.
- **Why:** this keeps the architecture's one-kernel rule intact while expanding beyond Claude
  Code. It also enriches the E16 governability-gap story with a third native-tool host class:
  Claude Code has structured hook decisions; OpenCode has a powerful pre-execute plugin seam
  plus permissions; Copilot/JetBrains remain MCP-only for now.
- **Alternatives rejected:** fork OpenCode or embed governance inside its source tree (too heavy,
  host-specific, and not a plugin product); write a standalone JS policy engine in the plugin
  (guaranteed drift from `world-kernel`); rely only on OpenCode `permission` patterns (useful
  defense-in-depth, but cannot express the compiled-world / taint / replay model); make WASM the
  first slice (interesting later, but subprocess `harness gate` is simpler and is already the
  conformance ABI for non-Rust hosts).
- **Related:** **D24** (gate ABI), **D34** (non-Rust hosts use the wire ABI), **E17**
  (OpenCode Governance Pack), **E16** (host governability scorecard).
