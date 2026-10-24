#!/usr/bin/env bash
# Antigravity CLI (`agy`) PreToolUse bootstrap shim (DECISIONS D48; D37 pattern).
#
# No governance logic lives here — the kernel (`harness agy-hook`) decides against
# .agents/agy-world.yaml. This is the canonical PreToolUse wiring for Antigravity
# (.agents/hooks.json). Fail-open: no binary → emit the no-decision no-op `{}` and
# exit 0, so the host continues with its own permission flow.
#
# Unlike Claude Code's shim, the fail-open path must still PRINT `{}`: Antigravity
# expects a JSON object on stdout, and a response carrying no `decision` is its
# documented no-op. Silence is not a passthrough here.
#
# Antigravity runs hook commands with the working directory set to the directory
# containing hooks.json (i.e. `.agents/`), and provides no $CLAUDE_PROJECT_DIR
# equivalent — so the project root is derived from this script's own location
# rather than from the environment or the cwd.
#
# The project directory is untrusted input. Never resolve harness from a
# project-local path such as $PD/target; use an explicit absolute
# HARNESS_BIN/AI2RULES_HARNESS override, or a standard installer-owned absolute
# path (D46).
set -u

noop() { printf '{}\n'; exit 0; }

# .agents/hooks/world-gate.sh → project root is two levels up.
SD="$(cd "$(dirname "$0")" 2>/dev/null && pwd)" || noop
PD="${AI2RULES_PROJECT_DIR:-$(cd "$SD/../.." 2>/dev/null && pwd)}" || noop
[ -n "$PD" ] || noop

BIN="${HARNESS_BIN:-${AI2RULES_HARNESS:-}}"
if [ -n "$BIN" ]; then
  case "$BIN" in /*) [ -x "$BIN" ] || noop ;; *) noop ;; esac
else
  BIN=""
  for c in "$HOME/.local/bin/harness" "/usr/local/bin/harness" "/opt/ai2rules/bin/harness"; do
    if [ -x "$c" ]; then BIN="$c"; break; fi
  done
fi
if [ -z "$BIN" ]; then
  noop # fail-open: no kernel binary, fall through to the host's own flow
fi

WORLD="$PD/.agents/agy-world.yaml"
[ -f "$WORLD" ] || noop

exec "$BIN" agy-hook --world "$WORLD" --state "$PD/.agents/state"
