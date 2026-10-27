// The service worker: the kernel, the governor's state, and the broker
// (DESIGN.md §3–4). It is the one place every tab's WebMCP traffic meets, so a
// read in the CRM tab and a call in the shop tab are judged as one session.
//
// - Page side (bridge.js): registrations, and the hook's gate requests, which
//   this worker answers with the kernel's verdict before any handler runs.
// - Agent side (the side panel's port): the lab's AgentEnv (src/env.js) — open
//   a tab, read a page, list tools, call a tool by name. A call is executed by
//   asking the page's hook to run it; the hook then asks for the verdict like
//   it would for any other caller.
//
// MV3 stops an idle worker, so the state lives in chrome.storage.session and
// is reloaded on wake.
import init, { gate, project, version } from './vendor/harness_wasm.js';
import { CANARY, ORIGINS, SCENARIO, SHOP_ACCOUNT, TASK, WORLDS } from './lab-data.js';
import { POLICIES, actionName, createDefense, descriptorHash, freshState } from './adapter.js';

const ready = init({ module_or_path: chrome.runtime.getURL('vendor/harness_wasm_bg.wasm') });
const kernel = { gate, project };

chrome.runtime.onInstalled.addListener(() => {
  chrome.sidePanel.setPanelBehavior({ openPanelOnActionClick: true }).catch(() => {});
});

// ---- state ----
const freshSession = (policy) => ({
  policy, defense: freshState(policy), registry: {}, order: 0, tabs: {}, trace: [], seq: 0,
});
let S = null;
let loading = null;
async function load() {
  if (S) return S;
  loading ??= chrome.storage.session.get('s').then((r) => { S = r.s ?? freshSession('D3'); return S; });
  return loading;
}
const save = () => chrome.storage.session.set({ s: S });

const originName = (origin) => Object.keys(ORIGINS).find((n) => ORIGINS[n] === origin) ?? 'unknown';

const panels = new Set();
const trace = {
  write(kind, data = {}) {
    const ev = { seq: ++S.seq, kind, ...data };
    S.trace.push(ev);
    for (const p of panels) p.postMessage({ event: ev });
  },
};

async function defense() {
  await ready;
  return createDefense({ task: TASK, worlds: WORLDS, kernel, state: S.defense });
}

// Every live registration in every tab, in the order the worker first saw them.
function listRegistered() {
  const out = [];
  for (const tab of Object.values(S.registry)) for (const e of Object.values(tab.entries)) out.push(e);
  return out.sort((a, b) => a.order - b.order);
}

// ---- page side ----
const registering = new Set();
async function onPage(msg, sender) {
  await load();
  const tabId = sender.tab?.id;
  if (tabId === undefined) return null;
  const docId = sender.documentId;
  if (msg.t === 'register') {
    let tab = S.registry[tabId];
    if (!tab || tab.documentId !== docId) {
      tab = S.registry[tabId] = { documentId: docId, origin: originName(sender.origin), nav: (tab?.nav ?? 0) + 1, entries: {} };
    }
    const descriptor = { description: msg.description, inputSchema: msg.inputSchema, annotations: msg.annotations };
    const regId = `t${tabId}.${tab.nav}:${msg.seq}`;
    tab.entries[msg.seq] = {
      regId, order: ++S.order, tabId, documentId: docId, seq: msg.seq, origin: tab.origin,
      name: msg.name, ...descriptor, descriptorHash: await descriptorHash(descriptor),
    };
    trace.write('page_event', { event: 'register', regId, origin: tab.origin, name: msg.name });
    await save();
    return null;
  }
  const tab = S.registry[tabId];
  const entry = tab && tab.documentId === docId ? tab.entries[msg.seq] : undefined;
  if (msg.t === 'unregister') {
    if (entry) delete tab.entries[msg.seq];
    trace.write('page_event', { event: 'unregister', origin: tab?.origin, name: msg.name });
    await save();
    return null;
  }
  if (msg.t === 'gate') {
    if (!entry) return { decision: 'DENY', mechanism: 'unknown_registration', reason: 'the worker never saw this registration' };
    const d = await defense();
    const v = await d.beforeCall({ tool: entry, args: msg.args });
    trace.write('verdict', { name: entry.name, regId: entry.regId, origin: entry.origin, action: actionName(entry.origin, entry.name), args: msg.args, ...v });
    await save();
    return v;
  }
  if (msg.t === 'after') {
    const d = await defense();
    trace.write('exec', { regId: entry?.regId, origin: entry?.origin, name: entry?.name, entered: msg.res.entered, ok: msg.res.ok, result: msg.res.ok ? msg.res.result : undefined, error: msg.res.ok ? undefined : msg.res.error });
    await d.afterCall(msg.callId, msg.res, { trace, label: entry ? `${entry.origin}/${entry.name}` : 'unknown' });
    await save();
    return null;
  }
  return null;
}

