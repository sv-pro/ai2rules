// The interception point (DESIGN.md §2). Runs in the page's MAIN world at
// document_start, before any page script, and owns `document.modelContext`.
//
// Every tool a page registers is stored with its handler; the handler is only
// ever invoked through `governed()`, which first asks the kernel (in the
// extension's service worker) for a verdict on this exact registration and
// these exact arguments. Whoever asks for the execution (the extension's own
// agent today, a browser-provided agent once WebMCP ships natively), the page
// handler does not run without an ALLOW.
//
// If a browser already provides `document.modelContext`, its `registerTool` is
// wrapped instead: the browser receives a descriptor whose `execute` is
// `governed()`, so its agent reaches the handler through the same gate.
//
// The channel to the extension. The hook shares its realm with page scripts,
// so anything on `window` (postMessage, events) can be observed and forged by
// the page. Instead, at document_start, the hook hands the ISOLATED-world
// bridge a detached DOM node through one synchronous event, and the two talk
// only through events dispatched on that node. No page script can reach a
// detached node it was never given, and none has run yet when it is handed
// over. Built-ins are captured here first, as in the lab's shim (verifier
// finding F1). What this cannot protect is the environment a handler itself
// runs in (CONTRACTS.md §3, scenario S3b): the same limit as the lab's.
(() => {
  const { parse, stringify } = JSON;
  const { create, freeze, defineProperty } = Object;
  const ReflectApply = Reflect.apply;
  const TypeErr = TypeError;
  const DOMExc = DOMException;
  const Str = String;
  const Prom = Promise;
  const CustomEv = CustomEvent;
  const MouseEv = MouseEvent;
  const dispatch = EventTarget.prototype.dispatchEvent;
  const listen = EventTarget.prototype.addEventListener;
  const detailOf = Object.getOwnPropertyDescriptor(CustomEvent.prototype, 'detail').get;
  const createElement = Document.prototype.createElement;

  const TO_BRIDGE = 'ai2rules:to-bridge';
  const TO_HOOK = 'ai2rules:to-hook';

  // ---- the channel ----
  const node = ReflectApply(createElement, document, ['ai2rules-channel']);
  let connected = false;
  const waiting = create(null); // request id -> resolve
  let nextId = 0;

  const send = (msg) => ReflectApply(dispatch, node, [new CustomEv(TO_BRIDGE, { detail: stringify(msg) })]);
  const ask = (msg) => new Prom((resolve) => {
    const id = ++nextId;
    waiting[id] = resolve;
    send({ ...msg, id });
  });

  ReflectApply(listen, node, [TO_HOOK, (ev) => {
    let msg;
    try { msg = parse(ReflectApply(detailOf, ev, [])); } catch { return; }
    if (msg.t === 'connected') connected = true;
    else if (msg.re !== undefined && waiting[msg.re]) {
      const resolve = waiting[msg.re];
      delete waiting[msg.re];
      resolve(msg);
    } else if (msg.t === 'execute') {
      const entry = bySeq[msg.seq];
      const done = (res) => send({ t: 'executed', re: msg.id, res });
      if (entry === undefined) done({ ok: false, entered: false, error: `registration ${msg.seq} is no longer live` });
      else governed(entry, msg.args ?? {}).then(done);
    }
  }]);
  // The handshake: synchronous, so the bridge (already listening) holds the
  // node before this script returns, and before any page script exists.
  ReflectApply(dispatch, window, [new MouseEv('ai2rules:handshake', { relatedTarget: node })]);

  // ---- the registry (the lab's shim, CONTRACTS.md §2–3) ----
  const live = create(null); // name -> entry
  const bySeq = create(null); // seq -> entry
  let seq = 0;
  const clone = (v) => (v === undefined ? undefined : parse(stringify(v)));

  // The one road to a page handler. `entered` says whether the handler was
  // invoked (verifier finding F3): only then can the arguments have reached
  // anyone. A refusal never enters it.
  async function governed(entry, args, options = {}) {
    if (!connected) {
      return { ok: false, entered: false, refused: { decision: 'DENY', mechanism: 'no_governor', reason: 'the ai2rules extension is not connected; refusing by default' } };
    }
    const argsJson = stringify(args ?? {});
    const v = await ask({ t: 'gate', seq: entry.seq, name: entry.name, args: parse(argsJson) });
    if (v.verdict?.decision !== 'ALLOW') return { ok: false, entered: false, refused: v.verdict ?? { decision: 'DENY', mechanism: 'no_verdict' } };
    let res;
    try {
      const result = await ReflectApply(entry.execute, undefined, [parse(argsJson), options]);
      res = { ok: true, entered: true, result: result === undefined ? null : clone(result) };
    } catch (err) {
      res = { ok: false, entered: true, error: Str(err && err.message ? err.message : err) };
    }
    send({ t: 'after', callId: v.verdict.callId, res });
    return res;
  }

  function registerTool(tool) {
    if (!tool || typeof tool.name !== 'string' || typeof tool.execute !== 'function') {
      throw new TypeErr('registerTool: tool needs a string name and an execute function');
    }
    const name = tool.name;
    if (live[name] !== undefined) {
      throw new DOMExc(`Tool "${name}" is already registered`, 'InvalidStateError');
    }
    const entry = create(null);
    entry.seq = ++seq;
    entry.name = name;
    entry.description = Str(tool.description ?? '');
    entry.inputSchema = clone(tool.inputSchema) ?? { type: 'object', properties: {} };
    entry.annotations = clone(tool.annotations) ?? {};
    entry.execute = tool.execute; // read once; later changes to `tool` do not matter
    freeze(entry);
    live[name] = entry;
    bySeq[entry.seq] = entry;
    send({ t: 'register', seq: entry.seq, name, description: entry.description, inputSchema: entry.inputSchema, annotations: entry.annotations });
    return entry;
  }

  function unregisterTool(name) {
    const entry = live[name];
    if (entry === undefined) return;
    delete live[name];
    delete bySeq[entry.seq];
    send({ t: 'unregister', seq: entry.seq, name });
  }

  const native = document.modelContext;
  if (native && typeof native.registerTool === 'function') {
    // A browser-provided WebMCP (Chrome's early preview): shadow its
    // `registerTool` on the instance. The browser receives the page's
    // descriptor with `execute` replaced by `governed()`, so its own
    // `executeTool()`, and any agent that calls through it, meets the gate.
    // Everything else (`getTools`, `executeTool`, `toolchange`, unregistering
    // by AbortSignal) stays the browser's. Untested in the spike: no browser
    // the test runs in ships it yet (DESIGN.md §2).
    const nativeRegister = native.registerTool;
    defineProperty(native, 'registerTool', {
      value: function registerToolGoverned(tool, options) {
        const entry = registerTool(tool);
        const signal = options?.signal;
        if (signal) ReflectApply(listen, signal, ['abort', () => unregisterTool(entry.name), { once: true }]);
        const execute = async (input, opts) => {
          const res = await governed(entry, input, opts ?? {});
          if (res.refused) throw new Error(res.refused.decision === 'ABSENT' ? `Tool not found: ${entry.name}` : 'Blocked by policy');
          if (!res.ok) throw new Error(res.error);
          return res.result;
        };
        return ReflectApply(nativeRegister, native, [{ ...tool, execute }, options]);
      },
      writable: false, configurable: false, enumerable: false,
    });
  } else {
    // No WebMCP in this browser: provide the lab's spec-shaped surface.
    defineProperty(document, 'modelContext', {
      value: freeze({ registerTool(tool) { registerTool(tool); }, unregisterTool }), writable: false, configurable: false, enumerable: false,
    });
  }
})();
