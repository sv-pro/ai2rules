---
title: 'A Scanner Flagged Our Supply-Chain Package. It Was Right.'
description: "Socket gave our governance tool 64% on supply chain security. The install script fetched a binary from the internet, unpacked it with a shell and made it executable, which is a fair description of the thing scanners exist to catch. What we changed, and the honest reason the score only moved five points."
pubDate: 'Aug 12 2026'
heroImage: '../../assets/a-scanner-flagged-our-security-package.jpg'
---

We publish a tool that governs what AI coding agents are allowed to do on your
machine. Shortly after it went on npm, [Socket](https://socket.dev) scored it:

**Supply Chain Security: 64%.**

The instinct is to argue. We know what our code does. The scanner doesn't.

The instinct is wrong. Installing our package actually did this:

1. run a `postinstall` script
2. `fetch()` a binary from the internet
3. unpack it by shelling out to `tar`
4. `chmod 0o755` it
5. leave it sitting there, executable

Read that as a stranger. That is not *similar to* a supply-chain attack; it is a
step-by-step description of one. No scanner can tell our download from a hostile
one, and a scanner that tried to would be worse at its job, not better.

## The flaw the score did not catch

The score is a number. The thing underneath it was a real weakness, and we'd have
kept missing it if we'd spent the afternoon disputing the number instead.

We verified the download. `install.js` fetched a `.sha256` alongside the binary and
refused to install on a mismatch. That felt responsible.

It fetched the checksum **from the same host, at the same moment, as the artifact
it was verifying.** Anyone able to replace the binary could replace the digest
sitting next to it. What that check actually proves is that the file arrived
without being corrupted in transit.

That's **integrity**. It is not **provenance**. And the difference between those two
is a distinction this project spends its entire existence drawing everywhere else,
we'd written thousands of words on why a claim needs to be traceable to who made it,
then shipped an install path that couldn't tell you who made the binary.

## What we changed

The fix isn't a better checksum. It's not having the problem.

The binary now ships as a **per-platform package** (`ai2rules-harness-linux-x64`,
`-darwin-arm64`, and so on) listed in `optionalDependencies`. npm resolves exactly
one by `os`/`cpu` and skips the rest. The main package has **no `scripts` block at
all.**

```
$ npm install ai2rules-harness
added 2 packages in 852ms
```

Two packages. Under a second. **Nothing ran.** No network request, no shell, no
`chmod`, no script of ours executing on your machine at install time.

Three things follow, and only one of them is about the score:

- **The binary is covered by the integrity hash npm writes into *your* lockfile.**
  Not a digest we fetched: one your package manager recorded and will check
  forever. That's the provenance we didn't have.
- **Publishing moved into CI with signed attestation**, so each tarball is bound to
  the workflow and commit that produced it.
- **Installs became reproducible, offline-cacheable, and usable behind a corporate
  proxy or a mirrored registry**, none of which was true when installing meant
  reaching out to GitHub.

That third one is the giveaway that this was a real improvement rather than a
cosmetic one. Nobody's install got *worse* to make a scanner happier.

## The anticlimax

We rescanned. **64 → 69.**

Five points. For deleting the entire install-time attack surface.

The temptation here is to go hunting for the remaining thirty-one, and that
temptation is exactly what we spend our time telling other people not to indulge.
So we read the alert list instead. There are two:

- **"Unpopular package."** Zero weekly downloads, two days old.
- **"URL strings."** The package contains a URL. The URL is
  `https://github.com/sv-pro/ai2rules` — our own source repository.

That's the list. Neither is removable. The first is time and adoption; the second
would require hiding where our code lives, which is worse on every axis a security
scanner is supposed to care about.

**The absences are the actual result.** No install-script alert. No network alert.
No shell alert. No filesystem alert. Every signal the restructure targeted is
gone. What holds the number at 69 is a popularity metric and a link to our own
source.

Socket's docs explain the rest: the final score is raised to a power scaled by the
size and popularity of a project, compressing penalties for large, established
packages. That scaling helps npm's giants and does nothing for something published
on Tuesday. A two-day-old package with no users being treated as unknown is a
scanner behaving correctly, and we're the wrong people to complain about a system
that declines to assume good faith on no evidence.

## Three things worth stealing

**Take the flag seriously before you take it personally.** The number was a summary
of behaviour we had chosen. Arguing with it would have preserved the behaviour and
the weakness underneath, which the number never mentioned.

**"We verify the download" deserves one more question: verify against what, fetched
from where?** A checksum served from the host serving the artifact is a
transit-corruption check wearing a security check's clothes. Ours had been sitting
there looking reassuring for two releases.

**Removing a capability beats documenting it.** We could have written a paragraph
explaining that our `postinstall` was trustworthy. Every compromised package in
history could have written that paragraph. The install script is gone instead,
which is checkable by anyone in about four seconds:

```bash
npm view ai2rules-harness scripts
# undefined
```

That's a claim you can verify without believing anything we say, which is, roughly,
the whole point of the project the package belongs to.

---

*[`ai2rules-harness`](https://www.npmjs.com/package/ai2rules-harness) —
`npm install -g ai2rules-harness && harness init`. The packaging reasoning,
including the four alternatives we rejected, is in
[`DECISIONS.md`](https://github.com/sv-pro/ai2rules/blob/main/DECISIONS.md) D58.*
