import { createD0 } from './d0-none.js';
import { createD1 } from './d1-rules.js';
import { createD2 } from './d2-ai2rules.js';
import { withExfiltrationRefusal } from './addon-exfiltration.js';

export const DEFENSE_IDS = ['D0', 'D1', 'D2', 'D3'];

// Experiment 1 of AI2-104 (D96): each defense with the exfiltration add-on
// (AI2-101). Run with `npm run experiment:exfil`, never part of the matrix's
// own columns or of RESULTS.md. `<id>+exfil` is `<id>` with the add-on on.
export const EXPERIMENT_DEFENSE_IDS = ['D0+exfil', 'D2+exfil', 'D3+exfil'];

// Add-ons a demo can switch on beside any defense (D96): each is its own
// package (`addons/`), installed on its own, never part of the kernel.
export const ADDON_INFO = {
  exfiltration: {
    name: 'Exfiltration add-on',
    blurb: 'A separate package, outside the kernel. It remembers what a private site returned and spots it in a call to another site, as is or lightly disguised (other case, no dashes, reversed, base64, URL-encoded). With D3 its finding becomes a private label the kernel judges; with the others the host refuses the call.',
  },
};
export const ADDON_IDS = Object.keys(ADDON_INFO);

// How the demo pages describe each defense, kept beside the code that builds it.
export const DEFENSE_INFO = {
  D0: { name: 'No defense', blurb: 'Every tool call the agent makes runs.' },
  D1: { name: 'Rule-based guard', blurb: 'Per-call allow and deny rules, the kind many products ship today.' },
  D2: { name: 'ai2rules kernel', blurb: "The agent sees only its task's tools, and the broker remembers which registration each one is. After it reads CRM data the session is tainted, and writes and external actions are refused." },
  D3: { name: 'ai2rules kernel + value labels', blurb: 'The same, with labels instead of the taint: values read from the CRM are labelled private, and a private value cannot be sent to another site, through any tool.' },
};

// `addons` (D96): add-on ids from ADDON_INFO, on beside the defense. The
// exfiltration add-on rides the label carrier in the D3 world, where a flow
// contract can judge its finding, and the host-refusal carrier everywhere else
// (`addon-exfiltration.js`). In D2 the refusal comes after the kernel's own
// verdict, an approval included; it can only meet an ASK in a session that has
// read nothing private, where the add-on has nothing to find.
export function createDefense(id, { workDir, vouch, approvals, approve, addons = [] }) {
  if (id.endsWith('+exfil')) return createDefense(id.slice(0, -'+exfil'.length), { workDir, vouch, approvals, approve, addons: [...addons, 'exfiltration'] });
  for (const a of addons) if (!ADDON_INFO[a]) throw new Error(`unknown add-on ${a}`);
  const exfiltration = addons.includes('exfiltration');
  if (id === 'D3') return createD2({ workDir, labels: true, vouch, approvals, approve, exfiltration });
  const base = id === 'D0' ? createD0()
    : id === 'D1' ? createD1()
      : id === 'D2' ? createD2({ workDir, approvals, approve })
        : null;
  if (!base) throw new Error(`unknown defense ${id}`);
  return exfiltration ? withExfiltrationRefusal(base) : base;
}
