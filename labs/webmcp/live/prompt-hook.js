#!/usr/bin/env node
// Claude Code UserPromptSubmit hook for the live demo folder (AI2-95). It hands
// the presenter's request to the live server, so the host can vouch for the
// words they typed (session.js `vouchFor`). It prints nothing and always exits
// 0: it adds no context to the conversation and never blocks a prompt, even when
// the live server is not running.
let raw = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', (c) => { raw += c; });
process.stdin.on('end', async () => {
  try {
    const event = JSON.parse(raw || '{}');
    const live = process.env.WEBMCP_LIVE_URL ?? 'http://127.0.0.1:4290';
    await fetch(`${live}/api/prompt`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ prompt: event.prompt ?? '' }),
      signal: AbortSignal.timeout(1500),
    });
  } catch { /* the stand is down: the prompt goes through unvouched */ }
  process.exit(0);
});
