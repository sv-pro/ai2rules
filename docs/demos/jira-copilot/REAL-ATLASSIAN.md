# Running the demo against REAL Atlassian (E16.E)

This is the live version of the JIRA governability demo, pointed at the **Atlassian
Rovo MCP Server** instead of `harness mock-jira`. Nothing in the gateway or kernel
changes — only the *upstream* and the *manifest's tool names*.

> **Auth boundary:** the Rovo server uses **OAuth 2.1**; you sign in through your own
> browser when `mcp-remote` opens the consent page. Do this yourself — don't paste
> tokens or credentials into the manifest or scripts.

## Verify the shaping offline first (no Atlassian needed)

Everything that *sells* this demo — the shaped surface, the ABSENT destructive tools, the
taint floor — is decided at the gate **before** any call reaches Atlassian, so it's proven
offline against a stand-in that speaks the real Rovo tool names:

```bash
cargo test -p cli-harness --test mcp_gateway_atlassian --offline
```

That test drives this exact manifest (`jira-atlassian.world.yaml`) through the gateway over
`harness mock-jira --rovo` — which advertises the genuine Rovo names (`getJiraIssue`,
`transitionJiraIssue`, `addCommentToJiraIssue`, …) plus a Confluence tool as noise — and
asserts the exposed surface is exactly the **5** declared tools, that `transitionJiraIssue`
and `getConfluencePage` are **ABSENT**, and that a **tainted** session severs the comment
write. If Atlassian renames a tool and this manifest drifts, the test goes red — you find out
in CI, not in front of an audience. Only the **positive forwarded read/write** below needs a
real Atlassian; the refusal/absence/taint story is green with no creds.

## What's different from the mock path

| | Mock (`jira-world.yaml`) | Real (`jira-atlassian.world.yaml`) |
|---|---|---|
| Upstream | `harness mock-jira` (stdio) | Rovo MCP at `https://mcp.atlassian.com/v1/mcp/authv2` (HTTP+OAuth) via an `mcp-remote` stdio bridge |
| Read tools | `jira_get_issue`, `jira_search_issues_using_jql`, `jira_get_project` | `getJiraIssue`, `searchJiraIssuesUsingJql`, `getVisibleJiraProjects` |
| The one write | `jira_add_comment` | `addCommentToJiraIssue` |
| "Scary" ABSENT tools | `jira_delete_issue`, `jira_transition_issue`, `jira_bulk_create_issues` | `transitionJiraIssue`, `editJiraIssue`, `createJiraIssue` (**no delete tool exists** in Rovo) |

The gateway speaks **stdio** to its upstream, but Rovo is remote HTTP+OAuth — so an
`mcp-remote` bridge sits in between to do the transport + OAuth dance and expose stdio
downward. The gateway is unchanged.

## Prerequisites

- The `harness` binary built: `cargo build --offline` (from the repo root).
- **Node.js** on PATH (for `npx mcp-remote`).
- An **Atlassian Cloud** site you can sign into, with a Jira project + at least one
  issue (note its key, e.g. `DEMO-1`).

## Run it

From the repo root:

```bash
MANIFEST=docs/demos/jira-copilot/jira-atlassian.world.yaml \
UPSTREAM="npx -y mcp-remote https://mcp.atlassian.com/v1/mcp/authv2" \
bash docs/demos/jira-copilot/run-proxy.sh
```

On first launch `mcp-remote` opens a browser for OAuth — **sign in and approve**.
After consent it caches the token (`~/.mcp-auth`), so later runs are non-interactive.

`run-proxy.sh` reads JSON-RPC from stdin, so to drive it by hand pipe requests in:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"transitionJiraIssue","arguments":{}}}' \
  | MANIFEST=docs/demos/jira-copilot/jira-atlassian.world.yaml \
    UPSTREAM="npx -y mcp-remote https://mcp.atlassian.com/v1/mcp/authv2" \
    bash docs/demos/jira-copilot/run-proxy.sh
