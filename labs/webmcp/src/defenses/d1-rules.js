// D1 — rule-based per-call baseline: a reconstruction of publicly described
// enterprise WebMCP controls (allow / monitor / block at invocation, matching on
// tool name, provider origin, cross-origin flag, argument patterns and the
// untrustedContentHint annotation). Not any vendor's product.
//
// Like those controls, it decides each call on what the call itself shows. The
// agent still sees every registered tool. Rules: d1-rules.json; rationale:
// docs/D1-RULES.md.
import { readFileSync } from 'node:fs';

const CONFIG = JSON.parse(readFileSync(new URL('./d1-rules.json', import.meta.url), 'utf8'));

function compile(src) {
  const ci = src.startsWith('(?i)');
  return new RegExp(ci ? src.slice(4) : src, ci ? 'gi' : 'g');
}

const PATTERNS = Object.fromEntries(
  Object.entries(CONFIG.patterns).map(([k, list]) => [k, list.map(compile)]),
);

function luhn(digits) {
  let sum = 0;
  for (let i = 0; i < digits.length; i++) {
    let d = Number(digits[digits.length - 1 - i]);
    if (i % 2 === 1) { d *= 2; if (d > 9) d -= 9; }
    sum += d;
  }
  return sum % 10 === 0;
}

function hasCard(text) {
  for (const m of text.matchAll(/\b(?:\d[ -]?){13,19}\b/g)) {
    const digits = m[0].replace(/\D/g, '');
    if (digits.length >= 13 && digits.length <= 19 && luhn(digits)) return true;
  }
  return false;
}

export function argsMatch(kind, text) {
  if (kind === 'card') return hasCard(text);
  return (PATTERNS[kind] ?? []).some((re) => { re.lastIndex = 0; return re.test(text); });
}

export function evaluate(call, { focusOrigin }) {
  const { tool, args } = call;
  const text = JSON.stringify(args ?? {});
  const facts = {
    origin_allowlisted: CONFIG.allowlisted_origins.includes(tool.origin),
    read_only: tool.annotations?.readOnlyHint === true,
    cross_origin: focusOrigin != null && focusOrigin !== tool.origin,
    untrusted_content_hint: tool.annotations?.untrustedContentHint === true,
  };
  const matched = [];
  for (const rule of CONFIG.rules) {
    const w = rule.when;
    const ok = Object.entries(w).every(([k, v]) => {
      if (k === 'args_match') return v.some((kind) => argsMatch(kind, text));
      if (k === 'name_matches') return new RegExp(v, 'i').test(tool.name);
      return facts[k] === v;
    });
    if (ok) matched.push(rule);
  }
  const block = matched.find((r) => r.action === 'block');
  if (block) return { decision: 'DENY', mechanism: `rule:${block.id}`, matched: matched.map((r) => r.id) };
  if (matched.length) return { decision: 'MONITOR', mechanism: null, matched: matched.map((r) => r.id) };
  return { decision: CONFIG.default === 'allow' ? 'ALLOW' : 'DENY', mechanism: null, matched: [] };
}

export function createD1() {
  return {
    id: 'D1',
    label: 'rule-based per-call baseline',
    async init({ trace }) { trace.write('defense_init', { defense: 'D1', rules_version: CONFIG.version }); },
    async listTools(registered) { return registered; },
    async beforeCall(call, ctx) { return evaluate(call, ctx); },
  };
}
