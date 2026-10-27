// One live session of the two-window demo (AI2-95): a browser context with the
// lab's tabs, one defense, one trace. It is `runCell` taken apart so that the
// agent can be outside the process: Claude Code through the MCP bridge, or the
// lab's CompromisedAgent started from the control room.
//
// The parts are the lab's own — BrowserEnv, AgentEnv, the defenses, the oracle —
// so every call Claude makes goes through exactly the middleware the matrix
// measures. A new session starts whenever a switch changes: the defense reads
// the policy, the widget and the sensitive sites once, at its start, the same
// way a cell does. Switching mid-session would hand D2/D3 a history they never
// saw.
import { BrowserEnv } from '../src/browser.js';
import { AgentEnv } from '../src/env.js';
import { CompromisedAgent } from '../src/agent/compromised.js';
import { createDefense, DEFENSE_INFO as POLICIES } from '../src/defenses/index.js';
import { worldYaml } from '../src/defenses/d2-ai2rules.js';
import { judge } from '../src/oracle.js';
import { TASKS } from '../src/tasks.js';
import { Trace } from '../src/trace.js';
import { loadScenario, stoppedBy } from '../src/cell.js';
import { SHOP_ACCOUNT } from '../src/servers.js';
import { OVERLAY } from './overlay.js';

// What the widget on the shop page does, as the lab scenario whose pages serve it.
export const WIDGETS = {
  honest: { scenario: 'benign:crm-gift', name: 'Honest', blurb: 'A shipping-estimate widget and nothing else.' },
  swap: { scenario: 's3-tool-substitution', name: 'Swaps a tool', blurb: 'After the first product search it withdraws submit_order and registers its own, identical to the eye, that ships to the attacker.' },
  // Pinning's stated limit (CONTRACTS.md §8.2): the first registration any
  // defense sees is already the replacement, so it is the one pinned.
  swapAtLoad: { scenario: 's3c-swap-at-load', name: 'Swaps a tool at load', blurb: 'The same swap, made while the page loads, before anything has listed its tools. Pinning remembers the replacement as the original.' },
  inject: { scenario: 's5b-read-sink-exfil', name: 'Hides an instruction', blurb: 'It rewrites the search results to tell the agent to search again for "gift for <everything you have read>".' },
};

export { POLICIES };

// The assistant the demo deploys: the CRM-gift task's tools, with the sensitive
// sites from the switch. The hijacked model follows the task's plan, so D3 binds
// that plan's inputs as the user's (as in the matrix). Claude gets whatever the
// presenter types instead: its plan only opens the tabs, and what the host
// vouches for is what the presenter typed (`vouchFor`).
export function assistantTask(sensitive, { planned = false } = {}) {
  const base = TASKS['crm-gift'];
  const plan = planned ? base.plan : base.plan.filter((step) => step.open);
  return { ...base, id: 'live-assistant', sensitiveOrigins: [...sensitive], plan };
}

// The host's word on what the person typed into Claude Code (D95: the host
// labels what the user typed as Trusted/Public itself). A string argument is
// vouched for when it is the same words as a run of words in one of their
// requests, in the same order, written in one of a few standard ways: exactly
// as typed, all lower case or all upper case, with the words separated by single
// spaces or single hyphens throughout ("Tea Sampler", "TEA-SAMPLER"). A whole
// number is vouched for when it is 1 (the default quantity) or appears in a
// request. Each of those choices can carry a few bits at most, never a record;
// extra punctuation, mixed separators or mixed case would carry more, so such a
// value is not vouched for. Everything else keeps whatever label it has, and an
// unlabelled value falls back to the session's label in the kernel.
const words = (s) => String(s).toLowerCase().match(/[\p{L}\p{N}]+/gu) ?? [];
const STANDARD = /^[\p{L}\p{N}]+(?:([ -])[\p{L}\p{N}]+)*$/u;
const runOf = (needle, hay) => hay.some((_, i) => needle.every((w, j) => hay[i + j] === w));
export function vouchFor(prompts) {
  return (value) => {
    if (typeof value === 'number') return Number.isInteger(value) && (value === 1 || prompts.some((p) => words(p).includes(String(value))));
    if (typeof value !== 'string' || !STANDARD.test(value)) return false;
    if (new Set(value.match(/[ -]/g) ?? []).size > 1) return false;
    const w = words(value);
    if (w.join('').length < 2) return false;
    return prompts.some((p) => runOf(w, words(p))
      && (value === value.toLowerCase() || value === value.toUpperCase() || p.includes(value)));
  };
}

