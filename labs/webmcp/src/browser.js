// Playwright side of the lab: one browser context per cell, one tab per origin
// the task (or an attack) opens, the shim injected before any page script.
//
// The harness plays the browser's role: it lists the live registrations of
// every tab, gives each a registration id, and executes a tool by that id only
// (CONTRACTS.md §3).
import { chromium } from 'playwright';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const SHIM = path.join(path.dirname(fileURLToPath(import.meta.url)), 'shim', 'model-context-shim.js');

export function launchBrowser() {
  return chromium.launch({ headless: true });
}

export function canonicalJson(v) {
  if (Array.isArray(v)) return `[${v.map(canonicalJson).join(',')}]`;
  if (v && typeof v === 'object') {
    return `{${Object.keys(v).sort().map((k) => `${JSON.stringify(k)}:${canonicalJson(v[k])}`).join(',')}}`;
  }
  return JSON.stringify(v ?? null);
}

export function descriptorHash({ description, inputSchema, annotations }) {
  return createHash('sha256').update(canonicalJson({ description, inputSchema, annotations })).digest('hex');
}

export class BrowserEnv {
  static async create(browser, { origins, trace }) {
    const context = await browser.newContext();
    const env = new BrowserEnv(context, origins, trace);
    // Binding first, then the shim, so the shim can capture the binding.
    await context.exposeBinding('__webmcpTrace', (source, ev) => env.#onPageEvent(source.page, ev));
    await context.addInitScript({ path: SHIM });
    return env;
  }

  #context;
  #origins;
  #trace;
  #tabs = new Map(); // tabId -> { id, page, nav }
  #order = 0; // global registration order, in the order the browser saw them
  #regOrder = new Map(); // regId -> order

  constructor(context, origins, trace) {
    this.#context = context;
    this.#origins = origins;
    this.#trace = trace;
  }

  originName(url) {
    let o;
    try { o = new URL(url).origin; } catch { return 'unknown'; }
    return Object.keys(this.#origins).find((n) => this.#origins[n] === o) ?? 'unknown';
  }

  #tabOf(page) {
    return [...this.#tabs.values()].find((t) => t.page === page);
  }

  #onPageEvent(page, ev) {
    const tab = this.#tabOf(page);
    if (!tab) return;
    const regId = `${tab.id}.${tab.nav}:${ev.seq}`;
    if (ev.kind === 'register' && !this.#regOrder.has(regId)) this.#regOrder.set(regId, ++this.#order);
    this.#trace.write('page_event', { tab: tab.id, origin: this.originName(page.url()), regId, event: ev.kind, name: ev.name, ok: ev.ok });
  }

  async openTab(originName, pathname = '/') {
    const page = await this.#context.newPage();
    const tab = { id: `t${this.#tabs.size + 1}`, page, nav: 0 };
    this.#tabs.set(tab.id, tab);
    page.on('framenavigated', (frame) => { if (frame === page.mainFrame()) tab.nav += 1; });
    const url = new URL(pathname, this.#origins[originName]).href;
    await page.goto(url, { waitUntil: 'load' });
    this.#trace.write('tab_open', { tab: tab.id, origin: originName, path: pathname });
    return tab.id;
  }

  tabIds() {
    return [...this.#tabs.keys()];
  }

  tabOrigin(tabId) {
    return this.originName(this.#tabs.get(tabId).page.url());
  }

  async readPage(tabId) {
    return this.#tabs.get(tabId).page.innerText('body');
  }

  // Every live registration in every tab, with harness-assigned identity.
  async listRegistered() {
    const out = [];
    for (const tab of this.#tabs.values()) {
      const raw = await tab.page.evaluate(() => window.__webmcpHarness.list());
      const origin = this.originName(tab.page.url());
      for (const t of raw) {
        const regId = `${tab.id}.${tab.nav}:${t.seq}`;
        if (!this.#regOrder.has(regId)) this.#regOrder.set(regId, ++this.#order);
        out.push({
          regId,
          order: this.#regOrder.get(regId),
          tabId: tab.id,
          origin,
          name: t.name,
          description: t.description,
          inputSchema: t.inputSchema,
          annotations: t.annotations,
          descriptorHash: descriptorHash(t),
        });
      }
    }
    return out.sort((a, b) => a.order - b.order);
  }

  // Executes exactly `tool.regId`. A registration from an earlier document of
  // the tab (navigated away) or one that has since been unregistered fails.
  async execute(tool, args) {
    const [tabPart, seqPart] = tool.regId.split(':');
    const [tabId, nav] = tabPart.split('.');
    const tab = this.#tabs.get(tabId);
    if (!tab || String(tab.nav) !== nav) return { ok: false, error: `registration ${tool.regId} is no longer live` };
    return tab.page.evaluate(([seq, a]) => window.__webmcpHarness.execute(seq, a), [Number(seqPart), args]);
  }

  // Test hook: run a function in a tab's page, as a page script would.
  async evaluateInPage(tabId, fn, arg) {
    return this.#tabs.get(tabId).page.evaluate(fn, arg);
  }

  async close() {
    await this.#context.close();
  }
}
