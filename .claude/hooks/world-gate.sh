#!/usr/bin/env bash
# Bootstrap shim (DECISIONS D37): exec the Rust kernel's PreToolUse adapter.
#
# No governance logic lives here — the kernel (`harness cc-hook`) decides against
# .claude/cc-world.yaml. This is the canonical PreToolUse wiring (settings.json);
# world-gate.py is the same shim kept for sessions whose hook config snapshotted
# the old path. Fail-open: no binary → exit 0 (stdin passes through on exec).
#
# The project directory is untrusted input. Never resolve harness from
# $CLAUDE_PROJECT_DIR/target; use an explicit absolute HARNESS_BIN/AI2RULES_HARNESS
# override, or a standard installer-owned absolute path.
set -u
PD="${CLAUDE_PROJECT_DIR:-$(pwd)}"
BIN="${HARNESS_BIN:-${AI2RULES_HARNESS:-}}"
if [ -n "$BIN" ]; then
  case "$BIN" in /*) [ -x "$BIN" ] || exit 0 ;; *) exit 0 ;; esac
else
  BIN=""
  for c in "$HOME/.local/bin/harness" "/usr/local/bin/harness" "/opt/ai2rules/bin/harness"; do
    if [ -x "$c" ]; then BIN="$c"; break; fi
  done
fi
if [ -z "$BIN" ]; then
  exit 0 # fail-open: no kernel binary, fall through to the host's own flow
fi
exec "$BIN" cc-hook --world "$PD/.claude/cc-world.yaml" --state "$PD/.claude/state"
