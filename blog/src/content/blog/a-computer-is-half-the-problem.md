---
title: "Agents Need Their Own Computer. That's Half the Problem."
description: "LangChain says agents need their own computer. They're right — and it's only half the problem. A sandbox decides where an agent can go; governance decides what it may do once it's there. Here's the difference, and the honest thing we did this week when we couldn't build the sandbox yet: we made the command refuse to run rather than fake it."
pubDate: 'Jul 24 2026'
heroImage: '../../assets/a-computer-is-half-the-problem.jpg'
---

LangChain published a post this week called
[*Agents Need Their Own Computer*](https://www.langchain.com/blog/agents-need-their-own-computer).
It's right, and if you run agents against anything you care about, you should read it. The
core line is the one worth remembering: a sandbox doesn't stop prompt injection, but it does
*"contain the execution blast radius."* Give the agent a real machine to work on — and keep
that machine away from your laptop, from production, and from every other agent.

We agree so completely that we spent this week on the *other* half of the same problem. And
on being honest about the half we haven't built.

## Two jobs hide inside "keep the agent safe"

There are two different jobs here, and they get folded into one word so often that people
buy one and think they got both.

- **Isolation** — *where* the agent can reach. Can it leave this folder? Touch `~/.ssh`?
  Open a socket to the internet? This is the sandbox. It's what LangChain's microVM gives
  you, and it's what "blast radius" means: if something goes wrong, how far does the damage
  spread.
- **Governance** — *what* the agent may do, and *whether the thing driving it can be
  trusted*. Is this an action it's allowed to take at all? Did the instruction come from you,
  or from a web page it just read? This is what our kernel does.

A microVM is a room with a strong lock on the door. Governance is the rules for what you're
allowed to do while you're in the room. You need both — and, this is the part people skip,
**neither one implies the other.**

## Why one isn't the other

A perfect microVM will not stop an agent that's been talked into deleting the files inside
its *own* workspace. It won't stop one from reading a secret it legitimately has access to
and mailing it out the front door. Those actions are all "inside the room" — the sandbox has
no opinion about them. Deciding whether they should happen, and whether the thing that asked
for them is trustworthy, is governance's job. (A poisoned document turning your agent into a
courier is [a threat we've written about before](/blog/the-zombieagent-threat/); the sandbox
is not what stops it.)

And governance won't stop a plain bug — the agent's own code scribbling on `/etc` because of
an off-by-one, no injection required. That's the room's job.

Different tools. The mistake isn't picking one; it's picking one and believing you have
both. We made exactly that mistake ourselves, and
[wrote it down when we caught it](/blog/governed-is-not-confined/).

## The honest part: what we shipped this week

Here's where it stops being theory, because we just lived it.

We were reviewing our own execution layer — the piece that actually runs a shell command an
agent asked for — and found something uncomfortable. On every call, that runner is handed a
policy: no network, writes only inside these folders. **And it ignored it.** It shelled out
and let the subprocess do anything the host user could do. Open a socket. Write anywhere on
disk. The policy was a decoration.

The *right* fix is the room — the OS-level sandbox LangChain is describing. You cannot fence
a subprocess with a few lines of Rust; it needs the operating system's help (namespaces, a
microVM, that class of thing). We don't have that yet. It's real work — and, agreeing with
the article once more, it should be a real isolation boundary, not a clever hack we talk
ourselves into trusting.

So we had a choice. Keep running commands and quietly pretend the policy meant something. Or
stop pretending.

We stopped pretending. The command runner now **fails closed**: with no sandbox present to
enforce the policy it was handed, it refuses to run the command at all — unless you
*explicitly* tell it, in code, "I know this runs unconfined." No silent gap. The refusal
states exactly why. A dry run (which touches nothing) still works; a real run does not, until
either a sandbox is there to back the policy or someone has signed the waiver out loud.

## Why "not yet — and we'll say so" beats a half-built jail

It would have been easy to ship a *partial* sandbox instead: fence the filesystem, leave the
network wide open, call it done. That is worse than shipping nothing, because it **reads**
like a jail. The first person who tests it finds the hole — and now every other safety claim
you've made is suspect too. A tool that overstates its own protection fails at the exact
moment someone checks it.

So the rule we hold to is small and boring: enforce what you can, refuse what you can't, and
never let the label outrun the mechanism. *"We can't confine this yet, so we won't run it and
call it safe"* is a sentence we can stand behind. A half-lit sandbox with the label "secure"
is not.

## Where the room will come from

When we do build the isolation layer, it won't be bespoke. It'll be a slot that the real
thing plugs into — a namespace backend, gVisor, a Firecracker-class microVM, quite possibly a
hosted one exactly like the article's. And the refusal we shipped this week *is that slot*:
it's the point where a sandbox will announce itself and flip the command from "refused" to
"allowed." We built the seam first, on purpose. The engine drops in behind it — and nothing
in front of it has to change.

## The takeaway

Give your agent its own computer. LangChain is right about that. Just don't mistake the
computer for the whole answer. The machine decides *where* the agent can go. Something else
has to decide *what it may do once it's there* — and whether to trust whatever is doing the
asking. A room and a rulebook. Most "agent security" I run into is one of the two, quietly
hoping it counts as both.

We spent this week on the rulebook — and on refusing to pretend we'd already built the room.
When we build the room, you'll hear about it, because we'll finally get to use the word we
couldn't this week: *confined.*
