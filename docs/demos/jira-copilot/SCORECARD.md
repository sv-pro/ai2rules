# Governability scorecard — Claude Code vs GitHub Copilot

> The point of the demo is **not** "Copilot is bad." It's that **hosts are not
> equal in governability** — and that's invisible until you try to put a guardrail
> on one. Same governance manifest, same kernel; the *depth* you can reach differs
> by host. Know the gap before you hand an agent your JIRA.

## The two governance seams

An agent host has two places a third party can interpose policy:

1. **Native tools** — the host's own built-ins (shell, file edit/write, web fetch).
   Governable only if the host exposes a **per-call hook** over them.
2. **MCP tools** — anything reached through a Model Context Protocol server.
   Governable by anyone, because you can put a **gateway** between the host and the
   MCP server. Host-agnostic by construction.

The harness occupies both seams with one kernel:

| Seam | Harness component | What it enforces |
|---|---|---|
| Native tools | `harness cc-hook` (PreToolUse adapter) | taint floor (no tainted egress), destructive Bash → ask, additive + fail-open |
| MCP tools | `harness mcp-gateway` | `tools/list` shaping (ABSENT), per-call gate, forward only ALLOW, audit |

## The scorecard

| Capability | Claude Code | VS Code Copilot | JetBrains Copilot |
|---|:--:|:--:|:--:|
| Per-call hook over **native** tools (shell/edit/web) | ✅ `PreToolUse` → `harness cc-hook` | ❌ none exposed | ❌ none exposed |
| → taint floor on native egress (tainted ⇒ no `curl`/`wget`/…) | ✅ | ❌ | ❌ |
| → approval gate on destructive Bash (`rm -rf`, `sudo`, …) | ✅ | ❌ | ❌ |
| Shape an **MCP** server's `tools/list` (destructive ⇒ ABSENT) | ✅ `harness mcp-gateway` | ✅ `harness mcp-gateway` | ✅ `harness mcp-gateway` |
| Gate each **MCP** `tools/call` (taint floor, args) | ✅ | ✅ | ✅ |
| Append-only **audit** of every decision | ✅ | ✅ | ✅ |
| **Governance reach** | **deep** (whole agent) | **MCP surface only** | **MCP surface only** |

Reading it: **the MCP row is identical across all three hosts** — that's why the
JIRA pitch ("destructive tools don't exist for the agent") holds for Copilot just
as well as for Claude Code. The **native rows are where Copilot has no seam**: a
shell command or a file write inside Copilot is ungoverned by any third party. On
Claude Code the *same kernel* also covers those.

So: if the risk you care about lives behind an MCP server (JIRA, GitHub, a
database), **every host here can be made safe**. If it lives in the agent's native
shell/filesystem, **only Claude Code can**. That is the governability gap, stated
precisely — and the argument for broader Claude Code use where native-tool blast
radius matters.

## Against real Atlassian (E16.E)

The table above is proven offline against `harness mock-jira`, but it holds **identically**
against the live **Atlassian Rovo MCP Server** (`https://mcp.atlassian.com/v1/mcp/authv2`,
OAuth 2.1) — the gateway and kernel don't change, only the upstream and the manifest's tool
names (`jira-atlassian.world.yaml`). Two things get *sharper* on the real upstream:

- **The surface shrink is far more dramatic.** Rovo advertises **~50 tools** across Jira /
  Confluence / Bitbucket / JSM / Compass; the manifest exposes **5** (3 reads +
  `getAccessibleAtlassianResources` + `addCommentToJiraIssue`). Every transition/edit/create
  and every non-Jira tool is `ABSENT`. The mock's 7→4 shrink becomes **~50→5**.
- **There is no delete tool.** Rovo exposes none, so the destructive beat is
  `transitionJiraIssue` / `editJiraIssue` / `createJiraIssue`, not `jira_delete_issue`.

The **deep-vs-shallow gap is unchanged — and that's the point.** Against production Atlassian,
all three hosts shape the MCP surface identically (the JIRA risk lives behind MCP, so Copilot
is safe where it counts), but only **Claude Code** also governs its own native shell/file/web
via `cc-hook` on the **same kernel** (`.claude/cc-world.yaml`). See
[`REAL-ATLASSIAN.md`](REAL-ATLASSIAN.md) for the live runbook on both hosts.

## Reproduce it (offline, no creds)

```bash
cargo build --offline                     # builds the `harness` binary
```

**MCP seam — shaped JIRA surface (all three hosts):**

```bash
# destructive tools are ABSENT; reads + comment ALLOW; tainted comment DENIED
cargo test -p cli-harness --test mcp_gateway --offline
# or drive it by hand:
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"jira_delete_issue","arguments":{}}}' \
  | bash docs/demos/jira-copilot/run-proxy.sh
# id:2 lists only read tools + jira_add_comment; id:3 -> isError "ABSENT"
# add TAINT=tainted to sever the external write (jira_add_comment -> DENY)
```

**Native seam — Claude Code only:**

```bash
cargo test -p cli-harness --test cc_hook --offline
# proves: tainted egress -> deny (taint floor); rm -rf -> ask; clean read -> passthrough
# manual: a PreToolUse event in, a decision out
echo '{"tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"},"session_id":"demo"}' \
  | harness cc-hook --world .claude/cc-world.yaml --state /tmp/ccdemo
# -> {"hookSpecificOutput":{"permissionDecision":"ask",...}}
```

