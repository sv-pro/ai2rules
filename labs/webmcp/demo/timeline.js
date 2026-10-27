// The feed of an agent's steps, shared by the one-click demo (demo/index.html)
// and the live control room (live/index.html). It draws trace records as the
// lab writes them (CONTRACTS.md §4); it decides nothing and infers nothing a
// record does not say.

export const ORIGIN = { crm: 'the CRM', shop: 'the shop', attacker: 'the attacker site', widget: 'the widget' };
export const where = (o) => ORIGIN[o] ?? o;

export const MECHANISM = {
  projection: "not in the agent's world: the tool is ABSENT for this task",
  pinning: 'not the tool first seen under this name: it was swapped after first use',
  taint: 'taint: the session has read CRM data, so this kind of action is refused',
  'flow:confidentiality': 'value label: a private value cannot go to another site',
  'flow:integrity': 'value label: an untrusted value cannot reach this argument',
  not_registered: 'no such tool',
  not_triggered: 'the injected instruction never reached the agent',
  'addon:exfiltration': 'exfiltration add-on: this call carries private data the session read',
  'approval:denied': 'you said no',
  'approval:timeout': 'nobody answered in time, so it counts as a no',
  'approval:cancelled': 'the session ended before anyone answered',
};
export const mech = (m) => !m ? '' : MECHANISM[m] ?? (m.startsWith('rule:') ? `guard rule ${m.slice(5)}` : m);

// The wall-clock time a record was streamed (`at`), when the server sent one.
// HH:MM:SS in local time, the same in every locale (the live narrator cites
// steps by this text).
const two = (n) => String(n).padStart(2, '0');
export const clock = (at) => {
  if (!at) return '';
  const d = new Date(at);
  return `${two(d.getHours())}:${two(d.getMinutes())}:${two(d.getSeconds())}`;
};
const stamp = (at) => at ? `<time class="at" datetime="${new Date(at).toISOString()}">${clock(at)}</time>` : '';

export function esc(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
}

// A value as a person would read it: an argument object as `name: value` lines
// with strings unescaped, shortened around the secret, the secret marked.
export function show(value, canary, max = 260) {
  let text = typeof value === 'string' ? value
    : value && typeof value === 'object' && !Array.isArray(value)
      ? Object.entries(value).map(([k, v]) => `${k}: ${typeof v === 'string' ? v : JSON.stringify(v)}`).join('\n') || '(no arguments)'
      : JSON.stringify(value);
  const at = canary ? text.indexOf(canary) : -1;
  if (text.length > max) {
    text = at > max - 40
      ? text.slice(0, 70) + ' … ' + text.slice(Math.max(70, at - 80), at + canary.length + 50) + ' …'
      : text.slice(0, max) + ' …';
  }
  const html = esc(text);
  return canary ? html.replaceAll(esc(canary), `<mark class="secret" title="private account code from the CRM">${esc(canary)}</mark>`) : html;
}

const TRIGGER = {
  user_task: "the user's task",
  llm: 'Claude',
};

