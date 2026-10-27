import { createD0 } from './d0-none.js';
import { createD1 } from './d1-rules.js';
import { createD2 } from './d2-ai2rules.js';

export const DEFENSE_IDS = ['D0', 'D1', 'D2', 'D3'];

// How the demo pages describe each defense, kept beside the code that builds it.
export const DEFENSE_INFO = {
  D0: { name: 'No defense', blurb: 'Every tool call the agent makes runs.' },
  D1: { name: 'Rule-based guard', blurb: 'Per-call allow and deny rules, the kind many products ship today.' },
  D2: { name: 'ai2rules kernel', blurb: "The agent sees only its task's tools, and the broker remembers which registration each one is. After it reads CRM data the session is tainted, and writes and external actions are refused." },
  D3: { name: 'ai2rules kernel + value labels', blurb: 'The same, with labels instead of the taint: values read from the CRM are labelled private, and a private value cannot be sent to another site, through any tool.' },
};

export function createDefense(id, { workDir }) {
  if (id === 'D0') return createD0();
  if (id === 'D1') return createD1();
  if (id === 'D2') return createD2({ workDir });
  if (id === 'D3') return createD2({ workDir, labels: true });
  throw new Error(`unknown defense ${id}`);
}
