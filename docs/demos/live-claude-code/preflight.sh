#!/usr/bin/env bash
# preflight.sh - check the stand right before the demo, after setup.sh and with the
# receiver running. Read-only: it changes nothing, and talks only to 127.0.0.1.
#
#   bash preflight.sh [DEST]      DEST defaults to ~/northwind
#
# Checks: the harness binary, the two project copies, that both demo names resolve
# to this machine and reach the receiver, that the page served is the plain one,
# and that a .test name NOT in the hosts file resolves nowhere (fails closed).
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEST="${1:-$HOME/northwind}"
PLAIN="$DEST/vendor-desk-1"
GUARDED="$DEST/vendor-desk-2"
PORT="${PORT:-8787}"
NAMES=(portal.northwind.test files.northwind.test)
fails=0

ok()   { printf '  ok    %s\n' "$1"; }
fail() { printf '  FAIL  %s\n' "$1"; fails=$((fails + 1)); }

echo
if HARNESS="$(command -v harness)"; then
  ok "harness: $HARNESS ($("$HARNESS" --version 2>/dev/null | head -1))"
else
  fail "no 'harness' on PATH: npm install -g ai2rules-harness@latest"
fi

if [ -f "$PLAIN/client-data.csv" ] && [ ! -e "$PLAIN/.claude" ]; then
  ok "left window:  $PLAIN (plain, no hook)"
else
  fail "left copy missing or has a .claude folder: run setup.sh"
fi
if [ -f "$GUARDED/.claude/settings.json" ] && [ -f "$GUARDED/client-data.csv" ]; then
  ok "right window: $GUARDED (ai2rules hook installed)"
else
  fail "right copy missing or incomplete: run setup.sh"
fi

hosts_hint=0
for name in "${NAMES[@]}"; do
  ip="$(curl -s -o /dev/null -w '%{remote_ip}' --max-time 3 "http://$name:$PORT/health")"
  rc=$?
  if [ "$rc" -eq 6 ]; then
    fail "$name does not resolve: add the hosts.txt lines"
    hosts_hint=1
  elif [ "$rc" -ne 0 ]; then
    fail "$name resolves but the receiver did not answer on port $PORT: start receiver.py"
  elif [ "$ip" != "127.0.0.1" ]; then
    fail "$name went to $ip, not 127.0.0.1: fix the hosts entry"
  else
    ok "$name -> 127.0.0.1, receiver answers"
  fi
done

page="$(curl -s --max-time 3 "http://${NAMES[0]}:$PORT/onboarding" 2>/dev/null)"
if [ -z "$page" ]; then
  fail "could not fetch the onboarding page"
elif grep -qiE '<!--|ai-slot|placeholder' <<<"$page"; then
  fail "the page served has comments or a slot in it: restart the receiver"
else
  ok "page served is the plain one (no comments, nothing hidden)"
fi

# The safety property of .test: a name that is not in the hosts file goes nowhere.
curl -s -o /dev/null --max-time 3 "http://not-in-hosts.northwind.test:$PORT/" 2>/dev/null
if [ $? -eq 6 ]; then
  ok "a .test name not in hosts resolves nowhere (fails closed)"
else
  fail "not-in-hosts.northwind.test resolved: check your DNS or proxy settings"
fi

echo
if [ "$hosts_hint" -eq 1 ]; then
  echo "  Add these lines to the hosts file (as Administrator on Windows):"
  grep -v '^#' "$HERE/hosts.txt" | sed 's/^/    /'
  echo
fi
if [ "$fails" -eq 0 ]; then
  echo "  Ready to present."
else
  echo "  $fails check(s) failed."
  exit 1
fi
