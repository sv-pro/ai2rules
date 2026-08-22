# D49 — The gate governs proposals; MRTR means results can make demands


**Date:** 2026-08-01. Prompted by MCP protocol revision **2026-07-28**. Extends D24 (gate
ABI) and D33 (mcp-gateway). **Design debt, not a live hole:** nothing in the shipped code
can receive an `InputRequiredResult` today — there is no 2026-07-28 server anywhere in the
loop. This entry records the gap and the shape of the answer while the reading is fresh;
implementation waits for a real modern upstream.

- **What changed in the protocol.** 2026-07-28 makes MCP stateless and, in doing so,
  **inverts the direction of untrusted influence**. Server-initiated requests are gone as a
  mechanism ("this is a breaking change"): `roots/list`, `sampling/createMessage` and
  `elicitation/create` are no longer requests a server sends, they are *fields inside a
  result the client already asked for*. A server answers `tools/call` with an
  `InputRequiredResult` (`resultType: "input_required"`) carrying an `inputRequests` map,
  and per the client requirements the client **MUST** construct those inputs before
  retrying. See [Multi Round-Trip Requests](https://modelcontextprotocol.io/specification/2026-07-28/basic/patterns/mrtr).
- **Why that lands on us specifically.** The unit of governance in this codebase is the
  *proposed call*: `gate()` decides on `tools/call` before it leaves. Under MRTR a call the
  kernel **ALLOWed** can come back carrying a server-authored demand that the host collect
  user input or run an LLM completion — untrusted content steering host behavior *after* the
  verdict, through a channel that has no verdict shape at all. `harness mcp-gateway`
  currently returns the upstream result verbatim on ALLOW (`Ok(r) => r`). Stated plainly:
  **the kernel has no concept of "the result asked for something."**
- **Decision: govern the result, not only the call.** The gate ABI grows a second decision
  point — a post-call verdict on the *response* — rather than special-casing MRTR inside
  each host adapter. Three rules, all of which fall out of primitives that already exist:
  1. **An `InputRequiredResult` taints the retry.** It arrives on `mcp_output`, already
     `Untrusted` + `Tainted` by default, and the retry carries that floor forward. Monotonic
     taint already says this; nothing new is needed but the plumbing.
  2. **`inputRequests` are checked against the ontology.** An `elicitation/create` asking for
     a credential is `ABSENT` if the world never declared a credential-elicitation action.
     This is closed-ontology applied to a demand instead of a call — the same move, one
     direction later.
  3. **`requestState` can be bounded but never inspected.** The client **MUST** echo it back
     and **MUST NOT** parse it. So it is, by mandate, a server-controlled channel crossing
     the governed boundary opaquely. The kernel cannot reason about its contents; it can bind
     it to a session and a manifest hash, refuse it across worlds or sessions, and treat its
     presence as taint-carrying. **Recorded as a residual risk, not a solved one.**
- **Interim default until the above ships: deny any `input_required` result.** Fail-closed
  and honest about the gap. It is the default, not the design — see the rejected alternative
  below. **Implemented 2026-08-01** (issue #40): `harness mcp-gateway` refuses to relay a result
  carrying `resultType: "input_required"` — or a bare `inputRequests`, so a server that
  half-implements the shape is still caught — audits it with `stage: "result"`, and labels it a
  *gateway* interim deny rather than a kernel verdict, because that is what it is. **Note the
  bound:** the upstream call already happened, so this protects the host from the demand, not the
  upstream from the call. Closing that second gap needs the post-call verdict above.
- **Where the spec moved toward the thesis** (evidence, not comfort): the security section
  still concedes that *"MCP itself cannot enforce these security principles at the protocol
  level"* and hands consent and authorization to the host — the thesis premise in the
  protocol's own words. It now also says tool annotations *"should be considered untrusted,
  unless obtained from a trusted server"*, which is the trust-pins feature D29 consciously
  parked; **that parking deserves revisiting on its own terms.** Statelessness pushes
  cross-call state into *tool arguments* as server-minted handles, which makes argument
  sealing (invariant 12, D45) more load-bearing, not less. And MCP `roots` is deprecated with
  a suggested migration to configuration — where ours already live (see `docs/GLOSSARY.md`,
  which now records the name collision).
- **Alternatives rejected:**
  - *Handle MRTR in the gateway adapter only.* It is the only host that speaks MCP today, so
    this looks cheapest — but it puts a taint rule and an ontology check into an adapter,
    which is the exact policy-in-the-adapter drift D34/D36/D48 keep out. If the answer is a
    verdict, it belongs in the kernel.
  - *Deny `input_required` permanently.* Adopted as the interim default above; rejected as
    the permanent answer because it forecloses elicitation entirely, and elicitation is the
    one MRTR use that is genuinely user-serving (a server asking a human for a value it
    needs).
  - *Track the spec now — implement `server/discover`, `resultType`, `ttlMs`/`cacheScope`,
    dual-era support.* Rejected for cost against zero present value: the spec's own
    backward-compatibility rules make an honest legacy server legal, and claiming a protocol
    version you have not implemented is worse than an old pin. The gateway keeps declaring
    `2024-11-05` until a real 2026-07-28 upstream exists to front.
- **Consequence for the thesis.** The thesis is written against a request-shaped protocol.
  2026-07-28 makes MCP result-shaped as well, and the border has to be drawn on both
  directions of the wire. One sentence, for the record: **the gate governs proposals; results
  can now make demands, and nothing in the current design governs a demand.**
- **Related:** D24 (gate ABI), D29 (trust pins, parked), D33 (mcp-gateway), D34, D45
  (declared arguments only), D48; `docs/harness-gate-abi.md`, `docs/GLOSSARY.md`,
  commit `cbd9541` (the transparent-proxy fix this reading produced).
