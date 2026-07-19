# Approval-capable host paths — how `ASK` gets satisfied

`ASK` is the kernel verdict that means *"a human must decide this one."* The kernel
already produces it (`approval_required` on an action, or a policy that escalates), and
the durable machinery to satisfy it already exists — `ApprovalStore`
(`mint / approve / is_granted / mark_executed`), an `ApprovalToken` bound to the exact
call, and `is_granted`'s **drift-voiding**. What is missing is a *channel to a human*,
and that channel differs by host. This doc records the per-host design.

See also: [`DECISIONS.md` D41](../DECISIONS.md) (the decision), E6 (approvals), E16.C
(`cc-hook`), D24 (gate ABI), and [`docs/demos/confluence-docs/`](demos/confluence-docs/)
(the `approval_required` motivating example).

## Why hosts differ

`gate()` emits `ASK`; the shared [`host_outcome`](../crates/harness-preview/src/host.rs)
layer maps it to `HostOutcome::NeedsApproval`. From there each adapter must surface *its*
approval channel — and hosts have very different ones:

| Host | Approval channel | Status |
|---|---|---|
| **Claude Code** (PreToolUse via `cc-hook`) | The host's own native Allow/Deny prompt | **Already works** — no new code |
| **MCP-only** (Copilot VS Code / JetBrains, via `mcp-gateway`) | *None* per call — MCP `tools/call` is one shot | The gap; needs out-of-band approval |
| **Automated / background / CI** | No human present | Stays **DENY** (`background_denies_ask`) — correct as-is |

**Fail-closed is the default everywhere.** A host with no way to obtain approval must not
execute the call. Background mode collapses `ASK → DENY` in the kernel itself (invariant
8/10); the MCP gateway, lacking any channel, surfaces `ASK` as a non-forwarded block. The
verdict keeps the name `ASK` (rather than being relabelled `DENY`) precisely so that a host
which *can* prompt still knows to.

## Path 1 — Claude Code (native prompt), no new code

`cc-hook` already maps the approval outcome to Claude Code's structured decision channel:

```rust
// crates/cli-harness/src/cc_hook.rs
HostOutcome::NeedsApproval { reason } => emit("ask", &reason),
//                                            ^ permissionDecision: "ask"
```

So an `approval_required` action becomes Claude Code's native **Allow / Deny** prompt, and
the host owns the pause/resume and the UI. Nothing else is needed.

**Nuance worth knowing:** Claude Code's `PreToolUse` fires for **MCP** tool calls too
(matcher e.g. `mcp__jira-governed__updateConfluencePage`). So even tools reached *through*
the governed gateway can get Claude Code's native approval prompt when running on the CC
host — `approval_required` "just works" there without the out-of-band path below.

## Path 2 — MCP-only hosts (out-of-band approval via `ApprovalStore`)

MCP `tools/call` is a single request→response with no channel to pause for a human. Use the
`ApprovalStore` as an **async bridge**: the gateway mints a pending token bound to the exact
call; a human approves it out of band; the agent retries and the gateway lets it through
exactly once.

**New pieces (both small):**
- `harness mcp-gateway --approvals <path.jsonl>` — a shared `ApprovalStore`.
- `harness approvals list` and `harness approve <id>` / `--reject <id>` — the human's
  review CLI over that store.

**Flow:**

```
agent → gateway   tools/call updateConfluencePage(pageId=196809, body=B)
gateway           gate() → ASK
gateway           store.is_granted(binding)? → false
gateway           store.mint(pending, binding)
gateway → agent   isError "ASK: pending approval a1 — run `harness approve a1`"
── out of band ──
human             harness approvals list      # a1  updateConfluencePage  pageId=196809  body≈"…"
human             harness approve a1          # store.approve(a1) → Approved
── agent retries, SAME args ──
agent → gateway   tools/call updateConfluencePage(pageId=196809, body=B)
gateway           gate() → ASK
gateway           store.is_granted(binding)? → TRUE
gateway → upstream  forward → success
gateway           store.mark_executed(a1)     # single-use
gateway → agent   ALLOW result
```

The **binding** is exactly `ApprovalToken::pending`'s fields — reuse them, do not invent a
parallel identity:

```
(action, params_hash, world_id, descriptor_hash, provenance, effect_mode)
```

## Safety properties (each falls out of an existing primitive)

- **Approve exactly what you saw.** `params_hash` is in the binding, so if the agent changes
  `body` on retry, `is_granted` is false → a *new* pending token; the old approval is inert.
  (This is what the existing `drift_voids_the_approval` test asserts.)
- **Single-use.** `mark_executed` after a successful forward; a replay re-gates to `ASK`
  (state `Executed` ≠ `Approved`).
- **World-frozen.** `descriptor_hash` + `world_id` in the binding: editing the manifest
  between approve and execute voids the approval.
- **No silent clobber.** Confluence's own version concurrency still guards the real write —
  a stale update is rejected upstream, protecting any manual edit made in between.
- **Fail-closed preserved.** Background mode denies *before* any minting occurs.

## Subtleties to get right

1. **Derive the binding the same way `gate()` does.** The gateway must compute
   `descriptor_hash / provenance / effect_mode` identically to the kernel, or the retry's
   `is_granted` will not match the mint. Reuse the kernel's derivation; do not recompute it
   divergently.
2. **Retry is agent-driven.** MCP has no server→client push, so after approval the *agent*
   must re-issue the call (poll, or a human tells it to try again). The clean native fix is
   **MCP elicitation** (a server-initiated request for user input mid-call); adopt it for
   hosts that support it to collapse the retry round-trip. It is the forward path, not the
   baseline, because support is not yet universal.
3. **TTL on pending tokens.** `ApprovalToken` has no expiry today — add "approve within N
   minutes" so a stale pending approval cannot be rubber-stamped much later.
4. **Idempotent mint.** Key the pending token on the binding hash so repeated identical
   `ASK`s do not pile up duplicate rows.

## Implementation slices

| Slice | Size |
|---|---|
| `harness approvals list` + `harness approve/reject <id>` over `ApprovalStore` | S |
| Gateway `--approvals`; on `ASK`: `is_granted` → forward + `mark_executed`, else `mint` pending + informative block | M |
| Idempotent `mint` (key on binding hash) + TTL on `ApprovalToken` | S |
| Tests: `ASK` → pending → approve → retry → `ALLOW` → executed; changed-args → new pending (old inert); replay → `ASK` | M |
| (later) MCP elicitation path for hosts that support it | L |

Claude Code needs **none** of these — its native prompt already satisfies `approval_required`.
