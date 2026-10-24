#!/usr/bin/env bash
# run-demo.sh - the whole policy-console stand from one command, so nobody copies
# commands from the runbook in front of an audience:
#
#   1. rebuild the project (setup.sh: a clean copy, an empty decision log)
#   2. start the receiver (the "outside" sites) and the console, in the background
#   3. run preflight.sh and stop if the stand is not ready
#   4. open the console in the browser
#   5. start Claude Code in the project, on Opus 4.8 and in the asking mode; without
#      the `claude` CLI (the Claude app on Windows), say how to open the session
#      and show the receiver's request log here instead
#
# Ctrl+C, or leaving Claude Code, stops the receiver and the console it started.
#
#   bash run-demo.sh [--no-claude] [--no-browser] [DEST]     DEST defaults to ~/northwind
#
# Needs `harness` with the `console` command on PATH (or HARNESS=/abs/path/to/harness).
# Logs go to ~/northwind-logs (LOGS=...), outside the project so Claude does not find
# them. PORT and CONSOLE_PORT move the receiver and the console off 8787 and 8790.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
KIT="$HERE/../live-claude-code"
START_CLAUDE=1
OPEN_BROWSER=1
DEST=""
while [ $# -gt 0 ]; do
  case "$1" in
    --no-claude) START_CLAUDE=0 ;;
    --no-browser) OPEN_BROWSER=0 ;;
    -h | --help) sed -n '2,21p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "run-demo: unknown option $1 (try --help)" >&2; exit 2 ;;
    *) DEST="$1" ;;
  esac
  shift
done
DEST="${DEST:-$HOME/northwind}"
DESK="$DEST/vendor-desk"
RECEIVED="${RECEIVED:-$HOME/northwind-inbox}"
LOGS="${LOGS:-$HOME/northwind-logs}"
PORT="${PORT:-8787}"
CONSOLE_PORT="${CONSOLE_PORT:-8790}"
URL="http://127.0.0.1:$CONSOLE_PORT/"

HARNESS="${HARNESS:-$(command -v harness || true)}"
if [ -z "$HARNESS" ] || ! "$HARNESS" console --help >/dev/null 2>&1; then
  echo "run-demo: no 'harness' with the console command (found: ${HARNESS:-none})." >&2
  echo "  Put a build that has it first on PATH, or run: HARNESS=/abs/path/to/harness bash $0" >&2
  exit 2
fi
# preflight.sh looks harness up on PATH: make it find this one. In Git Bash a
# `C:/...` path would split PATH at its colon, so convert it first.
HARNESS_DIR="$(dirname "$HARNESS")"
command -v cygpath >/dev/null 2>&1 && HARNESS_DIR="$(cygpath -u "$HARNESS_DIR")"
PATH="$HARNESS_DIR:$PATH"
export PATH

PY=python3
"$PY" -c '' 2>/dev/null || PY=python

# A receiver or console left over from an earlier run would answer the checks with
# the wrong project, or make ours fail to bind. Refuse rather than guess.
answers() { curl --noproxy '*' -fsS -o /dev/null --max-time 2 "$1" 2>/dev/null; }
if answers "http://127.0.0.1:$PORT/health"; then
  echo "run-demo: something already answers on port $PORT (an old receiver?). Stop it first." >&2
  exit 1
fi
if answers "http://127.0.0.1:$CONSOLE_PORT/api/state"; then
  echo "run-demo: something already answers on port $CONSOLE_PORT (an old console?). Stop it first." >&2
  exit 1
fi

pids=()
cleanup() {
  for p in "${pids[@]+"${pids[@]}"}"; do kill "$p" 2>/dev/null || true; done
}
trap cleanup EXIT
trap 'echo; echo "Stopping the receiver and the console."; exit 130' INT TERM

wait_for() { # url, what
  for _ in $(seq 1 40); do
    answers "$1" && return 0
    sleep 0.25
  done
  echo "run-demo: the $2 did not come up; its log is in $LOGS" >&2
  exit 1
}

echo "1/5  Rebuilding the project in $DESK"
HARNESS="$HARNESS" RECEIVED="$RECEIVED" bash "$HERE/setup.sh" "$DEST" >/dev/null

echo "2/5  Starting the receiver (:$PORT) and the console (:$CONSOLE_PORT); logs in $LOGS"
mkdir -p "$LOGS"
"$PY" -u "$KIT/receiver.py" --port "$PORT" --out "$RECEIVED" >"$LOGS/receiver.log" 2>&1 &
pids+=("$!")
"$HARNESS" console --project "$DESK" --port "$CONSOLE_PORT" >"$LOGS/console.log" 2>&1 &
pids+=("$!")
wait_for "http://127.0.0.1:$PORT/health" receiver
wait_for "http://127.0.0.1:$CONSOLE_PORT/api/state" console

echo "3/5  Checking the stand"
if ! PORT="$PORT" CONSOLE_PORT="$CONSOLE_PORT" bash "$HERE/preflight.sh" "$DEST"; then
  echo "run-demo: the stand is not ready (see above). Logs: $LOGS" >&2
  exit 1
fi

echo "4/5  Console: $URL"
if [ "$OPEN_BROWSER" -eq 1 ]; then
  case "$(uname -s)" in
    MINGW* | MSYS* | CYGWIN*) cmd.exe //c start "" "$URL" ;;
    Darwin) open "$URL" ;;
    *)
      if grep -qi microsoft /proc/version 2>/dev/null; then
        explorer.exe "$URL" || true # WSL: explorer returns 1 even when it worked
      else
        xdg-open "$URL" >/dev/null 2>&1 || echo "     (open it yourself)"
      fi
      ;;
  esac
fi

if [ "$START_CLAUDE" -eq 1 ] && command -v claude >/dev/null 2>&1; then
  echo "5/5  Starting Claude Code in $DESK (Opus 4.8, asking mode)."
  echo "     Receiver log in another pane:  tail -f $LOGS/receiver.log"
  echo "     Answer permission questions with \"1. Yes\" or \"No\" only."
  echo
  # Not exec: when Claude Code exits, the EXIT trap stops the servers.
  (cd "$DESK" && claude --model claude-opus-4-8 --permission-mode default) || true
else
  # The Claude app's folder picker wants C:\..., not /c/...
  DESK_SHOWN="$DESK"
  command -v cygpath >/dev/null 2>&1 && DESK_SHOWN="$(cygpath -w "$DESK")"
  echo "5/5  Open Claude yourself:"
  echo "       Claude app: a new Code session on the folder $DESK_SHOWN"
  echo "                   mode Manual, model Opus 4.8"
  echo "       terminal  : cd $DESK && claude --model claude-opus-4-8 --permission-mode default"
  echo
  echo "     The receiver's request log follows. Ctrl+C here stops the receiver and the console."
  echo
  tail -n +1 -f "$LOGS/receiver.log"
fi
