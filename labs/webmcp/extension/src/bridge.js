// The bridge (DESIGN.md §2): the hook's only link to the extension. Runs in the
// ISOLATED world at document_start, before the hook, so it is listening when
// the hook hands over its private channel node. It decides nothing; it carries
// messages between the page's hook and the service worker, where the kernel
// lives, and answers the broker's reads of this page.
(() => {
  const TO_BRIDGE = 'ai2rules:to-bridge';
  const TO_HOOK = 'ai2rules:to-hook';
  let node = null;
  const executions = new Map(); // request id -> sendResponse
  let nextId = 0;

  const toHook = (msg) => node.dispatchEvent(new CustomEvent(TO_HOOK, { detail: JSON.stringify(msg) }));
  const toWorker = (msg) => chrome.runtime.sendMessage({ kind: 'page', msg });

  window.addEventListener('ai2rules:handshake', (ev) => {
    // Only the first handshake: it happens before any page script runs, so a
    // later one can only be a page script imitating it.
    if (node || !(ev.relatedTarget instanceof Element)) return;
    node = ev.relatedTarget;
    node.addEventListener(TO_BRIDGE, async (e) => {
      let msg;
      try { msg = JSON.parse(e.detail); } catch { return; }
      if (msg.t === 'gate') {
        let verdict;
        try {
          verdict = await toWorker(msg);
        } catch (err) {
          verdict = { decision: 'DENY', mechanism: 'no_governor', reason: `the governor is unreachable: ${err?.message ?? err}` };
        }
        toHook({ re: msg.id, verdict });
      } else if (msg.t === 'executed') {
        executions.get(msg.re)?.(msg.res);
        executions.delete(msg.re);
      } else {
        toWorker(msg).catch(() => {});
      }
    });
    toHook({ t: 'connected' });
  }, { capture: true });

  // The broker's requests for this page (from the service worker).
  chrome.runtime.onMessage.addListener((req, _sender, sendResponse) => {
    if (req?.kind === 'execute') {
      if (!node) {
        sendResponse({ ok: false, entered: false, error: 'this page has no WebMCP hook' });
        return false;
      }
      const id = ++nextId;
      executions.set(id, sendResponse);
      toHook({ t: 'execute', id, seq: req.seq, args: req.args });
      return true; // the response is async
    }
    if (req?.kind === 'read') {
      sendResponse({ text: document.body?.innerText ?? '' });
      return false;
    }
    if (req?.kind === 'sim') {
      // The simulated server's record of what it received (demo/sim-server.js).
      sendResponse({ state: JSON.parse(sessionStorage.getItem('ai2rules-sim') ?? 'null') });
      return false;
    }
    return false;
  });
})();
