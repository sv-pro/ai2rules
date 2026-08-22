# D72 — Discovery projection is a sibling wire ABI; the world owns existence and the host owns schema


**Context.** `harness gate` can return `ABSENT` for an invocation, but issue #55
requires DeepSeek Harness not to advertise ABSENT capabilities in the first place.
Loading `WorldManifest` YAML in the TypeScript plugin would duplicate compilation
and projection rules outside the kernel. Calling `gate` with invented empty
arguments for every schema would confuse discovery with an effect proposal (and
can classify commands, consult context, or charge an ALLOW). DeepSeek provides one
authoritative `system-prompt/assemble` waterfall, but its registry owns the actual
operational schemas.

**Decision.** Add `harness project --world W`, a host-neutral single-shot wire
operation. A host submits the exact schema array it is about to expose. The
operation compiles `W` through the same loader and root normalization as `gate`,
filters by `CompiledWorld::projected_actions()` and the declared source channel's
compiled capability matrix, preserves surviving host schemas, and returns ABSENT
names, the gate-compatible manifest identity, and SHA-256 of the exact visible
schema array. Missing/undeclared source context and duplicate or malformed names
are rejected.

The DeepSeek plugin invokes `project` after downstream prompt-assembly listeners
settle, so its returned assembly is authoritative. It re-runs on every assembly;
invocation separately recompiles the current world through `gate`. A stale schema
therefore never grants authority after revocation. Projector failure produces an
empty surface and the already-engaged invocation adapter still denies on failure
(D71).

The world owns **whether a capability exists**; the host registry owns **how that
capability is called**. The projector hashes the latter rather than parsing or
regenerating it. DeepSeek Code Mode's generated SDK is unstructured prompt text;
unless the reserved `run_code` transport is projected, the plugin removes the SDK
and transport instruction instead of pretending it safely filtered prose.

**Alternatives.**
- *Parse the manifest in TypeScript.* Rejected: a second compiler/projection engine.
- *Probe each tool through `gate`.* Rejected: fabricated calls are not discovery.
- *Copy world descriptor schemas over dsh schemas.* Rejected: the demo world does
  not own dsh's runtime contract, and silent schema skew would make visible calls
  invalid at the host boundary.
- *Cache projection until a file watcher fires.* Rejected: the next assembly is a
  cheap, exact revocation boundary and does not depend on watcher reliability.

**Consequence.** Discovery and invocation now share one compiled-world identity
without moving policy into the plugin. The ABI is reusable by any non-Rust host
with an authoritative schema seam; MCP's in-process projection remains free to
re-issue world-owned descriptors because that gateway's manifest does own them.
