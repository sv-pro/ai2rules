# D48 — Antigravity CLI (`agy`) is the third live host; adapters share a `hostkit`


**Date:** 2026-07-26. **Executes `NEXT.md` P1** ("port the harness to a second live host"),
choosing Antigravity over the Codex target P1 sketched. Extends D24/D34/D36/D37.

- **Decision:** `agy` is governed by the real Rust kernel through `harness agy-hook`, a
  PreToolUse adapter sibling to `cc-hook`, wired via `.agents/hooks.json` →
  `.agents/hooks/world-gate.sh` against `.agents/agy-world.yaml`. No new governance logic:
  the adapter translates shape, `gate()` decides, `host_outcome()` maps the verdict. The
  Antigravity entry point joins the `one_kernel.rs` conformance suite, fed the host's real
  payload shape so the translation is inside the parity claim.
- **The contract was reverse-engineered, then verified.** Antigravity's hook ABI is not
  vendor-published; it was extracted from the shipped binary and confirmed against a live
  session: `.agents/hooks.json` discovery, the camelCase/`toolCall` payload, `deny` actually
  blocking (the agent visibly replanned around a denied `run_command`, and the `reason`
  reached the model), and `{}` as the no-decision passthrough. **Recorded as a risk:** a
  future `agy` release can move this contract; `tests/agy_hook.rs` is the regression net.
- **Argument aliasing lives in the adapter, not the world** (the load-bearing call).
  Antigravity spells tool arguments in PascalCase (`CommandLine`, `AbsolutePath`,
  `TargetFile`); D36 `command_classes` classifies the neutral `command`. The adapter aliases
  host keys → neutral vocabulary, additively (originals preserved), before gating.
  *Alternative rejected:* give `agy-world.yaml` its own `command_classes` with
  `arg: CommandLine` — that forks D36 world data per host, which is the exact drift D36
  exists to prevent, and would have broken the byte-identical pattern-list guarantee.
  The failure mode this protects against is silent: an alias that stops firing does not
  error, it drops every shell command into the fail-closed `unclassified` branch (D44), so
  the suite pins it with a same-command aliased/unaliased pair.
- **ASK maps to `force_ask`, not `ask`.** Antigravity's `ask` respects cached "Always Allow"
  grants; a kernel ASK means a human must decide *this time*. Defaulting to the cache-
  respecting channel would let a past click satisfy a present approval requirement.
  `--soft-ask` is the explicit, greppable opt-out.
- **Fail-open prints `{}`.** Per-adapter fail-open strategy is documented, not uniform:
  cc-hook's fail-open is silence, but Antigravity parses stdout, so the no-op must be an
  actual JSON object carrying no `decision`. A process failure is still never an outcome.
- **Shared `hostkit` prevents copy #2.** `sanitize` / `resolve_action_path` /
  `canonicalize_*` / `normalize_tool` moved from `cc_hook.rs` to
  `cli-harness/src/hostkit.rs`, used by both Rust adapters. `docs/one-kernel-many-hosts.md`
  keeps a duplication survey whose rows are literally "copy #2 / copy #3"; a second adapter
  pasting these — **including the D46 symlink canonicalization** — would have been the next
  row, and the security-relevant half would have drifted silently.
- **Alternatives rejected:** port Codex first (P1's target — same seam, still open, but agy
  was the host actually installed and exercisable here); document agy via `AGENTS.md` only,
  as before (legible ≠ governed — that is the whole distinction this port closes); wire
  through the `harness gate` wire ABI like the OpenCode plugin (D34 already settled that a
  Rust host links `gate()` in-process; a subprocess per tool call buys nothing here).
- **Related:** D24, D34, D35, D36, D37, D44 (unclassified fails closed), D46 (shim binary
  resolution + path canonicalization), `docs/one-kernel-many-hosts.md`,
  `docs/demos/antigravity/README.md`, `NEXT.md` P1.
