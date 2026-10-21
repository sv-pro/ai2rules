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

rm -rf "$DEST/governed" "$DEST/ungoverned"
mkdir -p "$DEST/governed" "$DEST/ungoverned" "$DEST/received"
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

echo "Ready."
echo
echo "  governed   : $DEST/governed      (ai2rules hook installed)"
echo "  ungoverned : $DEST/ungoverned    (plain Claude Code)"
echo
echo "Next (three terminals):"
echo "  1) receiver:  python3 $HERE/receiver.py --out $DEST/received"
echo "  2) ungoverned Claude Code:  cd $DEST/ungoverned && claude"
echo "  3) governed   Claude Code:  cd $DEST/governed   && claude"
echo
echo "Run the SAME prompts in 2) and 3). See README.md for the script and plan B."
echo "Kill switch for the governed copy if anything wedges:  touch \$HOME/.claude/gate-off"
