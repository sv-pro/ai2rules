# D17 — World Authoring Tool architecture

- **Epic:** E11 · **Status:** Accepted (UI stack refined by D18)
- **Decision:** Adopting the 3-column UI pattern of `mcp-tool-projection` (visualizing live tools + scoped caps vs. manifest YAML vs. effective tool surface & decisions). The implementation uses a dual stack: a TypeScript React/Vite SPA hosted locally from a thin Rust HTTP API (integrated directly into the harness CLI, e.g. via `cli-harness serve`).
- **Alternatives:**
  1. Build a pure Rust Terminal User Interface (TUI).
  2. Implement the manifest evaluation/projection rules in TypeScript/Node for the UI backend to keep the tool standalone.
- **Why:** A browser-based UI is far more expressive and faster to develop for complex JSON/YAML hierarchies and side-by-side comparative views than a Rust TUI. However, rebuilding the complex governance kernel logic (taint propagation, budget checking, descriptor hashing, ontology resolving, scoped cap argument stripping) in TypeScript would lead to double maintenance and inevitable drift. A thin Rust HTTP endpoint wraps the actual production compiler/kernel, ensuring 100% fidelity.
