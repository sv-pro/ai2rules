---
title: 'What Your Coding Agent Lets You Control — and Three Things We Got Wrong'
description: "Almost everything written about AI agent safety measures how the model behaves. Almost nothing measures what the product permits. We wrote down nine yes/no questions about that, answered them for the tools we use every day, and the first thing the answers did was correct three claims of our own."
pubDate: 'Aug 8 2026'
heroImage: '../../assets/what-your-coding-agent-lets-you-control.jpg'
---

Here is a question you can answer about your coding agent in about five minutes, without
running a single agent task:

**Can a program of yours see a tool call before it happens, and say no?**

Not *"will the model behave sensibly."* Not *"how good is it at refusing bad instructions."*
Just: does the product let your code get a word in first. It is a question about the
software, not about the model, and it has a yes-or-no answer that will still be true in
six months.

Almost everything published about agent safety measures the first kind of question. Almost
nothing measures the second. So we wrote down nine of the second kind, answered them for
the tools we actually use, and published the answers.

Then the answers marked us down three times. That is the part worth writing about.

## The awkward part first

We build a governance tool for coding agents. That is a problem for anyone we ask to trust
a scorecard we wrote, because a scorecard that rewards *"has a hook my product can plug
into"* is an advertisement wearing a table's clothes.

So the index carries three rules, and they constrain us rather than excuse us:

1. **No question may require our tooling to answer.** If you cannot check a cell without
   ever having heard of us, it does not belong in the index — however useful it would be.
2. **No question about model behaviour.** Only about what the product permits. That is what
   keeps a result from March still true in November.
3. **No single score.** Per-question answers only. One number would hide exactly the one of
   the nine you should have cared about.

And a fourth that turned out to matter most: **"we looked and didn't find one" is not a
measurement.** Unknown is a publishable answer. A guess is a defect.

Those rules cost us something immediately.

## Three claims of ours that the measurement corrected

**1. "Our hook can grant permission." Downgraded to unknown.**

One of the nine questions is whether your code can return *allowed* and have the host skip
its own approval prompt. This is the difference between an overlay and an authority. Without
it, your policy can only ever add friction on top of the host's permission list; with it,
your policy can *be* the list.

We had that marked yes for Claude Code. Our evidence was our own demo, which shows that
**our hook emits an "allow" decision**. Read that again, because we didn't for a while: it
shows what our code sends, not what the host does with it. Those are different claims and we
had only tested the first.

By the index's own rule — that "documented" means the *vendor's* documentation — our demo
never qualified. The cell is now **unknown**.

**2. "Our hook can grant permission on Antigravity." Measured. It cannot.**

Same claim, different tool, and this time we could actually test it, because Antigravity's
headless mode has a useful property: it cannot show you a prompt, so when something *would*
have prompted, it refuses and tells you why. "Would have prompted" becomes an observable
outcome with nobody sitting there to click.

A hook returning `allow` left the command refused, with the same message as a hook returning
nothing at all.

That result only means something because of the control we ran next: the same hook returning
`deny` **did** stop a command, and the tool told the model *"blocked by a system hook"*,
quoting our reason text. So the hook is being consulted and obeyed. `deny` is authoritative;
`allow` is not. Overlay, not authority — which is exactly what the question was asking.

Our own documentation said otherwise. It has been corrected.

**3. "Configuration is partly file-based." It is worse than partly.**

Another question asks whether everything the agent can reach is written in files you can
read, diff and commit — or whether some of it exists only in a settings screen.

We had this at *partial*. Then we counted. In one working session there were **six MCP
servers live**. **Exactly one** was declared in a configuration file.

Four of the others appear on disk only inside a list recording what has *ever* been
connected — which is history, not configuration. It included a connector that was not
running, and editing it changes nothing about what loads. The sixth left no trace anywhere.

A surface you cannot enumerate from disk is one you cannot review, diff, or put in version
control. One out of six is not *partial*. The cell now reads **no**.

## What we learned about the tools themselves

Since the point of all this is the practitioner's question, the useful findings:

**You can refuse a built-in tool, but you cannot remove one.** A deny rule makes the call
fail; the tool is still there, still offered, still callable. We also tried the other
mechanism — a `disallowedTools` list in the settings file — and it did nothing at all. Not
an error, not a warning: the call simply succeeded. A setting that is accepted and ignored
is worse than one that is rejected.

**There is a post-execution hook, and it lets you count what you stopped.** A pre-execution
hook records what was *proposed*; a post-execution hook records what actually *ran*, with the
result and how long it took. Subtract one from the other and you have a direct count of calls
that were proposed and never happened. We tested it with three deliberately-blocked commands:
the pre-hook logged four events, the post-hook logged one, and the three refusals appear in
no post-hook line at all. Most governance tooling can only estimate that number. Here you can
just count it.

**A denial is a message to the model, not a log line.** When a hook refuses a call, the
reason text you write is handed straight to the assistant. That makes the wording part of the
interface. "Denied by policy" teaches the model nothing; "denied: this path is outside the
project root" tells it what to do differently.

## The method, which is the transferable part

Three separate times during this work, a single run looked like a clean answer, and the
control run destroyed it.

The clearest case: we installed a hook that granted permission, ran a command, and it worked
with no prompt. That looks like proof. Then we removed the hook, ran the identical command,
and it *also* worked with no prompt. Nothing had been demonstrated — the command was going to
succeed either way, and the hook was irrelevant to the outcome we'd just observed.

**The measured quantity is the difference between two runs, not the outcome of one.** All
three near-misses would have published a confident, wrong claim about somebody else's
product.

There is a second failure worth naming because it is embarrassing and easy to repeat. One
measurement appeared to show a tool call bypassing our hook entirely — a dramatic result. It
was not real. The command being tested shared a line with the command that cleared the log,
so the hook faithfully wrote its record and the test then deleted it. The instrument was
standing in its own output. When a result looks dramatic, suspect your instrument first.

## What we don't know

Two of the nine questions are still unanswered for Claude Code, and not for lack of trying.
Both need an action the tool will *prompt* about, and on the machine we tested, nothing
prompted: an unrecognised command, the same with the sandbox disabled, and a file write
outside every permitted directory all ran silently. A hook explicitly asking for a prompt
didn't produce one either. Those cells need a differently-configured machine, not more
effort, so they say **unknown** and explain why.

Two whole columns — Codex CLI and Copilot — are entirely unknown. We have not run the
procedures there. Listing them with guesses would be worse than listing them empty.

## Take it and check us

The parameter definitions and the results table are
[in the ai2rules repo](https://github.com/sv-pro/ai2rules/blob/main/docs/GOVERNABILITY-INDEX.md).
The step-by-step procedures — what to type and what to look for, including the ones we ran
and the ones we haven't — are in
[agentic-coding-lab](https://github.com/sv-pro/agentic-coding-lab/tree/main/artifacts/governability-probe).
The procedures are MIT; the index lives in a repo dual-licensed MIT or Apache-2.0. Copy
either. None of it requires our software.

Every cell carries a date and a version number, because these products ship weekly and a
result without them isn't a result. A cell changes when somebody runs the procedure and
publishes what they saw — including when what they saw contradicts us. That has already
happened three times, and all three times it was us doing the contradicting.

That is the whole design. An index that has never cost its author anything isn't measuring;
it's marketing.
