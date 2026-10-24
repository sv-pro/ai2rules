#!/usr/bin/env bash
# setup.sh - lay out the project for the policy-console demo: one copy of the
# Northwind vendor desk, governed by ai2rules, with the decision log on and
# ABSENT enforced, so the console's "Off" really takes an action away.
# Idempotent: re-running rebuilds the folder and empties the log.
#
#   bash setup.sh [DEST]      DEST defaults to ~/northwind
#
#   DEST/vendor-desk     the project Claude Code works in (the name is neutral on
#                        purpose: Claude sees its working directory)
#   ~/northwind-inbox    where the receiver stores uploads (outside DEST)
#
# Needs the `harness` binary OUTSIDE the project, version with `harness console`:
#   npm install -g ai2rules-harness@latest    (or HARNESS=/abs/path/to/harness)
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
KIT="$HERE/../live-claude-code"
DEST="${1:-$HOME/northwind}"
DESK="$DEST/vendor-desk"
RECEIVED="${RECEIVED:-$HOME/northwind-inbox}"
HARNESS="${HARNESS:-$(command -v harness || true)}"
PORT="${PORT:-8787}"
CONSOLE_PORT="${CONSOLE_PORT:-8790}"
for port in "$PORT" "$CONSOLE_PORT"; do
  if ! [[ "$port" =~ ^[0-9]{1,5}$ ]] || (( 10#$port < 1 || 10#$port > 65535 )); then
    echo "setup: PORT and CONSOLE_PORT must be in 1..65535." >&2
    exit 2
  fi
done

if [ -z "$HARNESS" ] || [ ! -x "$HARNESS" ]; then
  echo "setup: no 'harness' binary found. Install it outside this project:" >&2
  echo "    npm install -g ai2rules-harness@latest && bash setup.sh" >&2
  echo "  or point HARNESS at a built binary: HARNESS=/abs/path/to/harness bash setup.sh" >&2
  exit 2
fi
case "$HARNESS" in "$HERE"*|"$DEST"*)
  echo "setup: HARNESS ($HARNESS) is inside the demo tree; use one installed elsewhere." >&2
  exit 2 ;;
esac
if ! "$HARNESS" console --help >/dev/null 2>&1; then
  echo "setup: $HARNESS has no 'console' command; it is too old for this demo." >&2
  exit 2
fi

echo "harness : $HARNESS"
echo "project : $DESK"
echo

# Empty rather than remove: on Windows a folder that is some process's working
# directory cannot be removed.
mkdir -p "$DESK" "$RECEIVED"
find "$DESK" "$RECEIVED" -mindepth 1 -delete
cp -r "$KIT/project/." "$DESK/"

# Govern it in replace mode, then swap in this demo's world.
"$HARNESS" init --grant "$DESK" >/dev/null
cp "$HERE/world.yaml" "$DESK/.claude/cc-world.yaml"

# The scenario as a skill: `/northwind-demo` runs the four steps and pauses for
# each switch the presenter flips. Only here, not in the shared project kit, so
# the side-by-side demo's copies do not get it.
mkdir -p "$DESK/.claude/skills/northwind-demo"
sed -e 's/:8787/:__NORTHWIND_RECEIVER__/g' -e 's/:8790/:__NORTHWIND_CONSOLE__/g' \
  -e "s/__NORTHWIND_RECEIVER__/$PORT/g" -e "s/__NORTHWIND_CONSOLE__/$CONSOLE_PORT/g" \
  "$HERE/skill/SKILL.md" > "$DESK/.claude/skills/northwind-demo/SKILL.md"

# Two flags the starter shim does not set:
#   --enforce-absent  an action switched Off is refused, not left to Claude Code's
#                     own prompt, so Off means "this agent does not have it"
#   --log             one line per decision, which the console shows live
SHIM="$DESK/.claude/hooks/world-gate.sh"
sed -i.bak 's#--state "$PD/.claude/state"$#--state "$PD/.claude/state" --enforce-absent --log "$PD/.claude/state/decisions.jsonl"#' "$SHIM"
rm -f "$SHIM.bak"
if ! grep -q -- '--enforce-absent --log' "$SHIM"; then
  echo "setup: could not add --enforce-absent and --log to $SHIM (did 'harness init' change?)" >&2
  exit 1
fi

PY=python3
"$PY" -c '' 2>/dev/null || PY=python

echo "Ready. Next, each in its own terminal, left running:"
echo
echo "  1) receiver (the outside sites, all on 127.0.0.1):"
echo "       $PY $KIT/receiver.py --port $PORT --out $RECEIVED"
echo "  2) the console, then open http://127.0.0.1:$CONSOLE_PORT/ in a browser:"
echo "       $HARNESS console --project $DESK --port $CONSOLE_PORT"
echo "  3) check the stand:"
echo "       HARNESS='$HARNESS' PORT=$PORT CONSOLE_PORT=$CONSOLE_PORT bash $HERE/preflight.sh $DEST"
echo "  4) Claude Code in the project:"
echo "       cd $DESK && claude --model claude-opus-4-8 --permission-mode default"
echo
echo "Kill switch if anything wedges:  touch \$HOME/.claude/gate-off"
