# Approval-capable host paths

`ASK` requires a human decision. A gateway without `--approvals` returns an MCP
tool error and forwards nothing. Background mode remains fail-closed. Claude Code
hooks map ASK to their native prompt; that prompt does not authorize a second,
independently governed MCP gateway.

This implementation is PLAN E13.4d, [AI2-72 / PR #17](https://github.com/sv-pro/ai2rules/pull/17),
and DECISIONS D89. It reuses D73 effect-bound authorization and D78 chained storage.

## Operator workflow

Configure a separate store outside paths the governed project/agent can write.
The log, `.key`, `.head`, and `.lock` sidecars are one security boundary. Do not
copy an older log/head pair over a live store or manually edit lifecycle records.

```sh
harness mcp-gateway --world world.yaml \
  --approvals /operator-private/approvals.jsonl --principal alice \
  -- upstream-server
harness approvals list --store /operator-private/approvals.jsonl
harness approvals approve <id> --store /operator-private/approvals.jsonl
# Or:
harness approvals reject <id> --store /operator-private/approvals.jsonl
```

`--principal` is required with `--approvals`: it is an identity assigned by the
trusted process launcher, never a principal asserted in an MCP request. Give each
principal a separate launch configuration; do not share one gateway among users.
The CLI default store is `.ai2rules/approvals.jsonl` for compatibility; use the
explicit operator-private path for both commands in a governed deployment.

1. An ASK records a Pending request and returns its ID. Sequential identical
   requests reuse a live Pending request. Concurrent first requests can create
   separate Pending records; each requires a separate human decision.
2. Review the proposed call in the host and compare its action, principal,
   resource and full parameters hash with `approvals list`. The log stores hashes,
   not the full argument body; listing alone cannot show you what you are approving.
3. Approve or reject that ID out of band. Retry the same MCP call after approval.
4. Before forwarding, the gateway atomically consumes consent under the store lock
   and persists the signed chain/head. A failed claim forwards nothing.
5. The kernel evaluates again with trusted approval. Budgets, taint, capability,
   policy ASK/DENY and scoped argument lowering still apply. Only ALLOW can run.
6. After an upstream reply, the gateway records Executed. The usual metadata
   forwarding and `input_required` refusal apply to approved calls too.

A grant expires **five minutes after creation**, including time spent Pending.
It binds the principal, proposed request (including `_meta`), effective scoped
call, configured upstream command, resource, world and descriptor hashes,
provenance and execution mode. Changing any bound field requires fresh consent.
The parameters hash is SHA-256 of canonical JSON for
`{v:1, request:<tools/call params>, effective:<gate effective>, upstream:[...]}`.
Dynamic `_meta` changes therefore also require new consent; this deliberately
prefers exact binding over automatic trace-metadata tolerance.

## Unknown outcomes and retries

Consumed is terminal for authorization. An upstream can commit an effect and
then disconnect: the gateway reports **outcome unknown**, retains Consumed, and
never restores that grant. A failure to record Executed likewise reports that
the reply arrived but execution evidence could not be persisted. Do not infer
that nothing happened from an MCP error or from a missing Executed event.

A retry may create a fresh Pending request, but cannot execute without another
human approval. For a prior Consumed request the response explicitly asks the
operator to reconcile the external state first. This is at-most-once forwarding
per grant, not an exactly-once external-effect protocol. If the kernel blocks
reevaluation after the claim, the grant is also spent conservatively.

Legacy/integrity-invalid stores fail closed; there is no automatic migration that
restores old approvals. Windows store key creation remains the separate #129
workstream. The integration suite runs on Unix with Python 3 as a fault-injecting
upstream and covers effect-before-disconnect, contention, persistence failures,
expiry, drift and the normal scoped execution path.

The pure gate ABI remains unchanged: `context.approval_token` is correlation data
and never grants authority. The native `gate_and_lower_authorized` API is only for
a trusted runtime that has already consumed an exact-bound authorization.
