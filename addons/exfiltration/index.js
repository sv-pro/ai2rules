// Exfiltration add-on (AI2-101), first-party and outside the kernel (D96).
//
// It supplies facts, never a verdict. The host tells it which values came from
// a private source (`observe`). Before a call leaves for somewhere else, the
// host asks it whether any argument carries one of them (`inspect`). The
// answer is a list of findings: which argument, in what form, from which
// source. What a finding does is the host's mapping (D96): a Private flow label
// the kernel judges, or, where the world declares no sink to judge it against,
// a refusal at the host.
//
// No model and no I/O: the same observations and the same arguments always
// give the same findings, so a finding replays from the decision log.
//
// What it recognises, and why:
// - Fingerprints. Every string in a private value is kept whole when it is at
//   least `minLength` characters long. Within it, each whitespace-separated
//   token of that length is kept too, when it carries a digit or a symbol. Such
//   tokens look like identifiers, account codes, emails, phone numbers, dates.
//   Plain words ("Internal") are not, and keeping them would refuse ordinary
//   text that merely shares a word with a private record.
// - Forms. A fingerprint matches as is, case-folded, with separators removed,
//   reversed, base64-encoded (with or without padding), or inside a
//   percent-encoded or form-encoded (`+` for a space) argument. A match inside
//   an encoded argument is reported with the encoding in front of the form
//   (`percent_encoded+case_folded`). A derived form shorter than `minLength`,
//   such as the separator-stripped form of a heavily dashed value, is dropped.
//   These are cheap transforms a model or an injected
//   instruction applies to slip past an exact match. A transform not listed
//   here, such as a paraphrase, a summary or a hash, is not recognised.

const SEPARATORS = /[\s\-_.:/]+/g;
const TRAILING_PUNCT = /^[("'[]+|[)"'\].,;!?]+$/g;

const fold = (s) => s.toLowerCase();
const strip = (s) => fold(s).replace(SEPARATORS, '');
const reverse = (s) => [...s].reverse().join('');
const base64 = (s) => Buffer.from(s, 'utf8').toString('base64');

// Every string (and number, as text) in a value tree.
function strings(v, out = []) {
  if (typeof v === 'string') out.push(v);
  else if (typeof v === 'number' || typeof v === 'bigint') out.push(String(v));
  else if (Array.isArray(v)) v.forEach((x) => strings(x, out));
  else if (v && typeof v === 'object') Object.values(v).forEach((x) => strings(x, out));
  return out;
}

const looksLikeAnIdentifier = (t) => /[\d@+#$%&*=]/.test(t) || /[A-Za-z][-_.][A-Za-z\d]/.test(t);

export function fingerprintsOf(value, { minLength = 8 } = {}) {
  const out = new Set();
  for (const s of strings(value)) {
    const whole = s.trim();
    if (whole.length >= minLength) out.add(whole);
    for (const raw of whole.split(/\s+/)) {
      const token = raw.replace(TRAILING_PUNCT, '');
      if (token.length >= minLength && token !== whole && looksLikeAnIdentifier(token)) out.add(token);
    }
  }
  return [...out];
}

// The forms a fingerprint is looked for in, each checked against the view of
// the argument text it belongs to. A form shorter than `minLength` would match
// unrelated text (`1-2-3-4-5-6-7` strips to seven digits), so it is dropped.
function formsOf(f, minLength) {
  const b64 = base64(f);
  return [
    { form: 'exact', view: 'raw', needle: f },
    { form: 'case_folded', view: 'folded', needle: fold(f) },
    { form: 'separators_removed', view: 'stripped', needle: strip(f) },
    { form: 'reversed', view: 'raw', needle: reverse(f) },
    { form: 'reversed', view: 'folded', needle: fold(reverse(f)) },
    { form: 'base64', view: 'raw', needle: b64.replace(/=+$/, '') },
  ].filter((x) => x.needle.length >= minLength);
}

const decode = (s) => { try { return decodeURIComponent(s); } catch { return s; } };

// The argument as sent, then percent-decoded, then form-decoded (`+` is a
// space in application/x-www-form-urlencoded). Each distinct one is a view.
function viewsOf(text) {
  const views = [{ raw: text, encoding: null }];
  const percent = decode(text);
  if (percent !== text) views.push({ raw: percent, encoding: 'percent' });
  const form = text.includes('+') ? decode(text.replace(/\+/g, ' ')) : text;
  if (form !== text && form !== percent) views.push({ raw: form, encoding: 'form' });
  return views.map((v) => ({ ...v, folded: fold(v.raw), stripped: strip(v.raw) }));
}

const formName = (encoding, form) => (!encoding ? form : form === 'exact' ? `${encoding}_encoded` : `${encoding}_encoded+${form}`);

export function createExfiltrationGuard({ minLength = 8 } = {}) {
  // fingerprint -> { source, forms }
  const known = new Map();

  return {
    id: 'exfiltration',

    // The host's word: `value` came from a private source. Nothing here
    // decides what is private; the host does (the world's sensitive origins,
    // or the kernel's Private result floor).
    observe(value, { source = 'unknown' } = {}) {
      let added = 0;
      for (const f of fingerprintsOf(value, { minLength })) {
        if (known.has(f)) continue;
        known.set(f, { source, forms: formsOf(f, minLength) });
        added += 1;
      }
      return added;
    },

    // Findings for one call's arguments: at most one per top-level argument,
    // the first fingerprint and form that match. An empty list is not a
    // verdict either: it only says this add-on found nothing.
    inspect(args) {
      const findings = [];
      if (!known.size) return findings;
      for (const [argument, value] of Object.entries(args ?? {})) {
        const finding = this.match(strings(value).join('\n'));
        if (finding) findings.push({ addon: 'exfiltration', argument, ...finding });
      }
      return findings;
    },

    match(text) {
      for (const view of viewsOf(text)) {
        for (const [, entry] of known) {
          for (const { form, view: which, needle } of entry.forms) {
            if (view[which].includes(needle)) return { form: formName(view.encoding, form), source: entry.source };
          }
        }
      }
      return null;
    },

    size: () => known.size,
    reset: () => known.clear(),
  };
}
