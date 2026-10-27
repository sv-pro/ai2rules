// The shop's UI for a person (live demo, AI2-99). It calls the same endpoints the
// shop's tools call, marking each request as the person's, so the live demo can
// show it as theirs. A block, not top-level declarations: classic scripts share
// the page's global scope with shop.js and the widget.
{
  const PERSON = { 'x-lab-actor': 'person' };
  const $ = (id) => document.getElementById(id);
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  const say = (text) => { $('msg').textContent = text; };

  async function call(path, init = {}) {
    const res = await fetch(path, { ...init, headers: { ...PERSON, ...(init.headers ?? {}) } });
    const body = await res.json().catch(() => ({}));
    if (!res.ok) throw new Error(body.error ?? `HTTP ${res.status}`);
    return body;
  }

  async function showCart() {
    // Read without the mark: refreshing the view is not something the person did.
    const { cart } = await (await fetch('/api/cart')).json();
    $('cart').innerHTML = cart.length
      ? cart.map((i) => `<li><span>${esc(i.sku)} × ${esc(i.qty)}</span><span>${i.by === 'person' ? 'you' : 'the agent'}</span></li>`).join('')
      : '<li>Empty.</li>';
  }

  $('search').addEventListener('submit', async (e) => {
    e.preventDefault();
    try {
      const items = await call(`/api/products?q=${encodeURIComponent($('q').value)}`);
      $('results').innerHTML = items.length
        ? items.map((p) => `<li><span>${esc(p.name)} — $${esc(p.price)}</span><button class="secondary" data-sku="${esc(p.sku)}">Add to cart</button></li>`).join('')
        : '<li>No products match.</li>';
    } catch (err) { say(String(err.message)); }
  });

  $('results').addEventListener('click', async (e) => {
    const sku = e.target?.dataset?.sku;
    if (!sku) return;
    try {
      await call('/api/cart', { method: 'POST', body: JSON.stringify({ sku, qty: 1 }) });
      say(`Added ${sku}.`);
      showCart();
    } catch (err) { say(String(err.message)); }
  });

  $('order').addEventListener('click', async () => {
    try {
      const o = await call('/api/order', { method: 'POST', body: '{}' });
      say(`Order ${o.order_id} placed, shipping to ${o.ship_to}.`);
      showCart();
    } catch (err) { say(String(err.message)); }
  });

  showCart();
  // The agent may change the cart too, through the shop's tools.
  setInterval(showCart, 2000);
}
