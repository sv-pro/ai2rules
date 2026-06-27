#!/usr/bin/env bash
# Launch the Safe MCP Proxy as a governed stdio MCP server for the JIRA demo.
# Hosts (Claude Code / VS Code / JetBrains Copilot) spawn THIS script; it shapes
# the JIRA MCP surface per jira-governed.world.yaml and forwards allowed calls to
# the real Atlassian Remote MCP Server upstream.
#
# ⚠ SCAFFOLD — not yet a one-command launch. The pieces exist in safe-mcp-proxy
#   but are not composed into a single entry point yet (PLAN E16.1):
#     - mcp_server.py        host-facing stdio MCP server (currently mock registry)
#     - mcp_upstream.py      UpstreamConnector — real MCP client to an upstream server
#     - atlassian/policy.py  ManifestPolicyEngine — ABSENT / arg_rules / taint
#   The integration task: a governed stdio server that (1) connects upstream via
#   UpstreamConnector, (2) ABSENT-filters tools/list against the manifest allowlist,
#   (3) routes tools/call through ManifestPolicyEngine, then forwards ALLOW upstream.
set -euo pipefail

# Path to your safe-mcp-proxy checkout (sibling reference repo by default).
SMP_REPO="${SMP_REPO:-$(git -C "$(dirname "$0")" rev-parse --show-toplevel)/repos/safe-mcp-proxy}"
MANIFEST="${MANIFEST:-$(dirname "$0")/jira-governed.world.yaml}"
export PYTHONPATH="$SMP_REPO/src/main/python"

# Upstream = the real Atlassian Remote MCP Server, bridged stdio<->SSE via mcp-remote.
# (Atlassian's MCP endpoint is remote/OAuth; `mcp-remote` adapts it to a stdio child.)
UPSTREAM=(npx -y mcp-remote https://mcp.atlassian.com/v1/sse)

echo "[run-proxy] manifest : $MANIFEST"            >&2
echo "[run-proxy] proxy    : $SMP_REPO"            >&2
echo "[run-proxy] upstream : ${UPSTREAM[*]}"       >&2

# TODO(E16.1): replace the line below with the composed governed-stdio entry point,
# e.g.  exec python -m safe_mcp_proxy.mcp_server --manifest "$MANIFEST" \
#            --engine atlassian --upstream "${UPSTREAM[@]}"
# Until that flag set lands, run the policy surface against the built-in registry
# for a dry run of ABSENT/allow decisions:
exec python -m safe_mcp_proxy.mcp_server --mode interactive
