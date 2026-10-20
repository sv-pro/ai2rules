# GitHub Copilot support: gaps and recommendations

Status: analysis, not measurement. Written 2026-10-21 from a read of the repo
(`DECISIONS.md` D32, `docs/GOVERNABILITY-INDEX.md`, `docs/demos/jira-copilot/`,
`crates/cli-harness/src/init.rs`, `.vscode/mcp.json`). Nothing here was executed; claims
about Copilot behaviour come from vendor documentation as recalled and from copilot-cli#4001,
and must be verified before they become `✓` cells in the governability index.

Scope: **Copilot CLI** and **VS Code Copilot** (agent mode). JetBrains and the cloud coding
agent are noted where they change a recommendation.

## What exists today

| Surface | Support |
|---|---|
| MCP surface (VS Code, JetBrains, CLI) | `harness mcp-gateway` shapes `tools/list`; host configs in `docs/demos/jira-copilot/hosts/`; committed `.vscode/mcp.json` |
| Copilot CLI native hook | Incidental only: the `.claude/settings.json` shim carries workarounds for copilot-cli#4001 (PowerShell, cwd `/`, no `$CLAUDE_PROJECT_DIR`) |
| Native-tool gate (`copilot-hook`) | None |
| Tests / benchmark | None for any Copilot host |

## Gaps

### G-1. D32 and the scorecard contradict the governability index
D32 and `SCORECARD.md` state Copilot has "no native seam". `GOVERNABILITY-INDEX.md` records
that GitHub documents a CLI hook system, that it ran on Windows (copilot-cli#4001), and that it
did not fire on CLI 1.0.79 / Linux. The project's own positioning is therefore stale or wrong
in one place.

**Recommend:** amend D32 (new decision, do not rewrite history) to "MCP-first, native hook
under evaluation", and update the scorecard's Copilot columns to `○ / unmeasured` rather than
"no seam".

### G-2. No native-tool adapter for Copilot
There is `cc-hook` and `agy-hook` over a shared `hostkit`, but no Copilot adapter, no
`.github/hooks/*.json`, and no Copilot world manifest. Native shell and file actions on
Copilot are ungoverned.

**Recommend:** add `harness copilot-hook` in `cli-harness`, reusing `hostkit` shape helpers
and the shared `command_classes` (pinned byte-identical by `tests/one_kernel.rs`). Keep the
rule that no governance logic lives in the shim.

### G-3. Payload and verdict shape differences are unmapped
Per vendor docs (verify): CLI uses camelCase `preToolUse`, a `toolName` / `toolArgs` payload
(args as a JSON string), tool names such as `bash` / `edit` / `create`, and honours only
`deny`. VS Code agent hooks follow the Claude-style `PreToolUse` with
`allow` / `deny` / `ask` and optional input rewriting. None of this is normalised.

**Recommend:** write a short contract note beside `docs/demos/antigravity/` (the verified
hook contract precedent), with captured real payloads from each host, and map Copilot tool
names to the existing action classes. Treat `ask` and `allow` as unsupported on the CLI until
shown otherwise; the kernel's approval flow must degrade to `deny` plus an actionable reason.

### G-4. VS Code agent hooks are not evaluated at all
The scorecard's VS Code column asserts "no native seam" without testing hooks. Unknown:
whether VS Code reads `.claude/settings.json`, `.github/hooks`, or its own location; whether
hooks cover MCP tools as well as native tools (G4); whether config reloads live (G9).

**Recommend:** run the G1–G9 probes on VS Code with a canary hook and record results with
dates and versions, following the index's `✓ / ○ / ?` convention.

### G-5. Copilot CLI G1 is unresolved (version / platform split)
The hook did not fire on 1.0.79 / Linux with a schema-valid file, and Copilot's log never
mentioned hooks. Cause unknown (rollout, platform, flag).

**Recommend:** re-measure on the current CLI release on Linux, macOS and Windows; try
user-level (`~/.copilot/`) as well as repo-level config; check for an experimental flag or
trust prompt in the release notes; file or link an upstream issue with the minimal repro.

### G-6. Silent-failure mode: governed on paper, ungoverned in practice
A skipped hook is invisible (the same class as Codex's untrusted-hook skip). The shim
fails open when the binary is missing; on Windows/PowerShell Copilot fails closed and bricks
every call.

**Recommend:** add a canary check to `harness doctor` (see
`docs/harness-doctor-schema-v0alpha3.md`) that proves the hook fires for the host, e.g. a
sentinel call whose denial is observable. Add a Copilot Windows smoke test to cover #4001's
failure class (`demo-platform-smoke.yml` is the existing pattern).

### G-7. `harness init` wires Claude Code only
It writes `.claude/`, the shim and the `.gitignore` line. It does not emit the Copilot hook
file, `.github/copilot-instructions.md`, or Copilot CLI MCP config. Copilot reads `AGENTS.md`
natively, so instructions are partly covered.

**Recommend:** `harness init --host copilot` (and `--host all`) that writes `.github/hooks/`
config with the single-quoted absolute command form already worked out in `init.rs`, and
prints the MCP gateway snippet for VS Code (`.vscode/mcp.json`) and CLI.

### G-8. Self-protection is Claude-specific
The kill-switch and state live under `~/.claude/...`. A Copilot session could edit its own
hook file, manifest or `.github/hooks/` unless those paths are governed.

**Recommend:** deny or hold-for-approval writes to hook config, the manifest and state for
Copilot, as `cc-world.yaml` does; move the kill-switch convention to a host-neutral path.

### G-9. No tests, benchmark target or CI for Copilot
No Copilot case in the one-kernel conformance suite; `govbench` has `WeakGateway` and
`Ai2rules` only; no path-filtered integration workflow.

**Recommend:** add Copilot payload fixtures to the shared case set; a `govbench` target that
drives `mcp-gateway` as Copilot would (cheap, no vendor dependency); and a
`copilot-integration.yml` modelled on `deepseek-integration.yml`, pinned to a CLI version.

### G-10. Cloud coding agent and CLI MCP config are not covered
No `copilot-setup-steps`, no governed MCP config for the cloud agent, no egress story.

**Recommend:** lower priority. Document it as out of scope in the index, or add a setup-steps
example that installs the harness and routes MCP through the gateway.

## Recommended order

1. **Measure** (G-4, G-5): VS Code and CLI hooks, G1–G4, dated and versioned. Everything
   else depends on whether a native seam exists.
2. **Fix the record** (G-1): D32 amendment, scorecard, index cells.
3. **Adapter and contract** (G-2, G-3): `copilot-hook` plus captured payloads, with the CLI's
   deny-only limit designed in.
4. **Safety** (G-6, G-8): doctor canary, self-protection, Windows smoke test.
5. **Packaging and tests** (G-7, G-9): `init --host copilot`, conformance cases, benchmark
   target, CI.
6. **Later** (G-10): cloud agent.

## Housekeeping when this becomes work

Per `AGENTS.md`: track in both GitHub and Linear, add a `D<n>` entry in `DECISIONS.md` for
G-1, update README and `PLAN.md` checkboxes, and re-run `harness-preview` / WASM and
governance-benchmark checks if the kernel or ABI changes.
