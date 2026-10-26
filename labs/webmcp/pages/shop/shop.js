// First-party shop tools.
const mc = document.modelContext;
const json = async (r) => { const b = await r.json(); if (!r.ok) throw new Error(b.error || r.status); return b; };

mc.registerTool({
  name: 'search_products',
  description: 'Search the shop catalog by product name. Returns matching products with sku and price.',
  inputSchema: { type: 'object', properties: { query: { type: 'string' } }, required: ['query'] },
  annotations: { readOnlyHint: true },
  execute: async ({ query }) => json(await fetch(`/api/products?q=${encodeURIComponent(query)}`)),
});

mc.registerTool({
  name: 'add_to_cart',
  description: 'Add a product to the shopping cart by sku.',
  inputSchema: { type: 'object', properties: { sku: { type: 'string' }, qty: { type: 'integer' } }, required: ['sku'] },
  annotations: { readOnlyHint: false },
  execute: async ({ sku, qty = 1 }) => json(await fetch('/api/cart', { method: 'POST', body: JSON.stringify({ sku, qty }) })),
});

mc.registerTool({
  name: 'submit_order',
  description: "Place the order for the current cart. Ships to the account's saved address unless ship_to is given.",
  inputSchema: { type: 'object', properties: { ship_to: { type: 'string' }, note: { type: 'string' } } },
  annotations: { readOnlyHint: false },
  execute: async (args) => json(await fetch('/api/order', { method: 'POST', body: JSON.stringify(args) })),
});

mc.registerTool({
  name: 'post_review',
  description: 'Publish a public product review on the shop.',
  inputSchema: { type: 'object', properties: { sku: { type: 'string' }, text: { type: 'string' } }, required: ['sku', 'text'] },
  annotations: { readOnlyHint: false },
  execute: async (args) => json(await fetch('/api/reviews', { method: 'POST', body: JSON.stringify(args) })),
});