```

### What proves the point (and needs no valid args)

DENY and ABSENT verdicts happen at the **gate, before any upstream call** — so they
don't depend on correct arguments or even a reachable Jira:

1. **`tools/list` (id 2)** → the Rovo server advertises ~50 tools across Jira /
   Confluence / Bitbucket / Compass. The gateway returns only the **4** in the
   manifest: `getJiraIssue`, `searchJiraIssuesUsingJql`, `getVisibleJiraProjects`,
   `addCommentToJiraIssue` (+ `getAccessibleAtlassianResources` if you kept it). Every
   write/transition/delete-equivalent is **gone**.
2. **`transitionJiraIssue` (id 3)** → `isError`, text contains **`ABSENT`**. The
   destructive tool does not exist for the agent — nothing is forwarded to Atlassian.
3. **Taint floor:** add `TAINT=tainted` to the env and call `addCommentToJiraIssue` →
   **`DENY` / `no_tainted_external`**, again before any upstream call. An untrusted
   context can't drive the external write.

### The positive (ALLOW) path

To actually read or comment, the call is **forwarded** to Atlassian, so it needs valid
arguments — including a **`cloudId`**. Get it first:

```bash
'{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"getAccessibleAtlassianResources","arguments":{}}}'
# -> returns your site(s) and their cloudId
'{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"getJiraIssue","arguments":{"cloudId":"<id>","issueIdOrKey":"DEMO-1"}}}'
# -> ALLOW, forwarded; returns the real issue
```

> Hand-crafting args is fiddly. The schema-aware way to drive the ALLOW path is to wire
> the governed gateway into a host (Claude Code: [`hosts/claude-code-atlassian.mcp.json`](hosts/claude-code-atlassian.mcp.json),
> the real-name counterpart of the mock `claude-code.mcp.json` — same `run-proxy.sh`, with the
> `MANIFEST` + `UPSTREAM` env vars set) and just ask in natural language. The host reads each
> tool's real schema, so you don't guess argument names. The shaping/gating is identical either way.

## VS Code Copilot against real Atlassian

The host spawns `run-proxy.sh` as a stdio MCP server; you point it at the real upstream
purely through the per-server `env` block in `.vscode/mcp.json` — no script edits. Use
[`hosts/vscode-atlassian.mcp.json`](hosts/vscode-atlassian.mcp.json) (copy it to
`.vscode/mcp.json` in your workspace):

```jsonc
{
  "servers": {
    "jira-governed-real": {
      "type": "stdio",
      "command": "bash",
      "args": ["${workspaceFolder}/docs/demos/jira-copilot/run-proxy.sh"],
      "env": {
        "MANIFEST": "${workspaceFolder}/docs/demos/jira-copilot/jira-atlassian.world.yaml",
        "UPSTREAM": "npx.cmd -y mcp-remote https://mcp.atlassian.com/v1/mcp/authv2"
      }
    }
  }
}
```

Start the server from the MCP view (or on first tool use). `mcp-remote` opens a browser
for OAuth — sign in and approve; the token caches under `~/.mcp-auth`. When connected,
the log reads `Discovered 5 tools` and the Copilot tool picker shows only the 5 governed
tools — the ~50-tool Rovo surface (every transition/edit/create, all Confluence/Bitbucket/
JSM/Compass) is filtered out at the gate.

Then drive it in Copilot Chat (agent mode), no hand-crafted args — Copilot reads each
tool's real schema:

1. **ABSENT refusal** — "move DEMO-1 to Done" / "create a bug in DEMO" / "edit DEMO-1's
   summary" → the tool doesn't exist for the agent; nothing is forwarded to Atlassian.
2. **ALLOW path** — "show me DEMO-1" (forwarded read) / "comment 'triaged' on DEMO-1"
   (forwarded write). Copilot calls `getAccessibleAtlassianResources` to resolve `cloudId`
   itself.
3. **Taint floor** — add `"TAINT": "tainted"` to the `env` block, restart the server, ask
   it to comment → `DENY` / `no_tainted_external`, at the gate before any upstream call.
   Reads still work (the floor only severs the `External` write).
4. **Audit** *(optional)* — add `"AUDIT": "${workspaceFolder}/jira-audit.jsonl"` to log
   every ALLOW/DENY/ABSENT decision as JSONL.

> **Scope caveat:** this governs the **MCP seam only**. VS Code Copilot exposes no
> per-call hook for its *native* shell/file/web tools, so those stay ungoverned — the
> gap only Claude Code closes via `cc-hook` (see [`SCORECARD.md`](SCORECARD.md)). The JIRA
> risk lives behind MCP, so it is fully governed here regardless.

## Gotchas

- **Exact tool names matter.** The gateway matches `tools/list` and `tools/call` by
  verbatim name. If Atlassian renames a tool, update `jira-atlassian.world.yaml` to
  match (re-check the [Supported tools](https://support.atlassian.com/atlassian-rovo-mcp-server/docs/supported-tools/) doc).
- **`cloudId` is required** by most Rovo Jira tools. Keep `getAccessibleAtlassianResources`
  in the manifest (it's there by default) so the agent can resolve it, or pass it by hand.
- **No delete tool.** Don't promise "watch it refuse to delete" — Rovo has no delete-issue
  tool. Demo the refusal on `transitionJiraIssue` / `editJiraIssue` / `createJiraIssue`.
- **Token cache.** `mcp-remote` caches OAuth under `~/.mcp-auth`; delete it to force a
  fresh sign-in during a live demo.
- **Windows: use `npx.cmd`, not `npx`.** `harness mcp-gateway` spawns the upstream
  directly (no shell), so the bare `npx` — a `.cmd` shim with no extension — fails with
  `cannot start upstream … program not found`. Set `UPSTREAM="npx.cmd -y mcp-remote …"`.
  On macOS/Linux use `npx`.
