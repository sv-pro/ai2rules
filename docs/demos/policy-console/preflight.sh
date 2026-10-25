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
PY=python3
"$PY" -c '' 2>/dev/null || PY=python
if ! "$PY" -c '' 2>/dev/null; then
  echo "preflight: Python is required (as it is for receiver.py)." >&2
  exit 2
fi

ok()   { printf '  ok    %s\n' "$1"; }
fail() { printf '  FAIL  %s\n' "$1"; fails=$((fails + 1)); }

echo
HARNESS="${HARNESS:-$(command -v harness || true)}"
if [ -n "$HARNESS" ] && [ -x "$HARNESS" ]; then
  if "$HARNESS" console --help >/dev/null 2>&1; then
    ok "harness: $HARNESS ($("$HARNESS" --version 2>/dev/null | head -1))"
  else
    fail "harness at $HARNESS has no 'console' command: install a newer one"
  fi
else
  fail "no 'harness' on PATH: npm install -g ai2rules-harness@latest"
fi

SHIM="$DESK/.claude/hooks/world-gate.sh"
if [ -f "$SHIM" ] && bash -n "$SHIM"; then
  ok "project: $DESK (hook shell syntax valid)"
else
  fail "project or valid hook missing: run setup.sh"
fi

# Inspect the generated wiring without running the hook: a real hook probe would
# write session state and decisions, while preflight must remain read-only.
if "$PY" - "$DESK" "$SHIM" <<'PY'
import json
import os
import pathlib
import shlex
import subprocess
import sys

desk, shim = (pathlib.Path(p).resolve() for p in sys.argv[1:])
try:
    settings = json.loads((desk / '.claude/settings.json').read_text())
    for name in ('settings.json', 'settings.local.json'):
        path = desk / '.claude' / name
        if path.exists() and json.loads(path.read_text()).get('disableAllHooks'):
            raise ValueError(f'{name} disables hooks')
    wired = False
    for entry in settings.get('hooks', {}).get('PreToolUse', []):
        if entry.get('matcher') != '*':
            continue
        for hook in entry.get('hooks', []):
            args = shlex.split(hook.get('command', ''))
            if (hook.get('type') == 'command' and len(args) == 2
                    and args[0] == 'bash' and pathlib.Path(args[1]).resolve() == shim):
                wired = True
    if not wired:
        raise ValueError('no all-tools PreToolUse entry pointing at this project shim')
    lines = shim.read_text(encoding='utf-8').replace('\\\n', '').splitlines()
    commands = [shlex.split(line) for line in lines if line.startswith('exec ')]
    if len(commands) != 1 or commands[0][:3] != ['exec', '$BIN', 'cc-hook']:
        raise ValueError('unrecognized hook exec; rerun setup.sh')
    command = commands[0]
    for flag in ('--grant', '--enforce-absent'):
        if command.count(flag) != 1:
            raise ValueError(f'hook must enable {flag} exactly once')
    for flag, value in (('--world', '$PD/.claude/cc-world.yaml'),
                        ('--state', '$PD/.claude/state'),
                        ('--log', '$PD/.claude/state/decisions.jsonl')):
        if command.count(flag) != 1 or command[command.index(flag) + 1] != value:
            raise ValueError(f'hook {flag} must point at {value}')
    binary = shlex.split(next(line.split('=', 1)[1] for line in lines
                             if line.startswith('TRUSTED_BIN=')))[0]
    binary = os.environ.get('HARNESS_BIN') or binary
    if not pathlib.Path(binary).is_absolute() or not os.access(binary, os.X_OK):
        raise ValueError(f'the hook binary is not an absolute executable: {binary}')
    off = shlex.split(next(line for line in lines if line.startswith('if [ -f ')))[3]
    if pathlib.Path(off).exists():
        raise ValueError(f'the project kill switch is on: {off}')
    request = {'v': 1, 'context': {'source_channel': 'user_prompt'},
               'tools': [{'name': 'Skill'}, {'name': 'AskUserQuestion'}]}
    projected = subprocess.run([binary, 'project', '--world', str(desk / '.claude/cc-world.yaml')],
                               input=json.dumps(request), capture_output=True, text=True, timeout=10)
    if projected.returncode or {t['name'] for t in json.loads(projected.stdout)['tools']} != {'Skill', 'AskUserQuestion'}:
        raise ValueError('the selected hook binary cannot compile/project Skill and AskUserQuestion')
    skill = desk / '.claude/skills/northwind-demo/SKILL.md'
    if not skill.is_file():
        raise ValueError('/northwind-demo is not installed; rerun setup.sh')
except (OSError, ValueError, StopIteration, IndexError, KeyError, TypeError, AttributeError, subprocess.TimeoutExpired) as exc:
    print(f'  FAIL  hook/skill configuration: {exc}')
    sys.exit(1)
print('  ok    all-tools hook, exact world/state/log, grant/ABSENT, selected binary projection and /northwind-demo')
PY
then :; else fails=$((fails + 1)); fi

if [ -f "$HOME/.claude/gate-off" ]; then
  fail "the kill switch is on (~/.claude/gate-off): the hook is not running"
fi

for name in "${NAMES[@]}"; do
  ip="$(curl --noproxy '*' -fsS -o /dev/null -w '%{remote_ip}' --max-time 3 "http://$name:$PORT/health" 2>/dev/null)"
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

state="$(curl --noproxy '*' -fsS --max-time 3 "http://127.0.0.1:$CONSOLE_PORT/api/state" 2>/dev/null)"
if [ -z "$state" ]; then
  fail "the console does not answer on 127.0.0.1:$CONSOLE_PORT: run 'harness console --project $DESK'"
elif "$PY" - "$DESK" "$state" <<'PY'
import json
import pathlib
import sys

try:
    desk = pathlib.Path(sys.argv[1]).resolve()
    state = json.loads(sys.argv[2])
    valid = (state.get('ok') is True
             and state.get('world_id') == 'northwind-vendor-desk'
             and pathlib.Path(state['world_path']).resolve() == desk / '.claude/cc-world.yaml'
             and pathlib.Path(state['log_path']).resolve() == desk / '.claude/state/decisions.jsonl')
    sys.exit(0 if valid else 1)
except (ValueError, KeyError, TypeError, AttributeError):
    sys.exit(1)
PY
then
  ok "console answers on http://127.0.0.1:$CONSOLE_PORT/ for this project"
else
  fail "the console on port $CONSOLE_PORT has the wrong project/world/log: restart it with --project $DESK"
fi

echo
if [ "$fails" -eq 0 ]; then
  echo "  Stand checks passed. Interactive /northwind-demo rehearsal is still required."
else
  echo "  $fails check(s) failed."
  exit 1
fi
