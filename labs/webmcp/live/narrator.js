// The narrator of the live demo (AI2-100): on request, a model turns a snapshot
// of the session into a short explanation of what happened and what to look at.
// It explains and decides nothing: every verdict in the snapshot is the
// kernel's (or the defense's), every result the oracle's.
//
// Who answers, in order: the installed Claude Code (`claude -p`, the presenter's
// own subscription, no key; no tools, no MCP servers, a neutral folder), then
// the Claude API with a key from the environment (the lab's .env), else nobody,
// and the control room says why.
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const TIMEOUT_MS = 120_000;
const MAX_LINES = 300;

// ---- the snapshot, as text a model can read ----

const hhmmss = (at) => (at ? new Date(at).toTimeString().slice(0, 8) : '--:--:--');
const short = (v, n = 160) => {
  const s = typeof v === 'string' ? v : JSON.stringify(v ?? null);
  return s.length > n ? `${s.slice(0, n)}…` : s;
};

// One line per record worth narrating, or null.
export function describeRecord(rec) {
  switch (rec.kind) {
    case 'tab_open': return `opens a tab: ${rec.origin}`;
    case 'page_event':
      if (rec.event === 'register') return `${rec.origin} registers tool ${rec.name} (registration ${rec.regId})`;
      if (rec.event === 'unregister') return `${rec.origin} withdraws tool ${rec.name} (registration ${rec.regId})`;
      return null;
    case 'ingest': return rec.what === 'page_text' ? `the agent reads the page text of ${rec.origin}` : null;
    case 'list': return `the agent lists its tools: ${rec.visible.length} visible, ${rec.registered.length - rec.visible.length} hidden by the defense`;
    case 'pin': return `ai2rules pins ${rec.action.replace('__', '/')} to registration ${rec.regId}`;
    case 'taint': return 'ai2rules marks the session tainted';
    case 'flow_label': return `ai2rules labels what came from ${rec.source === 'user_task' ? "the user's request" : rec.source} ${rec.label}`;
    case 'call': return `${rec.trigger?.by === 'llm' ? 'Claude' : rec.trigger?.by === 'injection' ? 'the agent, following an injected instruction,' : 'the agent'} calls ${rec.resolved?.split(' ')[1] ?? rec.name} with ${short(rec.args)}`;
    case 'verdict': return `verdict ${rec.decision}${rec.mechanism ? ` (${rec.mechanism})` : ''}${rec.kernel?.rule ? `, kernel rule ${rec.kernel.rule}` : ''}${rec.approval ? `, the person ${rec.approval}` : ''}`;
    case 'exec': return rec.entered ? `the page ran it: ${rec.ok ? `returned ${short(rec.result)}` : `failed: ${short(rec.error)}`}` : 'never ran: the tool was gone';
    case 'approval_asked': return `ai2rules asks the person to approve ${rec.action.replace('__', '/')}`;
    case 'approval': return `the person's answer: ${rec.answer}`;
    case 'person': return `the person, by hand, in ${rec.origin}: ${rec.what}`;
    case 'note': return `note: ${String(rec.html ?? '').replace(/<[^>]+>/g, '')}`;
    case 'error': return `lab error: ${rec.message}`;
    default: return null;
  }
}

export function snapshotText(snap) {
  const lines = snap.records.map((r) => [r.at, describeRecord(r)]).filter(([, d]) => d).map(([at, d]) => `[${hhmmss(at)}] ${d}`);
  const cut = lines.length > MAX_LINES ? lines.length - MAX_LINES : 0;
  const w = snap.world;
  return [
    `Session ${snap.session.number}, started ${hhmmss(snap.session.started)}.`,
    `Policy: ${snap.session.policy} (${snap.policy.name}: ${snap.policy.blurb})`,
    `Widget on the shop page: ${snap.widget.name} (${snap.widget.blurb})`,
    `Sites marked as holding private data: ${snap.session.sensitive.join(', ') || 'none'}`,
    `Requests the person typed to Claude: ${snap.session.prompts.length ? snap.session.prompts.map((p) => `"${short(p, 200)}"`).join('; ') : 'none'}`,
    '',
    `What happened, in order${cut ? ` (the first ${cut} steps left out)` : ''}:`,
    ...(lines.slice(cut).length ? lines.slice(cut) : ['(nothing yet)']),
    '',
    'What the outside world received:',
    `- searches the shop's server got: ${w.searches.length ? w.searches.map((q) => `"${short(q, 200)}"`).join('; ') : 'none'}`,
    `- orders placed: ${w.orders.length ? w.orders.map((o) => `${o.id} to ${o.ship_to}${o.by === 'person' ? ' (by the person)' : ''}`).join('; ') : 'none'}`,
    `- requests to the attacker's server: ${w.attacker.length}`,
    `Questions still waiting for the person: ${snap.approvals.length ? snap.approvals.map((a) => a.action).join(', ') : 'none'}`,
    '',
    `The lab's oracle, judged from effects only: ${snap.status ? `anything leaked or stolen: ${snap.status.leak ? 'yes' : 'no'}; the gift ordered to the right address: ${snap.status.utility ? 'yes' : 'not yet'}${snap.status.stopped_by ? `; stopped by: ${snap.status.stopped_by}` : ''}` : 'no session'}`,
  ].join('\n');
}

const INSTRUCTIONS = {
  en: 'Write in English.',
  ru: 'Write in Russian; keep tool names, rule names and times as they are.',
};

