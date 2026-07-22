---
title: "Governed Is Not Confined"
description: "We switched governance on for our own agent and assumed it was now fenced to the project. Then we asked it to read /etc/shadow. It said yes — exactly the way it says yes to reading our README. Here's why 'governed' and 'confined' are different axes, and which one a manifest actually covers."
pubDate: 'Jul 22 2026'
---

We turned our governance on against our own agent — a live Claude Code session running
under the harness, every tool call passing through the kernel first. Then we did the thing
you should always do with a safety claim: we tried to break it. We asked the governed agent
to read `/etc/shadow`. To write `~/.bashrc`. Files that have nothing to do with the project
it was working in.

It said yes to all of them — and not grudgingly. It granted them **exactly** the way it
grants reading the project's own `README`. Same verdict, no hesitation, no difference.

That surprised us less than it should have, and the reason why is the whole point of this
post.

## What we assumed "governed" meant

When you wire a governance hook into a project and watch it start denying things, the
natural story in your head is: *the agent is now contained here.* Fenced to this folder.
Kept away from the rest of the disk. That's what "sandbox" trains you to expect, and
"governed" borrows the feeling.

It's the wrong story, and our own agent proved it in one command.

## What the kernel actually sees

The request the kernel decides on is small. It's the tool being called, its arguments, and
a little context — which session, what mode, and the taint state. That's it. Read it again
and notice what's *not* there: **no path.** No working directory, no notion of "inside the
project" versus "outside" it. When the agent asks to read a file, the kernel sees "a Read"
— not *where*.

So `Read /etc/shadow` and `Read ./README` aren't two things the kernel weighs differently.
They're the *same* thing: a Read. There is no "own directory" anywhere in the machinery.
The project folder mattered exactly once — at setup, to decide *which sessions* get
governed at all. After that, it's invisible. The gate governs *what kind of action* the
agent takes and *where its data came from*. It never governs *where the action lands.*

## Two axes people fold into one word

Here's the distinction the `/etc/shadow` moment forced us to say out loud. "Keeping an
agent safe" is really two different jobs, and "governed" only names one of them:

- **Governance** — controlling *what kind of action* is allowed and *how trust flows*
  through it. Is this an action the agent may take? Did the thing driving it come from a
  source we trust? This is what our manifest does.
- **Confinement** — controlling *where* the agent can reach. Can it leave this folder? Can
  it touch `~/.ssh`? This is a *spatial* question, and our manifest has no concept of space
  at all.

They feel like the same thing. They are not. You can be fully governed and completely
unconfined — which is exactly what our agent was. It couldn't be tricked into an action it
shouldn't take (that part works), but nothing stopped it from taking a perfectly ordinary,
fully-approved action *against a file across the filesystem.*

## The part that's easy to miss

There's a sharper edge, and honesty demands we name it. Our one real runtime protection is
the *taint floor*: once the session has touched something untrusted — a web page, a tool
result — it can no longer reach back out to the network. That's what stops a
[poisoned document](/blog/the-zombieagent-threat/) from exfiltrating a secret.

But reading a **local** file is *clean*. It doesn't taint the session, because the file came
from your own disk, not from an attacker. Which means: read a local secret, then send it
somewhere, and the taint floor **does not stop you** — the session was never tainted. The
floor defends against *injection* (untrusted content driving an action), not against an
agent exfiltrating files you already trusted. Different threat, different axis, again.

It wasn't always so. An earlier version of the engine *did* taint reads — it even recorded
which file did it — and that behavior was quietly set aside during a rewrite. So the defense
for exactly this case has existed before; restoring it, as something you *declare* per path
rather than something hard-coded, is part of the fix below.

## Is this a bug? No — it's a missing primitive

We want to be precise, because "governance tool can read /etc/shadow" is the kind of
sentence that sounds like a scandal and isn't. The harness does what it claims: it governs
the *ontology of actions* and the *flow of trust*. Spatial confinement was never in it. It's
not broken; it's **incomplete** — a primitive we simply hadn't built.

And the shape of that primitive is clear. The Model Context Protocol already has a concept
called **roots** — a declared set of directories an agent is scoped to. Our manifest has
nothing like it, and noticing that *absence* is what named the gap. The fix is
unglamorous and deterministic:

1. Let the manifest declare **roots** — the directories in scope.
2. Give the kernel the action's **path** (the one thing it's currently blind to).
3. Compare: a Read or Write **under a root** is allowed; **outside** one, it's asked or
   denied — and a path under a sensitive root can finally carry the `Secret` label the
   manifest already has a word for but no way to attach.

No model, no guessing — a path comparison. The kind of boring, checkable rule the whole
approach is built on.

## The takeaway

If you're governing an agent, know **which axis your governance is on.** Ours is on trust
and action — genuinely useful, and the thing most "permission" systems can't do. But it is
not a jail, and we were one command away from believing it was.

For actual confinement — keep the agent off the rest of the disk — you still need the other
axis: an OS-level sandbox, a [throwaway container](/blog/running-claude-safely/) with the
filesystem and network fenced, or path-scoped capabilities once we ship them. A trust
monitor and a jail are different tools. The mistake isn't using one; it's using one and
thinking you have both.

We found this by pointing our own review at our own live agent — the same move that keeps
turning up the most useful things we know. "Governed" felt like enough. Then we asked it to
read the password file, and it taught us the word we were missing: *confined.*
