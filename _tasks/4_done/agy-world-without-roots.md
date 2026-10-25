# Antigravity dogfood world has no path scope (AI2-82)

**Found 2026-10-25** while closing AI2-80 (D91), and recorded there as the same gap on
the other host. Linear [AI2-82](https://linear.app/ai2rules/issue/AI2-82).
Follows [`dogfood-world-without-d57.md`](../4_done/dogfood-world-without-d57.md).

## The finding

`.agents/agy-world.yaml` had no `roots` block. Measured with `harness agy-hook` on
`main` at `4e50aea`, each of the following passed with no decision:

- writes to `.agents/agy-world.yaml`, `.agents/hooks.json` and `.agents/hooks/world-gate.sh`;
- writes to Claude Code's `.claude/` files;
- `write_to_file /etc/x`;
- `view_file ~/.ssh/id_rsa`.

The gap also ran the other way. Under D91 `./.agents` is inside the project's
`ReadWrite` root, so Claude Code could silently write all three Antigravity
control-plane files.

## Development — 2026-10-25

**Decision (maintainer):** put the rule in the starter so that one `roots` block governs
all three worlds. Recorded as D92.

- Code:
  - `scripts/starter-world.yaml` gains `./.agents` → `Read`.
  - Both dogfood worlds carry the block verbatim.
  - `init --user/--managed` drops the rule, failing loudly if it is missing.
- Tests:
  - `committed_worlds.rs` pins both worlds to the starter.
  - `agy_hook.rs::the_dogfood_world_keeps_both_control_planes_read_only` covers
    Antigravity.
  - `cc_hook.rs` and `init.rs` now also cover the `.agents/` files.
  - `init_machine.rs` asserts that the machine world drops the rule.
  - The new tests fail against the old worlds.
  - Two `agy_hook` tests moved off a made-up `/workspace` root onto a real temp
    workspace, following D91's Windows lesson.
- Validation:
  - `cargo test --workspace`: 595 passed.
  - fmt, clippy `-D warnings` and `check-demos.sh` are clean.

## Done — 2026-10-25

PR #157 was merged into `main` as `6cd805d` (commit `f082706`, decision D92). CI was
green on the merged head: all 15 checks passed on the first run, including
`check-windows`. Moving the two `agy_hook` tests off the made-up `/workspace` root
headed off the Windows failure that D91 hit. Linear's GitHub integration moved AI2-82 to
Done on merge.

Carried forward:

- Shell writes to `.agents/` and `.claude/` remain ASK, which is the D57 residual.
- Machine-scope worlds keep the starter's three-line comment above the dropped
  `./.agents` rule. It is cosmetic.
