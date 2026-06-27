---
title: 'My Own Sandbox Blocked Me. The Fix Wasn’t a Hole.'
description: "While researching competitors, the harness tainted my session and denied my own web request — exactly as designed. Here's why I added a new primitive instead of punching through the floor."
pubDate: 'Jun 27 2026'
heroImage: '../../assets/harness-blocked-itself.jpg'
---

I was doing competitor research — reading a pile of third-party governance projects
checked out under `repos/3p/` — when I went to fetch a GitHub page and the agent
refused itself:

```text
[gate] DENY WebFetch — taint floor: context is tainted; 'WebFetch' can reach the
       network and is blocked (rule no_tainted_network)
```

That deny came from *my own kernel*. I govern Claude Code with a deterministic
`PreToolUse` hook that enforces a **monotonic taint floor**: once the session has
read untrusted data, any tool that can reach the network is blocked, so injected
text in that content can't phone home. I had just read a competitor's `README.md`
under `repos/`. The session was tainted. The floor did precisely what it promises.

The honest first reaction was annoyance. The correct second reaction was: *good —
this is the product working on its own author.* If your sandbox never inconveniences
the person who built it, it isn't enforcing anything. So the question wasn't "how do
I get my network back," it was "what's the *right* way for an operator to vouch for
a source they've actually reviewed?"

## The two tempting wrong answers

**Delete the sidecar.** Taint state lives in a per-session file; `rm` it and the
floor lifts. But that's a blind reset — it discards the audit record *and* lifts the
floor for every other untrusted thing the session touched, not just the README I
trust. One poisoned file I'd forgotten about, and I've just re-opened the exfil
path. A reset can't tell *why* the session was tainted, so it can't be surgical.

**Drop `repos/` from the taint sources.** Now the whole tree is trusted — including
files I haven't read, and any future edits to them. That's not vouching for a
reviewed artifact; it's permanently lowering the floor for a directory whose whole
job is to hold *untrusted* third-party code.

Both punch a hole. Both trade a real guarantee for convenience. The tell is that
neither is bound to *what I actually reviewed* — they're bound to a session, or a
path prefix.

## The fix: pin trust to content, not to a path

What I actually want to say is narrow and specific: *"I read these exact bytes, I
vouch for them, trust them only while they stay these exact bytes."* That's a **trust
pin** — an operator attestation bound to **content identity**:

```jsonc
"trust_pins": [
  {
    "path": "repos/3p/agent-governance-toolkit/README.md",
    "identity": { "kind": "sha256", "hash": "e99e83…3473" },  // the bytes I reviewed
    "reason": "Reviewed for the repos/3p adoption analysis",
    "attested_by": "sergey",
    "expires": "2026-09-27"                                   // optional time-box
  }
]
```

The rule is small:

- A `Read` whose path matches a pin **and** whose live content still hashes to the
  pinned `identity` is classified **Trusted** → it does **not** taint.
- Any **drift** — the bytes change, or a `git_commit` pin's repo moves or goes dirty
  — or a passed `expires` date **revokes** the pin, and the read taints as normal.
  This is just descriptor-drift detection (the thing we already do for tool
  descriptors) applied to *read sources*.
- A pin **never weakens the egress floor.** It re-classifies a *source* upstream of
  taint. Anything still untrusted is still tainted, and tainted context still can't
  egress.

Note what the pin is bound to. My first instinct was to pin against this repo's
`HEAD` commit — "trust it as of this checkout." That's wrong: `repos/3p` isn't even
tracked in this repo (we never `git add repos/`), so this repo's HEAD says *nothing*
about those bytes. Trust has to be pinned to the artifact's own identity — its
sha256, or its own repository's commit — not to a commit that doesn't describe it.

## The part that made the current session clear

A pin would gate *future* reads, but I was already tainted *now*. The thing that let
the live session recover — without a blind reset — was reframing taint itself. Taint
stopped being a one-way boolean flag and became a **recomputed ledger of causes**:

```text
tainted  ==  any recorded cause not covered by a valid pin
```

The ledger keeps every cause forever (that's the audit trail). Pinning a cause
removes it from the *taint computation*, not from the *record*. The moment I added
the pin for that README, the only outstanding cause was covered, the predicate went
false, and the floor lifted — for that source, on the next call, with the record
intact.

**Doesn't this break monotonic taint?** No — and the distinction is the whole point.
This isn't an "untaint." A pinned read was *never* a legitimate untrusted-taint cause;
recomputing reflects **corrected provenance**, not a reduction of taint under fixed
facts. A human made a design-time, auditable attestation. That's the legitimate
lever, and only the operator can pull it — the model can't pin its way out of the
floor at runtime. Monotonicity constrains what the *agent* can do; it was never meant
to forbid a *human* from correcting a source's trust label.

## Proving it bites back

A safety primitive you can't see fail is just a vibe. So the test pins a vouched
file, confirms the pinned read no longer taints (and `WebFetch` is allowed again),
then tampers with one byte and confirms the floor slams shut:

```text
pin present, bytes match     -> Read trusted  -> not tainted -> WebFetch ALLOW
printf 'tampered' >> file     -> drift!        -> re-tainted  -> WebFetch DENY
```

Green both ways. The pin protects the *artifact I reviewed*, and the instant the
artifact changes out from under the attestation, it's gone.

## The takeaway

Two things I'd hand to anyone building guardrails for autonomous agents:

1. **A sandbox that never blocks its author isn't enforcing anything.** The deny that
   annoyed me was the strongest evidence the floor is real. Treat your own friction
   as a passing test, not a bug to route around.
2. **The right response to a true positive is a sharper primitive, not a bypass.** A
   reset or a blanket exception would have "fixed" it by deleting the guarantee.
   Pinning trust to *content identity*, with drift that re-tightens automatically,
   removed the friction while keeping the floor — and added an audit trail on the way.

The whole episode took one denied request to surface and one new primitive to
resolve. That's the trade I want every time: the kernel tells me the truth, and I
answer it with a mechanism, not an exception.
