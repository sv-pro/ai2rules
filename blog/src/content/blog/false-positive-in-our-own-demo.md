---
title: 'We Found a False Positive in Our Own Flagship Demo'
description: "A paper proved that a safety check shaped like ours has to be wrong somewhere. It was — and the bug was hiding in our own demo. Here's the per-argument fix, and the one part of the paper we wouldn't adopt."
pubDate: 'Jul 18 2026'
heroImage: '../../assets/false-positive-argument-taint.jpg'
---

Most security papers hand you a system to measure yourself against. This one handed
us a proof — and the proof was about our own code.

The paper is [*The Granularity Mismatch in Agent Security*](https://arxiv.org/abs/2605.11039)
(Fan et al., 2026). Its claim, once you sit with it, is hard to shake: a prompt
injection does its damage not when the untrusted text is merely *sitting in the
agent's context*, but when it gets to **decide an argument that carries authority** —
the address a request is sent to, the command that runs, the secret that's used.
Watch a tool call as a single unit and you're looking at the wrong resolution. And
the paper *proves* that a guard built that way has to be wrong somewhere.

We run a guard built exactly that way. So we pointed the proof at it.

## First, what "taint" means

Our safety floor runs on *taint*, and it helps to know the idea isn't new. Its most
familiar home is the Linux kernel: load a proprietary driver and the kernel marks
itself **tainted** — and it stays tainted even after you unload the driver, because
the point was never the driver, it's that the kernel's trustworthiness is already
spent ([kernel docs](https://docs.kernel.org/admin-guide/tainted-kernels.html)). Perl
shipped the same reflex as *taint mode* back in 1989. We aim it at agent tool calls:
anything that came from an untrusted source is **tainted**, taint only ever spreads
(it never washes back out), and once a session is tainted, nothing that can reach the
outside world — a web request, a file write — is allowed out. That's the floor behind
the [ZombieAgent post](/blog/the-zombieagent-threat/) and the
[subagent experiment](/blog/subagent-taint-experiment/).

## One label for the whole call

Here's the catch, and it's the whole story. Our floor tracks taint as a **single
label for the entire session**. Any tainted value anywhere, and the next tool call is
treated as tainted, full stop. In the kernel it's essentially one line: *if this call
can reach outside and the session is tainted, block it.*

The paper has a name for exactly this shape — a **flat tool-level monitor**, one taint
label per call — and it proves that any monitor built this way must, in a session that
mixes trusted and untrusted data, make one of two mistakes: let something dangerous
out, *or* block something harmless. We don't let dangerous things out; the floor is
conservative to a fault. So the other mistake had to be hiding in there somewhere — a
harmless call we wrongly block. The proof said it existed. It didn't say where.

## We found it in our own demo

It was in the flagship. `poisoned_knowledge_demo` reads a poisoned document, then
makes three web requests. Two of them matter:

- One goes to `attacker.evil/collect?k=SECRET` — an address lifted straight out of the
  injected instruction. **Blocked.** Exactly right.
- One goes to `docs.example/guide` — a fixed address hard-coded into the demo, the same
  harmless request it made *before* it ever saw the poison. **Also blocked.**

We had shipped that second block as a *feature*: "same call, opposite verdict — the
session learned to be careful." The paper reframes it as the bug. That address is
clean; it has nothing to do with the poison. Only the *session around it* is tainted.
But the floor can't tell the two apart, because by the time it runs it holds one label
for everything and no idea which argument the taint actually touched.

Here are the same three calls through the real kernel, next to a check that looks at
each argument's *own* history instead of the whole session's:

```text
request                                session floor   per-argument   result
1. normal request, clean session       allow           allow          agree
2. exfil: address built from poison    BLOCK           BLOCK          agree — still safe
3. normal request, tainted session     BLOCK           allow          ← the false positive
```

Row 3 is the proof made concrete, sitting in our own repository.

## Adopt the proof, refuse the classifier

The paper has two parts, and we treat them very differently — this is the part worth
reading closely.

The **enforcement** part is a fixed set of rules: compare a few labels, check a few
sets, decide. No model anywhere in it. We can take that as-is; it's the same kind of
machinery our kernel already is.

The **inference** part is how the paper *fills in* those labels while the agent runs,
and it leans on a language model to classify the ambiguous arguments. That's a model
deciding what's allowed — the one thing [we never do](/blog/why-deny-is-dangerous/).
And it shows: on real tools, their inference is right about **77% of the time**. A gate
that's right three times in four isn't a gate.

So we keep the rules and drop the classifier. Our argument labels aren't guessed at
runtime — they're written down ahead of time, by a human, in the tool manifest. The
paper's weak spot is a step we simply don't have.

## The fix — the floor never moved

The floor rule itself didn't change. Three things around it did:

1. **Each argument can carry a role.** In the tool manifest an argument is tagged as
   an address, a command, a credential, and so on — the roles that carry authority.
   `fetch_web` opts in with a single line. It's hand-written data, not code and not a
   model.
2. **Taint is tracked per argument, not just per session.** The old single label stays
   on as a backstop.
3. **The floor is handed a sharper number.** When a tool declares roles, the floor
   judges it on the taint of its *authority-carrying arguments only* — the address,
   the command — and ignores taint on arguments that merely carry text. When a tool
   declares nothing, it behaves exactly as before.

Then the piece that makes it real: a step that fills in each argument's taint from
where its data *actually came from* — no model, just following the data. It's
deliberately paranoid. An argument counts as clean only if we can show its value came
straight from the user's request; tainted only if we can show it came from poisoned
output; and when it can't tell, it says nothing and the call falls back to the old
session floor. No proof of a clean origin, no relaxation.

## The result

Run it back through the kernel:

- The harmless request in a tainted session (row 3) is now **allowed**. The utility we
  were throwing away is back.
- The exfil (row 2) is still **blocked** — its address was built from the poison, so it
  never looked clean.

**152 tests pass**, and the false positive in our own demo is now something we assert
on, not a feature we misread. Nothing regresses: a tool that declares no roles behaves
exactly as it did yesterday.

## The honest caveat

The floor was never *wrong* — just blunt. It over-blocked because it couldn't prove the
address was clean, so it assumed the worst. We didn't loosen a safety rule; we gave it
better information.

And the win has a hard edge, on purpose. We follow *exact* data flow. Paraphrase the
poisoned instruction — reword it, summarize it — and the trail stops being a literal
match, so the call falls back to the blunt floor and gets blocked again. That's
precisely the fuzzy case the paper hands to its language model. We don't. We'd rather
over-block on a rule we can trust than allow on a guess that's right 77% of the time,
because a gate you can't trust is worse than one that sometimes says no. Recover
utility where the origin is *provable*; stay shut where it isn't.

## The takeaway

This is the loop we want. A real paper — not a hunch — aimed a proof at our code. The
proof found a real bug: a false positive we'd been shipping as a feature. The fix
sharpened the kernel without loosening it, and drew the line between fixed rules and
model guesses in exactly the place we always draw it. Take the proof, refuse the
classifier.

"Same call, opposite verdict" used to be a slogan. Now it's something we can show one
argument at a time. The floor still never washes out. It just finally knows which
argument it's afraid of.