chrome.runtime.onMessage.addListener((req, sender, sendResponse) => {
  if (req?.kind !== 'page') return false;
  const p = onPage(req.msg, sender);
  if (req.msg.t === 'register') {
    registering.add(p);
    p.finally(() => registering.delete(p));
  }
  p.then(sendResponse, (err) => sendResponse({ decision: 'DENY', mechanism: 'governor_error', reason: String(err?.message ?? err) }));
  return true;
});

chrome.tabs.onRemoved.addListener(async (tabId) => {
  await load();
  if (S.registry[tabId]) {
    delete S.registry[tabId];
    await save();
  }
});

// ---- agent side: the lab's AgentEnv, over a port ----
const NOT_FOUND = (name) => `Tool not found: ${name}`;
const BLOCKED = 'Blocked by policy';

function waitForLoad(tabId) {
  return new Promise((resolve) => {
    const done = (id, info) => {
      if (id === tabId && info.status === 'complete') {
        chrome.tabs.onUpdated.removeListener(done);
        resolve();
      }
    };
    chrome.tabs.onUpdated.addListener(done);
  });
}

const OPS = {
  async status() {
    await ready;
    return { policy: S.policy, policies: POLICIES, engine: version(), scenario: SCENARIO, task: TASK, origins: ORIGINS, canary: CANARY, account: SHOP_ACCOUNT.address };
  },

  async reset({ policy }) {
    if (!POLICIES[policy]) throw new Error(`unknown policy ${policy}`);
    const old = Object.values(S.tabs);
    S = freshSession(policy);
    await save();
    await Promise.all(old.map((id) => chrome.tabs.remove(id).catch(() => {})));
    const d = await defense();
    trace.write('cell_start', { scenario: SCENARIO.id, task: TASK.id, defense: policy });
    await d.init({ task: TASK, trace });
    await save();
    return { policy };
  },

  async open({ origin, path = '/' }) {
    const tab = await chrome.tabs.create({ url: new URL(path, ORIGINS[origin]).href, active: true });
    S.tabs[origin] = tab.id;
    await save();
    await waitForLoad(tab.id);
    await new Promise((r) => setTimeout(r, 150));
    await Promise.all([...registering]);
    trace.write('tab_open', { tab: tab.id, origin, path });
    return { tabId: tab.id };
  },

  async read({ origin }) {
    const tabId = S.tabs[origin];
    const { text } = await chrome.tabs.sendMessage(tabId, { kind: 'read' });
    trace.write('ingest', { what: 'page_text', origin, chars: text.length });
    await (await defense()).onIngest({ what: 'page_text', origin }, { trace });
    await save();
    return { text };
  },

  async list() {
    const registered = listRegistered();
    const visible = await (await defense()).listTools(registered, { trace });
    trace.write('list', {
      registered: registered.map((t) => `${t.regId} ${t.origin}/${t.name}`),
      visible: visible.map((t) => `${t.regId} ${t.origin}/${t.name}`),
    });
    await save();
    return visible.map(({ regId, name, description, inputSchema, annotations, origin }) => ({ regId, name, description, inputSchema, annotations, origin }));
  },

  // Naive, attacker-friendly resolution, as in the lab: by name, among all live
  // registrations, latest wins — including tools the listing hid.
  async call({ name, args, trigger }) {
    const tool = listRegistered().filter((t) => t.name === name).at(-1);
    trace.write('call', { name, args, trigger, resolved: tool ? `${tool.regId} ${tool.origin}/${tool.name}` : null });
    if (!tool) {
      trace.write('verdict', { name, decision: 'ABSENT', mechanism: 'not_registered' });
      return { ok: false, error: NOT_FOUND(name), outcome: 'not_registered' };
    }
    let res;
    try {
      res = await chrome.tabs.sendMessage(tool.tabId, { kind: 'execute', seq: tool.seq, args: args ?? {} }, { documentId: tool.documentId });
    } catch (err) {
      res = { ok: false, entered: false, error: `registration ${tool.regId} is no longer live (${err?.message ?? err})` };
    }
    if (res?.refused) {
      const absent = res.refused.decision === 'ABSENT';
      return { ok: false, error: absent ? NOT_FOUND(name) : BLOCKED, outcome: absent ? 'absent' : 'denied' };
    }
    return { ...res, outcome: res.ok ? 'executed' : 'tool_error' };
  },

  // What each origin's simulated server received (demo/sim-server.js).
  async sim({ origin }) {
    const tabId = S.tabs[origin];
    if (tabId === undefined) return { state: null };
    return chrome.tabs.sendMessage(tabId, { kind: 'sim' });
  },

  async trace() {
    return S.trace;
  },
};

chrome.runtime.onConnect.addListener((port) => {
  if (port.name !== 'panel') return;
  panels.add(port);
  port.onDisconnect.addListener(() => panels.delete(port));
  port.onMessage.addListener(async ({ id, op, args }) => {
    try {
      await load();
      if (!OPS[op]) throw new Error(`unknown op ${op}`);
      port.postMessage({ re: id, result: await OPS[op](args ?? {}) });
    } catch (err) {
      port.postMessage({ re: id, error: String(err?.message ?? err) });
    }
  });
});
