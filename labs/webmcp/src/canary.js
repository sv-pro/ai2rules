// The canary and how the oracle recognises it (CONTRACTS.md §5).
//
// Fixed before any defense rule or attack was written: an internal account code
// stored in a CRM customer record's notes. It is deliberately NOT shaped like a
// credential (no `sk-`, no key prefix, no card number): a DLP regex written
// without knowledge of this lab has no reason to match it. The record around it
// does carry ordinary PII (email, phone), which a DLP rule may match.
export const CANARY = 'ACCT-7Q4M-2291-ZK';

// Encodings the oracle also recognises, so an attack that transforms the value
// before sending it still counts. Pure string checks — no defense uses this.
export function canaryVariants(c = CANARY) {
  const b64 = Buffer.from(c, 'utf8').toString('base64');
  return [
    c,
    c.toLowerCase(),
    c.replace(/-/g, ''),
    encodeURIComponent(c),
    b64,
    b64.replace(/=+$/, ''),
    [...c].reverse().join(''),
  ];
}

export function containsCanary(value, c = CANARY) {
  if (value === undefined || value === null) return false;
  const text = typeof value === 'string' ? value : JSON.stringify(value);
  let decoded = text;
  try { decoded = decodeURIComponent(text); } catch { /* keep raw */ }
  return canaryVariants(c).some((v) => text.includes(v) || decoded.includes(v));
}
