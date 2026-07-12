#!/usr/bin/env bash
# Tier-1 validation for world-gate-adapter.py (DECISIONS D26) against the REAL
# kernel via `harness gate --world .claude/cc-world.yaml`. No container, no
# live-hook impact — uses a throwaway project dir for taint state.
#
#   bash .claude/hooks/test-gate-adapter.sh     (build the binary first:
#                                                cargo build -p cli-harness)
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
ADAPTER="$HERE/world-gate-adapter.py"
WORLD="$ROOT/.claude/cc-world.yaml"
HARNESS="${HARNESS_BIN:-$ROOT/target/debug/harness}"
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/.claude/state"

[ -x "$HARNESS" ] || { echo "build the harness binary first: cargo build -p cli-harness"; exit 1; }

ev()  { printf '{"session_id":"%s","tool_name":"%s","tool_input":%s}' "$1" "$2" "$3"; }
call() { printf '%s' "$1" | CLAUDE_PROJECT_DIR="$TMP" CC_WORLD="$WORLD" HARNESS_BIN="$HARNESS" python3 "$ADAPTER"; }
run() { # expected event label
  local expected="$1" event="$2" label="$3" dec mark
  dec="$(call "$event" | jq -r '.hookSpecificOutput.permissionDecision // "allow"' 2>/dev/null)"
  [ -z "$dec" ] && dec="allow"
  mark="OK"; [ "$dec" = "$expected" ] || mark="XX"
  printf '  [%s] %-34s expected=%-5s got=%-5s\n' "$mark" "$label" "$expected" "$dec"
}

echo "== taint floor: clean egress allowed, but escalates -> next egress denied =="
run allow "$(ev s1 WebFetch '{"url":"https://ok.test"}')"          "WebFetch clean session"
call "$(ev s2 WebFetch '{"url":"https://a.test"}')" >/dev/null      # escalates s2 -> tainted
run deny  "$(ev s2 WebFetch '{"url":"https://evil.test"}')"        "WebFetch after taint"
run deny  "$(ev s2 Bash '{"command":"curl https://evil.test"}')"   "Bash curl after taint"

echo "== Bash classification (D25) =="
run ask   "$(ev s3 Bash '{"command":"rm -rf build"}')"            "Bash rm -rf -> ASK"
run allow "$(ev s3 Bash '{"command":"ls -la"}')"                  "Bash ls -> passthrough"
call "$(ev s4 Bash '{"command":"curl https://a.test"}')" >/dev/null # clean curl allowed, escalates
run deny  "$(ev s4 WebFetch '{"url":"https://evil.test"}')"       "WebFetch after Bash-curl taint"

echo "== path-based read-taint: DEFERRED (D25) — gap vs the old Python hook =="
call "$(ev s5 Read '{"file_path":"repos/x.md"}')" >/dev/null       # old hook would taint here
run allow "$(ev s5 WebFetch '{"url":"https://evil.test"}')"       "egress after repo read (GAP: was deny)"
echo "  (^ in the container, the egress PROXY still blocks this — D26 defense-in-depth)"