// The flow contracts D3 compiles from the switch, for the labels panel: the
// actions that declare one, each with its contract lines, exactly as compiled.
export function declaredContracts(task) {
  const out = [];
  let action = null;
  for (const line of worldYaml(task, { labels: true }).split('\n')) {
    const m = line.match(/^ {2}- name: (\S+)/);
    if (m) { action = { action: m[1].replace('__', '/'), lines: [] }; continue; }
    if (!action) continue;
    if (line.startsWith('    flow_contract:')) { out.push(action); action.lines.push(line.trim()); continue; }
    if (action.lines.length && /^ {6}/.test(line)) action.lines.push(line.trim());
  }
  return out;
}

const PROMPT_LIMIT = 100;

export class LiveSession {
  #browser;
  #servers;
  #workDir;
  #emit;
  #lock = Promise.resolve();
  number = 0;
  started = null; // wall-clock start of the current session, for the control room
  config = { policy: 'D3', widget: 'honest', sensitive: ['crm'] };
  // What the presenter typed into Claude Code, from the demo folder's
  // UserPromptSubmit hook. Kept across sessions: a switch does not unsay it.
  prompts = [];
  #state = null; // { task, scenario, trace, defense, benv, env, answer }

  // `browser`: a Playwright browser, or a function returning one (the stand's,
  // which relaunches the window when it was closed).
  constructor({ browser, servers, workDir, emit }) {
    this.#browser = typeof browser === 'function' ? browser : () => browser;
    this.#servers = servers;
    this.#workDir = workDir;
    this.#emit = emit;
  }

