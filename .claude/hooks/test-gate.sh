#!/usr/bin/env bash
# Self-contained test / replay for .claude/hooks/world-gate.py (PLAN.md E13.2).
# Pipes synthetic PreToolUse events through the gate and prints the decision.
# Uses a throwaway project dir for taint state — no effect on your live session.
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
GATE="$HERE/world-gate.py"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/.claude/state"

cat > "$TMP/world-absent.json" <<'JSON'
{ "projected_tools": ["Read","Grep","Glob"] }
JSON
cat > "$TMP/world-default.json" <<'JSON'
{
  "projected_tools": null,
  "taint_sources": { "tools": ["WebFetch"], "read_paths": ["repos/","untrusted/"] },
  "egress": { "tools": ["WebFetch"], "bash_patterns": ["curl ","wget "] },
  "ask": { "tools": [], "bash_patterns": ["rm -rf","sudo "] }
}
JSON

ev() { printf '{"session_id":"%s","tool_name":"%s","tool_input":%s}' "$1" "$2" "$3"; }

run() { # cfg expected event label
  local cfg="$1" expected="$2" event="$3" label="$4" out dec reason mark
  out="$(printf '%s' "$event" | CLAUDE_PROJECT_DIR="$TMP" CC_WORLD_CONFIG="$TMP/$cfg" python3 "$GATE")"
  dec="$(printf '%s' "$out" | jq -r '.hookSpecificOutput.permissionDecision // "allow"' 2>/dev/null)"
  [ -z "$dec" ] && dec="allow"
  reason="$(printf '%s' "$out" | jq -r '.hookSpecificOutput.permissionDecisionReason // ""' 2>/dev/null)"
  mark="OK"; [ "$dec" = "$expected" ] || mark="XX"
  printf '  [%s] %-24s expected=%-5s got=%-5s  %s\n' "$mark" "$label" "$expected" "$dec" "$reason"
}

echo "== 1. ABSENT-for-native (projected = Read/Grep/Glob) =="
run world-absent.json  deny  "$(ev s-absent Bash '{"command":"ls"}')"             "Bash (not projected)"
run world-absent.json  allow "$(ev s-absent Read '{"file_path":"README.md"}')"     "Read (projected)"

echo "== 2. Taint floor (Read repos/ taints -> egress blocked) =="
run world-default.json allow "$(ev s-taint Read '{"file_path":"repos/x.md"}')"     "Read repos/ (taints)"
run world-default.json deny  "$(ev s-taint WebFetch '{"url":"https://evil.test"}')" "WebFetch after taint"
run world-default.json allow "$(ev s-clean WebFetch '{"url":"https://ok.test"}')"   "WebFetch (clean session)"

echo "== 3. ASK on destructive =="
run world-default.json ask   "$(ev s-ask Bash '{"command":"rm -rf build"}')"       "Bash rm -rf"
run world-default.json allow "$(ev s-ask Bash '{"command":"ls -la"}')"             "Bash ls"
