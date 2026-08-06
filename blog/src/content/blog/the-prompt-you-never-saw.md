---
title: 'The Prompt You Never Saw'
description: "A new Microsoft protocol for AI coding agents includes a type that decides when you get asked to approve a tool call. The only source it can name for that decision is a model, and the answer is a number between 0 and 1. Here is why that is the wrong shape — and the narrower, truer version of the complaint."
pubDate: 'Aug 6 2026'
heroImage: '../../assets/the-prompt-you-never-saw.jpg'
---

If you have used an AI coding assistant, you know the prompt.

> Run `rm -rf build/` in your terminal? **[Allow]** **[Deny]**

Something decided you should see that. Something else decided you should *not* see the
other forty commands that ran during the same session without asking. That second decision
is the invisible one, and it is by far the more interesting of the two. Almost nobody looks
at how it gets made.

In March, Microsoft published a specification called the **Agent Host Protocol**. It solves a
real and slightly boring problem: if an AI agent is working on your code, you might want to
watch it from your editor, your phone, and a browser tab at the same time, and you would like
all three to show the same thing. That is harder than it sounds. The protocol is a careful
piece of engineering — an agreed-upon way for several screens to share one agent session
without drifting out of sync. It is open source, MIT-licensed, at version 0.7, with a
reference implementation inside VS Code.

Somewhere in the middle of it, in the types that describe a tool call waiting for your
approval, is this:

```ts
/** Identifies a model judge as the source of a confirmation requirement. */
export const enum ToolCallRiskAssessmentKind { Judge = 'judge' }

export interface ToolCallRiskAssessmentCompleteState {
  status: ToolCallRiskAssessmentStatus.Complete;
  reason: StringOrMarkdown;
  /** The judge's normalized safety score, where `0` is unsafe and `1` is safe. */
  safety: number;
}
```

Two things are worth unpacking for anyone who does not read TypeScript for a living.

An **enum** is a fixed menu of allowed values — the complete list of things a field is
permitted to be. This one has exactly one item on it: `Judge`. In the protocol's own words,
a *model judge*: another AI, asked to look at the pending action and say how dangerous it
is.

And the answer it gives back is `safety` — **a number between 0 and 1**.

So the protocol can express *"a model looked at this command and scored it 0.31."* It has no
way at all to express *"a rule we wrote down last Tuesday says anything that touches the
network gets a prompt."* The second sentence has nowhere to go. There is no field for it.

## First, the part where we don't overclaim

This is where a more exciting version of this post would go badly wrong, so let us kill it
early.

**The judge does not decide whether your command runs. It decides whether you get asked.**

Those are different, and the difference matters. Nothing in that type blocks a tool call.
If the assessment says a command is fine, the command proceeds the way it would have anyway;
if it says a command is risky, you see a dialog and *you* decide. It is an escalation hint —
a suggestion about when to interrupt a human — not a security control.

Anyone who tells you Microsoft has shipped a protocol where an AI decides what other AIs are
allowed to do is describing something that is not there. We nearly wrote that sentence
ourselves. It was a better headline and it was false.

Here is the version that survives contact with the actual code.

## The failure mode is a dialog that never appears

An escalation hint fails quietly in one direction and loudly in the other.

If the judge is wrong in the cautious direction, you get an unnecessary prompt. Mildly
annoying, entirely visible, and you learn to trust it less.

If it is wrong in the other direction, **nothing happens**. No dialog. No entry that says "we
considered interrupting you and decided not to." Afterwards, a session where the judge scored
a destructive command at 0.9 and a session where the command was genuinely routine look
exactly alike. The record shows a tool call that ran without confirmation, and that is all it
ever shows.

That is the same shape as your spam folder — a score decides what reaches you — with one
important difference. You know your spam folder exists. You go and check it when something
important never turns up. There is no folder here.

And there is a second entry in the same file worth reading next to it. Alongside "the user
approved this" and "no approval was needed," the protocol records a third possibility:

> `Setting` — Approved by a persistent user setting.

That is the **Always Allow** button, promoted to a protocol-level fact. It means an approval
can be satisfied by a decision you made three weeks ago, on a different task, in a different
mood. Afterwards, nothing distinguishes a yes from a present human and a yes from a stored
preference.

We know that one is real because we shipped it. Our own tool governs Google's Antigravity CLI,
and we found that when our kernel said *ask the human*, the host could satisfy that request
out of its own cache of past "always allow" answers. A question we thought we were asking had
already been answered by somebody who was no longer in the room. We changed the default so
that asking is not cacheable, and left the old behaviour available behind a flag for people
who want the convenience and now know what it costs.

## Smoke alarms and fire doors

The useful comparison is not "good tool versus bad tool." It is two different kinds of thing.

A **smoke alarm** is a sensor. It is probabilistic, it has false positives, and it burns
toast into a fire alarm about once a month. That is fine, because it is cheap and its job is
to raise attention.

A **fire door** is structure. It does not have an opinion about whether there is a fire. It
does not score the corridor. It is a slab of material in a doorway, and it holds whether or
not anything detected anything.

Buildings have both, and nobody argues about it. What nobody does is remove the fire doors on
the grounds that the smoke alarms have got very good.

The model judge is a smoke alarm, and a reasonable one. The complaint is not that it exists.
The complaint is that in this protocol's type system, **the smoke alarm is the only fixture
that can be named**. There is no slot for a fire door.

## What the other shape looks like

The alternative is not more sophisticated. It is duller, which is the point.

A decision that gets written down before the call happens, in a file a human edited. Not "how
risky does this look" but "commands in this class require approval; this capability is not
available in this workspace at all." Then, when something is stopped or escalated, the record
does not say `0.31`. It says which rule fired.

That gives you three things a score cannot. The same input produces the same answer every
time, so you can test it. Two people reading the record reach the same conclusion about *why*,
so you can argue with it. And a capability that was never granted cannot be talked into
existing — there is no prompt to phrase persuasively, because the thing being asked for is
not on the menu.

None of that requires the judge to go away. Run both. A structural rule decides what is
possible; a model judge is welcome to raise its hand about anything that remains.

## The narrow ask

`ToolCallRiskAssessmentKind` currently has one member. The fix is a second one: a variant
whose payload is not a safety score but a rule identifier, a hash of the ruleset it came
from, and the decision that ruleset produced.

That is a small change today. The protocol is at 0.7, it is MIT-licensed, it has a proposals
folder and two dozen contributors, and its own documentation says breaking changes are still
expected. After 1.0, adding a second way to answer "why is this being escalated" stops being
a pull request and starts being a migration.

We have written down our side of this as a formal decision, including the rule we hold
ourselves to: our kernel may *produce* a risk assessment for a protocol like this one, and
must never *consume* one. A verdict that arrives over a display channel, from a client that
optimistically applies its own changes before the host confirms them, is a rumour. It is
excellent for drawing a user interface and disqualifying as an input to a decision about
authority.

The prompt you see is a design choice. So is the one you don't.

---

*The protocol is at [microsoft/agent-host-protocol](https://github.com/microsoft/agent-host-protocol).
Our reasoning, including the parts we could not verify and therefore did not claim, is
recorded as D53 in [DECISIONS.md](https://github.com/sv-pro/ai2rules/blob/main/DECISIONS.md).*