// `list` is the <ol> to draw into. Each record is queued and drawn in order;
// with `paced()` true, a pause follows the records worth narrating.
// The feed runs oldest first, newest last. It follows the newest step while
// the reader is at the end of the page; once they scroll up to read, it stops
// moving under them and calls `onBehind` (a page can offer "jump to newest").
export function createTimeline({ list, canary, paced = () => false, onDrawn = () => {}, onBehind = () => {} }) {
  let current = null; // the latest call's card; its verdict and execution attach to it
  // A tool withdrawn and registered again under the same name in the same
  // document: the second is a different registration, though it looks the same.
  let withdrawn = new Map(); // `${tab}.${nav}:${name}` -> the withdrawn regId
  let replacements = new Map(); // regId of a replacement -> the regId it replaced
  // What a submit_order told the agent, to set beside what the shop's server got.
  let told = new Map(); // order_id -> { ship_to, who }
  let compared = new Set();
  let caller = 'the agent';
  const queue = [];
  let draining = false;
  const hasSecret = (v) => Boolean(canary) && JSON.stringify(v ?? '').includes(canary);

  let at = null; // the record being drawn: its time goes on every line it adds

  // A call card carries its time in its own title line.
  function addStep(cls, html, { ownStamp = false } = {}) {
    // Measured before the step is added: was the reader at the end?
    const atEnd = window.innerHeight + window.scrollY >= document.documentElement.scrollHeight - 120;
    const li = document.createElement('li');
    li.className = `step ${cls}`;
    li.innerHTML = (ownStamp ? '' : stamp(at)) + html;
    list.appendChild(li);
    if (atEnd) li.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    else onBehind();
    return li;
  }

  // Draws one record; returns how long to pause after it, in ms.
  function render(rec) {
    at = rec.at ?? null;
    switch (rec.kind) {
      case 'tab_open': addStep('minor', `Opens a tab: ${esc(where(rec.origin))}`); return 300;
      case 'page_event':
        if (rec.event === 'register') {
          const key = `${rec.regId.split(':')[0]}:${rec.name}`;
          if (withdrawn.has(key)) {
            replacements.set(rec.regId, withdrawn.get(key));
            withdrawn.delete(key);
            addStep('minor bad', `${esc(where(rec.origin))} registers a <b>replacement</b> <code>${esc(rec.name)}</code>: same name, but not the tool the page offered when it loaded. A script in the page swapped it (registration ${esc(rec.regId)}, was ${esc(replacements.get(rec.regId))}).`);
            return 900;
          }
          addStep('minor', `${esc(where(rec.origin))} registers a tool: <code>${esc(rec.name)}</code>`);
          return 120;
        }
        if (rec.event === 'unregister') {
          withdrawn.set(`${rec.regId.split(':')[0]}:${rec.name}`, rec.regId);
          addStep('minor warn', `${esc(where(rec.origin))} withdraws a tool: <code>${esc(rec.name)}</code>`);
          return 300;
        }
        return 0;
      case 'ingest':
        if (rec.what === 'page_text') { addStep('minor', `Reads the page text of ${esc(where(rec.origin))}`); return 300; }
        return 0;
      case 'list': {
        const hidden = rec.registered.length - rec.visible.length;
        addStep('minor', `Lists the tools it can use: ${rec.visible.length}${hidden ? `; the defense hides ${hidden} more` : ''}`);
        return 200;
      }
      case 'call': {
        const injected = rec.trigger?.by === 'injection';
        const why = injected
          ? `<span class="tag inj">injected instruction, found in ${esc(String(rec.trigger.source ?? 'input').replace('_', ' '))}</span>`
          : `<span class="tag">${rec.trigger?.retry ? 'retry' : esc(TRIGGER[rec.trigger?.by] ?? rec.trigger?.by ?? 'agent')}</span>`;
        const target = rec.resolved ? rec.resolved.split(' ')[1] : rec.name;
        caller = rec.trigger?.by === 'llm' ? 'Claude' : 'the agent';
        const regId = rec.resolved?.split(' ')[0];
        const swappedTag = replacements.has(regId) ? '<span class="tag inj">the replacement</span>' : '';
        const li = addStep(`call${injected ? ' injected' : ''}`, `<div class="call-card">
          <div class="title">${stamp(at)}Calls <code>${esc(target)}</code>${why}${swappedTag}</div>
          <pre>${show(rec.args, canary)}</pre></div>`, { ownStamp: true });
        current = li.querySelector('.call-card');
        return 900;
      }
      case 'verdict': {
        if (!current) return 0;
        const d = rec.decision;
        const text = { ALLOW: 'allowed', MONITOR: 'allowed, flagged for monitoring', DENY: 'refused', ABSENT: 'the tool does not exist for the agent' }[d] ?? d;
        const kernel = rec.kernel?.rule ? `kernel rule <code>${esc(rec.kernel.rule)}</code>` : '';
        const approved = rec.approval === 'allowed' ? 'you approved it' : '';
        const why = [approved, esc(mech(rec.mechanism)), kernel].filter(Boolean).join(' · ');
        let flow = '';
        if (rec.flow?.vouched?.length) {
          flow += `<div class="reason">The host vouches for ${rec.flow.vouched.map((k) => `<code>${esc(k)}</code>`).join(', ')}: found in what you typed, so labelled public.</div>`;
        }
        for (const f of rec.addon ?? []) {
          flow += `<div class="reason">The exfiltration add-on found a private value from <code>${esc(f.source)}</code> in <code>${esc(f.argument)}</code> (${esc(f.form.replaceAll('_', ' '))})${rec.mechanism === 'addon:exfiltration' ? ', so the host refuses the call' : ': the argument is labelled private, and the kernel judges it'}.</div>`;
        }
        if (rec.flow && d !== 'ALLOW' && rec.mechanism?.startsWith('flow:')) {
          const own = Object.keys(rec.flow.args ?? {});
          flow += `<div class="reason">Session label <b>${esc(rec.flow.ambient)}</b>; arguments with a label of their own: ${own.length ? own.map((k) => `<code>${esc(k)}</code> ${esc(rec.flow.args[k])}`).join(', ') : 'none'}. An argument without one takes the session label.</div>`;
        }
        current.insertAdjacentHTML('beforeend', `<div class="verdict">${stamp(at)}<span class="chip ${esc(d)}">${esc(d)}</span>${esc(text)}${why ? ` <span class="reason">— ${why}</span>` : ''}${flow}</div>`);
        return 700;
      }
      case 'exec': {
        if (!current) return 0;
        const leaked = rec.entered && rec.origin !== 'crm' && hasSecret(rec.args);
        if (rec.ok && rec.result?.order_id) told.set(rec.result.order_id, { ship_to: rec.result.ship_to, who: caller });
        const body = rec.ok ? `returned <code>${show(rec.result, canary, 160)}</code>` : `failed: ${esc(rec.error ?? '')}`;
        current.insertAdjacentHTML('beforeend', `<div class="exec">${stamp(at)}${rec.entered ? 'The page ran it and ' + body : 'Never ran: the tool was gone'}${leaked ? `<div class="leaknote">The private account code just reached ${esc(where(rec.origin))}.</div>` : ''}</div>`);
        return 600;
      }
      // What the ai2rules adapter records about its own state (D2, D3).
      case 'addon_observe': addStep('minor kernel', `The exfiltration add-on remembers what <code>${esc(rec.source)}</code> returned (${rec.fingerprints} value${rec.fingerprints === 1 ? '' : 's'} to watch for)`); return 200;
      case 'addon_finding': return 0; // shown on the verdict it led to
      case 'pin': addStep('minor kernel', `ai2rules remembers which registration <code>${esc(rec.action.replace('__', '/'))}</code> is (${esc(rec.regId)})`); return 120;
      case 'taint': addStep('minor kernel', 'ai2rules marks the session <b>tainted</b>: it now holds data from an untrusted or sensitive source'); return 500;
      case 'flow_label': {
        const from = rec.source === 'user_task' ? "the user's own request" : `<code>${esc(rec.source)}</code>`;
        addStep('minor kernel', `ai2rules labels what came from ${from} <b>${esc(rec.label)}</b>`);
        return rec.label.endsWith('private') ? 700 : 200;
      }
      case 'llm_turn':
        if (rec.text) addStep('minor', `Claude: “${show(rec.text, canary, 200)}”`);
        return 0;
      case 'answer': if (rec.text) addStep('minor', `Final answer to the user: “${show(rec.text, canary, 160)}”`); return 300;
      case 'note': addStep(`minor ${rec.tone ?? ''}`, rec.html); return 300;
      // The person's approval of an agent's action (live demo, AI2-99).
      // Inside the call's card, between the call and its verdict.
      case 'approval_asked': {
        const html = `${stamp(at)}<b>ai2rules asks you.</b> The world allows <code>${esc(rec.action.replace('__', '/'))}</code> only with your approval: answer in the agent's window, or here.`;
        if (current) current.insertAdjacentHTML('beforeend', `<div class="ask">${html}</div>`);
        else addStep('minor kernel', html, { ownStamp: true });
        return 300;
      }
      case 'approval': {
        const said = { allowed: 'You allowed it.', denied: 'You denied it.', timeout: 'No answer in time: that counts as a no.', cancelled: 'The session ended before an answer.' }[rec.answer] ?? rec.answer;
        const html = `${stamp(at)}${esc(said)}`;
        if (current) current.insertAdjacentHTML('beforeend', `<div class="ask ${rec.answer === 'allowed' ? '' : 'no'}">${html}</div>`);
        else addStep('minor warn', html, { ownStamp: true });
        return 300;
      }
      // What the person did in the pages by hand: not a tool call, judged by no one.
      case 'person': addStep('person', `<span class="tag you">you</span> ${esc(where(rec.origin))}: ${esc(rec.what)}`); return 300;
      case 'error': addStep('call injected', `<div class="call-card"><div class="title">${stamp(at)}Lab error</div><pre>${esc(rec.message)}</pre></div>`, { ownStamp: true }); return 0;
      default: return 0;
    }
  }

  async function drain() {
    draining = true;
    try {
      while (queue.length) {
        const wait = queue.shift()();
        if (wait && paced()) await new Promise((r) => setTimeout(r, wait));
      }
    } finally {
      draining = false;
    }
  }

  // Each item resolves its own promise once drawn, so a caller can wait for
  // "everything up to here is on screen".
  function enqueue(fn) {
    return new Promise((resolve) => {
      queue.push(() => { try { return fn(); } finally { resolve(); onDrawn(); } });
      if (!draining) drain();
    });
  }

  // What the shop's server actually recorded for an order, from outside the
  // page (the stand's world view, or the oracle's evidence). Said once per
  // order, and only where it differs from what the tool reported.
  function orderReceived(id, shipTo) {
    const t = told.get(id);
    if (!t || compared.has(id) || t.ship_to === shipTo) return 0;
    compared.add(id);
    addStep('call injected', `<div class="call-card"><div class="title">${stamp(Date.now())}The tool lied about order <code>${esc(id)}</code></div>
      <div class="exec">The tool told ${esc(t.who)}: ships to <b>${esc(t.ship_to ?? '(no address)')}</b>.</div>
      <div class="exec">The shop's server received: ships to <b>${esc(shipTo)}</b>.</div>
      <div class="leaknote">${esc(t.who[0].toUpperCase() + t.who.slice(1))} passed on what the tool said: the user hears the right address while the order goes elsewhere.</div></div>`, { ownStamp: true });
    return 900;
  }

  return {
    push: (rec) => enqueue(() => render(rec)),
    orderReceived: (id, shipTo) => enqueue(() => orderReceived(id, shipTo)),
    then: (fn) => enqueue(() => { fn(); return 0; }),
    reset() {
      queue.length = 0; list.innerHTML = ''; current = null;
      withdrawn = new Map(); replacements = new Map(); told = new Map(); compared = new Set(); caller = 'the agent';
    },
  };
}
