# Governed Confluence doc authoring

The [JIRA skin](../jira-copilot/REAL-ATLASSIAN.md) shows one manifest shaping the
Atlassian **Jira** surface. This is the same `harness mcp-gateway`, the same real
Atlassian Rovo MCP upstream, but a **different world** — one that lets an AI agent
write **documentation** into a single Confluence space, and revise a single page
behind an approval. Same kernel, no code change; only the manifest differs.

It's a worked example of two ideas from the thesis:

- **Design-time stochastic, runtime deterministic** (PLAN principle 7). An LLM may
  *draft* [`confluence-docs.world.yaml`](confluence-docs.world.yaml); a human reviews
  it; the compiler freezes it. The runtime gate that enforces it is the pure kernel.
- **Object-capability scoping.** Create is locked to a *space*; update is locked to a
  single *page id* — a capability handed to exactly one object, not an ambient
  permission over a namespace.

## What the world grants

| Tool | Grant | Enforcement at the gate |
|---|---|---|
| `getConfluenceSpaces`, `getPagesInConfluenceSpace`, `getConfluencePage`, `searchConfluenceUsingCql`, `getAccessibleAtlassianResources` | reads | ALLOW when clean |
| `createConfluencePage` | **one space** | `arg_constraints: spaceId const` — any other space → **DENY** (`schema_violation`) |
| `updateConfluencePage` | **one page, behind approval** | `arg_constraints: pageId const` + `approval_required` — wrong page → **DENY**; right page → **ASK** |
| everything else (all Jira, comments, Compass, admin) | — | **ABSENT** (not declared) |

All writes are `side_effect: External`, so the taint floor severs them in a tainted
session (`no_tainted_external`).

## Run it

Point the gateway's `MANIFEST` here (resolve your own `spaceId`/`pageId` first and
replace the two `REPLACE_WITH_*` placeholders):

```bash
MANIFEST=docs/demos/confluence-docs/confluence-docs.world.yaml \
UPSTREAM="npx -y mcp-remote https://mcp.atlassian.com/v1/mcp/authv2" \
bash docs/demos/jira-copilot/run-proxy.sh
```

`mcp-remote` handles OAuth in your browser on first launch (token caches under
`~/.mcp-auth`); the manifest never holds credentials — only tool names and pinned ids.

## The three composed guarantees

1. **Pin** (which page/space) — `arg_constraints const`, enforced by the kernel's
   schema check.
2. **Explicit consent** (may this update happen, now) — `approval_required` → `ASK`;
   the durable binding (`ApprovalStore`, E6.4) grants only for the identical
   action + params + world + descriptor, so an approval is **voided by drift**.
3. **No silent clobber** (don't overwrite a manual edit) — Confluence's own version
   concurrency rejects a stale update.

Only #2 asks a human — which is where it belongs.

## Live-validated

Run against the `ai2rules.atlassian.net` site (space **FH**, id `65709`):

- `tools/list` → the 5 reads + create (+ update); the ~40-tool Rovo surface is shaped away.
- `createConfluencePage` → **ALLOW**, a real guide page created (id `196809`).
- `updateConfluencePage` on `196809` through the gateway → **ASK**, surfaced as a
  non-forwarded block (`isError: "ASK: human approval is required"`); the page stays
  at **version 1** — the attempt never reached Atlassian.
- Offline gate matrix: wrong space/page → **DENY** `schema_violation`; tainted →
  **DENY** `taint_invariant`; background → **DENY** `background_denies_ask`; approved
  token → **ALLOW**.
