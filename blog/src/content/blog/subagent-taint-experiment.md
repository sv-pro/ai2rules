---
title: 'Does Taint Cross the Subagent Boundary? An Experiment'
description: "I assumed a multi-agent setup would launder tainted data back to the parent. Then I instrumented the gate and ran it — and the host surprised me."
pubDate: 'Jun 20 2026'
heroImage: '../../assets/taint-subagent-boundary.jpg'
---

In [an earlier post](/blog/the-zombieagent-threat/) I argued that an agent can be
re-compromised across sessions when untrusted data is laundered through persistent
storage. This post is about its sibling: **does the same laundering happen
*within* a single run, across the subagent boundary?**

The setup is real. We govern Claude Code with a deterministic `PreToolUse` hook —
`world-gate.py` — that enforces a **monotonic taint floor**: once the session has
ingested untrusted data (a file under `repos/`, a web fetch), any tool that can
reach the network is denied. Taint state lives in a per-session sidecar:

```python
taint_file = ".../state/taint-" + sanitize(event["session_id"])
```

That single line is the whole question.

## The hypothesis (a fail-open gap)

Modern agents spawn **subagents** — a reviewer, a researcher, an explorer — each
with its own context. So I reasoned: a subagent gets its *own* session, writes its
*own* `taint-<child>` file, and when it summarizes a poisoned file back to the
parent, the parent reads `taint-<parent>` — which is still clean. The untrusted
data crosses the boundary and comes back laundered. Worse, it fails **open**: the
parent could then exfiltrate freely. The ZombieAgent, intra-run.

A tidy theory. So I tested it instead of shipping it.

## The experiment

I added an opt-in debug log to the gate (dump the raw event when
`touch .claude/state/debug-on`), then spawned a subagent with a deliberately
distinctive task — *read `blog/src/consts.ts` and report it* — so its tool call
would be unmistakable in the log. Then I diffed the session ids the gate saw.

```
distinct session_ids seen by the gate:   23d7e5e5-…   (only one)
the subagent's Read of consts.ts:         session_id = 23d7e5e5-…   (same as the parent)
                                          …but with extra keys: agent_id, agent_type
```

**One shared `session_id` for the entire in-process agent tree.** The subagent
isn't a separate session at all — it's the same session, tagged with an `agent_id`
and `agent_type`. The `SubagentStop` event confirmed it, and threw in the detail
that the subagent's return value rides back as `last_assistant_message`:

```json
{ "session_id": "23d7e5e5-…", "agent_type": "general-purpose",
  "agent_transcript_path": ".../subagents/agent-….jsonl",
  "last_assistant_message": "# CLI Agent Harness" }
```

## The finding: I was wrong, and that's the point

Because the gate keys taint by `session_id`, and Claude Code shares one across the
tree, **taint already propagates parent ↔ subagent for free** — they read and
write the *same* sidecar. The conservative thing happens by default: a subagent
that touches untrusted data taints the whole tree, including the parent. No gap.

This is exactly why you test the *physics of the host* rather than reasoning from
your own model. The in-process kernel makes taint ride the data; on a host you
don't control, you inherit the host's boundaries — and here they happened to be
stronger than I assumed.

Here's the propagation, end to end (a self-contained replay, no live session
touched):

```text
1) Baseline — a CLEAN session can reach the network:
   gate[clean ] parent WebFetch (clean)            -> allow
2) A SUBAGENT (shared session) reads untrusted data under repos/:
   gate[shared] subagent Read repos/intel.md       -> allow
   taint marker: tainted by Read (agent: general-purpose): repos/intel.md
3) Subagent finishes -> SubagentStop surfaces the taint:
   SubagentStop: ⚠ Session tainted: a subagent read untrusted data...
4) The PARENT now tries to exfiltrate over the SAME session -> blocked:
   gate[shared] parent WebFetch (post-subagent)    -> deny
```

## Where the gap *does* reopen

The shared session is an in-process detail. Agents that run **isolated** — a
separate worktree, a background job, a remote sandbox — get a **distinct**
session id *and* a distinct `.claude/state`. There, the sidecar no longer
overlaps, and the laundering path returns.

So the hardening isn't "build propagation" (the common case already has it) — it's
**make it explicit and observable, and cover the distinct-session case**. A
`SubagentStop` hook does both: it surfaces the taint when a subagent finishes
(so the floor is never silent), and — if the host ever exposes a parent link for
isolated agents — unions the child's taint into the parent. Relying on an
undocumented shared-id coincidence is fragile; naming the invariant and enforcing
it ourselves is the point.

## The takeaway

Two lessons, both cheap and both earned the hard way:

1. **Verify the host's actual semantics before trusting your threat model.** One
   `echo … | python3 hook.py` and a single spawned subagent settled a question I'd
   have otherwise guessed wrong.
2. **Taint must follow information flow, not session boundaries.** It just happens
   that, in-process, Claude Code's boundary *is* the flow. Off the happy path, you
   have to reconstruct the edge yourself.

Monotonic taint that never decreases across sessions is one of the harness's core
invariants. It survived contact with a real multi-agent host — and got an
observability upgrade for its trouble.