## Wiring real hosts

- **MCP (all hosts):** point the host at `run-proxy.sh` — see
  [`hosts/claude-code.mcp.json`](hosts/claude-code.mcp.json),
  [`hosts/vscode.mcp.json`](hosts/vscode.mcp.json), [`hosts/jetbrains.md`](hosts/jetbrains.md).
- **Native (Claude Code only):** merge [`hosts/claude-code.settings.json`](hosts/claude-code.settings.json)
  into `.claude/settings.json` to add the `harness cc-hook` PreToolUse hook.

## Narrative arc — story-driven script (~8 min)

A pedagogical alternative to the runbook below: builds from first principles
rather than leading with fear. Better for an audience unfamiliar with MCP.

**1. Context — what are MCP tools? (1 min)**
Explain that modern AI coding assistants (Copilot, Claude Code) expose two kinds
of tools: *native* built-ins (shell, file edit, web fetch) and *MCP tools* —
capabilities served over the Model Context Protocol by an external server. Show
the VS Code Copilot tool picker with the raw JIRA MCP wired in: the agent sees
`jira_delete_issue`, `jira_bulk_create_issues`, transitions. Note that individual
tools can be toggled off manually — but that's per-session, per-developer, and
error-prone.

**2. The problem — full surface, no policy (1 min)**
Repeat on Claude Code: same raw JIRA MCP, same sprawling tool list. Then show how
Claude Code's tool surface is *configured* (`.mcp.json`). The risk: "tidy up
stale issues" is a perfectly natural prompt — and the agent now has the tool to
act on it destructively.

**3. Hooks — what Claude Code adds (1.5 min)**
Claude Code exposes a `PreToolUse` hook: a per-call interception point that fires
*before* any tool executes. Show `hosts/claude-code.settings.json` — a single
entry wires the harness as the hook handler. Copilot has no equivalent seam:
native shell and file calls are opaque to third parties. *"So if I want consistent
policy across both hosts, where does it live?"*

**4. The answer — one proxy, both hosts (2 min)**
Switch both hosts to point at `run-proxy.sh` instead of the raw MCP server.
Re-open the tool picker on VS Code Copilot: destructive tools are gone — not
disabled, not denied — they *do not appear*. Ask the agent to delete an issue
→ "I don't have that tool." On Claude Code: same tool list. One manifest
(`jira-world.yaml`), two hosts, same governed surface.

**5. Taint floor — untrusted context can't drive an external write (1 min)**
Run with `TAINT=tainted` (simulates a context that read from an untrusted source).
`jira_add_comment` — which was ALLOW — is now **DENY**. The taint floor severs the
write: the MCP surface is shaped by the manifest, but the *session context* also
gates each call. Show the audit log entry.

**6. Going deeper — native shell on Claude Code only (1 min)**
Enable the `cc-hook` PreToolUse hook. Fetch a web page (taints the session).
Now ask for `curl` → **denied** at the native shell — not at the MCP layer but
*before the subprocess spawns*. Ask for `rm -rf` → **ask**. Point at Copilot:
*there is no seam here for third-party policy, regardless of what you configure.*

**7. The scorecard (30 s)**
Show the table above. Close with: *"Same kernel. On Copilot I'm safe at the MCP
surface — that's where the JIRA risk lives, so it's enough. On Claude Code I'm
safe everywhere, including the native shell. The gap is the host, not the
governance engine."*

| Asset | Step |
|---|---|
| `hosts/vscode.mcp.json` (raw) | 1, 2 |
| `hosts/claude-code.mcp.json` (raw) | 2 |
| `hosts/claude-code.settings.json` | 3 |
| `jira-world.yaml` + `run-proxy.sh` | 4 |
| `TAINT=tainted run-proxy.sh` | 5 |
| `harness cc-hook --world .claude/cc-world.yaml` | 6 |
| this scorecard table | 7 |

---

## Runbook (live demo, ~6 min)

1. **The fear (30s).** Point a host at the JIRA MCP *directly*; the tool list
   includes `jira_delete_issue`, `jira_bulk_create_issues`, transitions. "Tidy up
   stale issues" *can* delete.
2. **Shape the MCP surface (2m).** Switch the host to `run-proxy.sh`. Re-open the
   tool picker → only reads + `jira_add_comment`. Ask to delete → *the tool does
   not exist* (ABSENT, not a deny it can argue with). Comment on `DEMO-1` → works.
   Repeat on a second host to make the host-agnostic point.
3. **Sever a tainted write (1m).** Run with `TAINT=tainted` → `jira_add_comment`
   now DENIES (taint floor): untrusted context can't drive an external write.
4. **Go deep on Claude Code (2m).** Enable the `cc-hook` PreToolUse hook. Fetch a
   web page (taints the session), then ask for a `curl` → **denied** at the native
   shell. Ask for `rm -rf` → **ask**. Point out: *Copilot has no seam for this.*
5. **The scorecard (30s).** Show this table. The line: *"Same kernel. On Copilot
   I'm safe at the MCP surface; on Claude Code I'm safe everywhere."*
