# Social: "My Own Sandbox Blocked Me. The Fix Wasn't a Hole."

- **Post:** `blog/src/content/blog/harness-blocked-itself.md`
- **Canonical URL:** https://ai2rules.dev/blog/harness-blocked-itself/
- **Angle:** First-person engineering log — the taint floor denied the harness's own
  `WebFetch` (a true positive against its own author), and the fix was a new primitive
  (`trust_pins`, D29), not a bypass.

---

## X / Twitter — thread (5)

> **1/** My own AI sandbox just blocked *me*.
> I was reading competitor repos, went to fetch a web page — and my own security kernel refused:
> *"taint floor: context is tainted, WebFetch blocked."*
> The product worked. On its own author.
> Why I didn't punch a hole 🧵

> **2/** The floor is simple: once the agent reads untrusted data, anything that can reach the network is denied — so injected text in that data can't phone home.
> I'd just read an untrusted README. Tainted. Working exactly as designed.
> The annoying deny was the proof it's real.

> **3/** Two tempting "fixes," both wrong:
> • delete the taint state → blind reset, lifts the floor for *everything*, kills the audit trail
> • trust the whole repos/ tree → permanently lowers the floor for a dir whose job is holding untrusted code
> Both punch a hole.

> **4/** The real fix: pin trust to *content*, not a path.
> "I read these exact bytes, I vouch for them — trust them only while they stay these exact bytes."
> A sha256 attestation. Change one byte → drift → re-tainted → network blocked again.

> **5/** Taint became a recomputed *ledger of causes*, not a one-way flag. Pin a cause and it drops out of the computation but stays in the record — the live session cleared with its audit intact, no reset.
> Full write-up 👇
> https://ai2rules.dev/blog/harness-blocked-itself/

---

## X / Twitter — standalone posts (pick any)

> **A.** "A sandbox that never blocks its author isn't enforcing anything."
> My own taint floor denied my own web request mid-research. I added a new primitive instead of a bypass 👇
> https://ai2rules.dev/blog/harness-blocked-itself/

> **B.** The right response to a true positive isn't a bypass — it's a sharper primitive.
> How I fixed my AI agent blocking its own network access *without* lowering the floor:
> https://ai2rules.dev/blog/harness-blocked-itself/

> **C.** New post: I let my coding agent's security kernel block *me*, then fixed the friction with trust pins — attestations bound to content identity, with drift detection — instead of deleting the guarantee.
> https://ai2rules.dev/blog/harness-blocked-itself/ #AIagents #PromptInjection

---

## HN / Reddit title (optional seed)

- **HN:** *My own AI sandbox blocked my own web request — and the fix wasn't a hole*
- **r/LocalLLaMA / r/rust:** *I let my coding agent's taint floor block me, then fixed it with content-pinned trust attestations (not a bypass)*
