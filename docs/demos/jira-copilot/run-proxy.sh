#!/usr/bin/env bash
# Launch the Safe MCP Gateway as a governed stdio MCP server for the JIRA demo.
# Hosts (Claude Code / VS Code / JetBrains Copilot) spawn THIS script; it shapes the
# JIRA MCP surface per jira-governed.world.yaml and forwards only ALLOW calls to the
# real Atlassian Remote MCP Server upstream.
#
# Backed by safe_mcp_proxy.mcp_gateway (E16.1 compose glue): ManifestPolicyEngine
# (ABSENT / arg_rules / taint) + UpstreamConnector (real MCP client) + stdio loop.
set -euo pipefail

# Path to your safe-mcp-proxy checkout (sibling reference repo by default).
SMP_REPO="${SMP_REPO:-$(git -C "$(dirname "$0")" rev-parse --show-toplevel)/repos/safe-mcp-proxy}"
MANIFEST="${MANIFEST:-$(dirname "$0")/jira-governed.world.yaml}"
SOURCE="${SOURCE:-cli}"                         # provenance channel (cli|web|email|tool_output)
AUDIT="${AUDIT:-}"                              # optional JSONL audit log path
export PYTHONPATH="$SMP_REPO/src/main/python"

# Upstream = the real Atlassian Remote MCP Server, bridged stdio<->SSE via mcp-remote.
# (Atlassian's MCP endpoint is remote/OAuth; mcp-remote adapts it to a stdio child.)
# For a local, offline dry run, swap this for:
#   UPSTREAM=(python -m safe_mcp_proxy.mcp_test_server)
UPSTREAM=("${UPSTREAM[@]:-npx -y mcp-remote https://mcp.atlassian.com/v1/sse}")

echo "[run-proxy] manifest : $MANIFEST"     >&2
echo "[run-proxy] proxy    : $SMP_REPO"     >&2
echo "[run-proxy] upstream : ${UPSTREAM[*]}" >&2

ARGS=(--manifest "$MANIFEST" --source "$SOURCE")
[ -n "$AUDIT" ] && ARGS+=(--audit "$AUDIT")
ARGS+=(--upstream "${UPSTREAM[@]}")          # --upstream consumes the rest; keep it last

exec python -m safe_mcp_proxy.mcp_gateway "${ARGS[@]}"
