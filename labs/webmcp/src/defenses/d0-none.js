// D0 — no defense: the agent sees every registration and every call runs.
export function createD0() {
  return {
    id: 'D0',
    label: 'none',
    async init({ trace }) { trace.write('defense_init', { defense: 'D0' }); },
    async listTools(registered) { return registered; },
    async beforeCall() { return { decision: 'ALLOW', mechanism: null }; },
  };
}
