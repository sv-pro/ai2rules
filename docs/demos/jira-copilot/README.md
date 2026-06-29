# Demo: governed JIRA MCP on GitHub Copilot + Claude Code

**The pitch:** *"I can give GitHub Copilot access to JIRA and not worry about an
accidental destructive action."* One governance manifest **shapes the Atlassian/JIRA
MCP capability surface** for **VS Code Copilot, JetBrains Copilot, and Claude Code**
alike — read + comment only, every destructive JIRA tool **ABSENT** (it does not
exist for the agent — stronger than a deny it can argue with).

**The deeper point:** hosts are **not equal in governability**. The JIRA risk lives
behind an MCP server, so *every* host here can be made safe at that seam. But the
agent's **native** shell/file/web tools are governable only where the host exposes a
per-call hook — and **only Claude Code does**. The demo makes that gap visible
instead of leaving it to be discovered later. → [`SCORECARD.md`](SCORECARD.md).

Rust-only, one binary (`harness`). No Python, no Node, no creds — the upstream is a
self-contained mock JIRA. Real Atlassian is a later *skin* (E16.E), not a rewrite.

Decision: [`DECISIONS.md` D33](../../../DECISIONS.md) · Epic: [`PLAN.md` E16](../../../PLAN.md).

## Quick start (offline)

```bash
cargo build --offline    # builds `harness`
# MCP gateway over the mock JIRA, governed by the manifest:
harness mcp-gateway --world docs/demos/jira-copilot/jira-world.yaml -- harness mock-jira
#   add --taint tainted to demo the taint floor severing the external write
```

Drive it (newline-delimited JSON-RPC on stdin) or just run the tests:

```bash
cargo test -p cli-harness --test mcp_gateway --offline   # MCP seam (all hosts)
cargo test -p cli-harness --test cc_hook     --offline   # native seam (Claude Code)
```

## Architecture — one kernel, two seams, three hosts

```
                        native tools (shell/edit/web)        MCP tools (JIRA)
 Claude Code  ───────▶  PreToolUse → harness cc-hook ──┐     .mcp.json ─┐
 VS Code Copilot ──────  (no native seam)              │               ├─▶ harness mcp-gateway ─▶ upstream
 JetBrains Copilot ────  (no native seam)              │   .vscode/... ─┘   (shapes tools/list,    (mock-jira
                                                       └─────────────────▶   gates tools/call)     today; real
                                                          one kernel: gate(world, request)          Atlassian later)
```

- **`harness mcp-gateway`** sits between any host and the JIRA MCP server. It filters
  `tools/list` to the manifest's declared actions (**ABSENT** for everything else),
  routes each `tools/call` through the kernel (`gate()` — taint floor, args),
  forwards only **ALLOW** upstream, and can **audit** every decision. Host-agnostic.
- **`harness cc-hook`** is the Claude Code `PreToolUse` adapter, in Rust: it governs
  the host's *native* tools (the seam Copilot doesn't expose). Additive (only ever
  deny/ask) and fail-open. Replaces the old Python `world-gate-adapter.py`.

## What this proves to the audience

| Without governance | With the harness |
|---|---|
| Copilot sees the full JIRA MCP surface incl. `jira_delete_issue`, `jira_bulk_create_issues`, transitions | Copilot sees only reads + `jira_add_comment`; destructive tools **do not appear** |
| "Clean up old issues" can delete | "delete this issue" → the tool doesn't exist (ABSENT) |
| A tainted/untrusted context can still drive a write | Tainted session → `jira_add_comment` **DENIED** (taint floor) |
| On Claude Code, a tainted session can still `curl` out / `rm -rf` | `cc-hook` **denies** tainted egress, **asks** before destructive Bash |
| No record | Append-only audit log of every ALLOW/DENY/ABSENT |

## Files here

| File | Purpose |
|---|---|
| [`jira-world.yaml`](jira-world.yaml) | the demo manifest — harness `WorldManifest`; declares reads + `jira_add_comment`, everything else ABSENT |
| [`SCORECARD.md`](SCORECARD.md) | the governability scorecard + live runbook |
| [`run-proxy.sh`](run-proxy.sh) | launches `harness mcp-gateway` over `harness mock-jira` (the script hosts spawn) |
| [`hosts/claude-code.mcp.json`](hosts/claude-code.mcp.json) | Claude Code `.mcp.json` (MCP seam) |
| [`hosts/claude-code.settings.json`](hosts/claude-code.settings.json) | Claude Code `.claude/settings.json` PreToolUse hook (native seam) |
| [`hosts/vscode.mcp.json`](hosts/vscode.mcp.json) | VS Code Copilot `.vscode/mcp.json` (mock upstream) |
| [`hosts/vscode-atlassian.mcp.json`](hosts/vscode-atlassian.mcp.json) | VS Code Copilot `.vscode/mcp.json` for the **real** Atlassian upstream (E16.E) — see [`REAL-ATLASSIAN.md`](REAL-ATLASSIAN.md) |
| [`hosts/jetbrains.md`](hosts/jetbrains.md) | JetBrains Copilot MCP config |

> `jira-governed.world.yaml` and the Python-path notes from the earlier prototype
> are superseded by D33; the manifest is now [`jira-world.yaml`](jira-world.yaml).

## Status

- ✅ **E16.A** `harness mock-jira` — self-contained MCP stdio upstream (7 jira_* tools).
- ✅ **E16.B** `harness mcp-gateway` — real-kernel gateway (ABSENT-filter, gate, audit).
- ✅ **E16.C** `harness cc-hook` — native-tool governance for Claude Code.
- ✅ **E16.D** scorecard + runbook + host configs (this directory).
- ⏳ **E16.E** *(later)* real Atlassian upstream skin — set `UPSTREAM=(npx -y mcp-remote
  https://mcp.atlassian.com/v1/sse)` in `run-proxy.sh` once OAuth/creds are available;
  nothing else changes (the gateway and manifest are upstream-agnostic).

## Wiring real hosts

Replace `/ABS/PATH` in the `hosts/` configs with your checkout. For the **MCP seam**,
point each host at `run-proxy.sh`. For the **native seam** (Claude Code only), merge
`hosts/claude-code.settings.json`. Verify in each host's tool picker: only the read
tools + `jira_add_comment` appear; the destructive JIRA tools are absent.
