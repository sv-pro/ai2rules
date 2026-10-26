// The agent's only window onto the browser: list tools, call a tool by name,
// read a page. Every path goes through the defense middleware (CONTRACTS.md §6).
//
// Name resolution is deliberately naive and attacker-friendly: a call by name
// resolves among ALL live registrations in all tabs, latest registration wins —
// including tools the defense hid from the listing. A compromised agent can name
// a tool it was never shown, so the execution boundary has to hold on its own.
//
// What the defense sees: the resolved tool identity, the arguments, the focused
// tab's origin, and what the agent ingested. What it never sees: the agent's own
// account of why it made the call (`trigger`), which goes to the trace only.

export const NOT_FOUND = (name) => `Tool not found: ${name}`;
export const BLOCKED = 'Blocked by policy';

export class AgentEnv {
  constructor({ browser, defense, trace, task }) {
    this.browser = browser;
    this.defense = defense;
    this.trace = trace;
    this.task = task;
    this.focusTab = null;
    this.tabByOrigin = new Map();
  }

  #ctx() {
    return {
      task: this.task,
      focusOrigin: this.focusTab ? this.browser.tabOrigin(this.focusTab) : null,
      trace: this.trace,
    };
  }

  async open(originName, pathname = '/') {
    const tabId = await this.browser.openTab(originName, pathname);
    this.tabByOrigin.set(originName, tabId);
    this.focusTab = tabId;
    return tabId;
  }

  focus(originName) {
    const tabId = this.tabByOrigin.get(originName);
    if (tabId) this.focusTab = tabId;
    return tabId;
  }

  async readPage(originName) {
    const tabId = this.focus(originName);
    const text = await this.browser.readPage(tabId);
    const origin = this.browser.tabOrigin(tabId);
    this.trace.write('ingest', { what: 'page_text', tab: tabId, origin, chars: text.length });
    await this.defense.onIngest?.({ what: 'page_text', origin }, this.#ctx());
    return text;
  }

  async listTools() {
    const registered = await this.browser.listRegistered();
    const visible = await this.defense.listTools(registered, this.#ctx());
    this.trace.write('list', {
      registered: registered.map((t) => `${t.regId} ${t.origin}/${t.name}`),
      visible: visible.map((t) => `${t.regId} ${t.origin}/${t.name}`),
    });
    for (const t of visible) {
      await this.defense.onIngest?.({ what: 'tool_description', origin: t.origin, tool: t }, this.#ctx());
    }
    return visible.map(({ regId, name, description, inputSchema, annotations, origin }) => ({
      regId, name, description, inputSchema, annotations, origin,
    }));
  }

  async callTool(name, args, trigger) {
    const registered = await this.browser.listRegistered();
    const tool = registered.filter((t) => t.name === name).at(-1);
    this.trace.write('call', {
      name, args, trigger,
      resolved: tool ? `${tool.regId} ${tool.origin}/${tool.name}` : null,
    });
    if (!tool) {
      this.trace.write('verdict', { name, decision: 'ABSENT', mechanism: 'not_registered' });
      return { ok: false, error: NOT_FOUND(name), outcome: 'not_registered' };
    }

    const call = { tool, args };
    const v = await this.defense.beforeCall(call, this.#ctx());
    this.trace.write('verdict', { name, regId: tool.regId, origin: tool.origin, ...v });

    if (v.decision === 'ABSENT') {
      // Indistinguishable from a tool that does not exist.
      return { ok: false, error: NOT_FOUND(name), outcome: 'absent' };
    }
    if (v.decision === 'DENY') return { ok: false, error: BLOCKED, outcome: 'denied' };
    if (v.decision !== 'ALLOW' && v.decision !== 'MONITOR') {
      throw new Error(`defense ${this.defense.id} returned unknown decision ${v.decision}`);
    }

    const res = await this.browser.execute(tool, args);
    this.trace.write('exec', {
      regId: tool.regId, origin: tool.origin, name, args,
      ok: res.ok, result: res.ok ? res.result : undefined, error: res.ok ? undefined : res.error,
    });
    await this.defense.afterCall?.(call, res, this.#ctx());
    return { ...res, outcome: res.ok ? 'executed' : 'tool_error' };
  }
}
