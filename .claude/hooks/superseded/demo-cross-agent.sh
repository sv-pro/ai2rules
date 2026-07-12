#!/usr/bin/env bash
# Demo (PLAN.md E13.6): cross-agent taint propagation on the Claude Code host.
#
# Shows that a SUBAGENT reading untrusted data taints the SHARED session, so the
# PARENT's later network egress is denied — closing the "ZombieAgent" data-
# laundering path *intra-run*. Claude Code gives an in-process agent tree one
# shared session_id, and world-gate keys taint by session_id, so the taint the
# subagent writes is the same taint the parent reads.
#
# Self-contained: throwaway project dir + config, no effect on your real session.
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
GATE="$HERE/world-gate.py"
NOTIFY="$HERE/taint-notify.py"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/.claude/state"

cat > "$TMP/world.json" <<'JSON'
{
  "projected_tools": null,
  "taint_sources": { "tools": ["WebFetch"], "read_paths": ["repos/", "untrusted/"] },
  "egress": { "tools": ["WebFetch"], "bash_patterns": ["curl ", "wget "] },
  "ask": { "tools": [], "bash_patterns": ["rm -rf"] }
}
JSON

gate() { # session label event
  local out dec
  out="$(printf '%s' "$3" | CLAUDE_PROJECT_DIR="$TMP" CC_WORLD_CONFIG="$TMP/world.json" python3 "$GATE")"
  dec="$(printf '%s' "$out" | jq -r '.hookSpecificOutput.permissionDecision // "allow"' 2>/dev/null)"
  [ -z "$dec" ] && dec="allow"
  printf '   gate[%-6s] %-34s -> %s\n' "$1" "$2" "$dec"
}

stop() { # event
  printf '%s' "$1" | CLAUDE_PROJECT_DIR="$TMP" CC_WORLD_CONFIG="$TMP/world.json" python3 "$NOTIFY" \
    | jq -r '.systemMessage // empty' 2>/dev/null | sed 's/^/   SubagentStop: /'
}

echo "1) Baseline — a CLEAN session can reach the network:"
gate clean  "parent WebFetch (clean)" \
  '{"session_id":"clean","tool_name":"WebFetch","tool_input":{"url":"https://api.example"}}'

echo
echo "2) A SUBAGENT (shared session) reads untrusted data under repos/:"
gate shared "subagent Read repos/intel.md" \
  '{"session_id":"shared","agent_type":"general-purpose","tool_name":"Read","tool_input":{"file_path":"repos/intel.md"}}'
echo "   taint marker: $(cat "$TMP/.claude/state/taint-shared" 2>/dev/null)"

echo
echo "3) Subagent finishes -> SubagentStop surfaces the taint (observability):"
stop '{"session_id":"shared","agent_type":"general-purpose","hook_event_name":"SubagentStop"}'

echo
echo "4) The PARENT now tries to exfiltrate over the SAME session -> blocked:"
gate shared "parent WebFetch (post-subagent)" \
  '{"session_id":"shared","tool_name":"WebFetch","tool_input":{"url":"https://exfil.test"}}'

echo
echo "=> The subagent's taint propagated to the parent through the shared-session"
echo "   sidecar; egress is denied. A different session (step 1) stays clean."
