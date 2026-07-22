# Task: World Manifest modifications mechanism — from ASK decisions to policy

**Owner:** design (open). **Status:** design todo — raised while planning the live Tier-1
grant-mode dogfood (`dogfood/live-grant-mode`). Not started.

## Why

Running the live instance under `harness cc-hook --grant` (Tier 1) makes the manifest the
allowlist: known-good actions are granted (no prompt), everything else falls through to a
normal **ASK**. Every ASK is a signal — "the manifest didn't cover this; a human decided."
Those decisions are exactly the raw material for *evolving* the manifest. Today they evaporate.

**Goal:** capture ASK decisions, then design what becomes of them — turning a stream of
human yes/no calls into manifest改 (WorldManifest modifications) without hand-editing YAML.

## Part 1 — capture (the concrete first step)

`cc-hook` has **no audit/log flag today** (confirmed: only `--grant` / `--enforce-absent`). So
step one is logging: record each decision (ALLOW/DENY/ASK/ABSENT) with tool, args, effective
action, taint, session, timestamp, manifest hash → append-only JSONL (mirror `mcp-gateway
--audit`).

**Subtlety — capturing the human's *answer* is non-trivial.** PreToolUse fires *before* the
prompt; the hook emits `ask` and never learns what the human chose. The choice must be
inferred: a **PostToolUse** hook observes whether the tool actually ran (ran ⇒ human allowed;
absent ⇒ denied), correlated back to the PreToolUse ASK by tool+args+session. So the capture
mechanism is a Pre/Post pair, not a single hook. Design this correlation explicitly.

## Part 2 — what to do with the log (the open discussion)

The subject of debate — each axis is a real decision, not yet made:

- **Timing: apply immediately vs. lay over.** Auto-amend the manifest the moment a human
  allows something (fast, but the manifest mutates under you), or *stage* proposed edits for
  batch human review (safe, but a queue to tend)? Likely a knob, default = lay over.
- **Scope: session / project / user.** A one-off "yes" for this session ≠ a standing project
  rule ≠ a user-global preference. Where does a captured decision land, and who promotes it up
  a level? (Echoes the taint/trust scoping; and D-lineage of trust pins.)
- **Form: individual rules vs. integral policy.** Append each decision as its own narrow rule
  (simple, but the manifest bloats into the same unreadable pile we're fleeing — see the
  "permission list" post), or *compile/optimize* accumulated decisions into a coherent policy
  (dedupe, generalize `Bash(npm run test:foo)` + `…:bar` → a class)? The optimization is the
  hard, valuable part — and it's where an LLM could help at **design time** (propose a
  generalization; a human reviews and the compiler freezes it — thesis-aligned: LLM proposes,
  deterministic kernel disposes).

## The tension to hold

This is governance-of-the-governance. The whole project's point is that the manifest is a
*reviewable, deterministic* artifact. An auto-growing manifest that mutates from runtime
decisions risks becoming exactly the ungoverned sprawl we criticize. So: **capture is cheap
and safe; application must stay design-time and human-reviewed.** Whatever the mechanism, the
border holds — runtime *proposes* manifest changes, a human at design time *disposes*.

## Acceptance (Part 1 only, for now)

- `cc-hook` logs every decision to an append-only JSONL (opt-in flag), and a PostToolUse
  companion records the observed outcome so ASK→(allowed/denied) is recoverable.
- Part 2 stays a written design doc + a decision (D-entry) before any auto-modification ships.

## Related

- The live dogfood that motivates it: `dogfood/live-grant-mode` (Tier-1 grant on the live
  instance). - `docs/demos/replace-permissions/` (grant mode). - the "permission list" +
  "skipped permissions" blog posts. - trust pins (D29) for the scope axis.
