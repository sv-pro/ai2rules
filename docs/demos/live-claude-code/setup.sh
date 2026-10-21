#!/usr/bin/env bash
# setup.sh - lay out the two project copies for the live demo: one governed by
# ai2rules, one not. Run once before the demo. Idempotent: re-running rebuilds
# both copies from docs/demos/live-claude-code/project.
#
#   bash setup.sh [DEST]      DEST defaults to ~/ai2rules-live-demo
#
# Needs the `harness` binary OUTSIDE the project it governs. Install it first:
#   npm install -g ai2rules-harness     (or cargo build and point HARNESS at it)
# then:  HARNESS=/abs/path/to/harness bash setup.sh
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEST="${1:-$HOME/ai2rules-live-demo}"
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
mkdir -p "$DEST/governed" "$DEST/ungoverned" "$DEST/received"
find "$DEST/governed" "$DEST/ungoverned" "$DEST/received" -mindepth 1 -delete
cp -r "$HERE/project/." "$DEST/governed/"
cp -r "$HERE/project/." "$DEST/ungoverned/"

# Govern one copy: writes .claude/{cc-world.yaml,hooks/world-gate.sh,settings.json}
# and bakes this harness binary's absolute path into the shim. --grant makes the
# manifest the allowlist so ALLOW is explicit; the taint DENY and destructive ASK
# show in additive mode too.
"$HARNESS" init --grant "$DEST/governed" >/dev/null
# Swap the starter manifest for this demo's smaller world, so the beats are the
# only verdicts. (init writes the generic starter; the demo wants the quiet one.)
cp "$HERE/world.yaml" "$DEST/governed/.claude/cc-world.yaml"

# On Windows `python3` is often a Microsoft Store stub that does nothing.
PY=python3
"$PY" -c '' 2>/dev/null || PY=python

echo "Ready."
echo
echo "  governed   : $DEST/governed      (ai2rules hook installed)"
echo "  ungoverned : $DEST/ungoverned    (plain Claude Code)"
echo
echo "Next:"
echo "  1) start the receiver, and leave it running:"
echo "       $PY $HERE/receiver.py --out $DEST/received"
echo "  2) open Claude Code twice, one session per folder, in the SAME permission mode"
echo "     (Manual in the Claude app, the default ask mode in the terminal):"
echo "       Claude app : a new Code session on each folder, side by side"
echo "       terminal   : cd <folder> && claude"
echo
echo "Type the SAME prompts into both. See README.md for the script and plan B."
echo "Kill switch for the governed copy if anything wedges:  touch \$HOME/.claude/gate-off"
