# Security policy

This project is a governance tool: it decides what an AI coding agent is allowed
to do on your machine. That makes both halves of this document load-bearing — how
to report a weakness, and what the tool does **not** claim to protect you from.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting: the repository's **Security** tab →
**Report a vulnerability**. That opens a private thread visible only to the
maintainers.

If private reporting is unavailable to you, open a normal issue saying only that
you have a security report and asking for a private channel — **no details in the
public issue**.

Useful in a report, roughly in order of usefulness:

- the manifest (world) you were running, or the smallest one that reproduces
- the host (Claude Code, Antigravity, OpenCode, the `harness gate` CLI, the MCP
  gateway) and how the harness was wired in
- the exact tool call or payload, and the verdict you got versus the one you
  expected
- whether the harness was running additive (default) or `--grant` (replace) mode

You will get an acknowledgement within a few days. This is a small project without
a staffed on-call rotation; what is promised is a real answer, not a fast one.

## Supported versions

The latest published release is supported. Fixes are made on `main` and shipped in
the next release rather than backported.

| Version | Supported |
|---|---|
| 0.2.x | ✅ |
| < 0.2 | ❌ |

## What is in scope

Anything that makes the kernel's verdict wrong, unenforced, or bypassable:

- a call that should be `DENY`/`ASK` and comes back `ALLOW`
- a taint escalation that is lost, skipped, or reversed — taint is monotonic and
  must stay so
- two hosts reaching **different** verdicts for the same manifest and the same
  call, which breaks the one-kernel guarantee
- path scope (`roots`) failing to bind: a `Deny` root reachable through symlinks,
  `..`, or path spellings the adapters resolve differently
- approval bypass — a request-supplied token being honoured, an `ASK` satisfied
  without a human, an `ASK` silently collapsing to `ALLOW` in background mode
- anything in the install or release path: the npm packages, the release workflow,
  the published binaries
- the control plane: making the harness modify, disable, or point away from the
  policy that governs it

## What is out of scope, and why

These are known properties of the design, not undiscovered bugs. Reporting them is
not wasted effort — arguments that any of these *should* change are welcome as
issues — but they will not be treated as vulnerabilities.

**The harness is advisory, not a sandbox.** It is a `PreToolUse` hook. It decides
before the host acts, and the host is what actually acts. It does not confine a
process that is already running, and nothing here stops a tool that never asks. If
you need containment, you need containment — see `docker/` for the isolation
layer. A verdict is not a jail.

**A hook cannot remove tools from the host's surface.** `ABSENT` means "this action
is not in your world", but a `PreToolUse` hook cannot un-offer a native tool to the
model. Enforcing `ABSENT` as a denial is opt-in (`--enforce-absent`) because
denying every tool outside the manifest bricks most hosts.

**Process failures fail open, deliberately.** An unreadable event or an
uncompilable manifest exits quietly and lets the session continue, because a
governance tool that bricks your editor on a bad day gets uninstalled. Note the
boundary: that covers failures to *reach* a decision. A failure to *record* a
decision's consequence fails closed instead (D59) — if those two look like the same
category to you, that distinction is exactly the bug class we care about.

**Command classification is a heuristic over shell strings.** `command_classes`
matches patterns against a command line. `"curl" http://x`, `curl$IFS'...'`, and
`echo <base64> | base64 -d | sh` all evade the specific patterns — by design they
fall through to the classifier's `default_to` bucket, which every shipped manifest
sets to a fail-closed, approval-required class. **An evasion that lands in the
fail-closed bucket is expected behaviour. An evasion that reaches a more permissive
class than the command deserves is a vulnerability** — report that one.

**A manifest can be written insecurely.** The kernel enforces what the manifest
says. A manifest that omits `default_to`, orders a permissive class ahead of a
restrictive one, or grants `ReadWrite` at `/` will do what it says. Cases where the
*language* invites a mistake are tracked as findings and are worth reporting as
design issues.

**Decision and execution are separate moments.** Paths are resolved and judged at
gate time; the host executes afterwards. A filesystem racing between those two
points is real, and outside what an advisory hook can close.

**The pure gate ABI trusts its caller.** `harness gate` takes a resolved path and a
taint value from whoever invokes it, and cannot verify either — that is what makes
it pure and testable. Its callers (the host adapters) are in scope; feeding the ABI
false context yourself is not.

## Known gaps, stated rather than discovered

Live limitations we would rather write down than have someone find:

- **`source_channel` is pinned to `user_prompt` on live hosts.** The channel-trust
  machinery exists and works, but both shipped adapters declare the most-trusted
  channel for every call, including one the model proposed right after reading a
  poisoned file. Data-flow taint still applies; channel trust currently does not.
  Tracked as finding #21.
- **Argument schemas and cross-host portability are mutually exclusive.** Adapters
  alias host argument keys by adding them, and schema validation rejects undeclared
  properties, so a schema-bearing action fails on some hosts. Tracked as #27.

The full list, open and fixed, is in [`docs/reviews/`](docs/reviews/); the
reasoning behind each design decision is in
[`DECISIONS.md`](DECISIONS.md).

## Disclosure

Findings get a number, a fix, and a written record — including the ones we find
ourselves and the ones that are embarrassing. Reporters are credited unless they
ask not to be. If a report turns out to be a design property rather than a defect,
it still tends to become documentation, because the fact that it needed explaining
is itself the finding.
