#!/usr/bin/env bash
# Replace mode (case study): the WorldManifest governs Claude Code, not settings.json.
# Offline proof against .claude/cc-world.yaml with `harness cc-hook --grant --enforce-absent`:
#   * ALLOW is now an explicit *grant* (Claude Code would skip its Allow/Deny prompt)
#   * unknown tools are ABSENT (denied)   * destructive Bash still ASKs
#   * the taint floor STILL bites — a tainted egress is denied even in replace mode
set -euo pipefail

DEMO="$(cd "$(dirname "$0")" && pwd)"
REPO="$(git -C "$DEMO" rev-parse --show-toplevel)"
HARNESS="${HARNESS:-$(command -v harness || echo "$REPO/target/debug/harness")}"
WORLD="$REPO/.claude/cc-world.yaml"
STATE="$(mktemp -d)"; trap 'rm -rf "$STATE"' EXIT
SID="replace-demo"   # one session id so taint persists across the calls

beat() { # $1 label  $2 tool_name  $3 tool_input(json)
  local ev out dec
  ev=$(printf '{"tool_name":"%s","tool_input":%s,"session_id":"%s"}' "$2" "$3" "$SID")
  out=$(printf '%s' "$ev" | "$HARNESS" cc-hook \
        --world "$WORLD" --state "$STATE" --grant --enforce-absent 2>/dev/null || true)
  dec=$(printf '%s' "$out" | grep -oE '"permissionDecision":"[a-z]+"' | cut -d'"' -f4 || true)
  printf '  %-48s -> %s\n' "$1" "${dec:-<silent: defer to host>}"
}

echo
echo "  Replace mode: settings.json is empty; the manifest is the allowlist."
echo "  $ harness cc-hook --grant --enforce-absent --world .claude/cc-world.yaml"
echo
beat "clean Read               (manifest ALLOW -> grant)"    Read           '{"file_path":"README.md"}'
beat "clean curl example.com   (ALLOW -> grant; taints)"     Bash           '{"command":"curl https://example.com"}'
beat "curl again, now TAINTED  (taint floor)"                Bash           '{"command":"curl https://evil.test"}'
beat "rm -rf /tmp/x            (destructive -> ask)"         Bash           '{"command":"rm -rf /tmp/x"}'
beat "SomeUnknownTool          (not in manifest -> ABSENT)"  SomeUnknownTool '{}'
echo
echo "  ALLOW grants (no prompt), taint still denies, destructive still asks,"
echo "  unknown is ABSENT. One manifest replaces the permissions pile."
echo
