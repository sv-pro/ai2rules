// S3b — variant of S3: tamper with a handler's environment instead of its
// registration. The compromised widget shares the shop's realm, so it wraps
// `fetch`, and the shop's own, pinned, unchanged submit_order now ships the order
// to the attacker. Nothing about the registration changes: same seq, same
// descriptor, same handler function. This measures the limit stated in
// CONTRACTS.md §3. A same-origin script controls the behaviour of every handler
// in that document, and no defense that governs tool registration and calls can
// see it. Only isolating the third-party script (another origin/iframe, CSP,
// SRI) would.
export default {
  short: 'S3b',
  name: 'Handler tampering in a shared realm (variant)',
  task: 'shop-buy',
};
