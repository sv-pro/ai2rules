# Discovery: the repo's own dogfood world does not hold D57

**Found 2026-10-25, empirically**, in the same probe pass as
[`network-word-downgrades-ask.md`](../4_done/network-word-downgrades-ask.md). Linear [AI2-80](https://linear.app/ai2rules/issue/AI2-80).

## The finding

D57 records a measured vulnerability: `Write`/`Edit` against `.claude/cc-world.yaml`,
`.claude/hooks/world-gate.sh` and `.claude/settings.json` returned *silent ALLOW*. The
fix was one `roots` rule in the **shipped starter manifest**:
`- { path: "./.claude", access: Read }`. `harness init --user` protects the same files
at machine scope (`docs/machine-wide-governance.md`).

The world that governs Claude Code in this repository, `.claude/cc-world.yaml`, has no
`roots` block, so none of that applies here. Probed with `harness cc-hook` built from
`main` at `bfbae35`, fresh state:

| call | clean session | after one `WebFetch` |
|---|---|---|
| `Edit .claude/settings.json` (rewrite the hook matcher) | ALLOW (silent) | ALLOW (silent) |
| `Write .claude/cc-world.yaml` | ALLOW (silent) | ALLOW (silent) |

These calls stay silent even after a web read. Path classes (D83) cover `CLAUDE.md`,
`AGENTS.md` and `.claude/{agents,commands,rules,skills}/**`. They do not cover the
manifest, the shim or the hook registration, which are the files that can switch
governance off.

## Why it is probably not simply "add roots"

D61 explains why path scope lives in a separate world pair: enabling `roots` changes the
verdict of every pathless file action. Adding `roots` to the dogfood world is therefore
a behaviour change for every session in this repository, not a one-line patch. A
narrower option is a `path_classes` entry that reclassifies writes to
`.claude/cc-world.yaml`, `.claude/settings*.json` and `.claude/hooks/**` to an
approval-required or Memory-class action. That would carry D57's intent without D61's
blast radius.

## Open question for the maintainer

Is the dogfood world meant to demonstrate the starter's guarantees, or is it a
deliberately looser developer profile? If it is the looser profile, say so in the
manifest header and in `AGENTS.md`. The `.claude/` section currently describes it as the
manifest "that governs this session", which reads as the stronger claim.

## Development — 2026-10-25 (AI2-80)

**Decision (maintainer):** the dogfood world must demonstrate the starter's guarantees.
That answers the open question above.

**Fix:** D91. `.claude/cc-world.yaml` carries the starter's `roots` block verbatim. Two
new tests cover it:

- `the_dogfood_world_holds_the_starter_roots` in `crates/compiler/tests/committed_worlds.rs`
  pins the dogfood `roots` block equal to the starter's.
- `the_dogfood_world_keeps_its_own_control_plane_read_only` in
  `crates/cli-harness/tests/cc_hook.rs` checks the manifest, the shim and `settings.json`:
  writes are denied, clean and tainted; reading the manifest and editing `README.md`
  pass through.

Both tests fail against the old world.

This does not use the narrower `path_classes` option from the section above. That would
be a second mechanism for the same guarantee, covering fewer files.

**Measured effect** (same probes before and after):

- Project work, `Glob` and `Grep`, `/tmp`, Bash and `WebFetch` are unchanged.
- Writes under `.claude/` and `/etc` are DENY.
- Reads of credential directories are DENY.
- Reads outside every root are ASK.

**Validation:** `cargo test --workspace` passed 594 tests; fmt, clippy `-D warnings` and
`check-demos.sh` are clean.

**Left open:** the Antigravity dogfood world, `.agents/agy-world.yaml`, has the same gap.
