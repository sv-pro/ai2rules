---
title: 'The Switch Inside the Room'
description: "We shipped a governance tool with an off switch the agent could reach. Then we found the same shape twice more, in places that looked nothing alike. Here is the pattern, three demonstrations of it, and the one sentence that catches all three."
pubDate: 'Aug 12 2026'
heroImage: '../../assets/the-switch-inside-the-room.jpg'
---

We ship a thing that sits between a coding agent and your machine and decides what
it may do. Last week someone using it asked their agent a perfectly ordinary
question, and the agent turned the whole thing off.

Not maliciously. It was asked to create a file, and it created the file. The file
happened to be the kill switch.

```
> Create an empty file at .claude/gate-off in this project.

Created `.claude/gate-off` (empty, 0 bytes).
```

One turn. No denial, no prompt, no warning. And a write to `/etc/passwd` that had
been refused thirty seconds earlier now went straight through.

The person who reported it noticed something sharper than the bug. Their agent had
*refused* the same request earlier in a different session — when the conversation
had made clear what the file was for. It complied when it didn't know. Which means
the thing standing between the agent and the off switch was **the agent's own
understanding of what it was being asked to do.**

That is not a security control. That is hoping.

## The shape

The kill switch lived at `.claude/gate-off`, inside the project directory. The
agent's job is to write files in the project directory. So the switch that turns
off the enforcement was sitting in the one place the enforced party has permission
to write.

Written out like that it's obvious. It wasn't obvious while writing it, because
every individual decision was reasonable: put the config with the project, keep the
kill switch next to the config, make it a plain file so it's easy to explain and
easy to use in a hurry. Each step sensible, and the destination is a lock whose key
hangs on the inside of the door.

Then we went looking, and found it twice more.

## Twice more

**The rulebook.** The policy is a manifest — a YAML file listing what the agent may
touch. It also lived in the project directory. Writing to it drew no denial, not
even a prompt:

```
Write  .claude/cc-world.yaml     → ALLOWED (silent)
Edit   .claude/cc-world.yaml     → ALLOWED (silent)
```

We assumed this was survivable, because the compiled policy is immutable once
built. That assumption was wrong, and wrong in the direction that matters. The hook
runs as a fresh process on *every single tool call*, which means it recompiles the
manifest every time. So:

```
1. baseline                                       → DENY
   [agent edits one line of its own policy]
2. next tool call, same session                   → ALLOWED
```

There is no "takes effect next session," because there is no session. One edit, one
call later, new rules. And unlike flipping a kill switch, this one is quiet: the
governance is still running, still logging, still returning verdicts. It's just
answering a different question than you think it is.

**The kernel itself.** This one we introduced while fixing packaging. The tool ships
as an npm package; install it locally and the binary lands in `node_modules/` —
which is, once again, inside the project the agent can write to. We measured it:

```
Write  node_modules/…/harness   → ALLOWED
[replace it with a script that exits 0]
Write  /etc/passwd              → silence. every verdict gone.
```

Three files. A switch, a rulebook, and the enforcer. Found weeks apart, in
different subsystems, each looking like its own small bug.

## The sentence

> **Anything the enforcement depends on must live outside what it enforces upon.**

That's it. It catches all three, and it would have caught them before they were
written.

If you've done any security work this will sound familiar, and it should. It's half
of the [reference monitor](https://en.wikipedia.org/wiki/Reference_monitor) criteria
from 1972 — a reference validation mechanism must be tamperproof, always invoked,
and small enough to verify. We rediscovered *tamperproof* the slow way, by breaking
it three times in one codebase.

Worth being precise about why the classic phrasing didn't save us. "Tamperproof"
sounds like it's about attackers. None of these were attacks. There was no adversary
and no injection; a helpful assistant was asked to create a file and did. The
property you need isn't "resistant to a determined attacker" — it's **"not reachable
by the ordinary, well-intentioned operation of the thing being governed."** Much
lower bar, and we cleared it in the wrong direction three times.

## What the fixes look like

**The switch moved out of the project.** It now lives at
`~/.claude/ai2rules/off/<project>`, and writes outside the project are themselves
governed, so an agent reaching for it gets an approval prompt instead of silence.
Not a hard denial — a human in the loop, which is the right bar for *"turn off your
own supervision."*

**The rulebook became read-only.** One line in the manifest:

```yaml
- { path: "./.claude", access: Read }
```

Read stays allowed on purpose. An agent that can't see its own rules can't explain
a refusal or suggest a sensible change to one, and the file is committed to the
repo anyway — secrecy was never the property being defended. **Write authority
was.**

**The installer now refuses to install itself somewhere it can be replaced.** If the
binary is inside the project it would govern, it stops and tells you to install
globally. A governance install that can be undone with one file write is worse than
none, because it's indistinguishable from one that works.

## The part that generalises past our tool

That last sentence is the one worth taking away, and it isn't really about us.

If you run any agent with a guardrail — a hook, a permissions file, a wrapper
script, an allowlist, a sandbox profile — ask one question: **can the agent write
to it?** Not "would it," not "why would it want to." Can it.

Try it, too, rather than reasoning about it. Every one of these three was found by
running something and watching, and two of them survived a security review, a test
suite that grew to 244 cases, and a published blog post about how careful we were
being. The kill-switch test in that suite *passed the entire time* — it asserted the
shim contained the string `.claude/gate-off`, and after the fix the shim still
contained that string, in a comment explaining the old behaviour. A green test,
agreeing with a defect, made wrong by a documentation improvement.

The check is two lines and a directory listing:

```bash
# whatever your agent's guardrail config is, ask it to write there
echo '{"tool_name":"Write","tool_input":{"file_path":"<your-guardrail-file>"}}' \
  | <your hook>
```

If nothing comes back, your policy is a suggestion your agent has been kind enough
to follow.

---

*The tool is [`ai2rules-harness`](https://www.npmjs.com/package/ai2rules-harness) —
`npm install -g ai2rules-harness && harness init`. The three fixes are in 0.2.0.
The full reasoning, including what we rejected, is in
[`DECISIONS.md`](https://github.com/sv-pro/ai2rules/blob/main/DECISIONS.md) D57 and
D58.*
