# D57 — The control plane is read-only to the agent; policy changes are *proposed*, and merged only by deterministic non-agent tools


**Date:** 2026-08-11. **Forced by a working exploit**, reported from real use and reproduced
here. Constrained by THESIS §2 (no stochastic component in the trust path), D37 (the governed
project is untrusted input), and E11 (the World Authoring Tool, which this entry gives a second
job).

- **The vulnerability.** `.claude/` sat inside the project's own writable root, so an agent
  under governance could rewrite the policy binding it. Measured, not inferred:
  `Write`/`Edit` against `.claude/cc-world.yaml`, `.claude/hooks/world-gate.sh` and
  `.claude/settings.json` all returned **silent ALLOW — not even ASK**. Flipping one roots rule
  from `Read` to `ReadWrite` moved a `/etc` write from `DENY` to `ALLOWED` **on the very next
  tool call.**
- **Why `CompiledWorld` immutability did not save us, which was the assumption worth killing.**
  The invariant is real but scoped to a single decision. The shim runs
  `harness cc-hook --world .claude/cc-world.yaml` as a **fresh process per tool call**, so the
  manifest is recompiled every time. There is no "takes effect next session" — there is no
  session. An earlier note in `NEXT.md` speculated immutability might blunt this; it does not,
  and that speculation is retracted here.
- **Decision, part 1 — the control plane is `Read`.** The shipped starter manifest gains one
  rule: `- { path: "./.claude", access: Read }`, in the same "out of bounds even inside an
  allowed parent" block as `~/.ssh` and `/etc/shadow`. Structured writes to the manifest, the
  shim and the hook registration now **DENY**.
- **Read stays allowed, deliberately.** An agent that cannot see its own rules cannot explain a
  denial or propose a sensible change to one, and opacity buys nothing here — the manifest is
  committed to the repo anyway. Secrecy is not the property being defended; **write authority
  is.**
- **Decision, part 2 — the sanctioned path is a *proposal*, not an edit.** Removing write access
  must not remove the useful workflow ("loosen this rule so I can finish"). The agent may emit a
  **patch** against the manifest; that patch is inert until merged by a **deterministic,
  non-agent, user-facing tool** — the World Authoring Tool (E11, `harness serve`) being the
  obvious host, since it already compiles manifests with the real compiler and already puts a
  human in front of the result.
  **This is the project's own propose/decide split applied one level up.** A `ToolCall` is a
  proposal until `IRBuilder::build` seals it; a manifest patch is a proposal until a human-driven
  deterministic tool merges it. The model's authority over policy becomes exactly what its
  authority over execution already is: **it may ask, and nothing it writes takes effect by
  itself.**
  Proposals live **outside `.claude/`** (the whole directory is read-only, and a carve-out is a
  path-traversal target — this repo has fixed that class twice, #36 and #37). Implementation is
  deliberately not specified here beyond that constraint.
- **Known residual, stated because it is the obvious next question: the shell path is `ASK`, not
  `DENY`.** Path scoping keys on the `file_path` argument of structured tools. `sed -i`,
  `echo >`, `cat >` and `rm` against the same files are classified as commands and land on
  approval-required. A human is in the loop, which is a categorical improvement on silent allow,
  but it is not a block. Closing it properly means the kernel understanding write targets inside
  shell commands — which is a real piece of work and belongs to its own entry, not to a
  parenthesis in this one.
- **Alternatives rejected.**
  - *Hash-pin the manifest in the shim.* Makes tampering loud rather than impossible, and adds a
    second source of truth that drifts the moment someone legitimately edits the manifest.
  - *Move the manifest out of the project.* Closes it, and forfeits the property that makes a
    manifest worth having: policy committed alongside the code it governs, reviewed in the same
    pull request.
  - *Deny reads too.* Rejected above — costs the proposal workflow and buys no secrecy.
  - *A privileged MCP tool that edits the manifest.* Rejected as the primary mechanism. It is
    strictly worse than the file write it replaces: advertised in `tools/list`, therefore the
    first thing an injection enumerating tools will find, and certain to be allow-listed for
    convenience. If one is ever built it needs all four of — direct writes closed, edits applying
    only on the next compile, manifest edits denied under taint as a **kernel** invariant (a
    manifest rule protecting the manifest is circular), and a human-minted approval token
    (`ApprovalStore`, E6) the model cannot forge.
- **Related:** D37 (untrusted project directory), D47, E6 (`ApprovalStore`), E11 (the authoring
  tool that gains the merge role), #27/#28 (`roots`), #36/#37 (path canonicalization — the class
  a `.claude/` carve-out would reopen), `scripts/starter-world.yaml`, and
  `crates/cli-harness/tests/init.rs` (three tests: writes denied, reads allowed, ordinary project
  writes unaffected).
