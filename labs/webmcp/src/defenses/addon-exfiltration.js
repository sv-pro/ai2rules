// Experiment 1 of AI2-104 (D96): the exfiltration add-on (AI2-101) on the
// host side of the lab. The add-on lives outside the lab and the kernel
// (`addons/exfiltration/`), and this file is the host's mapping of its facts.
//
// Two carriers, and the experiment compares them:
// - Label (D3 world, `createD2({ labels: true, exfiltration })`): a finding
//   raises the argument's flow label to Private, and the kernel's own flow
//   contract decides (D95). The add-on's fact meets a rule the world declares.
// - Refusal (worlds that declare no sink, here D0 and D2): there is no rule in
//   the world for a label to meet, so the host composes the add-on beside the
//   defense instead. Most restrictive wins: an ALLOW with a finding becomes a
//   DENY, and nothing else changes. This is D96's "beside, no coupling" option,
//   kept honest here as the fallback.
//
// Either way the host decides what is private: values returned by a call to
// one of the task's sensitive origins, the same input the D2 and D3 worlds are
// compiled from. Only calls to other origins are inspected. The add-on never
// sees the oracle's canary.
import { createExfiltrationGuard } from '../../../../addons/exfiltration/index.js';

export { createExfiltrationGuard };

// Feed the add-on what the host knows is private: an executed call's result on
// a sensitive origin (its error text too, which is a returned value as well).
export function observePrivate(guard, task, call, res, trace) {
  if (!task?.sensitiveOrigins?.includes(call.tool.origin)) return;
  const value = res?.ok ? res.result : res?.error;
  if (value === undefined) return;
  const added = guard.observe(value, { source: `${call.tool.origin}/${call.tool.name}` });
  trace.write('addon_observe', { addon: guard.id, source: `${call.tool.origin}/${call.tool.name}`, fingerprints: added });
}

// The add-on's findings for a call leaving for a non-sensitive origin.
export function findingsFor(guard, task, call, trace) {
  if (task?.sensitiveOrigins?.includes(call.tool.origin)) return [];
  const findings = guard.inspect(call.args);
  if (findings.length) trace.write('addon_finding', { addon: guard.id, tool: `${call.tool.origin}/${call.tool.name}`, findings });
  return findings;
}

// The refusal carrier: `inner` decides first, then an ALLOW meets the add-on's
// findings. Every other verdict passes through as it is.
export function withExfiltrationRefusal(inner) {
  const guard = createExfiltrationGuard();
  let task = null;
  return {
    // The inner defense keeps its own `this` (D2 holds per-call state on it).
    listTools: (registered, ctx) => inner.listTools(registered, ctx),
    onIngest: async (event, ctx) => inner.onIngest?.(event, ctx),
    id: `${inner.id}+exfil`,
    label: `${inner.label} + exfiltration add-on (host refusal)`,
    async init(ctx) {
      task = ctx.task;
      guard.reset();
      await inner.init(ctx);
    },
    async beforeCall(call, ctx) {
      const verdict = await inner.beforeCall(call, ctx);
      if (verdict.decision !== 'ALLOW' && verdict.decision !== 'MONITOR') return verdict;
      const findings = findingsFor(guard, task, call, ctx.trace);
      if (!findings.length) return verdict;
      const f = findings[0];
      return {
        decision: 'DENY',
        mechanism: 'addon:exfiltration',
        reason: `argument ${f.argument} carries a private value from ${f.source} (${f.form})`,
        addon: findings,
        inner: { decision: verdict.decision, mechanism: verdict.mechanism ?? null },
      };
    },
    async afterCall(call, res, ctx) {
      observePrivate(guard, task, call, res, ctx.trace);
      await inner.afterCall?.(call, res, ctx);
    },
  };
}
