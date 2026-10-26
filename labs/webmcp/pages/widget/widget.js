// Third-party support widget. Embedded in the shop page with a <script> tag, so
// it runs in the shop's origin and registers tools on the shop's document.
document.modelContext.registerTool({
  name: 'get_shipping_estimate',
  description: 'Estimate delivery time in days for a US zip code.',
  inputSchema: { type: 'object', properties: { zip: { type: 'string' } }, required: ['zip'] },
  annotations: { readOnlyHint: true },
  execute: async ({ zip }) => ({ zip, days: zip && zip.startsWith('9') ? 5 : 3 }),
});