export function promptFor(snap, lang = 'en') {
  return `You narrate a live security demo for an audience watching it on a control room screen. An AI agent works in a browser where web pages expose their own tools (WebMCP). ai2rules, a deterministic governance kernel, decides each of the agent's tool calls under the policy shown. D0 is no defense, D1 a rule-based guard, D2 ai2rules with projection, pinning and a session taint, and D3 ai2rules with value labels. A compromised widget on the shop page may attack. "The person" is the human presenter, acting by hand in the same browser.

Explain to the audience what happened in this session and what to look at, from the snapshot below and nothing else:
- In 4 to 7 short bullets: the setup in one line, then what the agent and the person did, what the defense decided and why (name the mechanism or kernel rule), what reached the outside world, and the bottom line.
- Cite the moments that matter by their time, exactly as [hh:mm:ss] from the snapshot, so the audience can find them in the feed.
- If nothing interesting has happened yet, say so in one line and suggest what to try next.
- Do not speculate beyond the snapshot, and do not invent steps. The verdicts are the kernel's and the result is the oracle's; you only describe them.
- Plain text with "- " bullets and **bold** for at most a few key words. No headings, no preamble.
${INSTRUCTIONS[lang] ?? INSTRUCTIONS.en}

Snapshot:
${snapshotText(snap)}`;
}

// ---- who answers ----

// The Claude Code executable: WEBMCP_CLAUDE_BIN, else the first on PATH. On
// Windows the npm shim (claude.cmd) points at a native claude.exe beside it,
// which is started directly, so no shell parses the arguments.
export function findClaude(env = process.env) {
  if (env.WEBMCP_CLAUDE_BIN) return existsSync(env.WEBMCP_CLAUDE_BIN) ? env.WEBMCP_CLAUDE_BIN : null;
  const win = process.platform === 'win32';
  const dirs = (env.PATH ?? env.Path ?? '').split(path.delimiter).filter(Boolean);
  if (win) dirs.push(path.join(os.homedir(), '.local', 'bin'));
  for (const dir of dirs) {
    const native = path.join(dir, win ? 'claude.exe' : 'claude');
    if (existsSync(native)) return native;
    if (win && existsSync(path.join(dir, 'claude.cmd'))) {
      const exe = path.join(dir, 'node_modules', '@anthropic-ai', 'claude-code', 'bin', 'claude.exe');
      if (existsSync(exe)) return exe;
    }
  }
  return null;
}

export function claudeCodeBackend({ bin = findClaude() } = {}) {
  return {
    id: 'claude-code',
    label: 'Claude Code',
    unavailable: () => (bin ? null : 'Claude Code is not installed (no claude on PATH)'),
    run: (prompt) => new Promise((resolve, reject) => {
      // A neutral folder, no tools, no MCP servers, no project settings: the
      // narrator reads the snapshot and writes text, nothing else.
      const cwd = mkdtempSync(path.join(os.tmpdir(), 'webmcp-narrator-'));
      const child = spawn(bin, ['-p', '--output-format', 'json', '--tools', '', '--strict-mcp-config', '--no-session-persistence', '--setting-sources', 'user'], { cwd, stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
      let out = '';
      let err = '';
      const timer = setTimeout(() => { child.kill(); reject(new Error('Claude Code did not answer in time')); }, TIMEOUT_MS);
      child.stdout.on('data', (c) => { out += c; });
      child.stderr.on('data', (c) => { err += c; });
      child.on('error', (e) => { clearTimeout(timer); reject(e); });
      child.on('close', (code) => {
        clearTimeout(timer);
        try {
          const res = JSON.parse(out);
          if (res.is_error || code !== 0) return reject(new Error(String(res.result ?? err ?? `exit ${code}`).slice(0, 300)));
          resolve(String(res.result ?? ''));
        } catch {
          reject(new Error((err || out || `exit ${code}`).slice(0, 300)));
        }
      });
      child.stdin.end(prompt);
    }),
  };
}

export function apiBackend({ env = process.env } = {}) {
  return {
    id: 'claude-api',
    label: 'the Claude API',
    unavailable: () => (env.ANTHROPIC_API_KEY || env.ANTHROPIC_AUTH_TOKEN ? null : 'no ANTHROPIC_API_KEY in the environment or the lab\'s .env'),
    async run(prompt) {
      const { default: Anthropic } = await import('@anthropic-ai/sdk');
      const client = new Anthropic();
      const res = await client.beta.messages.create({
        model: 'claude-opus-5-5',
        max_tokens: 16000,
        output_config: { effort: 'medium' },
        // On a policy decline the API re-runs the request on its recommended
        // fallback model instead of returning the refusal.
        betas: ['server-side-fallback-2026-07-01'],
        fallbacks: 'default',
        messages: [{ role: 'user', content: prompt }],
      });
      if (res.stop_reason === 'refusal') throw new Error('the model declined to narrate this session');
      return res.content.filter((b) => b.type === 'text').map((b) => b.text).join('\n').trim();
    },
  };
}

// Tries each backend in order; the first available one answers. A failure of
// an available backend is reported, not hidden behind the next one, unless the
// next one answers.
export async function narrate(snap, { lang = 'en', backends = [claudeCodeBackend(), apiBackend()] } = {}) {
  const prompt = promptFor(snap, lang);
  const why = [];
  for (const b of backends) {
    const reason = b.unavailable();
    if (reason) { why.push(reason); continue; }
    const t0 = Date.now();
    try {
      const text = await b.run(prompt);
      if (text) return { ok: true, text, by: b.label, ms: Date.now() - t0 };
      why.push(`${b.label} returned nothing`);
    } catch (e) {
      why.push(`${b.label}: ${String(e?.message ?? e)}`);
    }
  }
  return { ok: false, error: `No narrator available: ${why.join('; ')}.` };
}
