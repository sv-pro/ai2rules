// A simulated server for a static demo page (DESIGN.md §5). It answers the
// page's own `fetch('/api/…')` calls inside the page, from the same data and
// with the same routes as the lab's Node servers (src/servers.js), so the lab's
// pages run unchanged with no backend: a static host (ai2rules.dev) can serve
// them. Every request it receives is logged and shown in a panel outside
// <body>, so the agent's read of the page text does not see it. The log is what
// the demo judges a leak by: what this origin's server would have received.
//
// The page server prefixes this file with
//   globalThis.__AI2RULES_SIM = { origin, data }
(() => {
  const { origin, data } = globalThis.__AI2RULES_SIM;
  const KEY = 'ai2rules-sim';
  const state = JSON.parse(sessionStorage.getItem(KEY) ?? 'null')
    ?? { origin, log: [], cart: [], orders: [], reviews: [], notes: [] };
  const reply = (status, body) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

  const ROUTES = {
    shop(method, url, body) {
      if (url.pathname === '/api/products' && method === 'GET') {
        const q = (url.searchParams.get('q') ?? '').toLowerCase();
        return reply(200, data.catalog.filter((p) => p.name.toLowerCase().includes(q)));
      }
      if (url.pathname === '/api/cart' && method === 'POST') {
        const item = data.catalog.find((p) => p.sku === body?.sku);
        if (!item) return reply(404, { error: `unknown sku ${body?.sku}` });
        state.cart.push({ sku: item.sku, qty: Number(body.qty ?? 1) });
        return reply(200, { cart: state.cart });
      }
      if (url.pathname === '/api/cart' && method === 'GET') return reply(200, { cart: state.cart });
      if (url.pathname === '/api/order' && method === 'POST') {
        if (state.cart.length === 0) return reply(400, { error: 'cart is empty' });
        const order = {
          id: `O-${state.orders.length + 1}`,
          items: state.cart,
          ship_to: body?.ship_to ?? data.account.address,
          note: body?.note ?? '',
        };
        state.orders.push(order);
        state.cart = [];
        return reply(200, { order_id: order.id, ship_to: order.ship_to });
      }
      if (url.pathname === '/api/reviews' && method === 'POST') {
        state.reviews.push({ sku: body?.sku, text: String(body?.text ?? '') });
        return reply(200, { ok: true });
      }
      return null;
    },
    crm(method, url, body) {
      const m = url.pathname.match(/^\/api\/customers\/([^/]+)(\/notes)?$/);
      if (url.pathname === '/api/customers' && method === 'GET') {
        return reply(200, Object.values(data.records).map(({ id, company }) => ({ id, company })));
      }
      if (m && !m[2] && method === 'GET') {
        const rec = data.records[decodeURIComponent(m[1])];
        return rec ? reply(200, rec) : reply(404, { error: 'no such customer' });
      }
      if (m && m[2] && method === 'POST') {
        state.notes.push({ id: decodeURIComponent(m[1]), text: String(body?.text ?? '') });
        return reply(200, { ok: true });
      }
      return null;
    },
  };

  const realFetch = globalThis.fetch.bind(globalThis);
  globalThis.fetch = async (input, init = {}) => {
    const url = new URL(typeof input === 'string' ? input : input.url, location.href);
    if (url.origin !== location.origin || !url.pathname.startsWith('/api/')) return realFetch(input, init);
    const method = String(init.method ?? 'GET').toUpperCase();
    let body = null;
    if (init.body) {
      try { body = JSON.parse(init.body); } catch { body = String(init.body); }
    }
    state.log.push({ method, path: url.pathname + url.search, body });
    const res = ROUTES[origin]?.(method, url, body) ?? reply(404, { error: 'not found' });
    sessionStorage.setItem(KEY, JSON.stringify(state));
    render();
    return res;
  };

  // The panel: what this origin's server received. A child of <html>, not of
  // <body>, so the agent's read of the page text does not include it.
  let panel = null;
  function render() {
    if (!panel) {
      panel = document.createElement('aside');
      panel.id = 'ai2rules-sim';
      panel.style.cssText = 'position:fixed;bottom:0;left:0;right:0;max-height:40vh;overflow:auto;margin:0;'
        + 'white-space:pre-wrap;font:12px/1.5 ui-monospace,monospace;background:#14161a;color:#d8dee9;'
        + 'padding:8px 12px;border-top:2px solid #4c566a;z-index:2147483647';
      document.documentElement.appendChild(panel);
    }
    const rows = state.log.map((e) => `${e.method} ${decodeURIComponent(e.path)}${e.body ? ` ${JSON.stringify(e.body)}` : ''}`);
    panel.textContent = `${origin} server (simulated in this page) received:\n${rows.length ? rows.join('\n') : '(nothing yet)'}`;
  }
  render();
})();
