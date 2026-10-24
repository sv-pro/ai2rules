#!/usr/bin/env bash
# Run the governed Claude Code SUT container (PLAN.md E13 / E8), refreshed for the
# Rust hook (D37: `harness cc-hook`).
#
#   ./docker/run.sh                                   # offline shell (NET=none)
#   NET=bridge ./docker/run.sh claude                 # live agent, dogfood governance
#   MODE=replace NET=bridge ./docker/run.sh claude    # REPLACE-mode experiment
#
# The container is the safe place to try governance that could lock the agent out of
# its own tools: a bricked SUT is just Ctrl-C + a fresh `docker run`.
#
# NET= is the OS-level egress floor (E8):
#   none    (default) hard-block ALL network — offline hook/replay testing only
#                     (model API unreachable). Inside: bash docs/demos/replace-permissions/demo.sh
#   bridge            full network; egress governed ONLY by the in-agent hook.
#   For a LIVE-but-contained agent (model API only) use docker/compose.yaml.
#
# MODE= chooses which settings.json governs the SUT — the point of the sandbox:
#   dogfood (default) the repo's own .claude/settings.json (additive governance).
#   replace           overlay docker/sut/settings.replace.json — empty native
#                     permissions, governed entirely by `harness cc-hook --grant
#                     --enforce-absent` against .claude/cc-world.yaml. The
#                     "empty settings.json, govern by the manifest" experiment.
set -euo pipefail

IMAGE="${IMAGE:-governed-claude}"
NET="${NET:-none}"
MODE="${MODE:-dogfood}"
REPO="$(cd "$(dirname "$0")/.." && pwd)"

if [ "$NET" != "none" ] && [ -z "${ANTHROPIC_API_KEY:-}" ]; then
  echo "warning: ANTHROPIC_API_KEY is unset — a live Claude Code session won't authenticate." >&2
fi

MOUNTS=(-v "$REPO:/workspace")
case "$MODE" in
  dogfood) ;;
  replace)
    # Overlay a dedicated settings.json over the project's — container-only, so the
    # host repo file is untouched. This is "separate settings under dev vs runtime".
    MOUNTS+=(-v "$REPO/docker/sut/settings.replace.json:/workspace/.claude/settings.json:ro")
    echo "[run] MODE=replace — native permissions emptied; manifest is the allowlist (--grant --enforce-absent)." >&2
    ;;
  *) echo "unknown MODE=$MODE (use dogfood|replace)" >&2; exit 2 ;;
esac

# --cap-drop ALL / no-new-privileges: minimal privileges. --rm: ephemeral.
exec docker run --rm -it \
  --network "$NET" \
  --cap-drop ALL \
  --security-opt no-new-privileges \
  "${MOUNTS[@]}" \
  -e ANTHROPIC_API_KEY \
  "$IMAGE" "${@:-bash}"
