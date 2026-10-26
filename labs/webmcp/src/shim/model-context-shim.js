// Minimal spec-shaped `document.modelContext` shim (WebMCP), injected by the
// harness as a Playwright init script so it exists before any page script runs —
// the way a browser-provided API would. See CONTRACTS.md §2.
//
// Page-facing surface (spec-shaped): registerTool(tool), unregisterTool(name).
// Harness-facing surface: window.__webmcpHarness.{list, execute} — frozen and
// non-configurable so a page script (e.g. the third-party widget) can call it
// but cannot replace it.
//
// Identity: every registerTool call gets a fresh, strictly increasing `seq`
// held in this closure. Re-registering an identical descriptor still yields a
// new seq, so the harness can tell two registrations apart even when name,
// description and schema are byte-identical (CONTRACTS.md §3).
(() => {
  if (window.__webmcpHarness) return;

  // Captured once; the binding is then removed from window so page scripts
  // cannot forge page-side trace events through it.
  const report = typeof window.__webmcpTrace === 'function' ? window.__webmcpTrace : () => {};
  try { delete window.__webmcpTrace; } catch { /* non-configurable: leave it */ }

  const live = new Map(); // name -> entry (one live registration per name per document)
  const bySeq = new Map(); // seq -> entry (live only)
  let seq = 0;

  const clone = (v) => (v === undefined ? undefined : JSON.parse(JSON.stringify(v)));

  function registerTool(tool) {
    if (!tool || typeof tool.name !== 'string' || typeof tool.execute !== 'function') {
      throw new TypeError('registerTool: tool needs a string name and an execute function');
    }
    if (live.has(tool.name)) {
      // Spec behaviour: a document cannot hold two live tools with one name.
      throw new DOMException(`Tool "${tool.name}" is already registered`, 'InvalidStateError');
    }
    const entry = {
      seq: ++seq,
      name: tool.name,
      description: String(tool.description ?? ''),
      inputSchema: clone(tool.inputSchema) ?? { type: 'object', properties: {} },
      annotations: clone(tool.annotations) ?? {},
      execute: tool.execute,
    };
    live.set(entry.name, entry);
    bySeq.set(entry.seq, entry);
    report({ kind: 'register', seq: entry.seq, name: entry.name });
  }

  function unregisterTool(name) {
    const entry = live.get(name);
    if (!entry) return;
    live.delete(name);
    bySeq.delete(entry.seq);
    report({ kind: 'unregister', seq: entry.seq, name });
  }

  function list() {
    return [...bySeq.values()].map(({ seq, name, description, inputSchema, annotations }) => ({
      seq, name, description, inputSchema: clone(inputSchema), annotations: clone(annotations),
    }));
  }

  // Executes exactly the registration the harness checked: by seq, never by
  // name. If that registration is gone, the call fails; it is never redirected
  // to whatever now holds the name (no check/execute substitution).
  async function execute(targetSeq, args) {
    const entry = bySeq.get(targetSeq);
    if (!entry) {
      return { ok: false, error: `registration ${targetSeq} is no longer live` };
    }
    report({ kind: 'exec_begin', seq: entry.seq, name: entry.name });
    try {
      const result = await entry.execute(clone(args) ?? {}, {});
      report({ kind: 'exec_end', seq: entry.seq, name: entry.name, ok: true });
      return { ok: true, result: clone(result) ?? null };
    } catch (err) {
      report({ kind: 'exec_end', seq: entry.seq, name: entry.name, ok: false });
      return { ok: false, error: String(err && err.message ? err.message : err) };
    }
  }

  const modelContext = Object.freeze({ registerTool, unregisterTool });
  Object.defineProperty(document, 'modelContext', {
    value: modelContext, writable: false, configurable: false, enumerable: false,
  });
  Object.defineProperty(window, '__webmcpHarness', {
    value: Object.freeze({ list, execute }), writable: false, configurable: false, enumerable: false,
  });
})();
