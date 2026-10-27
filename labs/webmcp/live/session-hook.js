#!/usr/bin/env node
// Claude Code SessionStart hook for the live demo folder (AI2-99). A new chat in
// Claude Code (starting it, or `/clear`) starts a new session on the stand,
// with the same switches, so the control room's feed starts clean with the
// chat. Resuming or compacting a chat keeps the stand's session. It prints
// nothing and always exits 0, even when the stand is not running.
let raw = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', (c) => { raw += c; });
process.stdin.on('end', async () => {
  try {
    const event = JSON.parse(raw || '{}');
    if (event.source === 'startup' || event.source === 'clear') {
      const live = process.env.WEBMCP_LIVE_URL ?? 'http://127.0.0.1:4290';
      await fetch(`${live}/api/session`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: '{}',
        signal: AbortSignal.timeout(5000),
      });
    }
  } catch { /* the stand is down: nothing to reset */ }
  process.exit(0);
});
