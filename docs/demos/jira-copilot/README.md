# Demo: governed JIRA MCP on GitHub Copilot + Claude Code

> **⚠ Pivoted — now Rust-only, one binary (DECISIONS D33).** The Python `safe-mcp-proxy`
> path described further down is the *superseded prototype*. The live demo runs the **real
> kernel** via `harness mcp-gateway` over a self-contained `harness mock-jira` upstream — no
> creds, no Node, no Python. Quick start (from the repo root, after `cargo build`):
>
> ```bash
> harness mcp-gateway --world docs/demos/jira-copilot/jira-world.yaml -- harness mock-jira
> # add --taint tainted to demo the taint floor severing the external write
> ```
>
> Verdicts (proven by `crates/cli-harness/tests/mcp_gateway.rs`): `tools/list` hides the
> destructive tools (ABSENT); `jira_get_issue`/`jira_add_comment` ALLOW when clean;
> `jira_delete_issue` → ABSENT; under taint, `jira_add_comment` → DENY (taint floor).
> Manifest: [`jira-world.yaml`](jira-world.yaml) (harness `WorldManifest` schema). The host
> MCP configs in `hosts/` still apply — point them at `harness mcp-gateway` instead of
> `run-proxy.sh`. A full rewrite of the sections below is pending.

**The pitch:** *"I can give GitHub Copilot access to JIRA and not worry about an
accidental destructive action."* One governance manifest **shapes the Atlassian/JIRA
MCP capability surface** for **VS Code Copilot, JetBrains Copilot, and Claude Code**
alike — read + comment only, scoped to one project, every destructive JIRA tool
**ABSENT** (it does not exist for the agent — stronger than a deny it can argue with).

Decision: [`DECISIONS.md` D32](../../../DECISIONS.md) · Epic: [`PLAN.md` E16](../../../PLAN.md).

## Architecture — one proxy, three hosts

```
 VS Code Copilot ─┐
 JetBrains Copilot├─ (MCP/stdio) ─▶  Safe MCP Proxy  ─ (MCP) ─▶  Atlassian Remote
 Claude Code     ─┘                  (governed by         mcp-remote   MCP Server
                                      jira-governed.world.yaml)        (mcp.atlassian.com)
```

The host never talks to Atlassian directly. The proxy: filters `tools/list` to the
allowlist (**ABSENT** for everything else), routes each `tools/call` through the
deterministic policy engine (taint floor, arg rules), forwards only ALLOW upstream,
and **audits every decision**. Copilot exposes no per-call hook over its *native*
tools, but the **MCP surface is exactly where it is governable** — and it's
host-agnostic, so the same proxy config serves all three.

## What this proves to the audience

| Without governance | With the proxy |
|---|---|
| Copilot sees the full JIRA MCP surface incl. `jira_delete_issue`, `jira_bulk_create_issues`, transitions, edits, across all projects | Copilot sees only read + `jira_add_comment`; destructive tools **do not appear** |
| "Clean up old issues" can delete | "delete this issue" → the tool doesn't exist (ABSENT) |
| No record | Append-only audit log + dashboard of every ALLOW/DENY/ABSENT |

## Files here

| File | Purpose |
|---|---|
| `jira-governed.world.yaml` | the demo manifest (read + comment, scoped, destructive ABSENT) |
| `run-proxy.sh` | launches the governed gateway the hosts spawn (`safe_mcp_proxy.mcp_gateway`) |
| `hosts/claude-code.mcp.json` | Claude Code `.mcp.json` |
| `hosts/vscode.mcp.json` | VS Code `.vscode/mcp.json` |
| `hosts/jetbrains.md` | JetBrains Copilot MCP config |

## Status — honest build state

The hard parts already **exist and work** in `repos/safe-mcp-proxy`:

- ✅ a host-facing **stdio MCP server** (`mcp_server.py`)
- ✅ a real upstream **MCP client** (`mcp_upstream.py::UpstreamConnector`, official `mcp` SDK)
- ✅ the **Atlassian policy engine** — ABSENT / `arg_rules` / taint (`atlassian/policy.py`)
- ✅ a starter manifest (`manifests/atlassian_mvp.yaml`) with real Atlassian tool names
- ✅ an append-only **audit log + dashboard**

**E16.1 — compose glue: ✅ DONE.** `safe_mcp_proxy.mcp_gateway` is the host-facing stdio
server that connects upstream via `UpstreamConnector`, ABSENT-filters `tools/list` against
the manifest allowlist, routes `tools/call` through `ManifestPolicyEngine`, forwards only
ALLOW upstream, and audits decisions. Verified by 12 tests incl. a real end-to-end
(gateway ⇄ `UpstreamConnector` ⇄ test upstream); full safe-mcp-proxy suite 550 OK.
`run-proxy.sh` now launches it directly.

Remaining work:

- **E16.2 — per-project scoping (optional for the core punch).** `arg_rules` today
  only do exact `allowed_values`. Scoping `jira_get_issue` / `jira_add_comment` by
  issue-key prefix (`DEMO-*`) or constraining `jql` needs a ~10-line `allowed_pattern`
  branch in `policy.py`. **Interim:** use a dedicated `DEMO` project + the ABSENT list
  — the "destructive tools don't exist" headline needs no per-issue scoping.

## Prerequisites

- A JIRA instance + the **Atlassian Remote MCP Server** (OAuth) — or a sandbox JIRA.
- `node`/`npx` (for the `mcp-remote` stdio↔SSE bridge), `python` ≥ 3.10.
- A `repos/safe-mcp-proxy` checkout (point `run-proxy.sh`'s `SMP_REPO` at it).

## Demo script

1. **Baseline (the fear).** Point a host at the Atlassian MCP *directly*; show the tool
   list includes `jira_delete_issue` etc.; ask the agent to "tidy up stale issues" and
   watch it be *able* to delete.
2. **Governed.** Switch the host to `run-proxy.sh` (the configs in `hosts/`). Re-open
   the tool picker → only the read tools + `jira_add_comment`. Ask to delete an issue →
   *the tool does not exist*. Ask to comment on `DEMO-123` → it works. Open the audit
   dashboard → every decision is logged.
3. **Same manifest, Claude Code.** Repeat step 2 on Claude Code to make the
   host-agnostic / "broader CC use" point.

## Validation checklist (E16.1)

- [ ] proxy connects to the real Atlassian MCP upstream via `mcp-remote`
- [ ] `tools/list` returns only the allowlisted tools (destructive ones ABSENT)
- [ ] a real `jira_get_issue` / search returns data
- [ ] a real `jira_add_comment` posts a comment
- [ ] a `jira_delete_issue` attempt is ABSENT (not offered, refused if forced)
- [ ] every call appears in the audit log
- [ ] all three host configs spawn the proxy and see the shaped surface

## What I need from you to wire real JIRA

JIRA instance + auth · target **project key(s)** · the exact **read tool set** you want
allowed (defaults in the manifest) · confirm you can add an MCP server in **both** VS
Code and JetBrains Copilot at work.
