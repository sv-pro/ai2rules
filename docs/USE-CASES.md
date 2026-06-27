# Use cases & delivery model

What a user actually **gets when they install ai2rules**, who each package is for, and
the order we ship. Decision of record: [`DECISIONS.md`](../DECISIONS.md) **D31**. Builds
on the five-layer thesis ([`THESIS.md`](THESIS.md)) and the host-neutral gate ABI
([`harness-gate-abi.md`](harness-gate-abi.md), D24).

## The one-line answer

**ai2rules is infrastructure, not an application** — a deterministic governance *engine*
that always wraps a host the user already runs. Think **OPA / seccomp / Envoy, but for
agent actions**: it ships as its own binary / crate / hook set (**standalone in form**)
yet has no value alone, because it exists to govern *another* system (**plugin in
role**). It is therefore *both* of the things the question asked — a standalone engine
deployed as a plugin/sidecar — and **not** a standalone agent of its own.

> **What's cut:** the "custom CLI agent on a Claude Code basis" ambition is retired as a
> *product*. Shipping our own agent contradicts the host-neutral thesis and forces us to
> compete where we're weakest (model + host UX, vs. Anthropic/OpenAI/Hermes) while
> diluting the only thing that's ours (the governance). **Sell the border, not the
> agent.** The `cli-harness` CLI / TUI (E9) stay as a dev, demo, and reference harness.

## Who it's for

| Segment | Pain | Package |
|---|---|---|
| **Individual dev on Claude Code** | "my agent inherits my whole machine — SSH keys, prod creds, `rm -rf`" | CC Governance Pack (free wedge) |
| **Team / org** | "can we let agents touch this repo / prod — and *prove* what they couldn't do?" | same engine + policy-as-code + audit/replay (the paid story) |
| **Platform / framework builder** | "I'm building an agent and don't want to reinvent the gate" | `world-kernel` crate / `harness gate` binary |
| **Security researcher** | Zombie-Agent / prompt-injection / AgentDojo-style work | the kernel + the WASM playground + scenarios |

## The product family (one kernel, several surfaces)

These are **not separate projects** — they're the same kernel projected onto different
hosts/protocols (the "1 kernel + adapters" collapse).

| Package | Install | What they get | Form |
|---|---|---|---|
| **Claude Code Governance Pack** ← lead | `pipx install ai2rules; ai2rules init` | ABSENT tools · taint floor · ASK/DENY · audit ledger · trust pins — same Claude Code, now governed | **plugin** (hooks) |
| **Safe MCP Proxy** | run as a sidecar between MCP client ↔ servers | tool projection · descriptor-drift detection · ABSENT · audit — host-agnostic | **sidecar / proxy** |
| **`harness gate`** | drop-in binary, wired via the stdio ABI | one governance engine callable from a Python hook / shell / any CLI | **sidecar (ABI)** |
| **`world-kernel`** | `cargo add` (or a binding) | `IntentIR` + `CompiledWorld` + monotonic taint + deterministic disposition as a vetted dependency | **library** |

## Lead package — Claude Code Governance Pack: install → get

```
pipx install ai2rules          # or npx
cd my-repo && ai2rules init     # writes the PreToolUse hook config + a starter cc-world manifest
```

Nothing about the workflow changes — they keep using Claude Code — **except** every tool
call now passes the kernel, so they get:

- tools outside the manifest are **ABSENT** (never even offered to the model);
- **tainted context** (an untrusted file or web fetch) **can't drive network egress**;
- dangerous actions → **ASK / DENY** per the manifest;
- an **append-only audit ledger** they can replay ("prove the agent couldn't exfiltrate");
- **trust pins** to vouch for reviewed sources (drift re-taints).

Five-minute install, zero behaviour change, a concrete guarantee: *a seatbelt for the
agent you already use.*

## Adoption → monetization path

1. **Wedge (free, individual):** the CC Governance Pack — frictionless and shareable
   ("I made Claude Code unable to `rm -rf` or phone home — here's the audit log").
2. **Revenue (teams / orgs):** the same engine plus **policy-as-code + audit + replay**
   across many agents — the **OPA-for-agents** compliance/security story.
3. **Reach multiplier (embedders):** the `world-kernel` crate / `harness gate` binary —
   lands the engine inside *other people's* products.

## Ship order

1. **Claude Code Governance Pack** (plugin) — *v1*. Packaging + `init` over the
   dogfooded hooks; backed by `world-kernel` through the gate ABI. (PLAN: **E13**, esp.
   **E13.8**, on **E1/E2**.)
2. **Safe MCP Proxy** (sidecar) — protocol-level reach. (PLAN: **E7**, **E13.4**, +
   `safe-mcp-proxy` / `mcp-tool-projection` references.)
3. **`harness gate` binary + `world-kernel` crate** (sidecar / library) — embedders.
   (PLAN: **E13.8 / D24** + kernel crates.)
4. **Supporting layers** (knowledge / intent / substrate) ship **later**, each as an
   optional sidecar / MCP-server behind a spine contract — never a v1 prerequisite.

## Related

[`DECISIONS.md`](../DECISIONS.md) D31 (this decision), D24 (gate ABI) · [`THESIS.md`](THESIS.md)
§4 (layers) / §7 (consolidation) · [`harness-gate-abi.md`](harness-gate-abi.md) ·
[`RESEARCH-BACKLOG.md`](RESEARCH-BACKLOG.md) R1 (the cross-host super-harness — a *later*
surface, not v1) · [`PLAN.md`](../PLAN.md) (Delivery model & packaging).
