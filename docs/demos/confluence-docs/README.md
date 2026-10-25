# Governed Confluence doc authoring

The [JIRA skin](../jira-copilot/REAL-ATLASSIAN.md) shows one manifest shaping the
Atlassian Jira surface. This example uses the same `harness mcp-gateway` with a
different world: creation is confined to one Confluence tenant/space, and updates
to one tenant/page require approval. No kernel or gateway implementation changes.

An LLM may draft [the manifest](confluence-docs.world.yaml); a human reviews its
tenant and object pins before the compiler freezes it. The pure kernel enforces
the resulting world at runtime (PLAN principle 7).

## What the world grants

| Tool | Scope | Gate behavior |
|---|---|---|
| `getAccessibleAtlassianResources`, `getConfluenceSpaces`, `getPagesInConfluenceSpace`, `getConfluencePage`, `searchConfluenceUsingCql` | reads available to the upstream account | ALLOW; the MCP result taints the session |
| `createConfluencePage` | one `cloudId` + `spaceId` pair | clean context: ALLOW; wrong/missing tenant or space: DENY (`schema_violation`) |
| `updateConfluencePage` | one `cloudId` + `pageId` pair | clean context: ASK; wrong/missing tenant or page: DENY (`schema_violation`) |
| everything else (Jira, comments, Compass, admin) | absent | ABSENT |

The reads have explicit `McpServer` backings, even though their side effect is
`Read`. Returned remote content is untrusted. The gateway carries that taint into
later calls: **after any of these reads, both create and update are DENY**
(`taint_invariant`, enforcing `no_tainted_external`). Further reads remain allowed.
An allowed create also returns remote content and taints the session. These pins
limit writes; they do not confine reads to the chosen tenant/space/page.

This is not a read-edit-write workflow: the manifest deliberately prevents remote
page content from driving an external write. Do not reset taint while retaining
that content or relabel it as trusted input. A clean write must originate from
independently trusted input in a clean session.

## Configure and run

During trusted setup, resolve your tenant and object identifiers separately, then
replace all occurrences of these three placeholders in a copy of the manifest:

| Placeholder | Meaning |
|---|---|
| `REPLACE_WITH_CLOUD_ID` | the same tenant identifier in **both** write constraints |
| `REPLACE_WITH_SPACE_ID` | the space in that tenant where creation is permitted |
| `REPLACE_WITH_PAGE_ID` | the page in that tenant where an update may be proposed |

Page and space IDs alone do not identify a tenant. Changing only `cloudId` must
fail, even when the numeric page/space ID matches. Both coordinates are required
and constrained by the kernel.

From the repository root, point the existing launcher at your configured copy:

```bash
MANIFEST=/absolute/path/to/confluence-docs.world.yaml \
UPSTREAM="npx -y mcp-remote https://mcp.atlassian.com/v1/mcp/authv2" \
bash docs/demos/jira-copilot/run-proxy.sh
```

The OAuth bridge handles sign-in; the manifest contains identifiers, not
credentials. The offline regressions below do not launch this bridge or access
Atlassian.

## Approval boundary

For a correctly scoped, clean update, `approval_required` produces ASK. The
current MCP gateway returns that as an error and **does not forward the update**;
it does not implement a human approval/retry flow. Background updates are DENY
(`background_denies_ask`). A tainted update is DENY before approval is considered.

The pure gate's `approval_token` is a correlation ID, not a grant (D41). The
separate `ApprovalStore` contract does not mean this gateway can consume approvals.
An upstream version/concurrency check cannot be evidence here because ASK prevents
the update from reaching the server.

## Offline regression evidence

```bash
cargo test -p cli-harness --test confluence_world --offline
```

Five tests compile the committed template with synthetic IDs and exercise:

- All five reads: ALLOW with the correct MCP backing, returned taint carried into
  create/update -> DENY, and continued reads retaining taint.
- Clean positive controls: the pinned tenant/space creates; the pinned tenant/page
  asks. Changing or omitting either coordinate, or using a wrong type, is denied.
- Background update refusal and undeclared tools remaining ABSENT.
- A real gateway process over `harness mock-jira --rovo`: a page read returns
  `Mock Page`, then create is denied with carried taint in the audit log.

The mock provides the page read only; this test does not claim live write execution
or full current Rovo surface compatibility. The pre-fix manifest failed the read
taint, both tenant-scope tests and the gateway sequence.

## Historical live evidence (before the review fixes)

The original July 2026 PR reported a run against `ai2rules.atlassian.net`, space
FH (`65709`): the surface exposed five reads plus create/update, creation produced
page `196809`, and an update returned ASK without advancing that page's version.
That run used the old manifest, which omitted MCP backing on reads and the write
tenant pins. It does **not** validate the corrected read -> write behavior.

The original claim that an approval token makes the pure gate ALLOW is obsolete
under D41. The corrected manifest is verified offline; no new live Atlassian run
has been performed for this repair (AI2-71 / PR #16).
