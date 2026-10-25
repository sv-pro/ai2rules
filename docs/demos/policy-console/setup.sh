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

# Current init already logs PreToolUse and outcome events; older releases did
# not. Add ABSENT enforcement without depending on physical line breaks, and
# add a log only when init did not supply one.
PY=python3
"$PY" -c '' 2>/dev/null || PY=python
SHIM="$DESK/.claude/hooks/world-gate.sh"
# The script goes in with -c, not on stdin with `-`: on Windows `python3` can be
# the Python install manager's alias, which reads the first file argument's
# shebang (here the hook's `bash`) and runs that instead of Python.
patch_shim="$(cat <<'PY'
import pathlib
import shlex
import sys

path = pathlib.Path(sys.argv[1])
text = path.read_text(encoding='utf-8').replace('\\\n', '')
lines = text.splitlines()
indices = [i for i, line in enumerate(lines) if line.startswith('exec ')]
if len(indices) != 1:
    raise SystemExit('setup: expected one generated hook exec line; rerun harness init')
i = indices[0]
args = shlex.split(lines[i])
if args[:3] != ['exec', '$BIN', 'cc-hook'] or '--grant' not in args:
    raise SystemExit('setup: unrecognized generated hook; expected cc-hook --grant')
if '--enforce-absent' not in args:
    lines[i] += ' --enforce-absent'
if '--log' not in args:
    lines[i] += ' --log "$PD/.claude/state/decisions.jsonl"'
path.write_bytes(('\n'.join(lines) + '\n').encode('utf-8'))
PY
)"
"$PY" -c "$patch_shim" "$SHIM"
bash -n "$SHIM"

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
