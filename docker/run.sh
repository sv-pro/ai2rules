#!/usr/bin/env bash
# Run the governed Claude Code SUT container (PLAN.md E13 / E8).
#
#   ./docker/run.sh                         # offline shell (NET=none)
#   NET=bridge ./docker/run.sh claude       # live, hook-governed agent
#
# Network policy IS the OS-level egress floor (E8). Choose with NET=:
#   none    (default) hard-block ALL network. Claude Code can't reach the model
#                     API, so use this for OFFLINE hook/replay testing only
#                     (inside: bash .claude/hooks/test-gate.sh).
#   bridge            full network; egress is governed ONLY by the in-agent hook
#                     (no OS floor). For a LIVE-but-contained agent you want an
#                     egress ALLOWLIST (model API only) — see docker/README.md.
set -euo pipefail

IMAGE="${IMAGE:-governed-claude}"
NET="${NET:-none}"
REPO="$(cd "$(dirname "$0")/.." && pwd)"

if [ "$NET" != "none" ] && [ -z "${ANTHROPIC_API_KEY:-}" ]; then
  echo "warning: ANTHROPIC_API_KEY is unset — a live Claude Code session won't authenticate." >&2
fi

# Notes on the flags:
#   --cap-drop ALL / --security-opt no-new-privileges : minimal privileges.
#   -v REPO:/workspace                                 : the code + .claude/ config.
#   -v governed-claude-state:.../.claude/state         : a SHARED, persistent taint
#       store — multiple SUT containers sharing this volume share taint (the
#       cross-instance fix for the local sidecar's locality limit).
#   -e CC_WORLD_CONFIG : point the gate at a stricter SUT world without touching
#       the host's .claude/cc-world.json.
# Optional hardening: add `--read-only --tmpfs /home/agent` for an immutable root,
# and `:ro` on the workspace mount for a look-but-don't-touch demo.
exec docker run --rm -it \
  --network "$NET" \
  --cap-drop ALL \
  --security-opt no-new-privileges \
  -v "$REPO:/workspace" \
  -v governed-claude-state:/workspace/.claude/state \
  -e ANTHROPIC_API_KEY \
  -e CC_WORLD_CONFIG \
  "$IMAGE" "${@:-bash}"
