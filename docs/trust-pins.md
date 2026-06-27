# Trust pins

*Operator attestations that a specific read source is trusted, pinned to its
content identity.* The principled alternative to deleting taint state or blanket-
trusting a directory. Decision: [`DECISIONS.md`](../DECISIONS.md) **D29** (extends
**D25**). Concept home in the thesis: [`THESIS.md`](THESIS.md) §3 (taint), §5
(provenance / descriptor drift).

## The problem

The harness taints a session when it reads an untrusted source (e.g. anything under
`repos/`), and the taint floor (invariant 7) then denies network egress
(`WebFetch`, `curl`, …) so injected text in that content can't exfiltrate. Correct —
but sometimes you, the operator, have *reviewed* a specific source and want to vouch
that those exact bytes are safe, without (a) blindly resetting taint or (b)
permanently trusting a whole tree including its future edits.

## The mechanism

A **trust pin** says: *"I attest this source is trusted, and here is its content
identity. Trust it only while the content still matches."*

- A `Read` whose path matches a pin **and** whose live content matches the pinned
  `identity` is classified **Trusted** → it does **not** taint.
- Any **drift** (the bytes or commit no longer match) or a passed `expires` date
  **revokes** the pin → the read taints as normal. This is the descriptor-drift
  primitive (`safe-mcp-proxy`) applied to read sources.
- A pin **never weakens the egress floor**. It re-classifies a *source* upstream of
  taint; an unpinned or drifted source still tainted → egress still `DENY`. In the
  manifest's channel model, a valid pin flips a read's `source_channel` from
  `workspace_files` (SemiTrusted, `taint:true`) to **Trusted (`taint:false`)**.

Taint is therefore a **ledger of causes**, recomputed every call:
`tainted == any recorded cause not covered by a valid pin`. The ledger keeps every
cause (audit); pinning a cause removes it from the *taint* computation, not from the
record.

### Why this doesn't violate monotonic taint (invariant 6)

It is not an "untaint." A pinned read was *never* an untrusted-taint cause, so
recomputing reflects **corrected provenance** — a human, design-time, auditable
attestation — not a reduction of taint under fixed facts. The operator changing a
source's trust at design time is exactly the legitimate lever; the model cannot do
it at runtime.

## Schema

`trust_pins` is an array on the WorldManifest. Live host form
(`.claude/cc-world.json`), canonical form will be the same field on the compiled
`WorldManifest` (`.claude/cc-world.yaml` / kernel — pending the D26 cutover).

```jsonc
"trust_pins": [
  {
    "path": "repos/3p/agent-governance-toolkit/README.md",  // substring or glob (** ok)
    "identity": { "kind": "sha256", "hash": "e99e83…3473" }, // file bytes
    "reason": "Reviewed for the repos/3p adoption analysis",
    "attested_by": "sergey",
    "expires": "2026-09-27"                                   // optional time-box
  },
  {
    "path": "repos/3p/MGP/**",
    "identity": { "kind": "git_commit",                       // a whole clean tree
                  "repo": "repos/3p/MGP", "commit": "<sub-repo HEAD sha>" }
  }
]
```

### `identity.kind`

| kind | binds to | granularity | revoked by |
|---|---|---|---|
| `sha256` | the file's exact bytes | one file | any byte change |
| `git_commit` | a reference repo's own `HEAD`, **clean tree** | whole repo | a pull / any local edit |

> **Bind to content identity, not the harness repo's `HEAD`.** `repos/3p` is not
> tracked in this repo (`AGENTS.md`: never `git add repos/`), so this repo's commit
> says nothing about that content. Use `sha256`, or the *reference repo's own*
> commit.

## Adding a pin

```bash
# sha256 of a reviewed file:
sha256sum repos/3p/<repo>/README.md
# or a whole reviewed reference repo at its current commit:
git -C repos/3p/<repo> rev-parse HEAD
git -C repos/3p/<repo> status --porcelain   # must be empty (clean tree)
```

Add the entry to `trust_pins`. Effect is immediate (the hook re-reads the manifest
each call); an already-tainted session whose *only* causes are now pinned recomputes
to untainted with no manual reset.

## Where it runs

- **Interim (shipped):** the live PreToolUse hook. Logic in
  `.claude/hooks/_gatelib.py` (shared by `world-gate.py` + `taint-notify.py` so both
  agree on "is this tainted?"); pins in `.claude/cc-world.json`. Fails **open**.
  Tested by `.claude/hooks/test-gate.sh` §4.
- **Canonical (pending):** a `trust_pins` field on the real `WorldManifest`, checked
  in the pure `gate()` (same code native + WASM, conformance-tested), landing with
  the D26 host cutover. This also closes D25's deferred read-taint: D25 tags an
  untrusted read with an untrusted channel; a pin is the exception that re-tags a
  vouched read Trusted.

## Limits

A pin is only as good as the operator's review of those bytes — it deliberately
moves trust from *the model's runtime judgement* to *a human's design-time
attestation*. `git_commit` pins are coarse (whole tree). The interim grows the
Python hook that D24 plans to retire; the canonical home is one `gate()` in the
kernel.