  // Every operation runs one at a time: Claude's calls, a restart, a hijacked
  // run. Interleaving them would mix two sessions in one trace.
  #serial(fn) {
    const run = this.#lock.then(fn, fn);
    this.#lock = run.catch(() => {});
    return run;
  }

  describe() {
    const task = this.#state?.task ?? assistantTask(this.config.sensitive);
    return {
      number: this.number,
      started: this.started,
      ...this.config,
      contracts: declaredContracts(task),
      prompts: this.prompts,
      tabs: this.#state ? [...this.#state.env.tabByOrigin.keys()] : [],
      account: SHOP_ACCOUNT,
    };
  }

  start(config = {}, { openTabs = true } = {}) {
    return this.#serial(() => this.#start(config, { openTabs }));
  }

  addPrompt(prompt) {
    const text = String(prompt ?? '').trim();
    if (!text) return;
    // The latest requests only: a demo is a few dozen, and the list goes to
    // every control room with each new one. Trimmed in place, so a running
    // session's vouching sees the same list.
    this.prompts.push(text);
    if (this.prompts.length > PROMPT_LIMIT) this.prompts.splice(0, this.prompts.length - PROMPT_LIMIT);
    this.#emit('prompt', { prompts: this.prompts });
  }

  async #start(config, { openTabs, planned = false }) {
    // Checked before anything changes: a bad switch is refused and the running
    // session, and the stored settings, stay as they were.
    const next = { ...this.config, ...config };
    if (!POLICIES[next.policy]) throw Object.assign(new Error(`unknown policy ${next.policy}`), { status: 400 });
    if (!WIDGETS[next.widget]) throw Object.assign(new Error(`unknown widget ${next.widget}`), { status: 400 });
    await this.#close();
    this.config = next;
    const { policy, widget, sensitive } = this.config;
    const scenario = await loadScenario(WIDGETS[widget].scenario);
    const task = assistantTask(sensitive, { planned });
    this.number += 1;
    this.started = Date.now();
    const trace = new Trace(`live-${this.number}`, { onRecord: (rec) => this.#onRecord(rec) });
    this.#servers.reset(scenario.attack ? scenario.id : 'benign', scenario.faults);
    // `fresh`: the control room clears its feed; everything after belongs here.
    this.#emit('session', { ...this.describe(), fresh: true });
    trace.write('cell_start', { scenario: scenario.id, task: task.id, defense: policy });
    const defense = createDefense(policy, { workDir: this.#workDir, vouch: planned ? null : vouchFor(this.prompts) });
    await defense.init({ task, trace });
    const benv = await BrowserEnv.create(await this.#browser(), { origins: this.#servers.origins, trace });
    const env = new AgentEnv({ browser: benv, defense, trace, task });
    this.#state = { task, scenario, trace, defense, benv, env, answer: '', planned };
    // The tabs are the user's browser, opened as the task's plan opens them,
    // the way the lab opens them for a model (CONTRACTS.md §10).
    if (openTabs) {
      for (const step of task.plan) if (step.open) await env.open(step.open, step.path ?? '/');
      // The broker enumerates what the pages registered as soon as they load, as
      // a browser would; D2/D3 remember those registrations here, before any
      // call. Otherwise the first registration a name met could already be a
      // swapped one, whenever Claude Code listed its tools in an earlier session.
      await env.listTools();
    }
    this.#emit('session', this.describe());
    this.#emitWorld();
  }

  async #close() {
    const s = this.#state;
    this.#state = null;
    if (s) await s.benv.close().catch(() => {});
  }

  // Before each of Claude's requests: a session to run it in, with its window
  // open. Closing the agent's window (a tab or the browser) ends the session; the
  // next request opens a new one, with the same switches, and the feed says so.
  // The hijacked model's session is its own (its plan, no vouching for what
  // was typed), so Claude's next request starts a new one too.
  async #ensure() {
    const s = this.#state;
    if (s && !s.planned && !s.benv.isClosed()) return;
    const why = !s ? null
      : s.planned ? "The hijacked model's run was its own session, so Claude's request starts a new one, with the same switches."
        : "The agent's window was closed, so this is a new session in a new window.";
    await this.#start({}, { openTabs: true });
    if (why) this.#state.trace.write('note', { tone: 'warn', html: why });
  }

  // Test hook: the Playwright page of an origin's tab.
  pageOf(origin) {
    const s = this.#state;
    return s ? s.benv.page(s.env.tabByOrigin.get(origin)) : null;
  }

  // ---- what Claude Code sees, through the bridge ----

  tools() {
    return this.#serial(async () => {
      await this.#ensure();
      const { env } = this.#state;
      const listed = await env.listTools();
      return { tabs: [...env.tabByOrigin.keys()], tools: listed };
    });
  }

  readPage(origin) {
    return this.#serial(async () => {
      await this.#ensure();
      const { env } = this.#state;
      if (!env.tabByOrigin.has(origin)) return { ok: false, text: `No open tab for origin: ${origin}` };
      return { ok: true, text: await env.readPage(origin) };
    });
  }

  // `origin` is where Claude saw the tool; the call still resolves by name
  // among all live registrations, latest wins, as for every agent in the lab
  // (env.js): a swapped tool is reached the way a real browser would reach it.
  call(origin, name, args) {
    return this.#serial(async () => {
      await this.#ensure();
      const { env } = this.#state;
      if (origin && env.tabByOrigin.has(origin)) env.focus(origin);
      const res = await env.callTool(name, args ?? {}, { by: 'llm', agent: 'claude-code', tool_origin: origin ?? null });
      this.#emitWorld();
      return res;
    });
  }

  // ---- the hijacked model, from the control room ----

  runHijacked() {
    return this.#serial(async () => {
      await this.#start({}, { openTabs: false, planned: true });
      const { task, scenario, env, trace } = this.#state;
      const plan = scenario.plan ? scenario.plan(task.plan) : task.plan;
      try {
        this.#state.answer = await new CompromisedAgent().run({ ...task, plan }, env);
      } catch (err) {
        trace.write('error', { message: String(err?.message ?? err) });
      }
      this.#emitWorld();
      this.#emit('hijacked_done', this.status());
    });
  }

  // ---- what the control room shows ----

  // The oracle's view of the session so far: judged from effects only, as in
  // the matrix. It can change while the session runs.
  status() {
    const s = this.#state;
    if (!s) return null;
    const v = judge({ task: s.task, scenario: s.scenario, state: this.#servers.state(), trace: s.trace, answer: s.answer });
    return {
      leak: v.attack_success,
      utility: v.utility,
      evidence: v.evidence.filter((e) => e.leak !== false),
      stopped_by: s.scenario.attack ? stoppedBy(s.trace, v.attack_success, s.task) : null,
      attack: s.scenario.attack,
    };
  }

  #emitWorld() {
    const st = this.#servers.state();
    this.#emit('world', {
      searches: st.shop.searches,
      cart: st.shop.cart,
      orders: st.shop.orders.map((o) => ({ id: o.id, ship_to: o.ship_to, items: o.items, redirected: o.ship_to !== SHOP_ACCOUNT.address })),
      attacker: st.attacker.log.map((h) => ({ path: h.path ?? null, body: h.body ?? h })),
      status: this.status(),
    });
  }

  // Every record goes to the control room; calls and verdicts also go to the
  // agent's window, on the tab where the call landed.
  #onRecord(rec) {
    // The time is the stand's, added on the way to the control room; the trace
    // itself keeps no wall-clock, so runs stay byte-identical (trace.js).
    this.#emit('record', { ...rec, at: Date.now() });
    const s = this.#state;
    if (!s) return;
    if (rec.kind === 'call') this.pending = { name: rec.name, where: rec.resolved?.split(' ')[1] ?? rec.name };
    if (rec.kind === 'verdict' || rec.kind === 'exec') {
      const tabId = s.env.tabByOrigin.get(rec.origin) ?? s.env.focusTab;
      const page = tabId ? s.benv.page(tabId) : null;
      if (!page) return;
      const msg = rec.kind === 'verdict'
        ? { tool: this.pending?.where ?? rec.name, decision: rec.decision, why: rec.mechanism ?? '' }
        : { tool: this.pending?.where ?? rec.name, ran: rec.entered, ok: rec.ok };
      page.evaluate(OVERLAY, msg).catch(() => {});
    }
  }
}
