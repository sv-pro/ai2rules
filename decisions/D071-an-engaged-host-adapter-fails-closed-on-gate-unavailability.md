# D71 — An *engaged* host adapter fails closed on gate unavailability


**Context.** The host-neutral Gate ABI (D24) leaves fail-open vs fail-closed to the
adapter: "The adapter decides fail-open vs fail-closed on `≠0`." The two shipped
adapters — the Claude Code `cc-hook` bootstrap shim and the OpenCode
`tool.execute.before` plugin — both fail **open**: no binary / process error →
allow, so a broken gate never bricks a session. That is the right default for a
*bootstrap shim*: a shim that cannot even locate the kernel never governed the
session, and bricking every tool call over a missing binary is worse than the
ungoverned state that already existed.

But the #54 governor-integrity benchmark review, and issue #55 explicitly, drew
the line the blanket "log + allow" blurred: *"governed effects must not silently
fail open."* Once an adapter has **engaged** the gate — the plugin is mounted and
handling a call — a subsequent gate unavailability (missing binary, exit 1/2,
unparseable stdout, unmappable verdict, unpersistable taint/usage sidecar) is no
longer "never governed." Allowing the effect there silently drops governance that
the deployment asked for.

**Decision.** The DeepSeek Harness `tools/pre-execute` adapter (issue #55,
`docs/demos/deepseek-harness/ai2rules-gate.ts`) **fails closed**: every path that
cannot produce a trustworthy `ALLOW` returns `{kind:'deny'}`. The only sanctioned
bypass is the explicit operator switch `AI2RULES_DISABLE=1` ("do not govern this
session at all" — the *not-activated* case, structurally distinct from an
engaged-but-broken gate). This is tested against the real dsh registry + the real
`harness gate` binary (`ai2rules-gate.spec.ts`: `FAIL-CLOSED` binary-missing and
exit-2 cases, plus the `BYPASS` case).

**Alternatives.**
- *Keep fail-open everywhere* (the shim default). Rejected for an engaged adapter:
  it is the exact silent-fail-open #55 forbids, and it makes gate availability an
  unmonitored dependency of the security property.
- *Fail-open with a loud warning.* Rejected: a warning is not a control; the effect
  still happens.
- *Make the shim fail closed too.* Deliberately **not** done. The distinction is
  the point: a bootstrap shim that cannot locate the kernel was never engaged, and
  D37 keeps that layer fail-open on purpose (a missing hook file must not brick a
  session). The fail-closed rule attaches to the *engaged* adapter, not the shim.

**Consequence.** Adapters now come in two postures by role, and the ABI doc's
"adapter decides" is refined to: *bootstrap shim → fail-open (not-yet-engaged);
engaged adapter → fail-closed (governed effects)*. The `cc-hook`/OpenCode
fail-open remains correct for their shim role; a future engaged in-process adapter
should follow the fail-closed rule established here.
