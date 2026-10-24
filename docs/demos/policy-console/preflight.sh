#!/usr/bin/env bash
# preflight.sh - check the policy-console stand right before the demo, after
# setup.sh and with the receiver and the console running. Read-only: it changes
# nothing, and talks only to 127.0.0.1.
#
#   bash preflight.sh [DEST]      DEST defaults to ~/northwind
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEST="${1:-$HOME/northwind}"
DESK="$DEST/vendor-desk"
PORT="${PORT:-8787}"
CONSOLE_PORT="${CONSOLE_PORT:-8790}"
NAMES=(portal.northwind.test files.northwind.test)
fails=0

ok()   { printf '  ok    %s\n' "$1"; }
fail() { printf '  FAIL  %s\n' "$1"; fails=$((fails + 1)); }

echo
if HARNESS="$(command -v harness)"; then
  if "$HARNESS" console --help >/dev/null 2>&1; then
    ok "harness: $HARNESS ($("$HARNESS" --version 2>/dev/null | head -1))"
  else
    fail "harness at $HARNESS has no 'console' command: install a newer one"
  fi
else
  fail "no 'harness' on PATH: npm install -g ai2rules-harness@latest"
fi

SHIM="$DESK/.claude/hooks/world-gate.sh"
if [ -f "$SHIM" ] && grep -q -- '--enforce-absent --log' "$SHIM"; then
  ok "project: $DESK (hook with ABSENT enforced and the decision log on)"
else
  fail "project missing or its hook lacks --enforce-absent/--log: run setup.sh"
fi
if [ -f "$HOME/.claude/gate-off" ]; then
  fail "the kill switch is on (~/.claude/gate-off): the hook is not running"
fi

for name in "${NAMES[@]}"; do
  ip="$(curl -s -o /dev/null -w '%{remote_ip}' --max-time 3 "http://$name:$PORT/health")"
  rc=$?
  if [ "$rc" -eq 6 ]; then
    fail "$name does not resolve: add the lines from ../live-claude-code/hosts.txt"
  elif [ "$rc" -ne 0 ]; then
    fail "$name resolves but the receiver did not answer on port $PORT: start receiver.py"
  elif [ "$ip" != "127.0.0.1" ]; then
    fail "$name went to $ip, not 127.0.0.1: fix the hosts entry"
  else
    ok "$name -> 127.0.0.1, receiver answers"
  fi
done

state="$(curl -s --max-time 3 "http://127.0.0.1:$CONSOLE_PORT/api/state")"
if [ -z "$state" ]; then
  fail "the console does not answer on 127.0.0.1:$CONSOLE_PORT: run 'harness console --project $DESK'"
elif grep -q '"world_id":"northwind-vendor-desk"' <<<"$state"; then
  ok "console answers on http://127.0.0.1:$CONSOLE_PORT/ for this project"
else
  fail "the console on port $CONSOLE_PORT is serving another world: restart it with --project $DESK"
fi

echo
if [ "$fails" -eq 0 ]; then
  echo "  Ready to present."
else
  echo "  $fails check(s) failed."
  exit 1
fi
