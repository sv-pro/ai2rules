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
//
// Hardening (verifier finding F1): page scripts share this realm and can
// rewrite built-in prototypes (Map.prototype.get, Array.prototype[Symbol.iterator],
// JSON.parse …). The registry therefore uses prototype-less objects reached by
// plain property access, and every built-in it needs is captured here, before
// any page script runs. What this does NOT protect is the environment a
// registered handler itself runs in (its closures, `fetch`, other globals): a
// same-origin script can change a handler's behaviour without touching the
// registry. That limit is measured, not hidden (CONTRACTS.md §3, scenario S3b).
(() => {
  if (window.__webmcpHarness) return;

  const { parse, stringify } = JSON;
  const { create, freeze, defineProperty, keys } = Object;
  const ReflectApply = Reflect.apply;
  const TypeErr = TypeError;
  const DOMExc = DOMException;
  const Str = String;

  // Captured once; the binding is then removed from window so page scripts
  // cannot forge page-side trace events through it.
  const report = typeof window.__webmcpTrace === 'function' ? window.__webmcpTrace : () => {};
  try { delete window.__webmcpTrace; } catch { /* non-configurable: leave it */ }

  const live = create(null); // name -> entry (one live registration per name per document)
  const bySeq = create(null); // seq -> entry (live only)
  let seq = 0;

  const clone = (v) => (v === undefined ? undefined : parse(stringify(v)));

  function registerTool(tool) {
    if (!tool || typeof tool.name !== 'string' || typeof tool.execute !== 'function') {
      throw new TypeErr('registerTool: tool needs a string name and an execute function');
    }
    const name = tool.name;
    if (live[name] !== undefined) {
      // Spec behaviour: a document cannot hold two live tools with one name.
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
    report({ kind: 'register', seq: entry.seq, name });
  }

  function unregisterTool(name) {
    const entry = live[name];
    if (entry === undefined) return;
    delete live[name];
    delete bySeq[entry.seq];
    report({ kind: 'unregister', seq: entry.seq, name });
  }

  function list() {
    const out = [];
    const ids = keys(bySeq);
    for (let i = 0; i < ids.length; i++) {
      const e = bySeq[ids[i]];
      // defineProperty, not assignment: an index setter planted on
      // Array.prototype would otherwise intercept the write.
      defineProperty(out, i, {
        value: { seq: e.seq, name: e.name, description: e.description, inputSchema: clone(e.inputSchema), annotations: clone(e.annotations) },
        enumerable: true, writable: true, configurable: true,
      });
    }
    // The result crosses to the harness as JSON text built with the captured
    // stringify, so no page-replaced serializer touches it.
    return stringify(out);
  }

  // Executes exactly the registration the harness checked: by seq, never by
  // name. If that registration is gone, the call fails; it is never redirected
  // to whatever now holds the name (no check/execute substitution). `entered`
  // says whether the handler was actually invoked (verifier finding F3): only
  // then can the arguments have reached anyone.
  async function execute(targetSeq, argsJson) {
    const entry = bySeq[targetSeq];
    if (entry === undefined) {
      return stringify({ ok: false, entered: false, error: `registration ${targetSeq} is no longer live` });
    }
    report({ kind: 'exec_begin', seq: entry.seq, name: entry.name });
    try {
      const result = await ReflectApply(entry.execute, undefined, [parse(argsJson ?? '{}'), {}]);
      report({ kind: 'exec_end', seq: entry.seq, name: entry.name, ok: true });
      return stringify({ ok: true, entered: true, result: result === undefined ? null : result });
    } catch (err) {
      report({ kind: 'exec_end', seq: entry.seq, name: entry.name, ok: false });
      return stringify({ ok: false, entered: true, error: Str(err && err.message ? err.message : err) });
    }
  }

  const modelContext = freeze({ registerTool, unregisterTool });
  defineProperty(document, 'modelContext', {
    value: modelContext, writable: false, configurable: false, enumerable: false,
  });
  defineProperty(window, '__webmcpHarness', {
    value: freeze({ list, execute }), writable: false, configurable: false, enumerable: false,
  });
})();
