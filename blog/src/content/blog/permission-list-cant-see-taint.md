---
title: "Your Permission List Can't See Taint"
description: "A Claude Code permission list can't tell a command you typed from one a web page slipped in. A governed manifest can — and replacing the pile turns on a trick most people get wrong: a hook that says 'allow' doesn't defer, it grants."
pubDate: 'Jul 21 2026'
heroImage: '../../assets/permission-taint-gate.jpg'
---

Open your Claude Code `settings.json` and look at the `permissions` block. If you've
lived in it a while, it's a pile: `Bash(npm run test:*)`, `Edit(src/**)`,
`WebFetch(domain:…)`, an `mcp__…` line you don't remember adding. It grows, nobody
prunes it, and no one can read it top to bottom and say what the agent can actually do.

That's the visible problem. Here's the one underneath it: **the pile is blind to where
a request came from.** `Bash(curl:*)` says "curl is allowed." It cannot say "curl is
allowed *unless the agent just read a web page that told it to run this one*." A flat
allowlist has no notion of provenance — and provenance is the whole game in prompt
injection.

There's a name for the thing the pile can't see. It's *taint*.

## Taint, and why it isn't new

The idea is older than agents. Its most familiar home is the Linux kernel: load a
proprietary driver and the kernel marks itself **tainted** — and it stays tainted even
after you unload the driver, because the point was never the driver, it's that the
kernel's trustworthiness is already spent
([kernel docs](https://docs.kernel.org/admin-guide/tainted-kernels.html)). Perl shipped
the same reflex as *taint mode* in 1989: data from outside the program is marked, the
mark spreads to anything derived from it, and tainted data isn't allowed to do dangerous
things until you've explicitly checked it.

Point that reflex at an agent's tool calls and you get a safety floor: anything that
came from an untrusted source is tainted, taint only ever spreads (it never washes back
out), and once a session is tainted, nothing that reaches the outside world — a web
request, a file write — is allowed out. That floor is what stops a
[poisoned memory](/blog/the-zombieagent-threat/) from turning into an exfiltration.

Now look back at the `settings.json` pile. It has nowhere to *put* a taint mark. It
matches on the shape of a call — the tool, the argument pattern — and never on the
call's history. That isn't a missing feature you could bolt on. It's the wrong kind of
object.

## Govern the actor, not the file

The reframe is small and it changes everything: you're not editing a tool's config
file, you're **describing the world a particular actor is allowed to act in.** The actor
is Claude. The description is a manifest — one reviewable artifact that says which
actions exist, what each one may touch, and how trust flows through them.

Two things a manifest can express that a pile can't:

- **Absence, not refusal.** A dangerous tool isn't on a deny-list the model can argue
  with; it simply *doesn't exist* in the agent's world. We've written about
  [why that distinction matters](/blog/why-deny-is-dangerous/) — `ABSENT` is not `DENY`.
- **Taint.** The manifest carries the floor above. The same request is allowed with a
  clean history and denied with a tainted one.

Which raises the obvious question: your host already *has* a permission system. Can a
manifest actually replace it, or only nag from the sidelines?

## The trick that makes it possible: "allow" doesn't mean allow

Claude Code lets you run a **PreToolUse hook** — a small program that sees every tool
call before it runs and returns a verdict. Here's the part almost everyone gets wrong,
and it's the crux of the whole thing. A hook has three ways to *not* block, and two of
them look identical until they don't:

- **Defer** — the hook exits quietly with no verdict. The call falls through to Claude
  Code's *normal* permission flow, which may still show you an Allow/Deny prompt.
- **Grant** — the hook explicitly answers `"allow"`. Claude Code **skips the prompt** and
  runs the call. It doesn't defer; it *authorizes*.
- **Escalate** — the hook answers `"ask"` and forces the prompt.

"Allow" in plain English sounds like "don't block." In the hook contract it means
*grant* — silence the prompt, wave it through. The gap between those two readings is a
genuine footgun: a security hook that returns `"allow"` as its default "nothing to see
here" doesn't hand control back to the permission system, it **replaces** the permission
system, silently, for every call that passes its check. That behavior has been
surprising enough to file about, more than once
([1](https://github.com/anthropics/claude-code/issues/28812),
[2](https://github.com/anthropics/claude-code/issues/52822),
[3](https://github.com/anthropics/claude-code/issues/18312),
[4](https://github.com/anthropics/claude-code/issues/39344)).

Read as a footgun it's a hazard. Read as a hinge, it's exactly the door we want: if a
hook can *grant*, then a manifest driving that hook can be the whole policy — not an
overlay on top of the pile, a replacement for it.

## Replacing the pile

So we empty the `settings.json` permission block down to nothing and let the manifest do
the deciding. In our harness that's a hook running in "grant" mode against a world
manifest: it answers `allow` for what the manifest permits, blocks what it doesn't, and
carries the taint floor underneath.

Here is the same handful of actions, judged by the old pile and by the manifest:

```text
what the agent tries                     the pile        the manifest
a clean file read                        allow           allow  (granted, no prompt)
curl to a web page                       allow           allow  (granted — session now tainted)
the same kind of curl, right after       allow           DENY   ← the pile can't see the difference
rm -rf on a path                         a match, maybe  ask
a tool you never declared                absent = ¯\_(ツ)_/¯  ABSENT — it doesn't exist for the agent
```

Row three is the whole post in one line. Same command, opposite answer — because between
the two the session touched the network, and the manifest can see that. The pile matches
both `curl` calls against the same rule and lets both through. No amount of pruning your
allowlist fixes that, because the allowlist is asking the wrong question.

## The honest caveats

This is a real mechanism, not a magic wand, and the boundaries matter.

- **It governs *calls*, not the *surface*.** A hook can deny or grant a call, but it
  can't make a tool vanish from the model's menu. Full `ABSENT` — the tool genuinely not
  existing — still needs the host to prune its tool list. The manifest becomes the source
  of truth for *decisions*; making tools disappear is a separate lever.
- **Empty the baseline, or it fights you.** A native `deny`/`ask` rule still fires even
  when the hook grants. Leave rules in `settings.json` and they'll quietly override the
  manifest. Replace mode wants the pile actually *empty*.
- **Some prompts can't be silenced,** by design — an org-mandated approval, or a tool
  flagged as needing a human, still asks no matter what the hook says. Good.
- **Pin your version and check.** The grant behavior has shifted across releases (that's
  what those issues are about). Verify it does what you expect before you rely on it.

## Try it where it can't hurt you

One flag in this setup — the one that makes undeclared tools `ABSENT` — will lock the
agent out of anything your manifest forgot to list. That's the *point* when you're
testing a policy, and a disaster if you do it to the Claude you're currently working in.
So don't. Run the experiment against a **throwaway, containerized Claude** — a disposable
instance with the manifest mounted in and the internet fenced off behind an
allowlist, exactly the [sandbox we've written up before](/blog/running-claude-safely/).
Lock that agent out and the fix is `docker rm`, not a ruined afternoon.

The container is also the honest home for the emptied `settings.json`: the stripped-down
config lives *in the container only*, overlaid at runtime, while your real project file
never changes. Separate the config you develop with from the config that governs the
runtime — and get to be reckless in the one place recklessness is free.

## The takeaway

A permission list is a fine thing for what it is: a set of shapes you've decided to wave
through. But it will never know that the shape it's waving through was suggested, thirty
seconds ago, by a document the agent doesn't trust. That's not a rule you can add. It's a
question the format can't hold.

A manifest can hold it. Give the actor a world instead of the tool a list, put the taint
floor underneath, and let the hook *grant* instead of defer — and the same `curl` that
was fine a moment ago is refused now, for the only reason that ever mattered: not what it
does, but where it came from.
