# JetBrains — GitHub Copilot MCP config

JetBrains GitHub Copilot (agent mode) supports MCP servers. The config format
mirrors VS Code / Claude Code (a JSON map of servers launched over stdio).

**Where to set it** (confirm against your plugin version — the path moves between
releases):

- **Settings → Languages & Frameworks → GitHub Copilot → Model Context Protocol**,
  then *Edit `mcp.json`* (some versions: a `mcp.json` under the IDE config dir, or a
  per-project `.idea` location).

**Config** (same proxy, same manifest as the other hosts):

```json
{
  "servers": {
    "jira-governed": {
      "type": "stdio",
      "command": "bash",
      "args": ["/ABS/PATH/TO/ai2rules/docs/demos/jira-copilot/run-proxy.sh"]
    }
  }
}
```

> If your JetBrains plugin version expects the `mcpServers` key (the Claude-style
> map) instead of `servers`, use `hosts/claude-code.mcp.json` as the shape. Either
> way it spawns the *same* `run-proxy.sh`, so one governed proxy serves all three
> hosts.

**Verify:** open Copilot Chat → agent mode → the tool list should show only the
read tools + `jira_add_comment`; the destructive JIRA tools should be absent.
