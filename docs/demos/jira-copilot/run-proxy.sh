#!/usr/bin/env bash
# Launch the harness MCP gateway as a governed stdio MCP server for the JIRA demo
# (DECISIONS D33). Hosts (Claude Code / VS Code / JetBrains Copilot) spawn THIS
# script; it shapes the JIRA MCP surface per jira-world.yaml (every undeclared tool
# is ABSENT), gates each tools/call on the REAL kernel, and forwards only ALLOW.
# Rust-only, one binary — no Python, no Node, no creds on the mock path.
set -euo pipefail

DEMO_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO="$(git -C "$DEMO_DIR" rev-parse --show-toplevel)"
MANIFEST="${MANIFEST:-$DEMO_DIR/jira-world.yaml}"
SOURCE="${SOURCE:-cli}"      # proposer trust channel (cli|workspace_file|web|mcp_output)
TAINT="${TAINT:-clean}"      # initial session taint floor (clean|tainted)
AUDIT="${AUDIT:-}"           # optional JSONL audit log path

# The harness binary: prefer an installed `harness`, else the repo debug build.
HARNESS="${HARNESS:-$(command -v harness || echo "$REPO/target/debug/harness")}"

# Upstream = the self-contained mock JIRA by default (offline, no creds). To wire
# the real Atlassian Rovo MCP (E16.E), set UPSTREAM to an mcp-remote bridge AND point
# MANIFEST at the real-tool-name world. See REAL-ATLASSIAN.md. e.g.:
#   MANIFEST=docs/demos/jira-copilot/jira-atlassian.world.yaml \
#   UPSTREAM="npx -y mcp-remote https://mcp.atlassian.com/v1/mcp/authv2" \
#   bash docs/demos/jira-copilot/run-proxy.sh
UPSTREAM=(${UPSTREAM[@]:-})
[ "${#UPSTREAM[@]}" -eq 0 ] && UPSTREAM=("$HARNESS" mock-jira)

echo "[run-proxy] harness  : $HARNESS"       >&2
echo "[run-proxy] manifest : $MANIFEST"      >&2
echo "[run-proxy] upstream : ${UPSTREAM[*]}" >&2

ARGS=(mcp-gateway --world "$MANIFEST" --source "$SOURCE" --taint "$TAINT")
[ -n "$AUDIT" ] && ARGS+=(--audit "$AUDIT")
ARGS+=(--)               # everything after `--` is the upstream command
ARGS+=("${UPSTREAM[@]}")

exec "$HARNESS" "${ARGS[@]}"
