#!/usr/bin/env bash
# Bootstrap shim (DECISIONS D37): exec the Rust kernel's PreToolUse adapter.
#
# No governance logic lives here — the kernel (`harness cc-hook`) decides against
# .claude/cc-world.yaml. This is the canonical PreToolUse wiring (settings.json);
# world-gate.py is the same shim kept for sessions whose hook config snapshotted
# the old path. Fail-open: no binary → exit 0 (stdin passes through on exec).
set -u
PD="${CLAUDE_PROJECT_DIR:-$(pwd)}"
# Rollback layer #1 — instant kill-switch, no restart. `touch .claude/gate-off`
# (project) or `touch ~/.claude/gate-off` (panic, everywhere) disables governance on
# the NEXT tool call; `rm` re-enables. The shim runs per-call, so the toggle is immediate.
if [ -f "$PD/.claude/gate-off" ] || [ -f "$HOME/.claude/gate-off" ]; then exit 0; fi
BIN="${HARNESS_BIN:-}"
if [ -z "$BIN" ] || [ ! -x "$BIN" ]; then
  BIN=""
  for c in "$PD/target/release/harness" "$PD/target/debug/harness"; do
    if [ -x "$c" ]; then BIN="$c"; break; fi
  done
fi
if [ -z "$BIN" ]; then
  BIN="$(command -v harness 2>/dev/null || true)"
fi
if [ -z "$BIN" ]; then
  exit 0 # fail-open: no kernel binary, fall through to the host's own flow
fi
# Tier-1 dogfood: --grant makes the manifest the allowlist (ALLOW -> explicit `allow`,
# no prompt). Deliberately NO --enforce-absent: undeclared tools fall through to the
# normal prompt, so a manifest gap is "still asks", never a lockout.
exec "$BIN" cc-hook --grant --world "$PD/.claude/cc-world.yaml" --state "$PD/.claude/state"
