#!/usr/bin/env bash
# setup.sh - lay out the two project copies for the live demo: one governed by
# ai2rules, one not. Run once before the demo. Idempotent: re-running rebuilds
# both copies from docs/demos/live-claude-code/project.
#
#   bash setup.sh [DEST]      DEST defaults to ~/northwind
#
# The folder names are neutral on purpose: Claude sees its working directory, and
# "governed", "ungoverned" or "demo" in the path would tell it what is coming.
#   DEST/vendor-desk-1   plain Claude Code (the left window)
#   DEST/vendor-desk-2   governed by ai2rules (the right window)
# Uploads the receiver gets go to RECEIVED (default ~/northwind-inbox), outside
# DEST, so neither window finds them by looking around.
#
# Needs the `harness` binary OUTSIDE the project it governs. Install it first:
#   npm install -g ai2rules-harness     (or cargo build and point HARNESS at it)
# then:  HARNESS=/abs/path/to/harness bash setup.sh
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEST="${1:-$HOME/northwind}"
PLAIN="$DEST/vendor-desk-1"
GUARDED="$DEST/vendor-desk-2"
RECEIVED="${RECEIVED:-$HOME/northwind-inbox}"
HARNESS="${HARNESS:-$(command -v harness || true)}"

if [ -z "$HARNESS" ] || [ ! -x "$HARNESS" ]; then
  echo "setup: no 'harness' binary found." >&2
  echo "  Install it outside this project, then re-run:" >&2
  echo "    npm install -g ai2rules-harness && bash setup.sh" >&2
  echo "  or point HARNESS at a built binary:" >&2
  echo "    HARNESS=/abs/path/to/harness bash setup.sh" >&2
  exit 2
fi

case "$HARNESS" in "$HERE"*|"$DEST"*)
  echo "setup: HARNESS ($HARNESS) is inside the demo tree; use one installed elsewhere." >&2
  exit 2 ;;
esac

echo "harness : $HARNESS"
echo "dest    : $DEST"
echo

# Empty the folders rather than removing them: on Windows a folder that is some
# process's working directory (a Claude app session, even a closed one) cannot be
# removed, and `rm -rf` fails with "Device or resource busy".
mkdir -p "$GUARDED" "$PLAIN" "$RECEIVED"
find "$GUARDED" "$PLAIN" "$RECEIVED" -mindepth 1 -delete
cp -r "$HERE/project/." "$GUARDED/"
cp -r "$HERE/project/." "$PLAIN/"

# Govern one copy: writes .claude/{cc-world.yaml,hooks/world-gate.sh,settings.json}
# and bakes this harness binary's absolute path into the shim. --grant makes the
# manifest the allowlist so ALLOW is explicit; the taint DENY and destructive ASK
# show in additive mode too.
"$HARNESS" init --grant "$GUARDED" >/dev/null
# Swap the starter manifest for this demo's smaller world, so the beats are the
# only verdicts. (init writes the generic starter; the demo wants the quiet one.)
# world.yaml says nothing about a demo: Claude can read it in the project.
cp "$HERE/world.yaml" "$GUARDED/.claude/cc-world.yaml"

# On Windows `python3` is often a Microsoft Store stub that does nothing.
PY=python3
"$PY" -c '' 2>/dev/null || PY=python

echo "Ready."
echo
echo "  left  window : $PLAIN    (plain Claude Code)"
echo "  right window : $GUARDED    (ai2rules hook installed)"
echo "  uploads land : $RECEIVED"
echo
echo "Next:"
echo "  1) start the receiver, and leave it running:"
echo "       $PY $HERE/receiver.py --out $RECEIVED"
echo "     then check the stand (needs the hosts.txt lines once per machine):"
echo "       bash $HERE/preflight.sh $DEST"
echo "  2) open Claude Code twice, one session per folder, in the SAME permission mode"
echo "     (Manual in the Claude app, the default ask mode in the terminal):"
echo "       Claude app : a new Code session on each folder, side by side"
echo "       terminal   : cd <folder> && claude"
echo
echo "Type the SAME prompts into both. See README.md for the script and plan B."
echo "Kill switch for the right window if anything wedges:  touch \$HOME/.claude/gate-off"
