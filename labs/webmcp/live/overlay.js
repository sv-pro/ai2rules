// The banner the agent's window shows for each call (AI2-95). It runs in the
// page, through Playwright, and must stay invisible to the agent: it lives in a
// shadow root on an element appended to <html>, outside <body>, so the page text
// the agent reads (`innerText('body')`, browser.js) never contains it. A test
// pins that (test/live.test.js).
//
// Self-contained on purpose: Playwright serializes the function's source.
export function OVERLAY(msg) {
  const ID = '__ai2rules_live_overlay';
  let host = document.getElementById(ID);
  if (!host) {
    host = document.createElement('div');
    host.id = ID;
    host.setAttribute('aria-hidden', 'true');
    host.style.cssText = 'position:fixed;left:16px;right:16px;bottom:16px;z-index:2147483647;pointer-events:none;';
    host.attachShadow({ mode: 'open' }).innerHTML = `<style>
      .bar { font: 15px/1.4 system-ui, -apple-system, "Segoe UI", sans-serif; border-radius: 10px; padding: 10px 14px;
             box-shadow: 0 6px 24px rgba(0,0,0,.25); background: #1d1d1b; color: #fff; display: flex; gap: 10px; align-items: center; }
      .chip { font-weight: 700; font-size: 12px; letter-spacing: .04em; padding: 2px 8px; border-radius: 4px; }
      .ALLOW { background: #1f7a44; } .MONITOR { background: #8a5a00; } .DENY { background: #b3261e; } .ABSENT { background: #555; }
      .why { opacity: .8; font-size: 13px; }
      code { font: 13px ui-monospace, Consolas, monospace; }
    </style><div class="bar"></div>`;
    document.documentElement.appendChild(host);
  }
  const bar = host.shadowRoot.querySelector('.bar');
  const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  if (msg.decision) {
    const label = { ALLOW: 'allowed', MONITOR: 'allowed, flagged', DENY: 'refused', ABSENT: 'does not exist for the agent' }[msg.decision] ?? msg.decision;
    bar.innerHTML = `<span>Agent calls <code>${esc(msg.tool)}</code></span><span class="chip ${esc(msg.decision)}">${esc(msg.decision)}</span><span>${esc(label)}</span>${msg.why ? `<span class="why">${esc(msg.why)}</span>` : ''}`;
  } else {
    bar.insertAdjacentHTML('beforeend', `<span class="why">${msg.ran ? (msg.ok ? '· ran' : '· ran, failed') : '· never ran'}</span>`);
  }
}
