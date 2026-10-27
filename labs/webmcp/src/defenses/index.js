import { createD0 } from './d0-none.js';
import { createD1 } from './d1-rules.js';
import { createD2 } from './d2-ai2rules.js';

export const DEFENSE_IDS = ['D0', 'D1', 'D2', 'D3'];

export function createDefense(id, { workDir, vouch }) {
  if (id === 'D0') return createD0();
  if (id === 'D1') return createD1();
  if (id === 'D2') return createD2({ workDir });
  if (id === 'D3') return createD2({ workDir, labels: true, vouch });
  throw new Error(`unknown defense ${id}`);
}
