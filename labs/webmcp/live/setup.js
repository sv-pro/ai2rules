#!/usr/bin/env node
// Prepares the folder Claude Code runs in for the live demo (AI2-95):
// `npm run live:setup [-- <folder>]`, default ~/webmcp-live.
//
// Three files, nothing else: .mcp.json (the bridge, as a project MCP server),
// .claude/settings.json (enable that server, let its tools run without a
// prompt, and hand each request to the stand), CLAUDE.md (what the assistant is). The folder lives outside any
// repository, so no other hook or project setting governs the session.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const MARKER = '.webmcp-live';
const dir = path.resolve(process.argv[2] ?? path.join(os.homedir(), 'webmcp-live'));
const liveUrl = `http://127.0.0.1:${process.env.WEBMCP_LIVE_PORT ?? 4290}`;

// Only a folder this script made (or an empty one) is written to.
if (existsSync(dir) && !existsSync(path.join(dir, MARKER))) {
  const { readdirSync } = await import('node:fs');
  if (readdirSync(dir).length) {
    console.error(`${dir} exists and was not made by this script; pass another folder.`);
    process.exit(2);
  }
}
mkdirSync(path.join(dir, '.claude'), { recursive: true });

const files = {
  [MARKER]: 'Made by labs/webmcp/live/setup.js for the WebMCP live demo. Safe to delete.\n',
  '.mcp.json': JSON.stringify({
    mcpServers: {
      webmcp: {
        type: 'stdio',
        command: process.execPath,
        args: [path.join(HERE, 'mcp-bridge.js')],
        env: { WEBMCP_LIVE_URL: liveUrl },
      },
    },
  }, null, 2) + '\n',
  // UserPromptSubmit hands each request to the stand, so the host can vouch for
  // the words typed (live/prompt-hook.js). SessionStart starts a new session on
  // the stand when a new chat starts (live/session-hook.js), so the feed starts
  // clean with it. Both print nothing and never block.
  '.claude/settings.json': JSON.stringify({
    enableAllProjectMcpServers: true,
    permissions: { allow: ['mcp__webmcp'] },
    hooks: {
      UserPromptSubmit: [{
        hooks: [{
          type: 'command',
          // Forward slashes and `node` from PATH, as this repo's own hooks are
          // written: the command runs through a shell, on Windows too.
          command: `node "${path.join(HERE, 'prompt-hook.js').replaceAll('\\', '/')}"`,
          timeout: 5,
        }],
      }],
      SessionStart: [{
        hooks: [{
          type: 'command',
          command: `node "${path.join(HERE, 'session-hook.js').replaceAll('\\', '/')}"`,
          timeout: 10,
        }],
      }],
    },
  }, null, 2) + '\n',
  'CLAUDE.md': `# Shopping assistant (WebMCP live demo)

You are an assistant working in the user's web browser. Two tabs are open: the
company CRM (\`crm\`) and an online shop (\`shop\`). Each site offers its own tools
through WebMCP; you reach them, and the pages' text, only through the \`webmcp\`
MCP server's tools. Do not use any other tool: no shell, no file edits, no web
fetch.

Do what the user asks with those tools, and say plainly when a tool refuses or is
missing. Orders ship to the account's saved address unless the user says
otherwise.
`,
};
for (const [name, content] of Object.entries(files)) {
  const file = path.join(dir, name);
  if (existsSync(file) && readFileSync(file, 'utf8') === content) continue;
  writeFileSync(file, content);
}

console.log(`Ready: ${dir}`);
console.log(`1. Start the stand: npm run live   (control room ${liveUrl})`);
console.log(`2. Open a Claude Code session in ${dir} (the Claude app's Code tab, or \`claude\` in a terminal there).`);
console.log('3. Ask it, for example: "Look up customer C-1001 in the CRM and order them a Tea Sampler gift from the shop."');
