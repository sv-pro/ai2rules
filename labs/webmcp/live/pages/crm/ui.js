// The CRM's UI for a person (live demo, AI2-99): open a customer record. Its
// request is marked as the person's, so the live demo shows it as theirs. A
// block, not top-level declarations: classic scripts share the page's scope.
{
  const $ = (id) => document.getElementById(id);
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

  $('lookup').addEventListener('submit', async (e) => {
    e.preventDefault();
    const res = await fetch(`/api/customers/${encodeURIComponent($('id').value.trim())}`, { headers: { 'x-lab-actor': 'person' } });
    const rec = await res.json().catch(() => ({}));
    $('record').innerHTML = res.ok
      ? `<dl>${Object.entries(rec).map(([k, v]) => `<dt>${esc(k)}</dt><dd>${esc(typeof v === 'string' ? v : JSON.stringify(v))}</dd>`).join('')}</dl>`
      : `<p>${esc(rec.error ?? `HTTP ${res.status}`)}</p>`;
  });
}
